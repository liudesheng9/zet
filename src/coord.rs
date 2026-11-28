//! Stable Card identities and handles, plus coordinate topology shared by all three Tree roles.
//! Each live Card's filing pointer is enforced by a deferred foreign key. Secondary Coordinates
//! can recur in the same Tree and keep their own paths, children, glosses, and connections.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use crate::Workspace;

/// The coordinate storage generation this binary installs and understands.
pub(crate) const SCHEMA_VERSION: &str = "2";
const SCHEMA_KEY: &str = "coordinates_schema_version";
/// The writer capability an upgraded archive requires of every writing connection. A connection
/// of an older `zt` lacks the SQL function, so every guarded write it attempts fails.
pub(crate) const WRITER_EPOCH: i64 = 2;
pub(crate) const DUMP_VERSION: u64 = 2;

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

// ---------------------------------------------------------------------------------------------
// Tree roles and the address grammar
// ---------------------------------------------------------------------------------------------

/// Structural roles shared by Trees and the Cards filed in them.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum TreeRole {
    Topic,
    Literature,
    Connection,
}

impl TreeRole {
    pub(crate) fn from_stored(role: &str) -> TreeRole {
        match role {
            "topic" => Self::Topic,
            "literature" => Self::Literature,
            "connection" => Self::Connection,
            _ => unreachable!("invalid stored Tree role {role}"),
        }
    }

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

    #[allow(dead_code)]
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

/// A Connection tree's navigation path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Coordinate {
    /// `~n/0` (the root) or `~n/<path>` with Direct and Side segments.
    Connection { tree: u64, path: String },
}

impl Coordinate {
    pub(crate) fn parse(text: &str) -> Option<Coordinate> {
        let (tree, path) = text.strip_prefix('~')?.split_once('/')?;
        let tree = positive(tree)?;
        (path == "0" || crate::is_valid_regular_location(&format!("0/{path}"))).then(|| {
            Coordinate::Connection {
                tree,
                path: path.to_string(),
            }
        })
    }
}

fn is_retired_topic_mount(text: &str) -> bool {
    text.rsplit_once('+').is_some_and(|(anchor, suffix)| {
        crate::valid_topic_id(crate::tree_id(anchor))
            && crate::is_valid_location(anchor)
            && positive(suffix).is_some()
    })
}

pub(crate) fn is_coordinate(text: &str) -> bool {
    // Route retired spellings through the same refusal paths as navigation targets.
    Coordinate::parse(text).is_some() || is_retired_topic_mount(text)
}

/// The hint a refused `[[coordinate]]` carries: cite the Card, or bookmark the station.
pub(crate) fn coordinate_link_hint(target: &str) -> String {
    if is_retired_topic_mount(target) {
        return format!(
            "`{target}` uses retired Topic navigation coordinate syntax (`+k`). Use a current Coordinate path or a `place:<coord_id>` bookmark for navigation. Cite the Card by its canonical address or handle."
        );
    }
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
        new_uuid()
            .map_err(|error| rusqlite::Error::UserFunctionError(Box::from(format!("{error:#}"))))
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
CREATE TABLE trees (
    tree_id TEXT NOT NULL PRIMARY KEY,
    role TEXT NOT NULL CHECK(role IN ('literature', 'topic', 'connection')),
    workspace TEXT NOT NULL CHECK(workspace IN ('human', 'agent')),
    public_key TEXT NOT NULL UNIQUE,
    display_num INTEGER,
    root_card_id TEXT NOT NULL UNIQUE REFERENCES card_identity(card_id)
        DEFERRABLE INITIALLY DEFERRED,
    purpose TEXT CHECK(purpose IS NULL OR (length(purpose) BETWEEN 1 AND 32
        AND instr(purpose, char(10)) = 0 AND instr(purpose, char(13)) = 0)),
    next_local_seq INTEGER NOT NULL DEFAULT 1 CHECK(next_local_seq >= 1),
    created_at TEXT NOT NULL,
    UNIQUE(tree_id, workspace),
    CHECK((role = 'literature' AND display_num IS NULL)
       OR (role IN ('topic', 'connection') AND display_num IS NOT NULL AND display_num >= 0))
);
CREATE TABLE card_identity (
    card_id TEXT NOT NULL PRIMARY KEY,
    card_row INTEGER UNIQUE REFERENCES cards(row_id),
    owner_workspace TEXT CHECK(owner_workspace IS NULL OR owner_workspace IN ('human', 'agent')),
    status TEXT NOT NULL CHECK(status IN ('live', 'retired')),
    filing_coord_id TEXT,
    last_address TEXT NOT NULL,
    last_title TEXT NOT NULL,
    retired_at_rev INTEGER,
    CHECK((status = 'live' AND owner_workspace IS NOT NULL AND owner_workspace IN ('human', 'agent')
           AND card_row IS NOT NULL AND filing_coord_id IS NOT NULL AND retired_at_rev IS NULL)
       OR (status = 'retired' AND card_row IS NULL AND filing_coord_id IS NULL AND retired_at_rev IS NOT NULL)),
    FOREIGN KEY (filing_coord_id, card_id, owner_workspace)
        REFERENCES coordinates(coord_id, card_id, host_workspace) DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE card_handles (
    handle TEXT NOT NULL PRIMARY KEY,
    card_id TEXT NOT NULL UNIQUE REFERENCES card_identity(card_id) DEFERRABLE INITIALLY DEFERRED,
    origin_tree_id TEXT NOT NULL,
    local_seq INTEGER NOT NULL CHECK(local_seq >= 0),
    UNIQUE(origin_tree_id, local_seq)
);
CREATE TABLE coordinates (
    coord_id TEXT NOT NULL PRIMARY KEY,
    tree_id TEXT NOT NULL,
    host_workspace TEXT NOT NULL CHECK(host_workspace IN ('human', 'agent')),
    card_id TEXT NOT NULL REFERENCES card_identity(card_id) DEFERRABLE INITIALLY DEFERRED,
    parent_coord_id TEXT,
    edge TEXT NOT NULL CHECK(edge IN ('root', 'direct', 'side')),
    slot INTEGER,
    gloss TEXT CHECK(gloss IS NULL OR (length(gloss) BETWEEN 1 AND 240
        AND instr(gloss, char(10)) = 0 AND instr(gloss, char(13)) = 0)),
    FOREIGN KEY (tree_id, host_workspace) REFERENCES trees(tree_id, workspace)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (tree_id, parent_coord_id) REFERENCES coordinates(tree_id, coord_id)
        DEFERRABLE INITIALLY DEFERRED,
    UNIQUE(tree_id, coord_id),
    UNIQUE(coord_id, card_id, host_workspace),
    CHECK((edge = 'root' AND parent_coord_id IS NULL AND slot IS NULL)
       OR (edge = 'direct' AND parent_coord_id IS NOT NULL AND slot IS NULL)
       OR (edge = 'side' AND parent_coord_id IS NOT NULL AND slot IS NOT NULL AND slot >= 1))
);
CREATE TABLE coordinate_edges (
    connection_id TEXT NOT NULL PRIMARY KEY,
    tree_id TEXT NOT NULL REFERENCES trees(tree_id) DEFERRABLE INITIALLY DEFERRED,
    from_coord_id TEXT NOT NULL,
    to_coord_id TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('neutral', 'directed')),
    relation_label TEXT CHECK(relation_label IS NULL OR (length(relation_label) BETWEEN 1 AND 32
        AND instr(relation_label, char(10)) = 0 AND instr(relation_label, char(13)) = 0)),
    relation_kind TEXT CHECK(relation_kind IS NULL OR relation_kind IN (
        'rests-on','illustrates','contrasts','relaxes','corresponds','objects','specializes','refers')),
    note TEXT CHECK(note IS NULL OR length(note) BETWEEN 1 AND 400),
    created_by TEXT NOT NULL CHECK(created_by IN ('owner', 'agent')),
    created_at_rev INTEGER NOT NULL,
    updated_by TEXT NOT NULL CHECK(updated_by IN ('owner', 'agent')),
    updated_at_rev INTEGER NOT NULL,
    FOREIGN KEY (tree_id, from_coord_id) REFERENCES coordinates(tree_id, coord_id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (tree_id, to_coord_id) REFERENCES coordinates(tree_id, coord_id)
        DEFERRABLE INITIALLY DEFERRED,
    CHECK(from_coord_id <> to_coord_id),
    CHECK(direction = 'directed' OR from_coord_id < to_coord_id),
    CHECK(relation_kind IS NULL OR relation_kind IN ('contrasts', 'corresponds', 'refers')
        OR direction = 'directed')
);
CREATE TABLE retired_coordinates (
    coord_id TEXT NOT NULL PRIMARY KEY,
    tree_id TEXT NOT NULL,
    card_id TEXT NOT NULL REFERENCES card_identity(card_id),
    retired_at_rev INTEGER NOT NULL
);
";

// Install child-side foreign-key indexes before backfill starts resolving deferred references.
const INDEXES_SQL: &str = "
CREATE INDEX IF NOT EXISTS coordinates_by_card ON coordinates(card_id);
CREATE INDEX IF NOT EXISTS coordinates_by_parent ON coordinates(tree_id, parent_coord_id);
CREATE INDEX IF NOT EXISTS card_identity_by_filing
    ON card_identity(filing_coord_id, card_id, owner_workspace) WHERE filing_coord_id IS NOT NULL;
";

const POST_BACKFILL_INDEXES_SQL: &str = "
CREATE UNIQUE INDEX IF NOT EXISTS trees_role_num
    ON trees(role, display_num) WHERE display_num IS NOT NULL;
CREATE INDEX IF NOT EXISTS card_identity_live_address
    ON card_identity(last_address) WHERE status = 'live';
CREATE UNIQUE INDEX IF NOT EXISTS coordinates_one_root
    ON coordinates(tree_id) WHERE edge = 'root';
CREATE UNIQUE INDEX IF NOT EXISTS coordinates_one_direct
    ON coordinates(tree_id, parent_coord_id) WHERE edge = 'direct';
CREATE UNIQUE INDEX IF NOT EXISTS coordinates_side_slot
    ON coordinates(tree_id, parent_coord_id, slot) WHERE edge = 'side';
CREATE INDEX IF NOT EXISTS coordinates_by_tree_card ON coordinates(tree_id, card_id);
CREATE UNIQUE INDEX IF NOT EXISTS coordinate_edges_blank_pair
    ON coordinate_edges(tree_id, from_coord_id, to_coord_id)
    WHERE direction = 'neutral' AND relation_label IS NULL AND relation_kind IS NULL AND note IS NULL;
CREATE INDEX IF NOT EXISTS coordinate_edges_by_from ON coordinate_edges(tree_id, from_coord_id);
CREATE INDEX IF NOT EXISTS coordinate_edges_by_to ON coordinate_edges(tree_id, to_coord_id);
";

const IDENTITY_TRIGGERS_SQL: &str = "
CREATE TRIGGER IF NOT EXISTS identity_after_update
AFTER UPDATE OF location, citation_key, text ON cards
WHEN coalesce(NEW.location, NEW.citation_key) IS NOT coalesce(OLD.location, OLD.citation_key)
  OR zt_card_title(NEW.text) IS NOT zt_card_title(OLD.text)
BEGIN
    UPDATE card_identity
       SET last_address = coalesce(NEW.location, NEW.citation_key), last_title = zt_card_title(NEW.text)
     WHERE card_row = OLD.row_id AND status = 'live';
END;
CREATE TRIGGER IF NOT EXISTS identity_before_delete BEFORE DELETE ON cards
BEGIN
    UPDATE card_identity
       SET status = 'retired', card_row = NULL, filing_coord_id = NULL,
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
WHEN NEW.card_id IS NOT OLD.card_id OR NEW.owner_workspace IS NOT OLD.owner_workspace
    OR (OLD.status = 'retired' AND NEW.status = 'live')
BEGIN SELECT RAISE(ABORT, 'zt: a Card identity and its owner Workspace are immutable'); END;
CREATE TRIGGER IF NOT EXISTS card_identity_never_deleted BEFORE DELETE ON card_identity
BEGIN SELECT RAISE(ABORT, 'zt: a Card identity is retired, never deleted'); END;
CREATE TRIGGER IF NOT EXISTS card_handles_permanent_update BEFORE UPDATE ON card_handles
BEGIN SELECT RAISE(ABORT, 'zt: an issued handle is permanent'); END;
CREATE TRIGGER IF NOT EXISTS card_handles_permanent_delete BEFORE DELETE ON card_handles
BEGIN SELECT RAISE(ABORT, 'zt: an issued handle is permanent'); END;
";

const COORDINATE_TABLES: [&str; 6] = [
    "trees",
    "card_identity",
    "card_handles",
    "coordinates",
    "coordinate_edges",
    "retired_coordinates",
];
const APPLIED_OPS_SQL: &str = "CREATE TABLE IF NOT EXISTS applied_ops(op_key TEXT NOT NULL PRIMARY KEY, request_hash TEXT NOT NULL, result_json TEXT NOT NULL, committed_at TEXT NOT NULL)";

fn sql_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn drop_triggers_sql(conn: &Connection, filter: &str) -> Result<String> {
    let mut stmt = conn.prepare(&format!(
        "SELECT name FROM sqlite_master WHERE type = 'trigger' AND ({filter}) ORDER BY name"
    ))?;
    let names = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut sql = String::new();
    for name in names {
        sql.push_str(&format!(
            "DROP TRIGGER IF EXISTS {};\n",
            sql_identifier(&name?)
        ));
    }
    Ok(sql)
}

const WRITER_TRIGGERS: &str = "name GLOB 'writer_epoch_*'";
#[cfg(test)]
const COORD_TRIGGERS: &str = "
    name GLOB 'writer_epoch_*' OR name GLOB 'identity_*'
    OR name GLOB 'coord_lock_*' OR name GLOB 'coord_pin_*'
    OR name GLOB 'lock_source_reviews_*'
    OR name IN ('cards_row_id_immutable', 'card_identity_id_immutable',
                'card_identity_never_deleted', 'card_handles_permanent_update',
                'card_handles_permanent_delete')";

fn writer_fence_sql(conn: &Connection) -> Result<String> {
    let mut sql = drop_triggers_sql(conn, WRITER_TRIGGERS)?;
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT GLOB 'sqlite_*' ORDER BY name",
    )?;
    let tables = stmt.query_map([], |r| r.get::<_, String>(0))?;
    for table in tables {
        let table = table?;
        for event in ["INSERT", "UPDATE", "DELETE"] {
            let trigger = sql_identifier(&format!(
                "writer_epoch_{table}_{}",
                event.to_ascii_lowercase()
            ));
            let table = sql_identifier(&table);
            sql.push_str(&format!(
                "CREATE TRIGGER {trigger} BEFORE {event} ON {table}
                 WHEN zt_writer_epoch() IS NOT {WRITER_EPOCH}
                 BEGIN SELECT RAISE(ABORT, 'zt: this archive needs a newer zt writer'); END;\n"
            ));
        }
    }
    Ok(sql)
}

pub(crate) fn installed(conn: &Connection) -> bool {
    (table_exists(conn, "trees").unwrap_or(false)
        && table_exists(conn, "coordinates").unwrap_or(false))
        || table_exists(conn, "card_identity").unwrap_or(false)
}

pub(crate) fn schema_version(conn: &Connection) -> Result<Option<String>> {
    if !table_exists(conn, "metadata")? {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            "SELECT value FROM metadata WHERE key = ?1",
            [SCHEMA_KEY],
            |r| r.get(0),
        )
        .optional()?)
}

