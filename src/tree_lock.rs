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
use std::path::Path;

use crate::Workspace;

/// The current Tree Lock schema generation this binary installs and understands.
pub(crate) const SCHEMA_VERSION: &str = "2";
/// The previous generation this binary validates and upgrades once (S07, #149).
const LEGACY_SCHEMA_VERSION: &str = "1";
const SCHEMA_KEY: &str = "tree_lock_schema_version";

/// One stored Tree Lock protection generation. Generation 1 protects the pre-Term authored
/// fields; generation 2 adds the Term Card's name. Both keep the same four lock tables, indexes
/// and trigger names, so the difference is one exact trigger definition plus the declared
/// generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Generation {
    One,
    Two,
}

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

/// The six canonical protection triggers of generation 1, each with the exact statement that
/// creates it, in the order the design lists them. The scan compares the stored definition because
/// a trigger replaced under the same name would still pass a name-only check (R026, R287).
const TRIGGER_DEFS_V1: [(&str, &str); 6] = [
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

/// The canonical protection triggers of one generation. Generation 2 adds the Term Card's name to
/// the Locked-member update guard, so a name-only change is an authored change. Every other
/// statement is the same text, so a generation-1 trigger is compared with the generation-1
/// statement and is never called tampered merely because it lacks the new field (S07-AC07).
fn trigger_defs(generation: Generation) -> Vec<(&'static str, String)> {
    TRIGGER_DEFS_V1
        .iter()
        .map(|(name, sql)| {
            let mut sql = (*sql).to_string();
            if generation == Generation::Two && *name == "lock_no_change" {
                let anchor = "    OR NEW.math_form    IS NOT OLD.math_form\n";
                assert!(
                    sql.contains(anchor),
                    "the generation-1 update guard carries its last old field"
                );
                sql = sql.replace(
                    anchor,
                    "    OR NEW.math_form    IS NOT OLD.math_form\n    OR NEW.term_name    IS NOT OLD.term_name\n",
                );
            }
            (*name, sql)
        })
        .collect()
}

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

/// Register the deterministic SQL wrappers on a connection. This installs no schema and runs no
/// migration, so opening a connection never recurses into the writer lock.
pub(crate) fn register_sql_functions(conn: &Connection) -> Result<()> {
    use rusqlite::functions::FunctionFlags;
    let flags = FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_UTF8;
    // The eight-argument form is the pre-Term contract: it hashes the legacy fields and treats the
    // term name as absent, exactly as it did before the column existed. Retained SQL that calls the
    // old arity keeps its old answer; a term-aware caller uses the nine-argument form (S07, #149).
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
            None,
        )
        .map_err(|error| rusqlite::Error::UserFunctionError(Box::from(format!("{error:#}"))))
    })
    .context("failed to register zt_authored_fp")?;
    conn.create_scalar_function("zt_authored_fp", 9, flags, |ctx| {
        let text: String = ctx.get(0)?;
        let bibtex: Option<String> = ctx.get(1)?;
        let card_type: String = ctx.get(2)?;
        let facet: Option<String> = ctx.get(3)?;
        let emphasis: Option<String> = ctx.get(4)?;
        let model_level: Option<String> = ctx.get(5)?;
        let hub_level: Option<String> = ctx.get(6)?;
        let form: Option<String> = ctx.get(7)?;
        let term_name: Option<String> = ctx.get(8)?;
        fingerprint_of_raw(
            &text,
            bibtex.as_deref(),
            &card_type,
            facet.as_deref(),
            emphasis.as_deref(),
            model_level.as_deref(),
            hub_level.as_deref(),
            form.as_deref(),
            term_name.as_deref(),
        )
        .map_err(|error| rusqlite::Error::UserFunctionError(Box::from(format!("{error:#}"))))
    })
    .context("failed to register the term-aware zt_authored_fp")?;
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
    term_name: Option<&str>,
) -> Result<String> {
    let parsed = crate::parse_card_text(text)?;
    let card_type = crate::CardType::parse(card_type)?;
    crate::term::ensure_on_type(term_name, card_type)?;
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
    Ok(crate::card::authored_fingerprint_framed_term(
        &parsed.title,
        &parsed.body,
        bibtex.unwrap_or_default(),
        card_type.id(),
        facet.as_ref().map_or("", |facet| facet.id()),
        emphasis.map_or("", |emphasis| emphasis.id()),
        &level_id,
        form.map_or("", |form| form.id()),
        term_name,
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

/// The generation this archive declares, `None` when the Tree Lock schema is not installed. An
/// unknown generation is refused here, before any caller can read or repair it.
pub(crate) fn stored_generation(conn: &Connection) -> Result<Option<Generation>> {
    match schema_version(conn)?.as_deref() {
        None => Ok(None),
        Some(LEGACY_SCHEMA_VERSION) => Ok(Some(Generation::One)),
        Some(SCHEMA_VERSION) => Ok(Some(Generation::Two)),
        Some(other) => bail!(
            "unsupported Tree Lock schema version `{other}`; archive was not modified"
        ),
    }
}

/// Install or validate the current generation and its protection. Called once at service startup,
/// after the Card-table rebuilds, so it never fights a migration. An unsupported generation refuses
/// to write rather than attempting a repair; a supported generation has a missing or altered
/// trigger, index or table re-created here, and the caller has already recorded that fault. A
/// generation-1 archive is never repaired here: its one validated upgrade belongs to the Card
/// migration, so an altered old trigger is refused rather than silently replaced. Answers the
/// canonical objects this call had to reinstall, in catalog order.
fn install_or_repair(conn: &Connection) -> Result<Vec<String>> {
    let mut repaired = Vec::new();
    match stored_generation(conn)? {
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
        Some(Generation::Two) => {
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
        Some(Generation::One) => bail!(
            "Tree Lock generation 1 was not upgraded by the Card migration; archive was not modified"
        ),
    }
    Ok(repaired)
}

/// The structural generation check the boot sequence runs before any repair or Card migration
/// (R303, S07-AC02/AC04). A missing lock schema is a first installation and passes; either
/// generation this binary can interpret must still hold the four tables with the columns it reads;
/// any other stored generation refuses startup, so no migration ever runs against an archive whose
/// promise this binary cannot interpret. It creates, repairs and records nothing.
pub(crate) fn check_generation(conn: &Connection) -> Result<()> {
    check_generation_within(conn, &[LEGACY_SCHEMA_VERSION, SCHEMA_VERSION])
}

/// The same structural check with an explicit supported set. Production supports generations 1 and
/// 2; the compatibility fixture runs the baseline release's rule (generation 1 only) against a
/// generation-2 archive, proving the old binary refuses before it modifies anything (S07-AC08,
/// R0248). This parameterizes the decision, not the stored check.
pub(crate) fn check_generation_within(conn: &Connection, supported: &[&str]) -> Result<()> {
    match schema_version(conn)? {
        None => Ok(()),
        Some(version) if supported.contains(&version.as_str()) => validate_schema(conn),
        Some(other) => bail!(
            "unsupported Tree Lock schema version `{other}`; archive was not modified"
        ),
    }
}

/// Install generation 2 over a validated generation-1 archive, atomically with the Card schema
/// widening (S07-AC07, R0247). The caller has already compared the old canonical protection and
/// recorded the active member evidence; this call replaces the six protection triggers, validates
/// them, and only then moves the declared generation. A missing or altered generation-1 trigger is
/// refused by the pre-migration check, never repaired here. Answers whether it upgraded.
pub(crate) fn upgrade_generation(conn: &Connection) -> Result<bool> {
    if stored_generation(conn)? != Some(Generation::One) {
        return Ok(false);
    }
    let tx = conn.unchecked_transaction()?;
    let upgraded = upgrade_generation_tx(&tx)?;
    tx.commit()?;
    Ok(upgraded)
}

/// The generation-2 protection installation inside the caller's transaction, so a Card-table
/// rebuild and the protection it needs commit together. Nothing is read from or written to a Card,
/// so every old identity is untouched.
pub(crate) fn upgrade_generation_tx(conn: &Connection) -> Result<bool> {
    if stored_generation(conn)? != Some(Generation::One) {
        return Ok(false);
    }
    for name in TRIGGER_NAMES {
        conn.execute_batch(&format!("DROP TRIGGER IF EXISTS {name};"))
            .with_context(|| format!("failed to replace Tree Lock trigger `{name}`"))?;
    }
    for (name, sql) in trigger_defs(Generation::Two) {
        conn.execute_batch(&sql)
            .with_context(|| format!("failed to install Tree Lock trigger `{name}`"))?;
    }
    validate_indexes(conn)?;
    validate_triggers(conn)?;
    conn.execute(
        "UPDATE metadata SET value = ?1 WHERE key = ?2",
        params![SCHEMA_VERSION, SCHEMA_KEY],
    )?;
    Ok(true)
}

/// R302 (S07-AC01/AC09): one consistent SQLite backup of the archive before the first installation
/// of the Tree Lock schema, following the repository's existing migration backup pattern. `VACUUM
/// INTO` writes a committed copy, so a live WAL database is never file-copied. A fresh archive and a
/// start that finds the schema already installed are not backed up, and the backup is kept even when
/// the installation that follows fails. Answers the path it wrote.
pub(crate) fn backup_before_install(
    conn: &Connection,
    root: &Path,
    archive_existed: bool,
) -> Result<Option<std::path::PathBuf>> {
    if !archive_existed || schema_version(conn)?.is_some() {
        return Ok(None);
    }
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // A retry in the same second must not collide with the backup the failed attempt retained;
    // `VACUUM INTO` refuses an existing output file.
    let mut backup = root.join(format!("zt.sqlite3.pre-tree-lock-{seconds}.bak"));
    let mut attempt = 1;
    while backup.exists() {
        backup = root.join(format!("zt.sqlite3.pre-tree-lock-{seconds}-{attempt}.bak"));
        attempt += 1;
    }
    conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])
        .with_context(|| format!("failed to back up the archive to {}", backup.display()))?;
    Ok(Some(backup))
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

/// Whether one schema object exists with its canonical definition. A name present with another
/// definition is a different promise, not a compatibility success (R287).
fn is_canonical(conn: &Connection, kind: &str, name: &str, sql: &str) -> Result<bool> {
    Ok(matches!(
        stored_sql(conn, kind, name)?,
        Some(stored) if same_definition(&stored, sql)
    ))
}

fn trigger_exists(conn: &Connection, name: &str) -> Result<bool> {
    Ok(stored_sql(conn, "trigger", name)?.is_some())
}

/// Validate the protection triggers of one exact generation. A generation-1 archive is checked
/// against its own statements, so its missing new field is not tampering.
fn validate_triggers_for(conn: &Connection, generation: Generation) -> Result<()> {
    for name in TRIGGER_NAMES {
        if !trigger_exists(conn, name)? {
            bail!("Tree Lock trigger `{name}` is missing; archive was not modified");
        }
    }
    for (name, sql) in trigger_defs(generation) {
        let stored = stored_sql(conn, "trigger", name)?
            .with_context(|| format!("Tree Lock trigger `{name}` is missing"))?;
        if !same_definition(&stored, &sql) {
            bail!(
                "Tree Lock trigger `{name}` has an altered definition; archive was not modified"
            );
        }
    }
    Ok(())
}

/// Validate the current generation's triggers.
fn validate_triggers(conn: &Connection) -> Result<()> {
    validate_triggers_for(conn, Generation::Two)
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

/// Re-create every missing or altered canonical trigger of one generation, in catalog order, and
/// answer their names.
fn reinstall_triggers_for(conn: &Connection, generation: Generation) -> Result<Vec<String>> {
    let mut repaired = Vec::new();
    for (name, sql) in trigger_defs(generation) {
        if is_canonical(conn, "trigger", name, &sql)? {
            continue;
        }
        conn.execute_batch(&format!("DROP TRIGGER IF EXISTS {name};"))
            .with_context(|| format!("failed to replace Tree Lock trigger `{name}`"))?;
        conn.execute_batch(&sql)
            .with_context(|| format!("failed to install Tree Lock trigger `{name}`"))?;
        repaired.push(name.to_string());
    }
    Ok(repaired)
}

/// Re-create every missing or altered current-generation trigger, in catalog order, and answer their
/// names.
fn reinstall_triggers(conn: &Connection) -> Result<Vec<String>> {
    reinstall_triggers_for(conn, Generation::Two)
}

/// Re-create every missing or altered canonical index, in catalog order, and answer their names.
fn reinstall_indexes(conn: &Connection) -> Result<Vec<String>> {
    let mut repaired = Vec::new();
    for (name, sql) in INDEX_DEFS {
        if is_canonical(conn, "index", name, sql)? {
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

/// The canonical protection that is present and exact right now, so the boot sequence can tell what
/// its own authorized Card rebuild dropped. The generation is the stored one, so a generation-1
/// archive is compared with its own exact statements. Empty when no supported lock schema is
/// installed, which is also the answer that makes the restoration a no-op (R051, R304).
pub(crate) fn canonical_protection(conn: &Connection) -> Result<BTreeSet<&'static str>> {
    let mut present = BTreeSet::new();
    let Some(generation) = stored_generation(conn)? else {
        return Ok(present);
    };
    for (name, sql) in INDEX_DEFS {
        if is_canonical(conn, "index", name, sql)? {
            present.insert(name);
        }
    }
    for (name, sql) in trigger_defs(generation) {
        if is_canonical(conn, "trigger", name, &sql)? {
            present.insert(name);
        }
    }
    Ok(present)
}

/// Re-create the canonical protection a rebuild of `cards` dropped, before the guarded startup
/// comparison, so an authorized Card rebuild is not recorded as tampering (R051, R304, S07-AC05). An
/// object that was canonical before the rebuild and is missing or altered after it was lost by this
/// boot's own rebuild; one that was already missing before keeps its recorded evidence and is left
/// to the guarded comparison. A damaged table or an unsupported generation is never repaired here.
/// The stored generation decides the exact statements; after the Term migration it is generation 2.
/// Answers the objects it re-created, in catalog order.
pub(crate) fn restore_rebuilt_protection(
    conn: &Connection,
    before: &BTreeSet<&'static str>,
) -> Result<Vec<String>> {
    let Some(generation) = stored_generation(conn)? else {
        return Ok(Vec::new());
    };
    validate_schema(conn)?;
    let mut restored = Vec::new();
    for (name, sql) in INDEX_DEFS {
        if before.contains(name) && !is_canonical(conn, "index", name, sql)? {
            conn.execute_batch(&format!("DROP INDEX IF EXISTS {name};"))
                .with_context(|| format!("failed to replace Tree Lock index `{name}`"))?;
            conn.execute_batch(sql)
                .with_context(|| format!("failed to install Tree Lock index `{name}`"))?;
            restored.push(name.to_string());
        }
    }
    for (name, sql) in trigger_defs(generation) {
        if before.contains(name) && !is_canonical(conn, "trigger", name, &sql)? {
            conn.execute_batch(&format!("DROP TRIGGER IF EXISTS {name};"))
                .with_context(|| format!("failed to replace Tree Lock trigger `{name}`"))?;
            conn.execute_batch(&sql)
                .with_context(|| format!("failed to install Tree Lock trigger `{name}`"))?;
            restored.push(name.to_string());
        }
    }
    if !restored.is_empty() {
        validate_indexes(conn)?;
        validate_triggers_for(conn, generation)?;
    }
    Ok(restored)
}

// ---------------------------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------------------------

/// The exact Card effect an active Lock refused (R184). The reason is the effect the caller asked
/// for, so a client acts on it without reading English: `edit`, `delete`, `move`, `create-inside`,
/// `move-inside`, `pinned-move` and `pinned-delete`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Reason {
    /// The Card is a member of an active Locked tree and its authored identity would change.
    Edit,
    /// The Card is a member of an active Locked tree and would be deleted.
    Delete,
    /// The Card is a member of an active Locked tree and would move or be renamed.
    Move,
    /// A new Card would be created inside an active Locked tree.
    CreateInside,
    /// A Card would be moved into an active Locked tree.
    MoveInside,
    /// The Card is pinned by an active Locked tree and would move, be renamed or compacted.
    PinnedMove,
    /// The Card is pinned by an active Locked tree and would be deleted.
    PinnedDelete,
}

impl Reason {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Reason::Edit => "edit",
            Reason::Delete => "delete",
            Reason::Move => "move",
            Reason::CreateInside => "create-inside",
            Reason::MoveInside => "move-inside",
            Reason::PinnedMove => "pinned-move",
            Reason::PinnedDelete => "pinned-delete",
        }
    }

    /// The actionable sentence the shell and the Session append to the message (R185). Every Card
    /// effect has an owner action: release the Lock, or write elsewhere and link the locked Card.
    pub(crate) fn hint(self) -> &'static str {
        match self {
            Reason::PinnedMove | Reason::PinnedDelete => {
                "write a new Card in another tree and link the locked Card, or unlock the tree in the GUI first"
            }
            _ => "unlock the tree in the GUI first",
        }
    }
}

/// One active Lock that blocks a change: its numeric id and its Locked tree's root address.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct Blocker {
    pub(crate) lock_id: i64,
    pub(crate) tree: String,
}

/// One active Lock that pins a Card, with the locked source Cards holding the Links.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PinSource {
    pub(crate) lock_id: i64,
    pub(crate) tree: String,
    pub(crate) sources: Vec<String>,
}

