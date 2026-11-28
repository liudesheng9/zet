//! Coordinate evidence for Tree Locks (docs/multi-coordinate-cards-proposal.md §8.3).
//!
//! A v2 Lock seals every Coordinate and connection of its tree. Each live occurrence of a Card
//! filed outside the tree gets its own pin, lock-time authored fingerprint and review history.
//! Pins protect the promised filing address and live Card. Authored drift is advisory.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use crate::Access;
use crate::coord::{self, TreeRecord};

const TABLES_SQL: &str = "
CREATE TABLE IF NOT EXISTS lock_coordinate_seals (
    lock_id          INTEGER NOT NULL PRIMARY KEY REFERENCES locks(lock_id),
    evidence_version INTEGER NOT NULL CHECK(evidence_version = 2),
    host_tree_key    TEXT NOT NULL,
    structure_fp     TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS lock_placements (
    lock_id        INTEGER NOT NULL REFERENCES locks(lock_id),
    placement_id   TEXT NOT NULL,
    card_id        TEXT NOT NULL,
    snapshot_json  TEXT NOT NULL,
    PRIMARY KEY(lock_id, placement_id)
);
CREATE TABLE IF NOT EXISTS lock_connections (
    lock_id        INTEGER NOT NULL REFERENCES locks(lock_id),
    connection_id  TEXT NOT NULL,
    snapshot_json  TEXT NOT NULL,
    PRIMARY KEY(lock_id, connection_id)
);
CREATE TABLE IF NOT EXISTS lock_placement_pins (
    lock_id              INTEGER NOT NULL REFERENCES locks(lock_id),
    placement_id         TEXT NOT NULL,
    card_id              TEXT NOT NULL,
    target_row           INTEGER NOT NULL,
    target_address       TEXT NOT NULL,
    observed_authored_fp TEXT NOT NULL,
    fp_version           INTEGER NOT NULL,
    observed_at_rev      INTEGER NOT NULL,
    PRIMARY KEY(lock_id, placement_id)
);
CREATE INDEX IF NOT EXISTS lock_placement_pins_by_row ON lock_placement_pins(target_row, lock_id);
CREATE INDEX IF NOT EXISTS lock_placement_pins_by_card ON lock_placement_pins(card_id, lock_id);
CREATE TABLE IF NOT EXISTS lock_source_reviews (
    review_id        INTEGER PRIMARY KEY,
    lock_id          INTEGER NOT NULL REFERENCES locks(lock_id),
    placement_id     TEXT NOT NULL,
    card_id          TEXT NOT NULL,
    reviewed_fp      TEXT NOT NULL,
    reviewed_at_rev  INTEGER NOT NULL,
    reviewed_at      TEXT NOT NULL,
    note             TEXT CHECK(note IS NULL OR length(note) <= 400)
);
CREATE INDEX IF NOT EXISTS lock_source_reviews_latest ON lock_source_reviews(lock_id, placement_id, review_id);
";

fn triggers_sql() -> String {
    let locked = |side: &str| {
        format!(
            "EXISTS (SELECT 1 FROM trees t JOIN locks l ON l.unlocked_at IS NULL
                     AND (l.root_address = t.public_key
                          OR (t.role = 'topic' AND l.root_address = t.public_key || '/0'))
                     WHERE t.tree_id = {side}.tree_id)"
        )
    };
    let refuse = "BEGIN SELECT RAISE(ABORT, 'zt: card is in a locked tree'); END;";
    format!(
        "
CREATE TRIGGER IF NOT EXISTS coord_lock_placement_insert BEFORE INSERT ON coordinates
WHEN {new_tree} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_placement_update BEFORE UPDATE ON coordinates
WHEN {old_tree} OR {new_tree} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_placement_delete BEFORE DELETE ON coordinates
WHEN {old_tree} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_edge_insert BEFORE INSERT ON coordinate_edges
WHEN {new_tree} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_edge_update BEFORE UPDATE ON coordinate_edges
WHEN {old_tree} OR {new_tree} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_edge_delete BEFORE DELETE ON coordinate_edges
WHEN {old_tree} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_tree_change BEFORE UPDATE ON trees
WHEN {old_tree} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_tree_delete BEFORE DELETE ON trees
WHEN {old_tree} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_filing_change BEFORE UPDATE OF filing_coord_id ON card_identity
WHEN NEW.filing_coord_id IS NOT OLD.filing_coord_id AND OLD.status = 'live' AND NEW.status = 'live'
BEGIN
    SELECT RAISE(ABORT, 'zt: card is in a locked tree')
    WHERE EXISTS (SELECT 1 FROM lock_members m JOIN locks l ON l.lock_id = m.lock_id
                  WHERE m.row_id = OLD.card_row AND l.unlocked_at IS NULL);
    SELECT RAISE(ABORT, 'zt: card is pinned by a locked tree (pinned-move)')
    WHERE EXISTS (SELECT 1 FROM lock_placement_pins p JOIN locks l ON l.lock_id = p.lock_id
                  WHERE p.card_id = OLD.card_id AND l.unlocked_at IS NULL);
END;
CREATE TRIGGER IF NOT EXISTS coord_pin_no_delete BEFORE DELETE ON cards
WHEN EXISTS (SELECT 1 FROM lock_placement_pins p JOIN locks l ON l.lock_id = p.lock_id
             WHERE p.target_row = OLD.row_id AND l.unlocked_at IS NULL)
BEGIN SELECT RAISE(ABORT, 'zt: card is pinned by a locked tree (pinned-delete)'); END;
CREATE TRIGGER IF NOT EXISTS coord_pin_no_move BEFORE UPDATE OF location, citation_key ON cards
WHEN (NEW.location IS NOT OLD.location OR NEW.citation_key IS NOT OLD.citation_key)
 AND EXISTS (SELECT 1 FROM lock_placement_pins p JOIN locks l ON l.lock_id = p.lock_id
             WHERE p.target_row = OLD.row_id AND l.unlocked_at IS NULL)
BEGIN SELECT RAISE(ABORT, 'zt: card is pinned by a locked tree (pinned-move)'); END;
CREATE TRIGGER IF NOT EXISTS coord_lock_native_insert BEFORE INSERT ON cards
WHEN NEW.location LIKE '~%#%' AND EXISTS (SELECT 1 FROM locks l WHERE l.unlocked_at IS NULL
      AND l.root_address = substr(NEW.location, 1, instr(NEW.location, '#') - 1))
BEGIN SELECT RAISE(ABORT, 'zt: locked tree takes no new cards'); END;
CREATE TRIGGER IF NOT EXISTS coord_lock_native_move_in BEFORE UPDATE OF location ON cards
WHEN NEW.location IS NOT OLD.location AND NEW.location LIKE '~%#%' AND EXISTS (SELECT 1 FROM locks l
      WHERE l.unlocked_at IS NULL AND l.root_address = substr(NEW.location, 1, instr(NEW.location, '#') - 1))
BEGIN SELECT RAISE(ABORT, 'zt: locked tree takes no new cards'); END;
CREATE TRIGGER IF NOT EXISTS lock_source_reviews_append_only_update BEFORE UPDATE ON lock_source_reviews
BEGIN SELECT RAISE(ABORT, 'zt: a source review is append-only'); END;
CREATE TRIGGER IF NOT EXISTS lock_source_reviews_append_only_delete BEFORE DELETE ON lock_source_reviews
BEGIN SELECT RAISE(ABORT, 'zt: a source review is append-only'); END;
",
        new_tree = locked("NEW"),
        old_tree = locked("OLD"),
    )
}