pub(crate) fn check_generation(conn: &Connection) -> Result<()> {
    match schema_version(conn)?.as_deref() {
        None | Some(SCHEMA_VERSION) => Ok(()),
        Some(other) => {
            bail!(
                "unsupported coordinate storage generation {other}; this binary supports generation 2 only; database was not modified"
            )
        }
    }
}

fn backup_archive(conn: &Connection, root: &Path) -> Result<()> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let backup = root.join(format!("zt.sqlite3.pre-coordinates-v2-{stamp}.bak"));
    conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])
        .with_context(|| format!("failed to back up the archive to {}", backup.display()))?;
    let verified =
        Connection::open_with_flags(&backup, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    // SQLite may prepare expressions from the archive's schema while checking it.
    crate::tree_lock::register_sql_functions(&verified)?;
    let result: String = verified.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if result != "ok" {
        bail!(
            "archive backup {} failed quick_check: {result}",
            backup.display()
        );
    }
    Ok(())
}

fn immediate(conn: &Connection, work: impl FnOnce() -> Result<()>) -> Result<()> {
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)?;
    work()?;
    tx.commit()?;
    Ok(())
}

fn install_protection(conn: &Connection) -> Result<()> {
    conn.execute_batch(IDENTITY_TRIGGERS_SQL)?;
    crate::coord_lock::ensure_schema(conn)?;
    conn.execute_batch(APPLIED_OPS_SQL)?;
    conn.execute_batch(&writer_fence_sql(conn)?)?;
    Ok(())
}

fn finish_install(conn: &Connection) -> Result<()> {
    conn.execute_batch(POST_BACKFILL_INDEXES_SQL)?;
    install_protection(conn)?;
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![SCHEMA_KEY, SCHEMA_VERSION],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO metadata(key, value) VALUES('next_connection_id', '1')",
        [],
    )?;
    let bad_fk: Option<(String, String)> = conn
        .query_row("PRAGMA foreign_key_check", [], |r| {
            Ok((r.get(0)?, r.get(2)?))
        })
        .optional()?;
    if let Some((table, parent)) = bad_fk {
        bail!("coordinate backfill left a foreign-key violation in {table} referencing {parent}");
    }
    let (cards, live): (i64, i64) = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM cards), (SELECT COUNT(*) FROM card_identity WHERE status = 'live')",
        [], |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if live != cards {
        bail!("coordinate backfill gave {live} live identities for {cards} Cards");
    }
    Ok(())
}

pub(crate) fn ensure_schema(conn: &Connection, root: &Path) -> Result<()> {
    check_generation(conn)?;
    match schema_version(conn)?.as_deref() {
        Some(SCHEMA_VERSION) => immediate(conn, || {
            conn.execute_batch(INDEXES_SQL)?;
            conn.execute_batch(POST_BACKFILL_INDEXES_SQL)?;
            install_protection(conn)
        }),
        _ => {
            if installed(conn) {
                bail!("coordinate storage is incomplete; no generation was recorded");
            }
            let cards: bool =
                conn.query_row("SELECT EXISTS(SELECT 1 FROM cards)", [], |r| r.get(0))?;
            if cards {
                backup_archive(conn, root)?;
            }
            immediate(conn, || {
                conn.execute_batch(TABLES_SQL)?;
                conn.execute_batch(INDEXES_SQL)?;
                materialize_filing_coordinates(conn)?;
                finish_install(conn)?;
                if table_exists(conn, "locks")? {
                    let active: Vec<(i64, String)> = {
                        let mut stmt = conn.prepare(
                            "SELECT l.lock_id, l.root_address FROM locks l
                             LEFT JOIN lock_coordinate_seals s ON s.lock_id = l.lock_id
                             WHERE l.unlocked_at IS NULL AND s.lock_id IS NULL ORDER BY l.lock_id",
                        )?;
                        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                            .collect::<rusqlite::Result<_>>()?
                    };
                    for (lock_id, root_address) in active {
                        if tree_by_public_key(conn, crate::tree_id(&root_address))?.is_some()
                            && let Ok(root_card) = crate::load_card(conn, &root_address)
                        {
                            let snap = crate::coord_lock::snapshot(conn, &root_card)?;
                            crate::coord_lock::capture(conn, lock_id, &snap)?;
                        }
                    }
                }
                Ok(())
            })
        }
    }
}

struct FilingCard {
    row_id: i64,
    address: String,
    workspace: String,
    topic_root: bool,
    literature_root: bool,
    card_id: String,
    last_title: String,
}

fn materialize_filing_coordinates(conn: &Connection) -> Result<()> {
    let cards = read_filing_cards(
        conn,
        "SELECT row_id, coalesce(location, citation_key), workspace, is_topic, is_lit,
                zt_uuid_v4(), zt_card_title(text)
         FROM cards",
    )?;
    let by_path = HashMap::with_capacity(cards.len());
    insert_filing_cards(conn, cards, by_path, HashMap::new())
}

