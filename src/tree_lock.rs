//! Tree Lock (S01, #124): freeze one whole Topic or Literature tree.
//!
//! A Lock is one owner promise about one real tree. Until the owner unlocks it, the tree takes no
//! new Cards, none of its Cards changes its authored identity, address, Workspace or kind, and every
//! external Card that the tree's Links name keeps its address and row identity. The owner sets and
//! releases a Lock only through the token-guarded GUI route; there is no Lock command, MCP tool or
//! agent authority.
//!
//! This module owns the four lock tables in the existing archive, the pure plan/integrity work, the
//! application gate that every transaction-level Card writer calls, and the two SQL functions plus
//! six triggers that stop a forgotten write path. It adds no second writer mutex: every mutation here
//! expects its caller's SQLite transaction and, above it, the existing edit lock.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

use crate::Workspace;

/// The one Tree Lock schema generation this binary understands.
pub(crate) const SCHEMA_VERSION: &str = "1";
const SCHEMA_KEY: &str = "tree_lock_schema_version";

/// The four lock tables of schema 1. Their column sets are compared as well, because a rebuild
/// that loses or reorders a column would otherwise let the baseline evidence be misread (R298).
const SCHEMA_TABLES_SQL: &str = "
CREATE TABLE IF NOT EXISTS locks (
    lock_id           INTEGER PRIMARY KEY,
    root_row          INTEGER NOT NULL,
    root_address      TEXT NOT NULL,
    tree              TEXT NOT NULL,
    workspace         TEXT NOT NULL CHECK(workspace IN ('human','agent')),
    note              TEXT CHECK(note IS NULL OR length(note) <= 500),
    locked_at         TEXT NOT NULL,
    range_fingerprint TEXT NOT NULL,
    member_count      INTEGER NOT NULL CHECK(member_count >= 1),
    unlocked_at       TEXT
);
CREATE TABLE IF NOT EXISTS lock_members (
    lock_id     INTEGER NOT NULL REFERENCES locks(lock_id),
    row_id      INTEGER NOT NULL,
    address     TEXT NOT NULL,
    authored_fp TEXT NOT NULL,
    PRIMARY KEY(lock_id, row_id)
);
CREATE TABLE IF NOT EXISTS lock_pins (
    lock_id        INTEGER NOT NULL REFERENCES locks(lock_id),
    source_row     INTEGER NOT NULL,
    target_row     INTEGER NOT NULL,
    target_address TEXT NOT NULL,
    PRIMARY KEY(lock_id, source_row, target_row)
);
CREATE TABLE IF NOT EXISTS lock_events (
    event_id INTEGER PRIMARY KEY,
    lock_id  INTEGER NOT NULL REFERENCES locks(lock_id),
    action   TEXT NOT NULL
        CHECK(action IN ('lock','unlock','note','reseal','integrity-fault')),
    at       TEXT NOT NULL,
    detail   TEXT NOT NULL
);
";

/// The required indexes of schema 1, each with the exact statement that creates it. An integrity
/// scan compares the stored definition, so an index replaced under the same name is a fault, not a
/// silent success (R287, S06-AC03).
const INDEX_DEFS: [(&str, &str); 4] = [
    (
        "locks_active_tree",
        "CREATE UNIQUE INDEX IF NOT EXISTS locks_active_tree
    ON locks(root_address) WHERE unlocked_at IS NULL;",
    ),
    (
        "lock_members_by_row",
        "CREATE INDEX IF NOT EXISTS lock_members_by_row ON lock_members(row_id);",
    ),
    (
        "lock_pins_by_target",
        "CREATE INDEX IF NOT EXISTS lock_pins_by_target ON lock_pins(target_row);",
    ),
    (
        "lock_events_by_lock",
        "CREATE INDEX IF NOT EXISTS lock_events_by_lock ON lock_events(lock_id, event_id);",
    ),
];

/// The six canonical protection triggers of schema 1, each with the exact statement that creates
/// it, in the order the design lists them. The scan compares the stored definition because a
/// trigger replaced under the same name would still pass a name-only check (R026, R287).
const TRIGGER_DEFS: [(&str, &str); 6] = [
    (
        "lock_no_delete",
        "CREATE TRIGGER IF NOT EXISTS lock_no_delete BEFORE DELETE ON cards
WHEN EXISTS (
    SELECT 1 FROM lock_members m JOIN locks l ON l.lock_id = m.lock_id
    WHERE m.row_id = OLD.row_id AND l.unlocked_at IS NULL
)
BEGIN SELECT RAISE(ABORT, 'zt: card is in a locked tree'); END;",
    ),
    (
        "lock_no_change",
        "CREATE TRIGGER IF NOT EXISTS lock_no_change BEFORE UPDATE ON cards
WHEN EXISTS (
    SELECT 1 FROM lock_members m JOIN locks l ON l.lock_id = m.lock_id
    WHERE m.row_id = OLD.row_id AND l.unlocked_at IS NULL
) AND (
       NEW.row_id       IS NOT OLD.row_id
    OR NEW.location     IS NOT OLD.location
    OR NEW.citation_key IS NOT OLD.citation_key
    OR NEW.workspace    IS NOT OLD.workspace
    OR NEW.is_topic     IS NOT OLD.is_topic
    OR NEW.is_lit       IS NOT OLD.is_lit
    OR NEW.bibtex       IS NOT OLD.bibtex
    OR NEW.card_type    IS NOT OLD.card_type
    OR NEW.model_facet  IS NOT OLD.model_facet
    OR NEW.model_emphasis IS NOT OLD.model_emphasis
    OR NEW.model_level  IS NOT OLD.model_level
    OR NEW.hub_level    IS NOT OLD.hub_level
    OR NEW.math_form    IS NOT OLD.math_form
    OR (CASE WHEN instr(NEW.text, '<!-- zt:reverse-links -->') = 0 THEN NEW.text
             ELSE substr(NEW.text, 1, instr(NEW.text, '<!-- zt:reverse-links -->') - 1) END)
       IS NOT
       (CASE WHEN instr(OLD.text, '<!-- zt:reverse-links -->') = 0 THEN OLD.text
             ELSE substr(OLD.text, 1, instr(OLD.text, '<!-- zt:reverse-links -->') - 1) END)
)
BEGIN SELECT RAISE(ABORT, 'zt: card is in a locked tree'); END;",
    ),
    (
        "lock_no_insert",
        "CREATE TRIGGER IF NOT EXISTS lock_no_insert BEFORE INSERT ON cards
WHEN EXISTS (
    SELECT 1 FROM locks l WHERE l.unlocked_at IS NULL
      AND l.tree = CASE WHEN instr(COALESCE(NEW.location, NEW.citation_key), '/') = 0
                        THEN COALESCE(NEW.location, NEW.citation_key)
                        ELSE substr(COALESCE(NEW.location, NEW.citation_key), 1,
                                    instr(COALESCE(NEW.location, NEW.citation_key), '/') - 1)
                   END
)
BEGIN SELECT RAISE(ABORT, 'zt: locked tree takes no new cards'); END;",
    ),
    (
        "lock_no_move_in",
        "CREATE TRIGGER IF NOT EXISTS lock_no_move_in BEFORE UPDATE OF location, citation_key ON cards
WHEN (NEW.location IS NOT OLD.location OR NEW.citation_key IS NOT OLD.citation_key)
 AND EXISTS (
    SELECT 1 FROM locks l WHERE l.unlocked_at IS NULL
      AND l.tree = CASE WHEN instr(COALESCE(NEW.location, NEW.citation_key), '/') = 0
                        THEN COALESCE(NEW.location, NEW.citation_key)
                        ELSE substr(COALESCE(NEW.location, NEW.citation_key), 1,
                                    instr(COALESCE(NEW.location, NEW.citation_key), '/') - 1)
                   END
)
BEGIN SELECT RAISE(ABORT, 'zt: locked tree takes no new cards'); END;",
    ),
    (
        "lock_pin_no_delete",
        "CREATE TRIGGER IF NOT EXISTS lock_pin_no_delete BEFORE DELETE ON cards
WHEN EXISTS (
    SELECT 1 FROM lock_pins p JOIN locks l ON l.lock_id = p.lock_id
    WHERE p.target_row = OLD.row_id AND l.unlocked_at IS NULL
)
BEGIN SELECT RAISE(ABORT, 'zt: card is a pinned target of a locked tree'); END;",
    ),
    (
        "lock_pin_no_move",
        "CREATE TRIGGER IF NOT EXISTS lock_pin_no_move BEFORE UPDATE ON cards
WHEN (NEW.location IS NOT OLD.location OR NEW.citation_key IS NOT OLD.citation_key
      OR NEW.row_id IS NOT OLD.row_id OR NEW.workspace IS NOT OLD.workspace
      OR NEW.is_topic IS NOT OLD.is_topic OR NEW.is_lit IS NOT OLD.is_lit)
 AND EXISTS (
    SELECT 1 FROM lock_pins p JOIN locks l ON l.lock_id = p.lock_id
    WHERE p.target_row = OLD.row_id AND l.unlocked_at IS NULL
)
BEGIN SELECT RAISE(ABORT, 'zt: card is a pinned target of a locked tree'); END;",
    ),
];

// Every trigger joins `locks` with `unlocked_at IS NULL`. Insertion and move-in compare the
// destination's Tree id with the active `locks.tree`, which is the same canonical value as
// `tree_root_address(tree_id(address))` without a function call. Update protection compares the
// authored bytes before the generated Reverse marker; a Reverse-only refresh keeps them identical.
// The predicates use only built-in SQL so that an external connection can still write an archive
// that holds no Lock (T11 keeps the function-less connection as an unsupported writer of a locked
// body, not of every Card).

/// The canonical trigger names, in the order the design lists them.
pub(crate) const TRIGGER_NAMES: [&str; 6] = [
    "lock_no_delete",
    "lock_no_change",
    "lock_no_insert",
    "lock_no_move_in",
    "lock_pin_no_delete",
    "lock_pin_no_move",
];

fn schema_sql() -> String {
    SCHEMA_TABLES_SQL.to_string()
}

// ---------------------------------------------------------------------------------------------
// SQL backstop
// ---------------------------------------------------------------------------------------------

/// Register the two deterministic SQL wrappers on a connection. This installs no schema and runs no
/// migration, so opening a connection never recurses into the writer lock.
pub(crate) fn register_sql_functions(conn: &Connection) -> Result<()> {
    use rusqlite::functions::FunctionFlags;
    let flags = FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_UTF8;
    conn.create_scalar_function("zt_authored_fp", 8, flags, |ctx| {
        let text: String = ctx.get(0)?;
        let bibtex: Option<String> = ctx.get(1)?;
        let card_type: String = ctx.get(2)?;
        let facet: Option<String> = ctx.get(3)?;
        let emphasis: Option<String> = ctx.get(4)?;
        let model_level: Option<String> = ctx.get(5)?;
        let hub_level: Option<String> = ctx.get(6)?;
        let form: Option<String> = ctx.get(7)?;
        fingerprint_of_raw(
            &text,
            bibtex.as_deref(),
            &card_type,
            facet.as_deref(),
            emphasis.as_deref(),
            model_level.as_deref(),
            hub_level.as_deref(),
            form.as_deref(),
        )
        .map_err(|error| rusqlite::Error::UserFunctionError(Box::from(format!("{error:#}"))))
    })
    .context("failed to register zt_authored_fp")?;
    conn.create_scalar_function("zt_tree_root", 1, flags, |ctx| {
        let address: String = ctx.get(0)?;
        Ok::<Option<String>, rusqlite::Error>(final_tree_root(&address))
    })
    .context("failed to register zt_tree_root")?;
    Ok(())
}

/// The authored fingerprint of one stored Card row, with every raw metadata value validated instead
/// of falling back to a default. A parse or hash failure aborts the statement.
#[allow(clippy::too_many_arguments)]
fn fingerprint_of_raw(
    text: &str,
    bibtex: Option<&str>,
    card_type: &str,
    facet: Option<&str>,
    emphasis: Option<&str>,
    model_level: Option<&str>,
    hub_level: Option<&str>,
    form: Option<&str>,
) -> Result<String> {
    let parsed = crate::parse_card_text(text)?;
    let card_type = crate::CardType::parse(card_type)?;
    let facet = facet.map(crate::model::Facet::parse).transpose()?;
    let emphasis = emphasis.map(crate::model::Emphasis::parse).transpose()?;
    let level = model_level.map(crate::model::Level::parse).transpose()?;
    let hub_level = hub_level.map(crate::hub::HubLevel::parse).transpose()?;
    let form = form.map(crate::math::MathForm::parse).transpose()?;
    let level_id = level
        .as_ref()
        .map(|level| level.id().to_string())
        .or_else(|| hub_level.as_ref().map(|level| level.id().to_string()))
        .unwrap_or_default();
    Ok(crate::card::authored_fingerprint_framed(
        &parsed.title,
        &parsed.body,
        bibtex.unwrap_or_default(),
        card_type.id(),
        facet.as_ref().map_or("", |facet| facet.id()),
        emphasis.map_or("", |emphasis| emphasis.id()),
        &level_id,
        form.map_or("", |form| form.id()),
    ))
}

/// The final Tree root address of a Card address, or `None` for the internal `__moving__`
/// placeholders and for text that is not a Card address at all.
pub(crate) fn final_tree_root(address: &str) -> Option<String> {
    if address.starts_with("__moving__") || address.starts_with("__deleted__") {
        return None;
    }
    let tree = crate::tree_id(address);
    let is_literal_address =
        address == tree || address.starts_with(&format!("{tree}/"));
    if !(crate::is_valid_location(address)
        || (is_literal_address && crate::is_literature_tree_id(tree)))
    {
        return None;
    }
    Some(crate::tree_root_address(tree))
}

// ---------------------------------------------------------------------------------------------
// Schema installation
// ---------------------------------------------------------------------------------------------

fn schema_version(conn: &Connection) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM metadata WHERE key = ?1",
            [SCHEMA_KEY],
            |row| row.get(0),
        )
        .optional()?)
}

/// Install or validate schema version 1 and its protection. Called once at service startup, after
/// the Card-table rebuilds, so it never fights a migration. An unsupported generation refuses to
/// write rather than attempting a repair; a supported generation has a missing or altered trigger,
/// index or table re-created here, and the caller has already recorded that fault. Answers the
/// canonical objects this call had to reinstall, in catalog order.
fn install_or_repair(conn: &Connection) -> Result<Vec<String>> {
    let mut repaired = Vec::new();
    match schema_version(conn)? {
        None => {
            conn.execute_batch(&schema_sql())
                .context("failed to install the Tree Lock tables")?;
            validate_schema(conn)?;
            repaired.extend(reinstall_indexes(conn)?);
            repaired.extend(reinstall_triggers(conn)?);
            validate_indexes(conn)?;
            validate_triggers(conn)?;
            conn.execute(
                "INSERT INTO metadata(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![SCHEMA_KEY, SCHEMA_VERSION],
            )?;
        }
        Some(version) if version == SCHEMA_VERSION => {
            // A damaged column set is reported, never repaired: fabricating a snapshot or a new
            // seal would replace the owner's evidence with a lie (R298).
            validate_schema(conn)?;
            conn.execute_batch(&schema_sql())
                .context("failed to restore the Tree Lock tables")?;
            repaired.extend(reinstall_indexes(conn)?);
            repaired.extend(reinstall_triggers(conn)?);
            validate_indexes(conn)?;
            validate_triggers(conn)?;
        }
        Some(other) => bail!(
            "unsupported Tree Lock schema version `{other}`; archive was not modified"
        ),
    }
    Ok(repaired)
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(columns)
}

/// The column set of each lock table, in the order schema 1 stores it.
fn expected_columns(table: &str) -> Option<&'static [&'static str]> {
    match table {
        "locks" => Some(&[
            "lock_id",
            "root_row",
            "root_address",
            "tree",
            "workspace",
            "note",
            "locked_at",
            "range_fingerprint",
            "member_count",
            "unlocked_at",
        ]),
        "lock_members" => Some(&["lock_id", "row_id", "address", "authored_fp"]),
        "lock_pins" => Some(&["lock_id", "source_row", "target_row", "target_address"]),
        "lock_events" => Some(&["event_id", "lock_id", "action", "at", "detail"]),
        _ => None,
    }
}

const LOCK_TABLES: [&str; 4] = ["locks", "lock_members", "lock_pins", "lock_events"];

fn validate_schema(conn: &Connection) -> Result<()> {
    for table in LOCK_TABLES {
        let columns = expected_columns(table).expect("every lock table has an expected column set");
        let found = table_columns(conn, table)
            .with_context(|| format!("Tree Lock table `{table}` is unreadable"))?;
        if found != columns {
            bail!(
                "Tree Lock table `{table}` has an unsupported column set; archive was not modified"
            );
        }
    }
    Ok(())
}

/// The stored definition of one schema object, `None` when it does not exist.
fn stored_sql(conn: &Connection, kind: &str, name: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = ?1 AND name = ?2",
            params![kind, name],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten())
}

/// Two schema definitions are the same promise: the same tokens in the same order, so the stored
/// text may differ in spacing, line breaks, letter case, an `IF NOT EXISTS` prefix or a trailing
/// semicolon. A changed predicate, table, column or message is a different definition.
fn same_definition(stored: &str, canonical: &str) -> bool {
    fn normalized(sql: &str) -> String {
        sql.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
            .trim_end_matches(';')
            .trim()
            .to_string()
            .replace("if not exists ", "")
    }
    normalized(stored) == normalized(canonical)
}

fn trigger_exists(conn: &Connection, name: &str) -> Result<bool> {
    Ok(stored_sql(conn, "trigger", name)?.is_some())
}

fn validate_triggers(conn: &Connection) -> Result<()> {
    for name in TRIGGER_NAMES {
        if !trigger_exists(conn, name)? {
            bail!("Tree Lock trigger `{name}` is missing; archive was not modified");
        }
    }
    for (name, sql) in TRIGGER_DEFS {
        let stored = stored_sql(conn, "trigger", name)?
            .with_context(|| format!("Tree Lock trigger `{name}` is missing"))?;
        if !same_definition(&stored, sql) {
            bail!(
                "Tree Lock trigger `{name}` has an altered definition; archive was not modified"
            );
        }
    }
    Ok(())
}

fn validate_indexes(conn: &Connection) -> Result<()> {
    for (name, _) in INDEX_DEFS {
        if stored_sql(conn, "index", name)?.is_none() {
            bail!("Tree Lock index `{name}` is missing; archive was not modified");
        }
    }
    for (name, sql) in INDEX_DEFS {
        let stored = stored_sql(conn, "index", name)?
            .with_context(|| format!("Tree Lock index `{name}` is missing"))?;
        if !same_definition(&stored, sql) {
            bail!("Tree Lock index `{name}` has an altered definition; archive was not modified");
        }
    }
    Ok(())
}

