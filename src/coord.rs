//! Multi-tree coordinates (docs/multi-tree-coordinates-proposal.md): one Card, one canonical Home,
//! at most one coordinate per tree, and an identifiable author for every claimed relationship.
//!
//! This module owns the storage that sits beside `cards`:
//!
//! - `card_identity`: one immutable `card_id` per Card, kept in step with `cards` by SQL triggers,
//!   retired (never deleted) when its Card is deleted;
//! - `card_handles`: the permanent citation handles `~7` (a Connection root) and `~7#4` (a native
//!   Card issued by Connection tree 7), never reused;
//! - `connection_trees`, `placements` and `placement_edges`: the Connection trees, the occurrences of
//!   Cards in Connection trees and beside Topic Cards, and the tree-local connections between two
//!   occurrences of one Connection tree;
//! - `retired_placements`: the occurrence IDs a write removed, so a `place:<uuid>` bookmark can say
//!   "removed" rather than "unknown".
//!
//! The address grammar is a partition: a Topic Location starts with a digit, a Literature address
//! with an ASCII letter, and a Connection handle or coordinate with `~`. `/` always means tree
//! traversal and `#` always means an immutable handle. Coordinates (`~7/2|a`, `3/2+1`) are
//! navigation only and are refused inside `[[…]]`.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::Workspace;

/// The coordinate storage generation this binary installs and understands.
pub(crate) const SCHEMA_VERSION: &str = "1";
const SCHEMA_KEY: &str = "coordinates_schema_version";
/// The writer capability an upgraded archive requires of every writing connection. A connection
/// of an older `zt` lacks the SQL function, so every guarded write it attempts fails.
const WRITER_EPOCH: i64 = 1;

/// The longest positional gloss, placement note and connection label, in Unicode scalar values.
pub(crate) const GLOSS_MAX: usize = 240;
pub(crate) const NOTE_MAX: usize = 400;
pub(crate) const LABEL_MAX: usize = 32;
pub(crate) const PURPOSE_MAX: usize = 32;

/// The ordinary Relation kinds a tree connection may carry (`term` stays reserved for Term links).
pub(crate) const RELATION_KINDS: [&str; 8] = [
    "rests-on",
    "illustrates",
    "contrasts",
    "relaxes",
    "corresponds",
    "objects",
    "specializes",
    "refers",
];
/// The kinds that hold between two Cards in no particular direction.
const SYMMETRIC_KINDS: [&str; 3] = ["contrasts", "corresponds", "refers"];

// ---------------------------------------------------------------------------------------------
// Tree roles and the address grammar
// ---------------------------------------------------------------------------------------------

/// The three structural tree roles. A Card's Home role follows from its stored address, which is a
/// partition of the address grammar, so no column stores it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum TreeRole {
    Topic,
    Literature,
    Connection,
}

impl TreeRole {
    pub(crate) fn id(self) -> &'static str {
        match self {
            TreeRole::Topic => "topic",
            TreeRole::Literature => "literature",
            TreeRole::Connection => "connection",
        }
    }

    /// The Home role of a stored address (a Location, a Citation key or a Connection handle).
    pub(crate) fn of_address(address: &str) -> TreeRole {
        if address.starts_with('~') {
            return TreeRole::Connection;
        }
        let tree = crate::tree_id(address);
        if crate::valid_topic_id(tree) {
            TreeRole::Topic
        } else {
            TreeRole::Literature
        }
    }

    pub(crate) fn of(card: &crate::Card) -> TreeRole {
        TreeRole::of_address(&card.address)
    }
}

/// A canonical positive decimal: no sign, no leading zero, no zero, no overflow.
fn positive(value: &str) -> Option<u64> {
    if !crate::valid_positive_number(value) {
        return None;
    }
    value.parse::<u64>().ok()
}

/// A parsed permanent handle: `~n` (the root of Connection tree n) or `~n#k` (native Card k).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Handle {
    pub(crate) tree: u64,
    pub(crate) local: Option<u64>,
}

impl Handle {
    pub(crate) fn parse(text: &str) -> Option<Handle> {
        let rest = text.strip_prefix('~')?;
        match rest.split_once('#') {
            Some((tree, local)) => Some(Handle {
                tree: positive(tree)?,
                local: Some(positive(local)?),
            }),
            None => Some(Handle {
                tree: positive(rest)?,
                local: None,
            }),
        }
    }

    pub(crate) fn root(tree: u64) -> String {
        format!("~{tree}")
    }

    pub(crate) fn native(tree: u64, local: u64) -> String {
        format!("~{tree}#{local}")
    }
}

pub(crate) fn is_handle(text: &str) -> bool {
    Handle::parse(text).is_some()
}

/// A Card address that may stand inside `[[…]]`: a Location, a Citation key, or a handle.
pub(crate) fn is_card_address(text: &str) -> bool {
    crate::is_valid_location(text)
        || crate::literature::validate_citation_key(text).is_ok()
        || is_handle(text)
}

/// A navigation coordinate: a station of a Connection tree or a Topic-side mount. Never a Card
/// address: it changes when the tree is reorganized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Coordinate {
    /// `~n/0` (the root) or `~n/<path>` with the legacy Direct/Side arithmetic.
    Connection { tree: u64, path: String },
    /// `<topic home location>+k`.
    TopicMount { anchor: String, ordinal: u64 },
}

impl Coordinate {
    pub(crate) fn parse(text: &str) -> Option<Coordinate> {
        if let Some(rest) = text.strip_prefix('~') {
            let (tree, path) = rest.split_once('/')?;
            let tree = positive(tree)?;
            if path == "0" || crate::is_valid_regular_location(&format!("0/{path}")) {
                return Some(Coordinate::Connection {
                    tree,
                    path: path.to_string(),
                });
            }
            return None;
        }
        let (anchor, ordinal) = text.rsplit_once('+')?;
        let ordinal = positive(ordinal)?;
        let topic = crate::tree_id(anchor);
        if !crate::valid_topic_id(topic) || !crate::is_valid_location(anchor) {
            return None;
        }
        Some(Coordinate::TopicMount {
            anchor: anchor.to_string(),
            ordinal,
        })
    }

    pub(crate) fn text(&self) -> String {
        match self {
            Coordinate::Connection { tree, path } => format!("~{tree}/{path}"),
            Coordinate::TopicMount { anchor, ordinal } => format!("{anchor}+{ordinal}"),
        }
    }
}

pub(crate) fn is_coordinate(text: &str) -> bool {
    Coordinate::parse(text).is_some()
}

/// The hint a refused `[[coordinate]]` carries: cite the Card, or bookmark the station.
pub(crate) fn coordinate_link_hint(target: &str) -> String {
    format!(
        "`{target}` is a navigation coordinate, which changes when its tree is reorganized; cite the Card's canonical address or handle instead (for example `[[~7#4]]` or `[[Hart1995]]`), or bookmark the station with a Markdown link `[label](place:<placement id>)`"
    )
}

/// A `place:<uuid>` bookmark target, if `text` is one.
pub(crate) fn place_bookmark(text: &str) -> Option<&str> {
    let id = text.strip_prefix("place:")?;
    is_uuid(id).then_some(id)
}

pub(crate) fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(b),
        })
}

/// A fresh canonical UUIDv4 string.
pub(crate) fn new_uuid() -> Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| anyhow::anyhow!("OS randomness: {error}"))?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

