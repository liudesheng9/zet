//! `zt-digest term-sync`: the Terms of one book linked to their equivalents in the other books of its
//! Literature type (docs/term-sync-design.md). The skill `zt-term-sync` conducts it: no Workflow, no
//! staging, no ledger. A Term never changes: the only write is one Link line on a book's own Term Cards.
//!
//!   zt-digest term-sync -Step status [-Type history]
//!   zt-digest term-sync -Step start  -Book <key> -Work <folder> [-Type history] [-Model <m>]
//!   zt-digest term-sync -Step review -Work <folder> [-Job <id>]
//!   zt-digest term-sync -Step apply  -Work <folder>
//!
//!   status  which books of the shelf the list knows.
//!   start   reads one book, settles its exact matches, and writes jobs for the rows with candidates. The
//!           first book of a shelf has none, so it seeds the list and spends nothing.
//!   review  the answer's faults and real sentences: the job's SELF-CHECK, and what Opus reads before apply.
//!   apply   enters the book into the list and writes the Link lines on this book's Term Cards. A Reverse
//!           link shows each on the other Card, so an earlier book is never touched.

use crate::args::Args;
use crate::canon;
use crate::ps;
use crate::term_standard::{self, Row, Standard, Triage};
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::LazyLock;

/// The align instructions, embedded so a job never depends on a checkout.
const ALIGN_DOC: &str = include_str!("../../../scripts/book-digest/dsh-term-sync-instructions.zh.md");
/// Candidates of one row: one above the 5 that found every known pair on the three history books.
const CANDIDATES: usize = 6;
/// Rows of one align job.
const ROWS_PER_JOB: usize = 300;
/// An identified `apply_batch` call.
const BATCH_ITEMS: usize = 64;
const BATCH_BYTES: usize = 1 << 20;
/// Opens the line that holds a Term's Links to its equivalents.
const LINK_LABEL: &str = "等义术语：";
/// `get_cards` takes at most this many addresses.
const READ_CARDS: usize = 200;
/// Characters of context on each side of an evidence sentence.
const EVIDENCE: usize = 42;
const SAMPLES: usize = 2;
static LABEL_PLAIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]@\n]+)@[^\]\n]+\]\]").unwrap());
struct Params {
    step: String,
    work: String,
    kind: String,
    book: String,
    job: String,
    model: String,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(argv, &["Step", "Work", "Type", "Book", "Job", "Model"], &[])?;
    let p = Params {
        step: a.str("Step"),
        work: a.str("Work").trim_end_matches(['\\', '/']).to_string(),
        kind: a.str_or("Type", "history"),
        book: a.str("Book"),
        job: a.str("Job"),
        model: a.str_or("Model", crate::cited::MODEL),
    };
    if p.step != "status" && p.work.is_empty() {
        bail!("term-sync: -Work is required");
    }
    match p.step.as_str() {
        "status" => status(&p),
        "start" => start(&p),
        "review" => review(&p),
        "apply" => apply(&p),
        other => bail!("term-sync: unknown -Step `{other}`"),
    }
}

// --- The book ----------------------------------------------------------------------------------

/// One book: its rows, with its Cards when the step needs them.
#[derive(Clone, Debug)]
struct Book {
    pub key: String,
    pub title: String,
    pub list: String,
    pub rows: Vec<Row>,
    pub cards: Vec<Value>,
}

/// The agent Literature trees of one type, in shelf order: (key, title, locked).
fn shelf_keys(zt: &mut Zt, kind: &str) -> Result<Vec<(String, String, bool)>> {
    let listed = zt.call("list_trees", json!({ "workspace": "agent", "zone": "literature" }))?;
    let mut books: Vec<(String, String, bool)> = listed["trees"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|tree| ps::s(&tree["literature_type"]) == kind)
        .map(|tree| (ps::s(&tree["root"]), ps::s(&tree["title"]), tree["locked"] == true))
        .collect();
    books.sort();
    Ok(books)
}