/// Re-create every missing or altered canonical trigger, in catalog order, and answer their names.
fn reinstall_triggers(conn: &Connection) -> Result<Vec<String>> {
    let mut repaired = Vec::new();
    for (name, sql) in TRIGGER_DEFS {
        let canonical = match stored_sql(conn, "trigger", name)? {
            Some(stored) => same_definition(&stored, sql),
            None => false,
        };
        if canonical {
            continue;
        }
        conn.execute_batch(&format!("DROP TRIGGER IF EXISTS {name};"))
            .with_context(|| format!("failed to replace Tree Lock trigger `{name}`"))?;
        conn.execute_batch(sql)
            .with_context(|| format!("failed to install Tree Lock trigger `{name}`"))?;
        repaired.push(name.to_string());
    }
    Ok(repaired)
}

/// Re-create every missing or altered canonical index, in catalog order, and answer their names.
fn reinstall_indexes(conn: &Connection) -> Result<Vec<String>> {
    let mut repaired = Vec::new();
    for (name, sql) in INDEX_DEFS {
        let canonical = match stored_sql(conn, "index", name)? {
            Some(stored) => same_definition(&stored, sql),
            None => false,
        };
        if canonical {
            continue;
        }
        conn.execute_batch(&format!("DROP INDEX IF EXISTS {name};"))
            .with_context(|| format!("failed to replace Tree Lock index `{name}`"))?;
        conn.execute_batch(sql)
            .with_context(|| format!("failed to install Tree Lock index `{name}`"))?;
        repaired.push(name.to_string());
    }
    Ok(repaired)
}

// ---------------------------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------------------------

/// What kind of promise refused a change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RefusalKind {
    /// The Card is a member of an active Locked tree.
    Locked,
    /// The destination belongs to an active Locked tree.
    LockedTree,
    /// The Card is pinned by an active Locked tree.
    Pinned,
    /// The Link index and the authored bodies disagree, so pins cannot be trusted.
    Index,
}

impl RefusalKind {
    pub(crate) fn code(self) -> &'static str {
        match self {
            RefusalKind::Locked => "locked",
            RefusalKind::LockedTree => "locked_tree",
            RefusalKind::Pinned => "pinned",
            RefusalKind::Index => "index_integrity",
        }
    }
}

/// One active Lock that pins a Card, with the locked source Cards holding the Links.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PinSource {
    pub(crate) lock_id: i64,
    pub(crate) tree: String,
    pub(crate) sources: Vec<String>,
}

/// A typed refusal: a Card write the owner's Lock forbids. It carries enough facts for a client to
/// name the Locked tree, the Lock and the pinning sources, and renders the same facts as JSON.
#[derive(Clone, Debug)]
pub(crate) struct Refusal {
    pub(crate) kind: RefusalKind,
    pub(crate) message: String,
    pub(crate) card: Option<String>,
    pub(crate) lock_id: Option<i64>,
    pub(crate) tree: Option<String>,
    pub(crate) pinned_by: Vec<PinSource>,
}

impl Refusal {
    fn locked(card: &str, lock_id: i64, tree: &str) -> Self {
        Self {
            kind: RefusalKind::Locked,
            message: format!(
                "card `{card}` is in locked tree `{tree}` (lock {lock_id}); unlock the tree in the GUI first"
            ),
            card: Some(card.to_string()),
            lock_id: Some(lock_id),
            tree: Some(tree.to_string()),
            pinned_by: Vec::new(),
        }
    }

    fn locked_tree(destination: &str, lock_id: i64, tree: &str) -> Self {
        Self {
            kind: RefusalKind::LockedTree,
            message: format!(
                "`{destination}` is inside locked tree `{tree}` (lock {lock_id}); a locked tree takes no new cards"
            ),
            card: Some(destination.to_string()),
            lock_id: Some(lock_id),
            tree: Some(tree.to_string()),
            pinned_by: Vec::new(),
        }
    }

    fn pinned(card: &str, pins: &[PinSource]) -> Self {
        let trees = pins
            .iter()
            .map(|pin| format!("`{}` (lock {})", pin.tree, pin.lock_id))
            .collect::<Vec<_>>()
            .join(", ");
        let sources = pins
            .iter()
            .flat_map(|pin| pin.sources.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        Self {
            kind: RefusalKind::Pinned,
            message: format!(
                "card `{card}` is a pinned target of locked tree {trees}; locked source cards {sources} link it"
            ),
            card: Some(card.to_string()),
            lock_id: pins.first().map(|pin| pin.lock_id),
            tree: pins.first().map(|pin| pin.tree.clone()),
            pinned_by: pins.to_vec(),
        }
    }

    fn index(card: &str, tree: &str, detail: &str) -> Self {
        Self {
            kind: RefusalKind::Index,
            message: format!(
                "the Link index does not match the authored body of `{card}` in tree `{tree}`: {detail}"
            ),
            card: Some(card.to_string()),
            lock_id: None,
            tree: Some(tree.to_string()),
            pinned_by: Vec::new(),
        }
    }

    pub(crate) fn value(&self) -> Value {
        json!({
            "error": self.message,
            "code": self.kind.code(),
            "card": self.card,
            "lock_id": self.lock_id,
            "locked_tree": self.tree,
            "pinned_by": self
                .pinned_by
                .iter()
                .map(|pin| json!({
                    "tree": pin.tree,
                    "lock_id": pin.lock_id,
                    "sources": pin.sources,
                }))
                .collect::<Vec<_>>(),
        })
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Refusal {}

// ---------------------------------------------------------------------------------------------
// Stored state
// ---------------------------------------------------------------------------------------------

/// One Lock as stored.
#[derive(Clone, Debug)]
pub(crate) struct Lock {
    pub(crate) lock_id: i64,
    pub(crate) root_row: i64,
    pub(crate) root: String,
    pub(crate) tree: String,
    pub(crate) workspace: Workspace,
    pub(crate) note: Option<String>,
    pub(crate) locked_at: String,
    pub(crate) seal: String,
    pub(crate) member_count: i64,
    pub(crate) unlocked_at: Option<String>,
}

impl Lock {
    pub(crate) fn seal_display(&self) -> String {
        self.seal.chars().take(12).collect()
    }

    pub(crate) fn value(&self) -> Value {
        json!({
            "lock_id": self.lock_id,
            "root_row": self.root_row,
            "root": self.root,
            "tree": self.tree,
            "workspace": self.workspace.id(),
            "note": self.note,
            "locked_at": self.locked_at,
            "seal": self.seal,
            "seal_display": self.seal_display(),
            "member_count": self.member_count,
            "unlocked_at": self.unlocked_at,
        })
    }
}

/// The active-Lock facts a writer gate needs, loaded once.
struct State {
    locks: BTreeMap<i64, Lock>,
    member_by_row: BTreeMap<i64, (i64, String)>,
    member_by_address: BTreeMap<String, (i64, String)>,
    /// Each promised row's own address, so a refusal names the Card it refused (R217, S05-AC04).
    member_address_by_row: BTreeMap<i64, String>,
    pins_by_target: BTreeMap<i64, Vec<PinSource>>,
    roots: BTreeMap<String, i64>,
}

fn lock_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Lock> {
    let workspace: String = row.get(4)?;
    Ok(Lock {
        lock_id: row.get(0)?,
        root_row: row.get(1)?,
        root: row.get(2)?,
        tree: row.get(3)?,
        workspace: Workspace::parse(&workspace).unwrap_or(Workspace::Human),
        note: row.get(5)?,
        locked_at: row.get(6)?,
        seal: row.get(7)?,
        member_count: row.get(8)?,
        unlocked_at: row.get(9)?,
    })
}

const LOCK_COLUMNS: &str =
    "lock_id, root_row, root_address, tree, workspace, note, locked_at, range_fingerprint, member_count, unlocked_at";

/// Every Lock, active first by id. `None` means the Tree Lock schema is not installed, so nothing
/// is protected and every writer must refuse until the service installs it.
fn load_state(conn: &Connection) -> Result<Option<State>> {
    if schema_version(conn)? != Some(SCHEMA_VERSION.to_string()) {
        return Ok(None);
    }
    let mut locks = BTreeMap::new();
    let mut stmt = conn.prepare(&format!(
        "SELECT {LOCK_COLUMNS} FROM locks ORDER BY lock_id"
    ))?;
    for lock in stmt.query_map([], lock_from_row)? {
        let lock = lock?;
        locks.insert(lock.lock_id, lock);
    }
    let mut member_by_row = BTreeMap::new();
    let mut member_address_by_row = BTreeMap::new();
    let mut member_by_address = BTreeMap::new();
    let mut stmt = conn.prepare(
        "SELECT m.lock_id, m.row_id, m.address FROM lock_members m
         JOIN locks l ON l.lock_id = m.lock_id
         WHERE l.unlocked_at IS NULL",
    )?;
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })? {
        let (lock_id, row_id, address) = row?;
        let root = locks
            .get(&lock_id)
            .map(|lock| lock.root.clone())
            .unwrap_or_default();
        member_by_row.insert(row_id, (lock_id, root.clone()));
        member_address_by_row.insert(row_id, address.clone());
        member_by_address.insert(address, (lock_id, root));
    }
    let mut pins_by_target: BTreeMap<i64, Vec<PinSource>> = BTreeMap::new();
    let mut stmt = conn.prepare(
        "SELECT p.lock_id, p.target_row, m.address FROM lock_pins p
         JOIN locks l ON l.lock_id = p.lock_id
         LEFT JOIN lock_members m ON m.lock_id = p.lock_id AND m.row_id = p.source_row
         WHERE l.unlocked_at IS NULL
         ORDER BY p.lock_id, p.target_row, m.address",
    )?;
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })? {
        let (lock_id, target_row, source) = row?;
        let root = locks
            .get(&lock_id)
            .map(|lock| lock.root.clone())
            .unwrap_or_default();
        let pins = pins_by_target.entry(target_row).or_default();
        match pins.iter_mut().find(|pin| pin.lock_id == lock_id) {
            Some(pin) => {
                if let Some(source) = source {
                    pin.sources.push(source);
                }
            }
            None => pins.push(PinSource {
                lock_id,
                tree: root,
                sources: source.into_iter().collect(),
            }),
        }
    }
    let mut roots = BTreeMap::new();
    for lock in locks.values().filter(|lock| lock.unlocked_at.is_none()) {
        roots.insert(lock.root.clone(), lock.lock_id);
    }
    Ok(Some(State {
        locks,
        member_by_row,
        member_by_address,
        member_address_by_row,
        pins_by_target,
        roots,
    }))
}

/// The active Locked Tree root addresses, for a whole-archive delete that must refuse before it
/// prompts, even on a damaged archive.
pub(crate) fn active_roots(conn: &Connection) -> Result<Vec<String>> {
    let Some(state) = load_state(conn)? else {
        bail!("the Tree Lock schema is not installed; restart the service before deleting cards");
    };
    let mut roots = state.roots.keys().cloned().collect::<Vec<_>>();
    roots.sort();
    Ok(roots)
}

fn active_lock(conn: &Connection, root: &str) -> Result<Option<Lock>> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {LOCK_COLUMNS} FROM locks WHERE root_address = ?1 AND unlocked_at IS NULL"
            ),
            [root],
            lock_from_row,
        )
        .optional()?)
}

fn load_lock(conn: &Connection, lock_id: i64) -> Result<Option<Lock>> {
    Ok(conn
        .query_row(
            &format!("SELECT {LOCK_COLUMNS} FROM locks WHERE lock_id = ?1"),
            [lock_id],
            lock_from_row,
        )
        .optional()?)
}

// ---------------------------------------------------------------------------------------------
// The read view
// ---------------------------------------------------------------------------------------------

/// One active Lock as a Card read names it: the Lock's identity, its Locked tree's root address and
/// when the owner took it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LockFact {
    pub(crate) lock_id: i64,
    /// The Locked tree's root address. The public annotation calls this field `tree`, and it is the
    /// root address, never the Card `tree` id (R068, S03-AC01).
    pub(crate) tree: String,
    /// The Lock's UTC start, as `locks.locked_at` stores it.
    pub(crate) since: String,
}

/// The active Lock facts one read uses: which Card rows an active Lock promises, and which rows its
/// pins keep addressable. Bulk-loaded once from the member-row and target-row indexes (R206,
/// S03-AC04): a rendered Card costs no query, and no read reparses a locked tree or recomputes a
/// seal. It is not a cache: it lives for one read.
#[derive(Clone, Debug, Default)]
pub(crate) struct LockView {
    locked: BTreeMap<i64, LockFact>,
    pinned: BTreeMap<i64, Vec<String>>,
}

/// Card rows per `IN (...)` list, well below SQLite's bound on statement parameters.
const VIEW_ROWS: usize = 400;

impl LockView {
    /// An isolated pure fixture that has no archive: no Lock can govern it. A database-backed read
    /// must ask [`LockView::read`] instead, so it can never show a real archive as lock-free (R207).
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// The active Lock facts of `rows`, or of every Card row when `rows` is `None`. A read of an
    /// archive whose Tree Lock schema is not installed is legitimately lock-free. The caller reads
    /// its Cards, these facts and the archive revision in one SQLite read transaction.
    pub(crate) fn read(conn: &Connection, rows: Option<&[i64]>) -> Result<Self> {
        if schema_version(conn)? != Some(SCHEMA_VERSION.to_string()) {
            return Ok(Self::empty());
        }
        let mut view = Self::default();
        match rows {
            None => {
                let mut members = conn.prepare(
                    "SELECT m.row_id, l.lock_id, l.root_address, l.locked_at
                       FROM lock_members m JOIN locks l ON l.lock_id = m.lock_id
                      WHERE l.unlocked_at IS NULL",
                )?;
                for row in members.query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })? {
                    let (row_id, lock_id, tree, since) = row?;
                    view.locked.insert(row_id, LockFact { lock_id, tree, since });
                }
                let mut pins = conn.prepare(
                    "SELECT p.target_row, l.root_address
                       FROM lock_pins p JOIN locks l ON l.lock_id = p.lock_id
                      WHERE l.unlocked_at IS NULL",
                )?;
                for row in pins.query_map([], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })? {
                    let (row_id, tree) = row?;
                    view.pin(row_id, tree);
                }
            }
            Some(rows) => {
                for chunk in rows.chunks(VIEW_ROWS) {
                    let mut members = conn.prepare(&format!(
                        "SELECT m.row_id, l.lock_id, l.root_address, l.locked_at
                           FROM lock_members m JOIN locks l ON l.lock_id = m.lock_id
                          WHERE l.unlocked_at IS NULL AND m.row_id IN ({})",
                        in_list(chunk.len())
                    ))?;
                    for row in members.query_map(rusqlite::params_from_iter(chunk), |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })? {
                        let (row_id, lock_id, tree, since) = row?;
                        view.locked.insert(row_id, LockFact { lock_id, tree, since });
                    }
                    let mut pins = conn.prepare(&format!(
                        "SELECT p.target_row, l.root_address
                           FROM lock_pins p JOIN locks l ON l.lock_id = p.lock_id
                          WHERE l.unlocked_at IS NULL AND p.target_row IN ({})",
                        in_list(chunk.len())
                    ))?;
                    for row in
                        pins.query_map(rusqlite::params_from_iter(chunk), |row| {
                            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                        })?
                    {
                        let (row_id, tree) = row?;
                        view.pin(row_id, tree);
                    }
                }
            }
        }
        Ok(view)
    }

    /// One target row pinned by one Locked root. Several members of one Locked tree list that root
    /// once; the roots stay sorted and deduplicated (R202, R203).
    fn pin(&mut self, row_id: i64, tree: String) {
        let roots = self.pinned.entry(row_id).or_default();
        if let Err(at) = roots.binary_search(&tree) {
            roots.insert(at, tree);
        }
    }

    pub(crate) fn lock_of(&self, row_id: i64) -> Option<&LockFact> {
        self.locked.get(&row_id)
    }

    /// Whether an active Lock promises this Card row.
    pub(crate) fn is_locked(&self, row_id: i64) -> bool {
        self.locked.contains_key(&row_id)
    }

    /// The active Locked tree roots whose Cards link this row, sorted and deduplicated.
    pub(crate) fn pinned_by(&self, row_id: i64) -> &[String] {
        self.pinned.get(&row_id).map_or(&[], Vec::as_slice)
    }

    /// Whether `access` may change this Card's authored text (S03-AC03): Workspace permission and
    /// no active Lock. A pin alone leaves editing possible; movement and deletion are constrained
    /// by the writer gate instead.
    pub(crate) fn writable_for(
        &self,
        row_id: i64,
        access: crate::Access,
        workspace: Workspace,
    ) -> bool {
        access.can_write(workspace) && !self.is_locked(row_id)
    }
}

fn in_list(count: usize) -> String {
    std::iter::repeat_n("?", count).collect::<Vec<_>>().join(",")
}

/// Attach the active Lock facts to one Card projection (R201, R205, S03-AC01). `lock` is `null` for
/// an unlocked Card, or `{lock_id, tree, since}` where `tree` is the Locked tree's root address;
/// `pinned_by` is the sorted, deduplicated roots of the active Locks whose Cards link this one, and
/// is `[]` when no Lock pins it. A Card can have both. The existing Card `tree` and `root` fields do
/// not change.
pub(crate) fn decorate_card(value: &mut Value, row_id: i64, view: &LockView) {
    value["lock"] = match view.lock_of(row_id) {
        Some(fact) => json!({
            "lock_id": fact.lock_id,
            "tree": fact.tree,
            "since": fact.since,
        }),
        None => Value::Null,
    };
    value["pinned_by"] = json!(view.pinned_by(row_id));
}

// ---------------------------------------------------------------------------------------------
// Plan
// ---------------------------------------------------------------------------------------------

/// One Card promised by a Lock: its row identity, exact address and authored fingerprint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Member {
    pub(crate) row_id: i64,
    pub(crate) address: String,
    pub(crate) fingerprint: String,
    pub(crate) root: bool,
}

/// One distinct external target an active Lock promises, with every locked source Card naming it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Pin {
    pub(crate) target: String,
    pub(crate) target_row: i64,
    pub(crate) sources: Vec<(i64, String)>,
}

impl Pin {
    fn source_addresses(&self) -> Vec<String> {
        self.sources.iter().map(|(_, source)| source.clone()).collect()
    }
}

/// One authored Link whose target does not resolve. It is a warning, never a pin.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct Warning {
    pub(crate) source: String,
    pub(crate) target: String,
}

/// What the owner reviews in the GUI dialog, and what the confirmation hash binds.
#[derive(Clone, Debug)]
pub(crate) struct LockPlan {
    pub(crate) archive: String,
    pub(crate) revision: u64,
    pub(crate) root: String,
    pub(crate) tree: String,
    pub(crate) zone: &'static str,
    pub(crate) workspace: Workspace,
    pub(crate) title: String,
    pub(crate) note: Option<String>,
    pub(crate) members: Vec<Member>,
    pub(crate) pins: Vec<Pin>,
    pub(crate) warnings: Vec<Warning>,
    pub(crate) seal: String,
    pub(crate) plan_hash: String,
}

impl LockPlan {
    pub(crate) fn root_count(&self) -> usize {
        1
    }