// ---------------------------------------------------------------------------------------------
// SQL functions and schema
// ---------------------------------------------------------------------------------------------

/// The Card title a stored text states, for the identity registry's retirement snapshot.
fn stored_title(text: &str) -> String {
    crate::parse_card_text(text)
        .or_else(|_| crate::parse_literature_edit_text(text))
        .map(|parsed| parsed.title)
        .unwrap_or_default()
}

/// Register the coordinate SQL functions on one connection. The identity triggers call them, so a
/// connection of an older `zt` (which never registers them) cannot insert, delete, move or retitle
/// a Card of an upgraded archive, and cannot write any coordinate table.
pub(crate) fn register_sql_functions(conn: &Connection) -> Result<()> {
    use rusqlite::functions::FunctionFlags;
    conn.create_scalar_function("zt_uuid_v4", 0, FunctionFlags::SQLITE_UTF8, |_| {
        new_uuid().map_err(|error| rusqlite::Error::UserFunctionError(Box::from(format!("{error:#}"))))
    })
    .context("failed to register zt_uuid_v4")?;
    conn.create_scalar_function(
        "zt_card_title",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let text: Option<String> = ctx.get(0)?;
            Ok(stored_title(text.as_deref().unwrap_or_default()))
        },
    )
    .context("failed to register zt_card_title")?;
    conn.create_scalar_function(
        "zt_writer_epoch",
        0,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |_| Ok(WRITER_EPOCH),
    )
    .context("failed to register zt_writer_epoch")?;
    Ok(())
}

const TABLES_SQL: &str = "
CREATE TABLE IF NOT EXISTS card_identity (
    card_id         TEXT NOT NULL PRIMARY KEY,
    card_row        INTEGER UNIQUE REFERENCES cards(row_id),
    status          TEXT NOT NULL CHECK(status IN ('live', 'retired')),
    last_address    TEXT NOT NULL,
    last_title      TEXT NOT NULL,
    retired_at_rev  INTEGER,
    CHECK(
        (status = 'live'    AND card_row IS NOT NULL AND retired_at_rev IS NULL)
        OR
        (status = 'retired' AND card_row IS NULL     AND retired_at_rev IS NOT NULL)
    )
);
CREATE INDEX IF NOT EXISTS card_identity_live_address
    ON card_identity(last_address) WHERE status = 'live';