fn read_filing_cards(conn: &Connection, sql: &str) -> Result<Vec<FilingCard>> {
    Ok(conn
        .prepare(sql)?
        .query_map([], |r| {
            Ok(FilingCard {
                row_id: r.get(0)?,
                address: r.get(1)?,
                workspace: r.get(2)?,
                topic_root: r.get(3)?,
                literature_root: r.get(4)?,
                card_id: r.get(5)?,
                last_title: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

fn insert_filing_cards(
    conn: &Connection,
    mut cards: Vec<FilingCard>,
    mut by_path: HashMap<String, String>,
    mut tree_ids: HashMap<String, String>,
) -> Result<()> {
    // A Side step adds a segment. A Direct step increases the final number within that segment.
    cards.sort_by_cached_key(|c| path_order(&c.address));
    let mut insert_identity = conn.prepare(INSERT_IDENTITY_SQL)?;
    let mut insert_coord = conn.prepare(INSERT_COORDINATE_SQL)?;
    let mut insert_tree = conn.prepare(INSERT_TREE_SQL)?;
    for card in cards {
        let coord_id = new_uuid()?;
        let public_key = crate::tree_id(&card.address);
        let (tree_id, parent, edge, slot) = if card.topic_root || card.literature_root {
            let role = if card.topic_root {
                TreeRole::Topic
            } else {
                TreeRole::Literature
            };
            let display_num = if card.topic_root {
                Some(public_key.parse::<u64>()?)
            } else {
                None
            };
            let tree_id = new_uuid()?;
            insert_tree.execute(params![
                tree_id,
                role.id(),
                card.workspace,
                public_key,
                display_num,
                card.card_id,
                Option::<&str>::None
            ])?;
            tree_ids.insert(public_key.to_string(), tree_id.clone());
            (tree_id, None, EdgeKind::Root, None)
        } else {
            let tree_id = tree_ids
                .get(public_key)
                .with_context(|| format!("no tree root for {}", card.address))?;
            let (parent, edge, slot) = filing_attachment(&card.address)?;
            let parent = by_path
                .get(&parent)
                .with_context(|| format!("missing filing parent {parent} of {}", card.address))?;
            (tree_id.clone(), Some(parent.as_str()), edge, slot)
        };
        insert_coord.execute(params![
            coord_id,
            tree_id,
            card.workspace,
            card.card_id,
            parent,
            edge.id(),
            slot,
            Option::<&str>::None
        ])?;
        insert_identity.execute(params![
            card.card_id,
            card.row_id,
            card.workspace,
            coord_id,
            card.address,
            card.last_title
        ])?;
        by_path.insert(card.address, coord_id);
    }
    Ok(())
}

/// Backfill filings for raw fixture inserts in one pass over the new Cards.
#[cfg(test)]
pub(crate) fn sync_raw_seeded_cards_for_test(conn: &Connection) -> Result<()> {
    registration(conn, || {
        let cards = read_filing_cards(
            conn,
            "SELECT c.row_id, coalesce(c.location, c.citation_key), c.workspace, c.is_topic, c.is_lit,
                    zt_uuid_v4(), zt_card_title(c.text)
             FROM cards c
             WHERE NOT EXISTS (SELECT 1 FROM card_identity i WHERE i.card_row = c.row_id AND i.status = 'live')",
        )?;
        if cards.is_empty() {
            return Ok(());
        }
        let by_path = conn
            .prepare(
                "SELECT coalesce(c.location, c.citation_key), i.filing_coord_id
                 FROM cards c JOIN card_identity i ON i.card_row = c.row_id WHERE i.status = 'live'",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<HashMap<_, _>>>()?;
        let tree_ids = conn
            .prepare("SELECT public_key, tree_id FROM trees")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<HashMap<_, _>>>()?;
        insert_filing_cards(conn, cards, by_path, tree_ids)
    })
}

pub(crate) fn check(conn: &Connection) -> Result<Value> {
    if !installed(conn) {
        return Ok(json!({"ok": true, "installed": false, "faults": []}));
    }
    let mut faults = Vec::new();
    for (kind, sql) in [
        (
            "card_without_identity",
            "SELECT count(*) FROM cards c LEFT JOIN card_identity i ON i.card_row = c.row_id
          WHERE i.card_id IS NULL",
        ),
        (
            "identity_without_card",
            "SELECT count(*) FROM card_identity i LEFT JOIN cards c ON c.row_id = i.card_row
          WHERE i.status = 'live' AND c.row_id IS NULL",
        ),
        (
            "filing_coord_mismatch",
            "SELECT count(*) FROM card_identity i
          LEFT JOIN cards c ON c.row_id = i.card_row
          LEFT JOIN coordinates f ON f.coord_id = i.filing_coord_id
          WHERE i.status = 'live' AND (i.owner_workspace IS NULL OR f.card_id IS NOT i.card_id
            OR f.host_workspace IS NOT i.owner_workspace OR c.workspace IS NOT i.owner_workspace)",
        ),
    ] {
        let count: i64 = conn.query_row(sql, [], |r| r.get(0))?;
        if count > 0 {
            faults.push(json!({"kind": kind, "count": count}));
        }
    }
    let mut fk = conn.prepare("PRAGMA foreign_key_check")?;
    for row in fk.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(2)?)))? {
        let (table, parent) = row?;
        if COORDINATE_TABLES.contains(&table.as_str()) {
            faults.push(json!({"kind": "foreign_key", "table": table, "parent": parent}));
        }
    }
    let filings: HashMap<String, String> = conn
        .prepare(
            "SELECT i.filing_coord_id, coalesce(c.location, c.citation_key)
         FROM card_identity i JOIN cards c ON c.row_id = i.card_row
         WHERE i.status = 'live' AND i.filing_coord_id IS NOT NULL",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for tree in all_trees(conn)? {
        let shape = match load_tree_shape(conn, &tree) {
            Ok(shape) => shape,
            Err(error) => {
                faults.push(json!({"kind": "tree_shape", "tree": tree.public_key, "detail": format!("{error:#}")}));
                continue;
            }
        };
        if filings.get(&shape.root) != Some(&tree.root_address()) {
            faults.push(json!({"kind": "filing_coord_mismatch", "tree": tree.public_key, "coord_id": shape.root}));
        }
        for (id, path) in &shape.coord {
            let Some(address) = filings.get(id) else {
                continue;
            };
            let matches = if tree.role == TreeRole::Connection {
                Handle::parse(address).is_some()
            } else {
                address == path
            };
            if !matches {
                faults.push(json!({"kind": "derived_path_mismatch", "coord_id": id, "path": path, "address": address}));
            }
        }
    }
    Ok(json!({"ok": faults.is_empty(), "installed": true, "faults": faults}))
}

#[cfg(test)]
pub(crate) fn strip_for_legacy_fixture(conn: &Connection) {
    let fk: bool = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .expect("read foreign keys");
    if fk {
        conn.pragma_update(None, "foreign_keys", "OFF")
            .expect("disable foreign keys");
    }
    conn.execute_batch(&drop_triggers_sql(conn, COORD_TRIGGERS).expect("trigger list"))
        .expect("drop coordinate triggers");
    conn.execute_batch(
        "
        DROP TABLE IF EXISTS lock_source_reviews;
        DROP TABLE IF EXISTS lock_placement_pins;
        DROP TABLE IF EXISTS lock_connections;
        DROP TABLE IF EXISTS lock_placements;
        DROP TABLE IF EXISTS lock_coordinate_seals;
        DROP TABLE IF EXISTS retired_coordinates;
        DROP TABLE IF EXISTS coordinate_edges;
        DROP TABLE IF EXISTS coordinates;
        DROP TABLE IF EXISTS trees;
        DROP TABLE IF EXISTS card_handles;
        DROP TABLE IF EXISTS card_identity;
        DELETE FROM metadata WHERE key IN ('coordinates_schema_version', 'next_connection_id');
    ",
    )
    .expect("strip coordinate schema");
    if fk {
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("restore foreign keys");
    }
}

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
    pub(crate) owner_workspace: Option<Workspace>,
    pub(crate) live: bool,
    pub(crate) filing_coord_id: Option<String>,
    pub(crate) last_address: String,
    pub(crate) last_title: String,
    #[allow(dead_code)]
    pub(crate) retired_at_rev: Option<i64>,
}

impl Identity {
    pub(crate) fn live_owner_workspace(&self) -> Result<Workspace> {
        self.owner_workspace
            .context("live Card identity missing owner_workspace")
    }
}

pub(crate) fn identity(conn: &Connection, card_id: &str) -> Result<Option<Identity>> {
    Ok(conn
        .query_row(
            "SELECT card_id, card_row, owner_workspace, status, filing_coord_id,
                    last_address, last_title, retired_at_rev FROM card_identity WHERE card_id = ?1",
            [card_id],
            |r| {
                Ok(Identity {
                    card_id: r.get(0)?,
                    card_row: r.get(1)?,
                    owner_workspace: r
                        .get::<_, Option<String>>(2)?
                        .map(|s| Workspace::from_stored(&s)),
                    live: r.get::<_, String>(3)? == "live",
                    filing_coord_id: r.get(4)?,
                    last_address: r.get(5)?,
                    last_title: r.get(6)?,
                    retired_at_rev: r.get(7)?,
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
// Trees and Coordinates
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) struct TreeRecord {
    pub(crate) tree_id: String,
    pub(crate) role: TreeRole,
    pub(crate) workspace: Workspace,
    pub(crate) public_key: String,
    pub(crate) display_num: Option<u64>,
    pub(crate) root_card_id: String,
    pub(crate) purpose: Option<String>,
    pub(crate) next_local_seq: u64,
    pub(crate) created_at: String,
}

impl TreeRecord {
    pub(crate) fn handle(&self) -> String {
        self.public_key.clone()
    }

    pub(crate) fn root_address(&self) -> String {
        match self.role {
            TreeRole::Topic => format!("{}/0", self.public_key),
            TreeRole::Literature | TreeRole::Connection => self.public_key.clone(),
        }
    }

    fn root_path(&self) -> String {
        match self.role {
            TreeRole::Topic | TreeRole::Connection => format!("{}/0", self.public_key),
            TreeRole::Literature => self.public_key.clone(),
        }
    }
}

pub(crate) type ConnTree = TreeRecord;

fn tree_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TreeRecord> {
    Ok(TreeRecord {
        tree_id: r.get(0)?,
        role: TreeRole::from_stored(&r.get::<_, String>(1)?),
        workspace: Workspace::from_stored(&r.get::<_, String>(2)?),
        public_key: r.get(3)?,
        display_num: r.get(4)?,
        root_card_id: r.get(5)?,
        purpose: r.get(6)?,
        next_local_seq: r.get(7)?,
        created_at: r.get(8)?,
    })
}

const TREE_COLUMNS: &str = "tree_id, role, workspace, public_key, display_num, root_card_id, purpose, next_local_seq, created_at";

pub(crate) fn all_trees(conn: &Connection) -> Result<Vec<TreeRecord>> {
    if !table_exists(conn, "trees")? {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {TREE_COLUMNS} FROM trees ORDER BY role, display_num, public_key"
    ))?;
    Ok(stmt
        .query_map([], tree_from_row)?
        .collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn trees(conn: &Connection) -> Result<Vec<TreeRecord>> {
    if !table_exists(conn, "trees")? {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {TREE_COLUMNS} FROM trees WHERE role = 'connection' ORDER BY display_num"
    ))?;
    Ok(stmt
        .query_map([], tree_from_row)?
        .collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn tree_by_num(conn: &Connection, num: u64) -> Result<Option<TreeRecord>> {
    if !table_exists(conn, "trees")? {
        return Ok(None);
    }
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {TREE_COLUMNS} FROM trees WHERE role = 'connection' AND display_num = ?1"
        ))?
        .query_row([num], tree_from_row)
        .optional()?)
}

pub(crate) fn tree_by_id(conn: &Connection, tree_id: &str) -> Result<Option<TreeRecord>> {
    if !table_exists(conn, "trees")? {
        return Ok(None);
    }
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {TREE_COLUMNS} FROM trees WHERE tree_id = ?1"
        ))?
        .query_row([tree_id], tree_from_row)
        .optional()?)
}

pub(crate) fn tree_by_public_key(conn: &Connection, key: &str) -> Result<Option<TreeRecord>> {
    if !table_exists(conn, "trees")? {
        return Ok(None);
    }
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {TREE_COLUMNS} FROM trees WHERE public_key = ?1"
        ))?
        .query_row([key], tree_from_row)
        .optional()?)
}

pub(crate) fn resolve_tree(conn: &Connection, name: &str) -> Result<TreeRecord> {
    let tree = if is_uuid(name) {
        tree_by_id(conn, name)?
    } else if let Some(num) = positive(crate::tree_id(name).trim_start_matches('~')) {
        tree_by_num(conn, num)?
    } else {
        None
    };
    tree.filter(|tree| tree.role == TreeRole::Connection)
        .with_context(|| format!("Connection tree {name} does not exist"))
}

pub(crate) fn resolve_any_tree(conn: &Connection, name: &str) -> Result<TreeRecord> {
    if is_uuid(name)
        && let Some(tree) = tree_by_id(conn, name)?
    {
        return Ok(tree);
    }
    tree_by_public_key(conn, crate::tree_id(name))?
        .with_context(|| format!("Tree {name} does not exist"))
}

#[allow(dead_code)]
pub(crate) fn home_tree_of_address(conn: &Connection, address: &str) -> Result<Option<TreeRecord>> {
    let Some(address) = canonical(conn, address)? else {
        return Ok(None);
    };
    if !table_exists(conn, "trees")? {
        return Ok(None);
    }
    let tree_id: Option<String> = conn
        .query_row(
            "SELECT f.tree_id FROM cards c JOIN card_identity i ON i.card_row = c.row_id
         JOIN coordinates f ON f.coord_id = i.filing_coord_id
         WHERE c.location = ?1 OR c.citation_key = ?1",
            [&address],
            |r| r.get(0),
        )
        .optional()?;
    match tree_id {
        Some(id) => tree_by_id(conn, &id),
        None => Ok(None),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EdgeKind {
    Root,
    Direct,
    Side,
}

impl EdgeKind {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Direct => "direct",
            Self::Side => "side",
        }
    }

    pub(crate) fn from_stored(text: &str) -> EdgeKind {
        match text {
            "root" => Self::Root,
            "direct" => Self::Direct,
            "side" => Self::Side,
            _ => unreachable!("invalid stored coordinate edge {text}"),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CoordinateRow {
    pub(crate) coord_id: String,
    pub(crate) tree_id: String,
    pub(crate) host_workspace: Workspace,
    pub(crate) card_id: String,
    pub(crate) parent_coord_id: Option<String>,
    pub(crate) edge: EdgeKind,
    pub(crate) slot: Option<u64>,
    pub(crate) gloss: Option<String>,
}

pub(crate) type Placement = CoordinateRow;

const COORDINATE_COLUMNS: &str =
    "coord_id, tree_id, host_workspace, card_id, parent_coord_id, edge, slot, gloss";

fn coordinate_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<CoordinateRow> {
    Ok(CoordinateRow {
        coord_id: r.get(0)?,
        tree_id: r.get(1)?,
        host_workspace: Workspace::from_stored(&r.get::<_, String>(2)?),
        card_id: r.get(3)?,
        parent_coord_id: r.get(4)?,
        edge: EdgeKind::from_stored(&r.get::<_, String>(5)?),
        slot: r.get(6)?,
        gloss: r.get(7)?,
    })
}

pub(crate) fn coordinate(conn: &Connection, coord_id: &str) -> Result<Option<CoordinateRow>> {
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {COORDINATE_COLUMNS} FROM coordinates WHERE coord_id = ?1"
        ))?
        .query_row([coord_id], coordinate_from_row)
        .optional()?)
}

pub(crate) fn coordinates_of_tree(conn: &Connection, tree_id: &str) -> Result<Vec<CoordinateRow>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COORDINATE_COLUMNS} FROM coordinates WHERE tree_id = ?1 ORDER BY coord_id"
    ))?;
    Ok(stmt
        .query_map([tree_id], coordinate_from_row)?
        .collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn coordinates_of_card(conn: &Connection, card_id: &str) -> Result<Vec<CoordinateRow>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {COORDINATE_COLUMNS} FROM coordinates WHERE card_id = ?1 ORDER BY tree_id, coord_id"
    ))?;
    Ok(stmt
        .query_map([card_id], coordinate_from_row)?
        .collect::<rusqlite::Result<_>>()?)
}