/// One book's rows from its Term list. A tree with no Term list is no book of the shelf.
fn read_rows(zt: &mut Zt, key: &str, title: &str) -> Result<Option<Book>> {
    let reply = zt.request("get_terms", json!({ "at": key }))?;
    if zt_check::mcp::reply_error(&reply).is_some() {
        return Ok(None);
    }
    let terms: Value = serde_json::from_str(&zt_check::mcp::reply_text(&reply)).context("get_terms is not JSON")?;
    let uses: HashMap<String, i64> = terms["heads"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|head| (ps::s(&head["term"]), head["uses"].as_i64().unwrap_or(0)))
        .collect();
    let rows = terms["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            let term = ps::s(&row["term"]);
            Row {
                id: format!("R{}", row["row"].as_i64().unwrap_or(0)),
                uses: uses.get(&term).copied().unwrap_or(0),
                avoid: row["avoid"].as_array().into_iter().flatten().map(ps::s).filter(|word| !word.is_empty()).collect(),
                meaning: ps::s(&row["meaning"]),
                head: row["heads"].as_array().and_then(|heads| heads.first()).map(ps::s).unwrap_or_default(),
                term,
            }
        })
        .collect();
    Ok(Some(Book { key: key.to_string(), title: title.to_string(), list: ps::s(&terms["list"]), rows, cards: Vec::new() }))
}

/// One book with its Cards, or a refusal the step prints.
fn read_book(zt: &mut Zt, kind: &str, key: &str) -> Result<std::result::Result<Book, Value>> {
    let keys = shelf_keys(zt, kind)?;
    let Some((_, title, locked)) = keys.iter().find(|(book, _, _)| book == key) else {
        return Ok(Err(json!({"ok": false, "code": "no_book", "error": format!("{key} is no {kind} book of the agent Workspace")})));
    };
    if *locked {
        return Ok(Err(json!({"ok": false, "code": "card_locked", "error": format!("card_locked: the tree {key} is locked by the owner")})));
    }
    let Some(mut book) = read_rows(zt, key, title)? else {
        return Ok(Err(json!({"ok": false, "code": "no_term_list", "error": format!("{key} has no Term list")})));
    };
    let tree = zt.call("get_tree", json!({ "tree": key, "bodies": true }))?;
    book.cards = tree["cards"].as_array().cloned().unwrap_or_default();
    Ok(Ok(book))
}

fn row_json(row: &Row) -> Value {
    json!({"id": row.id, "term": row.term, "avoid": row.avoid, "meaning": row.meaning, "head": row.head, "uses": row.uses})
}

fn row_of(value: &Value) -> Row {
    Row {
        id: ps::s(&value["id"]),
        term: ps::s(&value["term"]),
        avoid: value["avoid"].as_array().into_iter().flatten().map(ps::s).collect(),
        meaning: ps::s(&value["meaning"]),
        head: ps::s(&value["head"]),
        uses: value["uses"].as_i64().unwrap_or(0),
    }
}

/// The list's file for the archive this command serves. A copied box has its own.
fn store_of(zt: &mut Zt, kind: &str) -> Result<PathBuf> {
    zt.call("list_workspaces", json!({}))?;
    let root = zt_check::mcp::observed_service().and_then(|facts| facts.archive_root).context("the archive root is not known")?;
    Ok(term_standard::store_path(&root, kind))
}

fn book_path(p: &Params) -> String {
    ps::join(&p.work, "book.json")
}

fn entries_path(p: &Params) -> String {
    ps::join(&p.work, "entries.json")
}

fn triage_path(p: &Params) -> String {
    ps::join(&p.work, "triage.json")
}

fn answer_path(p: &Params, job: &str) -> String {
    ps::join(&ps::join(&p.work, "out"), &format!("{job}.json"))
}