CREATE TABLE IF NOT EXISTS card_handles (
    handle          TEXT NOT NULL PRIMARY KEY,
    card_id         TEXT NOT NULL UNIQUE REFERENCES card_identity(card_id),
    origin_tree_id  TEXT NOT NULL,
    local_seq       INTEGER NOT NULL CHECK(local_seq >= 0),
    UNIQUE(origin_tree_id, local_seq)
);
CREATE TABLE IF NOT EXISTS connection_trees (
    tree_id         TEXT NOT NULL PRIMARY KEY,
    display_num     INTEGER NOT NULL UNIQUE CHECK(display_num >= 1),
    workspace       TEXT NOT NULL CHECK(workspace IN ('human', 'agent')),
    root_card_id    TEXT NOT NULL UNIQUE REFERENCES card_identity(card_id),
    purpose         TEXT CHECK(purpose IS NULL OR (length(purpose) BETWEEN 1 AND 32
                        AND instr(purpose, char(10)) = 0 AND instr(purpose, char(13)) = 0)),
    next_local_seq  INTEGER NOT NULL DEFAULT 1 CHECK(next_local_seq >= 1),
    created_at      TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS placements (
    placement_id    TEXT NOT NULL PRIMARY KEY,
    host_kind       TEXT NOT NULL CHECK(host_kind IN ('connection', 'topic')),
    host_tree_key   TEXT NOT NULL,
    card_id         TEXT NOT NULL REFERENCES card_identity(card_id),
    parent_id       TEXT,
    anchor_card_id  TEXT REFERENCES card_identity(card_id),
    edge            TEXT NOT NULL CHECK(edge IN ('root', 'direct', 'side', 'attach')),
    ordinal         INTEGER,
    gloss           TEXT CHECK(gloss IS NULL OR (length(gloss) BETWEEN 1 AND 240
                        AND instr(gloss, char(10)) = 0 AND instr(gloss, char(13)) = 0)),
    FOREIGN KEY (host_tree_key, parent_id) REFERENCES placements(host_tree_key, placement_id)
        DEFERRABLE INITIALLY DEFERRED,
    UNIQUE (host_tree_key, placement_id),
    UNIQUE (host_tree_key, card_id),
    CHECK(
        (host_kind = 'connection' AND anchor_card_id IS NULL AND (
            (edge = 'root'   AND parent_id IS NULL     AND ordinal IS NULL)
         OR (edge = 'direct' AND parent_id IS NOT NULL AND ordinal IS NULL)
         OR (edge = 'side'   AND parent_id IS NOT NULL AND ordinal IS NOT NULL AND ordinal >= 1)
        ))
        OR
        (host_kind = 'topic' AND parent_id IS NULL AND anchor_card_id IS NOT NULL
         AND edge = 'attach' AND ordinal IS NOT NULL AND ordinal >= 1)
    )
);
CREATE UNIQUE INDEX IF NOT EXISTS placements_one_root   ON placements(host_tree_key) WHERE edge = 'root';
CREATE UNIQUE INDEX IF NOT EXISTS placements_one_direct ON placements(host_tree_key, parent_id) WHERE edge = 'direct';
CREATE UNIQUE INDEX IF NOT EXISTS placements_side_ord   ON placements(host_tree_key, parent_id, ordinal) WHERE edge = 'side';
CREATE UNIQUE INDEX IF NOT EXISTS placements_attach_ord ON placements(host_tree_key, anchor_card_id, ordinal) WHERE edge = 'attach';
CREATE INDEX IF NOT EXISTS placements_by_card           ON placements(card_id);
CREATE INDEX IF NOT EXISTS placements_by_parent         ON placements(host_tree_key, parent_id);
CREATE INDEX IF NOT EXISTS placements_by_anchor         ON placements(anchor_card_id) WHERE host_kind = 'topic';
CREATE TABLE IF NOT EXISTS placement_edges (
    connection_id      TEXT NOT NULL PRIMARY KEY,
    tree_id            TEXT NOT NULL REFERENCES connection_trees(tree_id),
    from_placement_id  TEXT NOT NULL,
    to_placement_id    TEXT NOT NULL,
    direction          TEXT NOT NULL CHECK(direction IN ('neutral', 'directed')),
    relation_label     TEXT CHECK(relation_label IS NULL OR (
                           length(relation_label) BETWEEN 1 AND 32
                           AND instr(relation_label, char(10)) = 0 AND instr(relation_label, char(13)) = 0
                       )),
    relation_kind      TEXT CHECK(relation_kind IS NULL OR relation_kind IN (
                           'rests-on','illustrates','contrasts','relaxes',
                           'corresponds','objects','specializes','refers'
                       )),
    note               TEXT CHECK(note IS NULL OR length(note) BETWEEN 1 AND 400),
    created_by         TEXT NOT NULL CHECK(created_by IN ('owner', 'agent')),
    created_at_rev     INTEGER NOT NULL,
    updated_by         TEXT NOT NULL CHECK(updated_by IN ('owner', 'agent')),
    updated_at_rev     INTEGER NOT NULL,
    FOREIGN KEY (tree_id, from_placement_id) REFERENCES placements(host_tree_key, placement_id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (tree_id, to_placement_id)   REFERENCES placements(host_tree_key, placement_id)
        DEFERRABLE INITIALLY DEFERRED,
    CHECK(from_placement_id <> to_placement_id),
    CHECK(direction = 'directed' OR from_placement_id < to_placement_id),
    CHECK(
        relation_kind IS NULL
        OR relation_kind IN ('contrasts', 'corresponds', 'refers')
        OR direction = 'directed'
    )
);
CREATE UNIQUE INDEX IF NOT EXISTS placement_edges_blank_pair
    ON placement_edges(tree_id, from_placement_id, to_placement_id)
    WHERE direction = 'neutral' AND relation_label IS NULL AND relation_kind IS NULL AND note IS NULL;
CREATE INDEX IF NOT EXISTS placement_edges_by_from ON placement_edges(tree_id, from_placement_id);
CREATE INDEX IF NOT EXISTS placement_edges_by_to   ON placement_edges(tree_id, to_placement_id);
CREATE TABLE IF NOT EXISTS retired_placements (
    placement_id   TEXT NOT NULL PRIMARY KEY,
    host_tree_key  TEXT NOT NULL,
    card_id        TEXT NOT NULL REFERENCES card_identity(card_id),
    retired_at_rev INTEGER NOT NULL
);
";

/// The identity lifecycle triggers. They keep one live identity per Card through every SQL path
/// that writes `cards` (create, edit, move, rename, insert, compaction, delete), and retire the
/// identity of a deleted Card instead of deleting it.
const IDENTITY_TRIGGERS_SQL: &str = "
CREATE TRIGGER IF NOT EXISTS identity_after_insert AFTER INSERT ON cards
BEGIN
    INSERT INTO card_identity(card_id, card_row, status, last_address, last_title, retired_at_rev)
    VALUES (zt_uuid_v4(), NEW.row_id, 'live', coalesce(NEW.location, NEW.citation_key),
            zt_card_title(NEW.text), NULL);
END;
CREATE TRIGGER IF NOT EXISTS identity_after_update
AFTER UPDATE OF location, citation_key, text ON cards
WHEN coalesce(NEW.location, NEW.citation_key) IS NOT coalesce(OLD.location, OLD.citation_key)
  OR zt_card_title(NEW.text) IS NOT zt_card_title(OLD.text)
BEGIN
    UPDATE card_identity
       SET last_address = coalesce(NEW.location, NEW.citation_key),
           last_title = zt_card_title(NEW.text)
     WHERE card_row = OLD.row_id AND status = 'live';
END;
CREATE TRIGGER IF NOT EXISTS identity_before_delete BEFORE DELETE ON cards
BEGIN
    UPDATE card_identity
       SET status = 'retired', card_row = NULL,
           last_address = coalesce(OLD.location, OLD.citation_key),
           last_title = zt_card_title(OLD.text),
           retired_at_rev = 1 + coalesce(
             (SELECT CAST(value AS INTEGER) FROM metadata WHERE key = 'archive_revision'), 0)
     WHERE card_row = OLD.row_id AND status = 'live';
END;
CREATE TRIGGER IF NOT EXISTS cards_row_id_immutable BEFORE UPDATE OF row_id ON cards
WHEN NEW.row_id IS NOT OLD.row_id
BEGIN SELECT RAISE(ABORT, 'zt: a Card row id never changes'); END;
CREATE TRIGGER IF NOT EXISTS card_identity_id_immutable BEFORE UPDATE ON card_identity
WHEN NEW.card_id IS NOT OLD.card_id OR (OLD.status = 'retired' AND NEW.status = 'live')
BEGIN SELECT RAISE(ABORT, 'zt: a Card identity is immutable and never revived'); END;
CREATE TRIGGER IF NOT EXISTS card_identity_never_deleted BEFORE DELETE ON card_identity
BEGIN SELECT RAISE(ABORT, 'zt: a Card identity is retired, never deleted'); END;
CREATE TRIGGER IF NOT EXISTS card_handles_permanent_update BEFORE UPDATE ON card_handles
BEGIN SELECT RAISE(ABORT, 'zt: an issued handle is permanent'); END;
CREATE TRIGGER IF NOT EXISTS card_handles_permanent_delete BEFORE DELETE ON card_handles
BEGIN SELECT RAISE(ABORT, 'zt: an issued handle is permanent'); END;
";

/// The coordinate tables.
const COORDINATE_TABLES: [&str; 6] = [
    "card_identity",
    "card_handles",
    "connection_trees",
    "placements",
    "placement_edges",
    "retired_placements",
];

/// The batch receipt table, precreated here so that it is fenced like every other table.
const APPLIED_OPS_SQL: &str = "CREATE TABLE IF NOT EXISTS applied_ops(op_key TEXT NOT NULL PRIMARY KEY, request_hash TEXT NOT NULL, result_json TEXT NOT NULL, committed_at TEXT NOT NULL)";

/// One INSERT, UPDATE and DELETE guard per persistent application table: a writer connection of an
/// older `zt` lacks `zt_writer_epoch`, so every write it attempts on an upgraded archive fails and
/// changes nothing (§10.1.2). A table created later is fenced at the next startup.
fn writer_fence_sql(conn: &Connection) -> Result<String> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let tables = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut sql = String::new();
    for table in tables {
        if !table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        for event in ["INSERT", "UPDATE", "DELETE"] {
            sql.push_str(&format!(
                "CREATE TRIGGER IF NOT EXISTS writer_epoch_{table}_{} BEFORE {event} ON {table}
WHEN zt_writer_epoch() IS NOT {WRITER_EPOCH}
BEGIN SELECT RAISE(ABORT, 'zt: this archive needs a newer zt writer'); END;
",
                event.to_ascii_lowercase()
            ));
        }
    }
    Ok(sql)
}

pub(crate) fn installed(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'card_identity')",
        [],
        |row| row.get::<_, bool>(0),
    )
    .unwrap_or(false)
}

