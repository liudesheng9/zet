//! Transactional, derived index of authored Link occurrences.
//!
//! Bodies are authoritative. The full parser remains the migration/reindex
//! oracle; normal writes replace only one source and refresh affected targets.
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

type Row = (String, i64, String, Option<String>);
const INDEX_VERSION: &str = "1";

pub(crate) fn ready(conn: &Connection) -> bool {
    matches!(conn.query_row("SELECT value FROM metadata WHERE key='links_index_version'", [], |r| r.get::<_, String>(0)), Ok(v) if v == INDEX_VERSION)
}

pub(crate) fn revision(conn: &Connection) -> Result<u64> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata WHERE key='archive_revision'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    value
        .unwrap_or_else(|| "0".into())
        .parse()
        .context("invalid archive revision")
}

pub(crate) fn bump_revision(conn: &Connection) -> Result<()> {
    conn.execute("INSERT INTO metadata(key,value) VALUES('archive_revision','1') ON CONFLICT(key) DO UPDATE SET value=CAST(CAST(value AS INTEGER)+1 AS TEXT)", [])?;
    Ok(())
}

fn rows(card: &crate::Card) -> Result<Vec<Row>> {
    let parsed =
        crate::parse_card_text(&card.text).with_context(|| format!("Card `{}`", card.address))?;
    let body = crate::link::LinkBody::parse(&parsed.body)
        .with_context(|| format!("Card `{}`", card.address))?;
    let reads = crate::model::read_links(&parsed.body, str::to_string);
    debug_assert_eq!(
        body.occurrences().len(),
        reads.len(),
        "one LinkRead per occurrence"
    );
    Ok(body
        .occurrences()
        .iter()
        .zip(reads)
        .enumerate()
        .map(|(ord, (occurrence, read))| {
            // A Term use is its own kind, so a reader can tell it from a reasoning Link (design
            // 10.3, S11 builds its Term-uses section on this row).
            let kind = match occurrence.form {
                crate::link::LinkForm::Term => Some("term".to_string()),
                crate::link::LinkForm::Ordinary => {
                    (read.kind != crate::model::RelationKind::Refers)
                        .then(|| read.kind.id().to_string())
                }
            };
            (card.address.clone(), ord as i64, occurrence.target.to_string(), kind)
        })
        .collect())
}

