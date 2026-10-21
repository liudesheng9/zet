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

/// The six canonical trigger names, in the order the design lists them.
pub(crate) const TRIGGER_NAMES: [&str; 6] = [
    "lock_no_delete",
    "lock_no_change",
    "lock_no_insert",
    "lock_no_move_in",
    "lock_pin_no_delete",
    "lock_pin_no_move",
];

const SCHEMA_SQL: &str = "
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
CREATE UNIQUE INDEX IF NOT EXISTS locks_active_tree
    ON locks(root_address) WHERE unlocked_at IS NULL;
CREATE TABLE IF NOT EXISTS lock_members (
    lock_id     INTEGER NOT NULL REFERENCES locks(lock_id),
    row_id      INTEGER NOT NULL,
    address     TEXT NOT NULL,
    authored_fp TEXT NOT NULL,
    PRIMARY KEY(lock_id, row_id)
);
CREATE INDEX IF NOT EXISTS lock_members_by_row ON lock_members(row_id);
CREATE TABLE IF NOT EXISTS lock_pins (
    lock_id        INTEGER NOT NULL REFERENCES locks(lock_id),
    source_row     INTEGER NOT NULL,
    target_row     INTEGER NOT NULL,
    target_address TEXT NOT NULL,
    PRIMARY KEY(lock_id, source_row, target_row)
);
CREATE INDEX IF NOT EXISTS lock_pins_by_target ON lock_pins(target_row);
CREATE TABLE IF NOT EXISTS lock_events (
    event_id INTEGER PRIMARY KEY,
    lock_id  INTEGER NOT NULL REFERENCES locks(lock_id),
    action   TEXT NOT NULL
        CHECK(action IN ('lock','unlock','note','reseal','integrity-fault')),
    at       TEXT NOT NULL,
    detail   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS lock_events_by_lock ON lock_events(lock_id, event_id);
";

/// Every trigger joins `locks` with `unlocked_at IS NULL`. Insertion and move-in compare the
/// destination's Tree id with the active `locks.tree`, which is the same canonical value as
/// `tree_root_address(tree_id(address))` without a function call. Update protection compares the
/// authored bytes before the generated Reverse marker; a Reverse-only refresh keeps them identical.
/// The predicates use only built-in SQL so that an external connection can still write an archive
/// that holds no Lock (T11 keeps the function-less connection as an unsupported writer of a locked
/// body, not of every Card).
const TRIGGERS_SQL: &str = "
CREATE TRIGGER IF NOT EXISTS lock_no_delete BEFORE DELETE ON cards
WHEN EXISTS (
    SELECT 1 FROM lock_members m JOIN locks l ON l.lock_id = m.lock_id
    WHERE m.row_id = OLD.row_id AND l.unlocked_at IS NULL
)
BEGIN SELECT RAISE(ABORT, 'zt: card is in a locked tree'); END;

CREATE TRIGGER IF NOT EXISTS lock_no_change BEFORE UPDATE ON cards
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
BEGIN SELECT RAISE(ABORT, 'zt: card is in a locked tree'); END;

CREATE TRIGGER IF NOT EXISTS lock_no_insert BEFORE INSERT ON cards
WHEN EXISTS (
    SELECT 1 FROM locks l WHERE l.unlocked_at IS NULL
      AND l.tree = CASE WHEN instr(COALESCE(NEW.location, NEW.citation_key), '/') = 0
                        THEN COALESCE(NEW.location, NEW.citation_key)
                        ELSE substr(COALESCE(NEW.location, NEW.citation_key), 1,
                                    instr(COALESCE(NEW.location, NEW.citation_key), '/') - 1)
                   END
)
BEGIN SELECT RAISE(ABORT, 'zt: locked tree takes no new cards'); END;

CREATE TRIGGER IF NOT EXISTS lock_no_move_in BEFORE UPDATE OF location, citation_key ON cards
WHEN (NEW.location IS NOT OLD.location OR NEW.citation_key IS NOT OLD.citation_key)
 AND EXISTS (
    SELECT 1 FROM locks l WHERE l.unlocked_at IS NULL
      AND l.tree = CASE WHEN instr(COALESCE(NEW.location, NEW.citation_key), '/') = 0
                        THEN COALESCE(NEW.location, NEW.citation_key)
                        ELSE substr(COALESCE(NEW.location, NEW.citation_key), 1,
                                    instr(COALESCE(NEW.location, NEW.citation_key), '/') - 1)
                   END
)
BEGIN SELECT RAISE(ABORT, 'zt: locked tree takes no new cards'); END;

CREATE TRIGGER IF NOT EXISTS lock_pin_no_delete BEFORE DELETE ON cards
WHEN EXISTS (
    SELECT 1 FROM lock_pins p JOIN locks l ON l.lock_id = p.lock_id
    WHERE p.target_row = OLD.row_id AND l.unlocked_at IS NULL
)
BEGIN SELECT RAISE(ABORT, 'zt: card is a pinned target of a locked tree'); END;

CREATE TRIGGER IF NOT EXISTS lock_pin_no_move BEFORE UPDATE ON cards
WHEN (NEW.location IS NOT OLD.location OR NEW.citation_key IS NOT OLD.citation_key
      OR NEW.row_id IS NOT OLD.row_id OR NEW.workspace IS NOT OLD.workspace
      OR NEW.is_topic IS NOT OLD.is_topic OR NEW.is_lit IS NOT OLD.is_lit)
 AND EXISTS (
    SELECT 1 FROM lock_pins p JOIN locks l ON l.lock_id = p.lock_id
    WHERE p.target_row = OLD.row_id AND l.unlocked_at IS NULL
)
BEGIN SELECT RAISE(ABORT, 'zt: card is a pinned target of a locked tree'); END;
";

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

/// Install or validate schema version 1 and its triggers. Called once at service startup, after the
/// Card-table rebuilds, so it never fights a migration. An unsupported generation refuses to write
/// rather than attempting a repair.
pub(crate) fn ensure_tx(conn: &Connection) -> Result<()> {
    match schema_version(conn)? {
        None => {
            conn.execute_batch(SCHEMA_SQL)
                .context("failed to install the Tree Lock tables")?;
            validate_schema(conn)?;
            conn.execute_batch(TRIGGERS_SQL)
                .context("failed to install the Tree Lock triggers")?;
            validate_triggers(conn)?;
            conn.execute(
                "INSERT INTO metadata(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![SCHEMA_KEY, SCHEMA_VERSION],
            )?;
        }
        Some(version) if version == SCHEMA_VERSION => {
            validate_schema(conn)?;
            conn.execute_batch(TRIGGERS_SQL)
                .context("failed to install the Tree Lock triggers")?;
            validate_triggers(conn)?;
        }
        Some(other) => bail!(
            "unsupported Tree Lock schema version `{other}`; archive was not modified"
        ),
    }
    Ok(())
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(columns)
}

fn validate_schema(conn: &Connection) -> Result<()> {
    let expected: [(&str, &[&str]); 4] = [
        (
            "locks",
            &[
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
            ],
        ),
        (
            "lock_members",
            &["lock_id", "row_id", "address", "authored_fp"],
        ),
        (
            "lock_pins",
            &["lock_id", "source_row", "target_row", "target_address"],
        ),
        (
            "lock_events",
            &["event_id", "lock_id", "action", "at", "detail"],
        ),
    ];
    for (table, columns) in expected {
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

fn trigger_exists(conn: &Connection, name: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'trigger' AND name = ?1)",
        [name],
        |row| row.get(0),
    )?)
}