/// Install the coordinate storage once, or verify it is current. The first installation backs up a
/// non-empty archive, then in one transaction creates the tables, gives every existing Card one live
/// identity (no placement, no handle), installs the lifecycle triggers and the writer fence, and
/// records the generation and the never-reused Connection-tree allocator. A reopened archive issues
/// no new identity.
pub(crate) fn ensure_schema(conn: &Connection, root: &Path) -> Result<()> {
    if installed(conn) {
        match conn
            .query_row("SELECT value FROM metadata WHERE key = ?1", [SCHEMA_KEY], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .as_deref()
        {
            Some(SCHEMA_VERSION) => {}
            Some(other) => bail!(
                "unsupported coordinate storage generation `{other}`; this zt understands `{SCHEMA_VERSION}`; database was not modified"
            ),
            None => bail!("coordinate storage is incomplete (no generation recorded); database was not modified"),
        }
        // A Card-table rebuild by an earlier migration drops the triggers that belong to `cards`;
        // reinstalling them is idempotent.
        conn.execute_batch(IDENTITY_TRIGGERS_SQL)?;
        conn.execute_batch(APPLIED_OPS_SQL)?;
        conn.execute_batch(&writer_fence_sql(conn)?)?;
        return Ok(());
    }
    let cards: i64 = conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
    if cards > 0 {
        let backup = root.join(format!(
            "zt.sqlite3.pre-coordinates-{}.bak",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ));
        conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])
            .with_context(|| format!("failed to back up the archive to {}", backup.display()))?;
    }
    conn.execute_batch("BEGIN IMMEDIATE;")?;
    let result = (|| -> Result<()> {
        conn.execute_batch(TABLES_SQL)?;
        conn.execute(
            "INSERT INTO card_identity(card_id, card_row, status, last_address, last_title, retired_at_rev)
             SELECT zt_uuid_v4(), row_id, 'live', coalesce(location, citation_key), zt_card_title(text), NULL
             FROM cards ORDER BY row_id",
            [],
        )?;
        conn.execute_batch(IDENTITY_TRIGGERS_SQL)?;
        conn.execute_batch(APPLIED_OPS_SQL)?;
        conn.execute_batch(&writer_fence_sql(conn)?)?;
        conn.execute(
            "INSERT INTO metadata(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![SCHEMA_KEY, SCHEMA_VERSION],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO metadata(key, value) VALUES('next_connection_id', '1')",
            [],
        )?;
        let live: i64 = conn.query_row(
            "SELECT COUNT(*) FROM card_identity WHERE status = 'live'",
            [],
            |row| row.get(0),
        )?;
        if live != cards {
            bail!("identity backfill gave {live} identities for {cards} Cards");
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute_batch("COMMIT;")?;
            crate::log_event(
                root,
                &format!("installed multi-tree coordinates; {cards} Card(s) received an identity"),
            )
            .ok();
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK;");
            Err(error.context("failed to install multi-tree coordinates; database was not modified"))
        }
    }
}

/// Remove the coordinate storage from a test archive, so a fixture can rebuild it into the exact
/// shape of a release that predates multi-tree coordinates. The next startup installs it again.
#[cfg(test)]
pub(crate) fn strip_for_legacy_fixture(conn: &Connection) {
    let mut stmt = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'trigger'
             AND (name LIKE 'writer_epoch_%' OR name LIKE 'identity_%'
                  OR name IN ('cards_row_id_immutable', 'card_identity_id_immutable',
                              'card_identity_never_deleted', 'card_handles_permanent_update',
                              'card_handles_permanent_delete'))",
        )
        .expect("trigger list");
    let triggers = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .expect("triggers")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("trigger names");
    drop(stmt);
    for trigger in triggers {
        conn.execute_batch(&format!("DROP TRIGGER {trigger};"))
            .expect("drop trigger");
    }
    for table in COORDINATE_TABLES.iter().rev() {
        conn.execute_batch(&format!("DROP TABLE IF EXISTS {table};"))
            .expect("drop table");
    }
    conn.execute(
        "DELETE FROM metadata WHERE key IN (?1, 'next_connection_id')",
        [SCHEMA_KEY],
    )
    .expect("forget the generation");
}

/// The coordinate storage's own integrity: one live identity per Card and back, referential facts,
/// tree shape, and the rule that every live native Card has its home station.
pub(crate) fn check(conn: &Connection) -> Result<Value> {
    let mut faults = Vec::new();
    if !installed(conn) {
        return Ok(json!({"ok": true, "installed": false, "faults": []}));
    }
    let orphans: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cards c LEFT JOIN card_identity i ON i.card_row = c.row_id AND i.status='live'
         WHERE i.card_id IS NULL",
        [],
        |r| r.get(0),
    )?;
    if orphans > 0 {
        faults.push(json!({"kind": "card_without_identity", "count": orphans}));
    }
    let dangling: i64 = conn.query_row(
        "SELECT COUNT(*) FROM card_identity i LEFT JOIN cards c ON c.row_id = i.card_row
         WHERE i.status='live' AND c.row_id IS NULL",
        [],
        |r| r.get(0),
    )?;
    if dangling > 0 {
        faults.push(json!({"kind": "identity_without_card", "count": dangling}));
    }
    let mut stmt = conn.prepare("PRAGMA foreign_key_check")?;
    let fk: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (table, parent) in fk {
        if COORDINATE_TABLES.contains(&table.as_str()) {
            faults.push(json!({"kind": "foreign_key", "table": table, "parent": parent}));
        }
    }
    for tree in trees(conn)? {
        match load_tree_shape(conn, &tree) {
            Ok(_) => {}
            Err(error) => faults.push(json!({"kind": "tree_shape", "tree": format!("~{}", tree.display_num), "detail": format!("{error:#}")})),
        }
        let native_missing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM card_identity i JOIN cards c ON c.row_id = i.card_row
             WHERE i.status='live' AND (c.location = ?1 OR c.location LIKE ?2)
               AND NOT EXISTS (SELECT 1 FROM placements p WHERE p.host_tree_key = ?3 AND p.card_id = i.card_id)",
            params![Handle::root(tree.display_num), format!("~{}#%", tree.display_num), tree.tree_id],
            |r| r.get(0),
        )?;
        if native_missing > 0 {
            faults.push(json!({"kind": "native_without_home_station", "tree": format!("~{}", tree.display_num), "count": native_missing}));
        }
    }
    Ok(json!({"ok": faults.is_empty(), "installed": true, "faults": faults}))
}

// ---------------------------------------------------------------------------------------------
// Identity and resolution
// ---------------------------------------------------------------------------------------------

/// The live identity of a Card row.
pub(crate) fn card_id_of_row(conn: &Connection, row: i64) -> Result<String> {
    conn.query_row(
        "SELECT card_id FROM card_identity WHERE card_row = ?1 AND status = 'live'",
        [row],
        |r| r.get(0),
    )
    .with_context(|| format!("Card row {row} has no live identity"))
}

/// The live identity of a Card address (Location, Citation key or handle).
pub(crate) fn card_id_of(conn: &Connection, address: &str) -> Result<String> {
    let card = crate::load_card(conn, address)?;
    card_id_of_row(conn, card.row_id)
}

/// One identity's registry facts.
#[derive(Clone, Debug)]
pub(crate) struct Identity {
    pub(crate) card_id: String,
    pub(crate) card_row: Option<i64>,
    pub(crate) live: bool,
    pub(crate) last_address: String,
    pub(crate) last_title: String,
}

pub(crate) fn identity(conn: &Connection, card_id: &str) -> Result<Option<Identity>> {
    Ok(conn
        .query_row(
            "SELECT card_id, card_row, status, last_address, last_title FROM card_identity WHERE card_id = ?1",
            [card_id],
            |r| {
                Ok(Identity {
                    card_id: r.get(0)?,
                    card_row: r.get(1)?,
                    live: r.get::<_, String>(2)? == "live",
                    last_address: r.get(3)?,
                    last_title: r.get(4)?,
                })
            },
        )
        .optional()?)
}

/// The current canonical address of a live Card identity.
pub(crate) fn address_of_card_id(conn: &Connection, card_id: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT coalesce(c.location, c.citation_key) FROM card_identity i JOIN cards c ON c.row_id = i.card_row
             WHERE i.card_id = ?1 AND i.status = 'live'",
            [card_id],
            |r| r.get(0),
        )
        .optional()?)
}