impl PinSource {
    fn blocker(&self) -> Blocker {
        Blocker {
            lock_id: self.lock_id,
            tree: self.tree.clone(),
        }
    }
}

/// The structured `card_locked` refusal every writer shares (R182-R185): the blocking Locked tree's
/// root address, the affected Card addresses, the exact reason and the actionable hint, sorted and
/// deduplicated so equal facts always read the same. `tree` is the first blocking root and `locks`
/// explains every blocking Lock. A pin refusal additionally names the locked source Cards.
#[derive(Clone, Debug)]
pub(crate) struct Refusal {
    pub(crate) reason: Reason,
    pub(crate) message: String,
    /// The affected Card addresses, sorted and deduplicated; empty when the whole archive was asked for.
    pub(crate) cards: Vec<String>,
    /// The blocking Locked trees' root addresses, sorted and deduplicated; `trees[0]` is `tree`.
    pub(crate) trees: Vec<String>,
    /// The locked source Cards whose Links pin an affected Card; empty for a membership refusal.
    pub(crate) sources: Vec<String>,
    /// Every blocking Lock, sorted by Lock id and deduplicated.
    pub(crate) locks: Vec<Blocker>,
}

/// Sorted, deduplicated copy of `values`, so two refusals about the same facts read the same.
fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn quoted(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The grammatical forms a refusal message needs for one Card or several.
fn noun(cards: &[String]) -> &'static str {
    if cards.len() == 1 { "card" } else { "cards" }
}

fn verb(cards: &[String]) -> &'static str {
    if cards.len() == 1 { "is" } else { "are" }
}

fn pronoun(cards: &[String]) -> &'static str {
    if cards.len() == 1 { "it" } else { "they" }
}

fn source_noun(sources: &[String]) -> &'static str {
    if sources.len() == 1 { "card" } else { "cards" }
}

fn source_verb(sources: &[String]) -> &'static str {
    if sources.len() == 1 { "links" } else { "link" }
}

impl Refusal {
    /// One refusal about Cards that would change, move or be deleted in their Locked trees.
    /// `entries` are `(address, Lock)` facts; `reason` names the effect the caller asked for.
    fn by_blockers(reason: Reason, entries: impl IntoIterator<Item = (String, Blocker)>) -> Self {
        let entries = entries.into_iter().collect::<Vec<_>>();
        let cards = sorted_unique(entries.iter().map(|(card, _)| card.clone()));
        let mut locks = entries
            .iter()
            .map(|(_, blocker)| blocker.clone())
            .collect::<Vec<_>>();
        locks.sort();
        locks.dedup();
        let trees = sorted_unique(locks.iter().map(|blocker| blocker.tree.clone()));
        let message = match reason {
            Reason::Edit => format!(
                "{} {} {} in locked tree {}; locked Cards can be linked but not changed",
                noun(&cards),
                quoted(&cards),
                verb(&cards),
                quoted(&trees)
            ),
            Reason::Delete => format!(
                "{} {} cannot be deleted: {} {} in locked tree {}",
                noun(&cards),
                quoted(&cards),
                pronoun(&cards),
                verb(&cards),
                quoted(&trees)
            ),
            Reason::Move => format!(
                "{} {} cannot be moved: {} {} in locked tree {}",
                noun(&cards),
                quoted(&cards),
                pronoun(&cards),
                verb(&cards),
                quoted(&trees)
            ),
            Reason::CreateInside => format!(
                "{} {} inside locked tree {}; a locked tree takes no new Cards",
                quoted(&cards),
                verb(&cards),
                quoted(&trees)
            ),
            Reason::MoveInside => format!(
                "{} {} inside locked tree {}; a locked tree takes no moved-in Cards",
                quoted(&cards),
                verb(&cards),
                quoted(&trees)
            ),
            Reason::PinnedMove | Reason::PinnedDelete => {
                unreachable!("a pin refusal is built from the pinning Locks")
            }
        };
        Self {
            reason,
            message,
            cards,
            trees,
            sources: Vec::new(),
            locks,
        }
    }

    /// One refusal about Cards an active Locked tree pins: `entries` are `(target, pinning Locks)`.
    fn by_pins(
        reason: Reason,
        entries: impl IntoIterator<Item = (String, Vec<PinSource>)>,
    ) -> Self {
        let entries = entries.into_iter().collect::<Vec<_>>();
        let cards = sorted_unique(entries.iter().map(|(card, _)| card.clone()));
        let pins = entries.iter().flat_map(|(_, pins)| pins.iter());
        let mut locks = pins.clone().map(PinSource::blocker).collect::<Vec<_>>();
        locks.sort();
        locks.dedup();
        let trees = sorted_unique(locks.iter().map(|blocker| blocker.tree.clone()));
        let sources = sorted_unique(pins.flat_map(|pin| pin.sources.iter().cloned()));
        let effect = match reason {
            Reason::PinnedMove => "moved",
            Reason::PinnedDelete => "deleted",
            other => unreachable!("`{}` is not a pin reason", other.id()),
        };
        let message = format!(
            "{} {} cannot be {effect}: {} {} a pinned target of locked tree {}; locked source {} {} {} {}",
            noun(&cards),
            quoted(&cards),
            pronoun(&cards),
            verb(&cards),
            quoted(&trees),
            source_noun(&sources),
            quoted(&sources),
            source_verb(&sources),
            pronoun(&cards)
        );
        Self {
            reason,
            message,
            cards,
            trees,
            sources,
            locks,
        }
    }

    /// The whole archive would be emptied while some Lock is still active (R181): no single Card is
    /// named, and every blocking Lock is explained.
    fn deleting_all(blockers: Vec<Blocker>) -> Self {
        let mut locks = blockers;
        locks.sort();
        locks.dedup();
        let trees = sorted_unique(locks.iter().map(|blocker| blocker.tree.clone()));
        let message = format!(
            "the archive holds {} active Lock{}, locked tree {}; unlocking every tree is required before deleting all Cards",
            locks.len(),
            if locks.len() == 1 { "" } else { "s" },
            quoted(&trees)
        );
        Self {
            reason: Reason::Delete,
            message,
            cards: Vec::new(),
            trees,
            sources: Vec::new(),
            locks,
        }
    }

    pub(crate) fn value(&self) -> Value {
        let mut value = json!({
            "error": "card_locked",
            "message": self.message,
            "tree": self.trees.first(),
            "cards": self.cards,
            "reason": self.reason.id(),
            "hint": self.reason.hint(),
        });
        if !self.sources.is_empty() {
            value["sources"] = json!(self.sources);
        }
        if !self.locks.is_empty() {
            value["locks"] = Value::Array(
                self.locks
                    .iter()
                    .map(|blocker| json!({"lock_id": blocker.lock_id, "tree": blocker.tree}))
                    .collect(),
            );
        }
        value
    }
}

impl std::fmt::Display for Refusal {
    /// The one line the shell and the Session print: the message, then the owner's hint (R185).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}; {}", self.message, self.reason.hint())
    }
}

impl std::error::Error for Refusal {}

/// A Lock plan refused because the derived Link index disagrees with the authored bodies (R144), so
/// the pins it would capture cannot be trusted. It is its own outcome, not a `card_locked` refusal.
#[derive(Clone, Debug)]
pub(crate) struct IndexRefusal {
    pub(crate) card: String,
    pub(crate) tree: String,
    pub(crate) detail: String,
}

impl IndexRefusal {
    pub(crate) fn value(&self) -> Value {
        json!({
            "error": "index_integrity",
            "message": self.to_string(),
            "tree": self.tree,
            "cards": [self.card],
            "hint": self.detail,
        })
    }
}

impl std::fmt::Display for IndexRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the Link index does not match the authored body of `{}` in tree `{}`: {}",
            self.card, self.tree, self.detail
        )
    }
}

impl std::error::Error for IndexRefusal {}

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
    // An archive with no lock schema, or one whose declared generation this binary cannot read, has
    // no promise this binary may write; the generation check refuses the latter at startup.
    if stored_generation(conn).ok().flatten().is_none() {
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
        if stored_generation(conn).ok().flatten().is_none() {
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
            "lock_id": lock.lock_id,
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
            return Err(anyhow::Error::new(IndexRefusal {
                card: member.address.clone(),
                tree: plan.root.clone(),
                detail: "run `zt reindex` before locking".to_string(),
            }));
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
        bail!(
            "the Tree Lock schema is not installed or is not a supported generation; restart the service before writing cards"
        );
    };
    check_change(conn, &state, change)
}

fn check_change(conn: &Connection, state: &State, change: Change<'_>) -> Result<()> {
    match change {
        Change::Edit(row_id) => {
            if let Some(blocker) = member_blocker(state, row_id) {
                // The refused Card's own address, never another member's: the owner's refusal names
                // the exact Card whose write the Lock stopped (R217, S05-AC04).
                let address = state
                    .member_address_by_row
                    .get(&row_id)
                    .cloned()
                    .unwrap_or_default();
                return Err(anyhow::Error::new(Refusal::by_blockers(
                    Reason::Edit,
                    [(address, blocker)],
                )));
            }
        }
        Change::CreateAt(address) => {
            if let Some((lock_id, root)) = active_root_of(state, address) {
                return Err(anyhow::Error::new(Refusal::by_blockers(
                    Reason::CreateInside,
                    [(
                        address.to_string(),
                        Blocker {
                            lock_id,
                            tree: root,
                        },
                    )],
                )));
            }
        }
        Change::Delete(plan) => {
            // R030: a Lock forbids the requested tree operation even when its delete set is empty,
            // so emptying a Locked tree is refused while its retained root is a promised member.
            if let Some((root, _)) = plan.kept_root()
                && let Some((lock_id, tree)) = state.member_by_address.get(root)
            {
                return Err(anyhow::Error::new(Refusal::by_blockers(
                    Reason::Delete,
                    [(
                        root.to_string(),
                        Blocker {
                            lock_id: *lock_id,
                            tree: tree.clone(),
                        },
                    )],
                )));
            }
            let cards = crate::load_cards(conn)?;
            let by_address = cards
                .iter()
                .map(|card| (card.address.as_str(), card))
                .collect::<BTreeMap<_, _>>();
            // Every affected Card of one category is named once: a refused delete never hides the
            // other locked members of the same delete set (R184).
            let mut members = Vec::new();
            let mut pinned = Vec::new();
            for address in &plan.delete_set {
                let Some(card) = by_address.get(address.as_str()) else {
                    continue;
                };
                if let Some(blocker) = member_blocker(state, card.row_id) {
                    members.push((card.address.clone(), blocker));
                    continue;
                }
                let pins = pins_of(state, card.row_id);
                if !pins.is_empty() {
                    pinned.push((card.address.clone(), pins));
                }
            }
            if !members.is_empty() {
                return Err(anyhow::Error::new(Refusal::by_blockers(
                    Reason::Delete,
                    members,
                )));
            }
            if !pinned.is_empty() {
                return Err(anyhow::Error::new(Refusal::by_pins(
                    Reason::PinnedDelete,
                    pinned,
                )));
            }
            check_mapping(conn, state, &plan.compaction())?;
            let repoint = plan.repoint_mapping();
            check_change(conn, state, Change::LinkRewrites(&repoint))?;
        }
        Change::Mapping(mapping) => check_mapping(conn, state, mapping)?,
        Change::LinkRewrites(mapping) => check_link_rewrites(conn, state, mapping)?,
        Change::DeleteAll => {
            if !state.roots.is_empty() {
                return Err(anyhow::Error::new(Refusal::deleting_all(
                    state
                        .roots
                        .iter()
                        .map(|(root, lock_id)| Blocker {
                            lock_id: *lock_id,
                            tree: root.clone(),
                        })
                        .collect(),
                )));
            }
        }
    }
    Ok(())
}

/// The active Lock that promises this Card row, if any.
fn member_blocker(state: &State, row_id: i64) -> Option<Blocker> {
    state
        .member_by_row
        .get(&row_id)
        .map(|(lock_id, root)| Blocker {
            lock_id: *lock_id,
            tree: root.clone(),
        })
}