fn validate_triggers(conn: &Connection) -> Result<()> {
    for name in TRIGGER_NAMES {
        if !trigger_exists(conn, name)? {
            bail!("Tree Lock trigger `{name}` is missing; archive was not modified");
        }
    }
    Ok(())
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
        if !self.warnings.is_empty() {
            lines.push(format!(
                "already broken links: {} (they create no pin; relock after they resolve)",
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
            pins.entry(target_card.address.clone())
                .or_insert_with(|| Pin {
                    target: target_card.address.clone(),
                    target_row: target_card.row_id,
                    sources: Vec::new(),
                })
                .sources
                .push((card.row_id, card.address.clone()));
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
                let address = state
                    .member_by_address
                    .iter()
                    .find(|(_, (id, _))| id == lock_id)
                    .map(|(address, _)| address.clone())
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
// Integrity
// ---------------------------------------------------------------------------------------------

/// One observed integrity fault with a stable identifier, so a repeated scan adds no event.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct Fault {
    pub(crate) id: String,
    pub(crate) lock_id: Option<i64>,
    pub(crate) kind: String,
    pub(crate) card: Option<String>,
    pub(crate) detail: String,
}

impl Fault {
    fn new(lock_id: Option<i64>, kind: &str, subject: &str, detail: impl Into<String>) -> Self {
        Self {
            id: format!(
                "lock-{}:{kind}:{subject}",
                lock_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "schema".to_string())
            ),
            lock_id,
            kind: kind.to_string(),
            card: Some(subject.to_string()),
            detail: detail.into(),
        }
    }

    pub(crate) fn value(&self) -> Value {
        json!({
            "id": self.id,
            "lock_id": self.lock_id,
            "kind": self.kind,
            "card": self.card,
            "detail": self.detail,
        })
    }
}

/// The pure result of one integrity comparison. It repairs nothing and never reseals.
#[derive(Clone, Debug, Default)]
pub(crate) struct Integrity {
    pub(crate) faults: Vec<Fault>,
}

impl Integrity {
    pub(crate) fn ok(&self) -> bool {
        self.faults.is_empty()
    }

    pub(crate) fn value(&self) -> Value {
        json!({
            "ok": self.ok(),
            "faults": self.faults.iter().map(Fault::value).collect::<Vec<_>>(),
        })
    }
}

/// Compare every active Lock's promise with the stored Cards and the lock tables.
pub(crate) fn check(conn: &Connection) -> Result<Integrity> {
    let mut integrity = Integrity::default();
    let Some(state) = load_state(conn)? else {
        integrity.faults.push(Fault::new(
            None,
            "schema",
            "tree_lock",
            "the Tree Lock schema is not installed",
        ));
        return Ok(integrity);
    };
    for name in TRIGGER_NAMES {
        if !trigger_exists(conn, name)? {
            integrity.faults.push(Fault::new(
                None,
                "trigger-missing",
                name,
                "the canonical trigger is missing, so the SQL backstop is incomplete",
            ));
        }
    }
    let cards = crate::load_cards(conn)?;
    let by_address = cards
        .iter()
        .map(|card| (card.address.clone(), card))
        .collect::<BTreeMap<_, _>>();
    let by_row = cards
        .iter()
        .map(|card| (card.row_id, card))
        .collect::<BTreeMap<_, _>>();
    for lock in state.locks.values().filter(|lock| lock.unlocked_at.is_none()) {
        match by_address.get(&lock.root) {
            None => integrity.faults.push(Fault::new(
                Some(lock.lock_id),
                "root-missing",
                &lock.root,
                "the locked Tree root no longer exists at its saved address",
            )),
            Some(card) => {
                if card.workspace != lock.workspace {
                    integrity.faults.push(Fault::new(
                        Some(lock.lock_id),
                        "root-changed",
                        &lock.root,
                        "the locked Tree root changed its Workspace",
                    ));
                }
                if crate::tree_root_address(crate::tree_id(&card.address)) != lock.root
                    || !crate::is_tree_root(card)
                {
                    integrity.faults.push(Fault::new(
                        Some(lock.lock_id),
                        "root-changed",
                        &lock.root,
                        "the saved root address is no longer a Tree root",
                    ));
                }
            }
        }
        let mut members = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT row_id, address, authored_fp FROM lock_members WHERE lock_id = ?1 ORDER BY address",
        )?;
        for row in stmt.query_map([lock.lock_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })? {
            members.push(row?);
        }
        if members.len() as i64 != lock.member_count {
            integrity.faults.push(Fault::new(
                Some(lock.lock_id),
                "member-count",
                &lock.root,
                format!(
                    "stored member count is {} but {} snapshot rows exist",
                    lock.member_count,
                    members.len()
                ),
            ));
        }
        let mut baseline = BTreeMap::new();
        for (row_id, address, fingerprint) in &members {
            baseline.insert(address.clone(), fingerprint.clone());
            match by_row.get(row_id) {
                None => integrity.faults.push(Fault::new(
                    Some(lock.lock_id),
                    "member-missing",
                    address,
                    format!("snapshot row {row_id} no longer exists"),
                )),
                Some(card) => {
                    if &card.address != address {
                        integrity.faults.push(Fault::new(
                            Some(lock.lock_id),
                            "member-changed",
                            address,
                            format!("snapshot row {row_id} now holds `{}`", card.address),
                        ));
                        continue;
                    }
                    if card.workspace != lock.workspace {
                        integrity.faults.push(Fault::new(
                            Some(lock.lock_id),
                            "member-changed",
                            address,
                            "the member changed its Workspace",
                        ));
                    }
                    if address == &lock.root
                        && (!crate::is_tree_root(card)
                            || crate::tree_root_address(crate::tree_id(address)) != lock.root)
                    {
                        integrity.faults.push(Fault::new(
                            Some(lock.lock_id),
                            "member-changed",
                            address,
                            "the root member is no longer a Tree root",
                        ));
                    }
                    let current = crate::card::authored_fingerprint(card)?;
                    if &current != fingerprint {
                        integrity.faults.push(Fault::new(
                            Some(lock.lock_id),
                            "fingerprint-changed",
                            address,
                            "the member's authored identity changed",
                        ));
                    }
                }
            }
        }
        if seal_of(&baseline) != lock.seal {
            integrity.faults.push(Fault::new(
                Some(lock.lock_id),
                "seal-changed",
                &lock.root,
                "the stored snapshot no longer matches the Lock seal",
            ));
        }
        let snapshot = baseline.keys().cloned().collect::<BTreeSet<_>>();
        let current = cards
            .iter()
            .filter(|card| crate::is_tree_member(&card.address, &lock.tree))
            .map(|card| card.address.clone())
            .collect::<BTreeSet<_>>();
        for address in current.difference(&snapshot) {
            integrity.faults.push(Fault::new(
                Some(lock.lock_id),
                "member-extra",
                address,
                "a Card is inside the locked tree but not in its snapshot",
            ));
        }
        for address in snapshot.difference(&current) {
            // A missing Card is reported once, by its snapshot row, above.
            if by_address.contains_key(address) {
                integrity.faults.push(Fault::new(
                    Some(lock.lock_id),
                    "member-moved",
                    address,
                    "the member no longer belongs to the locked tree",
                ));
            }
        }
        let mut pins = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT p.source_row, p.target_row, p.target_address FROM lock_pins p WHERE p.lock_id = ?1",
        )?;
        for row in stmt.query_map([lock.lock_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })? {
            pins.push(row?);
        }
        for (source_row, target_row, target_address) in &pins {
            if !state.member_by_row.contains_key(source_row) {
                integrity.faults.push(Fault::new(
                    Some(lock.lock_id),
                    "pin-source",
                    target_address,
                    format!("pin source row {source_row} is not a member"),
                ));
            }
            match by_row.get(target_row) {
                None => integrity.faults.push(Fault::new(
                    Some(lock.lock_id),
                    "pin-missing",
                    target_address,
                    format!("pinned target row {target_row} no longer exists"),
                )),
                Some(card) if &card.address != target_address => {
                    integrity.faults.push(Fault::new(
                        Some(lock.lock_id),
                        "pin-changed",
                        target_address,
                        format!("pinned target row {target_row} now holds `{}`", card.address),
                    ));
                }
                Some(card) if crate::is_tree_member(&card.address, &lock.tree) => {
                    integrity.faults.push(Fault::new(
                        Some(lock.lock_id),
                        "pin-inside",
                        target_address,
                        "a pin names a Card inside its own locked tree",
                    ));
                }
                Some(_) => {}
            }
        }
        for fault in link_pin_faults(conn, lock, &members, &pins)? {
            integrity.faults.push(fault);
        }
    }
    integrity.faults.sort();
    integrity.faults.dedup();
    Ok(integrity)
}