/// The current address a permanent handle names, when the handle no longer equals the Card's own
/// stored address (a promoted native Card). `None` for an unknown handle or a retired Card.
pub(crate) fn handle_target(conn: &Connection, handle: &str) -> Result<Option<String>> {
    if !installed(conn) || !is_handle(handle) {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            "SELECT coalesce(c.location, c.citation_key) FROM card_handles h
               JOIN card_identity i ON i.card_id = h.card_id AND i.status = 'live'
               JOIN cards c ON c.row_id = i.card_row
             WHERE h.handle = ?1",
            [handle],
            |r| r.get(0),
        )
        .optional()?)
}

/// Every issued handle whose Card now lives at a different address: `{handle → current address}`.
/// Only promoted native Cards are aliased this way.
pub(crate) fn aliases(conn: &Connection) -> Result<BTreeMap<String, String>> {
    if !installed(conn) {
        return Ok(BTreeMap::new());
    }
    let mut stmt = conn.prepare_cached(
        "SELECT h.handle, coalesce(c.location, c.citation_key) FROM card_handles h
           JOIN card_identity i ON i.card_id = h.card_id AND i.status = 'live'
           JOIN cards c ON c.row_id = i.card_row
         WHERE h.handle IS NOT coalesce(c.location, c.citation_key)",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
    Ok(rows)
}

/// Every spelling under which a live Card can be cited: its address and any handle aliasing it.
pub(crate) fn spellings(conn: &Connection, address: &str) -> Result<Vec<String>> {
    let mut out = vec![address.to_string()];
    if !installed(conn) {
        return Ok(out);
    }
    let mut stmt = conn.prepare_cached(
        "SELECT h.handle FROM card_handles h
           JOIN card_identity i ON i.card_id = h.card_id AND i.status = 'live'
           JOIN cards c ON c.row_id = i.card_row
         WHERE coalesce(c.location, c.citation_key) = ?1 AND h.handle <> ?1",
    )?;
    for handle in stmt.query_map([address], |r| r.get::<_, String>(0))? {
        out.push(handle?);
    }
    Ok(out)
}

/// The canonical address `spelling` names: itself for a stored address, the current address for a
/// promoted handle, `None` when nothing live answers.
pub(crate) fn canonical(conn: &Connection, spelling: &str) -> Result<Option<String>> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cards WHERE location = ?1 OR citation_key = ?1)",
        [spelling],
        |r| r.get(0),
    )?;
    if exists {
        return Ok(Some(spelling.to_string()));
    }
    handle_target(conn, spelling)
}

/// The next archive revision a write in progress will commit (the retirement stamp).
pub(crate) fn next_revision(conn: &Connection) -> Result<i64> {
    Ok(crate::link_index::revision(conn)? as i64 + 1)
}

// ---------------------------------------------------------------------------------------------
// Connection trees, placements and coordinates
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) struct ConnTree {
    pub(crate) tree_id: String,
    pub(crate) display_num: u64,
    pub(crate) workspace: Workspace,
    pub(crate) root_card_id: String,
    pub(crate) purpose: Option<String>,
    pub(crate) next_local_seq: u64,
    pub(crate) created_at: String,
}

impl ConnTree {
    pub(crate) fn handle(&self) -> String {
        Handle::root(self.display_num)
    }
}

fn tree_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ConnTree> {
    Ok(ConnTree {
        tree_id: r.get(0)?,
        display_num: r.get::<_, i64>(1)? as u64,
        workspace: Workspace::from_stored(&r.get::<_, String>(2)?),
        root_card_id: r.get(3)?,
        purpose: r.get(4)?,
        next_local_seq: r.get::<_, i64>(5)? as u64,
        created_at: r.get(6)?,
    })
}

const TREE_COLUMNS: &str =
    "tree_id, display_num, workspace, root_card_id, purpose, next_local_seq, created_at";

pub(crate) fn trees(conn: &Connection) -> Result<Vec<ConnTree>> {
    if !installed(conn) {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {TREE_COLUMNS} FROM connection_trees ORDER BY display_num"
    ))?;
    let rows = stmt
        .query_map([], tree_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn tree_by_num(conn: &Connection, num: u64) -> Result<Option<ConnTree>> {
    if !installed(conn) {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            &format!("SELECT {TREE_COLUMNS} FROM connection_trees WHERE display_num = ?1"),
            [num as i64],
            tree_from_row,
        )
        .optional()?)
}

pub(crate) fn tree_by_id(conn: &Connection, tree_id: &str) -> Result<Option<ConnTree>> {
    if !installed(conn) {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            &format!("SELECT {TREE_COLUMNS} FROM connection_trees WHERE tree_id = ?1"),
            [tree_id],
            tree_from_row,
        )
        .optional()?)
}

/// A Connection tree named by `~n`, `~n/…`, `n`, or its UUID.
pub(crate) fn resolve_tree(conn: &Connection, name: &str) -> Result<ConnTree> {
    let name = name.trim();
    let found = if let Some(handle) = Handle::parse(name).filter(|h| h.local.is_none()) {
        tree_by_num(conn, handle.tree)?
    } else if let Some(Coordinate::Connection { tree, .. }) = Coordinate::parse(name) {
        tree_by_num(conn, tree)?
    } else if let Some(num) = positive(name) {
        tree_by_num(conn, num)?
    } else if is_uuid(name) {
        tree_by_id(conn, name)?
    } else {
        None
    };
    found.with_context(|| format!("Connection tree `{name}` does not exist"))
}