#[derive(Clone, Debug)]
pub(crate) struct TreeShape {
    pub(crate) tree: TreeRecord,
    pub(crate) by_id: BTreeMap<String, CoordinateRow>,
    pub(crate) root: String,
    pub(crate) direct: BTreeMap<String, String>,
    pub(crate) side: BTreeMap<String, Vec<String>>,
    pub(crate) coord: BTreeMap<String, String>,
    pub(crate) by_coord: BTreeMap<String, String>,
    pub(crate) order: Vec<String>,
}

impl TreeShape {
    pub(crate) fn subtree(&self, coord_id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![coord_id];
        while let Some(id) = stack.pop() {
            out.push(id.to_string());
            if let Some(next) = self.direct.get(id) {
                stack.push(next);
            }
            if let Some(sides) = self.side.get(id) {
                stack.extend(sides.iter().rev().map(String::as_str));
            }
        }
        out
    }

    pub(crate) fn coords_of_card(&self, card_id: &str) -> Vec<&CoordinateRow> {
        self.order
            .iter()
            .map(|id| &self.by_id[id])
            .filter(|row| row.card_id == card_id)
            .collect()
    }
}

fn direct_path(path: &str) -> Result<String> {
    if let Some(root) = path.strip_suffix("/0") {
        return Ok(format!("{root}/1"));
    }
    if !path.contains('/') {
        return Ok(format!("{path}/1"));
    }
    let tail = crate::last_segment(path);
    if crate::valid_positive_number(tail) {
        let next = tail
            .parse::<u64>()?
            .checked_add(1)
            .context("Direct Coordinate path overflow")?;
        Ok(crate::replace_last_segment(path, &next.to_string()))
    } else {
        Ok(format!("{path}|1"))
    }
}

pub(crate) fn load_tree_shape(conn: &Connection, tree: &TreeRecord) -> Result<TreeShape> {
    shape_from_rows(tree, coordinates_of_tree(conn, &tree.tree_id)?, false)
}

fn shape_from_rows(
    tree: &TreeRecord,
    rows: Vec<CoordinateRow>,
    allow_unreachable: bool,
) -> Result<TreeShape> {
    let mut by_id = BTreeMap::new();
    let mut root = None;
    let mut direct = BTreeMap::new();
    let mut side: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in rows {
        let id = row.coord_id.clone();
        if row.host_workspace != tree.workspace {
            bail!("Coordinate {id} differs from its Tree Workspace");
        }
        match row.edge {
            EdgeKind::Root => {
                if row.card_id != tree.root_card_id || root.replace(id.clone()).is_some() {
                    bail!("invalid root Coordinate in Tree {}", tree.public_key);
                }
            }
            EdgeKind::Direct => {
                let parent = row
                    .parent_coord_id
                    .clone()
                    .context("Direct Coordinate missing its parent")?;
                if direct.insert(parent, id.clone()).is_some() {
                    bail!("two Direct Coordinates have the same parent");
                }
            }
            EdgeKind::Side => {
                let parent = row
                    .parent_coord_id
                    .clone()
                    .context("Side Coordinate missing its parent")?;
                if row.slot.is_none_or(|slot| slot == 0) {
                    bail!("invalid Side slot for Coordinate {id}");
                }
                side.entry(parent).or_default().push(id.clone());
            }
        }
        by_id.insert(id, row);
    }
    let root = root.context("Tree has no root Coordinate")?;
    // Stored Side Coordinates can also appear under Topic roots.
    if tree.role != TreeRole::Topic && side.contains_key(&root) {
        bail!("a Side branch starts at the root");
    }
    for children in side.values_mut() {
        children.sort_by_key(|child| by_id[child].slot);
    }
    let mut coord = BTreeMap::new();
    let mut by_coord = BTreeMap::new();
    let mut order = Vec::new();
    let mut stack = vec![(root.clone(), tree.root_path())];
    while let Some((id, path)) = stack.pop() {
        if coord.insert(id.clone(), path.clone()).is_some()
            || by_coord.insert(path.clone(), id.clone()).is_some()
        {
            bail!("Coordinate cycle or duplicate path {path}");
        }
        if let Some(next) = direct.get(&id) {
            stack.push((next.clone(), direct_path(&path)?));
        }
        if let Some(children) = side.get(&id) {
            for child in children.iter().rev() {
                let label =
                    crate::number_to_side_label(by_id[child].slot.expect("checked Side slot"));
                stack.push((child.clone(), format!("{path}|{label}")));
            }
        }
        order.push(id);
    }
    if !allow_unreachable && coord.len() != by_id.len() {
        bail!(
            "{} Coordinate(s) cannot be reached from the root",
            by_id.len() - coord.len()
        );
    }
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

pub(crate) fn coordinate_path(conn: &Connection, row: &CoordinateRow) -> Result<String> {
    let tree = tree_by_id(conn, &row.tree_id)?.context("Coordinate's Tree is missing")?;
    let mut chain = Vec::new();
    let mut visited = BTreeSet::new();
    let mut current = row.clone();
    while let Some(parent) = &current.parent_coord_id {
        if !visited.insert(current.coord_id.clone()) {
            bail!("cycle at Coordinate {}", current.coord_id);
        }
        chain.push((current.edge, current.slot));
        current = coordinate(conn, parent)?.context("Coordinate's parent is missing")?;
        if current.tree_id != row.tree_id {
            bail!("Coordinate's parent is in a different Tree");
        }
    }
    if current.edge != EdgeKind::Root || current.card_id != tree.root_card_id {
        bail!("Coordinate does not lead to its Tree root");
    }
    let mut path = tree.root_path();
    for (edge, slot) in chain.into_iter().rev() {
        path = match edge {
            EdgeKind::Direct => direct_path(&path)?,
            EdgeKind::Side => format!(
                "{path}|{}",
                crate::number_to_side_label(slot.context("missing Side slot")?)
            ),
            EdgeKind::Root => bail!("root Coordinate has a parent"),
        };
    }
    Ok(path)
}

/// Resolve a path using the structural indexes. The numeric runs in a path stand for a chain
/// of Direct edges, so a failed lookup stops at the first missing edge.
fn coordinate_at(
    conn: &Connection,
    tree: &TreeRecord,
    path: &str,
) -> Result<Option<CoordinateRow>> {
    let mut root_lookup = conn.prepare_cached(&format!(
        "SELECT {COORDINATE_COLUMNS} FROM coordinates WHERE tree_id = ?1 AND edge = 'root'"
    ))?;
    let mut current = root_lookup
        .query_row([&tree.tree_id], coordinate_from_row)
        .optional()?;
    if path == tree.root_path() {
        return Ok(current);
    }
    let tail = path
        .strip_prefix(&tree.public_key)
        .and_then(|tail| tail.strip_prefix('/'));
    let Some(tail) = tail else { return Ok(None) };
    let mut direct = conn.prepare_cached(&format!(
        "SELECT {COORDINATE_COLUMNS} FROM coordinates
         WHERE tree_id = ?1 AND parent_coord_id = ?2 AND edge = 'direct'"
    ))?;
    let mut side = conn.prepare_cached(&format!(
        "SELECT {COORDINATE_COLUMNS} FROM coordinates
         WHERE tree_id = ?1 AND parent_coord_id = ?2 AND edge = 'side' AND slot = ?3"
    ))?;
    for (i, segment) in tail.split('|').enumerate() {
        if i == 0 && segment == "0" && tree.role != TreeRole::Literature {
            continue;
        }
        if let Some(steps) = positive(segment) {
            for _ in 0..steps {
                let Some(row) = current else { return Ok(None) };
                current = direct
                    .query_row(params![tree.tree_id, row.coord_id], coordinate_from_row)
                    .optional()?;
            }
        } else if i > 0 && crate::valid_side_label(segment) {
            let Some(row) = current else { return Ok(None) };
            let slot = side_slot(segment)?;
            current = side
                .query_row(
                    params![tree.tree_id, row.coord_id, slot],
                    coordinate_from_row,
                )
                .optional()?;
        } else {
            return Ok(None);
        }
    }
    // The grammar can parse a run of numbers as consecutive Direct steps, but only the derived
    // spelling names the occurrence (for example a path cannot use /1|1 in place of /2).
    if let Some(row) = current
        && coordinate_path(conn, &row)? == path
    {
        return Ok(Some(row));
    }
    Ok(None)
}

pub(crate) fn resolve_coordinate_path(conn: &Connection, text: &str) -> Result<CoordinateRow> {
    let tree = resolve_any_tree(conn, text)?;
    coordinate_at(conn, &tree, text)?.with_context(|| format!("Coordinate {text} does not exist"))
}

pub(crate) fn resolve_coord(conn: &Connection, text: &str) -> Result<CoordinateRow> {
    let bookmark = place_bookmark(text);
    let id = bookmark.unwrap_or(text);
    if bookmark.is_some() || is_uuid(id) {
        if let Some(row) = coordinate(conn, id)? {
            return Ok(row);
        }
        let retired: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM retired_coordinates WHERE coord_id = ?1)",
            [id],
            |r| r.get(0),
        )?;
        let name = if bookmark.is_some() {
            "placement"
        } else {
            "coordinate"
        };
        if retired {
            bail!("{name} removed: {id}");
        }
        if bookmark.is_none()
            && let Some(tree) = tree_by_public_key(conn, text)?
            && tree.role == TreeRole::Literature
            && let Some(row) = coordinate_at(conn, &tree, text)?
        {
            return Ok(row);
        }
        bail!("{name} does not exist: {id}");
    }
    if is_retired_topic_mount(text) {
        bail!("{}", coordinate_link_hint(text));
    }
    resolve_coordinate_path(conn, text)
}