/// Compare each member's resolvable external Links with the stored pins, excluding the addresses
/// that were already broken when the owner accepted the Lock.
fn link_pin_faults(
    conn: &Connection,
    lock: &Lock,
    members: &[(i64, String, String)],
    pins: &[(i64, i64, String)],
) -> Result<Vec<Fault>> {
    let mut faults = Vec::new();
    let cards = crate::load_cards(conn)?;
    let by_address = cards
        .iter()
        .map(|card| (card.address.clone(), card))
        .collect::<BTreeMap<_, _>>();
    let mut broken = BTreeSet::new();
    if let Some(detail) = conn
        .query_row(
            "SELECT detail FROM lock_events WHERE lock_id = ?1 AND action = 'lock'
             ORDER BY event_id LIMIT 1",
            [lock.lock_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        && let Ok(value) = serde_json::from_str::<Value>(&detail)
    {
        for warning in value["warnings"].as_array().into_iter().flatten() {
            if let (Some(source), Some(target)) = (
                warning["source"].as_str(),
                warning["target"].as_str(),
            ) {
                broken.insert((source.to_string(), target.to_string()));
            }
        }
    }
    for (row_id, address, _) in members {
        let Some(card) = by_address.get(address) else {
            continue;
        };
        let parsed = crate::parse_card_text(&card.text)?;
        let body = crate::link::LinkBody::parse(&parsed.body)?;
        let mut expected = BTreeSet::new();
        for occurrence in body.occurrences() {
            let Some(target) = by_address.get(occurrence.target) else {
                continue;
            };
            if crate::is_tree_member(&target.address, &lock.tree) {
                continue;
            }
            if broken.contains(&(address.clone(), occurrence.target.to_string())) {
                continue;
            }
            expected.insert(target.row_id);
        }
        let stored = pins
            .iter()
            .filter(|(source_row, _, _)| source_row == row_id)
            .map(|(_, target_row, _)| *target_row)
            .collect::<BTreeSet<_>>();
        if expected != stored {
            faults.push(Fault::new(
                Some(lock.lock_id),
                "pin-set",
                address,
                format!(
                    "resolvable external Links of row {row_id} do not agree with its stored pins"
                ),
            ));
        }
    }
    Ok(faults)
}

/// Persist newly observed faults as `integrity-fault` events under the caller's edit lock and
/// transaction. A repeated scan of the same fault adds no event.
pub(crate) fn record_faults_tx(conn: &Connection, integrity: &Integrity) -> Result<usize> {
    let mut recorded = 0;
    for fault in &integrity.faults {
        let Some(lock_id) = fault.lock_id else {
            continue;
        };
        let seen = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM lock_events
                 WHERE lock_id = ?1 AND action = 'integrity-fault'
                   AND json_extract(detail, '$.fault') = ?2)",
                params![lock_id, fault.id],
                |row| row.get::<_, bool>(0),
            )
            .unwrap_or(false);
        if seen {
            continue;
        }
        let detail = fault.value();
        conn.execute(
            "INSERT INTO lock_events(lock_id, action, at, detail)
             VALUES(?1, 'integrity-fault', strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?2)",
            params![lock_id, json!({"fault": fault.id, "evidence": detail}).to_string()],
        )?;
        recorded += 1;
    }
    if recorded > 0 {
        crate::link_index::bump_revision(conn)?;
    }
    Ok(recorded)
}