fn read_saved_book(p: &Params) -> Result<(Book, String)> {
    let saved = util::read_json(book_path(p)).context("term-sync: run -Step start first")?;
    let book = Book {
        key: ps::s(&saved["key"]),
        title: ps::s(&saved["title"]),
        list: ps::s(&saved["list"]),
        rows: saved["rows"].as_array().into_iter().flatten().map(row_of).collect(),
        cards: saved["cards"].as_array().cloned().unwrap_or_default(),
    };
    Ok((book, ps::s(&saved["type"])))
}

fn read_triage(p: &Params) -> Result<Triage> {
    let saved = util::read_json(triage_path(p)).context("term-sync: run -Step start first")?;
    Ok(Triage {
        same: saved["same"].as_array().into_iter().flatten().map(|pair| (ps::s(&pair[0]), ps::s(&pair[1]))).collect(),
        open: saved["open"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|item| (ps::s(&item[0]), item[1].as_array().into_iter().flatten().map(ps::s).collect()))
            .collect(),
        new: saved["new"].as_array().into_iter().flatten().map(ps::s).collect(),
        known: saved["known"].as_array().into_iter().flatten().map(ps::s).collect(),
    })
}

fn status(p: &Params) -> Result<i32> {
    let mut zt = crate::zt::start()?;
    let path = store_of(&mut zt, &p.kind)?;
    let standard = term_standard::load(&path, &p.kind)?;
    let mut lines = Vec::new();
    let mut waiting: Vec<(usize, String)> = Vec::new();
    for (key, title, _) in shelf_keys(&mut zt, &p.kind)? {
        let Some(book) = read_rows(&mut zt, &key, &title)? else { continue };
        let known = standard.aligned.get(&key).is_some_and(|hash| *hash == term_standard::rows_hash(&book.rows));
        lines.push(format!("{key}\t{} rows\t{}", book.rows.len(), if known { "linked" } else { "not linked" }));
        if !known {
            waiting.push((book.rows.len(), key));
        }
    }
    waiting.sort();
    lines.push(util::compact(&json!({"ok": true, "entries": standard.entries.len(), "waiting": waiting.len(), "next": waiting.last().map(|(_, key)| key.clone())})));
    util::print_lines(&lines)?;
    Ok(0)
}

/// The rows of one align job with their candidates, in the form the instructions describe.
fn job_text(rows: &[Row], open: &[(String, Vec<String>)], standard: &Standard) -> String {
    let mut text = String::from("\n## 本书待对齐的术语行\n\n");
    for (id, offered) in open {
        let Some(row) = rows.iter().find(|row| row.id == *id) else { continue };
        text.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", row.id, row.term, row.avoid.join("、"), row.meaning, row.uses));
        for candidate in offered {
            if let Some(entry) = standard.entries.iter().find(|entry| entry.id == *candidate) {
                text.push_str(&format!("  {}\t{}\t{}\t{}\n", entry.id, entry.term, entry.variants.join("、"), entry.meaning));
            }
        }
    }
    text
}

/// The entries a book's jobs were offered, so a review reads them without the box.
fn offered_entries(standard: &Standard, sorted: &Triage) -> Standard {
    let offered: HashSet<&String> = sorted.open.iter().flat_map(|(_, ids)| ids).collect();
    Standard { kind: standard.kind.clone(), entries: standard.entries.iter().filter(|entry| offered.contains(&entry.id)).cloned().collect(), aligned: Default::default() }
}