    pub(crate) fn regular_count(&self) -> usize {
        self.members.iter().filter(|member| !member.root).count()
    }

    pub(crate) fn plan_hash_value(&self) -> Value {
        json!({
            "action": "lock",
            "archive": self.archive,
            "revision": self.revision,
            "root": self.root,
            "tree": self.tree,
            "workspace": self.workspace.id(),
            "note": self.note,
            "member_count": self.members.len(),
            "seal": self.seal,
            "members": self
                .members
                .iter()
                .map(|member| json!([member.address, member.row_id, member.fingerprint]))
                .collect::<Vec<_>>(),
            "pins": self
                .pins
                .iter()
                .map(|pin| json!([pin.target, pin.target_row, pin.source_addresses()]))
                .collect::<Vec<_>>(),
            "warnings": self
                .warnings
                .iter()
                .map(|warning| json!([warning.source, warning.target]))
                .collect::<Vec<_>>(),
        })
    }

    pub(crate) fn value(&self) -> Value {
        json!({
            "action": "lock",
            "confirm": "lock",
            "plan_hash": self.plan_hash,
            "archive": self.archive,
            "revision": self.revision,
            "root": self.root,
            "tree": self.tree,
            "zone": self.zone,
            "workspace": self.workspace.id(),
            "title": self.title,
            "note": self.note,
            "root_count": self.root_count(),
            "regular_count": self.regular_count(),
            "member_count": self.members.len(),
            "seal": self.seal,
            "seal_display": self.seal.chars().take(12).collect::<String>(),
            "members": self
                .members
                .iter()
                .map(|member| json!({
                    "address": member.address,
                    "row_id": member.row_id,
                    "root": member.root,
                    "fingerprint": member.fingerprint,
                }))
                .collect::<Vec<_>>(),
            "pin_count": self.pins.len(),
            "pins": self
                .pins
                .iter()
                .map(|pin| json!({
                    "target": pin.target,
                    "target_row": pin.target_row,
                    "sources": pin.source_addresses(),
                }))
                .collect::<Vec<_>>(),
            "warnings": self
                .warnings
                .iter()
                .map(|warning| json!({"source": warning.source, "target": warning.target}))
                .collect::<Vec<_>>(),
        })
    }

    /// The human-readable lines a dialog prints under the counts.
    pub(crate) fn lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!(
                "lock `{}` in the {} workspace",
                self.root,
                self.workspace.label()
            ),
            format!(
                "cards: {} (1 root, {} regular)",
                self.members.len(),
                self.regular_count()
            ),
            format!(
                "seal: {} (full digest in the panel)",
                self.seal.chars().take(12).collect::<String>()
            ),
            format!("pinned targets: {}", self.pins.len()),
        ];
        for pin in &self.pins {
            lines.push(format!(
                "pinned: {} <- {}",
                pin.target,
                pin.source_addresses().join(", ")
            ));
        }
        lines.push(
            "pins keep a target's address and existence only: its meaning, and any external document, artifact or cache availability, stay outside the seal promise"
                .to_string(),
        );
        if !self.warnings.is_empty() {
            lines.push(format!(
                "already broken links: {} (they create no pin; if one resolves later, unlock and lock again to capture it)",
                self.warnings.len()
            ));
            for warning in &self.warnings {
                lines.push(format!("broken: {} -> {}", warning.source, warning.target));
            }
        }
        if let Some(note) = &self.note {
            lines.push(format!("note: {note}"));
        }
        lines
    }
}

fn validate_note(note: Option<&str>) -> Result<Option<String>> {
    let Some(note) = note else {
        return Ok(None);
    };
    if note.contains('\0') {
        bail!("a Lock note cannot contain a NUL character");
    }
    if note.trim().is_empty() {
        return Ok(None);
    }
    if note.chars().count() > 500 {
        bail!("a Lock note is at most 500 characters");
    }
    Ok(Some(note.to_string()))
}

/// The archive's stored identity, used to bind a plan hash to this archive.
pub(crate) fn archive_identity(conn: &Connection) -> Result<String> {
    Ok(conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'archive_id'",
            [],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or_else(|| "unidentified".to_string()))
}

fn seal_of(members: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    for (address, fingerprint) in members {
        hasher.update(address.as_bytes());
        hasher.update(b"\t");
        hasher.update(fingerprint.as_bytes());
        hasher.update(b"\n");
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn hash_json(value: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.to_string().as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Resolve one requested Tree root into everything the confirmation and its dialog need.
pub(crate) fn plan(
    conn: &Connection,
    root: &str,
    note: Option<&str>,
) -> Result<LockPlan> {
    let note = validate_note(note)?;
    let target = crate::load_card(conn, root)?;
    if !crate::is_tree_root(&target) {
        bail!(
            "`{root}` is a Regular Card; only a Topic or Literature Tree root can be locked"
        );
    }
    let tree = crate::tree_id(&target.address).to_string();
    if crate::tree_root_address(&tree) != target.address {
        bail!("`{root}` is not the root address of its tree");
    }
    if let Some(existing) = active_lock(conn, &target.address)? {
        bail!(
            "tree `{}` is already locked (lock {}, seal {})",
            target.address,
            existing.lock_id,
            existing.seal_display()
        );
    }
    let zone = if target.is_lit { "literature" } else { "topic" };
    let cards = crate::load_cards(conn)?;
    let by_address = cards
        .iter()
        .map(|card| (card.address.clone(), card))
        .collect::<BTreeMap<_, _>>();
    let mut members = BTreeMap::new();
    for card in &cards {
        if !crate::is_tree_member(&card.address, &tree) {
            continue;
        }
        if card.workspace != target.workspace {
            bail!(
                "tree `{}` holds card `{}` in the {} workspace; a Lock needs one Workspace",
                target.address,
                card.address,
                card.workspace.label()
            );
        }
        members.insert(card.address.clone(), card);
    }
    if !members.contains_key(&target.address) {
        bail!("tree `{}` does not hold its own root", target.address);
    }
    let mut fingerprints = BTreeMap::new();
    let mut member_list = Vec::new();
    for (address, card) in &members {
        let fingerprint = crate::card::authored_fingerprint(card)?;
        fingerprints.insert(address.clone(), fingerprint.clone());
        member_list.push(Member {
            row_id: card.row_id,
            address: address.clone(),
            fingerprint,
            root: address == &target.address,
        });
    }
    let mut pins: BTreeMap<String, Pin> = BTreeMap::new();
    let mut warnings = BTreeSet::new();
    for card in members.values() {
        let parsed = crate::parse_card_text(&card.text)?;
        let body = crate::link::LinkBody::parse(&parsed.body)?;
        for occurrence in body.occurrences() {
            let Some(target_card) = by_address.get(occurrence.target) else {
                warnings.insert(Warning {
                    source: card.address.clone(),
                    target: occurrence.target.to_string(),
                });
                continue;
            };
            if crate::is_tree_member(&target_card.address, &tree) {
                continue;
            }
            // One pin per distinct `(locked source row, existing target row)` pair: a repeated macro
            // in one body is the same promise, not a second row (R083).
            let pin = pins
                .entry(target_card.address.clone())
                .or_insert_with(|| Pin {
                    target: target_card.address.clone(),
                    target_row: target_card.row_id,
                    sources: Vec::new(),
                });
            if !pin.sources.iter().any(|(row_id, _)| *row_id == card.row_id) {
                pin.sources.push((card.row_id, card.address.clone()));
            }
        }
    }
    let seal = seal_of(&fingerprints);
    let revision = crate::link_index::revision(conn)?;
    let mut plan = LockPlan {
        archive: archive_identity(conn)?,
        revision,
        root: target.address.clone(),
        tree,
        zone,
        workspace: target.workspace,
        title: crate::parse_card_text(&target.text)?.title,
        note,
        members: member_list,
        pins: pins.into_values().collect(),
        warnings: warnings.into_iter().collect(),
        seal,
        plan_hash: String::new(),
    };
    plan.plan_hash = hash_json(&plan.plan_hash_value());
    Ok(plan)
}

/// The plan of the next owner action on one active Lock: release it, or change its note.
pub(crate) fn existing_plan(
    conn: &Connection,
    lock_id: i64,
    action: &str,
    note: Option<&str>,
) -> Result<Value> {
    let lock = load_lock(conn, lock_id)?.with_context(|| format!("lock {lock_id} does not exist"))?;
    if lock.unlocked_at.is_some() {
        bail!("lock {lock_id} has already ended; an ended Lock cannot be changed");
    }
    let note = validate_note(note)?;
    let revision = crate::link_index::revision(conn)?;
    let pin_count: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT target_row) FROM lock_pins WHERE lock_id = ?1",
        [lock_id],
        |row| row.get(0),
    )?;
    let hash = match action {
        "unlock" => hash_json(&json!({
            "action": "unlock",
            "archive": archive_identity(conn)?,
            "revision": revision,
            "lock_id": lock.lock_id,
            "root": lock.root,
            "seal": lock.seal,
            "member_count": lock.member_count,
        })),
        "note" => hash_json(&json!({
            "action": "note",
            "archive": archive_identity(conn)?,
            "revision": revision,
            "lock_id": lock.lock_id,
            "root": lock.root,
            "note": note,
        })),
        other => bail!("unknown Lock action `{other}`"),
    };
    Ok(json!({
        "action": action,
        "confirm": action,
        "plan_hash": hash,
        "archive": archive_identity(conn)?,
        "revision": revision,
        "lock_id": lock.lock_id,
        "root": lock.root,
        "tree": lock.tree,
        "workspace": lock.workspace.id(),
        "note": note,
        "previous_note": lock.note,
        "locked_at": lock.locked_at,
        "seal": lock.seal,
        "seal_display": lock.seal_display(),
        "member_count": lock.member_count,
        "pin_count": pin_count,
    }))
}

// ---------------------------------------------------------------------------------------------
// Owner actions
// ---------------------------------------------------------------------------------------------

/// Insert the Lock, its member and pin snapshots and its first event, and advance the revision
/// once. The caller owns the transaction, the edit lock and the commit.
pub(crate) fn lock_tx(conn: &Connection, plan: &LockPlan) -> Result<Lock> {
    let root = crate::load_card(conn, &plan.root)?;
    if !crate::is_tree_root(&root) || root.address != plan.root {
        bail!("`{}` is no longer a Tree root", plan.root);
    }
    if let Some(existing) = active_lock(conn, &plan.root)? {
        bail!(
            "tree `{}` is already locked (lock {}, seal {})",
            plan.root,
            existing.lock_id,
            existing.seal_display()
        );
    }
    verify_member_indexes(conn, plan)?;
    conn.execute(
        "INSERT INTO locks(root_row, root_address, tree, workspace, note, locked_at,
                           range_fingerprint, member_count)
         VALUES(?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?6, ?7)",
        params![
            plan.members
                .iter()
                .find(|member| member.root)
                .map(|member| member.row_id)
                .unwrap_or(root.row_id),
            plan.root,
            plan.tree,
            plan.workspace.id(),
            plan.note,
            plan.seal,
            plan.members.len() as i64,
        ],
    )?;
    let lock_id = conn.last_insert_rowid();
    for member in &plan.members {
        conn.execute(
            "INSERT INTO lock_members(lock_id, row_id, address, authored_fp) VALUES(?1, ?2, ?3, ?4)",
            params![lock_id, member.row_id, member.address, member.fingerprint],
        )?;
    }
    for pin in &plan.pins {
        for (source_row, _) in &pin.sources {
            conn.execute(
                "INSERT INTO lock_pins(lock_id, source_row, target_row, target_address)
                 VALUES(?1, ?2, ?3, ?4)",
                params![lock_id, source_row, pin.target_row, pin.target],
            )?;
        }
    }
    let detail = json!({
        "root": plan.root,
        "tree": plan.tree,
        "workspace": plan.workspace.id(),
        "note": plan.note,
        "seal": plan.seal,
        "member_count": plan.members.len(),
        "members": plan
            .members
            .iter()
            .map(|member| json!({
                "address": member.address,
                "row_id": member.row_id,
                "fingerprint": member.fingerprint,
            }))
            .collect::<Vec<_>>(),
        "pins": plan
            .pins
            .iter()
            .map(|pin| json!({
                "target": pin.target,
                "target_row": pin.target_row,
                "sources": pin.source_addresses(),
            }))
            .collect::<Vec<_>>(),
        "warnings": plan
            .warnings
            .iter()
            .map(|warning| json!({"source": warning.source, "target": warning.target}))
            .collect::<Vec<_>>(),
    });
    append_event(conn, lock_id, "lock", &detail)?;
    crate::link_index::bump_revision(conn)?;
    load_lock(conn, lock_id)?.context("committed Lock could not be read back")
}

/// Find `address` inside an active Locked tree, by its final Tree root.
fn active_root_of(state: &State, address: &str) -> Option<(i64, String)> {
    let tree = crate::tree_root_address(crate::tree_id(address));
    state
        .roots
        .get(&tree)
        .map(|lock_id| (*lock_id, tree.clone()))
}

fn pins_of(state: &State, row_id: i64) -> Vec<PinSource> {
    state
        .pins_by_target
        .get(&row_id)
        .cloned()
        .unwrap_or_default()
}

/// End exactly that active Lock: remove only its live snapshots, keep its history, and advance the
/// revision once.
pub(crate) fn unlock_tx(conn: &Connection, lock_id: i64) -> Result<Lock> {
    let lock = load_lock(conn, lock_id)?.with_context(|| format!("lock {lock_id} does not exist"))?;
    if lock.unlocked_at.is_some() {
        bail!("lock {lock_id} has already ended");
    }
    let pin_count: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT target_row) FROM lock_pins WHERE lock_id = ?1",
        [lock_id],
        |row| row.get(0),
    )?;
    let changed = conn.execute(
        "UPDATE locks SET unlocked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE lock_id = ?1 AND unlocked_at IS NULL",
        [lock_id],
    )?;
    if changed == 0 {
        bail!("lock {lock_id} is no longer active");
    }
    conn.execute("DELETE FROM lock_members WHERE lock_id = ?1", [lock_id])?;
    conn.execute("DELETE FROM lock_pins WHERE lock_id = ?1", [lock_id])?;
    append_event(
        conn,
        lock_id,
        "unlock",
        &json!({
            "root": lock.root,
            "seal": lock.seal,
            "note": lock.note,
            "member_count": lock.member_count,
            "pin_count": pin_count,
        }),
    )?;
    crate::link_index::bump_revision(conn)?;
    load_lock(conn, lock_id)?.context("unlocked Lock could not be read back")
}

/// Change one active Lock's note. An identical note changes nothing; a changed note adds one event
/// and one revision, and never reseals the tree.
pub(crate) fn note_tx(conn: &Connection, lock_id: i64, note: Option<&str>) -> Result<Lock> {
    let lock = load_lock(conn, lock_id)?.with_context(|| format!("lock {lock_id} does not exist"))?;
    if lock.unlocked_at.is_some() {
        bail!("lock {lock_id} has already ended; an ended Lock note cannot be changed");
    }
    let note = validate_note(note)?;
    if note == lock.note {
        return Ok(lock);
    }
    conn.execute(
        "UPDATE locks SET note = ?1 WHERE lock_id = ?2 AND unlocked_at IS NULL",
        params![note, lock_id],
    )?;
    append_event(
        conn,
        lock_id,
        "note",
        &json!({"root": lock.root, "previous_note": lock.note, "note": note}),
    )?;
    crate::link_index::bump_revision(conn)?;
    load_lock(conn, lock_id)?.context("noted Lock could not be read back")
}

fn append_event(conn: &Connection, lock_id: i64, action: &str, detail: &Value) -> Result<()> {
    conn.execute(
        "INSERT INTO lock_events(lock_id, action, at, detail)
         VALUES(?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?3)",
        params![lock_id, action, detail.to_string()],
    )?;
    Ok(())
}

