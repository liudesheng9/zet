//! Locks over Connection trees and Topic-side mounts (docs/multi-tree-coordinates-proposal.md §8.1).
//!
//! A Connection tree `~7` is locked like any tree: its root `~7` and its native Cards `~7#k` are the
//! Lock's members (the existing member seal, triggers and pins), and this module adds four
//! separate records for the coordinate data the Lock also promises:
//!
//! 1. **Structure and connection freeze**: `lock_placements` and `lock_connections` keep the exact
//!    tuple of every station (target, parent or anchor, edge, ordinal, gloss, live or missing) and
//!    every tree connection; SQL triggers refuse any change of a locked host's stations,
//!    connections or tree record. A Topic Lock freezes the Topic-side mounts beside its Cards the
//!    same way, also a Lock made before mounts existed.
//! 2. **Mounted target pins**: `lock_placement_pins` names each live mounted Card; its delete is
//!    refused (`pinned-delete`) and so is a change of its address (`pinned-move`). A pin reaches no
//!    further: a Card connected to a mounted Card is not pinned.
//! 3. **Source observation**: each pin keeps the target's authored fingerprint at lock time. An
//!    edit of an unlocked source succeeds; the reader shows that it differs from the lock-time
//!    version. This never blocks and never faults.
//! 4. **Append-only review**: `lock_source_reviews` records that the owner reviewed a source at a
//!    given fingerprint. It never changes the Lock seal and never writes a `reseal` event.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

use crate::Access;
use crate::coord::{self, Handle, TreeRole};

const TABLES_SQL: &str = "
CREATE TABLE IF NOT EXISTS lock_coordinate_seals (
    lock_id          INTEGER NOT NULL PRIMARY KEY REFERENCES locks(lock_id),
    evidence_version INTEGER NOT NULL CHECK(evidence_version = 1),
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

/// The SQL backstop: the active Lock of a host, by its root address (`3/0` for a Topic host, `~7`
/// for a Connection host).
const HOST_ROOT_SQL_NEW: &str = "(CASE NEW.host_kind WHEN 'topic' THEN NEW.host_tree_key || '/0'
    ELSE (SELECT '~' || display_num FROM connection_trees WHERE tree_id = NEW.host_tree_key) END)";
const HOST_ROOT_SQL_OLD: &str = "(CASE OLD.host_kind WHEN 'topic' THEN OLD.host_tree_key || '/0'
    ELSE (SELECT '~' || display_num FROM connection_trees WHERE tree_id = OLD.host_tree_key) END)";

fn triggers_sql() -> String {
    let locked = |root: &str| {
        format!("EXISTS (SELECT 1 FROM locks l WHERE l.unlocked_at IS NULL AND l.root_address = {root})")
    };
    let edge_root = |side: &str| format!("(SELECT '~' || display_num FROM connection_trees WHERE tree_id = {side}.tree_id)");
    let refuse = "BEGIN SELECT RAISE(ABORT, 'zt: card is in a locked tree'); END;";
    format!(
        "
CREATE TRIGGER IF NOT EXISTS coord_lock_placement_insert BEFORE INSERT ON placements
WHEN {new_p} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_placement_update BEFORE UPDATE ON placements
WHEN {old_p} OR {new_p} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_placement_delete BEFORE DELETE ON placements
WHEN {old_p} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_edge_insert BEFORE INSERT ON placement_edges
WHEN {new_e} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_edge_update BEFORE UPDATE ON placement_edges
WHEN {old_e} OR {new_e} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_edge_delete BEFORE DELETE ON placement_edges
WHEN {old_e} {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_tree_change BEFORE UPDATE ON connection_trees
WHEN EXISTS (SELECT 1 FROM locks l WHERE l.unlocked_at IS NULL AND l.root_address = '~' || OLD.display_num) {refuse}
CREATE TRIGGER IF NOT EXISTS coord_lock_tree_delete BEFORE DELETE ON connection_trees
WHEN EXISTS (SELECT 1 FROM locks l WHERE l.unlocked_at IS NULL AND l.root_address = '~' || OLD.display_num) {refuse}
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
      AND l.tree = substr(NEW.location, 1, instr(NEW.location, '#') - 1))
BEGIN SELECT RAISE(ABORT, 'zt: locked tree takes no new cards'); END;
CREATE TRIGGER IF NOT EXISTS coord_lock_native_move_in BEFORE UPDATE OF location ON cards
WHEN NEW.location IS NOT OLD.location AND NEW.location LIKE '~%#%' AND EXISTS (SELECT 1 FROM locks l
      WHERE l.unlocked_at IS NULL AND l.tree = substr(NEW.location, 1, instr(NEW.location, '#') - 1))
BEGIN SELECT RAISE(ABORT, 'zt: locked tree takes no new cards'); END;
CREATE TRIGGER IF NOT EXISTS lock_source_reviews_append_only_update BEFORE UPDATE ON lock_source_reviews
BEGIN SELECT RAISE(ABORT, 'zt: a source review is append-only'); END;
CREATE TRIGGER IF NOT EXISTS lock_source_reviews_append_only_delete BEFORE DELETE ON lock_source_reviews
BEGIN SELECT RAISE(ABORT, 'zt: a source review is append-only'); END;
",
        new_p = locked(HOST_ROOT_SQL_NEW),
        old_p = locked(HOST_ROOT_SQL_OLD),
        new_e = locked(&edge_root("NEW")),
        old_e = locked(&edge_root("OLD")),
    )
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
    conn.execute_batch(TABLES_SQL)?;
    conn.execute_batch(&triggers_sql())?;
    Ok(())
}