fn check_mapping(
    conn: &Connection,
    state: &State,
    mapping: &BTreeMap<String, String>,
) -> Result<()> {
    if mapping.is_empty() {
        return Ok(());
    }
    let mut members = Vec::new();
    let mut pinned = Vec::new();
    let mut destinations = Vec::new();
    for (old, new) in mapping {
        if let Some((lock_id, root)) = state.member_by_address.get(old) {
            members.push((
                old.clone(),
                Blocker {
                    lock_id: *lock_id,
                    tree: root.clone(),
                },
            ));
            continue;
        }
        let cards = crate::load_cards(conn)?;
        if let Some(card) = cards.iter().find(|card| &card.address == old) {
            let pins = pins_of(state, card.row_id);
            if !pins.is_empty() {
                pinned.push((old.clone(), pins));
            }
        }
        if let Some((lock_id, root)) = active_root_of(state, new) {
            destinations.push((
                new.clone(),
                Blocker {
                    lock_id,
                    tree: root,
                },
            ));
        }
    }
    if !members.is_empty() {
        return Err(anyhow::Error::new(Refusal::by_blockers(
            Reason::Move,
            members,
        )));
    }
    if !pinned.is_empty() {
        return Err(anyhow::Error::new(Refusal::by_pins(
            Reason::PinnedMove,
            pinned,
        )));
    }
    if !destinations.is_empty() {
        return Err(anyhow::Error::new(Refusal::by_blockers(
            Reason::MoveInside,
            destinations,
        )));
    }
    check_change(conn, state, Change::LinkRewrites(mapping))
}

/// A rewritten macro in a locked source would change that source's authored body, so it is refused
/// even when the source itself does not move. Every locked source the rewrite would touch is named.
fn check_link_rewrites(
    conn: &Connection,
    state: &State,
    mapping: &BTreeMap<String, String>,
) -> Result<()> {
    if mapping.is_empty() {
        return Ok(());
    }
    let cards = crate::load_cards(conn)?;
    let mut touched = Vec::new();
    for card in &cards {
        let Some(blocker) = member_blocker(state, card.row_id) else {
            continue;
        };
        let parsed = crate::parse_card_text(&card.text)?;
        let body = crate::link::LinkBody::parse(&parsed.body)?;
        if body
            .occurrences()
            .iter()
            .any(|occurrence| mapping.contains_key(occurrence.target))
        {
            touched.push((card.address.clone(), blocker));
        }
    }
    if !touched.is_empty() {
        return Err(anyhow::Error::new(Refusal::by_blockers(
            Reason::Edit,
            touched,
        )));
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

/// The active promise scope one `zt status` line reports (R233–R235, S08-AC04): the active Locks, the
/// member snapshot rows they promise and the distinct pinned target rows they freeze. A Lock that
/// ended counts nowhere, and two pin rows naming one target count once.
#[derive(Clone, Debug, Default)]
pub(crate) struct Scope {
    trees: i64,
    cards: i64,
    pinned_targets: i64,
    /// One root address per active Lock id, so a fault names the trees it leaves unprotected.
    roots: BTreeMap<i64, String>,
}

impl Scope {
    /// `locks: 3 trees, 412 cards, 57 pinned targets`: the counts alone, for a command that prints its
    /// integrity result on its own line.
    pub(crate) fn counts(&self) -> String {
        format!(
            "locks: {} {}, {} {}, {} pinned {}",
            self.trees,
            plural(self.trees, "tree"),
            self.cards,
            plural(self.cards, "card"),
            self.pinned_targets,
            plural(self.pinned_targets, "target"),
        )
    }

    /// The exact user-facing contract of §11: the counts and `; integrity ok`, or a concise
    /// diagnostic and the faulted roots instead of that claim (R234, R235).
    pub(crate) fn line(&self, integrity: &Integrity) -> String {
        let mut line = format!("{}; {}", self.counts(), integrity.summary());
        if !integrity.ok() {
            let roots = self.faulted_roots(integrity);
            line.push_str(&format!(
                "; faulted roots: {}",
                if roots.is_empty() {
                    "none".to_string()
                } else {
                    roots.join(", ")
                }
            ));
        }
        line
    }

    /// The active Locked roots that carry a current or latched fault. A schema fault leaves every
    /// active promise unprotected, so it names every root.
    fn faulted_roots(&self, integrity: &Integrity) -> Vec<String> {
        self.roots
            .iter()
            .filter(|(lock_id, _)| {
                !integrity.faults_of(**lock_id).is_empty() || !integrity.latched_of(**lock_id).is_empty()
            })
            .map(|(_, root)| root.clone())
            .collect()
    }
}

fn plural(count: i64, word: &str) -> String {
    if count == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

/// The active Lock scope of one archive. An archive whose Tree Lock record is not installed or is not
/// the supported generation cannot answer, so its caller reports `unavailable` instead of a healthy
/// zero-Lock result (R235, R298). Reading the scope starts no Lock operation and writes nothing.
pub(crate) fn scope(conn: &Connection) -> Result<Scope> {
    if stored_generation(conn)?.is_none() {
        bail!("the Tree Lock schema is not installed or is not the supported generation");
    }
    if conn.is_autocommit() {
        let view = conn.unchecked_transaction()?;
        let scope = read_scope(&view)?;
        let _ = view.rollback();
        Ok(scope)
    } else {
        read_scope(conn)
    }
}

fn read_scope(conn: &Connection) -> Result<Scope> {
    let active = "FROM locks l WHERE l.unlocked_at IS NULL";
    let trees = conn.query_row(&format!("SELECT COUNT(*) {active}"), [], |row| row.get(0))?;
    let cards = conn.query_row(
        "SELECT COUNT(*) FROM lock_members m JOIN locks l ON l.lock_id = m.lock_id
          WHERE l.unlocked_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let pinned_targets = conn.query_row(
        "SELECT COUNT(DISTINCT p.target_row) FROM lock_pins p JOIN locks l ON l.lock_id = p.lock_id
          WHERE l.unlocked_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let mut roots = BTreeMap::new();
    let mut stmt = conn.prepare(&format!("SELECT lock_id, root_address {active} ORDER BY lock_id"))?;
    for row in stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })? {
        let (lock_id, root) = row?;
        roots.insert(lock_id, root);
    }
    Ok(Scope {
        trees,
        cards,
        pinned_targets,
        roots,
    })
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
    term_name: Option<String>,
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
            self.term_name.as_deref(),
        )
    }

    fn is_tree_root(&self) -> bool {
        self.is_topic == 1 || self.is_lit == 1
    }
}

/// Whether this archive's `cards` table already holds the Term Card name column. A generation-1
/// archive opened before its migration does not, and the snapshot then reads the field as absent.
fn has_term_column(conn: &Connection) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('cards') WHERE name = 'term_name')",
        [],
        |row| row.get::<_, bool>(0),
    )?)
}

