//! Transactional, derived index of authored Link occurrences.
//!
//! Bodies are authoritative. The full parser remains the migration/reindex
//! oracle; normal writes replace only one source and refresh affected targets.
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
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
                    read.kind.names_a_relation().then(|| read.kind.id().to_string())
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
    let mut changes = Changes::default();
    for at in sources {
        changes.resync(conn, at)?;
    }
    let changed = refresh_reverse(conn, &changes.affected)?;
    write_stats(conn, sources.len(), changes.affected.len(), changed)
}

/// The Cards one structural write (move, insert, Side placement, delete, Citation key rename) changes,
/// gathered while it runs and refreshed once before its commit, so the write costs the Cards it
/// touches and their Link neighbours, not the archive (S1, #229). Addresses are current: each
/// rename maps the ones gathered before it. On an archive without the index, `finish` falls back to
/// the full regeneration.
#[derive(Default)]
pub(crate) struct Changes {
    /// Cards whose generated Reverse link or Term use sections may change.
    affected: BTreeSet<String>,
    /// Sources whose index rows were re-read, re-keyed or dropped.
    sources: BTreeSet<String>,
}

impl Changes {
    /// Re-read the index rows of the Card at `at` from its stored body; the Cards it linked before
    /// and links now need a refresh.
    pub(crate) fn resync(&mut self, conn: &Connection, at: &str) -> Result<()> {
        if !ready(conn) {
            return Ok(());
        }
        let card = crate::load_card(conn, at)?;
        let old = source_rows(conn, at)?;
        let new = rows(&card)?;
        self.affected
            .extend(old.iter().chain(&new).map(|r| r.2.clone()));
        self.affected.insert(at.to_string());
        self.sources.insert(at.to_string());
        if old != new {
            conn.execute("DELETE FROM links WHERE source=?1", [at])?;
            let mut insert = conn
                .prepare_cached("INSERT INTO links(source,ord,target,kind) VALUES(?1,?2,?3,?4)")?;
            for (source, ord, target, kind) in &new {
                insert.execute(params![source, ord, target, kind])?;
            }
        }
        Ok(())
    }

    /// A Card whose body the write rewrote in place (its rows already re-keyed): refresh it.
    pub(crate) fn rewrote(&mut self, at: &str) {
        self.affected.insert(at.to_string());
        self.sources.insert(at.to_string());
    }

    /// Drop the rows of the Cards about to be deleted; the Cards they linked need a refresh.
    pub(crate) fn drop_sources(&mut self, conn: &Connection, deleted: &BTreeSet<String>) -> Result<()> {
        if !ready(conn) {
            return Ok(());
        }
        for at in deleted {
            self.affected
                .extend(source_rows(conn, at)?.into_iter().map(|r| r.2));
            conn.execute("DELETE FROM links WHERE source=?1", [at])?;
            self.sources.insert(at.clone());
        }
        Ok(())
    }

    /// After the Cards at the mapping's keys took their new addresses: re-key their rows as sources
    /// and as targets. A Card that a renamed Card links lists that source's address in its Reverse
    /// links, so it needs a refresh.
    pub(crate) fn renamed(&mut self, conn: &Connection, mapping: &BTreeMap<String, String>) -> Result<()> {
        if !ready(conn) || mapping.is_empty() {
            return Ok(());
        }
        let through = |at: &String| mapping.get(at).cloned().unwrap_or_else(|| at.clone());
        self.affected = self.affected.iter().map(through).collect();
        self.sources = self.sources.iter().map(through).collect();
        for old in mapping.keys() {
            for row in source_rows(conn, old)? {
                self.affected.insert(through(&row.2));
            }
        }
        self.affected.extend(mapping.values().cloned());
        self.sources.extend(mapping.values().cloned());
        // Two steps through a marked value, so a chain (`3/2 -> 3/3`, `3/3 -> 3/4`) never maps a
        // row twice and never meets the (source, ord) key of a row still waiting to move.
        for (old, new) in mapping {
            conn.execute(
                "UPDATE links SET source=char(1)||?2 WHERE source=?1",
                params![old, new],
            )?;
            conn.execute(
                "UPDATE links SET target=char(1)||?2 WHERE target=?1",
                params![old, new],
            )?;
        }
        // Clear the marks row by row through the indexes; a scan of the whole table would cost the
        // archive's size on every rename.
        for new in mapping.values() {
            conn.execute(
                "UPDATE links SET source=?1 WHERE source=char(1)||?1",
                params![new],
            )?;
            conn.execute(
                "UPDATE links SET target=?1 WHERE target=char(1)||?1",
                params![new],
            )?;
        }
        Ok(())
    }