pub(crate) fn ensure(root: &Path, conn: &mut Connection) -> Result<()> {
    if ready(conn) {
        return Ok(());
    }
    let _lock = crate::acquire_edit_lock_within(root, std::time::Duration::from_secs(20), "reindex")?;
    if ready(conn) {
        return Ok(());
    }
    let version: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata WHERE key='links_index_version'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    if version.is_some() {
        bail!("unsupported Link index version; archive was not modified");
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let backup = root.join(format!("zt.sqlite3.pre-link-index-{stamp}.bak"));
    conn.execute("VACUUM INTO ?1", [backup.to_string_lossy().as_ref()])
        .context("back up archive before Link index migration")?;
    let saved = Connection::open_with_flags(&backup, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = saved.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if integrity != "ok" {
        bail!("Link index backup integrity check failed: {integrity}");
    }
    drop(saved);
    let tx = conn.transaction()?;
    tx.execute_batch("CREATE TABLE links(source TEXT NOT NULL, ord INTEGER NOT NULL CHECK(ord>=0), target TEXT NOT NULL, kind TEXT, PRIMARY KEY(source,ord)); CREATE INDEX links_by_target ON links(target,source);")?;
    let repaired = rebuild(&tx)?;
    let mut identity = [0u8; 16];
    getrandom::fill(&mut identity).map_err(|e| anyhow::anyhow!("archive identity: {e}"))?;
    let identity: String = identity.iter().map(|b| format!("{b:02x}")).collect();
    tx.execute(
        "INSERT OR IGNORE INTO metadata(key,value) VALUES('archive_id',?1)",
        [&identity],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO metadata(key,value) VALUES('archive_revision','0')",
        [],
    )?;
    // A migration that repairs a stored generated section changes Cards, so the
    // read view a reader already holds must change with it.
    if repaired > 0 {
        bump_revision(&tx)?;
    }
    tx.execute(
        "INSERT INTO metadata(key,value) VALUES('links_index_version',?1)",
        [INDEX_VERSION],
    )?;
    tx.commit()?;
    crate::log_event(
        root,
        &format!("Link index migration complete; backup {}", backup.display()),
    )?;
    Ok(())
}

/// Full rebuild for migrations and structural operations, within their transaction.
/// Only changed stored sections are written. Never parse generated sections.
pub(crate) fn rebuild(conn: &Connection) -> Result<usize> {
    let cards = crate::load_cards(conn)?;
    let expected = cards
        .iter()
        .map(rows)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    conn.execute("DELETE FROM links", [])?;
    let mut insert =
        conn.prepare_cached("INSERT INTO links(source,ord,target,kind) VALUES(?1,?2,?3,?4)")?;
    for (source, ord, target, kind) in expected {
        insert.execute(params![source, ord, target, kind])?;
    }
    drop(insert);
    let affected = cards.iter().map(|c| c.address.clone()).collect();
    let changed = refresh_reverse(conn, &affected)?;
    write_stats(conn, cards.len(), affected.len(), changed)?;
    Ok(changed)
}

fn write_stats(conn: &Connection, indexed: usize, refreshed: usize, changed: usize) -> Result<()> {
    let value = json!({"indexed_cards":indexed,"refreshed_cards":refreshed,"changed_reverse_sections":changed});
    conn.execute("INSERT INTO metadata(key,value) VALUES('link_write_stats',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [value.to_string()])?;
    Ok(())
}

/// Finalize a single authored source inside its caller's transaction.
pub(crate) fn source_changed(conn: &Connection, at: &str) -> Result<()> {
    sync_source(conn, at)?;
    if ready(conn) {
        bump_revision(conn)?;
    }
    Ok(())
}

/// A batch/edit owns the revision boundary and calls this before its commit.
pub(crate) fn sync_source(conn: &Connection, at: &str) -> Result<()> {
    sync_sources(conn, &BTreeSet::from([at.to_string()]))
}

/// Refresh the union of old/new targets once for one transaction's sources.
pub(crate) fn sync_sources(conn: &Connection, sources: &BTreeSet<String>) -> Result<()> {
    if !ready(conn) {
        return crate::regenerate_reverse_links_tx(conn);
    }
    let mut affected = BTreeSet::new();
    for at in sources {
        let card = crate::load_card(conn, at)?;
        let mut query = conn.prepare_cached(
            "SELECT source,ord,target,kind FROM links WHERE source=?1 ORDER BY ord",
        )?;
        let old = query
            .query_map([at], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let new = rows(&card)?;
        affected.extend(old.iter().chain(&new).map(|r| r.2.clone()));
        affected.insert(at.to_string());
        if old != new {
            conn.execute("DELETE FROM links WHERE source=?1", [at])?;
            let mut insert = conn
                .prepare_cached("INSERT INTO links(source,ord,target,kind) VALUES(?1,?2,?3,?4)")?;
            for (source, ord, target, kind) in &new {
                insert.execute(params![source, ord, target, kind])?;
            }
        }
    }
    let changed = refresh_reverse(conn, &affected)?;
    write_stats(conn, sources.len(), affected.len(), changed)
}

/// Validate only the referenced targets; generated text is never authoritative. The rules are the
/// root Link wrapper's: an ordinary Link needs an existing target, and a new Term use needs a Term
/// head with the legacy multiplicity exception (S10, #151).
pub(crate) fn validate(conn: &Connection, body: &str, old_body: Option<&str>) -> Result<()> {
    let exists = |at: &str| -> Result<bool> {
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM cards WHERE location=?1 OR citation_key=?1)",
            [at],
            |r| r.get(0),
        )?)
    };
    let is_head = |at: &str| crate::term::is_head(conn, at);
    crate::link::validate(body, old_body, exists, is_head)
}

fn refresh_reverse(conn: &Connection, affected: &BTreeSet<String>) -> Result<usize> {
    let mut changed = 0;
    for target in affected {
        let Some(card) = optional_card(conn, target)? else {
            continue;
        };
        // A Term use is no reasoning Link: its own "Term uses" section is design 10.3's later
        // slice (S11, #153), so the ordinary Reverse links skip its rows here.
        let mut sources = conn.prepare_cached(
            "SELECT DISTINCT l.source,c.text FROM links l
             JOIN cards c ON (c.location=l.source OR c.citation_key=l.source)
             WHERE l.target=?1 AND (l.kind IS NULL OR l.kind <> 'term') ORDER BY l.source",
        )?;
        let linked = sources
            .query_map([target], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let linked = linked
            .into_iter()
            .map(|(address, text)| Ok((address, crate::parse_card_text(&text)?.title)))
            .collect::<Result<Vec<_>>>()?;
        let reverse =
            crate::card::reverse_section(linked.iter().map(|(a, t)| (a.as_str(), t.as_str())));
        let text = crate::card::with_reverse_section(&card.text, &reverse)?;
        if text != card.text {
            changed += conn.execute(
                "UPDATE cards SET text=?1 WHERE row_id=?2",
                params![text, card.row_id],
            )?;
        }
    }
    Ok(changed)
}

fn optional_card(conn: &Connection, at: &str) -> Result<Option<crate::Card>> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cards WHERE location=?1 OR citation_key=?1)",
        [at],
        |r| r.get(0),
    )?;
    if exists {
        Ok(Some(crate::load_card(conn, at)?))
    } else {
        Ok(None)
    }
}