fn stored_cards(conn: &Connection) -> Result<Vec<StoredCard>> {
    let term = if has_term_column(conn)? {
        "term_name"
    } else {
        "NULL"
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT row_id, COALESCE(location, citation_key), is_topic, is_lit, bibtex, text,
                workspace, card_type, model_facet, model_emphasis, model_level, hub_level, math_form, {term}
           FROM cards"
    ))?;
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
                term_name: row.get(13)?,
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
    // An archive with no lock schema, or one whose declared generation this binary cannot read, has
    // no active promise it can attribute; the generation check has already refused the latter.
    if stored_generation(conn).ok().flatten().is_none() {
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
    let generation = match stored_generation(conn) {
        Ok(generation) => generation,
        Err(_) => {
            integrity.faults.push(
                Fault::schema(
                    "schema-version",
                    SCHEMA_KEY,
                    "the Tree Lock schema is not installed or is not the supported generation",
                )
                .expected(SCHEMA_VERSION)
                .actual(schema_version(conn)?.unwrap_or_else(|| "<missing>".to_string())),
            );
            return Ok(false);
        }
    };
    let Some(generation) = generation else {
        integrity.faults.push(
            Fault::schema(
                "schema-version",
                SCHEMA_KEY,
                "the Tree Lock schema is not installed or is not the supported generation",
            )
            .expected(SCHEMA_VERSION)
            .actual("<missing>"),
        );
        return Ok(false);
    };
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
    for (name, sql) in trigger_defs(generation) {
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
            Some(stored) if !same_definition(&stored, &sql) => integrity.faults.push(
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
/// every other member is a Regular Card. Regular Cards carry neither flag in either tree kind:
/// `is_lit` marks only the Literature root (the Citation-key row), as the cards table requires.
fn member_kind_ok(lock: &StoredLock, card: &StoredCard) -> bool {
    let literature = crate::is_literature_tree_id(&lock.tree);
    let root = card.address == lock.root;
    card.is_topic == i64::from(root && !literature) && card.is_lit == i64::from(root && literature)
}

fn member_kind_expected(lock: &StoredLock, address: &str) -> &'static str {
    if crate::is_literature_tree_id(&lock.tree) {
        if address == lock.root {
            "the Literature root"
        } else {
            "a Literature tree member"
        }
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

/// The comparison a Card, row-identity or trigger migration runs first (R293, S07-AC07). For a
/// generation-1 archive it also validates the old canonical protection exactly, so an altered or
/// missing old trigger refuses the upgrade instead of being silently replaced. A no-op when no lock
/// schema is installed or no promise could be affected; the protection comparison runs even with no
/// active Lock, because the upgrade replaces that protection.
pub(crate) fn check_before_migration(conn: &Connection) -> Result<usize> {
    let Some(generation) = stored_generation(conn)? else {
        return Ok(0);
    };
    if generation == Generation::One {
        validate_indexes(conn)?;
        validate_triggers_for(conn, Generation::One)?;
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
        let (members, pins) = panel_snapshot(conn, lock)?;
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

/// One Lock's member and pin snapshot for the panel, in one shape and one order for an active and an
/// ended Lock: members by address, then one pin row per locked source and target address, ordered by
/// target address and source row (R213, R218). An ended Lock's live rows were deleted by its unlock,
/// so its retained `lock` event is the record: the addresses, rows and fingerprints the owner
/// reviewed, never a historical row joined to today's Cards (R104, S04-AC04).
fn panel_snapshot(conn: &Connection, lock: &Lock) -> Result<(Vec<Value>, Vec<Value>)> {
    match lock.unlocked_at {
        None => live_snapshot(conn, lock.lock_id),
        Some(_) => retained_snapshot(conn, lock.lock_id),
    }
}

/// The live `lock_members` and `lock_pins` rows of one active Lock.
fn live_snapshot(conn: &Connection, lock_id: i64) -> Result<(Vec<Value>, Vec<Value>)> {
    let mut members = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT row_id, address, authored_fp FROM lock_members WHERE lock_id = ?1 ORDER BY address",
    )?;
    for row in stmt.query_map([lock_id], |row| {
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
        "SELECT p.target_row, p.target_address, p.source_row, m.address
           FROM lock_pins p
           LEFT JOIN lock_members m ON m.lock_id = p.lock_id AND m.row_id = p.source_row
          WHERE p.lock_id = ?1
          ORDER BY p.target_address, p.source_row",
    )?;
    for row in stmt.query_map([lock_id], |row| {
        Ok(json!({
            "target_row": row.get::<_, i64>(0)?,
            "target": row.get::<_, String>(1)?,
            "source_row": row.get::<_, i64>(2)?,
            "source": row.get::<_, Option<String>>(3)?,
        }))
    })? {
        pins.push(row?);
    }
    Ok((members, pins))
}

/// The member and pin snapshot one Lock's own `lock` event retained.
fn retained_snapshot(conn: &Connection, lock_id: i64) -> Result<(Vec<Value>, Vec<Value>)> {
    let detail = conn
        .query_row(
            "SELECT detail FROM lock_events WHERE lock_id = ?1 AND action = 'lock'
             ORDER BY event_id LIMIT 1",
            [lock_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(detail) = detail else {
        return Ok((Vec::new(), Vec::new()));
    };
    let Ok(value) = serde_json::from_str::<Value>(&detail) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let mut members = value["members"]
        .as_array()
        .map(|members| {
            members
                .iter()
                .map(|member| {
                    json!({
                        "row_id": member["row_id"],
                        "address": member["address"],
                        "fingerprint": member["fingerprint"],
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    members.sort_by(|left, right| left["address"].as_str().cmp(&right["address"].as_str()));
    let row_of = members
        .iter()
        .filter_map(|member| {
            Some((
                member["address"].as_str()?.to_string(),
                member["row_id"].as_i64()?,
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let mut pins = Vec::new();
    for pin in value["pins"].as_array().into_iter().flatten() {
        for source in pin["sources"].as_array().into_iter().flatten() {
            let source = source.as_str().unwrap_or_default();
            pins.push(json!({
                "target_row": pin["target_row"],
                "target": pin["target"],
                "source_row": row_of.get(source),
                "source": source,
            }));
        }
    }
    pins.sort_by(|left, right| {
        left["target"]
            .as_str()
            .cmp(&right["target"].as_str())
            .then(left["source_row"].as_i64().cmp(&right["source_row"].as_i64()))
    });
    Ok((members, pins))
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
// Dump export
// ---------------------------------------------------------------------------------------------

/// The version-1 `locks.json` a dump writes (R315–R323, S13): every Lock (active or ended), the live
/// member and pin snapshots of the active ones, every event in event order, and the integrity facts
/// of one captured read view. Addresses and Lock ids carry the portable meaning; a row id is audit
/// evidence inside that captured database. An integrity fault is exported as evidence and never
/// refuses a diagnostic dump; lock data that cannot be read fails the export, so the dump fails.
pub(crate) fn dump_export(conn: &Connection) -> Result<Value> {
    if conn.is_autocommit() {
        let view = conn.unchecked_transaction()?;
        let export = dump_export_in(&view);
        let _ = view.rollback();
        export
    } else {
        dump_export_in(conn)
    }
}

fn dump_export_in(conn: &Connection) -> Result<Value> {
    let integrity = scan(conn)?;
    let mut locks = Vec::new();
    let mut events = Vec::new();
    if stored_generation(conn)?.is_some() {
        for lock in all_locks(conn)? {
            let mut value = lock.value();
            let object = value.as_object_mut().expect("Lock value is an object");
            object.insert("members".into(), Value::Array(export_members(conn, lock.lock_id)?));
            object.insert("pins".into(), Value::Array(export_pins(conn, lock.lock_id)?));
            locks.push(value);
        }
        events = export_events(conn)?;
    }
    Ok(json!({
        "schema_version": 1,
        "archive": archive_identity(conn)?,
        "revision": crate::link_index::revision(conn)?,
        "locks": locks,
        "events": events,
        "integrity": integrity.value(),
    }))
}

/// Every Lock of the archive, in creation order: an ended Lock keeps its record after the unlock, so
/// the export holds the whole owner history (R318).
fn all_locks(conn: &Connection) -> Result<Vec<Lock>> {
    let mut stmt = conn.prepare(&format!("SELECT {LOCK_COLUMNS} FROM locks ORDER BY lock_id"))?;
    let locks = stmt
        .query_map([], lock_from_row)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(locks)
}

/// One Lock's live member snapshots, ordered by address, each keeping its row identity as audit
/// evidence and its authored baseline (R319). An ended Lock keeps none live; its initial `lock` event
/// retains the same addresses, rows and fingerprints (R104, S13-AC03).
fn export_members(conn: &Connection, lock_id: i64) -> Result<Vec<Value>> {
    Ok(stored_members(conn, lock_id)?
        .into_iter()
        .map(|(row_id, address, fingerprint)| {
            json!({"address": address, "row_id": row_id, "fingerprint": fingerprint})
        })
        .collect())
}

/// One Lock's live pins, ordered by target address, target row, source address and source row: both
/// captured addresses and both row identities (R320). A damaged archive may hold a pin whose source
/// snapshot is gone; that source address is exported as null evidence instead of failing the dump.
fn export_pins(conn: &Connection, lock_id: i64) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT p.source_row, m.address, p.target_row, p.target_address
           FROM lock_pins p
           LEFT JOIN lock_members m ON m.lock_id = p.lock_id AND m.row_id = p.source_row
          WHERE p.lock_id = ?1
          ORDER BY p.target_address, p.target_row, m.address, p.source_row",
    )?;
    let pins = stmt
        .query_map([lock_id], |row| {
            Ok(json!({
                "source": row.get::<_, Option<String>>(1)?,
                "source_row": row.get::<_, i64>(0)?,
                "target": row.get::<_, String>(3)?,
                "target_row": row.get::<_, i64>(2)?,
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(pins)
}

/// Every event of every Lock in event-id order, with its stored deterministic JSON detail, so the
/// initial snapshot of an ended Lock stays readable after the live rows are gone (R321, R104).
fn export_events(conn: &Connection) -> Result<Vec<Value>> {
    let mut stmt = conn.prepare(
        "SELECT event_id, lock_id, action, at, detail FROM lock_events ORDER BY event_id",
    )?;
    let events = stmt
        .query_map([], |row| {
            let detail: String = row.get(4)?;
            Ok(json!({
                "event_id": row.get::<_, i64>(0)?,
                "lock_id": row.get::<_, i64>(1)?,
                "action": row.get::<_, String>(2)?,
                "at": row.get::<_, String>(3)?,
                "detail": serde_json::from_str::<Value>(&detail)
                    .unwrap_or_else(|_| Value::String(detail.clone())),
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(events)
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
            .expect("create topic")
            .0;
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

    /// The term-aware SQL fingerprint: nine arguments, the last the stored term name.
    fn fingerprint_query(
        conn: &Connection,
        text: &str,
        metadata: [Option<&str>; 7],
        term_name: Option<&str>,
    ) -> rusqlite::Result<String> {
        conn.query_row(
            "SELECT zt_authored_fp(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                text,
                metadata[0],
                metadata[1],
                metadata[2],
                metadata[3],
                metadata[4],
                metadata[5],
                metadata[6],
                term_name,
            ],
            |row| row.get(0),
        )
    }

    /// The retained eight-argument form keeps the pre-Term bytes; retained SQL that calls the old
    /// arity never silently reads the new field.
    fn fingerprint_query_legacy(
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
                term_name: None,
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
                term_name: None,
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
                term_name: None,
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
                term_name: None,
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
                term_name: None,
            },
            Card {
                row_id: 6,
                address: "1/1".into(),
                citation_key: None,
                is_topic: false,
                is_lit: false,
                bibtex: None,
                text: text.clone(),
                workspace: Workspace::Human,
                card_type: CardType::Term,
                facet: None,
                emphasis: None,
                level: None,
                hub_level: None,
                form: None,
                term_name: Some("均衡".into()),
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
                card.term_name.as_deref(),
            )
            .expect("SQL fingerprint");
            assert_eq!(
                actual, expected,
                "case {} {:?}",
                card.card_type.id(),
                card.facet
            );
            // The old arity always hashes the legacy bytes: a Card without a term name matches it,
            // and a named Card deliberately does not.
            let legacy = fingerprint_query_legacy(
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
            .expect("legacy SQL fingerprint");
            if card.term_name.is_none() {
                assert_eq!(legacy, expected, "the legacy arity is unchanged");
            } else {
                assert_ne!(legacy, expected, "a named Card differs from the legacy bytes");
            }
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
            None,
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

    /// R234/R235/S08-AC04: the `zt status` line reports the active Locks, their member snapshot rows
    /// and the distinct pinned target rows. Two locked sources naming one target count once, an
    /// ended Lock counts nowhere, and the healthy form is exactly the user-facing contract.
    #[test]
    fn the_status_line_counts_active_locks_member_rows_and_distinct_pinned_targets() {
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
            "3/0",
            true,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Outside", "outside body"),
        )
        .expect("outside topic");
        crate::insert_card(
            &mut conn,
            "3/1",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Target", "target body"),
        )
        .expect("target card");
        // A second member of the same tree names the same external target, so the two pin rows are
        // one distinct pinned target.
        crate::insert_card(
            &mut conn,
            &format!("{}|b", first_address(&root)),
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("Side again", "links [[3/1]] as well"),
        )
        .expect("second side card");
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        let lock = lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        let healthy = scope(&conn).expect("scope");
        assert_eq!(healthy.trees, 1);
        assert_eq!(healthy.cards, 4, "the root and its three Regular Cards");
        assert_eq!(healthy.pinned_targets, 1, "two pin rows, one target row");
        assert_eq!(
            healthy.line(&check(&conn).expect("check")),
            "locks: 1 tree, 4 cards, 1 pinned target; integrity ok"
        );
        // A second active Lock counts; ending the first one removes its members from the counts.
        let second_root = "2/0";
        let second_plan = resolve_plan(&conn, second_root, None).expect("second plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &second_plan).expect("second lock");
        tx.commit().expect("commit");
        let both = scope(&conn).expect("scope both");
        assert_eq!((both.trees, both.cards, both.pinned_targets), (2, 5, 1));
        let tx = conn.transaction().expect("transaction");
        unlock_tx(&tx, lock.lock_id).expect("unlock");
        tx.commit().expect("commit");
        assert_eq!(
            scope(&conn).expect("scope after unlock").line(&check(&conn).expect("check")),
            "locks: 1 tree, 1 card, 0 pinned targets; integrity ok"
        );
    }

    /// R235/S08-AC04: a fault prints the faulted roots and a concise diagnostic instead of a health
    /// claim, and recorded evidence that a later comparison cannot erase keeps it faulted.
    #[test]
    fn a_faulted_status_line_names_the_faulted_roots_and_the_diagnostic() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        assert_eq!(
            scope(&conn).expect("scope").line(&check(&conn).expect("check")),
            "locks: 1 tree, 3 cards, 0 pinned targets; integrity ok"
        );
        // Outside tampering: a member's authored text changes while its protection trigger is gone.
        let original = crate::load_card(&conn, &first_address(&root)).expect("first");
        conn.execute_batch("DROP TRIGGER lock_no_change")
            .expect("drop trigger");
        conn.execute(
            "UPDATE cards SET text = '# First\n\nchanged body\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
            [first_address(&root)],
        )
        .expect("tamper");
        let faulted = scope(&conn).expect("scope").line(&check(&conn).expect("check"));
        assert!(
            faulted.starts_with("locks: 1 tree, 3 cards, 0 pinned targets; integrity faulted: 2 current faults"),
            "{faulted}"
        );
        assert!(faulted.ends_with(&format!("faulted roots: {root}")), "{faulted}");
        // Recording the evidence and restoring the archive does not restore health: the latch keeps
        // the tree's root faulted until the owner reviews it.
        let observed = check(&conn).expect("check");
        let tx = conn.transaction().expect("transaction");
        record_faults_tx(&tx, &observed).expect("record");
        tx.commit().expect("commit");
        // Restoring the text is only possible while the trigger is still gone; restoring the trigger
        // then leaves no current fault, only the recorded evidence.
        conn.execute(
            "UPDATE cards SET text = ?1 WHERE location = ?2",
            params![original.text, first_address(&root)],
        )
        .expect("restore the text");
        let trigger = trigger_defs(Generation::Two)
            .into_iter()
            .find(|(name, _)| *name == "lock_no_change")
            .map(|(_, sql)| sql)
            .expect("the canonical trigger");
        conn.execute_batch(&trigger).expect("restore the trigger");
        let latched = check(&conn).expect("check");
        assert!(latched.faults.is_empty(), "{:?}", latched.faults);
        assert_eq!(latched.latched.len(), 2, "{:?}", latched.latched);
        let line = scope(&conn).expect("scope").line(&latched);
        assert!(!line.contains("integrity ok"), "{line}");
        assert!(line.contains("2 latched"), "{line}");
        assert!(line.ends_with(&format!("faulted roots: {root}")), "{line}");
    }

    /// R235/R298/S08-AC04: an archive whose Tree Lock record is not installed cannot answer with
    /// counts, so the caller reports `unavailable` instead of a healthy zero-Lock result.
    #[test]
    fn a_scope_without_the_lock_schema_is_an_error_not_a_healthy_zero() {
        let (_temp, mut conn) = archive();
        topic_tree(&mut conn);
        conn.execute_batch(&format!(
            "DELETE FROM metadata WHERE key = '{SCHEMA_KEY}';"
        ))
        .expect("erase the version marker");
        let error = scope(&conn).expect_err("an uninstalled Lock record cannot be read");
        assert!(error.to_string().contains("not installed"), "{error}");
        // The same archive's comparison reports the unreadable record as a fault, never as health.
        assert!(!check(&conn).expect("check").ok());
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
        assert_eq!(refusal.reason, Reason::Edit);
        assert_eq!(refusal.trees, vec![root.clone()]);
        assert_eq!(refusal.cards, vec![first_address(&root)]);

        let pinned = crate::load_card(&conn, "2/1").expect("pinned");
        assert!(ensure_unlocked(&conn, Change::Edit(pinned.row_id)).is_ok());
        let mapping = BTreeMap::from([("2/1".to_string(), "2/3".to_string())]);
        let refusal = ensure_unlocked(&conn, Change::Mapping(&mapping)).expect_err("move pin");
        let refusal = refusal.downcast_ref::<Refusal>().expect("typed refusal");
        assert_eq!(refusal.reason, Reason::PinnedMove);
        assert_eq!(refusal.cards, vec!["2/1".to_string()]);
        assert!(!refusal.sources.is_empty());

        // An unlocked Card may still be edited, and a generated Reverse-link refresh passes.
        assert!(ensure_unlocked(&conn, Change::Edit(pinned.row_id)).is_ok());
        crate::regenerate_reverse_links_tx(&conn).expect("generated refresh");

        // A new successor inside the locked tree is refused at its final address.
        let refusal = ensure_unlocked(&conn, Change::CreateAt(&format!("{root}/2")))
            .expect_err("create inside");
        assert_eq!(
            refusal.downcast_ref::<Refusal>().expect("typed").reason,
            Reason::CreateInside
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
            refusal.downcast_ref::<Refusal>().expect("typed").reason,
            Reason::Delete
        );
        let pinned_plan = crate::delete_plan(&conn, "2/1").expect("plan pinned");
        let refusal = ensure_unlocked(&conn, Change::Delete(&pinned_plan)).expect_err("pinned");
        let refusal = refusal.downcast_ref::<Refusal>().expect("typed");
        assert_eq!(refusal.reason, Reason::PinnedDelete);
        assert!(!refusal.sources.is_empty());
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
            refusal.downcast_ref::<Refusal>().expect("typed").reason,
            Reason::Edit
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
            error.downcast_ref::<Refusal>().expect("typed").reason,
            Reason::MoveInside
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
        let installed = trigger_defs(Generation::Two);
        for (name, sql) in &installed {
            assert!(!sql.contains("zt_authored_fp"), "{name} uses only built-in SQL");
            assert!(!sql.contains("zt_tree_root"), "{name} uses only built-in SQL");
        }
        assert!(
            installed[1].1.contains(crate::card::REVERSE_MARKER),
            "the update trigger compares the authored text before the generated Reverse marker"
        );
        assert!(
            installed[1].1.contains("NEW.term_name    IS NOT OLD.term_name"),
            "the term-aware update trigger protects the Term Card name"
        );
        assert!(
            !TRIGGER_DEFS_V1[1].1.contains("term_name"),
            "generation 1 is compared with its own exact statement"
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
        assert_eq!(refusal.reason, Reason::PinnedMove);
        assert_eq!(refusal.trees, vec![plan.root.clone()]);
        assert!(
            refusal
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
            .expect("create topic")
            .0;
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
            .expect("create topic")
            .0;
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
        for (name, _) in trigger_defs(Generation::Two) {
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
        // S07-AC04/AC05/R303: the declared generation and the protection catalog are read before
        // the first migration, a rebuild's dropped triggers are re-created after the final rename,
        // and both the first-install backup and the guarded comparison come last.
        let generation = boot
            .find("tree_lock::check_generation")
            .expect("the pre-migration generation check");
        let protection = boot
            .find("tree_lock::canonical_protection")
            .expect("the catalog read before the rebuilds");
        let restore = boot
            .find("tree_lock::restore_rebuilt_protection")
            .expect("the post-rebuild restoration");
        let backup = boot
            .find("tree_lock::backup_before_install")
            .expect("the first-install backup");
        assert!(
            generation < first_migration && protection < rebuild,
            "the generation and the existing protection are validated before a migration"
        );
        assert!(
            rebuild < restore && restore < guarded,
            "the protection a rebuild dropped is re-created before the guarded comparison"
        );
        assert!(
            backup < guarded,
            "the consistent backup is taken before the guarded installation"
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
        // A faulted Lock stays active and keeps protecting its retained facts: the owner's review
        // is the only way out, never an automatic refresh or unlock (S06-AC05).
        let member = crate::load_card(&conn, &first).expect("member");
        let refusal = ensure_unlocked(&conn, Change::Edit(member.row_id))
            .expect_err("a faulted Lock still refuses its member's write");
        assert_eq!(
            refusal.downcast_ref::<Refusal>().expect("typed").reason,
            Reason::Edit
        );
        // The panel shows the broken seal beside the same seal the owner reviewed.
        let panel = export_with(&conn, &after).expect("export");
        let listed = &panel["locks"][0];
        assert_eq!(listed["seal"], json!(lock.seal));
        assert_eq!(listed["seal_intact"], false);
        assert_eq!(listed["latched_faults"].as_array().expect("latched").len(), 2);
    }

    /// An archive as Tree Lock generation 1 left it: the term-free `cards` table, the declared
    /// generation 1, and the six generation-1 canonical triggers around one active Lock. Every
    /// column of that release is present, so the promise can still be compared. A boot must keep the
    /// promise across its own authorized Card rebuild and install generation 2 (S07-AC07).
    fn previous_release_archive(conn: &mut Connection) -> (String, Lock) {
        let (root, lock) = locked_topic(conn);
        // The previous release's exact table: today's definition before Term Cards, with `term`
        // absent from the type CHECK and no `term_name` column.
        let previous_table = format!(
            "CREATE TABLE cards_previous (
                row_id INTEGER PRIMARY KEY,
                location TEXT UNIQUE,
                citation_key TEXT UNIQUE,
                is_topic INTEGER NOT NULL CHECK(is_topic IN (0, 1)),
                is_lit INTEGER NOT NULL CHECK(is_lit IN (0, 1)),
                bibtex TEXT,
                text TEXT NOT NULL,
                workspace TEXT NOT NULL DEFAULT 'human' CHECK(workspace IN ('human', 'agent')),
                card_type TEXT NOT NULL DEFAULT 'idea' CHECK(card_type IN ('idea', 'event', 'finding', 'model', 'hub')),
                model_facet TEXT CHECK(model_facet IS NULL OR length(model_facet) BETWEEN 1 AND {}),
                model_emphasis TEXT CHECK(model_emphasis IS NULL OR model_emphasis IN ('core', 'aside')),
                model_level TEXT CHECK(model_level IS NULL OR model_level IN ('concept', 'formal')),
                hub_level TEXT CHECK(hub_level IS NULL OR hub_level IN ('line', 'topic', 'field', 'register')),
                math_form TEXT CHECK(math_form IS NULL OR math_form IN ({})),
                CHECK(
                    (is_lit = 0 AND location IS NOT NULL AND citation_key IS NULL AND bibtex IS NULL)
                    OR
                    (is_lit = 1 AND is_topic = 0 AND location IS NULL AND citation_key IS NOT NULL AND bibtex IS NOT NULL)
                ),
                CHECK(model_emphasis IS NULL OR model_facet IS NOT NULL),
                CHECK(model_level IS NULL OR (card_type = 'model' AND model_facet IS NULL)),
                CHECK(hub_level IS NULL OR card_type = 'hub'),
                CHECK(math_form IS NULL OR (card_type = 'model' AND model_facet IS NULL))
            );",
            crate::model::Facet::MAX_CHARS,
            crate::math_form_values()
        );
        assert!(!previous_table.contains("term"), "{previous_table}");
        let columns = "row_id, location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis, model_level, hub_level, math_form";
        conn.execute_batch(&format!(
            "BEGIN IMMEDIATE;
             {previous_table}
             INSERT INTO cards_previous ({columns}) SELECT {columns} FROM cards;
             DROP TABLE cards;
             ALTER TABLE cards_previous RENAME TO cards;
             CREATE UNIQUE INDEX IF NOT EXISTS cards_citation_key_nocase
                 ON cards(citation_key COLLATE NOCASE)
                 WHERE citation_key IS NOT NULL;
             UPDATE metadata SET value = '{LEGACY_SCHEMA_VERSION}' WHERE key = '{SCHEMA_KEY}';
             COMMIT;"
        ))
        .expect("rebuild the archive into the previous release's generation");
        for (name, sql) in trigger_defs(Generation::One) {
            conn.execute_batch(&sql)
                .unwrap_or_else(|error| panic!("re-create trigger `{name}`: {error}"));
        }
        validate_schema(conn).expect("the previous release's lock tables");
        validate_indexes(conn).expect("the previous release's indexes");
        validate_triggers_for(conn, Generation::One)
            .expect("the previous release left canonical generation-1 triggers");
        (root, lock)
    }

    /// R303/S07-AC02/AC04: the boot sequence validates the stored generation before it repairs or
    /// migrates, so an unsupported generation changes nothing at all.
    #[test]
    fn an_unsupported_generation_refuses_before_any_repair_or_card_migration() {
        let (temp, mut conn) = archive();
        previous_release_archive(&mut conn);
        conn.execute(
            "UPDATE metadata SET value = '3' WHERE key = 'tree_lock_schema_version'",
            [],
        )
        .expect("mark an unsupported generation");
        drop(conn);
        let error = crate::initialize_database(temp.path()).expect_err("unsupported generation");
        let said = format!("{error:#}");
        assert!(
            said.contains("unsupported Tree Lock schema version `3`"),
            "{said}"
        );
        assert!(said.contains("archive was not modified"), "{said}");
        let conn = crate::open_db(temp.path()).expect("open the archive again");
        // No repair and no Card migration ran: the `cards` table keeps its old generation, the
        // version marker still names the unsupported one, and the promise is untouched.
        let definition: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get(0),
            )
            .expect("the table");
        assert!(
            !definition.contains("'term'"),
            "the Card migration must not run before the generation is accepted: {definition}"
        );
        let version: String = conn
            .query_row(
                "SELECT value FROM metadata WHERE key = 'tree_lock_schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("the retained marker");
        assert_eq!(version, "3");
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM locks", [], |row| row.get::<_, i64>(0))
                .expect("locks"),
            1,
            "the promise is retained, never repaired"
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM lock_members", [], |row| row
                .get::<_, i64>(0))
                .expect("members"),
            3
        );
        let error = ensure_unlocked(&conn, Change::CreateAt("9/0"))
            .expect_err("an unsupported generation refuses every write");
        assert!(
            error.to_string().contains("not a supported generation"),
            "{error}"
        );
    }

    /// S07-AC08/R0248 and S17-AC04/R0477: the baseline release reads only generation 1. Running
    /// its exact structural rule against a generation-2 archive that already holds Term Cards,
    /// an exact term name and a Term use refuses before anything is modified, so the generation
    /// check — not a metadata key an old binary never reads — supplies the compatibility floor.
    #[test]
    fn the_baseline_generation_rule_refuses_generation_two_without_modifying_the_archive() {
        let (_temp, mut conn) = archive();
        topic_tree(&mut conn);
        // The new syntax the generation-2 archive can hold: a Term list, a named head and a Term
        // use. A baseline binary would read the macro as an ordinary Link target.
        crate::insert_card_with_term(
            &mut conn,
            "0/1|b",
            false,
            Workspace::Human,
            CardType::Term,
            None,
            None,
            &card_text(
                "术语表",
                "| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |",
            ),
        )
        .expect("the Term list");
        crate::insert_card_with_term(
            &mut conn,
            "0/1|b|a",
            false,
            Workspace::Human,
            CardType::Term,
            None,
            Some("均衡"),
            &card_text(
                "均衡",
                "术语：均衡\n\n类别：概念\n\n## 释义\n\n双方都不改变策略。\n\n来源：[[0/0]] §1.1",
            ),
        )
        .expect("the Term head");
        crate::insert_card(
            &mut conn,
            "0/1|c",
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("使用者", "均衡 [[均衡@0/1|b|a]] 决定价格。"),
        )
        .expect("the Term use");
        let head_before: (Option<String>, String) = conn
            .query_row(
                "SELECT term_name, text FROM cards WHERE location = '0/1|b|a'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("the stored head");
        assert_eq!(head_before.0.as_deref(), Some("均衡"));
        let use_before: String = conn
            .query_row(
                "SELECT text FROM cards WHERE location = '0/1|c'",
                [],
                |row| row.get(0),
            )
            .expect("the stored use");
        let definition: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get(0),
            )
            .expect("the table");
        let rows = crate::card_identity_rows(&conn).expect("rows");
        let error = check_generation_within(&conn, &[LEGACY_SCHEMA_VERSION])
            .expect_err("a baseline binary understands only generation 1");
        assert_eq!(
            error.to_string(),
            "unsupported Tree Lock schema version `2`; archive was not modified"
        );
        assert_eq!(
            conn.query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("the table"),
            definition
        );
        assert_eq!(crate::card_identity_rows(&conn).expect("rows"), rows);
        assert_eq!(
            conn.query_row(
                "SELECT term_name, text FROM cards WHERE location = '0/1|b|a'",
                [],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("the retained head"),
            head_before,
            "the exact term name and the head text are untouched"
        );
        assert_eq!(
            conn.query_row(
                "SELECT text FROM cards WHERE location = '0/1|c'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("the retained use"),
            use_before,
            "the raw Term macro is untouched"
        );
        assert_eq!(
            schema_version(&conn).expect("generation").as_deref(),
            Some(SCHEMA_VERSION)
        );
    }

    /// R0247/S07-AC07: a generation-1 archive whose old protection was altered refuses the upgrade
    /// instead of silently repairing it, and nothing is widened or migrated.
    #[test]
    fn an_altered_generation_one_trigger_refuses_the_upgrade_without_a_repair() {
        let (temp, mut conn) = archive();
        previous_release_archive(&mut conn);
        conn.execute_batch(
            "DROP TRIGGER lock_no_change;
             CREATE TRIGGER lock_no_change BEFORE DELETE ON cards WHEN 0
             BEGIN SELECT RAISE(ABORT, 'zt: card is in a locked tree'); END;",
        )
        .expect("alter the old protection");
        drop(conn);
        let error = crate::initialize_database(temp.path()).expect_err("an altered old trigger");
        let said = format!("{error:#}");
        assert!(
            said.contains("Tree Lock trigger `lock_no_change` has an altered definition"),
            "{said}"
        );
        assert!(said.contains("archive was not modified"), "{said}");
        let conn = crate::open_db(temp.path()).expect("open the archive again");
        assert_eq!(
            schema_version(&conn).expect("generation").as_deref(),
            Some(LEGACY_SCHEMA_VERSION),
            "the declared generation was not moved"
        );
        let definition: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get(0),
            )
            .expect("the table");
        assert!(
            !definition.contains("'term'"),
            "the Card table was not widened: {definition}"
        );
        // The altered trigger is left exactly as it was found: no silent repair.
        let stored: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'lock_no_change'",
                [],
                |row| row.get(0),
            )
            .expect("the altered trigger");
        assert!(stored.contains("BEFORE DELETE"), "{stored}");
    }

    /// Erase the whole Tree Lock schema the way the archive before this feature looked: no four
    /// tables, no indexes, no triggers, no version marker. The Cards stay untouched.
    fn without_lock_schema(conn: &Connection) {        conn.execute_batch(
            "DROP TRIGGER IF EXISTS lock_no_delete;
             DROP TRIGGER IF EXISTS lock_no_change;
             DROP TRIGGER IF EXISTS lock_no_insert;
             DROP TRIGGER IF EXISTS lock_no_move_in;
             DROP TRIGGER IF EXISTS lock_pin_no_delete;
             DROP TRIGGER IF EXISTS lock_pin_no_move;
             DROP INDEX IF EXISTS locks_active_tree;
             DROP INDEX IF EXISTS lock_members_by_row;
             DROP INDEX IF EXISTS lock_pins_by_target;
             DROP INDEX IF EXISTS lock_events_by_lock;
             DROP TABLE IF EXISTS lock_events;
             DROP TABLE IF EXISTS lock_pins;
             DROP TABLE IF EXISTS lock_members;
             DROP TABLE IF EXISTS locks;
             DELETE FROM metadata WHERE key = 'tree_lock_schema_version';",
        )
        .expect("erase the Tree Lock schema");
    }

    /// The `zt.sqlite3.pre-tree-lock-<s>.bak` files of one archive root, newest name first.
    fn first_install_backups(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut names = std::fs::read_dir(root)
            .expect("archive root")
            .map(|entry| entry.expect("entry").path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.starts_with("zt.sqlite3.pre-tree-lock-") && name.ends_with(".bak")
                    })
            })
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    /// R302/S07-AC01/AC09: the first installation of the lock schema backs the prior archive up
    /// through SQLite first, so a failed upgrade can roll back to the archive it found.
    #[test]
    fn the_first_install_backs_up_the_prior_archive_consistently() {
        let (temp, mut conn) = archive();
        topic_tree(&mut conn);
        let rows_before: Vec<(i64, String)> = {
            let mut stmt = conn
                .prepare("SELECT row_id, location FROM cards ORDER BY row_id")
                .expect("cards");
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("rows")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("card rows")
        };
        without_lock_schema(&conn);
        assert!(schema_version(&conn).expect("version").is_none());
        let definition_before: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get(0),
            )
            .expect("the Card table");
        drop(conn);

        crate::initialize_database(temp.path()).expect("first installation");

        let backups = first_install_backups(temp.path());
        assert_eq!(backups.len(), 1, "one backup per first installation: {backups:?}");
        // The backup is the archive before the installation: a consistent SQLite copy with the
        // Cards and no Tree Lock schema.
        let saved = Connection::open_with_flags(
            &backups[0],
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open the backup read-only");
        let integrity: String = saved
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity check");
        assert_eq!(integrity, "ok", "the backup is a consistent SQLite database");
        let saved_rows: Vec<(i64, String)> = {
            let mut stmt = saved
                .prepare("SELECT row_id, location FROM cards ORDER BY row_id")
                .expect("cards");
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("rows")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("card rows")
        };
        assert_eq!(saved_rows, rows_before, "the backup holds every prior Card");
        let saved_locks: bool = saved
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'locks')",
                [],
                |row| row.get(0),
            )
            .expect("lock table");
        assert!(!saved_locks, "the backup is the archive before the installation");
        // The archive itself is installed, and every Card kept its row identity.
        let conn = crate::open_db(temp.path()).expect("open the installed archive");
        assert_eq!(
            schema_version(&conn).expect("version").as_deref(),
            Some(SCHEMA_VERSION)
        );
        validate_triggers(&conn).expect("the canonical triggers are installed");
        let rows_after: Vec<(i64, String)> = {
            let mut stmt = conn
                .prepare("SELECT row_id, location FROM cards ORDER BY row_id")
                .expect("cards");
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("rows")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("card rows")
        };
        assert_eq!(rows_after, rows_before, "the installation kept every row id");
        // S07-AC01: the feature adds tables, indexes and triggers; it never rebuilds `cards` or
        // changes its accepted column generations.
        let definition_after: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get(0),
            )
            .expect("the Card table");
        assert_eq!(
            definition_after, definition_before,
            "the first installation rebuilds no Card table"
        );
        // A second start installs nothing and adds no backup.
        drop(conn);
        crate::initialize_database(temp.path()).expect("second start");
        assert_eq!(
            first_install_backups(temp.path()).len(),
            1,
            "only the first installation backs the archive up"
        );
    }

    /// R302/S07-AC02/AC09: a failed installation changes nothing, keeps its consistent backup, and
    /// the same archive installs once the obstruction is gone.
    #[test]
    fn a_failed_install_keeps_the_prior_archive_retryable_and_its_backup() {
        let (temp, mut conn) = archive();
        topic_tree(&mut conn);
        without_lock_schema(&conn);
        // An obstruction the installation cannot share the namespace with: a `locks` table with a
        // column set this binary cannot read.
        conn.execute_batch("CREATE TABLE locks (obstruction TEXT NOT NULL);")
            .expect("obstruct the installation");
        drop(conn);
        let error = crate::initialize_database(temp.path()).expect_err("the installation fails");
        assert!(
            error.to_string().contains("unsupported column set"),
            "{error:#}"
        );
        let conn = crate::open_db(temp.path()).expect("the prior archive stays usable");
        assert!(
            schema_version(&conn).expect("version").is_none(),
            "no version marker is written for a failed installation"
        );
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT name FROM sqlite_master WHERE type = 'table' AND name IN
                     ('locks','lock_members','lock_pins','lock_events') ORDER BY name",
                )
                .expect("tables");
            stmt.query_map([], |row| row.get::<_, String>(0))
                .expect("table rows")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("table names")
        };
        assert_eq!(tables, vec!["locks".to_string()], "the installation rolled back");
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get::<_, i64>(0))
                .expect("cards"),
            3,
            "the Cards are untouched"
        );
        // The consistent backup is retained and holds the archive before the attempt.
        let backups = first_install_backups(temp.path());
        assert_eq!(backups.len(), 1, "{backups:?}");
        let saved = Connection::open_with_flags(
            &backups[0],
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("open the backup");
        let integrity: String = saved
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .expect("integrity check");
        assert_eq!(integrity, "ok");
        assert_eq!(
            saved
                .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get::<_, i64>(0))
                .expect("saved cards"),
            3
        );
        // The obstruction removed, the same archive installs.
        conn.execute_batch("DROP TABLE locks;")
            .expect("remove the obstruction");
        drop(conn);
        crate::initialize_database(temp.path()).expect("the retry installs");
        let conn = crate::open_db(temp.path()).expect("open the installed archive");
        assert_eq!(
            schema_version(&conn).expect("version").as_deref(),
            Some(SCHEMA_VERSION)
        );
        validate_triggers(&conn).expect("the canonical triggers are installed");
    }

    /// R051/R304/S07-AC05 (T21): a Card-table rebuild keeps every row identity and all Lock
    /// evidence, and the canonical protection it drops with the table is re-created after the final
    /// rename, so an authorized rebuild records no fault and still refuses a member write.
    #[test]
    fn an_authorized_card_rebuild_restores_protection_without_recording_a_fault() {
        let (temp, mut conn) = archive();
        let (root, lock) = previous_release_archive(&mut conn);
        let members_before = stored_members(&conn, lock.lock_id).expect("members");
        let pins_before = stored_pins(&conn, lock.lock_id).expect("pins");
        let rows_before: Vec<(i64, String)> = {
            let mut stmt = conn
                .prepare("SELECT row_id, location FROM cards ORDER BY row_id")
                .expect("cards");
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("rows")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("card rows")
        };
        let revision_before = crate::link_index::revision(&conn).expect("revision");
        // The old generation needs the rebuild this boot performs.
        let definition: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get(0),
            )
            .expect("the table");
        assert!(!definition.contains("'term'"), "{definition}");
        drop(conn);

        crate::initialize_database(temp.path()).expect("the boot rebuilds the Card table");

        let conn = crate::open_db(temp.path()).expect("open the rebuilt archive");
        // The rebuild happened and produced today's term-aware table with one backup.
        let definition: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get(0),
            )
            .expect("the rebuilt table");
        assert!(definition.contains("'term'"), "{definition}");
        assert!(definition.contains("term_name"), "{definition}");
        let rebuild_backups = std::fs::read_dir(temp.path())
            .expect("archive root")
            .filter(|entry| {
                entry.as_ref().expect("entry").file_name().to_string_lossy()
                    .starts_with("zt.sqlite3.pre-term-cards-")
            })
            .count();
        assert_eq!(rebuild_backups, 1, "the rebuild backed the archive up first");
        // Every row identity and the whole Lock evidence survived the table rebuild.
        let rows_after: Vec<(i64, String)> = {
            let mut stmt = conn
                .prepare("SELECT row_id, location FROM cards ORDER BY row_id")
                .expect("cards");
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("rows")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("card rows")
        };
        assert_eq!(rows_after, rows_before, "every row id is preserved");
        assert_eq!(
            stored_members(&conn, lock.lock_id).expect("members"),
            members_before
        );
        assert_eq!(stored_pins(&conn, lock.lock_id).expect("pins"), pins_before);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM locks", [], |row| row.get::<_, i64>(0))
                .expect("locks"),
            1
        );
        // The six generation-2 triggers are back, and the guarded comparison is clean: the loss this
        // boot caused is an authorized rebuild, not tampering.
        assert_eq!(
            schema_version(&conn).expect("generation").as_deref(),
            Some(SCHEMA_VERSION),
            "the term-aware generation is installed with the widened table"
        );
        validate_triggers(&conn).expect("the canonical triggers are installed");
        validate_indexes(&conn).expect("the canonical indexes are installed");
        let integrity = check(&conn).expect("check");
        assert!(
            integrity.faults.is_empty(),
            "{:?}",
            fault_ids(&integrity)
        );
        assert!(
            integrity.latched.is_empty(),
            "an authorized rebuild records no fault: {:?}",
            integrity
                .latched
                .iter()
                .map(|latched| latched.id.clone())
                .collect::<Vec<_>>()
        );
        assert!(integrity.ok(), "{}", integrity.summary());
        assert_eq!(
            lock_events_with(&conn, lock.lock_id, "integrity-fault"),
            0,
            "no integrity-fault event for an authorized rebuild"
        );
        assert_eq!(
            crate::link_index::revision(&conn).expect("revision"),
            revision_before,
            "the rebuild of an unchanged Card table changes no authored fact"
        );
        // The reinstalled backstop protects the promise again.
        let refused = conn
            .execute("DELETE FROM cards WHERE location = ?1", [&root])
            .expect_err("the reinstalled trigger protects the root again");
        assert!(refused.to_string().contains("locked tree"), "{refused}");
        // The promise still matches the rebuilt rows, so the owner keeps reading locked Cards with
        // the same intact seal.
        let reloaded = load_lock(&conn, lock.lock_id)
            .expect("read back")
            .expect("the Lock");
        assert_eq!(reloaded.seal, lock.seal);
        assert_eq!(reloaded.member_count, lock.member_count);
        let panel = export_with(&conn, &integrity).expect("export");
        assert_eq!(panel["locks"][0]["seal_intact"], true);
        // A second start rebuilds nothing and records nothing.
        let cards_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get(0),
            )
            .expect("the rebuilt table");
        drop(conn);
        crate::initialize_database(temp.path()).expect("second start");
        let conn = crate::open_db(temp.path()).expect("open again");
        let cards_sql_again: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get(0),
            )
            .expect("the table");
        assert_eq!(cards_sql_again, cards_sql, "the second start rebuilds nothing");
        assert_eq!(
            lock_events_with(&conn, lock.lock_id, "integrity-fault"),
            0
        );
        assert!(
            check(&conn).expect("check").ok(),
            "the second start stays healthy"
        );
    }

    /// One function's source text, from its signature to the closing brace of the function, with
    /// the checkout's line ending normalized so the search works in either one.
    fn function_body(source: &str, signature: &str) -> String {
        let normalized = source.replace("\r\n", "\n");
        let start = normalized
            .find(signature)
            .unwrap_or_else(|| panic!("`{signature}` is part of the boot sequence"));
        let rest = &normalized[start..];
        let end = rest.find("\n}\n").unwrap_or(rest.len());
        rest[..end].to_string()
    }

    /// Every `.rs` file of the crate, so an audit cannot miss a module by forgetting it.
    fn source_files() -> Vec<(std::path::PathBuf, String)> {
        fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>) {
            for entry in std::fs::read_dir(dir).expect("source directory") {
                let path = entry.expect("source entry").path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|extension| extension == "rs") {
                    out.push((path.clone(), std::fs::read_to_string(&path).expect("source")));
                }
            }
        }
        let mut files = Vec::new();
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        files
    }

    /// R050/S07-AC03: every connection that can write the archive is opened by the one opener that
    /// registers the SQL wrappers, or by the bootstrap path that registers them itself; a read-only
    /// diagnostic says so in its flags. A forgotten opener could write a Card whose trigger has no
    /// fingerprint function.
    #[test]
    fn every_connection_that_can_write_is_opened_by_the_registered_opener() {
        let files = source_files();
        assert!(files.len() > 20, "the audit sees the crate's modules");
        for (path, text) in &files {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("unknown.rs");
            if name == "main.rs" {
                continue;
            }
            let production = text.split("#[cfg(test)]").next().unwrap_or(text);
            assert_eq!(
                production.matches("Connection::open(").count(),
                0,
                "`{name}` must open the archive through crate::open_db: {}",
                path.display()
            );
            let flagged = production.matches("Connection::open_with_flags(").count();
            assert_eq!(
                production.matches("SQLITE_OPEN_READ_ONLY").count(),
                flagged,
                "`{name}` opens a connection without read-only flags: {}",
                path.display()
            );
        }
        let main = include_str!("main.rs");
        assert_eq!(
            main.matches("Connection::open(").count(),
            2,
            "only `open_db` and `initialize_database` open the archive directly"
        );
        // Opening a connection registers the wrappers and does nothing else: no schema, no
        // migration, no writer lock, so opening never recurses into it.
        let open_db = function_body(main, "fn open_db(root: &Path)");
        assert!(
            open_db.contains("tree_lock::register_sql_functions"),
            "the one opener registers the SQL wrappers"
        );
        for forbidden in ["CREATE", "ALTER", "VACUUM", "acquire_edit_lock", "ensure_"] {
            assert!(
                !open_db.contains(forbidden),
                "opening a connection must not run `{forbidden}`: {open_db}"
            );
        }
        let register = function_body(include_str!("tree_lock.rs"), "fn register_sql_functions");
        for forbidden in ["acquire_edit_lock", "execute", "execute_batch", "transaction"] {
            assert!(
                !register.contains(forbidden),
                "registering the SQL wrappers must not run `{forbidden}`"
            );
        }
        // The bootstrap path registers the wrappers before a Card migration can reach a trigger.
        let boot = function_body(main, "fn initialize_database(root: &Path)");
        let registered = boot
            .find("tree_lock::register_sql_functions")
            .expect("the bootstrap registers the wrappers");
        let first_migration = boot
            .find("ensure_workspace_column(&conn, root)")
            .expect("the first Card migration");
        assert!(
            registered < first_migration,
            "the wrappers are registered before a Card migration runs"
        );
        assert!(function_body(main, "fn card_counts").contains("open_db(root)"));
        assert!(function_body(main, "fn archive_facts").contains("SQLITE_OPEN_READ_ONLY"));
    }

    /// The trusted authored-representation reseal of design §14.2, as a test-only fixture: this
    /// release ships no reseal migration and exposes no reseal path (R305–R312, S07-AC06/AC07). It
    /// names the migration, takes a consistent SQLite backup under the caller's edit lock, verifies
    /// the old baselines and refuses a preexisting fault, transforms each member's representation
    /// and writes that member's candidate baseline immediately before its own Card row inside one
    /// transaction, recomputes the affected range seal, appends `reseal` with the old and new
    /// evidence, restores and verifies the canonical protection, and advances the revision once
    /// before commit.
    fn trusted_reseal(
        conn: &mut Connection,
        root: &std::path::Path,
        migration: &str,
        transform: &dyn Fn(&str) -> String,
    ) -> Result<Vec<Lock>> {
        // R306/R307: the migration is named and one consistent backup is taken before it.
        let backup = root.join(format!("zt.sqlite3.pre-{migration}-{}.bak", std::process::id()));
        conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])
            .with_context(|| format!("back up before `{migration}`"))?;
        // R308: verify the old baselines before the transformation; a preexisting fault refuses.
        let integrity = check(conn)?;
        if !integrity.ok() {
            bail!(
                "`{migration}` refuses a preexisting integrity fault: {}",
                integrity.summary()
            );
        }
        let tx = conn.unchecked_transaction()?;
        // The authored rows change inside one transaction, so the statement triggers that protect
        // authored bytes are removed here and restored before the commit (R311).
        for (name, _) in trigger_defs(Generation::Two) {
            tx.execute_batch(&format!("DROP TRIGGER IF EXISTS {name};"))?;
        }
        let mut resealed = Vec::new();
        for lock_id in active_lock_ids(&tx)? {
            let before = load_lock(&tx, lock_id)?.context("the active Lock")?;
            let rows = stored_cards(&tx)?;
            let mut changed = Vec::new();
            for (row_id, address, old_fp) in stored_members(&tx, lock_id)? {
                let card = rows
                    .iter()
                    .find(|card| card.row_id == row_id)
                    .with_context(|| format!("member `{address}` is missing"))?;
                let text = transform(&card.text);
                // R309: the candidate baseline is computed from the transformed representation and
                // written immediately before that row's own transformation.
                let candidate = fingerprint_of_raw(
                    &text,
                    card.bibtex.as_deref(),
                    &card.card_type,
                    card.facet.as_deref(),
                    card.emphasis.as_deref(),
                    card.level.as_deref(),
                    card.hub_level.as_deref(),
                    card.form.as_deref(),
                    card.term_name.as_deref(),
                )?;
                tx.execute(
                    "UPDATE lock_members SET authored_fp = ?1 WHERE lock_id = ?2 AND row_id = ?3",
                    params![candidate, lock_id, row_id],
                )?;
                tx.execute(
                    "UPDATE cards SET text = ?1 WHERE row_id = ?2",
                    params![text, row_id],
                )?;
                changed.push(json!({
                    "row_id": row_id,
                    "address": address,
                    "old": old_fp,
                    "new": candidate,
                }));
            }
            // R310/R311: the addresses and row ids are untouched, the range seal is recomputed from
            // the new baselines, and the old and new evidence is retained as one `reseal` event.
            let members = stored_members(&tx, lock_id)?
                .into_iter()
                .map(|(_, address, fingerprint)| (address, fingerprint))
                .collect::<BTreeMap<_, _>>();
            let seal = seal_of(&members);
            tx.execute(
                "UPDATE locks SET range_fingerprint = ?1 WHERE lock_id = ?2",
                params![seal, lock_id],
            )?;
            append_event(
                &tx,
                lock_id,
                "reseal",
                &json!({
                    "migration": migration,
                    "old_seal": before.seal,
                    "new_seal": seal,
                    "changed": changed,
                }),
            )?;
            resealed.push(lock_id);
        }
        reinstall_triggers(&tx)?;
        validate_indexes(&tx)?;
        validate_triggers(&tx)?;
        // R311: one revision for the whole trusted migration.
        crate::link_index::bump_revision(&tx)?;
        tx.commit()?;
        resealed
            .into_iter()
            .map(|lock_id| load_lock(conn, lock_id)?.context("the resealed Lock"))
            .collect()
    }

    /// The one authored-representation migration the contract test models: each body gains one
    /// restatement block, before the generated Reverse-link section that is not authored.
    fn restated_representation(text: &str) -> String {
        const MARKER: &str = "<!-- zt:reverse-links -->";
        match text.find(MARKER) {
            Some(index) => format!(
                "{}{}",
                text[..index].replacen("\n\n", "\n\n> restated\n\n", 1),
                &text[index..]
            ),
            None => text.replacen("\n\n", "\n\n> restated\n\n", 1),
        }
    }

    /// R305–R311/S07-AC06: the trusted authored-representation reseal keeps every address, row id
    /// and pin meaning, recomputes a matching seal, retains the old/new evidence as one `reseal`
    /// event, restores the canonical protection and advances the revision exactly once.
    #[test]
    fn a_trusted_authored_reseal_keeps_the_promise_and_records_its_evidence() {
        let (temp, mut conn) = archive();
        let (_root, lock) = locked_topic(&mut conn);
        let members_before = stored_members(&conn, lock.lock_id).expect("members");
        let pins_before = stored_pins(&conn, lock.lock_id).expect("pins");
        let rows_before: Vec<(i64, String)> = {
            let mut stmt = conn
                .prepare("SELECT row_id, location FROM cards ORDER BY row_id")
                .expect("cards");
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("rows")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("card rows")
        };
        let revision_before = crate::link_index::revision(&conn).expect("revision");
        assert!(check(&conn).expect("check").ok());

        let resealed = trusted_reseal(
            &mut conn,
            temp.path(),
            "restated-body-v2",
            &restated_representation,
        )
        .expect("the trusted reseal");
        assert_eq!(resealed.len(), 1);
        assert_eq!(resealed[0].lock_id, lock.lock_id);

        // The promise is healthy under the new representation: the baselines and the seal agree.
        let integrity = check(&conn).expect("check");
        assert!(
            integrity.ok(),
            "the resealed promise is healthy: {:?}",
            fault_ids(&integrity)
        );
        let after = load_lock(&conn, lock.lock_id)
            .expect("read back")
            .expect("the Lock");
        assert_ne!(after.seal, lock.seal, "the range seal names the new representation");
        let members_after = stored_members(&conn, lock.lock_id).expect("members");
        assert_eq!(
            members_after
                .iter()
                .map(|(_, address, _)| address.clone())
                .collect::<Vec<_>>(),
            members_before
                .iter()
                .map(|(_, address, _)| address.clone())
                .collect::<Vec<_>>(),
            "every address is preserved"
        );
        assert_eq!(
            members_after
                .iter()
                .map(|(row_id, _, _)| *row_id)
                .collect::<Vec<_>>(),
            members_before
                .iter()
                .map(|(row_id, _, _)| *row_id)
                .collect::<Vec<_>>(),
            "every row id is preserved"
        );
        assert!(
            members_after
                .iter()
                .zip(members_before.iter())
                .all(|(new, old)| new.2 != old.2),
            "every member baseline was recomputed"
        );
        assert_eq!(
            seal_of(
                &members_after
                    .iter()
                    .map(|(_, address, fingerprint)| (address.clone(), fingerprint.clone()))
                    .collect::<BTreeMap<_, _>>()
            ),
            after.seal,
            "the seal is the recomputed range of the new baselines"
        );
        assert_eq!(
            stored_pins(&conn, lock.lock_id).expect("pins"),
            pins_before,
            "pin meaning is preserved"
        );
        // Addresses and row identities of the Cards themselves are untouched.
        let rows_after: Vec<(i64, String)> = {
            let mut stmt = conn
                .prepare("SELECT row_id, location FROM cards ORDER BY row_id")
                .expect("cards");
            stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("rows")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("card rows")
        };
        assert_eq!(rows_after, rows_before);
        // One `reseal` event carries the old and new seal and which members changed.
        let (detail, events): (String, i64) = conn
            .query_row(
                "SELECT (SELECT detail FROM lock_events WHERE lock_id = ?1 AND action = 'reseal'),
                        (SELECT COUNT(*) FROM lock_events WHERE lock_id = ?1 AND action = 'reseal')",
                [lock.lock_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("the reseal evidence");
        assert_eq!(events, 1, "one reseal event");
        let evidence: Value = serde_json::from_str(&detail).expect("reseal detail is JSON");
        assert_eq!(evidence["migration"], "restated-body-v2");
        assert_eq!(evidence["old_seal"], json!(lock.seal));
        assert_eq!(evidence["new_seal"], json!(after.seal));
        let changed = evidence["changed"].as_array().expect("changed members");
        assert_eq!(changed.len(), 3, "every member's representation changed");
        assert!(
            changed
                .iter()
                .all(|member| member["old"].as_str() != member["new"].as_str()
                    && member["old"].as_str().is_some_and(|fp| fp.len() == 64)),
            "{evidence}"
        );
        // One revision for the whole trusted migration.
        assert_eq!(
            crate::link_index::revision(&conn).expect("revision"),
            revision_before + 1
        );
        // The canonical protection is back and refuses an ordinary edit again.
        validate_triggers(&conn).expect("the canonical triggers are restored");
        let refused = conn
            .execute(
                "UPDATE cards SET text = '# Topic\n\nedited\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
                ["0/0"],
            )
            .expect_err("an ordinary edit is refused after the reseal");
        assert!(refused.to_string().contains("locked tree"), "{refused}");
        // The panel keeps the reseal in the Lock's history.
        let panel = export_with(&conn, &integrity).expect("export");
        assert!(
            panel["locks"][0]["events"]
                .as_array()
                .expect("events")
                .iter()
                .any(|event| event["action"] == "reseal"),
            "{panel}"
        );
        assert_eq!(panel["locks"][0]["seal_intact"], true);
    }

    /// R308/S07-AC06: a preexisting integrity fault refuses the reseal, so a trusted migration never
    /// seals over evidence it did not explain.
    #[test]
    fn a_trusted_authored_reseal_refuses_a_preexisting_fault() {
        let (temp, mut conn) = archive();
        let (_root, lock) = locked_topic(&mut conn);
        let root = "0/0";
        let original: String = conn
            .query_row("SELECT text FROM cards WHERE location = ?1", [root], |row| {
                row.get(0)
            })
            .expect("the root text");
        conn.execute_batch("DROP TRIGGER lock_no_change;")
            .expect("tamper as an outside hand");
        conn.execute(
            "UPDATE cards SET text = '# Topic\n\ntampered\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
            [root],
        )
        .expect("tamper");
        let error = trusted_reseal(
            &mut conn,
            temp.path(),
            "restated-body-v2",
            &restated_representation,
        )
        .expect_err("a preexisting fault refuses the reseal");
        assert!(
            error.to_string().contains("preexisting integrity fault"),
            "{error}"
        );
        // The refused migration changed nothing: no reseal evidence, no new seal, no revision.
        assert_eq!(lock_events_with(&conn, lock.lock_id, "reseal"), 0);
        assert_eq!(
            load_lock(&conn, lock.lock_id)
                .expect("read back")
                .expect("the Lock")
                .seal,
            lock.seal
        );
        assert_eq!(
            conn.query_row("SELECT text FROM cards WHERE location = ?1", [root], |row| {
                row.get::<_, String>(0)
            })
            .expect("the tampered text"),
            "# Topic\n\ntampered\n\n<!-- zt:reverse-links -->\n"
        );
        // The owner's repair, not a reseal, is what makes the promise healthy again.
        conn.execute("UPDATE cards SET text = ?1 WHERE location = ?2", params![original, root])
            .expect("restore the authored text");
        let canonical = trigger_defs(Generation::Two)
            .into_iter()
            .find(|(name, _)| *name == "lock_no_change")
            .map(|(_, sql)| sql)
            .expect("the canonical update trigger");
        conn.execute_batch(&format!(
            "DROP TRIGGER IF EXISTS lock_no_change;
             {canonical}"
        ))
        .expect("restore the canonical trigger");
        assert!(check(&conn).expect("check").ok());
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM lock_events WHERE action = 'reseal'", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("reseal events"),
            0
        );
    }

    /// R312/S07-AC07: no public reseal command and no guard-disable flag ship with this release. The
    /// `reseal` action exists only as the retained event kind, and the writer gate reads no
    /// environment switch or bypass.
    #[test]
    fn no_public_reseal_command_or_guard_bypass_is_shipped() {
        for (name, source) in [
            ("main.rs", include_str!("main.rs")),
            ("gui.rs", include_str!("gui.rs")),
            ("mcp.rs", include_str!("mcp.rs")),
            ("session/command.rs", include_str!("session/command.rs")),
        ] {
            assert!(
                !source.contains("\"reseal\""),
                "`{name}` must not dispatch a reseal"
            );
            assert!(
                !source.contains("--reseal"),
                "`{name}` must not offer a reseal flag"
            );
            assert!(
                !source.contains("ZT_LOCK_BYPASS") && !source.contains("without_guard"),
                "`{name}` must not offer a guard bypass"
            );
        }
        let tree_lock = include_str!("tree_lock.rs");
        let gate = tree_lock.split("#[cfg(test)]").next().unwrap_or(tree_lock);
        assert!(
            gate.contains("'reseal'") && gate.contains("'integrity-fault'"),
            "the stored event vocabulary keeps the reseal action"
        );
        for switch in ["env::var", "var_os", "std::env", "ignore_check_constraints"] {
            assert!(
                !gate.contains(switch),
                "the writer gate and the reseal vocabulary read no `{switch}` switch"
            );
        }
        assert!(
            !include_str!("main.rs").contains("\"--lock\"")
                && include_str!("main.rs").contains("\"--check-locks\""),
            "the only Lock comparison the CLI offers is `--check-locks`"
        );
    }

    /// R313/S07-AC08: an older client's connection has no SQL wrappers. It can still read the
    /// archive, and the built-in triggers still refuse its locked Card write, so an upgraded archive
    /// is never handed back to an old executable as a supported writer.
    #[test]
    fn a_connection_without_the_sql_wrappers_reads_but_cannot_write_a_locked_card() {
        let (_temp, mut conn) = archive();
        let (root, _lock) = locked_topic(&mut conn);
        let path = conn.path().expect("database path").to_string();
        // The connection an older binary opens: plain SQLite, no registered wrappers.
        let old = Connection::open(&path).expect("an older client's connection");
        let cards: i64 = old
            .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
            .expect("Cards stay readable without the wrappers");
        assert_eq!(cards, 3);
        let missing = old
            .query_row("SELECT zt_tree_root('1/0')", [], |row| row.get::<_, String>(0))
            .expect_err("an older client has no fingerprint or tree-root wrapper");
        assert!(
            missing
                .to_string()
                .to_lowercase()
                .contains("no such function"),
            "{missing}"
        );
        // The six triggers use built-in SQL, so they still refuse the old client's authored write.
        let refused = old
            .execute(
                "UPDATE cards SET text = '# Topic\n\nrewritten\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
                [&root],
            )
            .expect_err("the built-in trigger refuses the older client's write");
        assert!(refused.to_string().contains("locked tree"), "{refused}");
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

    // -----------------------------------------------------------------------------------------
    // S13 (#136): the versioned dump export of Lock history and integrity evidence.
    // -----------------------------------------------------------------------------------------

    /// R315/R317/R322, S13-AC02: an archive that holds no Lock still exports `schema_version` 1, the
    /// archive identity, the captured revision and empty Lock/event arrays beside the integrity facts,
    /// and the serialized bytes of two runs of the same view are identical.
    #[test]
    fn dump_export_answers_version_one_with_empty_evidence_at_the_archive_identity() {
        let (_temp, conn) = archive();
        let export = dump_export(&conn).expect("export");
        assert_eq!(
            serde_json::to_string(&export).expect("serialize"),
            format!(
                r#"{{"archive":"{}","events":[],"integrity":{{"faulted":false,"faults":[],"latched":[],"ok":true,"summary":"integrity ok"}},"locks":[],"revision":0,"schema_version":1}}"#,
                archive_identity(&conn).expect("identity")
            )
        );
        assert_eq!(
            serde_json::to_string(&dump_export(&conn).expect("export")).expect("serialize"),
            serde_json::to_string(&export).expect("serialize"),
            "one captured state serializes to one deterministic document"
        );
    }

    /// R318–R321, S13-AC02: one active Lock exports its record, the member snapshots with addresses,
    /// row identities and authored baselines, its pins with both addresses and both rows, the ordered
    /// event list and the integrity facts — and writing the export changes no archive fact.
    #[test]
    fn dump_export_carries_the_active_lock_members_pins_events_and_integrity() {
        let (_temp, mut conn) = archive();
        let (_outside, target) = plain_tree(&mut conn, "Outside");
        let root = linking_tree(&mut conn, "Frozen", &target);
        let plan = resolve_plan(&conn, &root, Some("frozen for review")).expect("plan");
        let tx = conn.transaction().expect("transaction");
        let lock = lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        let revision = crate::link_index::revision(&conn).expect("revision");

        let export = dump_export(&conn).expect("export");

        assert_eq!(export["schema_version"], json!(1));
        assert_eq!(
            export["archive"],
            json!(archive_identity(&conn).expect("identity"))
        );
        assert_eq!(export["revision"], json!(revision));
        assert_eq!(export["integrity"]["ok"], json!(true));
        let locks = export["locks"].as_array().expect("locks array");
        assert_eq!(locks.len(), 1);
        assert_eq!(locks[0]["lock_id"], json!(lock.lock_id));
        assert_eq!(locks[0]["root"], json!(root));
        assert_eq!(locks[0]["tree"], json!(crate::tree_id(&root)));
        assert_eq!(locks[0]["workspace"], json!("human"));
        assert_eq!(locks[0]["note"], json!("frozen for review"));
        assert_eq!(locks[0]["member_count"], json!(2));
        assert_eq!(locks[0]["unlocked_at"], Value::Null);
        assert_eq!(locks[0]["seal"], json!(lock.seal));
        assert_eq!(locks[0]["root_row"], json!(row_of(&conn, &root)));
        let member_rows = conn
            .prepare("SELECT address, row_id, authored_fp FROM lock_members WHERE lock_id = ?1 ORDER BY address")
            .expect("member query")
            .query_map([lock.lock_id], |row| {
                Ok(json!({
                    "address": row.get::<_, String>(0)?,
                    "row_id": row.get::<_, i64>(1)?,
                    "fingerprint": row.get::<_, String>(2)?,
                }))
            })
            .expect("members")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("member rows");
        assert_eq!(locks[0]["members"], json!(member_rows));
        assert_eq!(
            member_rows
                .iter()
                .map(|member| member["address"].clone())
                .collect::<Vec<_>>(),
            [json!(root), json!(format!("{}/1", crate::tree_id(&root)))],
            "members are ordered by address"
        );
        assert_eq!(
            locks[0]["pins"],
            json!([{
                "source": format!("{}/1", crate::tree_id(&root)),
                "source_row": row_of(&conn, &format!("{}/1", crate::tree_id(&root))),
                "target": target,
                "target_row": row_of(&conn, &target),
            }])
        );
        let events = export["events"].as_array().expect("events array");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["event_id"], json!(1));
        assert_eq!(events[0]["lock_id"], json!(lock.lock_id));
        assert_eq!(events[0]["action"], json!("lock"));
        assert_eq!(events[0]["detail"]["member_count"], json!(2));
        assert_eq!(events[0]["detail"]["seal"], json!(lock.seal));
        assert_eq!(events[0]["detail"]["warnings"], json!([]));

        assert_eq!(
            crate::link_index::revision(&conn).expect("revision"),
            revision,
            "the export advances no revision and mutates no source fact"
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM lock_events", [], |row| row
                .get::<_, i64>(0))
                .expect("events"),
            1
        );
    }

    /// R322/R323, S13-AC05: a fault is exported as evidence of the captured state and never refuses a
    /// diagnostic dump; the Lock record, its baseline and its event stay in the same document.
    #[test]
    fn dump_export_exports_a_fault_as_evidence_and_still_answers() {
        let (_temp, mut conn) = archive();
        let (root, lock) = locked_topic(&mut conn);
        let baseline = export_members(&conn, lock.lock_id).expect("baseline");
        conn.execute_batch("DROP TRIGGER lock_no_change")
            .expect("drop trigger");
        conn.execute(
            "UPDATE cards SET text = '# First\n\nchanged body\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
            [first_address(&root)],
        )
        .expect("tamper");

        let export = dump_export(&conn).expect("a fault is evidence, not a failed export");

        assert_eq!(export["integrity"]["ok"], json!(false));
        assert_eq!(export["integrity"]["faulted"], json!(true));
        let faults = export["integrity"]["faults"].as_array().expect("faults");
        let kinds = faults
            .iter()
            .map(|fault| fault["kind"].clone())
            .collect::<Vec<_>>();
        assert!(kinds.contains(&json!("trigger-missing")), "{kinds:?}");
        assert!(kinds.contains(&json!("fingerprint-changed")), "{kinds:?}");
        assert!(
            faults.iter().all(|fault| {
                fault["lock_id"].is_null() || fault["lock_id"] == json!(lock.lock_id)
            }),
            "a fault names the Lock it leaves unprotected, or none when it is a schema fault"
        );
        let locks = export["locks"].as_array().expect("locks array");
        assert_eq!(locks.len(), 1, "a fault never hides the Lock record");
        assert_eq!(locks[0]["members"], json!(baseline));
        assert_eq!(
            export["events"].as_array().expect("events").len(),
            1,
            "the history stays readable beside the fault"
        );
    }

    /// R323, S13-AC05: lock data that cannot be read fails the export, so the dump fails instead of
    /// writing a document that claims an archive held no Lock.
    #[test]
    fn dump_export_fails_when_required_lock_data_cannot_be_read() {
        let (_temp, mut conn) = archive();
        locked_topic(&mut conn);
        conn.execute_batch("DROP TABLE lock_events")
            .expect("drop the stored history");

        let error = dump_export(&conn).expect_err("unreadable history fails the export");

        assert!(
            format!("{error:#}").contains("lock_events"),
            "the error names the unreadable lock data: {error:#}"
        );
    }

    /// R315/R316, S13-AC02: an archive whose Tree Lock schema was never installed exports the same
    /// version-1 document with empty Lock and event arrays, and its integrity block reports why.
    #[test]
    fn dump_export_answers_an_archive_without_the_lock_schema() {
        let (_temp, conn) = archive();
        conn.execute_batch(
            "DELETE FROM metadata WHERE key = 'tree_lock_schema_version';
             DROP TRIGGER lock_no_delete;
             DROP TRIGGER lock_no_change;
             DROP TRIGGER lock_no_insert;
             DROP TRIGGER lock_no_move_in;
             DROP TRIGGER lock_pin_no_delete;
             DROP TRIGGER lock_pin_no_move;
             DROP TABLE lock_events;
             DROP TABLE lock_pins;
             DROP TABLE lock_members;
             DROP TABLE locks;",
        )
        .expect("remove the lock schema");

        let export = dump_export(&conn).expect("an archive without Locks still dumps");

        assert_eq!(export["schema_version"], json!(1));
        assert_eq!(export["locks"], json!([]));
        assert_eq!(export["events"], json!([]));
        assert_eq!(export["integrity"]["ok"], json!(false));
        assert_eq!(
            export["integrity"]["faults"][0]["kind"],
            json!("schema-version")
        );
    }

    /// R323, S13-AC02: several Locks export in Lock-id order, their events in one event-id order
    /// across Locks, and each Lock's pins from both of its sources in source-address order — so two
    /// runs of one captured state serialize the same bytes.
    #[test]
    fn dump_export_orders_several_locks_their_events_and_their_pins_deterministically() {
        let (_temp, mut conn) = archive();
        let (_outside, target) = plain_tree(&mut conn, "Outside");
        let first_root = linking_tree(&mut conn, "First", &target);
        let second_root = linking_tree(&mut conn, "Second", &target);
        for root in [&first_root, &second_root] {
            crate::insert_card(
                &mut conn,
                &side_address(root),
                false,
                Workspace::Human,
                CardType::Idea,
                None,
                &card_text("Side", &format!("links [[{target}]]")),
            )
            .expect("side card");
        }
        let first_lock = lock_rows(&mut conn, &first_root);
        let second_lock = lock_rows(&mut conn, &second_root);
        let tx = conn.transaction().expect("transaction");
        note_tx(&tx, first_lock, Some("reviewed")).expect("note");
        tx.commit().expect("commit");

        let export = dump_export(&conn).expect("export");

        let locks = export["locks"].as_array().expect("locks array");
        assert_eq!(
            locks
                .iter()
                .map(|lock| lock["lock_id"].clone())
                .collect::<Vec<_>>(),
            [json!(first_lock), json!(second_lock)],
            "Locks are ordered by Lock id"
        );
        for lock in locks {
            let first = first_address(lock["root"].as_str().expect("root"));
            assert_eq!(
                lock["pins"]
                    .as_array()
                    .expect("pins")
                    .iter()
                    .map(|pin| pin["source"].clone())
                    .collect::<Vec<_>>(),
                [json!(first), json!(format!("{first}|a"))],
                "one target pinned by two sources is ordered by source address"
            );
        }
        let events = export["events"].as_array().expect("events array");
        assert_eq!(
            events
                .iter()
                .map(|event| event["action"].clone())
                .collect::<Vec<_>>(),
            [json!("lock"), json!("lock"), json!("note")]
        );
        let ids = events
            .iter()
            .map(|event| event["event_id"].as_i64().expect("event id"))
            .collect::<Vec<_>>();
        assert!(
            ids.windows(2).all(|pair| pair[0] < pair[1]),
            "one event-id order across every Lock: {ids:?}"
        );
        assert_eq!(
            serde_json::to_string(&dump_export(&conn).expect("export")).expect("serialize"),
            serde_json::to_string(&export).expect("serialize"),
            "two runs of one captured state serialize the same bytes"
        );
    }

    /// R104/R321/R323, S13-AC03: an ended Lock exports its record, its ordered events and its
    /// baseline snapshot, which the initial `lock` event retains after unlock deletes the live rows
    /// and the old tree is deleted. Row ids and addresses there are the captured database's evidence.
    #[test]
    fn dump_export_keeps_an_ended_locks_baseline_after_its_tree_is_gone() {
        let (_temp, mut conn) = archive();
        let (_outside, target) = plain_tree(&mut conn, "Outside");
        let root = linking_tree(&mut conn, "Frozen", &target);
        let plan = resolve_plan(&conn, &root, None).expect("plan");
        let tx = conn.transaction().expect("transaction");
        let lock = lock_tx(&tx, &plan).expect("lock");
        tx.commit().expect("commit");
        let baseline = export_members(&conn, lock.lock_id).expect("baseline");
        assert_eq!(baseline.len(), 2);
        let tx = conn.transaction().expect("transaction");
        unlock_tx(&tx, lock.lock_id).expect("unlock");
        tx.commit().expect("commit");
        let plan = crate::delete_plan(&conn, &root).expect("delete plan");
        crate::apply_delete_plan(&mut conn, plan).expect("delete the old tree");

        let export = dump_export(&conn).expect("export");

        let locks = export["locks"].as_array().expect("locks array");
        assert_eq!(locks.len(), 1, "the ended Lock keeps its record");
        assert_eq!(locks[0]["lock_id"], json!(lock.lock_id));
        assert_eq!(locks[0]["root"], json!(root));
        assert_eq!(locks[0]["seal"], json!(lock.seal));
        assert!(
            locks[0]["unlocked_at"].is_string(),
            "the record carries the end of the Lock"
        );
        assert_eq!(locks[0]["members"], json!([]), "unlock removed the live rows");
        assert_eq!(locks[0]["pins"], json!([]));
        let events = export["events"].as_array().expect("events array");
        assert_eq!(
            events
                .iter()
                .map(|event| event["action"].clone())
                .collect::<Vec<_>>(),
            [json!("lock"), json!("unlock")]
        );
        assert!(
            events[0]["event_id"].as_i64() < events[1]["event_id"].as_i64(),
            "events are in event-id order"
        );
        assert_eq!(events[0]["detail"]["members"], json!(baseline));
        assert_eq!(events[0]["detail"]["seal"], json!(lock.seal));
        assert_eq!(
            events[0]["detail"]["pins"][0]["target"],
            json!(target),
            "the retained pin keeps both captured addresses"
        );
        assert_eq!(
            events[0]["detail"]["pins"][0]["sources"],
            json!([format!("{}/1", crate::tree_id(&root))])
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM cards WHERE location LIKE ?1",
                [format!("{}/%", crate::tree_id(&root))],
                |row| row.get::<_, i64>(0)
            )
            .expect("cards"),
            0,
            "the old tree really is gone, so the export is the only record left"
        );
    }

    // --- Term uses in Lock promises (S11, #153) ------------------------------------------------

    const TERM_TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1 |\n";

    /// A fresh Topic holding a Term list at `<tree>/1|a` and its head `<tree>/1|a|a`.
    fn topic_with_term_head(conn: &mut Connection) -> (String, String, String) {
        let root = crate::create_topic(conn, Workspace::Human, &card_text("第二主题", "正文"))
            .expect("create topic")
            .0;
        let tree = crate::tree_id(&root).to_string();
        let first = format!("{tree}/1");
        crate::insert_card(
            conn,
            &first,
            false,
            Workspace::Human,
            CardType::Idea,
            None,
            &card_text("第一张", "正文"),
        )
        .expect("first card");
        let list = format!("{first}|a");
        crate::insert_card(
            conn,
            &list,
            false,
            Workspace::Human,
            CardType::Term,
            None,
            &card_text("术语表", TERM_TABLE),
        )
        .expect("term list");
        let head = format!("{list}|a");
        crate::insert_card_with_term(
            conn,
            &head,
            false,
            Workspace::Human,
            CardType::Term,
            None,
            Some("均衡"),
            &card_text("均衡", ""),
        )
        .expect("term head");
        (root, list, head)
    }

    /// R0268/S11-AC05: a Term use is an authored macro, so it pins a head in another tree like any
    /// Link, while a Term use to a head inside the same tree pins nothing.
    #[test]
    fn a_term_use_pins_a_cross_tree_head_and_a_same_tree_use_pins_nothing() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        let (other_root, _list, head) = topic_with_term_head(&mut conn);
        let side = crate::load_card(&conn, &side_address(&root)).expect("side");
        crate::update_card_text(
            &mut conn,
            &side,
            &card_text("Side", &format!("术语 [[均衡@{head}]]")),
        )
        .expect("write the Term use");

        let plan = resolve_plan(&conn, &root, Some("term pin")).expect("plan");
        assert_eq!(plan.pins.len(), 1, "one distinct cross-tree head");
        assert_eq!(plan.pins[0].target, head);
        assert_eq!(plan.pins[0].sources.len(), 1);
        assert_eq!(plan.pins[0].sources[0].1, side_address(&root));
        assert!(
            plan.warnings.is_empty(),
            "the head resolves, so the Term use is no warning"
        );

        // The same-tree case: the second topic's first Card uses its own tree's head.
        let first = format!("{}/1", crate::tree_id(&other_root));
        let card = crate::load_card(&conn, &first).expect("first");
        crate::update_card_text(
            &mut conn,
            &card,
            &card_text("第一张", &format!("[[均衡@{head}]]")),
        )
        .expect("write the same-tree use");
        let plan = resolve_plan(&conn, &other_root, Some("same tree")).expect("plan");
        assert!(
            plan.pins.is_empty(),
            "a same-tree Term use creates no external pin"
        );
        assert!(plan.warnings.is_empty());
    }

    /// R0268/S11-AC05: a move that would rewrite a Locked source's Term use is refused, exactly as
    /// for an ordinary Link. The use is a warning when the Lock is accepted because its target does
    /// not resolve yet; the target then resolves elsewhere and may not be renamed under the Lock.
    #[test]
    fn a_locked_source_refuses_a_rewrite_of_its_term_use() {
        let (_temp, mut conn) = archive();
        let root = topic_tree(&mut conn);
        let future_head = "1/1|a|a".to_string();
        let side = crate::load_card(&conn, &side_address(&root)).expect("side");
        // Seed a Term use to an address that holds no Card yet: the Lock accepts it as a warning.
        conn.execute(
            "UPDATE cards SET text = ?1 WHERE row_id = ?2",
            params![
                card_text("Side", &format!("[[均衡@{future_head}]]")),
                side.row_id
            ],
        )
        .expect("seed the Term use");
        crate::link_index::rebuild(&conn).expect("index the seeded use");
        let plan = resolve_plan(&conn, &root, Some("locked before the head exists")).expect("plan");
        assert!(plan.pins.is_empty(), "an unresolved Term use pins nothing");
        assert_eq!(plan.warnings.len(), 1);
        assert_eq!(plan.warnings[0].target, future_head);
        lock_tx(&conn, &plan).expect("apply the Lock");

        // The head now exists in an unlocked tree, at the address the locked source names.
        let (other_root, _list, head) = topic_with_term_head(&mut conn);
        assert_eq!(head, future_head);
        let mapping = BTreeMap::from([(head.clone(), format!("{head}|b"))]);
        let error = ensure_unlocked(&conn, Change::Mapping(&mapping))
            .expect_err("a Locked source's Term use may not be rewritten");
        let refusal = error.downcast_ref::<Refusal>().expect("a typed refusal");
        assert_eq!(refusal.reason.id(), "edit");
        assert!(
            refusal.message.contains("locked tree")
                && refusal.cards.contains(&side_address(&root)),
            "{refusal:?}"
        );
        let _ = other_root;
    }
}