/// Remove the coordinate Lock triggers before reinstallation.
pub(crate) fn drop_triggers(conn: &Connection) -> Result<()> {
    let names = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'trigger'
             AND (name GLOB 'coord_lock_*' OR name GLOB 'coord_pin_*'
                  OR name GLOB 'lock_source_reviews_*')",
        )?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for name in names {
        conn.execute_batch(&format!("DROP TRIGGER \"{}\";", name.replace('"', "\"\"")))?;
    }
    Ok(())
}

/// Install the coordinate Lock tables and triggers (after the Lock tables and coordinate storage).
pub(crate) fn ensure_schema(conn: &Connection) -> Result<()> {
    let locks_exist: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'locks')",
        [],
        |r| r.get(0),
    )?;
    if !locks_exist || !coord::installed(conn) {
        return Ok(());
    }
    conn.execute_batch("SAVEPOINT coord_lock_schema;")?;
    let result = (|| -> Result<()> {
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS coord_lock_meta_no_delete;
             DROP TRIGGER IF EXISTS coord_lock_meta_no_update;
             DROP TRIGGER IF EXISTS coord_lock_meta_no_insert;
             DROP TRIGGER IF EXISTS coord_lock_meta_no_replace;
             DELETE FROM metadata WHERE key = 'coord_v2_min_lock_id';",
        )?;
        drop_triggers(conn)?;
        conn.execute_batch(TABLES_SQL)?;
        conn.execute_batch(&triggers_sql())?;
        conn.execute_batch("RELEASE coord_lock_schema;")?;
        Ok(())
    })();
    if result.is_err() {
        conn.execute_batch("ROLLBACK TO coord_lock_schema; RELEASE coord_lock_schema;")?;
    }
    result
}