    /// Refresh the gathered Cards once and advance the archive revision.
    pub(crate) fn finish(self, conn: &Connection) -> Result<()> {
        if !ready(conn) {
            return crate::regenerate_reverse_links_tx(conn);
        }
        let changed = refresh_reverse(conn, &self.affected)?;
        write_stats(conn, self.sources.len(), self.affected.len(), changed)?;
        // Every unit test that makes a structural write also proves it against a fresh parse.
        #[cfg(test)]
        {
            // The oracle read does not count against the write it checks.
            let reads = crate::CARD_TEXT_READS.with(|count| count.get());
            let report = check(conn)?;
            crate::CARD_TEXT_READS.with(|count| count.set(reads));
            assert_eq!(report["ok"], true, "Link index after a structural write: {report}");
        }
        bump_revision(conn)
    }
}

/// The sources whose bodies link one of `targets`, read from the index (Term uses included).
pub(crate) fn sources_linking<'a>(
    conn: &Connection,
    targets: impl IntoIterator<Item = &'a String>,
) -> Result<BTreeSet<String>> {
    let mut query = conn.prepare_cached("SELECT DISTINCT source FROM links WHERE target=?1")?;
    let mut sources = BTreeSet::new();
    for target in targets {
        for source in query.query_map([target], |r| r.get::<_, String>(0))? {
            sources.insert(source?);
        }
    }
    Ok(sources)
}

/// How many authored Link macros (Term uses included) name one of `targets`.
pub(crate) fn occurrences_of<'a>(
    conn: &Connection,
    targets: impl IntoIterator<Item = &'a String>,
) -> Result<usize> {
    let mut query = conn.prepare_cached("SELECT COUNT(*) FROM links WHERE target=?1")?;
    let mut count = 0;
    for target in targets {
        count += query.query_row([target], |r| r.get::<_, i64>(0))? as usize;
    }
    Ok(count)
}