// ---------------------------------------------------------------------------------------------
// History and export
// ---------------------------------------------------------------------------------------------

/// The versioned dump and panel representation: every Lock (active or ended), its retained
/// snapshots, its ordered events and the current integrity result.
pub(crate) fn export(conn: &Connection) -> Result<Value> {
    let Some(state) = load_state(conn)? else {
        return Ok(json!({
            "version": 1,
            "schema_version": Value::Null,
            "locks": [],
            "integrity": {"ok": false, "faults": [{"id": "lock-schema:tree_lock:tree_lock", "kind": "schema"}]},
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
        object.insert("events".into(), Value::Array(events));
        locks.push(value);
    }
    Ok(json!({
        "version": 1,
        "schema_version": SCHEMA_VERSION,
        "locks": locks,
        "integrity": check(conn)?.value(),
    }))
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
        let export = export(&conn).expect("export");
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
        // The lock-bound fault is recorded; a schema fault has no Lock to attach its evidence to.
        assert_eq!(record_faults_tx(&tx, &integrity).expect("record"), 1);
        tx.commit().expect("commit");
        let recorded: Vec<String> = conn
            .prepare("SELECT detail FROM lock_events WHERE action = 'integrity-fault'")
            .expect("events")
            .query_map([], |row| row.get(0))
            .expect("events")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("events");
        assert_eq!(recorded.len(), 1);
        assert!(
            recorded[0].contains("fingerprint-changed"),
            "{}",
            recorded[0]
        );
        let tx = conn.transaction().expect("transaction");
        assert_eq!(record_faults_tx(&tx, &integrity).expect("record again"), 0);
        tx.commit().expect("commit");
        let _ = temp;
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
        assert!(TRIGGERS_SQL.contains(crate::card::REVERSE_MARKER));
        assert!(!TRIGGERS_SQL.contains("zt_authored_fp"));
        assert!(!TRIGGERS_SQL.contains("zt_tree_root"));
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
}

impl Refusal {
    #[cfg(test)]
    fn code_string(&self) -> &'static str {
        self.kind.code()
    }
}