/// Compare every indexed occurrence and stored generated section to a fresh parse.
pub(crate) fn check(conn: &Connection) -> Result<Value> {
    if !ready(conn) {
        bail!("Link index is not initialized; run a compatible `zt up` migration");
    }
    let cards = crate::load_cards(conn)?;
    let mut expected = cards
        .iter()
        .map(rows)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    expected.sort();
    let mut stmt = conn.prepare("SELECT source,ord,target,kind FROM links ORDER BY source,ord")?;
    let mut actual = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    actual.sort();
    let reverse = crate::link_lifecycle_from_cards(&cards)?.reverse_sections()?;
    let mut faults = Vec::new();
    if expected != actual {
        let expected_set: BTreeSet<_> = expected.iter().collect();
        let actual_set: BTreeSet<_> = actual.iter().collect();
        faults.push(
            json!({"kind":"occurrences","expected":expected.len(),"actual":actual.len(),
            "first_missing":expected_set.difference(&actual_set).next(),
            "first_unexpected":actual_set.difference(&expected_set).next()}),
        );
    }
    for c in &cards {
        let expected_text = crate::card::with_reverse_section(&c.text, &reverse[&c.address])?;
        if c.text != expected_text {
            faults.push(json!({"kind":"reverse","address":c.address}));
        }
    }
    let stats: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata WHERE key='link_write_stats'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let stats = stats
        .map(|s| serde_json::from_str::<Value>(&s))
        .transpose()?;
    Ok(
        json!({"ok":faults.is_empty(),"cards":cards.len(),"links":actual.len(),"archive_revision":revision(conn)?,"faults":faults,"last_write":stats}),
    )
}

pub(crate) fn command(reindex: bool) -> Result<()> {
    let root = crate::require_archive_root()?;
    let mut conn = crate::open_db(&root)?;
    if reindex {
        let _lock = crate::acquire_edit_lock_within(&root, std::time::Duration::from_secs(20), "migrate")?;
        if !ready(&conn) {
            bail!("Link index is not initialized; run `zt up` first");
        }
        let tx = conn.transaction()?;
        let before = check(&tx)?;
        rebuild(&tx)?;
        if before["ok"] != true {
            bump_revision(&tx)?;
        }
        tx.commit()?;
    }
    let read = conn.unchecked_transaction()?;
    let report = check(&read)?;
    read.commit()?;
    println!("{report}");
    if report["ok"] != true {
        bail!("Link index integrity check failed; run `zt reindex`");
    }
    Ok(())
}