fn source_rows(conn: &Connection, at: &str) -> Result<Vec<Row>> {
    let mut query =
        conn.prepare_cached("SELECT source,ord,target,kind FROM links WHERE source=?1 ORDER BY ord")?;
    let rows = query
        .query_map([at], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
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
    let mut titles = std::collections::HashMap::<String, String>::new();
    for target in affected {
        let Some(card) = optional_card(conn, target)? else {
            continue;
        };
        // One row per (source, form): ordinary Reverse links and Term uses are separate sections,
        // and a source that writes both forms to one target appears once in each (S11, #153).
        let mut sources = conn.prepare_cached(
            "SELECT DISTINCT l.source, CASE WHEN l.kind = 'term' THEN 1 ELSE 0 END AS term
             FROM links l
             JOIN cards c ON (c.location=l.source OR c.citation_key=l.source)
             WHERE l.target=?1 ORDER BY l.source, term",
        )?;
        let rows = sources
            .query_map([target], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut ordinary = Vec::new();
        let mut term_uses = Vec::new();
        for (address, is_term) in rows {
            // A refresh rewrites only generated sections, so a source's title stays put for the
            // whole call: read and parse each source once.
            let title = match titles.get(&address) {
                Some(title) => title.clone(),
                None => {
                    let title = crate::parse_card_text(&crate::load_card(conn, &address)?.text)?.title;
                    titles.insert(address.clone(), title.clone());
                    title
                }
            };
            if is_term {
                term_uses.push((address, title));
            } else {
                ordinary.push((address, title));
            }
        }
        let reverse = crate::card::reverse_section(
            ordinary.iter().map(|(a, t)| (a.as_str(), t.as_str())),
            term_uses.iter().map(|(a, t)| (a.as_str(), t.as_str())),
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{self, Outcome, Successor, Target, TypeChange};
    use crate::{Access, CardType, Workspace};

    const OWNER: Access = Access::Owner;

    /// A fresh archive: `indexed` runs the startup migration that builds the Link index, so the
    /// structural writers take the indexed path; without it they take the full-scan path the
    /// archive used before the index existed, which is the oracle the indexed path must match.
    fn archive(indexed: bool) -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).unwrap();
        if indexed {
            assert!(ready(&conn));
        } else {
            conn.execute_batch("DROP TABLE links; DELETE FROM metadata WHERE key='links_index_version';")
                .unwrap();
            assert!(!ready(&conn));
        }
        temp
    }

    fn cards(root: &Path) -> Vec<(String, String, String)> {
        let conn = crate::open_db(root).unwrap();
        let mut cards = crate::load_cards(&conn)
            .unwrap()
            .into_iter()
            .map(|c| (c.address, c.text, c.card_type.id().to_string()))
            .collect::<Vec<_>>();
        cards.sort();
        cards
    }

    fn regular(root: &Path) -> Vec<String> {
        let conn = crate::open_db(root).unwrap();
        let mut out = crate::load_cards(&conn)
            .unwrap()
            .into_iter()
            .filter(|c| !crate::is_tree_root(c))
            .map(|c| c.address)
            .collect::<Vec<_>>();
        out.sort();
        out
    }

    fn report(root: &Path) -> Value {
        let conn = crate::open_db(root).unwrap();
        check(&conn).unwrap()
    }

    /// Confirm a two-step change with the word its plan names.
    /// The reply carries the plan lines too, so two archives must agree on the plan as well (S2).
    fn confirmed(run: impl Fn(Option<&str>) -> Result<Outcome>) -> Result<Value> {
        match run(None)? {
            Outcome::Confirm { confirm, lines } => match run(Some(&confirm))? {
                Outcome::Done(value) => Ok(serde_json::json!({"plan": lines, "done": value})),
                Outcome::Confirm { .. } => panic!("a confirmed change asked again"),
            },
            Outcome::Done(value) => Ok(value),
        }
    }

    fn card(title: &str, links: &[&str]) -> String {
        let body = links
            .iter()
            .map(|at| format!("见 [[{at}]]。"))
            .collect::<Vec<_>>()
            .join("\n\n");
        format!("# {title}\n\n{title}的正文。\n\n{body}\n")
    }

    /// Two Topic trees and one Literature tree with Side branches, Direct chains, cross-tree Links,
    /// a Term Card and a Term use.
    fn seed(root: &Path) {
        ops::create_topic(root, OWNER, Workspace::Agent, "# 甲书\n").unwrap();
        ops::create_topic(root, OWNER, Workspace::Agent, "# 乙书\n").unwrap();
        ops::create_literature(
            root,
            OWNER,
            Workspace::Agent,
            "@book{Smith2024, title = {A Book}}",
            "# x\n\n文献。\n",
        )
        .unwrap();
        let direct = |at: &str, title: &str, links: &[&str]| {
            ops::create_successor(root, OWNER, Successor::Direct, at, CardType::Idea, None, &card(title, links))
                .unwrap()
                .0
        };
        let side = |at: &str, title: &str, links: &[&str]| {
            ops::create_successor(root, OWNER, Successor::Side, at, CardType::Idea, None, &card(title, links))
                .unwrap()
                .0
        };
        direct("0/0", "一", &[]);
        direct("0/1", "二", &["0/1"]);
        side("0/2", "二甲", &["0/2", "0/1"]);
        direct("0/2|a", "二甲续", &["0/2|a"]);
        side("0/2", "二乙", &["0/2|a|1"]);
        direct("0/2", "三", &["0/2|b", "0/0"]);
        direct("0/3", "四", &["0/2"]);
        side("0/4", "四甲", &["0/3"]);
        direct("1/0", "壹", &["0/4"]);
        direct("1/1", "贰", &["1/1", "0/2|a"]);
        direct("Smith2024", "第一节", &["0/3"]);
        direct("Smith2024/1", "第二节", &["Smith2024/1", "Smith2024"]);
        // A Term list (a Term Card without a name) and its Term head, a named Side successor.
        ops::create_successor_with_term(root, OWNER, Successor::Side, "1/1", CardType::Term, None, None, "# 术语表\n\n本书的术语。\n")
            .unwrap();
        ops::create_successor_with_term(
            root,
            OWNER,
            Successor::Side,
            "1/1|a",
            CardType::Term,
            None,
            Some("词"),
            "# 词\n\n术语：词\n",
        )
        .unwrap();
        let use_at = direct("1/2", "用词", &["1/1"]);
        let text = "# 用词\n\n这里用到[[词@1/1|a|a]]。\n\n见 [[1/1]]。\n";
        ops::edit(root, OWNER, &use_at, Some(text), TypeChange::default(), None).unwrap();
        side("Smith2024/1", "旁注", &["1/2", "0/4|a"]);
    }

    /// A small deterministic generator, so a failing sequence replays exactly.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn pick(&mut self, items: &[String]) -> String {
            items[(self.next() as usize) % items.len()].clone()
        }
    }

    /// One random structural (or plain) write, applied to `root`.
    fn step(root: &Path, rng: &mut Rng, op: u64, renames: &mut u32) -> std::result::Result<String, String> {
        let all = regular(root);
        let a = rng.pick(&all);
        let b = rng.pick(&all);
        let c = rng.pick(&all);
        let coin = rng.next() % 2 == 0;
        let letter = ["a", "b", "c"][(rng.next() % 3) as usize];
        let text = card(&format!("新{op}"), &[&c]);
        // A delete takes a whole Direct chain, so it waits until the archive has Cards to spare.
        let kind = if op % 9 == 5 && all.len() <= 12 { 7 } else { op % 9 };
        let outcome: Result<String> = match kind {
            0 => confirmed(|confirm| ops::move_card(root, OWNER, &a, &Target::Before(b.clone()), false, confirm))
                .map(|v| v.to_string()),
            1 => confirmed(|confirm| ops::move_card(root, OWNER, &a, &Target::Before(b.clone()), true, confirm))
                .map(|v| v.to_string()),
            2 => {
                let to = if coin {
                    crate::direct_successor(&b).unwrap()
                } else {
                    format!("{b}|{letter}")
                };
                confirmed(|confirm| ops::move_card(root, OWNER, &a, &Target::To(to.clone()), !coin, confirm))
                    .map(|v| v.to_string())
            }
            3 => confirmed(|confirm| {
                ops::insert_card_with_term(root, OWNER, &a, CardType::Idea, None, None, &text, confirm)
            })
            .map(|v| v.to_string()),
            4 => match crate::parent_location(&a) {
                Some(parent) => {
                    ops::create_side_before(root, OWNER, &parent, &a, CardType::Idea, None, None, &text).map(|v| v.0)
                }
                None => Ok(String::new()),
            },
            5 => {
                let into = coin.then_some(b.as_str());
                confirmed(|confirm| ops::delete(root, OWNER, &a, into, confirm)).map(|v| v.to_string())
            }
            6 => ops::edit(root, OWNER, &a, Some(&text), TypeChange::default(), None).map(|v| v.0),
            7 => {
                let kind = if coin { Successor::Direct } else { Successor::Side };
                ops::create_successor(root, OWNER, kind, &a, CardType::Idea, None, &text).map(|v| v.0)
            }
            _ => {
                *renames += 1;
                let key = format!("Smith2024r{}", *renames);
                let from = if *renames == 1 {
                    "Smith2024".to_string()
                } else {
                    format!("Smith2024r{}", *renames - 1)
                };
                let bibtex = format!("@book{{{key}, title = {{A Book}}}}");
                confirmed(|confirm| ops::edit_metadata(root, OWNER, &from, &bibtex, None, confirm))
                    .map(|v| v.to_string())
            }
        };
        outcome.map_err(|e| format!("{e:#}"))
    }

    /// S1 (#229): every structural writer on the indexed path leaves exactly the Cards the full-scan
    /// path leaves (authored bodies, Reverse links, Term uses), and the index matches a fresh parse.
    #[test]
    fn indexed_structural_writes_match_the_full_scan_path_step_by_step() {
        for seed_value in [1_u64, 7, 42] {
            let (indexed, scanned) = (archive(true), archive(false));
            seed(indexed.path());
            seed(scanned.path());
            assert_eq!(cards(indexed.path()), cards(scanned.path()), "seeded archives differ");
            let (mut left, mut right) = (Rng(seed_value), Rng(seed_value));
            let (mut renames_left, mut renames_right) = (0, 0);
            let mut applied = 0;
            for op in 0..60 {
                let kind = left.next();
                right.next();
                let got = step(indexed.path(), &mut left, kind, &mut renames_left);
                let want = step(scanned.path(), &mut right, kind, &mut renames_right);
                assert_eq!(got, want, "seed {seed_value} op {op}: the plan, the result or the refusal differs");
                applied += usize::from(got.is_ok());
                assert_eq!(
                    cards(indexed.path()),
                    cards(scanned.path()),
                    "seed {seed_value} op {op} (kind {}) {got:?}",
                    kind % 9
                );
                let report = report(indexed.path());
                assert_eq!(report["ok"], true, "seed {seed_value} op {op}: {report}");
            }
            assert!(applied >= 20, "seed {seed_value}: only {applied} writes applied");
        }
    }

    /// S1 (#229): a move re-indexes only the Cards it touches, not the whole archive.
    #[test]
    fn a_move_refreshes_only_the_cards_it_touches() {
        let temp = archive(true);
        let root = temp.path();
        seed(root);
        for n in 0..40 {
            ops::create_successor(root, OWNER, Successor::Side, "1/1", CardType::Idea, None, &card(&format!("填{n}"), &[]))
                .unwrap();
        }
        confirmed(|confirm| ops::move_card(root, OWNER, "0/4|a", &Target::Before("0/2|b".into()), false, confirm))
            .unwrap();
        let report = report(root);
        assert_eq!(report["ok"], true, "{report}");
        let last = &report["last_write"];
        assert!(last["indexed_cards"].as_u64().unwrap() <= 4, "{last}");
        assert!(last["refreshed_cards"].as_u64().unwrap() <= 12, "{last}");
    }
    /// S2 (#230): plans and gates read the Location set and the Link index, never every Card's
    /// text: a move (subtree, `alone`, to a free slot), an insert, a Side placement, a delete with
    /// `into`, a Citation key rename and the agent's Link-rewrite gate.
    #[test]
    fn planning_a_structural_write_reads_no_card_text() {
        let temp = archive(true);
        let root = temp.path();
        seed(root);
        let conn = crate::open_db(root).unwrap();
        let reads = || crate::CARD_TEXT_READS.with(|count| count.get());
        let before = reads();
        let subtree = crate::place::move_plan(&conn, "0/2|b", &Target::Before("0/2|a".into()), false).unwrap();
        assert_eq!(subtree.rewritten_links, 4, "0/2|a from 0/2|a|1 and 1/2, 0/2|a|1 from 0/2|b, 0/2|b from 0/3");
        crate::place::move_plan(&conn, "0/3", &Target::Before("0/2".into()), true).unwrap();
        crate::place::move_plan(&conn, "0/4", &Target::To("1/1|b".into()), false).unwrap();
        crate::place::insert_plan(&conn, "0/2|a").unwrap();
        crate::place::insert_side_plan(&conn, "0/2|a").unwrap();
        let delete = crate::delete_plan(&conn, "0/2|a").unwrap().into(&conn, "0/1").unwrap();
        assert_eq!(
            delete.inbound,
            vec![
                ("0/2|b".to_string(), "0/2|a|1".to_string()),
                ("1/2".to_string(), "0/2|a".to_string()),
            ]
        );
        crate::citation_rename_mapping(&conn, "Smith2024", "Smith2025").unwrap();
        Access::Agent
            .ensure_link_rewrites_allowed(&conn, &subtree.mapping)
            .unwrap();
        assert_eq!(reads(), before, "a plan read every Card");
    }

    /// S2 (#230): an agent change that would rewrite a Link in a human Card is refused with the same
    /// words on the indexed path and on the full-scan path.
    #[test]
    fn an_agent_change_that_rewrites_a_human_link_is_refused_on_both_paths() {
        for indexed in [true, false] {
            let temp = archive(indexed);
            let root = temp.path();
            seed(root);
            let (topic, _) = ops::create_topic(root, OWNER, Workspace::Human, "# 人\n").unwrap();
            let (human, _) = ops::create_successor(
                root,
                OWNER,
                Successor::Direct,
                &topic,
                CardType::Idea,
                None,
                &card("人卡", &["0/2|b"]),
            )
            .unwrap();
            let error = confirmed(|confirm| {
                ops::move_card(root, Access::Agent, "0/2|b", &Target::Before("0/2|a".into()), false, confirm)
            })
            .unwrap_err();
            assert_eq!(
                format!("{error:#}"),
                format!(
                    "this change would rewrite Links in human workspace Card `{human}`, which is read-only for AI agents"
                ),
                "indexed: {indexed}"
            );
        }
    }
    /// S2 (#230): a whole structural write, plan and apply, reads no Card text beyond the Cards it
    /// touches: the Lock gate looks rows up by address and the Link index names the rewritten
    /// sources, so a large mapping no longer reads the archive once per moved Card.
    #[test]
    fn a_structural_write_reads_no_card_text_end_to_end() {
        let temp = archive(true);
        let root = temp.path();
        seed(root);
        let reads = || crate::CARD_TEXT_READS.with(|count| count.get());
        let before = reads();
        let conn = crate::open_db(root).unwrap();
        let mapping = BTreeMap::from([("0/2|a".to_string(), "0/2|c".to_string())]);
        crate::tree_lock::ensure_unlocked(&conn, crate::tree_lock::Change::Mapping(&mapping)).unwrap();
        drop(conn);
        assert_eq!(reads(), before, "the Lock gate read every Card");
        let mut counted = Vec::new();
        let mut count = |name: &str, write: &dyn Fn()| {
            let start = reads();
            write();
            counted.push((name.to_string(), reads() - start));
        };
        count("Side placement", &|| {
            ops::create_side_before(root, OWNER, "0/2", "0/2|a", CardType::Idea, None, None, &card("旁", &["0/1"]))
                .unwrap();
        });
        count("move", &|| {
            confirmed(|confirm| ops::move_card(root, OWNER, "0/2|c", &Target::Before("0/2|a".into()), false, confirm))
                .unwrap();
        });
        count("move alone", &|| {
            confirmed(|confirm| ops::move_card(root, OWNER, "0/3", &Target::Before("0/2".into()), true, confirm))
                .unwrap();
        });
        count("insert", &|| {
            confirmed(|confirm| {
                ops::insert_card_with_term(root, OWNER, "0/2", CardType::Idea, None, None, &card("插", &["0/1"]), confirm)
            })
            .unwrap();
        });
        count("delete into", &|| {
            confirmed(|confirm| ops::delete(root, OWNER, "1/1|a|a", Some("1/1"), confirm)).unwrap();
        });
        count("Citation key rename", &|| {
            confirmed(|confirm| {
                ops::edit_metadata(root, OWNER, "Smith2024", "@book{Smith2025, title = {A Book}}", None, confirm)
            })
            .unwrap();
        });
        let read: Vec<_> = counted.iter().filter(|(_, n)| *n > 0).collect();
        assert!(read.is_empty(), "these writes read every Card: {read:?}");
    }
}