/// The Connection tree a Connection-homed address belongs to (`~7` and `~7#4` → tree 7), if live.
pub(crate) fn home_tree_of_address(conn: &Connection, address: &str) -> Result<Option<ConnTree>> {
    match Handle::parse(address) {
        Some(handle) => tree_by_num(conn, handle.tree),
        None => Ok(None),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EdgeKind {
    Root,
    Direct,
    Side,
    Attach,
}

impl EdgeKind {
    pub(crate) fn id(self) -> &'static str {
        match self {
            EdgeKind::Root => "root",
            EdgeKind::Direct => "direct",
            EdgeKind::Side => "side",
            EdgeKind::Attach => "attach",
        }
    }

    fn from_stored(value: &str) -> EdgeKind {
        match value {
            "root" => EdgeKind::Root,
            "direct" => EdgeKind::Direct,
            "side" => EdgeKind::Side,
            _ => EdgeKind::Attach,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Placement {
    pub(crate) placement_id: String,
    pub(crate) host_kind: String,
    pub(crate) host_tree_key: String,
    pub(crate) card_id: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) anchor_card_id: Option<String>,
    pub(crate) edge: EdgeKind,
    pub(crate) ordinal: Option<u64>,
    pub(crate) gloss: Option<String>,
}

const PLACEMENT_COLUMNS: &str =
    "placement_id, host_kind, host_tree_key, card_id, parent_id, anchor_card_id, edge, ordinal, gloss";

fn placement_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Placement> {
    Ok(Placement {
        placement_id: r.get(0)?,
        host_kind: r.get(1)?,
        host_tree_key: r.get(2)?,
        card_id: r.get(3)?,
        parent_id: r.get(4)?,
        anchor_card_id: r.get(5)?,
        edge: EdgeKind::from_stored(&r.get::<_, String>(6)?),
        ordinal: r.get::<_, Option<i64>>(7)?.map(|v| v as u64),
        gloss: r.get(8)?,
    })
}

pub(crate) fn placement(conn: &Connection, placement_id: &str) -> Result<Option<Placement>> {
    if !installed(conn) {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            &format!("SELECT {PLACEMENT_COLUMNS} FROM placements WHERE placement_id = ?1"),
            [placement_id],
            placement_from_row,
        )
        .optional()?)
}

pub(crate) fn placements_of_host(conn: &Connection, host_tree_key: &str) -> Result<Vec<Placement>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {PLACEMENT_COLUMNS} FROM placements WHERE host_tree_key = ?1"
    ))?;
    let rows = stmt
        .query_map([host_tree_key], placement_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn placements_of_card(conn: &Connection, card_id: &str) -> Result<Vec<Placement>> {
    if !installed(conn) {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {PLACEMENT_COLUMNS} FROM placements WHERE card_id = ?1 ORDER BY host_kind, host_tree_key"
    ))?;
    let rows = stmt
        .query_map([card_id], placement_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// The shape of one Connection tree: its placements by ID, children, and derived coordinates.
#[derive(Clone, Debug)]
pub(crate) struct TreeShape {
    pub(crate) tree: ConnTree,
    pub(crate) by_id: BTreeMap<String, Placement>,
    pub(crate) root: String,
    /// `parent → direct child`.
    pub(crate) direct: BTreeMap<String, String>,
    /// `parent → side children in ordinal order`.
    pub(crate) side: BTreeMap<String, Vec<String>>,
    /// `placement → coordinate`, and back.
    pub(crate) coord: BTreeMap<String, String>,
    pub(crate) by_coord: BTreeMap<String, String>,
    /// Placements in outline (depth-first, Direct chain as peers) order.
    pub(crate) order: Vec<String>,
}

impl TreeShape {
    /// The structural predecessor of a non-root placement: its parent placement.
    pub(crate) fn parent(&self, placement_id: &str) -> Option<&str> {
        self.by_id.get(placement_id)?.parent_id.as_deref()
    }

    /// Every placement under `placement_id` (itself included), in outline order.
    pub(crate) fn subtree(&self, placement_id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![placement_id.to_string()];
        while let Some(id) = stack.pop() {
            out.push(id.clone());
            if let Some(next) = self.direct.get(&id) {
                stack.push(next.clone());
            }
            if let Some(sides) = self.side.get(&id) {
                for side in sides.iter().rev() {
                    stack.push(side.clone());
                }
            }
        }
        out
    }

    pub(crate) fn placement_of_card(&self, card_id: &str) -> Option<&Placement> {
        self.by_id.values().find(|p| p.card_id == card_id)
    }
}

fn side_label(ordinal: u64) -> String {
    crate::number_to_side_label(ordinal)
}

fn direct_coord(coord: &str) -> String {
    // `~7/0` → `~7/1`; `~7/2` → `~7/3`; `~7/2|a` → `~7/2|a|1`; `~7/2|a|1` → `~7/2|a|2`.
    let (prefix, path) = coord.split_once('/').expect("coordinate has a slash");
    if path == "0" {
        return format!("{prefix}/1");
    }
    let last = crate::last_segment(path);
    if crate::valid_positive_number(last) {
        let next = last.parse::<u64>().unwrap_or(0) + 1;
        format!("{prefix}/{}", crate::replace_last_segment(path, &next.to_string()))
    } else {
        format!("{coord}|1")
    }
}

/// Load one Connection tree's placements and derive their coordinates, checking the structural
/// invariants: one root holding the root Card, one structural parent per station inside this tree,
/// no Side on the root, every station reachable, no cycle.
pub(crate) fn load_tree_shape(conn: &Connection, tree: &ConnTree) -> Result<TreeShape> {
    let placements = placements_of_host(conn, &tree.tree_id)?;
    let mut by_id = BTreeMap::new();
    let mut root = None;
    let mut direct = BTreeMap::new();
    let mut side: BTreeMap<String, Vec<(u64, String)>> = BTreeMap::new();
    for p in placements {
        match p.edge {
            EdgeKind::Root => {
                if root.replace(p.placement_id.clone()).is_some() {
                    bail!("two root stations");
                }
                if p.card_id != tree.root_card_id {
                    bail!("the root station does not hold the root Card");
                }
            }
            EdgeKind::Direct => {
                let parent = p.parent_id.clone().context("a Direct station without a parent")?;
                if direct.insert(parent, p.placement_id.clone()).is_some() {
                    bail!("two Direct successors of one station");
                }
            }
            EdgeKind::Side => {
                let parent = p.parent_id.clone().context("a Side station without a parent")?;
                side.entry(parent)
                    .or_default()
                    .push((p.ordinal.unwrap_or(0), p.placement_id.clone()));
            }
            EdgeKind::Attach => bail!("a Topic attachment inside a Connection tree"),
        }
        by_id.insert(p.placement_id.clone(), p);
    }
    let root = root.context("no root station")?;
    if side.contains_key(&root) {
        bail!("a Side branch starts at the root");
    }
    let side: BTreeMap<String, Vec<String>> = side
        .into_iter()
        .map(|(parent, mut kids)| {
            kids.sort();
            (parent, kids.into_iter().map(|(_, id)| id).collect())
        })
        .collect();
    for parent in direct.keys().chain(side.keys()) {
        if !by_id.contains_key(parent) {
            bail!("a station's parent is outside this tree");
        }
    }
    let mut coord = BTreeMap::new();
    let mut order = Vec::new();
    let mut stack = vec![(root.clone(), format!("~{}/0", tree.display_num))];
    while let Some((id, here)) = stack.pop() {
        if coord.insert(id.clone(), here.clone()).is_some() {
            bail!("a cycle in the station graph");
        }
        order.push(id.clone());
        if let Some(next) = direct.get(&id) {
            stack.push((next.clone(), direct_coord(&here)));
        }
        if let Some(kids) = side.get(&id) {
            for (index, kid) in kids.iter().enumerate().rev() {
                stack.push((kid.clone(), format!("{here}|{}", side_label(index as u64 + 1))));
            }
        }
    }
    if coord.len() != by_id.len() {
        bail!("{} station(s) cannot be reached from the root", by_id.len() - coord.len());
    }
    let by_coord = coord.iter().map(|(id, c)| (c.clone(), id.clone())).collect();
    Ok(TreeShape {
        tree: tree.clone(),
        by_id,
        root,
        direct,
        side,
        coord,
        by_coord,
        order,
    })
}

/// The ordered Topic-side mounts beside one Topic home Card.
pub(crate) fn attachments_of_anchor(conn: &Connection, anchor_card_id: &str) -> Result<Vec<Placement>> {
    if !installed(conn) {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {PLACEMENT_COLUMNS} FROM placements WHERE host_kind = 'topic' AND anchor_card_id = ?1 ORDER BY ordinal"
    ))?;
    let rows = stmt
        .query_map([anchor_card_id], placement_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// The current coordinate of any placement.
pub(crate) fn coordinate_of(conn: &Connection, placement: &Placement) -> Result<String> {
    match placement.edge {
        EdgeKind::Attach => {
            let anchor = placement.anchor_card_id.as_deref().context("attachment without anchor")?;
            let address = address_of_card_id(conn, anchor)?.context("attachment anchor is not live")?;
            let siblings = attachments_of_anchor(conn, anchor)?;
            let position = siblings
                .iter()
                .position(|p| p.placement_id == placement.placement_id)
                .context("attachment missing from its anchor")?;
            Ok(format!("{address}+{}", position + 1))
        }
        _ => {
            let tree = tree_by_id(conn, &placement.host_tree_key)?.context("station of a deleted tree")?;
            let shape = load_tree_shape(conn, &tree)?;
            shape
                .coord
                .get(&placement.placement_id)
                .cloned()
                .context("station has no coordinate")
        }
    }
}

/// Resolve a navigation coordinate to its placement.
pub(crate) fn resolve_coordinate(conn: &Connection, text: &str) -> Result<Placement> {
    match Coordinate::parse(text).with_context(|| format!("`{text}` is not a coordinate"))? {
        Coordinate::Connection { tree, .. } => {
            let tree = tree_by_num(conn, tree)?
                .with_context(|| format!("Connection tree `~{tree}` does not exist"))?;
            let shape = load_tree_shape(conn, &tree)?;
            let id = shape
                .by_coord
                .get(text)
                .with_context(|| format!("no station at `{text}`"))?;
            Ok(shape.by_id[id].clone())
        }
        Coordinate::TopicMount { anchor, ordinal } => {
            let anchor_id = card_id_of(conn, &anchor)?;
            let attached = attachments_of_anchor(conn, &anchor_id)?;
            attached
                .into_iter()
                .nth(ordinal as usize - 1)
                .with_context(|| format!("no Topic-side mount at `{text}`"))
        }
    }
}

/// A station named by a coordinate, `place:<uuid>`, or a bare placement UUID.
pub(crate) fn resolve_station(conn: &Connection, text: &str) -> Result<Placement> {
    let id = place_bookmark(text).unwrap_or(text);
    if is_uuid(id) {
        if let Some(p) = placement(conn, id)? {
            return Ok(p);
        }
        let retired: Option<String> = conn
            .query_row(
                "SELECT card_id FROM retired_placements WHERE placement_id = ?1",
                [id],
                |r| r.get(0),
            )
            .optional()?;
        if retired.is_some() {
            bail!("placement removed: station `{id}` was detached or its tree deleted");
        }
        bail!("placement `{id}` does not exist");
    }
    resolve_coordinate(conn, text)
}

// ---------------------------------------------------------------------------------------------
// Deletes: the host data a home delete owns
// ---------------------------------------------------------------------------------------------

/// Remove placements by ID (connections first), recording each ID as retired.
fn remove_placements(conn: &Connection, ids: &[String], revision: i64) -> Result<()> {
    for id in ids {
        conn.execute(
            "DELETE FROM placement_edges WHERE from_placement_id = ?1 OR to_placement_id = ?1",
            [id],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO retired_placements(placement_id, host_tree_key, card_id, retired_at_rev)
             SELECT placement_id, host_tree_key, card_id, ?2 FROM placements WHERE placement_id = ?1",
            params![id, revision],
        )?;
    }
    for id in ids {
        conn.execute("DELETE FROM placements WHERE placement_id = ?1", [id])?;
    }
    Ok(())
}

/// Before a home delete removes the Cards at `delete_set`: remove the host data those Cards own.
/// A deleted Connection root takes its whole tree (stations, connections, the tree record; its
/// issued handles stay); a kept Connection root (`keep_root`) keeps only its root station; a deleted
/// Topic Card takes the Topic-side mounts beside it. A station in another tree that mounts a deleted
/// Card is never touched: it becomes a Missing mount.
pub(crate) fn delete_hosts(
    conn: &Connection,
    delete_set: &BTreeSet<String>,
    keep_root: Option<&str>,
) -> Result<()> {
    if !installed(conn) {
        return Ok(());
    }
    let revision = next_revision(conn)?;
    for address in delete_set {
        if let Some(handle) = Handle::parse(address).filter(|h| h.local.is_none())
            && let Some(tree) = tree_by_num(conn, handle.tree)?
        {
            let ids: Vec<String> = placements_of_host(conn, &tree.tree_id)?
                .into_iter()
                .map(|p| p.placement_id)
                .collect();
            remove_placements(conn, &ids, revision)?;
            conn.execute("DELETE FROM connection_trees WHERE tree_id = ?1", [&tree.tree_id])?;
            continue;
        }
        if TreeRole::of_address(address) == TreeRole::Topic {
            let Ok(card) = crate::load_card(conn, address) else { continue };
            let card_id = card_id_of_row(conn, card.row_id)?;
            let ids: Vec<String> = attachments_of_anchor(conn, &card_id)?
                .into_iter()
                .map(|p| p.placement_id)
                .collect();
            remove_placements(conn, &ids, revision)?;
        }
    }
    if let Some(root) = keep_root
        && let Some(handle) = Handle::parse(root).filter(|h| h.local.is_none())
        && let Some(tree) = tree_by_num(conn, handle.tree)?
    {
        let ids: Vec<String> = placements_of_host(conn, &tree.tree_id)?
            .into_iter()
            .filter(|p| p.edge != EdgeKind::Root)
            .map(|p| p.placement_id)
            .collect();
        remove_placements(conn, &ids, revision)?;
    }
    Ok(())
}

/// Whether a keep-root delete of `root` has host data to remove even when no Card goes.
pub(crate) fn keep_root_has_host_work(conn: &Connection, root: &str) -> Result<bool> {
    let Some(handle) = Handle::parse(root).filter(|h| h.local.is_none()) else {
        return Ok(false);
    };
    let Some(tree) = tree_by_num(conn, handle.tree)? else {
        return Ok(false);
    };
    Ok(placements_of_host(conn, &tree.tree_id)?.len() > 1)
}

/// The whole-archive delete: every station, connection and tree record goes (their IDs retired);
/// identities, issued handles, retired history and the Connection-tree allocator stay.
pub(crate) fn delete_all_hosts(conn: &Connection) -> Result<()> {
    if !installed(conn) {
        return Ok(());
    }
    let revision = next_revision(conn)?;
    conn.execute("DELETE FROM placement_edges", [])?;
    conn.execute(
        "INSERT OR IGNORE INTO retired_placements(placement_id, host_tree_key, card_id, retired_at_rev)
         SELECT placement_id, host_tree_key, card_id, ?1 FROM placements",
        [revision],
    )?;
    conn.execute("DELETE FROM placements", [])?;
    conn.execute("DELETE FROM connection_trees", [])?;
    Ok(())
}

/// The plan lines a home delete adds for the coordinate data it affects: trees it removes, and
/// stations in other trees that will show as Missing mounts (with their connections, kept).
pub(crate) fn delete_impact_lines(conn: &Connection, delete_set: &BTreeSet<String>) -> Result<Vec<String>> {
    if !installed(conn) {
        return Ok(Vec::new());
    }
    let mut lines = Vec::new();
    let mut removed_hosts = BTreeSet::new();
    for address in delete_set {
        if let Some(handle) = Handle::parse(address).filter(|h| h.local.is_none())
            && let Some(tree) = tree_by_num(conn, handle.tree)?
        {
            let n = placements_of_host(conn, &tree.tree_id)?.len();
            lines.push(format!("Connection tree {} goes with its {n} station(s) and their connections; its handles stay issued", tree.handle()));
            removed_hosts.insert(tree.tree_id);
        }
    }
    let mut missing = 0;
    let mut hosts = BTreeSet::new();
    for address in delete_set {
        let Ok(card) = crate::load_card(conn, address) else { continue };
        let card_id = card_id_of_row(conn, card.row_id)?;
        for p in placements_of_card(conn, &card_id)? {
            if removed_hosts.contains(&p.host_tree_key) {
                continue;
            }
            missing += 1;
            hosts.insert(p.host_tree_key.clone());
        }
    }
    if missing > 0 {
        lines.push(format!(
            "{missing} station(s) in {} other tree(s) will show as Missing mounts, keeping their glosses, children and connections",
            hosts.len()
        ));
    }
    Ok(lines)
}

/// Every Missing mount: a station whose Card was deleted at its home. `workspace` filters by the
/// host tree's Workspace.
pub(crate) fn missing_mounts(conn: &Connection, workspace: Option<Workspace>) -> Result<Vec<Value>> {
    if !installed(conn) {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(&format!(
        "SELECT {PLACEMENT_COLUMNS} FROM placements p WHERE EXISTS (
             SELECT 1 FROM card_identity i WHERE i.card_id = p.card_id AND i.status = 'retired')
         ORDER BY host_kind, host_tree_key, placement_id"
    ))?;
    let rows = stmt
        .query_map([], placement_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut out = Vec::new();
    for p in rows {
        let identity = identity(conn, &p.card_id)?.context("unknown Card")?;
        let (host, host_workspace) = if p.host_kind == "topic" {
            let root = format!("{}/0", p.host_tree_key);
            let ws = crate::load_card(conn, &root).map(|c| c.workspace).unwrap_or(Workspace::Human);
            (root, ws)
        } else {
            let Some(tree) = tree_by_id(conn, &p.host_tree_key)? else { continue };
            (tree.handle(), tree.workspace)
        };
        if workspace.is_some_and(|w| w != host_workspace) {
            continue;
        }
        out.push(json!({
            "placement_id": p.placement_id,
            "host": host,
            "host_workspace": host_workspace.id(),
            "coord": coordinate_of(conn, &p).ok(),
            "card_id": p.card_id,
            "last_address": identity.last_address,
            "last_title": identity.last_title,
            "gloss": p.gloss,
        }));
    }
    Ok(out)
}

/// After Cards took new addresses: Topic-side mounts follow their anchor by identity. A mount whose
/// anchor moved into another Topic moves to that Topic's host; a Card moved into a Topic that
/// mounts it beside one of its Cards, or a mount that would meet the same Card already mounted in
/// the destination Topic, is refused (a Topic tree holds a Card once).
pub(crate) fn follow_anchor_moves(conn: &Connection, mapping: &BTreeMap<String, String>) -> Result<()> {
    if !installed(conn) {
        return Ok(());
    }
    for new in mapping.values() {
        if new.starts_with("__") || TreeRole::of_address(new) != TreeRole::Topic {
            continue;
        }
        let Ok(card) = crate::load_card(conn, new) else { continue };
        let card_id = card_id_of_row(conn, card.row_id)?;
        let topic = crate::tree_id(new).to_string();
        // The moved Card is mounted beside a Card of the Topic it moved into.
        if placements_of_host(conn, &topic)?.iter().any(|p| p.card_id == card_id) {
            bail!(
                "`{new}` is mounted beside a Card of Topic {topic} (a Topic-side mount); detach that mount before moving the Card into the Topic"
            );
        }
        // Mounts anchored on the moved Card follow it into its new Topic.
        for p in attachments_of_anchor(conn, &card_id)? {
            if p.host_tree_key == topic {
                continue;
            }
            if placements_of_host(conn, &topic)?.iter().any(|q| q.card_id == p.card_id) {
                bail!(
                    "the Card mounted beside `{new}` is already mounted in Topic {topic}; detach one of the two mounts before this move"
                );
            }
            if let Some(address) = address_of_card_id(conn, &p.card_id)?
                && TreeRole::of_address(&address) == TreeRole::Topic
                && crate::tree_id(&address) == topic
            {
                bail!("the Card mounted beside `{new}` has its home in Topic {topic}; detach the mount before this move");
            }
            conn.execute(
                "UPDATE placements SET host_tree_key = ?2 WHERE placement_id = ?1",
                params![p.placement_id, topic],
            )?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Dump
// ---------------------------------------------------------------------------------------------

/// The version of `coordinates.json` in a dump.
pub(crate) const DUMP_VERSION: u64 = 1;

fn table_rows(conn: &Connection, sql: &str) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(sql)?;
    let names: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let rows = stmt.query_map([], |r| {
        let mut map = serde_json::Map::new();
        for (i, name) in names.iter().enumerate() {
            let value: rusqlite::types::Value = r.get(i)?;
            let value = match value {
                rusqlite::types::Value::Null => Value::Null,
                rusqlite::types::Value::Integer(n) => json!(n),
                rusqlite::types::Value::Real(f) => json!(f),
                rusqlite::types::Value::Text(t) => json!(t),
                rusqlite::types::Value::Blob(_) => Value::Null,
            };
            map.insert(name.clone(), value);
        }
        Ok(Value::Object(map))
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        [name],
        |r| r.get(0),
    )?)
}

/// `coordinates.json`: every identity (live ones with their current address, retired ones with
/// their last address and title), every issued handle, the Connection trees, stations, tree
/// connections, retired station IDs, the allocators, and the coordinate Lock evidence, read in the
/// dump's one read transaction. The Card bodies travel once, in the dump's Card files.
pub(crate) fn dump_export(conn: &Connection) -> Result<Value> {
    if !installed(conn) {
        return Ok(json!({"version": DUMP_VERSION, "installed": false}));
    }
    let metadata = |key: &str| -> Result<Option<String>> {
        Ok(conn
            .query_row("SELECT value FROM metadata WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    };
    let mut lock_evidence = serde_json::Map::new();
    for table in [
        "lock_coordinate_seals",
        "lock_placements",
        "lock_connections",
        "lock_placement_pins",
        "lock_source_reviews",
    ] {
        if table_exists(conn, table)? {
            lock_evidence.insert(table.to_string(), json!(table_rows(conn, &format!("SELECT * FROM {table} ORDER BY 1, 2"))?));
        }
    }
    Ok(json!({
        "version": DUMP_VERSION,
        "installed": true,
        "archive_id": metadata("archive_id")?,
        "archive_revision": crate::link_index::revision(conn)?,
        "next_connection_id": metadata("next_connection_id")?,
        "identities": table_rows(conn,
            "SELECT i.card_id, i.status, COALESCE(c.location, c.citation_key, i.last_address) AS address,
                    i.last_address, i.last_title, i.retired_at_rev
             FROM card_identity i LEFT JOIN cards c ON c.row_id = i.card_row ORDER BY i.card_id")?,
        "handles": table_rows(conn, "SELECT handle, card_id, origin_tree_id, local_seq FROM card_handles ORDER BY handle")?,
        "connection_trees": table_rows(conn, "SELECT tree_id, display_num, workspace, root_card_id, purpose, next_local_seq, created_at FROM connection_trees ORDER BY display_num")?,
        "placements": table_rows(conn, "SELECT placement_id, host_kind, host_tree_key, card_id, parent_id, anchor_card_id, edge, ordinal, gloss FROM placements ORDER BY host_tree_key, placement_id")?,
        "placement_edges": table_rows(conn, "SELECT * FROM placement_edges ORDER BY tree_id, connection_id")?,
        "retired_placements": table_rows(conn, "SELECT * FROM retired_placements ORDER BY placement_id")?,
        "lock_evidence": Value::Object(lock_evidence),
    }))
}