/// Refuse to lock when the derived Link index and the authored bodies disagree: incomplete pins are
/// never silently accepted and no global repair runs here.
fn verify_member_indexes(conn: &Connection, plan: &LockPlan) -> Result<()> {
    if !crate::link_index::ready(conn) {
        return Ok(());
    }
    for member in &plan.members {
        let card = crate::load_card(conn, &member.address)?;
        let parsed = crate::parse_card_text(&card.text)?;
        let authored = crate::link::LinkBody::parse(&parsed.body)?
            .occurrences()
            .iter()
            .map(|occurrence| occurrence.target.to_string())
            .collect::<Vec<_>>();
        let mut stmt =
            conn.prepare("SELECT target FROM links WHERE source = ?1 ORDER BY ord")?;
        let indexed = stmt
            .query_map([&member.address], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if authored != indexed {
            return Err(anyhow::Error::new(Refusal::index(
                &member.address,
                &plan.root,
                "run `zt reindex` before locking",
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// The application gate
// ---------------------------------------------------------------------------------------------

/// One planned Card effect, borrowed from the operation that already computed it.
#[derive(Clone, Copy)]
pub(crate) enum Change<'a> {
    /// Edit authored text or metadata of the Card at this row identity.
    Edit(i64),
    /// Create a Card at this final address.
    CreateAt(&'a str),
    /// Apply this complete delete plan.
    Delete(&'a crate::DeletePlan),
    /// Delete every Card in the archive.
    DeleteAll,
    /// Move or rename every old address to its final new address.
    Mapping(&'a BTreeMap<String, String>),
    /// Rewrite Link macros only, with no address change.
    LinkRewrites(&'a BTreeMap<String, String>),
}

/// Return a typed [`Refusal`] when an active Lock forbids this Card effect. Every transaction-level
/// writer calls this before its SQL; the triggers remain the backstop for a forgotten path.
pub(crate) fn ensure_unlocked(conn: &Connection, change: Change<'_>) -> Result<()> {
    let Some(state) = load_state(conn)? else {
        bail!("the Tree Lock schema is not installed; restart the service before writing cards");
    };
    check_change(conn, &state, change)
}

fn check_change(conn: &Connection, state: &State, change: Change<'_>) -> Result<()> {
    match change {
        Change::Edit(row_id) => {
            if let Some((lock_id, root)) = state.member_by_row.get(&row_id) {
                // The refused Card's own address, never another member's: the owner's refusal names
                // the exact Card whose write the Lock stopped (R217, S05-AC04).
                let address = state
                    .member_address_by_row
                    .get(&row_id)
                    .cloned()
                    .unwrap_or_default();
                return Err(anyhow::Error::new(Refusal::locked(
                    &address, *lock_id, root,
                )));
            }
        }
        Change::CreateAt(address) => {
            if let Some((lock_id, root)) = active_root_of(state, address) {
                return Err(anyhow::Error::new(Refusal::locked_tree(
                    address, lock_id, &root,
                )));
            }
        }
        Change::Delete(plan) => {
            let cards = crate::load_cards(conn)?;
            let by_address = cards
                .iter()
                .map(|card| (card.address.as_str(), card))
                .collect::<BTreeMap<_, _>>();
            for address in &plan.delete_set {
                let Some(card) = by_address.get(address.as_str()) else {
                    continue;
                };
                if let Some((lock_id, root)) = state.member_by_row.get(&card.row_id) {
                    return Err(anyhow::Error::new(Refusal::locked(
                        &card.address,
                        *lock_id,
                        root,
                    )));
                }
                let pins = pins_of(state, card.row_id);
                if !pins.is_empty() {
                    return Err(anyhow::Error::new(Refusal::pinned(&card.address, &pins)));
                }
            }
            check_mapping(conn, state, &plan.compaction())?;
            let repoint = plan.repoint_mapping();
            check_change(conn, state, Change::LinkRewrites(&repoint))?;
        }
        Change::Mapping(mapping) => check_mapping(conn, state, mapping)?,
        Change::LinkRewrites(mapping) => check_link_rewrites(conn, state, mapping)?,
        Change::DeleteAll => {
            if let Some((root, lock_id)) = state.roots.iter().next() {
                return Err(anyhow::Error::new(Refusal {
                    kind: RefusalKind::Locked,
                    message: format!(
                        "the archive holds an active Lock on tree `{root}` (lock {lock_id}); unlocking every tree is required before deleting all cards"
                    ),
                    card: None,
                    lock_id: Some(*lock_id),
                    tree: Some(root.clone()),
                    pinned_by: Vec::new(),
                }));
            }
        }
    }
    Ok(())
}

fn check_mapping(
    conn: &Connection,
    state: &State,
    mapping: &BTreeMap<String, String>,
) -> Result<()> {
    if mapping.is_empty() {
        return Ok(());
    }
    for (old, new) in mapping {
        if let Some((lock_id, root)) = state.member_by_address.get(old) {
            return Err(anyhow::Error::new(Refusal::locked(old, *lock_id, root)));
        }
        let cards = crate::load_cards(conn)?;
        if let Some(card) = cards.iter().find(|card| &card.address == old) {
            let pins = pins_of(state, card.row_id);
            if !pins.is_empty() {
                return Err(anyhow::Error::new(Refusal::pinned(old, &pins)));
            }
        }
        if let Some((lock_id, root)) = active_root_of(state, new) {
            return Err(anyhow::Error::new(Refusal::locked_tree(
                new, lock_id, &root,
            )));
        }
    }
    check_change(conn, state, Change::LinkRewrites(mapping))
}

/// A rewritten macro in a locked source would change that source's authored body, so it is refused
/// even when the source itself does not move.
fn check_link_rewrites(
    conn: &Connection,
    state: &State,
    mapping: &BTreeMap<String, String>,
) -> Result<()> {
    if mapping.is_empty() {
        return Ok(());
    }
    let cards = crate::load_cards(conn)?;
    for card in &cards {
        let Some((lock_id, root)) = state.member_by_row.get(&card.row_id) else {
            continue;
        };
        let parsed = crate::parse_card_text(&card.text)?;
        let body = crate::link::LinkBody::parse(&parsed.body)?;
        if body
            .occurrences()
            .iter()
            .any(|occurrence| mapping.contains_key(occurrence.target))
        {
            return Err(anyhow::Error::new(Refusal::locked(
                &card.address,
                *lock_id,
                root,
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Integrity (S06, #129)
// ---------------------------------------------------------------------------------------------

/// One observed integrity fault with a stable identifier, so a repeated scan adds no event (R289).
/// The identifier is the Lock id (or the schema scope), the fault kind and the affected row or
/// address; the event records the expected and the observed fact that explains it (R103).
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct Fault {
    pub(crate) lock_id: Option<i64>,
    pub(crate) kind: String,
    /// The affected row, address or schema object name.
    pub(crate) subject: String,
    pub(crate) detail: String,
    pub(crate) expected: Option<String>,
    pub(crate) actual: Option<String>,
}

impl Fault {
    fn schema(kind: &str, subject: &str, detail: impl Into<String>) -> Self {
        Self {
            lock_id: None,
            kind: kind.to_string(),
            subject: subject.to_string(),
            detail: detail.into(),
            expected: None,
            actual: None,
        }
    }

    fn lock(lock_id: i64, kind: &str, subject: &str, detail: impl Into<String>) -> Self {
        Self {
            lock_id: Some(lock_id),
            ..Self::schema(kind, subject, detail)
        }
    }

    fn expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    fn actual(mut self, actual: impl Into<String>) -> Self {
        self.actual = Some(actual.into());
        self
    }

    /// This fault's stored identifier, which is what records are deduplicated by.
    pub(crate) fn id(&self) -> String {
        match self.lock_id {
            Some(lock_id) => self.id_for_lock(lock_id),
            None => format!("lock-schema:{}:{}", self.kind, self.subject),
        }
    }

    /// The same fault attributed to one active Lock, so a schema fault is latched on every Lock it
    /// leaves unprotected.
    pub(crate) fn id_for_lock(&self, lock_id: i64) -> String {
        format!("lock-{lock_id}:{}:{}", self.kind, self.subject)
    }

    pub(crate) fn value(&self) -> Value {
        json!({
            "id": self.id(),
            "lock_id": self.lock_id,
            "kind": self.kind,
            "card": self.subject,
            "detail": self.detail,
            "expected": self.expected,
            "actual": self.actual,
        })
    }
}

/// One recorded fault event of an active Lock: evidence that stays true about the promise even
/// after a later comparison succeeds (R289, S06-AC05).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Latched {
    pub(crate) lock_id: i64,
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) card: Option<String>,
    pub(crate) at: String,
    pub(crate) detail: String,
}

impl Latched {
    pub(crate) fn value(&self) -> Value {
        json!({
            "lock_id": self.lock_id,
            "id": self.id,
            "kind": self.kind,
            "card": self.card,
            "at": self.at,
            "detail": self.detail,
        })
    }
}

/// The result of one integrity comparison: the current faults and the fault evidence latched on
/// still-active Locks. It repairs nothing and never reseals (R300).
#[derive(Clone, Debug, Default)]
pub(crate) struct Integrity {
    pub(crate) faults: Vec<Fault>,
    pub(crate) latched: Vec<Latched>,
}

impl Integrity {
    /// Healthy requires both no current fault and no latched fault on an active Lock (R289).
    pub(crate) fn ok(&self) -> bool {
        self.faults.is_empty() && self.latched.is_empty()
    }

    /// Whether this comparison observed a fault that may not be recorded yet. A latch alone needs
    /// no recording: it is already stored evidence.
    pub(crate) fn needs_recording(&self) -> bool {
        !self.faults.is_empty()
    }

    /// The current faults that belong to one Lock, schema faults included: a broken backstop leaves
    /// every promise unverifiable.
    pub(crate) fn faults_of(&self, lock_id: i64) -> Vec<&Fault> {
        self.faults
            .iter()
            .filter(|fault| fault.lock_id.is_none() || fault.lock_id == Some(lock_id))
            .collect()
    }

    /// The recorded faults of one Lock that a later comparison has not erased.
    pub(crate) fn latched_of(&self, lock_id: i64) -> Vec<&Latched> {
        self.latched
            .iter()
            .filter(|latched| latched.lock_id == lock_id)
            .collect()
    }

    /// One concise page-level line for the lock panel (R229, S06-AC08).
    pub(crate) fn summary(&self) -> String {
        if self.ok() {
            return "integrity ok".to_string();
        }
        let current = self.faults.len();
        let mut line = format!(
            "integrity faulted: {current} current fault{}",
            if current == 1 { "" } else { "s" }
        );
        if let Some(first) = self.faults.first() {
            let scope = match first.lock_id {
                Some(lock_id) => format!("lock {lock_id} "),
                None => "schema ".to_string(),
            };
            line.push_str(&format!(" ({scope}{} {})", first.kind, first.subject));
        }
        if !self.latched.is_empty() {
            line.push_str(&format!(", {} latched", self.latched.len()));
        }
        line
    }

    pub(crate) fn value(&self) -> Value {
        json!({
            "ok": self.ok(),
            "faulted": self.needs_recording(),
            "summary": self.summary(),
            "faults": self.faults.iter().map(Fault::value).collect::<Vec<_>>(),
            "latched": self.latched.iter().map(Latched::value).collect::<Vec<_>>(),
        })
    }
}

/// A stored Card row with its raw metadata values, so a decoder fallback cannot hide a changed
/// Workspace, kind or type (R288).
struct StoredCard {
    row_id: i64,
    address: String,
    is_topic: i64,
    is_lit: i64,
    bibtex: Option<String>,
    text: String,
    workspace: String,
    card_type: String,
    facet: Option<String>,
    emphasis: Option<String>,
    level: Option<String>,
    hub_level: Option<String>,
    form: Option<String>,
}

impl StoredCard {
    /// The authored fingerprint of the stored row, or why the raw metadata cannot be decoded.
    fn fingerprint(&self) -> Result<String> {
        fingerprint_of_raw(
            &self.text,
            self.bibtex.as_deref(),
            &self.card_type,
            self.facet.as_deref(),
            self.emphasis.as_deref(),
            self.level.as_deref(),
            self.hub_level.as_deref(),
            self.form.as_deref(),
        )
    }

    fn is_tree_root(&self) -> bool {
        self.is_topic == 1 || self.is_lit == 1
    }
}

fn stored_cards(conn: &Connection) -> Result<Vec<StoredCard>> {
    let mut stmt = conn.prepare(
        "SELECT row_id, COALESCE(location, citation_key), is_topic, is_lit, bibtex, text,
                workspace, card_type, model_facet, model_emphasis, model_level, hub_level, math_form
           FROM cards",
    )?;
    let cards = stmt
        .query_map([], |row| {
            Ok(StoredCard {
                row_id: row.get(0)?,
                address: row.get(1)?,
                is_topic: row.get(2)?,
                is_lit: row.get(3)?,
                bibtex: row.get(4)?,
                text: row.get(5)?,
                workspace: row.get(6)?,
                card_type: row.get(7)?,
                facet: row.get(8)?,
                emphasis: row.get(9)?,
                level: row.get(10)?,
                hub_level: row.get(11)?,
                form: row.get(12)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(cards)
}

/// One active Lock with its raw stored facts, so the comparison never falls back to a decoded
/// default for a value it cannot read.
struct StoredLock {
    lock_id: i64,
    root_row: i64,
    root: String,
    tree: String,
    workspace: String,
    seal: String,
    member_count: i64,
}

fn stored_locks(conn: &Connection) -> Result<Vec<StoredLock>> {
    let mut stmt = conn.prepare(
        "SELECT lock_id, root_row, root_address, tree, workspace, range_fingerprint, member_count
           FROM locks WHERE unlocked_at IS NULL ORDER BY lock_id",
    )?;
    let locks = stmt
        .query_map([], |row| {
            Ok(StoredLock {
                lock_id: row.get(0)?,
                root_row: row.get(1)?,
                root: row.get(2)?,
                tree: row.get(3)?,
                workspace: row.get(4)?,
                seal: row.get(5)?,
                member_count: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(locks)
}

/// The member snapshot rows of one Lock: row identity, address and authored baseline.
fn stored_members(conn: &Connection, lock_id: i64) -> Result<Vec<(i64, String, String)>> {
    let mut stmt = conn
        .prepare("SELECT row_id, address, authored_fp FROM lock_members WHERE lock_id = ?1 ORDER BY address")?;
    let members = stmt
        .query_map([lock_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(members)
}

/// The pin snapshot rows of one Lock: source row, target row and target address.
fn stored_pins(conn: &Connection, lock_id: i64) -> Result<Vec<(i64, i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT source_row, target_row, target_address FROM lock_pins WHERE lock_id = ?1
         ORDER BY target_address, source_row",
    )?;
    let pins = stmt
        .query_map([lock_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(pins)
}

/// The ids of every active Lock, in creation order. A damaged archive answers none instead of
/// failing: a fault that cannot be attributed is still reported, just not recorded.
fn active_lock_ids(conn: &Connection) -> Result<Vec<i64>> {
    if schema_version(conn).ok().flatten().as_deref() != Some(SCHEMA_VERSION) {
        return Ok(Vec::new());
    }
    let Ok(mut stmt) = conn.prepare("SELECT lock_id FROM locks WHERE unlocked_at IS NULL ORDER BY lock_id")
    else {
        return Ok(Vec::new());
    };
    let ids = stmt
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// The schema-side facts of one scan: the version marker, the four lock tables and their columns,
/// the required indexes and the six canonical triggers. `false` means the baseline evidence cannot
/// be read at all, so no per-Lock comparison is attempted and nothing is fabricated (R298).
fn schema_faults(conn: &Connection, integrity: &mut Integrity) -> Result<bool> {
    let version = schema_version(conn)?;
    if version.as_deref() != Some(SCHEMA_VERSION) {
        integrity.faults.push(
            Fault::schema(
                "schema-version",
                SCHEMA_KEY,
                "the Tree Lock schema is not installed or is not the supported generation",
            )
            .expected(SCHEMA_VERSION)
            .actual(version.unwrap_or_else(|| "<missing>".to_string())),
        );
        return Ok(false);
    }
    let mut readable = true;
    for table in LOCK_TABLES {
        let columns = expected_columns(table).expect("every lock table has an expected column set");
        let found = table_columns(conn, table)
            .with_context(|| format!("Tree Lock table `{table}` is unreadable"))?;
        if found != columns {
            integrity.faults.push(
                Fault::schema(
                    "schema-table",
                    table,
                    "a required Tree Lock table is missing or has an unsupported column set",
                )
                .expected(columns.join(","))
                .actual(if found.is_empty() {
                    "missing".to_string()
                } else {
                    found.join(",")
                }),
            );
            readable = false;
        }
    }
    if !readable {
        return Ok(false);
    }
    for (name, sql) in INDEX_DEFS {
        match stored_sql(conn, "index", name)? {
            None => integrity.faults.push(
                Fault::schema("schema-index", name, "a required Lock index is missing")
                    .expected("the canonical index")
                    .actual("missing"),
            ),
            Some(stored) if !same_definition(&stored, sql) => {
                integrity.faults.push(
                    Fault::schema("schema-index", name, "a required Lock index was replaced")
                        .expected("the canonical index")
                        .actual("an altered definition"),
                )
            }
            Some(_) => {}
        }
    }
    for (name, sql) in TRIGGER_DEFS {
        match stored_sql(conn, "trigger", name)? {
            None => integrity.faults.push(
                Fault::schema(
                    "trigger-missing",
                    name,
                    "the canonical protection trigger is missing, so the SQL backstop is incomplete",
                )
                .expected("the canonical trigger")
                .actual("missing"),
            ),
            Some(stored) if !same_definition(&stored, sql) => integrity.faults.push(
                Fault::schema(
                    "trigger-altered",
                    name,
                    "a protection trigger was replaced under its canonical name",
                )
                .expected("the canonical trigger")
                .actual("an altered definition"),
            ),
            Some(_) => {}
        }
    }
    Ok(true)
}

/// The recorded fault events of every active Lock, newest record per identifier kept once.
fn latched_faults(conn: &Connection) -> Result<Vec<Latched>> {
    let mut stmt = conn.prepare(
        "SELECT e.lock_id, e.at, e.detail FROM lock_events e
           JOIN locks l ON l.lock_id = e.lock_id
          WHERE e.action = 'integrity-fault' AND l.unlocked_at IS NULL
          ORDER BY e.lock_id, e.event_id",
    )?;
    let mut latched = Vec::new();
    for row in stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })? {
        let (lock_id, at, raw) = row?;
        let detail = serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null);
        let evidence = &detail["evidence"];
        latched.push(Latched {
            lock_id,
            id: detail["fault"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| raw.clone()),
            kind: evidence["kind"]
                .as_str()
                .unwrap_or("integrity-fault")
                .to_string(),
            card: evidence["card"].as_str().map(str::to_string),
            at,
            detail: evidence["detail"].as_str().unwrap_or_default().to_string(),
        });
    }
    Ok(latched)
}

/// The kind facts of one member row: the root is the Topic or Literature root of its own tree and
/// every other member is a Regular Card of that tree kind.
fn member_kind_ok(lock: &StoredLock, card: &StoredCard) -> bool {
    let literature = crate::is_literature_tree_id(&lock.tree);
    let root = card.address == lock.root;
    card.is_topic == i64::from(root && !literature) && card.is_lit == i64::from(literature)
}

fn member_kind_expected(lock: &StoredLock, address: &str) -> &'static str {
    if crate::is_literature_tree_id(&lock.tree) {
        "a Literature tree member"
    } else if address == lock.root {
        "the Topic root"
    } else {
        "a Topic tree member"
    }
}

/// Compare one active Lock's promise with the stored archive and lock tables (R278–R289).
fn compare_lock(
    conn: &Connection,
    lock: &StoredLock,
    cards: &[StoredCard],
    by_address: &BTreeMap<&str, &StoredCard>,
    by_row: &BTreeMap<i64, &StoredCard>,
    integrity: &mut Integrity,
) -> Result<()> {
    let id = lock.lock_id;
    // R279: the root row exists at the saved root address, keeps the saved Workspace and is the
    // Tree root of the saved tree.
    match by_address.get(lock.root.as_str()) {
        None => integrity.faults.push(
            Fault::lock(
                id,
                "root-missing",
                &lock.root,
                "the locked Tree root no longer exists at its saved address",
            )
            .expected(lock.root.clone())
            .actual("no card at this address"),
        ),
        Some(card) => {
            if card.row_id != lock.root_row {
                integrity.faults.push(
                    Fault::lock(
                        id,
                        "root-changed",
                        &lock.root,
                        "the saved root address now resolves to a different Card row",
                    )
                    .expected(lock.root_row.to_string())
                    .actual(card.row_id.to_string()),
                );
            }
            if card.workspace != lock.workspace {
                integrity.faults.push(
                    Fault::lock(
                        id,
                        "root-changed",
                        &lock.root,
                        "the locked Tree root changed its Workspace",
                    )
                    .expected(lock.workspace.clone())
                    .actual(card.workspace.clone()),
                );
            }
            if crate::tree_root_address(crate::tree_id(&card.address)) != lock.root
                || !card.is_tree_root()
            {
                integrity.faults.push(
                    Fault::lock(
                        id,
                        "root-changed",
                        &lock.root,
                        "the saved root address is no longer a Tree root",
                    )
                    .expected(lock.root.clone())
                    .actual(card.address.clone()),
                );
            }
        }
    }
    // R280: the stored member count equals the number of snapshot rows.
    let members = stored_members(conn, id)?;
    if members.len() as i64 != lock.member_count {
        integrity.faults.push(
            Fault::lock(
                id,
                "member-count",
                &lock.root,
                format!(
                    "stored member count is {} but {} snapshot rows exist",
                    lock.member_count,
                    members.len()
                ),
            )
            .expected(lock.member_count.to_string())
            .actual(members.len().to_string()),
        );
    }
    let mut baseline = BTreeMap::new();
    let mut member_rows = BTreeSet::new();
    for (row_id, address, fingerprint) in &members {
        baseline.insert(address.clone(), fingerprint.clone());
        member_rows.insert(*row_id);
        // R281: every snapshot row resolves to its saved row, address, Workspace and kind.
        match by_row.get(row_id) {
            None => integrity.faults.push(
                Fault::lock(
                    id,
                    "member-missing",
                    address,
                    format!("snapshot row {row_id} no longer exists"),
                )
                .expected(format!("row {row_id} at `{address}`"))
                .actual("no such row"),
            ),
            Some(card) => {
                if &card.address != address {
                    integrity.faults.push(
                        Fault::lock(
                            id,
                            "member-changed",
                            address,
                            format!("snapshot row {row_id} now holds `{}`", card.address),
                        )
                        .expected(address.clone())
                        .actual(card.address.clone()),
                    );
                    continue;
                }
                if card.workspace != lock.workspace {
                    integrity.faults.push(
                        Fault::lock(
                            id,
                            "member-changed",
                            address,
                            "the member changed its Workspace",
                        )
                        .expected(lock.workspace.clone())
                        .actual(card.workspace.clone()),
                    );
                }
                if !member_kind_ok(lock, card) {
                    integrity.faults.push(
                        Fault::lock(id, "member-changed", address, "the member changed its kind")
                            .expected(member_kind_expected(lock, address))
                            .actual(format!(
                                "is_topic={} is_lit={}",
                                card.is_topic, card.is_lit
                            )),
                    );
                }
                // R282: each current authored fingerprint matches its stored baseline.
                match card.fingerprint() {
                    Ok(current) if &current != fingerprint => integrity.faults.push(
                        Fault::lock(
                            id,
                            "fingerprint-changed",
                            address,
                            "the member's authored identity changed",
                        )
                        .expected(fingerprint.clone())
                        .actual(current),
                    ),
                    Ok(_) => {}
                    Err(error) => integrity.faults.push(
                        Fault::lock(
                            id,
                            "member-undecodable",
                            address,
                            format!("the stored metadata cannot be decoded: {error:#}"),
                        )
                        .expected("valid stored metadata")
                        .actual("an unsupported value"),
                    ),
                }
            }
        }
    }
    // R284: the recomputed range fingerprint equals the saved seal.
    let recomputed = seal_of(&baseline);
    if recomputed != lock.seal {
        integrity.faults.push(
            Fault::lock(
                id,
                "seal-changed",
                &lock.root,
                "the stored snapshot no longer matches the Lock seal",
            )
            .expected(lock.seal.clone())
            .actual(recomputed),
        );
    }
    // R283: the current exact tree membership equals the snapshot, extras included.
    let snapshot = baseline.keys().cloned().collect::<BTreeSet<_>>();
    let current = cards
        .iter()
        .filter(|card| crate::is_tree_member(&card.address, &lock.tree))
        .map(|card| card.address.clone())
        .collect::<BTreeSet<_>>();
    for address in current.difference(&snapshot) {
        integrity.faults.push(Fault::lock(
            id,
            "member-extra",
            address,
            "a Card is inside the locked tree but not in its snapshot",
        ));
    }
    for address in snapshot.difference(&current) {
        // A missing Card is reported once, by its snapshot row, above.
        if by_address.contains_key(address.as_str()) {
            integrity.faults.push(Fault::lock(
                id,
                "member-moved",
                address,
                "the member no longer belongs to the locked tree",
            ));
        }
    }
    // R285: every pin source is a member, every target still exists at its saved row/address and
    // lies outside the locked tree.
    let pins = stored_pins(conn, id)?;
    for (source_row, target_row, target_address) in &pins {
        if !member_rows.contains(source_row) {
            integrity.faults.push(
                Fault::lock(
                    id,
                    "pin-source",
                    target_address,
                    format!("pin source row {source_row} is not a member of this Lock"),
                )
                .expected("a member row")
                .actual(format!("row {source_row}")),
            );
        }
        match by_row.get(target_row) {
            None => integrity.faults.push(
                Fault::lock(
                    id,
                    "pin-missing",
                    target_address,
                    format!("pinned target row {target_row} no longer exists"),
                )
                .expected(format!("row {target_row} at `{target_address}`"))
                .actual("no such row"),
            ),
            Some(card) if &card.address != target_address => integrity.faults.push(
                Fault::lock(
                    id,
                    "pin-changed",
                    target_address,
                    format!("pinned target row {target_row} now holds `{}`", card.address),
                )
                .expected(target_address.clone())
                .actual(card.address.clone()),
            ),
            Some(card) if crate::is_tree_member(&card.address, &lock.tree) => {
                integrity.faults.push(Fault::lock(
                    id,
                    "pin-inside",
                    target_address,
                    "a pin names a Card inside its own locked tree",
                ))
            }
            Some(_) => {}
        }
    }
    // R286: the authored external Links and the derived index still agree with the stored pins.
    integrity.faults.extend(link_pin_faults(
        conn,
        lock,
        &members,
        &pins,
        by_address,
        by_row,
    )?);
    Ok(())
}

/// The Broken Links the owner saw when one Lock was accepted, from its retained `lock` event.
fn captured_warnings(conn: &Connection, lock_id: i64) -> Result<BTreeSet<(String, String)>> {
    let mut broken = BTreeSet::new();
    if let Some(detail) = conn
        .query_row(
            "SELECT detail FROM lock_events WHERE lock_id = ?1 AND action = 'lock'
             ORDER BY event_id LIMIT 1",
            [lock_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        && let Ok(value) = serde_json::from_str::<Value>(&detail)
    {
        for warning in value["warnings"].as_array().into_iter().flatten() {
            if let (Some(source), Some(target)) =
                (warning["source"].as_str(), warning["target"].as_str())
            {
                broken.insert((source.to_string(), target.to_string()));
            }
        }
    }
    Ok(broken)
}

/// Compare each member's authored external Links with the stored pins, excluding the addresses that
/// were already broken when the owner accepted the Lock, and cross-check the Link index rows.
fn link_pin_faults(
    conn: &Connection,
    lock: &StoredLock,
    members: &[(i64, String, String)],
    pins: &[(i64, i64, String)],
    by_address: &BTreeMap<&str, &StoredCard>,
    by_row: &BTreeMap<i64, &StoredCard>,
) -> Result<Vec<Fault>> {
    let mut faults = Vec::new();
    let broken = captured_warnings(conn, lock.lock_id)?;
    let indexed = crate::link_index::ready(conn);
    for (row_id, address, _) in members {
        let Some(card) = by_address.get(address.as_str()) else {
            continue;
        };
        let Ok(parsed) = crate::parse_card_text(&card.text) else {
            // An unreadable body is already reported once, by the member's fingerprint check.
            continue;
        };
        let Ok(body) = crate::link::LinkBody::parse(&parsed.body) else {
            continue;
        };
        let occurrences = body.occurrences();
        let mut expected = BTreeSet::new();
        let mut authored = Vec::new();
        for occurrence in occurrences {
            let target = occurrence.target;
            authored.push(target.to_string());
            let Some(target_card) = by_address.get(target) else {
                continue;
            };
            if crate::is_tree_member(&target_card.address, &lock.tree) {
                continue;
            }
            if broken.contains(&(address.clone(), target.to_string())) {
                continue;
            }
            expected.insert(target_card.row_id);
        }
        let stored = pins
            .iter()
            .filter(|(source_row, _, _)| source_row == row_id)
            .map(|(_, target_row, _)| *target_row)
            .collect::<BTreeSet<_>>();
        if expected != stored {
            let expected_addresses = expected
                .iter()
                .map(|row| by_row.get(row).map_or_else(|| row.to_string(), |card| card.address.clone()))
                .collect::<Vec<_>>();
            let stored_addresses = pins
                .iter()
                .filter(|(source_row, _, _)| source_row == row_id)
                .map(|(_, _, target)| target.clone())
                .collect::<Vec<_>>();
            faults.push(
                Fault::lock(
                    lock.lock_id,
                    "pin-set",
                    address,
                    format!(
                        "resolvable external Links of row {row_id} do not agree with its stored pins"
                    ),
                )
                .expected(expected_addresses.join(", "))
                .actual(stored_addresses.join(", ")),
            );
        }
        if indexed {
            let mut stmt = conn.prepare("SELECT target FROM links WHERE source = ?1 ORDER BY ord")?;
            let rows = stmt
                .query_map([address.as_str()], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            if rows != authored {
                faults.push(
                    Fault::lock(
                        lock.lock_id,
                        "index-changed",
                        address,
                        "the derived Link index disagrees with the authored body; run `zt reindex`",
                    )
                    .expected(authored.join(", "))
                    .actual(rows.join(", ")),
                );
            }
        }
    }
    Ok(faults)
}

/// Compare every active Lock's promise with the stored archive in one consistent read view. This is
/// the pure scan (R027, R278): it records nothing, so it is also usable read-only.
fn scan(conn: &Connection) -> Result<Integrity> {
    let mut integrity = Integrity::default();
    if !schema_faults(conn, &mut integrity)? {
        integrity.faults.sort();
        integrity.faults.dedup();
        return Ok(integrity);
    }
    let locks = stored_locks(conn)?;
    if locks.is_empty() {
        integrity.faults.sort();
        integrity.faults.dedup();
        return Ok(integrity);
    }
    let cards = stored_cards(conn)?;
    let by_address = cards
        .iter()
        .map(|card| (card.address.as_str(), card))
        .collect::<BTreeMap<_, _>>();
    let by_row = cards
        .iter()
        .map(|card| (card.row_id, card))
        .collect::<BTreeMap<_, _>>();
    for lock in &locks {
        compare_lock(conn, lock, &cards, &by_address, &by_row, &mut integrity)?;
    }
    integrity.latched = latched_faults(conn)?;
    integrity.faults.sort();
    integrity.faults.dedup();
    integrity
        .latched
        .sort_by(|a, b| (&a.lock_id, &a.id).cmp(&(&b.lock_id, &b.id)));
    integrity.latched.dedup();
    Ok(integrity)
}

/// The full integrity comparison. It runs in one consistent read view: in its own read transaction
/// when the caller has none, or inside the caller's transaction (R278).
pub(crate) fn check(conn: &Connection) -> Result<Integrity> {
    if conn.is_autocommit() {
        let view = conn.unchecked_transaction()?;
        let integrity = scan(&view)?;
        let _ = view.rollback();
        Ok(integrity)
    } else {
        scan(conn)
    }
}

/// Persist newly observed faults as `integrity-fault` events under the caller's edit lock and
/// transaction (R027, R295). A repeated scan of the same fault adds no event; a schema fault is
/// latched once on every active Lock it leaves unprotected.
pub(crate) fn record_faults_tx(conn: &Connection, integrity: &Integrity) -> Result<usize> {
    let active = active_lock_ids(conn)?;
    let mut recorded = 0;
    for fault in &integrity.faults {
        let targets: Vec<i64> = match fault.lock_id {
            Some(lock_id) => vec![lock_id],
            None => active.clone(),
        };
        for lock_id in targets {
            let id = fault.id_for_lock(lock_id);
            let seen = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM lock_events
                     WHERE lock_id = ?1 AND action = 'integrity-fault'
                       AND CASE WHEN json_valid(detail) THEN json_extract(detail, '$.fault') END = ?2)",
                    params![lock_id, id],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap_or(false);
            if seen {
                continue;
            }
            conn.execute(
                "INSERT INTO lock_events(lock_id, action, at, detail)
                 VALUES(?1, 'integrity-fault', strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?2)",
                params![
                    lock_id,
                    json!({"fault": id, "evidence": fault.value()}).to_string()
                ],
            )?;
            recorded += 1;
        }
    }
    if recorded > 0 {
        crate::link_index::bump_revision(conn)?;
    }
    Ok(recorded)
}

/// What one guarded startup comparison did: the faults it recorded first, the protection it
/// reinstalled second, and the state the service starts with.
pub(crate) struct StartupGuard {
    pub(crate) recorded: usize,
    pub(crate) repaired: Vec<String>,
    pub(crate) integrity: Integrity,
}

/// The guarded startup comparison (R290, R296–R298). In one transaction it compares the promise with
/// the stored archive and the protection catalog **before** any repair, records every newly observed
/// fault, then reinstalls the canonical protection that is missing or altered and validates it. A
/// backstop that cannot be restored refuses writable startup.
pub(crate) fn startup_guard(conn: &Connection) -> Result<StartupGuard> {
    let tx = conn.unchecked_transaction()?;
    let integrity = scan(&tx)?;
    let recorded = record_faults_tx(&tx, &integrity)?;
    let repaired = install_or_repair(&tx)?;
    validate_schema(&tx)?;
    validate_indexes(&tx)?;
    validate_triggers(&tx)?;
    tx.commit()?;
    Ok(StartupGuard {
        recorded,
        repaired,
        integrity: check(conn)?,
    })
}

/// The comparison a Card, row-identity or trigger migration runs first (R293). A no-op when no
/// promise could be affected: no lock schema, or no active Lock.
pub(crate) fn check_before_migration(conn: &Connection) -> Result<usize> {
    if schema_version(conn)?.as_deref() != Some(SCHEMA_VERSION) {
        return Ok(0);
    }
    if active_lock_ids(conn)?.is_empty() {
        return Ok(0);
    }
    let tx = conn.unchecked_transaction()?;
    let integrity = scan(&tx)?;
    let recorded = record_faults_tx(&tx, &integrity)?;
    tx.commit()?;
    Ok(recorded)
}

// ---------------------------------------------------------------------------------------------
// History and export
// ---------------------------------------------------------------------------------------------

/// The versioned dump and panel representation: every Lock (active or ended), its retained
/// snapshots, its ordered events and the current integrity result. Each active Lock also carries its
/// own current and latched faults, so the panel can mark that tree's seal (R229, S06-AC08).
/// The same representation for one already computed comparison, so the panel route scans once.
pub(crate) fn export_with(conn: &Connection, integrity: &Integrity) -> Result<Value> {
    let Some(state) = load_state(conn)? else {
        return Ok(json!({
            "version": 1,
            "schema_version": Value::Null,
            "locks": [],
            "integrity": integrity.value(),
        }));
    };
    let mut locks = Vec::new();
    for lock in state.locks.values() {
        let mut members = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT row_id, address, authored_fp FROM lock_members WHERE lock_id = ?1 ORDER BY address",
        )?;
        for row in stmt.query_map([lock.lock_id], |row| {
            Ok(json!({
                "row_id": row.get::<_, i64>(0)?,
                "address": row.get::<_, String>(1)?,
                "fingerprint": row.get::<_, String>(2)?,
            }))
        })? {
            members.push(row?);
        }
        let mut pins = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT target_row, target_address, source_row FROM lock_pins WHERE lock_id = ?1
             ORDER BY target_address, source_row",
        )?;
        for row in stmt.query_map([lock.lock_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })? {
            let (target_row, target_address, source_row) = row?;
            pins.push(json!({
                "target_row": target_row,
                "target": target_address,
                "source_row": source_row,
            }));
        }
        let mut events = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT event_id, action, at, detail FROM lock_events WHERE lock_id = ?1 ORDER BY event_id",
        )?;
        for row in stmt.query_map([lock.lock_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })? {
            let (event_id, action, at, detail) = row?;
            let detail = serde_json::from_str::<Value>(&detail)
                .unwrap_or_else(|_| Value::String(detail.clone()));
            events.push(json!({
                "event_id": event_id,
                "action": action,
                "at": at,
                "detail": detail,
            }));
        }
        let mut value = lock.value();
        let object = value.as_object_mut().expect("Lock value is an object");
        object.insert("members".into(), Value::Array(members));
        object.insert("pins".into(), Value::Array(pins));
        let warnings = retained_warnings(conn, lock.lock_id)?;
        object.insert("warnings".into(), Value::Array(warnings));
        object.insert("events".into(), Value::Array(events));
        // A fault and a latch are both shown beside the Lock they belong to; a fault never hides a
        // Lock, a Card or the seal the owner reviewed.
        if lock.unlocked_at.is_none() {
            object.insert(
                "faults".into(),
                Value::Array(
                    integrity
                        .faults_of(lock.lock_id)
                        .iter()
                        .map(|fault| fault.value())
                        .collect(),
                ),
            );
            object.insert(
                "latched_faults".into(),
                Value::Array(
                    integrity
                        .latched_of(lock.lock_id)
                        .iter()
                        .map(|latched| latched.value())
                        .collect(),
                ),
            );
            let broken = !integrity.faults_of(lock.lock_id).is_empty()
                || !integrity.latched_of(lock.lock_id).is_empty();
            object.insert("seal_intact".into(), Value::Bool(!broken));
        }
        locks.push(value);
    }
    Ok(json!({
        "version": 1,
        "schema_version": SCHEMA_VERSION,
        "locks": locks,
        "integrity": integrity.value(),
    }))
}

/// The Broken Links the owner saw when the Lock was accepted, read back from its retained `lock`
/// event, so the panel shows the same source and target addresses after any later change.
fn retained_warnings(conn: &Connection, lock_id: i64) -> Result<Vec<Value>> {
    let detail = conn
        .query_row(
            "SELECT detail FROM lock_events WHERE lock_id = ?1 AND action = 'lock'
             ORDER BY event_id LIMIT 1",
            [lock_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(detail) = detail else {
        return Ok(Vec::new());
    };
    let Ok(value) = serde_json::from_str::<Value>(&detail) else {
        return Ok(Vec::new());
    };
    Ok(value["warnings"]
        .as_array()
        .map(|warnings| {
            warnings
                .iter()
                .map(|warning| {
                    json!({
                        "source": warning["source"],
                        "target": warning["target"],
                    })
                })
                .collect()
        })
        .unwrap_or_default())
}

// ---------------------------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use super::plan as resolve_plan;
    use crate::CardType;

    fn archive() -> (tempfile::TempDir, Connection) {
        let temp = tempfile::tempdir().expect("tempdir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        (temp, conn)
    }

    fn card_text(title: &str, body: &str) -> String {
        crate::compose_card_text(title, body, "")
    }

    /// A Topic with two Regular Cards: the root, the first Card `<tree>/1`, and its Side successor
    /// `<tree>/1|a`, which links the first Card.
    fn topic_tree(conn: &mut Connection) -> String {
        let root = crate::create_topic(conn, Workspace::Human, &card_text("Topic", "root body"))
            .expect("create topic");
        let tree = crate::tree_id(&root).to_string();
        let first = format!("{tree}/1");
        crate::insert_card(
            conn,
            &first,
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("First", "body"),
        )
        .expect("first card");
        crate::insert_card(
            conn,
            &format!("{first}|a"),
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Side", &format!("links [[{first}]]")),
        )
        .expect("side card");
        root
    }

    fn first_address(root: &str) -> String {
        format!("{}/1", crate::tree_id(root))
    }

    fn side_address(root: &str) -> String {
        format!("{}|a", first_address(root))
    }

    fn fingerprint_query(
        conn: &Connection,
        text: &str,
        metadata: [Option<&str>; 7],
    ) -> rusqlite::Result<String> {
        conn.query_row(
            "SELECT zt_authored_fp(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                text,
                metadata[0],
                metadata[1],
                metadata[2],
                metadata[3],
                metadata[4],
                metadata[5],
                metadata[6],
            ],
            |row| row.get(0),
        )
    }

    #[test]
    fn the_sql_fingerprint_matches_the_rust_baseline_for_every_field() {
        use crate::Card;
        let conn = Connection::open_in_memory().expect("memory database");
        register_sql_functions(&conn).expect("register functions");
        let text = card_text("Title", "body with a [[1/2]] link");
        let cases: Vec<Card> = vec![
            Card {
                row_id: 1,
                address: "1/1".into(),
                citation_key: None,
                is_topic: false,
                is_lit: false,
                bibtex: None,
                text: text.clone(),
                workspace: Workspace::Human,
                card_type: CardType::Idea,
                facet: None,
                emphasis: None,
                level: None,
                hub_level: None,
                form: None,
            },
            Card {
                row_id: 2,
                address: "1/1".into(),
                citation_key: None,
                is_topic: false,
                is_lit: false,
                bibtex: Some("booktitle = {X}".into()),
                text: text.clone(),
                workspace: Workspace::Human,
                card_type: CardType::Event,
                facet: None,
                emphasis: None,
                level: None,
                hub_level: None,
                form: None,
            },
            Card {
                row_id: 3,
                address: "1/1".into(),
                citation_key: None,
                is_topic: false,
                is_lit: false,
                bibtex: None,
                text: text.clone(),
                workspace: Workspace::Human,
                card_type: CardType::Model,
                facet: Some(crate::model::Facet::parse("Setting").expect("facet")),
                emphasis: Some(crate::model::Emphasis::parse("core").expect("emphasis")),
                level: None,
                hub_level: None,
                form: None,
            },
            Card {
                row_id: 4,
                address: "1/1".into(),
                citation_key: None,
                is_topic: false,
                is_lit: false,
                bibtex: None,
                text: text.clone(),
                workspace: Workspace::Human,
                card_type: CardType::Model,
                facet: None,
                emphasis: None,
                level: Some(crate::model::Level::parse("formal").expect("level")),
                hub_level: None,
                form: Some(crate::math::MathForm::parse("theorem").expect("form")),
            },
            Card {
                row_id: 5,
                address: "1/1".into(),
                citation_key: None,
                is_topic: false,
                is_lit: false,
                bibtex: None,
                text: text.clone(),
                workspace: Workspace::Human,
                card_type: CardType::Hub,
                facet: None,
                emphasis: None,
                level: None,
                hub_level: Some(crate::hub::HubLevel::parse("topic").expect("hub level")),
                form: None,
            },
        ];
        for card in cases {
            let expected = crate::card::authored_fingerprint(&card).expect("rust fingerprint");
            let actual = fingerprint_query(
                &conn,
                &card.text,
                [
                    card.bibtex.as_deref(),
                    Some(card.card_type.id()),
                    card.facet.as_ref().map(|facet| facet.id()),
                    card.emphasis.map(|emphasis| emphasis.id()),
                    card.level.map(|level| level.id()),
                    card.hub_level.map(|level| level.id()),
                    card.form.map(|form| form.id()),
                ],
            )
            .expect("SQL fingerprint");
            assert_eq!(
                actual, expected,
                "case {} {:?}",
                card.card_type.id(),
                card.facet
            );
        }
    }

    #[test]
    fn the_sql_fingerprint_aborts_on_an_unknown_stored_enum() {
        let conn = Connection::open_in_memory().expect("memory database");
        register_sql_functions(&conn).expect("register functions");
        let error = fingerprint_query(
            &conn,
            &card_text("Title", "body"),
            [None, Some("bogus"), None, None, None, None, None],
        )
        .expect_err("unknown enum aborts");
        assert!(error.to_string().contains("unknown card type"), "{error}");
    }

    #[test]
    fn zt_tree_root_resolves_topic_literature_and_moving_placeholders() {
        let conn = Connection::open_in_memory().expect("memory database");
        register_sql_functions(&conn).expect("register functions");
        let root = |address: &str| -> Option<String> {
            conn.query_row("SELECT zt_tree_root(?1)", [address], |row| row.get(0))
                .expect("tree root")
        };
        assert_eq!(root("3/0"), Some("3/0".into()));
        assert_eq!(root("3/12|a|2"), Some("3/0".into()));
        assert_eq!(root("30/1"), Some("30/0".into()));
        assert_eq!(root("Smith2024"), Some("Smith2024".into()));
        assert_eq!(root("Smith2024/1|a"), Some("Smith2024".into()));
        assert_eq!(root("__moving__3/1"), None);
        assert_eq!(root("not an address"), None);
        assert_eq!(final_tree_root("3/2"), Some("3/0".into()));
        assert_eq!(final_tree_root("__moving__Smith2024/1"), None);
    }

    #[test]
    fn the_seal_hashes_address_sorted_address_tab_fingerprint_lines() {
        let members = BTreeMap::from([
            ("1/1".to_string(), "aa".to_string()),
            ("1/0".to_string(), "bb".to_string()),
        ]);
        let mut hasher = Sha256::new();
        hasher.update(b"1/0\tbb\n1/1\taa\n");
        let expected = hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(seal_of(&members), expected);
        assert_eq!(seal_of(&members).len(), 64);
    }

    #[test]
    fn a_lock_plan_names_the_tree_its_members_its_pins_and_its_warnings() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        crate::insert_card(
            &mut conn,
            "2/0",
            true,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Far", "far body"),
        )
        .expect("far topic");
        crate::insert_card(
            &mut conn,
            "2/1",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Pinned", "pinned body"),
        )
        .expect("pinned card");
        crate::insert_card(
            &mut conn,
            "2/2",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Doomed", "will be deleted"),
        )
        .expect("doomed card");
        // The locked tree pins 2/1 and links 2/2, whose deletion leaves one preexisting Broken link.
        let side = crate::load_card(&conn, &side_address(&root)).expect("side");
        crate::update_card_text(
            &mut conn,
            &side,
            &card_text(
                "Side",
                &format!(
                    "links [[{}]] and [[2/1]] and [[2/2]]",
                    first_address(&root)
                ),
            ),
        )
        .expect("rewrite side");
        let doomed = crate::delete_plan(&conn, "2/2").expect("doomed plan");
        crate::apply_delete_plan(&mut conn, doomed).expect("delete doomed");
        let plan = resolve_plan(&conn, &root, Some("frozen for review")).expect("plan");
        assert_eq!(plan.root, root);
        assert_eq!(plan.tree, crate::tree_id(&root));
        assert_eq!(plan.zone, "topic");
        assert_eq!(plan.workspace, Workspace::Human);
        assert_eq!(plan.members.len(), 3);
        assert_eq!(plan.regular_count(), 2);
        assert_eq!(plan.members.iter().filter(|member| member.root).count(), 1);
        assert_eq!(plan.pins.len(), 1, "one distinct external target");
        assert_eq!(plan.pins[0].target, "2/1");
        assert_eq!(plan.pins[0].sources.len(), 1);
        assert_eq!(plan.warnings.len(), 1);
        assert_eq!(plan.warnings[0].target, "2/2");
        assert_eq!(plan.note.as_deref(), Some("frozen for review"));
        assert_eq!(plan.plan_hash.len(), 64);
        assert_eq!(plan.value()["confirm"], "lock");
        // A changed note changes the hash; the same facts reproduce it.
        let again = resolve_plan(&conn, &root, Some("frozen for review")).expect("plan again");
        assert_eq!(again.plan_hash, plan.plan_hash);
        let renamed = resolve_plan(&conn, &root, Some("other")).expect("other note");
        assert_ne!(renamed.plan_hash, plan.plan_hash);
    }

    #[test]
    fn a_regular_card_and_a_second_lock_are_refused() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        let error = resolve_plan(&conn, &first_address(&root), None).expect_err("regular anchor");
        assert!(error.to_string().contains("Regular Card"), "{error}");
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        let error = resolve_plan(&conn, &root, None).expect_err("already locked");
        assert!(error.to_string().contains("already locked"), "{error}");
    }

    #[test]
    fn unlock_ends_only_that_lock_and_keeps_its_history() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        let plan = resolve_plan(&conn, &root, Some("keep me")).expect("plan");
        let tx = conn.transaction().expect("transaction");
        let lock = lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        assert_eq!(lock.member_count, 3);
        let revision_after_lock = crate::link_index::revision(&conn).expect("revision");
        let tx = conn.transaction().expect("transaction");
        let ended = unlock_tx(&tx, lock.lock_id).expect("unlock");
        tx.commit().expect("commit");
        assert!(ended.unlocked_at.is_some());
        assert!(crate::link_index::revision(&conn).expect("revision") > revision_after_lock);
        let members: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lock_members WHERE lock_id = ?1",
                [lock.lock_id],
                |row| row.get(0),
            )
            .expect("member count");
        assert_eq!(members, 0);
        let events: Vec<String> = conn
            .prepare("SELECT action FROM lock_events WHERE lock_id = ?1 ORDER BY event_id")
            .expect("events")
            .query_map([lock.lock_id], |row| row.get(0))
            .expect("events")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("events");
        assert_eq!(events, ["lock", "unlock"]);
        let export = export_with(&conn, &check(&conn).expect("check")).expect("export");
        assert_eq!(export["locks"][0]["unlocked_at"].is_string(), true);
        assert_eq!(export["locks"][0]["events"][0]["action"], "lock");
        // The tree can be locked again, with a new Lock id and the same seal.
        let plan_again = resolve_plan(&conn, &root, None).expect("plan again");
        assert_eq!(plan_again.seal, plan.seal);
        let tx = conn.transaction().expect("transaction");
        let second = lock_tx(&tx, &plan_again).expect("second lock");
        tx.commit().expect("commit");
        assert_ne!(second.lock_id, lock.lock_id);
    }

    /// S02 (#125, R083): one pin per distinct `(locked source row, existing target row)` pair, so a
    /// repeated macro in one body still locks and still stores one row.
    #[test]
    fn a_repeated_macro_produces_one_pin_row_for_its_source() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        crate::insert_card(
            &mut conn,
            "2/0",
            true,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Far", "far body"),
        )
        .expect("far topic");
        crate::insert_card(
            &mut conn,
            "2/1",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Pinned", "pinned body"),
        )
        .expect("pinned card");
        let side = crate::load_card(&conn, &side_address(&root)).expect("side");
        crate::update_card_text(
            &mut conn,
            &side,
            &card_text(
                "Side",
                "names [[2/1]] once, then [[2/1]] again and [[2/1]] a third time",
            ),
        )
        .expect("rewrite side");

        let plan = resolve_plan(&conn, &root, None).expect("plan");
        assert_eq!(plan.pins.len(), 1, "one distinct target");
        assert_eq!(plan.pins[0].target, "2/1");
        assert_eq!(
            plan.pins[0].sources.len(),
            1,
            "one distinct source row for a repeated macro: {:?}",
            plan.pins[0].sources
        );

        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("a repeated macro still locks");
        tx.commit().expect("commit");
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM lock_pins", [], |row| row.get(0))
            .expect("pin rows");
        assert_eq!(rows, 1, "one pin row");
    }

    #[test]
    fn a_note_is_bounded_has_no_nul_and_an_empty_note_is_null() {
        assert_eq!(validate_note(None).expect("none"), None);
        assert_eq!(validate_note(Some("")).expect("empty"), None);
        assert_eq!(validate_note(Some("   ")).expect("blank"), None);
        assert_eq!(
            validate_note(Some("  keep inner  ")).expect("trimmed"),
            Some("  keep inner  ".into())
        );
        assert!(validate_note(Some("a\0b")).is_err());
        assert!(validate_note(Some(&"x".repeat(501))).is_err());
        assert_eq!(validate_note(Some(&"y".repeat(500))).expect("bounded"), Some("y".repeat(500)));
    }

    #[test]
    fn the_gate_refuses_member_edits_and_pinned_target_moves_and_allows_generated_refresh() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        crate::insert_card(
            &mut conn,
            "2/0",
            true,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Far", "far"),
        )
        .expect("far topic");
        crate::insert_card(
            &mut conn,
            "2/1",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Pinned", "pinned"),
        )
        .expect("pinned");
        let side = crate::load_card(&conn, &side_address(&root)).expect("side");
        crate::update_card_text(
            &mut conn,
            &side,
            &card_text("Side", &format!("links [[{}]] and [[2/1]]", first_address(&root))),
        )
        .expect("link");
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");

        let member = crate::load_card(&conn, &first_address(&root)).expect("member");
        let refusal = ensure_unlocked(&conn, Change::Edit(member.row_id)).expect_err("edit");
        let refusal = refusal
            .downcast_ref::<Refusal>()
            .expect("typed refusal")
            .clone();
        assert_eq!(refusal.code_string(), "locked");
        assert_eq!(refusal.tree.as_deref(), Some(root.as_str()));

        let pinned = crate::load_card(&conn, "2/1").expect("pinned");
        assert!(ensure_unlocked(&conn, Change::Edit(pinned.row_id)).is_ok());
        let mapping = BTreeMap::from([("2/1".to_string(), "2/3".to_string())]);
        let refusal = ensure_unlocked(&conn, Change::Mapping(&mapping)).expect_err("move pin");
        let refusal = refusal.downcast_ref::<Refusal>().expect("typed refusal");
        assert_eq!(refusal.kind, RefusalKind::Pinned);

        // An unlocked Card may still be edited, and a generated Reverse-link refresh passes.
        assert!(ensure_unlocked(&conn, Change::Edit(pinned.row_id)).is_ok());
        crate::regenerate_reverse_links_tx(&conn).expect("generated refresh");

        // A new successor inside the locked tree is refused at its final address.
        let refusal = ensure_unlocked(&conn, Change::CreateAt(&format!("{root}/2")))
            .expect_err("create inside");
        assert_eq!(
            refusal.downcast_ref::<Refusal>().expect("typed").kind,
            RefusalKind::LockedTree
        );
    }

    #[test]
    fn a_delete_plan_is_refused_when_it_names_a_member_or_a_pinned_target() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        crate::insert_card(
            &mut conn,
            "2/0",
            true,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Far", "far"),
        )
        .expect("far");
        crate::insert_card(
            &mut conn,
            "2/1",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Pinned", "pinned"),
        )
        .expect("pinned");
        crate::insert_card(
            &mut conn,
            "2/2",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Other", "other"),
        )
        .expect("other");
        let side = crate::load_card(&conn, &side_address(&root)).expect("side");
        crate::update_card_text(&mut conn, &side, &card_text("Side", "links [[2/1]]"))
            .expect("link");
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");

        let member_plan = crate::delete_plan(&conn, &first_address(&root)).expect("plan member");
        let refusal = ensure_unlocked(&conn, Change::Delete(&member_plan)).expect_err("member");
        assert_eq!(
            refusal.downcast_ref::<Refusal>().expect("typed").kind,
            RefusalKind::Locked
        );
        let pinned_plan = crate::delete_plan(&conn, "2/1").expect("plan pinned");
        let refusal = ensure_unlocked(&conn, Change::Delete(&pinned_plan)).expect_err("pinned");
        let refusal = refusal.downcast_ref::<Refusal>().expect("typed");
        assert_eq!(refusal.kind, RefusalKind::Pinned);
        assert!(!refusal.pinned_by[0].sources.is_empty());
        // An unrelated Card can still be deleted.
        let other_plan = crate::delete_plan(&conn, "2/2").expect("plan other");
        assert!(ensure_unlocked(&conn, Change::Delete(&other_plan)).is_ok());
    }

    /// S06-AC04/S06-AC05: a tampered member and a dropped trigger are exact faults, each is
    /// recorded once, the schema fault is latched on every active Lock, and a repeated scan adds no
    /// second event.
    #[test]
    fn check_reports_a_changed_member_and_a_dropped_trigger_once() {
        let (temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        assert!(check(&conn).expect("check").ok());
        conn.execute_batch("DROP TRIGGER lock_no_change")
            .expect("drop trigger");
        conn.execute(
            "UPDATE cards SET text = '# First\n\nchanged body\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
            [first_address(&root)],
        )
        .expect("tamper");
        let integrity = check(&conn).expect("check");
        let kinds = integrity
            .faults
            .iter()
            .map(|fault| fault.kind.as_str())
            .collect::<BTreeSet<_>>();
        assert!(kinds.contains("fingerprint-changed"), "{kinds:?}");
        assert!(kinds.contains("trigger-missing"), "{kinds:?}");
        let tx = conn.transaction().expect("transaction");
        // The member fault belongs to the Lock; the schema fault is latched on that same Lock, so
        // one Lock keeps one record per observed fault.
        assert_eq!(record_faults_tx(&tx, &integrity).expect("record"), 2);
        tx.commit().expect("commit");
        let recorded: Vec<String> = conn
            .prepare("SELECT detail FROM lock_events WHERE action = 'integrity-fault'")
            .expect("events")
            .query_map([], |row| row.get(0))
            .expect("events")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("events");
        assert_eq!(recorded.len(), 2);
        assert!(
            recorded
                .iter()
                .any(|detail| detail.contains("fingerprint-changed")),
            "{recorded:?}"
        );
        assert!(
            recorded
                .iter()
                .any(|detail| detail.contains("trigger-missing")),
            "{recorded:?}"
        );
        let tx = conn.transaction().expect("transaction");
        assert_eq!(record_faults_tx(&tx, &integrity).expect("record again"), 0);
        tx.commit().expect("commit");
        // The two records are latched on the still-active Lock: health now needs both a clean
        // comparison and no latch.
        let after = check(&conn).expect("check");
        assert!(!after.ok());
        assert_eq!(after.latched.len(), 2, "{:?}", after.latched);
        assert!(
            after
                .latched
                .iter()
                .any(|latched| latched.kind == "trigger-missing")
        );
        let _ = temp;
    }

    /// R079, R080 and R082: the seal hashes authored identity only. Address, Workspace and the
    /// `is_topic`/`is_lit` flags are compared separately, and even a no-op request is refused.
    #[test]
    fn the_seal_checks_address_workspace_and_kind_separately_and_refuses_no_ops() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");

        // The authored fingerprint ignores the facts the Lock checks separately.
        let card = crate::load_card(&conn, &root).expect("root card");
        let fingerprint = crate::card::authored_fingerprint(&card).expect("fingerprint");
        let mut elsewhere = card.clone();
        elsewhere.row_id = 999;
        elsewhere.address = "9/0".into();
        elsewhere.workspace = Workspace::Agent;
        elsewhere.is_topic = false;
        elsewhere.is_lit = true;
        assert_eq!(
            crate::card::authored_fingerprint(&elsewhere).expect("other fingerprint"),
            fingerprint,
            "address, row, Workspace and kind are outside the authored fingerprint"
        );

        // A kind change is refused by the statement trigger while the Lock is active.
        let error = conn
            .execute(
                "UPDATE cards SET is_topic = 0 WHERE row_id = ?1",
                [card.row_id],
            )
            .expect_err("the trigger refuses a kind change");
        assert!(error.to_string().contains("locked tree"), "{error}");

        // Even a request that would change nothing is a Lock refusal (R082).
        let refusal = ensure_unlocked(&conn, Change::Edit(card.row_id)).expect_err("no-op edit");
        assert_eq!(
            refusal.downcast_ref::<Refusal>().expect("typed").kind,
            RefusalKind::Locked
        );

        // With the statement trigger removed, the same kind change is an exact integrity fault
        // even though the authored fingerprint still matches its snapshot.
        conn.execute_batch("DROP TRIGGER lock_no_change")
            .expect("drop trigger");
        conn.execute(
            "UPDATE cards SET is_topic = 0 WHERE row_id = ?1",
            [card.row_id],
        )
        .expect("tamper");
        let integrity = check(&conn).expect("check");
        assert!(!integrity.ok());
        let kinds = integrity
            .faults
            .iter()
            .map(|fault| fault.kind.as_str())
            .collect::<Vec<_>>();
        assert!(kinds.contains(&"member-changed"), "{kinds:?}");
        assert!(kinds.contains(&"trigger-missing"), "{kinds:?}");
        assert!(
            !kinds.contains(&"fingerprint-changed"),
            "the authored identity did not change: {kinds:?}"
        );
    }

    #[test]
    fn a_raw_registered_connection_cannot_bypass_the_gate_and_its_abort_rolls_back() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        let before = crate::load_card(&conn, &first_address(&root)).expect("member").text;
        // A connection that has the SQL wrappers but none of the application gate: the trigger is
        // the only thing standing between it and a locked authored body.
        let raw = Connection::open(conn.path().expect("database path")).expect("raw connection");
        register_sql_functions(&raw).expect("register functions");
        let tx = raw.unchecked_transaction().expect("raw transaction");
        let error = tx
            .execute(
                "UPDATE cards SET text = '# First\n\nrewritten\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
                [first_address(&root)],
            )
            .expect_err("trigger aborts the statement");
        assert!(error.to_string().contains("locked tree"), "{error}");
        drop(tx);
        assert_eq!(
            crate::load_card(&conn, &first_address(&root))
                .expect("member")
                .text,
            before,
            "the aborted statement changed nothing"
        );
        // A generated-only update keeps authored identity, so the same trigger lets it pass.
        let member = crate::load_card(&conn, &first_address(&root)).expect("member");
        let parsed = crate::parse_card_text(&member.text).expect("parsed");
        let regenerated = crate::compose_card_text(
            &parsed.title,
            &parsed.body,
            "## Reverse links\n\n- [[2/1]] Other",
        );
        raw.execute(
            "UPDATE cards SET text = ?1 WHERE location = ?2",
            params![regenerated, first_address(&root)],
        )
        .expect("a generated Reverse-link rewrite passes the trigger");
    }

    #[test]
    fn a_refused_mapping_writes_no_temporary_location() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        crate::insert_card(
            &mut conn,
            "2/0",
            true,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Other", "other root"),
        )
        .expect("other topic");
        crate::insert_card(
            &mut conn,
            "2/1",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Loose", "loose card"),
        )
        .expect("loose card");
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        let mapping = BTreeMap::from([("2/1".to_string(), format!("{}/2", crate::tree_id(&root)))]);
        let error = crate::apply_location_mapping_tx(&conn, &mapping).expect_err("move in");
        assert_eq!(
            error.downcast_ref::<Refusal>().expect("typed").kind,
            RefusalKind::LockedTree
        );
        let moving: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cards WHERE location LIKE '__moving__%'",
                [],
                |row| row.get(0),
            )
            .expect("moving count");
        assert_eq!(moving, 0, "no temporary Location was written");
        let still = crate::load_card(&conn, "2/1").expect("loose card keeps its address");
        assert_eq!(still.address, "2/1");
    }

    #[test]
    fn the_installed_triggers_name_the_shared_reverse_marker_and_use_no_custom_functions() {
        for (name, sql) in TRIGGER_DEFS {
            assert!(!sql.contains("zt_authored_fp"), "{name} uses only built-in SQL");
            assert!(!sql.contains("zt_tree_root"), "{name} uses only built-in SQL");
        }
        assert!(
            TRIGGER_DEFS[1].1.contains(crate::card::REVERSE_MARKER),
            "the update trigger compares the authored text before the generated Reverse marker"
        );
        let (_temp, mut conn) = archive();
        let installed: String = conn
            .query_row(
                "SELECT group_concat(sql, ' ') FROM sqlite_master WHERE type = 'trigger'",
                [],
                |row| row.get(0),
            )
            .expect("trigger catalog");
        assert!(installed.contains(crate::card::REVERSE_MARKER));
        let _ = topic_tree(&mut conn);
    }

    #[test]
    fn a_locked_topic_tree_does_not_confuse_a_neighbouring_tree_id() {
        let (_temp, mut conn) = archive();
        crate::insert_card(
            &mut conn,
            "3/0",
            true,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Three", "tree three"),
        )
        .expect("topic three");
        crate::insert_card(
            &mut conn,
            "3/1",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Member", "member"),
        )
        .expect("member");
        let plan = resolve_plan(&conn, "3/0", None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        let raw = Connection::open(conn.path().expect("database path")).expect("raw connection");
        raw.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type)
             VALUES(?1, NULL, 0, 0, NULL, ?2, 'human', 'idea')",
            params!["30/1", card_text("Neighbour", "neighbour")],
        )
        .expect("tree 30 shares only a prefix and stays writable");
        let blocked = raw.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type)
             VALUES(?1, NULL, 0, 0, NULL, ?2, 'human', 'idea')",
            params!["3/9", card_text("Blocked", "blocked")],
        );
        assert!(
            blocked
                .expect_err("the locked tree refuses the insert")
                .to_string()
                .contains("locked tree takes no new cards")
        );
    }

    #[test]
    fn a_plan_refuses_membership_in_a_second_workspace() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        // A raw write can leave a member in the other Workspace; the plan must refuse, not guess.
        conn.execute(
            "UPDATE cards SET workspace = 'agent' WHERE location = ?1",
            [first_address(&root)],
        )
        .expect("mixed workspace membership");
        let error = resolve_plan(&conn, &root, None).expect_err("inconsistent membership");
        assert!(
            error.to_string().contains("one Workspace"),
            "{error}"
        );
    }

    #[test]
    fn moving_an_ancestor_of_a_pinned_target_is_refused() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        crate::insert_card(
            &mut conn,
            "2/0",
            true,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Far", "far"),
        )
        .expect("far topic");
        crate::insert_card(
            &mut conn,
            "2/1",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Ancestor", "ancestor"),
        )
        .expect("ancestor");
        crate::insert_card(
            &mut conn,
            "2/1|a",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Target", "target"),
        )
        .expect("pinned target");
        let side = crate::load_card(&conn, &side_address(&root)).expect("side");
        crate::update_card_text(
            &mut conn,
            &side,
            &card_text("Side", &format!("links [[2/1|a]]")),
        )
        .expect("link the pinned child");
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        // The mapping moves the pinned Card's parent, so the target's own address changes even
        // though the owner selected the ancestor.
        let mapping = BTreeMap::from([
            ("2/1".to_string(), "3/1".to_string()),
            ("2/1|a".to_string(), "3/1|a".to_string()),
        ]);
        let error = ensure_unlocked(&conn, Change::Mapping(&mapping)).expect_err("ancestor move");
        let refusal = error.downcast_ref::<Refusal>().expect("typed refusal");
        assert_eq!(refusal.kind, RefusalKind::Pinned);
        assert_eq!(refusal.pinned_by[0].tree, plan.root);
        assert!(
            refusal.pinned_by[0]
                .sources
                .iter()
                .any(|source| source == &side.address)
        );
    }

    #[test]
    fn a_note_change_advances_the_revision_only_when_it_changes() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        let lock = lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        let before = crate::link_index::revision(&conn).expect("revision");
        let tx = conn.transaction().expect("transaction");
        note_tx(&tx, lock.lock_id, None).expect("same note");
        tx.commit().expect("commit");
        assert_eq!(crate::link_index::revision(&conn).expect("revision"), before);
        let tx = conn.transaction().expect("transaction");
        let noted = note_tx(&tx, lock.lock_id, Some("why")).expect("note");
        tx.commit().expect("commit");
        assert_eq!(noted.note.as_deref(), Some("why"));
        assert!(crate::link_index::revision(&conn).expect("revision") > before);
        assert_eq!(noted.seal, lock.seal, "a note never reseals");
    }

    /// A Topic tree whose first Card links `target`, so a Lock on the tree pins that Card.
    fn linking_tree(conn: &mut Connection, title: &str, target: &str) -> String {
        let root = crate::create_topic(conn, Workspace::Human, &card_text(title, "root body"))
            .expect("create topic");
        let first = format!("{}/1", crate::tree_id(&root));
        crate::insert_card(
            conn,
            &first,
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("First", &format!("links [[{target}]]")),
        )
        .expect("first card");
        root
    }

    /// A Topic tree whose Cards link nothing.
    fn plain_tree(conn: &mut Connection, title: &str) -> (String, String) {
        let root = crate::create_topic(conn, Workspace::Human, &card_text(title, "root body"))
            .expect("create topic");
        let first = format!("{}/1", crate::tree_id(&root));
        crate::insert_card(
            conn,
            &first,
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Card", "no link"),
        )
        .expect("card");
        (root, first)
    }

    fn lock_rows(conn: &mut Connection, root: &str) -> i64 {
        let tx = conn.transaction().expect("transaction");
        let plan = resolve_plan(&tx, root, None).expect("plan");
        let lock = lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        lock.lock_id
    }

    fn row_of(conn: &Connection, address: &str) -> i64 {
        conn.query_row(
            "SELECT row_id FROM cards WHERE COALESCE(location,citation_key)=?1",
            [address],
            |row| row.get(0),
        )
        .expect("row id")
    }

    fn annotated(view: &LockView, conn: &Connection, address: &str) -> Value {
        let mut value = json!({ "address": address });
        decorate_card(&mut value, row_of(conn, address), view);
        value
    }

    /// S03-AC01/S03-AC04: one bulk view annotates every projection, a member may also be pinned, and
    /// a target several members of one Locked tree link lists that root once.
    #[test]
    fn the_lock_view_annotates_members_and_pinned_targets_from_one_bulk_read() {
        let (_temp, mut conn) = archive();
        let (outside, target) = plain_tree(&mut conn, "Outside");
        let root = linking_tree(&mut conn, "Frozen", &target);
        let first = first_address(&root);
        let second = format!("{first}|a");
        crate::insert_card(
            &mut conn,
            &second,
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Second", "no link"),
        )
        .expect("second card");
        let lock_id = lock_rows(&mut conn, &root);

        let view = LockView::read(&conn, None).expect("bulk view");
        let locked = annotated(&view, &conn, &root);
        assert_eq!(locked["lock"]["lock_id"], json!(lock_id));
        assert_eq!(
            locked["lock"]["tree"],
            json!(root),
            "the annotation names the root address"
        );
        assert!(
            locked["lock"]["since"]
                .as_str()
                .is_some_and(|at| at.contains('T'))
        );
        assert_eq!(locked["pinned_by"], json!([]), "a member need not be pinned");
        assert_eq!(annotated(&view, &conn, &first)["lock"]["tree"], json!(root));
        assert_eq!(annotated(&view, &conn, &second)["lock"]["tree"], json!(root));

        // The pinned target: not a member, held by the Locked tree's root address, once.
        let pinned = annotated(&view, &conn, &target);
        assert!(pinned["lock"].is_null(), "a pinned target is not a member");
        assert_eq!(pinned["pinned_by"], json!([root]));

        // A Card of neither tree carries no annotation at all.
        let free = annotated(&view, &conn, &outside);
        assert!(free["lock"].is_null());
        assert_eq!(free["pinned_by"], json!([]));

        // A scoped read answers the same facts from the member-row and target-row indexes.
        let rows = vec![
            row_of(&conn, &second),
            row_of(&conn, &target),
            row_of(&conn, &outside),
        ];
        let scoped = LockView::read(&conn, Some(&rows)).expect("scoped view");
        assert_eq!(
            annotated(&scoped, &conn, &second),
            annotated(&view, &conn, &second)
        );
        assert_eq!(
            annotated(&scoped, &conn, &target),
            annotated(&view, &conn, &target)
        );
        assert_eq!(
            annotated(&scoped, &conn, &outside),
            annotated(&view, &conn, &outside)
        );
        assert!(
            !scoped.is_locked(row_of(&conn, &root)),
            "a scoped view holds only the selected rows"
        );

        // An isolated fixture's empty view annotates nothing.
        let empty = LockView::empty();
        assert!(annotated(&empty, &conn, &root)["lock"].is_null());
        assert_eq!(annotated(&empty, &conn, &target)["pinned_by"], json!([]));

        // `writable` is Workspace permission and no active Lock: a pin alone leaves editing open.
        assert!(!view.writable_for(row_of(&conn, &root), crate::Access::Owner, Workspace::Human));
        assert!(!view.writable_for(row_of(&conn, &root), crate::Access::Agent, Workspace::Human));
        assert!(view.writable_for(row_of(&conn, &target), crate::Access::Owner, Workspace::Human));
        assert!(!view.writable_for(row_of(&conn, &target), crate::Access::Agent, Workspace::Human));
    }

    /// S03-AC01: a target two Locked trees pin names both roots, in one sorted order.
    #[test]
    fn a_target_pinned_by_two_trees_names_both_roots_once() {
        let (_temp, mut conn) = archive();
        let (_outside, target) = plain_tree(&mut conn, "Outside");
        let first_tree = linking_tree(&mut conn, "First", &target);
        let second_tree = linking_tree(&mut conn, "Second", &target);
        // A second member of the first tree links the same target.
        let side = side_address(&first_tree);
        crate::insert_card(
            &mut conn,
            &side,
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Side", &format!("also [[{target}]]")),
        )
        .expect("side card");
        lock_rows(&mut conn, &first_tree);
        lock_rows(&mut conn, &second_tree);

        let view = LockView::read(&conn, None).expect("bulk view");
        let mut expected = vec![first_tree.clone(), second_tree.clone()];
        expected.sort();
        assert_eq!(
            annotated(&view, &conn, &target)["pinned_by"],
            json!(expected)
        );
        assert_eq!(annotated(&view, &conn, &side)["lock"]["tree"], json!(first_tree));
    }

    // -----------------------------------------------------------------------------------------
    // S06 (#129): exact integrity faults, fault latches and the guarded startup comparison.
    // -----------------------------------------------------------------------------------------

    fn fault_ids(integrity: &Integrity) -> Vec<String> {
        integrity.faults.iter().map(Fault::id).collect()
    }

    fn fault_kinds(integrity: &Integrity) -> BTreeSet<&str> {
        integrity
            .faults
            .iter()
            .map(|fault| fault.kind.as_str())
            .collect()
    }

    /// A locked tree, the state every S06 test starts from.
    fn locked_topic(conn: &mut Connection) -> (String, Lock) {
        let root = topic_tree(conn);
        let plan = resolve_plan(conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        let lock = lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        (root, lock)
    }

    /// R278/R287/S06-AC03: an intact archive passes the canonical catalog comparison, records
    /// nothing, and a pure scan writes nothing at all.
    #[test]
    fn an_intact_locked_archive_has_no_fault_and_records_nothing() {
        let (_temp, mut conn) = archive();
        let (root, lock) = locked_topic(&mut conn);
        let revision = crate::link_index::revision(&conn).expect("revision");
        let integrity = check(&conn).expect("check");
        assert!(integrity.ok(), "{:?}", fault_ids(&integrity));
        assert!(integrity.latched.is_empty());
        assert!(!integrity.needs_recording());
        assert_eq!(integrity.summary(), "integrity ok");
        let events: i64 = conn
            .query_row("SELECT COUNT(*) FROM lock_events", [], |row| row.get(0))
            .expect("events");
        assert_eq!(events, 1, "the pure scan wrote no event");
        assert_eq!(
            crate::link_index::revision(&conn).expect("revision"),
            revision,
            "the pure scan changed no revision"
        );
        let tx = conn.transaction().expect("transaction");
        assert_eq!(record_faults_tx(&tx, &integrity).expect("record"), 0);
        tx.commit().expect("commit");
        let panel = export_with(&conn, &integrity).expect("export");
        assert_eq!(panel["integrity"]["ok"], true);
        assert_eq!(panel["locks"][0]["seal_intact"], true);
        assert_eq!(panel["locks"][0]["faults"], json!([]));
        assert_eq!(panel["locks"][0]["lock_id"], json!(lock.lock_id));
        assert_eq!(panel["locks"][0]["root"], json!(root));
    }

    /// R287/S06-AC03: a trigger replaced under its canonical name is a different definition, so it
    /// is an exact fault even though the name is present.
    #[test]
    fn a_trigger_replaced_under_its_canonical_name_is_an_altered_fault() {
        let (_temp, mut conn) = archive();
        let (_root, lock) = locked_topic(&mut conn);
        conn.execute_batch(
            "DROP TRIGGER lock_no_change;
             CREATE TRIGGER lock_no_change BEFORE UPDATE ON cards
             WHEN 0 BEGIN SELECT RAISE(ABORT, 'zt: card is in a locked tree'); END;",
        )
        .expect("replace the trigger");
        let integrity = check(&conn).expect("check");
        let kinds = fault_kinds(&integrity);
        assert!(kinds.contains("trigger-altered"), "{kinds:?}");
        assert!(!kinds.contains("trigger-missing"), "{kinds:?}");
        let fault = integrity
            .faults
            .iter()
            .find(|fault| fault.kind == "trigger-altered")
            .expect("altered fault");
        assert_eq!(fault.subject, "lock_no_change");
        assert!(fault.lock_id.is_none(), "a schema fault names no Lock");
        assert_eq!(fault.value()["expected"], "the canonical trigger");
        assert_eq!(fault.value()["actual"], "an altered definition");
        let tx = conn.transaction().expect("transaction");
        assert_eq!(record_faults_tx(&tx, &integrity).expect("record"), 1);
        tx.commit().expect("commit");
        // R296/R297: the guarded startup comparison records first, then repairs, and the latch
        // keeps the seal broken after the definition is canonical again.
        let guard = startup_guard(&conn).expect("guarded startup");
        assert_eq!(guard.recorded, 0, "the fault was already recorded");
        assert_eq!(guard.repaired, vec!["lock_no_change".to_string()]);
        assert!(
            check(&conn).expect("check").faults.is_empty(),
            "the canonical trigger is back"
        );
        let after = check(&conn).expect("check");
        assert!(!after.ok(), "the latch survives the repair");
        assert_eq!(after.latched.len(), 1);
        assert_eq!(after.latched[0].lock_id, lock.lock_id);
        assert_eq!(after.latched[0].kind, "trigger-altered");
        // A second guarded startup adds no event and repairs nothing.
        let again = startup_guard(&conn).expect("guarded startup again");
        assert_eq!(again.recorded, 0);
        assert!(again.repaired.is_empty(), "{:?}", again.repaired);
    }

    /// R290/R296: a trigger lost before startup is recorded *before* the canonical one is
    /// reinstalled, so its evidence is not lost by the repair.
    #[test]
    fn a_trigger_lost_before_startup_is_recorded_before_it_is_reinstalled() {
        let (_temp, mut conn) = archive();
        let (root, lock) = locked_topic(&mut conn);
        conn.execute_batch("DROP TRIGGER lock_pin_no_delete")
            .expect("drop trigger");
        let guard = startup_guard(&conn).expect("guarded startup");
        assert_eq!(guard.recorded, 1, "the loss is evidence before the repair");
        assert_eq!(guard.repaired, vec!["lock_pin_no_delete".to_string()]);
        assert!(
            guard.integrity.faults.is_empty(),
            "the repair restored the backstop: {:?}",
            fault_ids(&guard.integrity)
        );
        assert_eq!(guard.integrity.latched.len(), 1);
        let recorded: String = conn
            .query_row(
                "SELECT detail FROM lock_events WHERE action = 'integrity-fault' AND lock_id = ?1",
                [lock.lock_id],
                |row| row.get(0),
            )
            .expect("the recorded fault");
        assert!(recorded.contains("trigger-missing"), "{recorded}");
        assert!(recorded.contains("lock_pin_no_delete"), "{recorded}");
        // The repaired trigger really protects again.
        let error = conn
            .execute("DELETE FROM cards WHERE location = ?1", [&root])
            .expect_err("the reinstall protects the root again");
        assert!(error.to_string().contains("locked tree"), "{error}");
    }

    /// R287/S06-AC03: a required index that disappears is a fault, not a silent success; the same
    /// guarded startup restores it and keeps the latch.
    #[test]
    fn a_missing_index_is_a_fault_and_the_guarded_startup_restores_it() {
        let (_temp, mut conn) = archive();
        let (_root, lock) = locked_topic(&mut conn);
        conn.execute_batch("DROP INDEX lock_pins_by_target")
            .expect("drop index");
        let integrity = check(&conn).expect("check");
        let kinds = fault_kinds(&integrity);
        assert!(kinds.contains("schema-index"), "{kinds:?}");
        let guard = startup_guard(&conn).expect("guarded startup");
        assert_eq!(guard.recorded, 1);
        assert_eq!(guard.repaired, vec!["lock_pins_by_target".to_string()]);
        assert_eq!(
            lock_events_with(&conn, lock.lock_id, "integrity-fault"),
            1,
            "one event for the lost index"
        );
        let restored: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'lock_pins_by_target'",
                [],
                |row| row.get(0),
            )
            .expect("the restored index");
        assert!(restored.contains("lock_pins(target_row)"), "{restored}");
        assert!(!check(&conn).expect("check").ok(), "the latch stays");
    }

    fn lock_events_with(conn: &Connection, lock_id: i64, action: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM lock_events WHERE lock_id = ?1 AND action = ?2",
            params![lock_id, action],
            |row| row.get(0),
        )
        .expect("event count")
    }

    /// R287/S06-AC03: every one of the six canonical triggers is compared by definition, not only
    /// one of them, and the guarded startup restores each exact statement.
    #[test]
    fn all_six_canonical_triggers_are_compared_and_restored() {
        for (name, _) in TRIGGER_DEFS {
            let (_temp, mut conn) = archive();
            let (_root, lock) = locked_topic(&mut conn);
            conn.execute_batch(&format!(
                "DROP TRIGGER {name};
                 CREATE TRIGGER {name} BEFORE DELETE ON cards WHEN 0
                 BEGIN SELECT RAISE(ABORT, 'zt: card is in a locked tree'); END;"
            ))
            .expect("replace one trigger");
            let integrity = check(&conn).expect("check");
            let altered = integrity
                .faults
                .iter()
                .filter(|fault| fault.kind == "trigger-altered")
                .map(|fault| fault.subject.as_str())
                .collect::<Vec<_>>();
            assert_eq!(altered, vec![name], "exactly `{name}` is altered");
            let guard = startup_guard(&conn).expect("guarded startup");
            assert_eq!(guard.recorded, 1, "{name}");
            assert_eq!(guard.repaired, vec![name.to_string()]);
            assert!(
                validate_triggers(&conn).is_ok(),
                "{name} must be canonical again"
            );
            assert!(!check(&conn).expect("check").ok(), "{name}: the latch stays");
            assert_eq!(lock.lock_id, lock.lock_id);
        }
    }

    /// R295/S06-AC04: recording a newly observed fault advances the archive revision once, and a
    /// repeated recording of the same evidence advances nothing.
    #[test]
    fn recording_a_new_fault_advances_the_revision_once() {
        let (_temp, mut conn) = archive();
        let (_root, _lock) = locked_topic(&mut conn);
        conn.execute_batch("DROP TRIGGER lock_pin_no_move")
            .expect("drop a trigger");
        let integrity = check(&conn).expect("check");
        let before = crate::link_index::revision(&conn).expect("revision");
        let tx = conn.transaction().expect("transaction");
        assert_eq!(record_faults_tx(&tx, &integrity).expect("record"), 1);
        tx.commit().expect("commit");
        let after = crate::link_index::revision(&conn).expect("revision");
        assert_eq!(after, before + 1, "one recording, one revision");
        let tx = conn.transaction().expect("transaction");
        assert_eq!(record_faults_tx(&tx, &integrity).expect("record again"), 0);
        tx.commit().expect("commit");
        assert_eq!(
            crate::link_index::revision(&conn).expect("revision"),
            after,
            "repeated evidence changes nothing"
        );
    }

    /// R293/S06-AC06: the comparison a migration runs first records the promise it could break, and
    /// is a no-op when no promise exists.
    #[test]
    fn check_before_migration_records_only_the_promises_a_migration_could_break() {
        let (_temp, mut conn) = archive();
        // No lock schema: nothing to compare.
        assert_eq!(check_before_migration(&conn).expect("no schema"), 0);
        // A lock schema but no active Lock: no promise to compare.
        let root = topic_tree(&mut conn);
        let _ = &root;
        assert_eq!(check_before_migration(&conn).expect("no lock"), 0);
        // An active Lock with a tampered member: the migration check records the evidence.
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        let lock = lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        conn.execute_batch("DROP TRIGGER lock_no_change")
            .expect("drop the trigger");
        conn.execute(
            "UPDATE cards SET text = '# Topic\n\nchanged\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
            [&root],
        )
        .expect("tamper");
        assert_eq!(check_before_migration(&conn).expect("recorded"), 2);
        assert_eq!(lock_events_with(&conn, lock.lock_id, "integrity-fault"), 2);
        assert_eq!(check_before_migration(&conn).expect("again"), 0);
    }

    /// S06-AC06/R290/R293: the boot sequence keeps each check where the contract puts it — the
    /// promise comparison before a Card migration, the guarded comparison after the Card rebuilds,
    /// and the two explicit check points reachable from the shell and the panel.
    #[test]
    fn the_boot_sequence_keeps_each_integrity_check_where_it_belongs() {
        let source = include_str!("main.rs");
        let boot = source
            .split("fn initialize_database")
            .nth(1)
            .expect("initialize_database");
        let boot = &boot[..boot.find("fn cards_table").unwrap_or(boot.len())];
        let before_migration = boot
            .find("tree_lock::check_before_migration")
            .expect("the pre-migration comparison");
        let first_migration = boot
            .find("ensure_workspace_column(&conn, root)")
            .expect("the first Card migration");
        let rebuild = boot
            .find("ensure_markdown_cards(root)")
            .expect("the Card rebuild");
        let guarded = boot
            .find("tree_lock::startup_guard")
            .expect("the guarded startup comparison");
        assert!(
            before_migration < first_migration,
            "the promise is compared before a migration can change Cards"
        );
        assert!(
            rebuild < guarded,
            "the guarded comparison sees the protection catalog after the Card rebuilds"
        );
        assert!(
            source.contains("cmd == \"status\" && flag == \"--check-locks\""),
            "`zt status --check-locks` reaches the full comparison"
        );
        let gui = include_str!("gui.rs");
        let panel = gui
            .split("fn locks(&self)")
            .nth(1)
            .expect("the panel route");
        assert!(
            panel.contains("crate::tree_lock::check(&conn)?"),
            "opening the panel runs the comparison"
        );
        assert!(
            panel.contains("record_faults_tx"),
            "the panel records newly observed faults once"
        );
    }

    /// R288/S06-AC02: the comparison reads raw stored values, so a decoder fallback for a value
    /// the schema does not accept cannot hide a changed Workspace or kind.
    #[test]
    fn a_raw_undecodable_workspace_or_kind_is_a_fault_not_a_default() {
        let (_temp, mut conn) = archive();
        let (root, lock) = locked_topic(&mut conn);
        let first = first_address(&root);
        // The CHECK constraints refuse these values through ordinary SQL and the statement trigger
        // refuses any member change at all; the fixture allows both to prove the comparison reads the
        // raw stored values instead of falling back to `human` or an Idea Card.
        conn.execute_batch("PRAGMA ignore_check_constraints = ON; DROP TRIGGER lock_no_change;")
            .expect("allow the fixture");
        conn.execute(
            "UPDATE cards SET workspace = 'elsewhere' WHERE location = ?1",
            [&first],
        )
        .expect("tamper workspace");
        conn.execute(
            "UPDATE cards SET is_lit = 2 WHERE location = ?1",
            [&first],
        )
        .expect("tamper kind");
        conn.execute_batch("PRAGMA ignore_check_constraints = OFF;")
            .expect("restore the schema");
        let integrity = check(&conn).expect("check");
        let changed = integrity
            .faults
            .iter()
            .filter(|fault| fault.kind == "member-changed" && fault.subject == first)
            .collect::<Vec<_>>();
        assert!(
            changed
                .iter()
                .any(|fault| fault.actual.as_deref() == Some("elsewhere")),
            "{:?}",
            fault_ids(&integrity)
        );
        assert!(
            changed
                .iter()
                .any(|fault| fault.actual.as_deref() == Some("is_topic=0 is_lit=2")),
            "{:?}",
            fault_ids(&integrity)
        );
        assert_eq!(changed[0].lock_id, Some(lock.lock_id));
    }

    /// R283/R281/S06-AC01: an extra Card, a removed member and a member that moved out are three
    /// distinct exact faults, each naming its own address.
    #[test]
    fn extra_missing_and_moved_members_are_distinct_faults() {
        let (_temp, mut conn) = archive();
        let (root, _lock) = locked_topic(&mut conn);
        let side = side_address(&root);
        // An extra Card inside the locked tree, a removed member and a moved member, written by a
        // raw connection: every protection trigger that stops them is dropped first, which is its
        // own fault and the honest shape of an outside tamper.
        conn.execute_batch(
            "DROP TRIGGER lock_no_insert; DROP TRIGGER lock_no_delete; DROP TRIGGER lock_no_change;",
        )
        .expect("drop the protection triggers");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type)
             VALUES(?1, NULL, 0, 0, NULL, ?2, 'human', 'idea')",
            params![
                format!("{}/9", crate::tree_id(&root)),
                card_text("Extra", "not in the snapshot")
            ],
        )
        .expect("extra card");
        // A removed member and a member moved out of the tree.
        conn.execute("DELETE FROM cards WHERE location = ?1", [&side])
            .expect("delete a member");
        let first = first_address(&root);
        conn.execute(
            "UPDATE cards SET location = '7/1' WHERE location = ?1",
            [&first],
        )
        .expect("move a member out");
        let integrity = check(&conn).expect("check");
        let kinds = fault_kinds(&integrity);
        assert!(kinds.contains("member-extra"), "{kinds:?}");
        assert!(kinds.contains("member-missing"), "{kinds:?}");
        let extra = integrity
            .faults
            .iter()
            .find(|fault| fault.kind == "member-extra")
            .expect("extra fault");
        assert_eq!(extra.subject, format!("{}/9", crate::tree_id(&root)));
        let missing = integrity
            .faults
            .iter()
            .find(|fault| fault.kind == "member-missing")
            .expect("missing fault");
        assert_eq!(missing.subject, side);
        // A moved member is one exact fault at its old address: the snapshot row no longer holds
        // the Card it promised.
        let moved = integrity
            .faults
            .iter()
            .find(|fault| fault.subject == first)
            .unwrap_or_else(|| panic!("no fault names the moved member: {:?}", fault_ids(&integrity)));
        assert_eq!(moved.kind, "member-changed");
        assert_eq!(moved.actual.as_deref(), Some("7/1"));
    }

    /// R285/R286/S06-AC02: a pin whose target vanished or changed its row, a pin inside the tree,
    /// a pin source that is not a member, and a pin set that disagrees with the authored Links are
    /// each their own fault.
    #[test]
    fn every_pin_and_link_disagreement_is_its_own_fault() {
        let (_temp, mut conn) = archive();
        let (_outside, target) = plain_tree(&mut conn, "Outside");
        let root = linking_tree(&mut conn, "Frozen", &target);
        let lock_id = lock_rows(&mut conn, &root);
        // The pin target is deleted by a raw connection; the pin delete trigger refuses it, so the
        // fixture drops that trigger (itself a fault).
        conn.execute_batch("DROP TRIGGER lock_pin_no_delete")
            .expect("drop the pin trigger");
        conn.execute("DELETE FROM cards WHERE location = ?1", [&target])
            .expect("delete the pinned target");
        let integrity = check(&conn).expect("check");
        let kinds = fault_kinds(&integrity);
        assert!(kinds.contains("pin-missing"), "{kinds:?}");
        assert!(kinds.contains("pin-set"), "{kinds:?}");
        let pin_missing = integrity
            .faults
            .iter()
            .find(|fault| fault.kind == "pin-missing")
            .expect("pin-missing");
        assert_eq!(pin_missing.subject, target);
        assert_eq!(pin_missing.lock_id, Some(lock_id));
        // The pin set fault names the source member whose authored Link no longer has a target.
        let pin_set = integrity
            .faults
            .iter()
            .find(|fault| fault.kind == "pin-set")
            .expect("pin-set");
        assert_eq!(pin_set.subject, first_address(&root));

        // A pin source that is not one of the Lock's members, and a pin inside the locked tree.
        conn.execute(
            "INSERT INTO lock_pins(lock_id, source_row, target_row, target_address)
             VALUES(?1, 999999, 999998, 'nowhere')",
            [lock_id],
        )
        .expect("foreign pin source");
        let inside = first_address(&root);
        let inside_row = row_of(&conn, &inside);
        conn.execute(
            "INSERT INTO lock_pins(lock_id, source_row, target_row, target_address)
             VALUES(?1, ?2, ?2, ?3)",
            params![lock_id, inside_row, inside],
        )
        .expect("pin inside the tree");
        let integrity = check(&conn).expect("check");
        let kinds = fault_kinds(&integrity);
        assert!(kinds.contains("pin-source"), "{kinds:?}");
        assert!(kinds.contains("pin-inside"), "{kinds:?}");
    }

    /// R286/S06-AC02: the derived Link index is cross-checked against the authored body, so a
    /// dropped index row is a fault even when the pins still match.
    #[test]
    fn a_link_index_that_disagrees_with_the_authored_body_is_a_fault() {
        let (_temp, mut conn) = archive();
        let (_outside, target) = plain_tree(&mut conn, "Outside");
        let root = linking_tree(&mut conn, "Frozen", &target);
        let first = first_address(&root);
        lock_rows(&mut conn, &root);
        conn.execute("DELETE FROM links WHERE source = ?1", [&first])
            .expect("drop one indexed source");
        let integrity = check(&conn).expect("check");
        let fault = integrity
            .faults
            .iter()
            .find(|fault| fault.kind == "index-changed")
            .expect("index fault");
        assert_eq!(fault.subject, first);
        assert!(fault.expected.as_deref().is_some_and(|text| text.contains(&target)));
        assert_eq!(fault.actual.as_deref(), Some(""));
    }

    /// R289/S06-AC05: a recorded fault is never erased by a later clean comparison, so the archive
    /// needs the owner's explicit review; the pure scan does not re-record it.
    #[test]
    fn a_later_clean_comparison_does_not_erase_a_latched_fault() {
        let (_temp, mut conn) = archive();
        let (root, lock) = locked_topic(&mut conn);
        let first = first_address(&root);
        let original = crate::load_card(&conn, &first).expect("member").text;
        conn.execute_batch("DROP TRIGGER lock_no_change")
            .expect("drop the update trigger");
        conn.execute(
            "UPDATE cards SET text = '# First\n\ntampered\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
            [&first],
        )
        .expect("tamper");
        let integrity = check(&conn).expect("check");
        let tx = conn.transaction().expect("transaction");
        assert_eq!(
            record_faults_tx(&tx, &integrity).expect("record"),
            2,
            "the member fault and the lost trigger"
        );
        tx.commit().expect("commit");
        // The owner restores the authored text by hand and the next service start restores the
        // trigger: the current comparison is clean, the evidence is not.
        conn.execute("UPDATE cards SET text = ?1 WHERE location = ?2", params![original, first])
            .expect("restore");
        let guard = startup_guard(&conn).expect("guarded startup");
        assert_eq!(guard.recorded, 0);
        assert_eq!(guard.repaired, vec!["lock_no_change".to_string()]);
        let after = check(&conn).expect("check");
        assert!(after.faults.is_empty(), "{:?}", fault_ids(&after));
        assert!(!after.ok(), "the latch keeps the Lock unhealthy");
        assert_eq!(after.latched.len(), 2);
        assert_eq!(lock_events_with(&conn, lock.lock_id, "integrity-fault"), 2);
        // The panel shows the broken seal beside the same seal the owner reviewed.
        let panel = export_with(&conn, &after).expect("export");
        let listed = &panel["locks"][0];
        assert_eq!(listed["seal"], json!(lock.seal));
        assert_eq!(listed["seal_intact"], false);
        assert_eq!(listed["latched_faults"].as_array().expect("latched").len(), 2);
    }

    /// R278/R027: the comparison uses the caller's transaction when there is one, so a tamper
    /// inside an uncommitted transaction is visible and a rollback brings the clean answer back.
    #[test]
    fn check_reads_the_callers_transaction_and_writes_nothing() {
        let (_temp, mut conn) = archive();
        let (_root, _lock) = locked_topic(&mut conn);
        let tx = conn.transaction().expect("transaction");
        tx.execute_batch("DROP TRIGGER lock_no_delete")
            .expect("drop inside the transaction");
        let inside = check(&tx).expect("check inside the transaction");
        assert!(fault_kinds(&inside).contains("trigger-missing"));
        let events: i64 = tx
            .query_row("SELECT COUNT(*) FROM lock_events", [], |row| row.get(0))
            .expect("events");
        assert_eq!(events, 1, "a pure check records nothing, even inside a transaction");
        tx.rollback().expect("rollback");
        assert!(check(&conn).expect("check after rollback").faults.is_empty());
    }
}

impl Refusal {
    #[cfg(test)]
    fn code_string(&self) -> &'static str {
        self.kind.code()
    }
}