fn start(p: &Params) -> Result<i32> {
    if p.book.is_empty() {
        bail!("term-sync: -Book names the book to align");
    }
    std::fs::create_dir_all(ps::join(&p.work, "out"))?;
    std::fs::create_dir_all(ps::join(&p.work, "drive"))?;
    let mut zt = crate::zt::start()?;
    let path = store_of(&mut zt, &p.kind)?;
    let standard = term_standard::load(&path, &p.kind)?;
    let book = match read_book(&mut zt, &p.kind, &p.book)? {
        Ok(book) => book,
        Err(refusal) => {
            util::print_last(&refusal)?;
            return Ok(0);
        }
    };
    let sorted = term_standard::triage(&standard, &book.key, &book.rows, CANDIDATES);
    let saved = json!({
        "type": p.kind, "key": book.key, "title": book.title, "list": book.list,
        "rows": book.rows.iter().map(row_json).collect::<Vec<_>>(), "cards": book.cards,
    });
    util::write_atomic(book_path(p), &ps::json_compact(&saved))?;
    util::write_atomic(triage_path(p), &ps::json_compact(&json!({"same": sorted.same, "open": sorted.open, "new": sorted.new, "known": sorted.known})))?;
    term_standard::save(std::path::Path::new(&entries_path(p)), &offered_entries(&standard, &sorted))?;
    let mut jobs = Vec::new();
    for (index, part) in sorted.open.chunks(ROWS_PER_JOB).enumerate() {
        let id = format!("a{:03}", index + 1);
        let answer = answer_path(p, &id);
        let command = format!("zt-digest term-sync -Step review -Work \"{}\" -Job {id}", p.work);
        let text = ALIGN_DOC.replace("{{REVIEW}}", &command).replace("{{ANSWER}}", &answer);
        let task = ps::join(&ps::join(&p.work, "drive"), &format!("{id}.md"));
        util::write_atomic(&task, &format!("{text}{}", job_text(&book.rows, part, &standard)))?;
        let ids: Vec<&String> = part.iter().map(|(row, _)| row).collect();
        util::write_atomic(ps::join(&ps::join(&p.work, "drive"), &format!("{id}.rows.json")), &ps::json_compact(&json!(ids)))?;
        jobs.push(json!({"id": id, "files": [task], "prompt": "按任务说明的步骤做完全部工作，最后只回复一行 JSON。", "output": answer, "model": p.model}));
    }
    let file = ps::join(&ps::join(&p.work, "drive"), "jobs.json");
    util::write_atomic(&file, &ps::json_compact(&Value::Array(jobs.clone())))?;
    util::print_last(&json!({
        "ok": true, "book": book.key, "rows": book.rows.len(), "same": sorted.same.len(), "known": sorted.known.len(), "open": sorted.open.len(), "new": sorted.new.len(),
        "entries": standard.entries.len(), "jobs": jobs.len(), "file": file,
    }))?;
    Ok(0)
}

// --- The align job's review --------------------------------------------------------------------

/// The jobs with no answer file.
fn unanswered(jobs: &[String], has_answer: impl Fn(&str) -> bool) -> Vec<String> {
    jobs.iter().filter(|job| !has_answer(job)).cloned().collect()
}

/// Every (row, entry) pair the job answers have given.
fn answers(p: &Params, jobs: &[String]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for job in jobs {
        if let Ok(answer) = util::read_json(answer_path(p, job)) {
            pairs.extend(answer["align"].as_array().into_iter().flatten().map(|item| (ps::s(&item["row"]), ps::s(&item["to"]))));
        }
    }
    pairs
}

fn job_ids(p: &Params) -> Result<Vec<String>> {
    let jobs = util::read_json(ps::join(&ps::join(&p.work, "drive"), "jobs.json"))?;
    Ok(jobs.as_array().into_iter().flatten().map(|job| ps::s(&job["id"])).collect())
}

/// One char-safe excerpt of `text` around a byte range.
fn around(text: &str, range: Range<usize>, width: usize) -> (String, String) {
    let before: Vec<char> = text[..range.start].chars().rev().take(width).collect();
    let after: String = text[range.end..].chars().take(width).collect();
    (before.into_iter().rev().collect(), after)
}

fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Up to `limit` sentences of a book that use a word, labels counted.
fn samples(book: &Book, word: &str, limit: usize) -> Vec<String> {
    let mut found = Vec::new();
    for card in &book.cards {
        if ps::s(&card["type"]) == "term" || ps::s(&card["type"]) == "cited" {
            continue;
        }
        let body = LABEL_PLAIN.replace_all(card["body"].as_str().unwrap_or_default(), "$1").into_owned();
        if let Some(at) = body.find(word) {
            let (before, after) = around(&body, at..at + word.len(), EVIDENCE);
            found.push(format!("…{}【{word}】{}…", flat(&before), flat(&after)));
            if found.len() == limit {
                break;
            }
        }
    }
    found
}

fn review(p: &Params) -> Result<i32> {
    let (book, kind) = read_saved_book(p)?;
    let sorted = read_triage(p)?;
    // The list frozen at the start: a job's SELF-CHECK runs in its own process and never opens the box.
    let standard = term_standard::load(std::path::Path::new(&entries_path(p)), &kind)?;
    let jobs = if p.job.is_empty() { job_ids(p)? } else { vec![p.job.clone()] };
    let pairs = answers(p, &jobs);
    let scoped = if p.job.is_empty() {
        sorted.clone()
    } else {
        let rows: HashSet<String> = util::read_json(ps::join(&ps::join(&p.work, "drive"), &format!("{}.rows.json", p.job)))?
            .as_array()
            .into_iter()
            .flatten()
            .map(ps::s)
            .collect();
        Triage { open: sorted.open.iter().filter(|(row, _)| rows.contains(row)).cloned().collect(), ..sorted.clone() }
    };
    let faults = term_standard::answer_faults(&scoped, &pairs);
    let mut lines = faults.clone();
    lines.push(String::new());
    for (row_id, to) in &pairs {
        let (Some(row), Some(entry)) = (book.rows.iter().find(|row| row.id == *row_id), standard.entries.iter().find(|entry| entry.id == *to)) else { continue };
        lines.push(format!("### {row_id} {} → {to} {}", row.term, entry.term));
        lines.push(format!("本书：{}（{} 处）", row.meaning, row.uses));
        lines.extend(samples(&book, &row.term, SAMPLES));
        lines.push(format!("标准表：{}", entry.meaning));
        if !entry.example.is_empty() {
            lines.push(entry.example.clone());
        }
    }
    lines.push(format!("faults: {}", faults.len()));
    util::print_lines(&lines)?;
    Ok(0)
}

/// The Link line, closing the explanation. A later run replaces it, so a Card holds one.
pub(crate) fn with_equivalents(body: &str, links: &[String]) -> String {
    let line = format!("{LINK_LABEL}{}", links.iter().map(|address| format!("[[{address}]]")).collect::<Vec<_>>().join("、"));
    let mut lines: Vec<&str> = body.split('\n').collect();
    if let Some(held) = lines.iter().position(|text| text.starts_with(LINK_LABEL)) {
        lines[held] = &line;
        return lines.join("\n");
    }
    let Some(open) = lines.iter().position(|text| text.trim() == "## 释义") else { return body.to_string() };
    let close = lines.iter().skip(open + 1).position(|text| text.starts_with("## ") || text.starts_with("来源：")).map_or(lines.len(), |at| at + open + 1);
    lines.splice(close..close, [line.as_str(), ""]);
    lines.join("\n")
}

/// Read Cards fresh, in order, with the fields `authored_fingerprint` needs.
fn fresh_cards(zt: &mut Zt, addresses: &[String]) -> Result<HashMap<String, Value>> {
    let mut cards = HashMap::new();
    for part in addresses.chunks(READ_CARDS) {
        let read = zt.call("get_cards", json!({ "addresses": part, "view": "full" }))?;
        for card in read["cards"].as_array().into_iter().flatten() {
            cards.insert(ps::s(&card["address"]), card.clone());
        }
    }
    Ok(cards)
}