fn installed(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'lock_placement_pins')",
        [],
        |r| r.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

/// The root address whose Lock freezes a host: `3/0` for Topic `3`, `~7` for a Connection tree.
fn host_root(conn: &Connection, host_key: &str) -> Result<Option<String>> {
    if crate::valid_topic_id(host_key) {
        return Ok(Some(format!("{host_key}/0")));
    }
    Ok(coord::tree_by_id(conn, host_key)?.map(|tree| tree.handle()))
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

/// Whether an active Lock freezes the placements and connections of this host.
pub(crate) fn host_locked(conn: &Connection, host_key: &str) -> Result<bool> {
    Ok(match host_root(conn, host_key)? {
        Some(root) => active_lock_of_root(conn, &root)?.is_some(),
        None => false,
    })
}

/// Refuse a host write while its tree is locked.
pub(crate) fn ensure_host_unlocked(conn: &Connection, host_key: &str, label: &str) -> Result<()> {
    if host_locked(conn, host_key)? {
        bail!("card_locked: {label} is locked; its stations and connections do not change");
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

/// The stations a Lock of `root` freezes: a Connection tree's stations, or a Topic's side mounts.
fn host_stations(conn: &Connection, root: &crate::Card) -> Result<Option<(String, Vec<coord::Placement>)>> {
    if !coord::installed(conn) {
        return Ok(None);
    }
    if let Some(handle) = Handle::parse(&root.address).filter(|h| h.local.is_none()) {
        let tree = coord::tree_by_num(conn, handle.tree)?.context("Connection tree record is missing")?;
        let mut stations = coord::placements_of_host(conn, &tree.tree_id)?;
        stations.sort_by(|a, b| a.placement_id.cmp(&b.placement_id));
        return Ok(Some((tree.tree_id, stations)));
    }
    if root.is_topic {
        let topic = crate::tree_id(&root.address).to_string();
        let mut stations = coord::placements_of_host(conn, &topic)?;
        stations.sort_by(|a, b| a.placement_id.cmp(&b.placement_id));
        return Ok(Some((topic, stations)));
    }
    Ok(None)
}

fn station_tuple(conn: &Connection, p: &coord::Placement) -> Result<Value> {
    let live = coord::identity(conn, &p.card_id)?.map(|i| i.live).unwrap_or(false);
    Ok(json!([
        "station-v1",
        p.placement_id,
        p.host_kind,
        p.host_tree_key,
        p.card_id,
        p.parent_id,
        p.anchor_card_id,
        p.edge.id(),
        p.ordinal,
        p.gloss,
        if live { "live" } else { "missing" },
    ]))
}

fn connection_tuple(e: &crate::connection::Edge) -> Value {
    json!([
        "connection-v1",
        e.connection_id,
        e.from,
        e.to,
        if e.directed { "directed" } else { "neutral" },
        e.label,
        e.kind,
        e.note,
        e.created_by,
        e.created_at_rev,
        e.updated_by,
        e.updated_at_rev,
    ])
}

fn fp(value: &Value) -> String {
    Sha256::digest(value.to_string().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The coordinate evidence a Lock of `root` captures, or null when the tree has none (a Topic with
/// no side mount, a Literature tree). Bound into the Lock plan hash.
pub(crate) fn snapshot(conn: &Connection, root: &crate::Card) -> Result<Value> {
    let Some((host, stations)) = host_stations(conn, root)? else {
        return Ok(Value::Null);
    };
    if stations.is_empty() {
        return Ok(Value::Null);
    }
    let members: BTreeSet<i64> = {
        let tree = crate::tree_id(&root.address).to_string();
        crate::addresses(conn)?
            .into_iter()
            .filter(|a| crate::is_tree_member(a, &tree))
            .filter_map(|a| crate::load_card(conn, &a).ok().map(|c| c.row_id))
            .collect()
    };
    let mut placements = Vec::new();
    let mut pins = Vec::new();
    let mut missing = Vec::new();
    for p in &stations {
        placements.push(station_tuple(conn, p)?);
        let identity = coord::identity(conn, &p.card_id)?.context("station names an unknown Card")?;
        match identity.card_row.filter(|_| identity.live) {
            Some(row) if !members.contains(&row) => {
                let card = crate::connection::card_by_row(conn, row)?;
                pins.push(json!({
                    "placement_id": p.placement_id,
                    "card_id": p.card_id,
                    "target_row": row,
                    "target_address": card.address,
                    "observed_authored_fp": crate::card::authored_fingerprint(&card)?,
                    "fp_version": 1,
                }));
            }
            Some(_) => {}
            None => missing.push(json!({"placement_id": p.placement_id, "last_address": identity.last_address})),
        }
    }
    let connections: Vec<Value> = if TreeRole::of(root) == TreeRole::Connection {
        crate::connection::edges_of_tree(conn, &host)?
            .iter()
            .map(connection_tuple)
            .collect()
    } else {
        Vec::new()
    };
    let structure = json!({"host": host, "placements": placements, "connections": connections});
    Ok(json!({
        "host_tree_key": host,
        "structure_fp": fp(&structure),
        "placements": placements,
        "connections": connections,
        "pins": pins,
        "missing": missing,
    }))
}

/// Record the coordinate evidence of a Lock just inserted by `tree_lock::lock_tx`.
pub(crate) fn capture(conn: &Connection, lock_id: i64, snapshot: &Value) -> Result<()> {
    if snapshot.is_null() || !installed(conn) {
        return Ok(());
    }
    let revision = crate::link_index::revision(conn)? as i64;
    conn.execute(
        "INSERT INTO lock_coordinate_seals(lock_id, evidence_version, host_tree_key, structure_fp) VALUES(?1, 1, ?2, ?3)",
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

/// The drift state of one station under an active Lock: whether its mounted source differs from
/// the lock-time version, and whether its current version was reviewed. `None` without a pin.
pub(crate) fn drift(conn: &Connection, placement_id: &str) -> Result<Option<Value>> {
    if !installed(conn) {
        return Ok(None);
    }
    let pin: Option<(i64, String, i64, String)> = conn
        .query_row(
            "SELECT p.lock_id, p.card_id, p.target_row, p.observed_authored_fp FROM lock_placement_pins p
             JOIN locks l ON l.lock_id = p.lock_id
             WHERE p.placement_id = ?1 AND l.unlocked_at IS NULL",
            [placement_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((lock_id, card_id, row, observed)) = pin else {
        return Ok(None);
    };
    let card = crate::connection::card_by_row(conn, row)?;
    let current = crate::card::authored_fingerprint(&card)?;
    let reviewed: Option<String> = conn
        .query_row(
            "SELECT reviewed_fp FROM lock_source_reviews WHERE lock_id = ?1 AND placement_id = ?2 ORDER BY review_id DESC LIMIT 1",
            params![lock_id, placement_id],
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
    let placement_text = args.get("placement_id").and_then(Value::as_str).context("missing `placement_id`")?;
    let station = coord::resolve_station(conn, placement_text)?;
    let reviewed_fp = args.get("reviewed_fp").and_then(Value::as_str).context("missing `reviewed_fp`")?;
    let note = args.get("note").and_then(Value::as_str).map(str::trim).filter(|n| !n.is_empty());
    if note.is_some_and(|n| n.chars().count() > 400) {
        bail!("a review note is at most 400 characters");
    }
    let state = drift(conn, &station.placement_id)?.context("this station has no locked source to review (no active Lock pins it)")?;
    if state["current_fp"].as_str() != Some(reviewed_fp) {
        bail!("stale review: the source changed since it was shown; read it again and review the current version");
    }
    let lock_id = state["lock_id"].as_i64().context("lock id")?;
    if let Some(shown) = args.get("lock_id").and_then(Value::as_i64)
        && shown != lock_id
    {
        bail!("stale review: the station is now under Lock {lock_id}, not the Lock {shown} that was shown; review it again");
    }
    conn.execute(
        "INSERT INTO lock_source_reviews(lock_id, placement_id, card_id, reviewed_fp, reviewed_at_rev, reviewed_at, note)
         VALUES(?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?6)",
        params![
            lock_id,
            station.placement_id,
            station.card_id,
            reviewed_fp,
            coord::next_revision(conn)?,
            note
        ],
    )?;
    Ok(json!({"applied": true, "lock_id": lock_id, "placement_id": station.placement_id, "reviewed_fp": reviewed_fp, "changed": true}))
}

/// The coordinate faults of every active Lock, as `(lock_id, kind, subject, detail)`: the stored
/// evidence no longer reproduces its seal, the live stations or connections differ from it, a
/// Lock of a tree with stations has no evidence, or a live mounted target lacks its pin. Each is
/// only possible by tampering, because the triggers refuse the changes.
pub(crate) fn faults(conn: &Connection) -> Result<Vec<(i64, String, String, String)>> {
    let mut out = Vec::new();
    if !installed(conn) || !coord::installed(conn) {
        return Ok(out);
    }
    let mut stmt = conn.prepare(
        "SELECT l.lock_id, l.root_address, s.structure_fp FROM locks l
         LEFT JOIN lock_coordinate_seals s ON s.lock_id = l.lock_id
         WHERE l.unlocked_at IS NULL ORDER BY l.lock_id",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    for (lock_id, root, sealed) in rows {
        let Ok(card) = crate::load_card(conn, &root) else { continue };
        let now = snapshot(conn, &card)?;
        let Some(sealed) = sealed else {
            // A Topic Lock made before any mount exists, or before multi-tree coordinates, holds no
            // coordinate evidence; its mounts are frozen by the host triggers all the same. A
            // Connection-tree Lock always captures its root station.
            if root.starts_with('~') {
                out.push((lock_id, "coordinate-evidence-missing".into(), root.clone(), "the Lock of a Connection tree holds no coordinate evidence".into()));
            }
            continue;
        };
        // The stored evidence reproduces its own seal.
        let placements: Vec<Value> = conn
            .prepare("SELECT snapshot_json FROM lock_placements WHERE lock_id = ?1 ORDER BY placement_id")?
            .query_map([lock_id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|t| serde_json::from_str(&t).unwrap_or(Value::Null))
            .collect();
        let connections: Vec<Value> = conn
            .prepare("SELECT snapshot_json FROM lock_connections WHERE lock_id = ?1 ORDER BY connection_id")?
            .query_map([lock_id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|t| serde_json::from_str(&t).unwrap_or(Value::Null))
            .collect();
        let host: String = conn.query_row(
            "SELECT host_tree_key FROM lock_coordinate_seals WHERE lock_id = ?1",
            [lock_id],
            |r| r.get(0),
        )?;
        let stored = fp(&json!({"host": host, "placements": placements, "connections": connections}));
        if stored != sealed {
            out.push((lock_id, "coordinate-evidence-changed".into(), root.clone(), "the stored stations or connections no longer match the Lock's coordinate seal".into()));
        }
        if now["structure_fp"].as_str() != Some(sealed.as_str()) {
            out.push((lock_id, "coordinate-structure-changed".into(), root.clone(), "the locked stations or connections differ from their Lock-time evidence".into()));
        }
        for pin in now["pins"].as_array().into_iter().flatten() {
            let present: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM lock_placement_pins WHERE lock_id = ?1 AND placement_id = ?2 AND card_id = ?3)",
                params![lock_id, pin["placement_id"].as_str(), pin["card_id"].as_str()],
                |r| r.get(0),
            )?;
            if !present {
                out.push((lock_id, "coordinate-pin-missing".into(), pin["target_address"].as_str().unwrap_or_default().to_string(), "a mounted target of the locked tree has lost its pin".into()));
            }
        }
    }
    Ok(out)
}

/// The coordinate evidence of every active Lock compared with the stored structure: a fault when a
/// frozen station or connection differs (only possible by tampering, since triggers refuse it).
pub(crate) fn integrity(conn: &Connection) -> Result<Value> {
    if !installed(conn) {
        return Ok(json!({"ok": true, "faults": []}));
    }
    let mut stmt = conn.prepare(
        "SELECT s.lock_id, s.host_tree_key, s.structure_fp, l.root_address FROM lock_coordinate_seals s
         JOIN locks l ON l.lock_id = s.lock_id WHERE l.unlocked_at IS NULL",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut faults = Vec::new();
    for (lock_id, _host, stored, root) in rows {
        let card = crate::load_card(conn, &root)?;
        let now = snapshot(conn, &card)?;
        if now["structure_fp"].as_str() != Some(stored.as_str()) {
            faults.push(json!({"lock_id": lock_id, "root": root, "kind": "coordinate_structure_changed"}));
        }
    }
    Ok(json!({"ok": faults.is_empty(), "faults": faults}))
}