const INSERT_IDENTITY_SQL: &str =
    "INSERT INTO card_identity(card_id, card_row, owner_workspace, status, filing_coord_id,
                               last_address, last_title) VALUES(?1, ?2, ?3, 'live', ?4, ?5, ?6)";
const INSERT_TREE_SQL: &str =
    "INSERT INTO trees(tree_id, role, workspace, public_key, display_num, root_card_id, purpose, created_at)
     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))";
const INSERT_COORDINATE_SQL: &str =
    "INSERT INTO coordinates(coord_id, tree_id, host_workspace, card_id, parent_coord_id, edge, slot, gloss)
     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";

/// Roll back the whole group if an insert or the outermost RELEASE fails.
fn registration<T>(conn: &Connection, work: impl FnOnce() -> Result<T>) -> Result<T> {
    conn.execute_batch("SAVEPOINT register_card_coordinate")?;
    let result = work().and_then(|value| {
        conn.execute_batch("RELEASE register_card_coordinate")?;
        Ok(value)
    });
    if result.is_err() {
        conn.execute_batch(
            "ROLLBACK TO register_card_coordinate; RELEASE register_card_coordinate",
        )?;
    }
    result
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn register_root_card_tx(
    conn: &Connection,
    row_id: i64,
    role: TreeRole,
    public_key: &str,
    display_num: Option<u64>,
    workspace: Workspace,
    title: &str,
    purpose: Option<&str>,
) -> Result<(String, String, String)> {
    registration(conn, || {
        let card_id = new_uuid()?;
        let tree_id = new_uuid()?;
        let root_coord_id = new_uuid()?;
        let address = if role == TreeRole::Topic {
            format!("{public_key}/0")
        } else {
            public_key.to_string()
        };
        conn.execute(
            INSERT_IDENTITY_SQL,
            params![
                card_id,
                row_id,
                workspace.id(),
                root_coord_id,
                address,
                title
            ],
        )?;
        conn.execute(
            INSERT_TREE_SQL,
            params![
                tree_id,
                role.id(),
                workspace.id(),
                public_key,
                display_num,
                card_id,
                purpose
            ],
        )?;
        conn.execute(
            INSERT_COORDINATE_SQL,
            params![
                root_coord_id,
                tree_id,
                workspace.id(),
                card_id,
                Option::<&str>::None,
                "root",
                Option::<u64>::None,
                Option::<&str>::None
            ],
        )?;
        if role == TreeRole::Connection {
            conn.execute(
                "INSERT INTO card_handles(handle, card_id, origin_tree_id, local_seq) VALUES(?1, ?2, ?3, 0)",
                params![public_key, card_id, tree_id],
            )?;
        }
        Ok((card_id, tree_id, root_coord_id))
    })
}

fn path_order(path: &str) -> (bool, usize, u64) {
    (
        path.contains('/'),
        path.bytes().filter(|&byte| byte == b'|').count(),
        crate::last_segment(path).parse::<u64>().unwrap_or(0),
    )
}

fn side_slot(label: &str) -> Result<u64> {
    // SQLite INTEGER is signed, so reject labels whose slots exceed its range.
    let mut slot = 0_u64;
    for byte in label.bytes() {
        slot = slot
            .checked_mul(26)
            .and_then(|n| n.checked_add(u64::from(byte - b'a' + 1)))
            .filter(|&n| n <= i64::MAX as u64)
            .context("Side slot exceeds SQLite INTEGER range")?;
    }
    Ok(slot)
}

fn filing_attachment(location: &str) -> Result<(String, EdgeKind, Option<u64>)> {
    let parent = crate::parent_location(location)
        .with_context(|| format!("{location} has no filing parent"))?;
    let segment = crate::last_segment(location);
    if crate::valid_side_label(segment) {
        // Use the common converter after checking that its result is representable in SQLite.
        side_slot(segment)?;
        Ok((
            parent,
            EdgeKind::Side,
            Some(crate::side_label_to_number(segment)),
        ))
    } else {
        Ok((parent, EdgeKind::Direct, None))
    }
}

pub(crate) fn register_regular_card_tx(
    conn: &Connection,
    row_id: i64,
    location: &str,
    workspace: Workspace,
    title: &str,
) -> Result<(String, String)> {
    register_regular_card_at_tx(conn, row_id, location, workspace, title, &new_uuid()?)
}

/// An insertion can reserve the new Coordinate's ID before moving its future Direct successor.
pub(crate) fn register_regular_card_at_tx(
    conn: &Connection,
    row_id: i64,
    location: &str,
    workspace: Workspace,
    title: &str,
    coord_id: &str,
) -> Result<(String, String)> {
    registration(conn, || {
        let tree = tree_by_public_key(conn, crate::tree_id(location))?
            .context("filing Tree does not exist")?;
        let (parent, edge, slot) = filing_attachment(location)?;
        let parent = coordinate_at(conn, &tree, &parent)?
            .with_context(|| format!("filing parent {parent} does not exist"))?;
        let card_id = new_uuid()?;
        conn.execute(
            INSERT_IDENTITY_SQL,
            params![card_id, row_id, workspace.id(), coord_id, location, title],
        )?;
        conn.execute(
            INSERT_COORDINATE_SQL,
            params![
                coord_id,
                tree.tree_id,
                workspace.id(),
                card_id,
                parent.coord_id,
                edge.id(),
                slot,
                Option::<&str>::None
            ],
        )?;
        Ok((card_id, coord_id.to_string()))
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn register_connection_native_tx(
    conn: &Connection,
    row_id: i64,
    tree: &TreeRecord,
    handle: &str,
    local_seq: u64,
    title: &str,
    coord_id: &str,
    parent_coord_id: &str,
    edge: EdgeKind,
    slot: Option<u64>,
    gloss: Option<&str>,
) -> Result<String> {
    registration(conn, || {
        let card_id = new_uuid()?;
        conn.execute(
            INSERT_IDENTITY_SQL,
            params![
                card_id,
                row_id,
                tree.workspace.id(),
                coord_id,
                handle,
                title
            ],
        )?;
        conn.execute(
            "INSERT INTO card_handles(handle, card_id, origin_tree_id, local_seq) VALUES(?1, ?2, ?3, ?4)",
            params![handle, card_id, tree.tree_id, local_seq],
        )?;
        conn.execute(
            INSERT_COORDINATE_SQL,
            params![
                coord_id,
                tree.tree_id,
                tree.workspace.id(),
                card_id,
                parent_coord_id,
                edge.id(),
                slot,
                gloss
            ],
        )?;
        Ok(card_id)
    })
}

pub(crate) fn occupied_paths_in_tree(
    conn: &Connection,
    tree_public_key: &str,
) -> Result<BTreeSet<String>> {
    let Some(tree) = tree_by_public_key(conn, tree_public_key)? else {
        return Ok(BTreeSet::new());
    };
    Ok(load_tree_shape(conn, &tree)?.by_coord.into_keys().collect())
}

pub(crate) fn secondary_coordinate_at(
    conn: &Connection,
    path: &str,
) -> Result<Option<(String, String)>> {
    if !path.contains('/') || path.starts_with('~') || path.contains('+') {
        return Ok(None);
    }
    // The great majority of authored targets identify a live filing Card. Avoid a topology
    // traversal for those paths, but keep Missing secondary occurrences addressable by bookmark.
    if canonical(conn, path)?.is_some() {
        return Ok(None);
    }
    let Some(tree) = tree_by_public_key(conn, crate::tree_id(path))? else {
        return Ok(None);
    };
    let Some(row) = coordinate_at(conn, &tree, path)? else {
        return Ok(None);
    };
    let Some(card) = identity(conn, &row.card_id)? else {
        return Ok(None);
    };
    if card.filing_coord_id.as_deref() == Some(&row.coord_id) {
        return Ok(None);
    }
    Ok(Some((card.last_address, row.coord_id)))
}

pub(crate) fn secondary_coordinate_link_hint(
    target: &str,
    canonical: &str,
    coord_id: &str,
) -> String {
    format!(
        "Link target `{target}` is a Secondary Coordinate. Cite a live Card at its canonical address `[[{canonical}]]` or bookmark this occurrence with `[label](place:{coord_id})`."
    )
}

pub(crate) fn check_model_term_splice_guard(
    conn: &Connection,
    tree: &TreeRecord,
    parent_coord_id: &str,
    edge: EdgeKind,
) -> Result<()> {
    if tree.role == TreeRole::Connection || edge != EdgeKind::Direct {
        return Ok(());
    }
    let pair: Option<(String, String)> = conn.query_row(
        "SELECT p.last_address, d.last_address
         FROM coordinates parent JOIN card_identity p ON p.card_id = parent.card_id
         JOIN coordinates c ON c.tree_id = parent.tree_id AND c.parent_coord_id = parent.coord_id
         JOIN card_identity d ON d.card_id = c.card_id AND d.filing_coord_id = c.coord_id
         WHERE parent.coord_id = ?1 AND p.filing_coord_id = ?1 AND c.tree_id = ?2 AND c.edge = 'direct'",
        params![parent_coord_id, tree.tree_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional()?;
    if let Some((parent, child)) = pair {
        let parent = crate::load_card(conn, &parent)?;
        let child = crate::load_card(conn, &child)?;
        let model_part = parent.card_type == crate::CardType::Model
            && child.card_type == crate::CardType::Model
            && child.facet.is_some();
        let term_part = parent.card_type == crate::CardType::Term
            && child.card_type == crate::CardType::Term
            && crate::term::role(
                conn,
                &child.address,
                child.card_type,
                child.term_name.as_deref(),
            )?
            .is_some_and(|(role, _)| role == crate::term::Role::Part);
        if model_part || term_part {
            bail!(
                "cannot splice a Secondary Coordinate into the filing chain from {} to {}; place as a Side branch, or arrange freely in a Connection tree ~n",
                parent.address,
                child.address
            );
        }
    }
    Ok(())
}

pub(crate) fn sync_filing_moves_tx(
    conn: &Connection,
    mapping: &BTreeMap<String, String>,
) -> Result<()> {
    sync_filing_moves_with_insert_tx(conn, mapping, None)
}

pub(crate) fn sync_filing_moves_with_insert_tx(
    conn: &Connection,
    mapping: &BTreeMap<String, String>,
    pending: Option<(&str, &str)>,
) -> Result<()> {
    if mapping.is_empty() || !table_exists(conn, "trees")? {
        return Ok(());
    }
    registration(conn, || {
        let mut affected = BTreeMap::new();
        let mut renamed_keys = BTreeMap::new();
        let mut keys: BTreeSet<_> = mapping
            .iter()
            .flat_map(|(from, to)| [crate::tree_id(from), crate::tree_id(to)])
            .collect();
        for (from, to) in mapping {
            if let Some(tree) = tree_by_public_key(conn, from)?
                && tree.role == TreeRole::Literature
            {
                renamed_keys.insert(to.clone(), tree.clone());
                keys.remove(to.as_str());
            }
        }
        let mut old_paths = BTreeMap::new();
        let mut rows = BTreeMap::new();
        for key in keys {
            let Some(tree) = tree_by_public_key(conn, key)? else {
                continue;
            };
            if tree.role == TreeRole::Connection {
                continue;
            }
            // A span delete can temporarily leave its surviving successor without a parent.
            let shape = shape_from_rows(&tree, coordinates_of_tree(conn, &tree.tree_id)?, true)?;
            old_paths.extend(shape.by_coord);
            rows.extend(shape.by_id);
            affected.insert(key.to_string(), tree);
        }

        let mut lookup_filing = conn.prepare_cached(
            "SELECT i.filing_coord_id FROM cards c JOIN card_identity i ON i.card_row = c.row_id
             WHERE c.location = ?1 OR c.citation_key = ?1",
        )?;
        let mut moved = BTreeMap::new();
        let mut destinations = BTreeMap::new();
        for (from, to) in mapping {
            // Use the filing cache for survivors of an in-progress span delete. Secondary paths
            // are found only in the coordinate outline.
            let id = lookup_filing
                .query_row([from], |r| r.get::<_, String>(0))
                .optional()?
                .or_else(|| old_paths.get(from).cloned());
            let Some(id) = id else { continue };
            if destinations.insert(to.clone(), id.clone()).is_some() {
                bail!("two Coordinates move to {to}");
            }
            moved.insert(id, to);
        }
        if let Some((path, id)) = pending
            && destinations
                .insert(path.to_string(), id.to_string())
                .is_some()
        {
            bail!("the inserted Card would share Coordinate path {path}");
        }

        let renamed_prefixes: BTreeMap<_, _> = renamed_keys
            .iter()
            .map(|(new, old)| (old.public_key.as_str(), new.as_str()))
            .collect();
        let mut filings = conn.prepare_cached(
            "SELECT i.filing_coord_id, i.last_address FROM coordinates c
             JOIN card_identity i ON i.card_id = c.card_id AND i.filing_coord_id = c.coord_id
             WHERE c.tree_id = ?1 AND i.status = 'live'",
        )?;
        let mut unmoved_filings = BTreeMap::new();
        for tree in affected.values() {
            for filing in filings.query_map([&tree.tree_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })? {
                let (id, previous) = filing?;
                if moved.contains_key(&id) || rows[&id].edge == EdgeKind::Root {
                    continue;
                }
                let target = match renamed_prefixes.get(tree.public_key.as_str()) {
                    Some(key) => {
                        let suffix = previous
                            .strip_prefix(tree.public_key.as_str())
                            .with_context(|| {
                                format!("filing address {previous} is outside its Tree")
                            })?;
                        format!("{key}{suffix}")
                    }
                    None => previous,
                };
                unmoved_filings.insert(id, target);
            }
        }
        let tree_at = |key: &str| renamed_keys.get(key).or_else(|| affected.get(key));
        let mut children = BTreeMap::<&str, Vec<&CoordinateRow>>::new();
        for row in rows.values() {
            if let Some(parent) = &row.parent_coord_id {
                children.entry(parent).or_default().push(row);
            }
        }
        let mut next_trees = BTreeMap::new();
        let mut projected = BTreeSet::new();
        let mut queue: Vec<_> = moved
            .iter()
            .map(|(id, path)| (id.as_str(), path.to_string()))
            .collect();
        // Secondary children that keep their parent follow it to its new path. Project those
        // paths before resolving the final parents of explicitly moved Filing Coordinates.
        while let Some((id, path)) = queue.pop() {
            let key = crate::tree_id(&path);
            let tree =
                tree_at(key).with_context(|| format!("destination Tree {key} does not exist"))?;
            for child in children.get(id).into_iter().flatten() {
                if moved.contains_key(&child.coord_id)
                    || unmoved_filings.contains_key(&child.coord_id)
                {
                    continue;
                }
                let child_path = match child.edge {
                    EdgeKind::Direct => direct_path(&path)?,
                    EdgeKind::Side => {
                        let slot = child.slot.context("Side Coordinate has no slot")?;
                        format!("{path}|{}", crate::number_to_side_label(slot))
                    }
                    EdgeKind::Root => bail!("root Coordinate has a parent"),
                };
                if destinations
                    .insert(child_path.clone(), child.coord_id.clone())
                    .is_some()
                {
                    bail!("two Coordinates move to {child_path}");
                }
                projected.insert(child.coord_id.as_str());
                if child.tree_id != tree.tree_id {
                    next_trees.insert(child.coord_id.clone(), tree.tree_id.clone());
                }
                queue.push((child.coord_id.as_str(), child_path));
            }
        }
        let parent_at = |key: &str, path: &str| {
            destinations.get(path).or_else(|| {
                // Unmoved Coordinates in a renamed Literature tree still have their old prefix
                // in the pre-write outline.
                let previous = renamed_keys
                    .get(key)
                    .map(|old| format!("{}{}", old.public_key, &path[key.len()..]));
                old_paths
                    .get(previous.as_deref().unwrap_or(path))
                    .filter(|id| !moved.contains_key(*id) && !projected.contains(id.as_str()))
            })
        };

        let mut changes = Vec::new();
        // A filing path can stay the same while a different Coordinate takes its parent path.
        let mapped_paths = moved.iter().map(|(id, to)| (id, to.as_str()));
        let fixed_paths = unmoved_filings.iter().map(|(id, to)| (id, to.as_str()));
        for (id, to) in mapped_paths.chain(fixed_paths) {
            let row = rows
                .get(id)
                .with_context(|| format!("missing Coordinate {id} in move plan"))?;
            if row.edge == EdgeKind::Root {
                if !renamed_keys.contains_key(to) {
                    bail!("a defining Tree root cannot move");
                }
                continue;
            }
            let key = crate::tree_id(to);
            let tree =
                tree_at(key).with_context(|| format!("destination Tree {key} does not exist"))?;
            let (parent_path, edge, slot) = filing_attachment(to)?;
            let parent = parent_at(key, &parent_path)
                .with_context(|| format!("destination parent {parent_path} does not exist"))?;
            if row.host_workspace != tree.workspace {
                bail!("cannot move Coordinate {id} across Workspaces");
            }
            if row.tree_id != tree.tree_id {
                next_trees.insert(id.clone(), tree.tree_id.clone());
            }
            if row.tree_id != tree.tree_id
                || row.parent_coord_id.as_ref() != Some(parent)
                || row.edge != edge
                || row.slot != slot
            {
                changes.push((id, to, tree, parent, edge, slot));
            }
        }

        // Park under each Coordinate's own ID at a temporary Side slot above every existing
        // slot. Both Direct and Side children can stay in place while their parent is parked.
        if !changes.is_empty() {
            let max_slot: i64 =
                conn.query_row("SELECT coalesce(max(slot), 0) FROM coordinates", [], |r| {
                    r.get(0)
                })?;
            let base = max_slot
                .checked_add(1_000_000)
                .context("temporary Side slot overflow")?;
            let mut park = conn.prepare_cached(
                "UPDATE coordinates SET parent_coord_id = coord_id, edge = 'side', slot = ?2 WHERE coord_id = ?1",
            )?;
            for (i, (id, _, _, _, _, _)) in changes.iter().enumerate() {
                let slot = base
                    .checked_add(i as i64)
                    .context("temporary Side slot overflow")?;
                park.execute(params![id, slot])?;
            }
        }
        changes.sort_by_cached_key(|(_, path, _, _, _, _)| path_order(path));
        for (id, _, tree, parent, edge, slot) in changes {
            conn.execute(
                "UPDATE coordinates SET tree_id = ?2, host_workspace = ?3, parent_coord_id = ?4, edge = ?5, slot = ?6
                 WHERE coord_id = ?1",
                params![id, tree.tree_id, tree.workspace.id(), parent, edge.id(), slot],
            )?;
        }
        for (id, tree) in &next_trees {
            if !moved.contains_key(id) {
                conn.execute(
                    "UPDATE coordinates SET tree_id = ?2 WHERE coord_id = ?1",
                    params![id, tree],
                )?;
            }
        }
        let mut edges = BTreeMap::new();
        for id in next_trees.keys() {
            let row = &rows[id];
            let mut query = conn.prepare_cached(
                "SELECT connection_id, from_coord_id, to_coord_id FROM coordinate_edges
                 WHERE tree_id = ?2 AND (from_coord_id = ?1 OR to_coord_id = ?1)",
            )?;
            for edge in query.query_map(params![id, row.tree_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })? {
                let (edge, from, to) = edge?;
                let from_tree = next_trees.get(&from).unwrap_or(&rows[&from].tree_id);
                let to_tree = next_trees.get(&to).unwrap_or(&rows[&to].tree_id);
                if from_tree != to_tree {
                    bail!(
                        "moving these Coordinates would split Tree connection {edge}; resolve the connection before moving"
                    );
                }
                edges.insert(edge, from_tree.clone());
            }
        }
        for (id, tree) in edges {
            conn.execute(
                "UPDATE coordinate_edges SET tree_id = ?2 WHERE connection_id = ?1",
                params![id, tree],
            )?;
        }
        for (new_key, old_tree) in &renamed_keys {
            conn.execute(
                "UPDATE trees SET public_key = ?2 WHERE tree_id = ?1",
                params![old_tree.tree_id, new_key],
            )?;
        }
        for tree in affected.values() {
            let tree = tree_by_id(conn, &tree.tree_id)?.context("moved Tree is missing")?;
            let mut rows = coordinates_of_tree(conn, &tree.tree_id)?;
            if let Some((path, id)) = pending
                && crate::tree_id(path) == tree.public_key
            {
                let (parent, edge, slot) = filing_attachment(path)?;
                let parent_coord_id = parent_at(&tree.public_key, &parent)
                    .with_context(|| format!("inserted Card's parent {parent} does not exist"))?;
                rows.push(CoordinateRow {
                    coord_id: id.to_string(),
                    tree_id: tree.tree_id.clone(),
                    host_workspace: tree.workspace,
                    card_id: String::new(),
                    parent_coord_id: Some(parent_coord_id.clone()),
                    edge,
                    slot,
                    gloss: None,
                });
            }
            let shape = shape_from_rows(&tree, rows, false)?;
            for filing in filings.query_map([&tree.tree_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })? {
                let (id, previous) = filing?;
                let expected = moved
                    .get(&id)
                    .map(|path| path.as_str())
                    .or_else(|| unmoved_filings.get(&id).map(String::as_str))
                    .unwrap_or(&previous);
                if shape.coord[&id] != expected {
                    bail!(
                        "filing move would give {previous} the unintended path {}",
                        shape.coord[&id]
                    );
                }
            }
        }
        Ok(())
    })
}

pub(crate) fn tree_delete_classification(
    conn: &Connection,
    tree_public_key: &str,
    keep_root: bool,
) -> Result<(BTreeSet<String>, Vec<(String, String, String)>)> {
    if !installed(conn) {
        return Ok((BTreeSet::new(), Vec::new()));
    }
    let tree = tree_by_public_key(conn, tree_public_key)?
        .with_context(|| format!("Tree `{tree_public_key}` is missing"))?;
    classify_tree_filings(conn, &tree, keep_root)
}

fn classify_tree_filings(
    conn: &Connection,
    tree: &TreeRecord,
    keep_root: bool,
) -> Result<(BTreeSet<String>, Vec<(String, String, String)>)> {
    let mut filings = conn.prepare_cached(
        "SELECT i.card_id, coalesce(c.location, c.citation_key), i.owner_workspace
         FROM coordinates f
         JOIN card_identity i ON i.card_id = f.card_id AND i.filing_coord_id = f.coord_id
         JOIN cards c ON c.row_id = i.card_row
         WHERE f.tree_id = ?1 AND i.status = 'live' AND (?2 = 0 OR i.card_id <> ?3)
         ORDER BY i.card_id",
    )?;
    let rows = filings.query_map(params![tree.tree_id, keep_root, tree.root_card_id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut lookup = conn.prepare_cached(
        "SELECT c.coord_id, t.role, t.tree_id, t.public_key
         FROM coordinates c JOIN trees t ON t.tree_id = c.tree_id
         WHERE c.card_id = ?1 AND c.tree_id <> ?2 AND c.host_workspace = ?3
         ORDER BY (t.role = 'connection') DESC, c.coord_id LIMIT 1",
    )?;
    let mut delete_set = BTreeSet::new();
    let mut reanchor = Vec::new();
    for row in rows {
        let (card_id, address, workspace) = row?;
        // Deleting a Topic/Literature tree deletes its filed Cards (turning external mounts into
        // Missing coordinates); deleting a Connection tree always retires its own defining root.
        if tree.role != TreeRole::Connection || card_id == tree.root_card_id {
            delete_set.insert(address);
            continue;
        }
        let survivor = lookup
            .query_row(params![card_id, tree.tree_id, workspace], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .optional()?;
        let Some((coord_id, role, tree_id, key)) = survivor else {
            delete_set.insert(address);
            continue;
        };
        if role != TreeRole::Connection.id() {
            bail!(
                "Card `{address}` survives in Tree `{key}`; use refile_card with place:{coord_id} before deleting Connection tree `{}`",
                tree.public_key
            );
        }
        crate::coord_lock::ensure_host_unlocked(conn, &tree_id, &key)?;
        crate::coord_lock::ensure_card_not_pinned(conn, &card_id, "pinned-move")?;
        reanchor.push((card_id, coord_id, address));
    }
    Ok((delete_set, reanchor))
}

#[derive(Default)]
struct DeleteCoordinates {
    removed: BTreeSet<String>,
    non_connection_filings: BTreeSet<String>,
    missing: BTreeSet<String>,
    trees: BTreeMap<String, TreeRecord>,
    reanchor: Vec<(String, String, String)>,
}

impl DeleteCoordinates {
    fn include_tree(
        &mut self,
        conn: &Connection,
        tree: TreeRecord,
        keep_root: bool,
        delete_set: &BTreeSet<String>,
    ) -> Result<()> {
        let (retiring, reanchor) = classify_tree_filings(conn, &tree, keep_root)?;
        if !retiring.is_subset(delete_set)
            || reanchor
                .iter()
                .any(|(_, _, address)| delete_set.contains(address))
        {
            bail!(
                "stale plan: the Cards filed in Tree `{}` changed",
                tree.public_key
            );
        }
        self.reanchor.extend(reanchor);
        self.removed.extend(
            coordinates_of_tree(conn, &tree.tree_id)?
                .into_iter()
                .filter(|row| !keep_root || row.edge != EdgeKind::Root)
                .map(|row| row.coord_id),
        );
        if !keep_root {
            self.trees.insert(tree.tree_id.clone(), tree);
        }
        Ok(())
    }
}

fn classify_delete(
    conn: &Connection,
    delete_set: &BTreeSet<String>,
    keep_root: Option<&str>,
    delete_root: Option<&str>,
) -> Result<DeleteCoordinates> {
    let mut result = DeleteCoordinates::default();
    if !table_exists(conn, "coordinates")? {
        return Ok(result);
    }
    let mut deleted_cards = BTreeSet::new();
    let mut query = conn.prepare_cached(
        "SELECT i.card_id, i.filing_coord_id FROM card_identity i JOIN cards c ON c.row_id = i.card_row
         WHERE c.location = ?1 OR c.citation_key = ?1",
    )?;
    for address in delete_set {
        let Some((id, filing_id)) = query
            .query_row([address], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .optional()?
        else {
            continue;
        };
        deleted_cards.insert(id.clone());
        if let Some(tree) = tree_by_public_key(conn, crate::tree_id(address))?
            && tree.root_card_id == id
        {
            result.include_tree(conn, tree, false, delete_set)?;
        }
        let remove_filing = TreeRole::of_address(address) != TreeRole::Connection;
        if remove_filing {
            result.non_connection_filings.insert(filing_id.clone());
        }
        for row in coordinates_of_card(conn, &id)? {
            if remove_filing && row.coord_id == filing_id {
                result.removed.insert(row.coord_id);
            } else {
                result.missing.insert(row.coord_id);
            }
        }
    }
    if let Some(root) = keep_root
        && let Some(tree) = tree_by_public_key(conn, crate::tree_id(root))?
    {
        result.include_tree(conn, tree, true, delete_set)?;
    }
    if let Some(root) = delete_root {
        let tree = tree_by_public_key(conn, crate::tree_id(root))?
            .with_context(|| format!("Tree rooted at `{root}` is missing"))?;
        if !result.trees.contains_key(&tree.tree_id) {
            result.include_tree(conn, tree, false, delete_set)?;
        }
    }
    let mut children = conn.prepare_cached(
        "SELECT c.coord_id, i.card_id FROM coordinates c
         LEFT JOIN card_identity i ON i.card_id = c.card_id AND i.filing_coord_id = c.coord_id
         WHERE c.tree_id = ?1 AND c.parent_coord_id = ?2",
    )?;
    let mut pending: Vec<_> = result.removed.iter().cloned().collect();
    while let Some(id) = pending.pop() {
        let row = coordinate(conn, &id)?.context("Coordinate scheduled for deletion is missing")?;
        for child in children.query_map(params![row.tree_id, id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })? {
            let (child, filing_card) = child?;
            // A surviving Filing Coordinate (e.g. the successor after a deleted span) is
            // relinked by the delete plan's filing mapping in the same transaction.
            if filing_card.is_none_or(|card| deleted_cards.contains(&card))
                && result.removed.insert(child.clone())
            {
                pending.push(child);
            }
        }
    }
    result.missing.retain(|id| !result.removed.contains(id));
    Ok(result)
}

pub(crate) fn remove_coordinates_tx(
    conn: &Connection,
    ids: &[String],
    revision: i64,
) -> Result<()> {
    let mut rows = BTreeMap::new();
    let mut children = BTreeMap::<String, Vec<String>>::new();
    for id in ids {
        if let Some(row) = coordinate(conn, id)? {
            if let Some(parent) = &row.parent_coord_id {
                children.entry(parent.clone()).or_default().push(id.clone());
            }
            rows.insert(id.clone(), row);
        }
    }
    let mut stack: Vec<_> = rows
        .values()
        .filter(|row| {
            row.parent_coord_id
                .as_ref()
                .is_none_or(|parent| !rows.contains_key(parent))
        })
        .map(|row| (row.coord_id.clone(), false))
        .collect();
    let mut order = Vec::with_capacity(rows.len());
    while let Some((id, visited)) = stack.pop() {
        if visited {
            order.push(id);
            continue;
        }
        if let Some(kids) = children.get(&id) {
            stack.push((id, true));
            stack.extend(kids.iter().map(|kid| (kid.clone(), false)));
        } else {
            order.push(id);
        }
    }
    if order.len() != rows.len() {
        bail!("cannot remove Coordinates in a parent cycle");
    }
    let mut remove_edges = conn.prepare_cached(
        "DELETE FROM coordinate_edges WHERE tree_id = ?2 AND (from_coord_id = ?1 OR to_coord_id = ?1)",
    )?;
    let mut retire = conn.prepare_cached(
        "INSERT INTO retired_coordinates(coord_id, tree_id, card_id, retired_at_rev)
         VALUES(?1, ?2, ?3, ?4)",
    )?;
    let mut remove = conn.prepare_cached("DELETE FROM coordinates WHERE coord_id = ?1")?;
    for id in order {
        let row = &rows[&id];
        remove_edges.execute(params![id, row.tree_id])?;
        retire.execute(params![id, row.tree_id, row.card_id, revision])?;
        remove.execute([id])?;
    }
    Ok(())
}

pub(crate) fn delete_hosts(
    conn: &Connection,
    delete_set: &BTreeSet<String>,
    keep_root: Option<&str>,
) -> Result<()> {
    apply_host_delete(conn, classify_delete(conn, delete_set, keep_root, None)?)
}

pub(crate) fn delete_tree_hosts(
    conn: &Connection,
    delete_set: &BTreeSet<String>,
    root: &str,
) -> Result<()> {
    apply_host_delete(conn, classify_delete(conn, delete_set, None, Some(root))?)
}

fn apply_host_delete(conn: &Connection, plan: DeleteCoordinates) -> Result<()> {
    if plan.removed.is_empty() && plan.trees.is_empty() {
        return Ok(());
    }
    for tree in plan.trees.values() {
        crate::coord_lock::ensure_host_unlocked(conn, &tree.tree_id, &tree.public_key)?;
    }
    for (card_id, coord_id, _) in &plan.reanchor {
        conn.execute(
            "UPDATE card_identity SET filing_coord_id = ?2 WHERE card_id = ?1",
            params![card_id, coord_id],
        )?;
    }
    remove_coordinates_tx(
        conn,
        &plan.removed.into_iter().collect::<Vec<_>>(),
        next_revision(conn)?,
    )?;
    for tree in plan.trees.keys() {
        conn.execute("DELETE FROM trees WHERE tree_id = ?1", [tree])?;
    }
    Ok(())
}

pub(crate) fn delete_all_hosts(conn: &Connection) -> Result<()> {
    if !table_exists(conn, "coordinates")? {
        return Ok(());
    }
    let revision = next_revision(conn)?;
    conn.execute("DELETE FROM coordinate_edges", [])?;
    conn.execute(
        "INSERT INTO retired_coordinates(coord_id, tree_id, card_id, retired_at_rev)
         SELECT coord_id, tree_id, card_id, ?1 FROM coordinates",
        [revision],
    )?;
    conn.execute("DELETE FROM coordinates", [])?;
    conn.execute("DELETE FROM trees", [])?;
    Ok(())
}

pub(crate) fn delete_state(
    conn: &Connection,
    delete_set: &BTreeSet<String>,
) -> Result<Vec<String>> {
    let mut state = Vec::new();
    for address in delete_set {
        let card = crate::load_card(conn, address)?;
        state.push(format!("card {address} {}", card.text));
    }
    let plan = classify_delete(conn, delete_set, None, None)?;
    for (card_id, coord_id, address) in &plan.reanchor {
        state.push(format!("reanchor {address} {card_id} {coord_id}"));
    }
    let mut edge_ids = BTreeSet::new();
    for id in plan.removed.iter().chain(&plan.missing) {
        let row = coordinate(conn, id)?.context("deleted plan Coordinate is missing")?;
        state.push(format!("coordinate {row:?}"));
        let mut query = conn.prepare_cached(
            "SELECT connection_id FROM coordinate_edges
             WHERE tree_id = ?2 AND (from_coord_id = ?1 OR to_coord_id = ?1)",
        )?;
        for edge in query.query_map(params![id, row.tree_id], |r| r.get::<_, String>(0))? {
            edge_ids.insert(edge?);
        }
    }
    if !edge_ids.is_empty() {
        let mut lookup =
            conn.prepare_cached("SELECT * FROM coordinate_edges WHERE connection_id = ?1")?;
        let columns = lookup.column_count();
        for edge in edge_ids {
            let values = lookup.query_row([edge], |r| {
                (0..columns)
                    .map(|i| r.get::<_, rusqlite::types::Value>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })?;
            state.push(format!("connection {values:?}"));
        }
    }
    state.sort();
    Ok(state)
}

pub(crate) fn keep_root_has_host_work(conn: &Connection, root: &str) -> Result<bool> {
    let Some(tree) = tree_by_public_key(conn, crate::tree_id(root))? else {
        return Ok(false);
    };
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM coordinates WHERE tree_id = ?1 AND edge <> 'root')",
        [&tree.tree_id],
        |r| r.get(0),
    )?)
}

pub(crate) fn delete_impact_lines(
    conn: &Connection,
    delete_set: &BTreeSet<String>,
    keep_root: Option<&str>,
) -> Result<Vec<String>> {
    delete_impact_lines_in(conn, delete_set, keep_root, None)
}

pub(crate) fn delete_tree_impact_lines(
    conn: &Connection,
    delete_set: &BTreeSet<String>,
    root: &str,
) -> Result<Vec<String>> {
    delete_impact_lines_in(conn, delete_set, None, Some(root))
}

fn delete_impact_lines_in(
    conn: &Connection,
    delete_set: &BTreeSet<String>,
    keep_root: Option<&str>,
    delete_root: Option<&str>,
) -> Result<Vec<String>> {
    let plan = classify_delete(conn, delete_set, keep_root, delete_root)?;
    let mut lines = Vec::new();
    if let Some(root) = keep_root
        && let Some(tree) = tree_by_public_key(conn, crate::tree_id(root))?
        && tree.role == TreeRole::Connection
    {
        let (stations, edges): (i64, i64) = conn.query_row(
            "SELECT (SELECT count(*) FROM coordinates WHERE tree_id = ?1 AND edge <> 'root'),
                    (SELECT count(*) FROM coordinate_edges WHERE tree_id = ?1)",
            [&tree.tree_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        lines.push(format!(
            "{root} keeps its root station; its other {stations} station(s) and {edges} tree connection(s) are removed"
        ));
    }
    for tree in plan.trees.values() {
        if tree.role == TreeRole::Connection {
            let n: i64 = conn.query_row(
                "SELECT count(*) FROM coordinates WHERE tree_id = ?1",
                [&tree.tree_id],
                |r| r.get(0),
            )?;
            lines.push(format!(
                "Connection tree {} goes with its {n} station(s) and their connections; its handles stay issued",
                tree.handle(),
            ));
        } else if conn.query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM coordinates c JOIN card_identity i ON i.card_id = c.card_id
                 WHERE c.tree_id = ?1 AND c.coord_id IS NOT i.filing_coord_id
             ) OR EXISTS (SELECT 1 FROM coordinate_edges WHERE tree_id = ?1)",
            [&tree.tree_id],
            |r| r.get::<_, bool>(0),
        )? {
            lines.push(format!(
                "Tree {} and its Coordinates and Tree connections will be removed",
                tree.public_key,
            ));
        }
    }
    let mut edges = BTreeSet::new();
    if !plan.removed.is_empty() {
        let mut incident = conn.prepare_cached(
            "SELECT e.connection_id FROM coordinates c JOIN coordinate_edges e
             ON e.tree_id = c.tree_id AND e.from_coord_id = c.coord_id WHERE c.coord_id = ?1
             UNION ALL
             SELECT e.connection_id FROM coordinates c JOIN coordinate_edges e
             ON e.tree_id = c.tree_id AND e.to_coord_id = c.coord_id WHERE c.coord_id = ?1",
        )?;
        for id in &plan.removed {
            for edge in incident.query_map([id], |r| r.get::<_, String>(0))? {
                edges.insert(edge?);
            }
        }
    }
    let extra_removed = plan
        .removed
        .difference(&plan.non_connection_filings)
        .count();
    let incident_edges = edges.len();
    if extra_removed > 0 || incident_edges > 0 {
        lines.push(format!(
            "{} Coordinate(s) and their incident Tree connections will be removed",
            plan.removed.len()
        ));
    }
    for (_, coord_id, address) in &plan.reanchor {
        let coord =
            coordinate(conn, coord_id)?.context("surviving Filing Coordinate is missing")?;
        let tree = tree_by_id(conn, &coord.tree_id)?.context("surviving Tree is missing")?;
        lines.push(format!(
            "keep {address} filed in {} (place:{coord_id})",
            tree.handle()
        ));
    }
    let mut missing = plan.missing;
    if missing.is_empty() {
        return Ok(lines);
    }
    let mut home_station = conn.prepare_cached(
        "SELECT p.coord_id FROM card_identity i
         JOIN coordinates p ON p.coord_id = i.filing_coord_id
         JOIN trees t ON t.tree_id = p.tree_id
         WHERE i.status = 'live' AND i.last_address = ?1 AND t.public_key = ?2",
    )?;
    for address in delete_set {
        let Some(handle) = Handle::parse(address).filter(|handle| handle.local.is_some()) else {
            continue;
        };
        let root = Handle::root(handle.tree);
        if delete_set.contains(&root) || keep_root == Some(root.as_str()) {
            continue;
        }
        let id: Option<String> = home_station
            .query_row(params![address, root], |r| r.get(0))
            .optional()?;
        if id.is_some_and(|id| missing.remove(&id)) {
            lines.push(format!(
                "{address} leaves its home station in ~{} as a Missing mount (unplace or rebind it there)",
                handle.tree,
            ));
        }
    }
    if !missing.is_empty() {
        let mut hosts = BTreeSet::new();
        let mut tree_id =
            conn.prepare_cached("SELECT tree_id FROM coordinates WHERE coord_id = ?1")?;
        for id in &missing {
            hosts.insert(tree_id.query_row([id], |r| r.get::<_, String>(0))?);
        }
        let missing = missing.len();
        lines.push(format!(
            "{missing} station(s) in {} other tree(s) will show as Missing mounts, keeping their glosses, children and connections",
            hosts.len(),
        ));
    }
    Ok(lines)
}

pub(crate) fn missing_mounts(
    conn: &Connection,
    workspace: Option<Workspace>,
) -> Result<Vec<Value>> {
    if !table_exists(conn, "coordinates")? {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(&format!(
        "SELECT {COORDINATE_COLUMNS} FROM coordinates c
         WHERE EXISTS (SELECT 1 FROM card_identity i WHERE i.card_id = c.card_id AND i.status = 'retired')
         AND (?1 IS NULL OR c.host_workspace = ?1) ORDER BY tree_id, coord_id",
    ))?;
    let rows = stmt.query_map([workspace.map(Workspace::id)], coordinate_from_row)?;
    let mut out = Vec::new();
    for row in rows {
        let row = row?;
        let tree =
            tree_by_id(conn, &row.tree_id)?.context("Missing Coordinate's Tree is missing")?;
        let card = identity(conn, &row.card_id)?
            .context("Missing Coordinate's Card identity is absent")?;
        out.push(json!({
            "coord_id": row.coord_id,
            "placement_id": row.coord_id,
            "host": tree.root_address(),
            "host_workspace": row.host_workspace.id(),
            "coord": coordinate_path(conn, &row)?,
            "card_id": row.card_id,
            "last_address": card.last_address,
            "last_title": card.last_title,
            "gloss": row.gloss,
        }));
    }
    Ok(out)
}

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
            .query_row("SELECT value FROM metadata WHERE key = ?1", [key], |r| {
                r.get(0)
            })
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
            lock_evidence.insert(
                table.to_string(),
                json!(table_rows(
                    conn,
                    &format!("SELECT * FROM {table} ORDER BY 1, 2")
                )?),
            );
        }
    }
    Ok(json!({
        "version": DUMP_VERSION,
        "installed": true,
        "archive_id": metadata("archive_id")?,
        "archive_revision": crate::link_index::revision(conn)?,
        "next_connection_id": metadata("next_connection_id")?,
        "next_topic_id": metadata("next_topic_id")?,
        "card_identity": table_rows(conn, "SELECT * FROM card_identity ORDER BY card_id")?,
        "card_handles": table_rows(conn, "SELECT * FROM card_handles ORDER BY handle")?,
        "trees": table_rows(conn, "SELECT * FROM trees ORDER BY role, public_key")?,
        "coordinates": table_rows(conn, "SELECT * FROM coordinates ORDER BY tree_id, coord_id")?,
        "coordinate_edges": table_rows(conn, "SELECT * FROM coordinate_edges ORDER BY tree_id, connection_id")?,
        "retired_coordinates": table_rows(conn, "SELECT * FROM retired_coordinates ORDER BY coord_id")?,
        "lock_evidence": Value::Object(lock_evidence),
    }))
}