/// Send edits in identified calls. A refusal stops the pass with the archive's own words.
fn send(zt: &mut Zt, run: &str, phase: &str, items: Vec<Value>) -> Result<usize> {
    let mut sent = 0;
    let mut batch: Vec<Value> = Vec::new();
    let mut bytes = 0;
    let mut flush = |zt: &mut Zt, batch: &mut Vec<Value>| -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let reply = zt.request("apply_batch", json!({ "items": batch }))?;
        if let Some(message) = zt_check::mcp::reply_error(&reply) {
            bail!("apply_batch refused a term-sync edit: {message}");
        }
        sent += batch.len();
        batch.clear();
        Ok(())
    };
    for (index, mut item) in items.into_iter().enumerate() {
        item["op_key"] = json!(format!("{run}/shelf/1/{phase}/item-{index:06}"));
        item["client_ref"] = json!(index.to_string());
        let size = ps::json_compact(&item).len();
        if batch.len() == BATCH_ITEMS || bytes + size > BATCH_BYTES {
            flush(zt, &mut batch)?;
            bytes = 0;
        }
        bytes += size;
        batch.push(item);
    }
    flush(zt, &mut batch)?;
    Ok(sent)
}

fn apply(p: &Params) -> Result<i32> {
    let (book, kind) = read_saved_book(p)?;
    let sorted = read_triage(p)?;
    let jobs = job_ids(p)?;
    let open = unanswered(&jobs, |job| std::path::Path::new(&answer_path(p, job)).exists());
    if !open.is_empty() {
        util::print_last(&json!({"ok": false, "code": "no_answer", "error": format!("no answer from {}", open.join(", "))}))?;
        return Ok(0);
    }
    let pairs = answers(p, &jobs);
    let faults = term_standard::answer_faults(&sorted, &pairs);
    if !faults.is_empty() {
        util::print_last(&json!({"ok": false, "code": "answer_faults", "error": faults[0], "faults": faults.len()}))?;
        return Ok(0);
    }
    let mut zt = crate::zt::start()?;
    let path = store_of(&mut zt, &kind)?;
    let mut standard = term_standard::load(&path, &kind)?;
    let examples: HashMap<String, String> = book
        .rows
        .iter()
        .filter_map(|row| samples(&book, &row.term, 1).into_iter().next().map(|sentence| (row.id.clone(), sentence)))
        .collect();
    let before = standard.entries.len();
    term_standard::merge(&mut standard, &book.key, &book.rows, &sorted, &pairs, &examples);
    let equivalents = term_standard::equivalents(&standard, &book.key);
    // Addresses are read live: a Card may have moved since its book joined the list.
    let keys: BTreeSet<&String> = equivalents.iter().flat_map(|item| item.others.iter().map(|(key, _)| key)).chain([&book.key]).collect();
    let mut heads: HashMap<(String, String), String> = HashMap::new();
    for key in keys {
        for row in read_rows(&mut zt, key, "")?.map(|held| held.rows).unwrap_or_default() {
            if !row.head.is_empty() {
                heads.insert((key.clone(), row.term), row.head);
            }
        }
    }
    let mut planned: Vec<(String, Vec<String>)> = Vec::new();
    for item in &equivalents {
        let Some(head) = heads.get(&(book.key.clone(), item.term.clone())) else { continue };
        let links: Vec<String> = item.others.iter().filter_map(|(key, term)| heads.get(&(key.clone(), term.clone())).cloned()).collect();
        if !links.is_empty() {
            planned.push((head.clone(), links));
        }
    }
    let addresses: Vec<String> = planned.iter().map(|(head, _)| head.clone()).collect();
    let fresh = fresh_cards(&mut zt, &addresses)?;
    let mut items = Vec::new();
    for (head, links) in &planned {
        let card = fresh.get(head).with_context(|| format!("`{head}` is gone"))?;
        let body = ps::s(&card["body"]);
        let next = with_equivalents(&body, links);
        if next != body {
            items.push(json!({"tool": "edit_card", "arguments": {"at": head, "body": next, "base_fingerprint": crate::calls::authored_fingerprint(card)}}));
        }
    }
    let run = format!("term-sync-{}", &canon::sha256_hex(ps::json_compact(&json!([book.key, items])).as_bytes())[..16]);
    let linked = send(&mut zt, &run, "apply", items)?;
    term_standard::save(&path, &standard)?;
    util::print_last(&json!({
        "ok": true, "book": book.key, "aligned": pairs.len(), "same": sorted.same.len(), "cards": linked,
        "added": standard.entries.len() - before, "entries": standard.entries.len(),
    }))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARD: &str = "术语：撒拉森人\n不用：无\n类别：概念\n\n## 释义\n\n正文一句。\n\n## 书中的说法\n\n另一句。\n\n来源：[[k]] §1";

    fn link(address: &str) -> String {
        address.to_string()
    }

    #[test]
    fn the_link_line_closes_the_explanation_and_nothing_else_changes() {
        let text = with_equivalents(CARD, &[link("a/1|b"), link("c/2")]);
        assert_eq!(text, "术语：撒拉森人\n不用：无\n类别：概念\n\n## 释义\n\n正文一句。\n\n等义术语：[[a/1|b]]、[[c/2]]\n\n## 书中的说法\n\n另一句。\n\n来源：[[k]] §1");
    }

    #[test]
    fn a_later_run_replaces_the_line_and_adds_no_second_one() {
        let once = with_equivalents(CARD, &[link("a/1")]);
        assert_eq!(with_equivalents(&once, &[link("a/1")]), once);
        let twice = with_equivalents(&once, &[link("a/1"), link("c/2")]);
        assert_eq!(twice.matches("等义术语：").count(), 1, "{twice}");
        assert!(twice.contains("等义术语：[[a/1]]、[[c/2]]\n"), "{twice}");
    }

    #[test]
    fn a_card_with_only_an_explanation_keeps_its_source_line_last() {
        let text = with_equivalents("术语：甲\n\n## 释义\n\n正文。\n\n来源：[[k]] §1", &[link("a/1")]);
        assert_eq!(text, "术语：甲\n\n## 释义\n\n正文。\n\n等义术语：[[a/1]]\n\n来源：[[k]] §1");
    }

    #[test]
    fn a_job_with_no_answer_is_named() {
        // A missing answer is no decision: read as "nothing aligned", it would add every row as a new entry.
        let jobs = ["a001".to_string(), "a002".to_string()];
        assert_eq!(unanswered(&jobs, |job| job == "a001"), vec!["a002".to_string()]);
        assert!(unanswered(&jobs, |_| true).is_empty());
    }

    #[test]
    fn the_frozen_list_holds_only_the_entries_a_job_was_offered() {
        let standard = Standard { entries: vec![
            term_standard::Entry { id: "s1".into(), ..Default::default() },
            term_standard::Entry { id: "s2".into(), ..Default::default() },
        ], ..Standard::default() };
        let sorted = Triage { open: vec![("R1".into(), vec!["s2".into()])], ..Triage::default() };
        let frozen = offered_entries(&standard, &sorted);
        assert_eq!(frozen.entries.iter().map(|entry| entry.id.as_str()).collect::<Vec<_>>(), ["s2"]);
    }

    #[test]
    fn a_job_shows_each_row_with_its_candidates_and_nothing_else_of_the_list() {
        let standard = Standard { entries: vec![
            term_standard::Entry { id: "s1".into(), term: "撒拉森人".into(), meaning: "通称".into(), ..Default::default() },
            term_standard::Entry { id: "s2".into(), term: "无关".into(), ..Default::default() },
        ], ..Standard::default() };
        let rows = vec![Row { id: "R1".into(), term: "萨拉森人".into(), meaning: "称呼".into(), uses: 3, ..Row::default() }];
        let text = job_text(&rows, &[("R1".to_string(), vec!["s1".to_string()])], &standard);
        assert!(text.contains("R1\t萨拉森人\t\t称呼\t3\n  s1\t撒拉森人\t\t通称\n"), "{text}");
        assert!(!text.contains("无关"), "{text}");
    }
}