pub(crate) fn installed(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'lock_placement_pins')",
        [],
        |r| r.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

fn active_lock_of_root(conn: &Connection, root: &str) -> Result<Option<i64>> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'locks')",
        [],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            "SELECT lock_id FROM locks WHERE root_address = ?1 AND unlocked_at IS NULL",
            [root],
            |r| r.get(0),
        )
        .optional()?)
}

/// Whether an active Lock freezes this tree, addressed by ID or public key.
pub(crate) fn host_locked(conn: &Connection, tree_id: &str) -> Result<bool> {
    let tree = match coord::tree_by_id(conn, tree_id)? {
        Some(tree) => Some(tree),
        None => coord::tree_by_public_key(conn, tree_id)?,
    };
    match tree {
        Some(tree) => Ok(active_lock_of_root(conn, &tree.root_address())?.is_some()),
        None => Ok(false),
    }
}

/// Refuse a host write while its tree is locked.
pub(crate) fn ensure_host_unlocked(conn: &Connection, tree_id: &str, label: &str) -> Result<()> {
    if host_locked(conn, tree_id)? {
        bail!("card_locked: {label} is locked; its Coordinates and connections do not change");
    }
    Ok(())
}

fn pinning_lock(conn: &Connection, card_id: &str) -> Result<Option<(i64, String)>> {
    if !installed(conn) {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            "SELECT p.lock_id, l.root_address FROM lock_placement_pins p JOIN locks l ON l.lock_id = p.lock_id
             WHERE p.card_id = ?1 AND l.unlocked_at IS NULL ORDER BY p.lock_id LIMIT 1",
            [card_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

/// Refuse to delete or move a Card that an active Lock pins as a mounted target.
pub(crate) fn ensure_card_not_pinned(conn: &Connection, card_id: &str, reason: &str) -> Result<()> {
    if let Some((lock_id, root)) = pinning_lock(conn, card_id)? {
        bail!("card_locked ({reason}): this Card is mounted in locked tree `{root}` (lock {lock_id}); unlock that tree first");
    }
    Ok(())
}

/// Refuse a home delete of any Card an active Lock pins as a mounted target (`pinned-delete`).
pub(crate) fn ensure_delete_allowed(conn: &Connection, delete_set: &BTreeSet<String>) -> Result<()> {
    if !installed(conn) {
        return Ok(());
    }
    for address in delete_set {
        let Ok(card) = crate::load_card(conn, address) else { continue };
        let card_id = coord::card_id_of_row(conn, card.row_id)?;
        if let Some((lock_id, root)) = pinning_lock(conn, &card_id)? {
            bail!(
                "card_locked (pinned-delete): `{address}` is mounted in locked tree `{root}` (lock {lock_id}); unlock that tree before deleting it"
            );
        }
    }
    Ok(())
}

/// Refuse an address change of a pinned Card (`pinned-move`).
pub(crate) fn ensure_moves_allowed<'a>(conn: &Connection, moved: impl IntoIterator<Item = &'a String>) -> Result<()> {
    if !installed(conn) {
        return Ok(());
    }
    for address in moved {
        let Ok(card) = crate::load_card(conn, address) else { continue };
        let card_id = coord::card_id_of_row(conn, card.row_id)?;
        if let Some((lock_id, root)) = pinning_lock(conn, &card_id)? {
            bail!(
                "card_locked (pinned-move): `{address}` is mounted in locked tree `{root}` (lock {lock_id}); its address cannot change while that tree is locked"
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Snapshot at Lock time
// ---------------------------------------------------------------------------------------------

fn connection_tuples(conn: &Connection, tree_id: &str) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT connection_id, from_coord_id, to_coord_id, direction, relation_label, relation_kind,
                note, created_by, created_at_rev, updated_by, updated_at_rev
         FROM coordinate_edges WHERE tree_id = ?1 ORDER BY connection_id",
    )?;
    let tuples = stmt.query_map([tree_id], |row| {
        Ok(json!([
            "connection-v2",
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, i64>(8)?,
            row.get::<_, String>(9)?,
            row.get::<_, i64>(10)?,
        ]))
    })?;
    Ok(tuples.collect::<rusqlite::Result<_>>()?)
}

fn fp(value: &Value) -> String {
    Sha256::digest(value.to_string().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn structure_fp(
    host: &str,
    placements: &[Value],
    connections: &[Value],
    pin_tuples: &[Value],
) -> String {
    fp(&json!({"host": host, "placements": placements, "connections": connections, "pins": pin_tuples}))
}

fn card_address(conn: &Connection, row: i64) -> Result<String> {
    Ok(conn.query_row(
        "SELECT COALESCE(location, citation_key) FROM cards WHERE row_id = ?1",
        [row],
        |r| r.get(0),
    )?)
}

fn pin_target(coord_id: &str, card_id: &str, row: i64, address: &str) -> Value {
    json!({
        "placement_id": coord_id,
        "coord_id": coord_id,
        "card_id": card_id,
        "target_row": row,
        "target_address": address,
        "fp_version": 1,
    })
}

fn tree_coordinates(conn: &Connection, tree_id: &str) -> Result<Vec<coord::CoordinateRow>> {
    let mut coordinates = coord::coordinates_of_tree(conn, tree_id)?;
    coordinates.sort_by(|a, b| a.coord_id.cmp(&b.coord_id));
    Ok(coordinates)
}

/// The v2 evidence for any Tree root, bound into the Lock plan hash.
pub(crate) fn snapshot(conn: &Connection, root: &crate::Card) -> Result<Value> {
    if !coord::installed(conn) {
        return Ok(Value::Null);
    }
    let tree = coord::tree_by_public_key(conn, crate::tree_id(&root.address))?
        .with_context(|| format!("tree record is missing for `{}`", root.address))?;
    snapshot_v2(conn, &tree, None)
}

fn snapshot_v2(
    conn: &Connection,
    tree: &TreeRecord,
    stored_observed_fps: Option<&BTreeMap<&str, &str>>,
) -> Result<Value> {
    let coordinates = tree_coordinates(conn, &tree.tree_id)?;
    let coord_ids: BTreeSet<&str> = coordinates.iter().map(|c| c.coord_id.as_str()).collect();
    let mut placements = Vec::with_capacity(coordinates.len());
    let mut pins = Vec::new();
    let mut pin_tuples = Vec::new();
    let mut missing = Vec::new();
    for c in &coordinates {
        let identity =
            coord::identity(conn, &c.card_id)?.context("Coordinate names an unknown Card")?;
        placements.push(json!([
            "coord-v2",
            c.coord_id,
            c.tree_id,
            c.host_workspace.id(),
            c.card_id,
            c.parent_coord_id,
            c.edge.id(),
            c.slot,
            c.gloss,
            if identity.live { "live" } else { "missing" },
        ]));
        let filed_here = identity
            .filing_coord_id
            .as_deref()
            .is_some_and(|id| coord_ids.contains(id));
        match identity.card_row.filter(|_| identity.live) {
            Some(row) if !filed_here => {
                let address = card_address(conn, row)?;
                let mut pin = pin_target(&c.coord_id, &c.card_id, row, &address);
                pin["observed_authored_fp"] = match stored_observed_fps {
                    Some(fps) => json!(fps.get(c.coord_id.as_str()).copied()),
                    None => {
                        let card = crate::load_card(conn, &address)?;
                        json!(crate::card::authored_fingerprint(&card)?)
                    }
                };
                pin_tuples.push(json!([
                    "pin-v2", c.coord_id, c.card_id, row, address,
                    pin["observed_authored_fp"], pin["fp_version"],
                ]));
                pins.push(pin);
            }
            Some(_) => {}
            None => missing.push(json!({
                "coord_id": c.coord_id,
                "placement_id": c.coord_id,
                "last_address": identity.last_address,
            })),
        }
    }
    let connections = connection_tuples(conn, &tree.tree_id)?;
    Ok(json!({
        "evidence_version": 2,
        "host_tree_key": tree.tree_id,
        "structure_fp": structure_fp(&tree.tree_id, &placements, &connections, &pin_tuples),
        "placements": placements,
        "connections": connections,
        "pins": pins,
        "missing": missing,
    }))
}

/// Record the coordinate evidence of a Lock just inserted by `tree_lock::lock_tx`.
pub(crate) fn capture(conn: &Connection, lock_id: i64, snapshot: &Value) -> Result<()> {
    if snapshot.is_null() {
        return Ok(());
    }
    let revision = crate::link_index::revision(conn)? as i64;
    conn.execute(
        "INSERT INTO lock_coordinate_seals(lock_id, evidence_version, host_tree_key, structure_fp) VALUES(?1, 2, ?2, ?3)",
        params![lock_id, snapshot["host_tree_key"].as_str(), snapshot["structure_fp"].as_str()],
    )?;
    for tuple in snapshot["placements"].as_array().into_iter().flatten() {
        conn.execute(
            "INSERT INTO lock_placements(lock_id, placement_id, card_id, snapshot_json) VALUES(?1, ?2, ?3, ?4)",
            params![lock_id, tuple[1].as_str(), tuple[4].as_str(), tuple.to_string()],
        )?;
    }
    for tuple in snapshot["connections"].as_array().into_iter().flatten() {
        conn.execute(
            "INSERT INTO lock_connections(lock_id, connection_id, snapshot_json) VALUES(?1, ?2, ?3)",
            params![lock_id, tuple[1].as_str(), tuple.to_string()],
        )?;
    }
    for pin in snapshot["pins"].as_array().into_iter().flatten() {
        conn.execute(
            "INSERT INTO lock_placement_pins(lock_id, placement_id, card_id, target_row, target_address,
                 observed_authored_fp, fp_version, observed_at_rev)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                lock_id,
                pin["placement_id"].as_str(),
                pin["card_id"].as_str(),
                pin["target_row"].as_i64(),
                pin["target_address"].as_str(),
                pin["observed_authored_fp"].as_str(),
                pin["fp_version"].as_i64(),
                revision,
            ],
        )?;
    }
    Ok(())
}

/// The drift state of one Coordinate under an active Lock: whether its source differs from
/// the lock-time version, and whether its current version was reviewed. `None` without a pin.
pub(crate) fn drift(conn: &Connection, coord_id: &str) -> Result<Option<Value>> {
    if !installed(conn) {
        return Ok(None);
    }
    let pin: Option<(i64, String, i64, String)> = conn
        .query_row(
            "SELECT p.lock_id, p.card_id, p.target_row, p.observed_authored_fp FROM lock_placement_pins p
             JOIN locks l ON l.lock_id = p.lock_id
             WHERE p.placement_id = ?1 AND l.unlocked_at IS NULL",
            [coord_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((lock_id, card_id, row, observed)) = pin else {
        return Ok(None);
    };
    let card = crate::load_card(conn, &card_address(conn, row)?)?;
    let current = crate::card::authored_fingerprint(&card)?;
    let reviewed: Option<String> = conn
        .query_row(
            "SELECT reviewed_fp FROM lock_source_reviews WHERE lock_id = ?1 AND placement_id = ?2 ORDER BY review_id DESC LIMIT 1",
            params![lock_id, coord_id],
            |r| r.get(0),
        )
        .optional()?;
    let differs = current != observed;
    Ok(Some(json!({
        "lock_id": lock_id,
        "card_id": card_id,
        "observed_fp": observed,
        "current_fp": current,
        "differs": differs,
        "reviewed": differs && reviewed.as_deref() == Some(current.as_str()),
        "label": if differs { "Source differs from lock-time version" } else { "Source as at lock time" },
    })))
}

/// Mark a mounted source reviewed at its current version: owner only, under an active Lock, with
/// the fingerprint the owner was shown. Append-only; the Lock seal never changes.
pub(crate) fn review_source(conn: &Connection, access: Access, args: &Value) -> Result<Value> {
    if access != Access::Owner {
        bail!("only the owner marks a locked tree's source reviewed");
    }
    let coord_text = args
        .get("coord_id")
        .or_else(|| args.get("placement_id"))
        .and_then(Value::as_str)
        .context("missing `coord_id` or `placement_id`")?;
    let coordinate = coord::resolve_coord(conn, coord_text)?;
    let reviewed_fp = args.get("reviewed_fp").and_then(Value::as_str).context("missing `reviewed_fp`")?;
    let note = args.get("note").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty());
    if note.is_some_and(|n| n.chars().count() > 400) {
        bail!("a review note is at most 400 characters");
    }
    let state = drift(conn, &coordinate.coord_id)?
        .context("this Coordinate has no locked source to review (no active Lock pins it)")?;
    if state["current_fp"].as_str() != Some(reviewed_fp) {
        bail!("stale review: the source changed since it was shown; read it again and review the current version");
    }
    let lock_id = state["lock_id"].as_i64().context("lock id")?;
    let shown = args
        .get("lock_id")
        .and_then(Value::as_i64)
        .context("a review names the lock_id it was shown under")?;
    if shown != lock_id {
        bail!("stale review: the Coordinate is now under Lock {lock_id}, not the Lock {shown} that was shown; review it again");
    }
    conn.execute(
        "INSERT INTO lock_source_reviews(lock_id, placement_id, card_id, reviewed_fp, reviewed_at_rev, reviewed_at, note)
         VALUES(?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?6)",
        params![
            lock_id,
            coordinate.coord_id,
            coordinate.card_id,
            reviewed_fp,
            coord::next_revision(conn)?,
            note
        ],
    )?;
    Ok(json!({
        "applied": true,
        "lock_id": lock_id,
        "coord_id": coordinate.coord_id,
        "placement_id": coordinate.coord_id,
        "reviewed_fp": reviewed_fp,
        "changed": true,
    }))
}

fn stored_connections(conn: &Connection, lock_id: i64) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT snapshot_json FROM lock_connections WHERE lock_id = ?1 ORDER BY connection_id",
    )?;
    let tuples = stmt.query_map([lock_id], |row| row.get::<_, String>(0))?;
    Ok(tuples
        .map(|tuple| tuple.map(|text| serde_json::from_str(&text).unwrap_or(Value::Null)))
        .collect::<rusqlite::Result<_>>()?)
}

fn stored_placements(conn: &Connection, lock_id: i64) -> Result<(Vec<Value>, bool)> {
    let mut stmt = conn.prepare(
        "SELECT placement_id, card_id, snapshot_json FROM lock_placements
         WHERE lock_id = ?1 ORDER BY placement_id",
    )?;
    let rows = stmt.query_map([lock_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?;
    let mut placements = Vec::new();
    let mut ids_match = true;
    for row in rows {
        let (placement_id, card_id, text) = row?;
        let tuple: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        ids_match &= tuple[1].as_str() == Some(placement_id.as_str())
            && tuple[4].as_str() == Some(card_id.as_str());
        placements.push(tuple);
    }
    Ok((placements, ids_match))
}

fn stored_pin_tuples(conn: &Connection, lock_id: i64) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT placement_id, card_id, target_row, target_address, observed_authored_fp, fp_version
         FROM lock_placement_pins WHERE lock_id = ?1 ORDER BY placement_id",
    )?;
    let rows = stmt.query_map([lock_id], |row| {
        Ok(json!([
            "pin-v2",
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i64>(5)?,
        ]))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Verify stored and live v2 coordinate evidence for every active Lock.
pub(crate) fn faults(conn: &Connection) -> Result<Vec<(i64, String, String, String)>> {
    if !coord::installed(conn) {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT l.lock_id, l.root_address, s.evidence_version, s.host_tree_key, s.structure_fp
         FROM locks l
         LEFT JOIN lock_coordinate_seals s ON s.lock_id = l.lock_id
         WHERE l.unlocked_at IS NULL ORDER BY l.lock_id",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
        )))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    let mut out = Vec::new();
    for (lock_id, root, version, host, sealed) in rows {
        let Some(version) = version else {
            out.push((lock_id, "coordinate-evidence-missing".into(), root,
                "the Lock holds no coordinate evidence for its tree's Coordinates or connections".into()));
            continue;
        };
        if version != 2 {
            out.push((lock_id, "coordinate-evidence-changed".into(), root,
                format!("the Lock has coordinate evidence version {version}, expected version 2")));
            continue;
        }
        let (placements, ids_match) = stored_placements(conn, lock_id)?;
        let connections = stored_connections(conn, lock_id)?;
        let pin_tuples = stored_pin_tuples(conn, lock_id)?;
        let stored = structure_fp(host.as_deref().unwrap_or_default(), &placements, &connections, &pin_tuples);
        if !ids_match || sealed.as_deref() != Some(stored.as_str()) {
            out.push((lock_id, "coordinate-evidence-changed".into(), root.clone(),
                "the stored Coordinates, connections, or source pins no longer match the Lock's coordinate seal".into()));
        }
        let tree = coord::tree_by_public_key(conn, crate::tree_id(&root))?;
        let now = if let Some(tree) = tree {
            let fps = pin_tuples
                .iter()
                .filter_map(|tuple| Some((tuple[1].as_str()?, tuple[5].as_str()?)))
                .collect();
            snapshot_v2(conn, &tree, Some(&fps))?
        } else {
            Value::Null
        };
        // Both sides reuse the recorded source observations so authored drift stays advisory.
        if now["structure_fp"].as_str() != Some(stored.as_str()) {
            out.push((lock_id, "coordinate-structure-changed".into(), root,
                "the locked Coordinates or connections differ from their Lock-time evidence".into()));
        }
        if !now.is_null() {
            verify_pins(conn, lock_id, &now["pins"], &mut out)?;
        }
    }
    Ok(out)
}

fn verify_pins(
    conn: &Connection,
    lock_id: i64,
    pins: &Value,
    out: &mut Vec<(i64, String, String, String)>,
) -> Result<()> {
    let mut stored: BTreeMap<String, (String, i64, String, i64)> = conn
        .prepare("SELECT placement_id, card_id, target_row, target_address, fp_version
                  FROM lock_placement_pins WHERE lock_id = ?1")?
        .query_map([lock_id], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))))?
        .collect::<rusqlite::Result<_>>()?;
    // The pin promises a Card at its row and address. Its current authored text can drift.
    for pin in pins.as_array().into_iter().flatten() {
        let coord_id = pin["coord_id"].as_str().context("pin Coordinate ID")?;
        let address = pin["target_address"].as_str().context("pin target address")?;
        match stored.remove(coord_id) {
            None => out.push((lock_id, "coordinate-pin-missing".into(), address.into(),
                format!("Coordinate `{coord_id}` has lost its source pin"))),
            Some((card_id, row, stored_address, version))
                if pin["card_id"].as_str() != Some(card_id.as_str())
                    || pin["target_row"].as_i64() != Some(row)
                    || address != stored_address.as_str()
                    || pin["fp_version"].as_i64() != Some(version) =>
            {
                out.push((lock_id, "coordinate-pin-changed".into(), address.into(),
                    format!("Coordinate `{coord_id}` has a different source pin")));
            }
            Some(_) => {}
        }
    }
    for (coord_id, (_, _, address, _)) in stored {
        out.push((lock_id, "coordinate-pin-changed".into(), address,
            format!("the stored pin for Coordinate `{coord_id}` has no live external source")));
    }
    Ok(())
}
