//! `zt-digest term-align`: one book aligned to the standard terminology list of its Literature type
//! (docs/term-align-design.md). The skill `zt-term-align` conducts it: no Workflow, no staging, no ledger.
//!
//!   zt-digest term-align -Step status  [-Type history]
//!   zt-digest term-align -Step start   -Book <key> -Work <folder> [-Type history] [-Model <m>]
//!   zt-digest term-align -Step review  -Work <folder> [-Job <id>]
//!   zt-digest term-align -Step apply   -Work <folder> -Relink <term-relink.ps1>
//!   zt-digest term-align -Step respell -Entry <id> -Term <spelling> [-Type history]
//!   zt-digest term-align -Step check   [-Type history]
//!
//!   status  which books of the shelf the list knows.
//!   start   reads one book, settles its exact matches, and writes jobs for the rows with candidates. The
//!           first book of a shelf has none, so it seeds the list and spends nothing.
//!   review  the answer's faults and real sentences: the job's SELF-CHECK, and what Opus reads before apply.
//!   apply   renames this book's Terms, replaces what is safe in its prose and titles, enters the book into
//!           the list and relinks it. Earlier books are never touched.
//!   respell gives a standard term a better spelling. Each book that holds the old one follows at its next
//!           start and apply, with no agent: this is how Opus corrects a poor spelling the first book set.
//!   check   the cross-book Term check: a term of one book that another book of the shelf rejects.

use crate::args::Args;
use crate::canon;
use crate::ps;
use crate::term_link::spans::{self, Profile};
use crate::term_standard::{self, Rename, Row, Standard, Triage};
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::sync::LazyLock;

/// The align instructions, embedded so a job never depends on a checkout.
const ALIGN_DOC: &str = include_str!("../../../scripts/book-digest/dsh-term-align-instructions.zh.md");
/// Candidates of one row: one above the 5 that found every known pair on the three history books.
const CANDIDATES: usize = 6;
/// Rows of one align job.
const ROWS_PER_JOB: usize = 300;
/// An identified `apply_batch` call.
const BATCH_ITEMS: usize = 64;
const BATCH_BYTES: usize = 1 << 20;
/// `get_cards` takes at most this many addresses.
const READ_CARDS: usize = 200;
/// Characters of context on each side of an evidence sentence.
const EVIDENCE: usize = 42;
const SAMPLES: usize = 2;
/// A Han word shorter than this is never replaced without a reader.
const SAFE_HAN: usize = 3;

static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]@\n]+)@([^\]\n]+)\]\]").unwrap());
static LABEL_PLAIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]@\n]+)@[^\]\n]+\]\]").unwrap());
static HAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}]").unwrap());

struct Params {
    step: String,
    work: String,
    kind: String,
    book: String,
    job: String,
    model: String,
    relink: String,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(argv, &["Step", "Work", "Type", "Book", "Job", "Model", "Relink", "Entry", "Term"], &[])?;
    if a.str("Step") == "respell" {
        let kind = a.str_or("Type", "history");
        let mut zt = crate::zt::start()?;
        let path = store_of(&mut zt, &kind)?;
        let mut standard = term_standard::load(&path, &kind)?;
        term_standard::respell(&mut standard, &a.str("Entry"), &a.str("Term"))?;
        term_standard::save(&path, &standard)?;
        util::print_last(&json!({"ok": true, "entry": a.str("Entry"), "term": a.str("Term")}))?;
        return Ok(0);
    }
    let p = Params {
        step: a.str("Step"),
        work: a.str("Work").trim_end_matches(['\\', '/']).to_string(),
        kind: a.str_or("Type", "history"),
        book: a.str("Book"),
        job: a.str("Job"),
        model: a.str_or("Model", crate::cited::MODEL),
        relink: a.str("Relink"),
    };
    if !matches!(p.step.as_str(), "status" | "check") && p.work.is_empty() {
        bail!("term-align: -Work is required");
    }
    match p.step.as_str() {
        "status" => status(&p),
        "start" => start(&p),
        "review" => review(&p),
        "apply" => apply(&p),
        "check" => check(&p),
        other => bail!("term-align: unknown -Step `{other}`"),
    }
}

// --- The book ----------------------------------------------------------------------------------

/// One book: its rows, with its Cards when the step needs them.
#[derive(Clone, Debug)]
pub(crate) struct Book {
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
pub(crate) fn read_rows(zt: &mut Zt, key: &str, title: &str) -> Result<Option<Book>> {
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
pub(crate) fn read_book(zt: &mut Zt, kind: &str, key: &str) -> Result<std::result::Result<Book, Value>> {
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
pub(crate) fn store_of(zt: &mut Zt, kind: &str) -> Result<PathBuf> {
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
    let saved = util::read_json(book_path(p)).context("term-align: run -Step start first")?;
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
    let saved = util::read_json(triage_path(p)).context("term-align: run -Step start first")?;
    let pairs = |value: &Value| -> Vec<(String, String)> {
        value.as_array().into_iter().flatten().map(|pair| (ps::s(&pair[0]), ps::s(&pair[1]))).collect()
    };
    Ok(Triage {
        same: pairs(&saved["same"]),
        open: saved["open"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|item| (ps::s(&item[0]), item[1].as_array().into_iter().flatten().map(ps::s).collect()))
            .collect(),
        new: saved["new"].as_array().into_iter().flatten().map(ps::s).collect(),
        known: pairs(&saved["known"]),
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
        let follow = term_standard::must_follow(&standard, &key);
        let known = !follow && standard.aligned.get(&key).is_some_and(|hash| *hash == term_standard::rows_hash(&book.rows));
        lines.push(format!("{key}\t{} rows\t{}", book.rows.len(), if known { "aligned" } else if follow { "follows a respelled term" } else { "not aligned" }));
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
pub(crate) fn job_text(rows: &[Row], open: &[(String, Vec<String>)], standard: &Standard) -> String {
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
        bail!("term-align: -Book names the book to align");
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
        let command = format!("zt-digest term-align -Step review -Work \"{}\" -Job {id}", p.work);
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
pub(crate) fn around(text: &str, range: Range<usize>, width: usize) -> (String, String) {
    let before: Vec<char> = text[..range.start].chars().rev().take(width).collect();
    let after: String = text[range.end..].chars().take(width).collect();
    (before.into_iter().rev().collect(), after)
}

pub(crate) fn flat(text: &str) -> String {
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

// --- The replacement ---------------------------------------------------------------------------

/// A word in a body that a reader must judge.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Held {
    pub at: usize,
    pub word: String,
    pub term: String,
}

fn han_count(word: &str) -> usize {
    HAN.find_iter(word).count()
}

fn is_auto(old: &str) -> bool {
    han_count(old) >= SAFE_HAN || han_count(old) == 0
}

/// Whether the word at `range` lies inside a longer term the book keeps.
fn covered(body: &str, range: &Range<usize>, word: &str, kept: &[String]) -> bool {
    kept.iter().filter(|term| term.len() > word.len()).any(|term| {
        term.match_indices(word).any(|(offset, _)| {
            range.start >= offset && body.get(range.start - offset..).is_some_and(|rest| rest.starts_with(term.as_str()))
        })
    })
}

/// Every eligible place of `word` in the body: prose runs only, whole characters.
fn places(body: &str, word: &str, runs: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut found = Vec::new();
    for run in runs {
        let text = &body[run.clone()];
        let mut from = 0;
        while let Some(at) = text[from..].find(word) {
            let start = run.start + from + at;
            let end = start + word.len();
            if spans::boundaries_ok(body, start, end) {
                found.push(start..end);
            }
            from += at + word.len();
        }
    }
    found
}

/// The labels of the renamed Terms' links. A label must equal its Term's name in every Card, a Cited
/// Card too, whose prose stays as it is.
pub(crate) fn relabel(body: &str, renames: &[Rename]) -> (String, usize) {
    let mut count = 0;
    let relabelled = LABEL
        .replace_all(body, |caps: &regex::Captures<'_>| {
            let (label, target) = (&caps[1], caps[2].replace("\\|", "|"));
            match renames.iter().find(|rename| rename.old == label && rename.head == target) {
                Some(rename) => {
                    count += 1;
                    format!("[[{}@{}]]", rename.canon, &caps[2])
                }
                None => caps[0].to_string(),
            }
        })
        .into_owned();
    (relabelled, count)
}

/// Replace the labels of the renamed Terms' links, then the safe prose occurrences.
pub(crate) fn auto_edit(body: &str, renames: &[Rename], kept: &[String], profile: Profile<'_>) -> (String, usize) {
    let (relabelled, mut count) = relabel(body, renames);
    let runs = spans::layout(&relabelled, &profile).runs;
    let mut edits: Vec<(Range<usize>, &str)> = Vec::new();
    let mut ordered: Vec<&Rename> = renames.iter().filter(|rename| is_auto(&rename.old)).collect();
    ordered.sort_by_key(|rename| std::cmp::Reverse(rename.old.len()));
    for rename in ordered {
        for range in places(&relabelled, &rename.old, &runs) {
            let free = edits.iter().all(|(edit, _)| edit.end <= range.start || range.end <= edit.start);
            if free && !covered(&relabelled, &range, &rename.old, kept) {
                edits.push((range, &rename.canon));
            }
        }
    }
    count += edits.len();
    edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
    let mut text = relabelled;
    for (range, canon) in edits {
        text.replace_range(range, canon);
    }
    (text, count)
}

/// Every remaining use of a word a reader must judge, longest word first at each place.
pub(crate) fn scan_held(body: &str, variants: &[(String, String)], kept: &[String], profile: Profile<'_>) -> Vec<Held> {
    let runs = spans::layout(body, &profile).runs;
    let mut taken: Vec<Range<usize>> = Vec::new();
    let mut held = Vec::new();
    let mut ordered: Vec<&(String, String)> = variants.iter().collect();
    ordered.sort_by_key(|(word, _)| std::cmp::Reverse(word.len()));
    for (word, term) in ordered {
        for range in places(body, word, &runs) {
            let free = taken.iter().all(|edit| edit.end <= range.start || range.end <= edit.start);
            if free && !covered(body, &range, word, kept) {
                held.push(Held { at: range.start, word: word.clone(), term: term.clone() });
                taken.push(range);
            }
        }
    }
    held.sort_by_key(|item| item.at);
    held
}

/// The Term list table with its rows renamed; each old name joins the row's 不用 cell.
pub(crate) fn rewrite_table(body: &str, renames: &[Rename]) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in body.split('\n') {
        let mut cells: Vec<String> = line.split('|').map(str::to_string).collect();
        let hit = (cells.len() >= 5)
            .then(|| renames.iter().find(|rename| cells[1].trim() == rename.old))
            .flatten();
        let Some(rename) = hit else {
            lines.push(line.to_string());
            continue;
        };
        let mut avoid: Vec<String> = cells[2]
            .trim()
            .split('、')
            .map(|word| word.trim().to_string())
            .filter(|word| !word.is_empty() && word != "—" && *word != rename.canon)
            .collect();
        avoid.push(rename.old.clone());
        cells[1] = format!(" {} ", rename.canon);
        cells[2] = format!(" {} ", avoid.join("、"));
        lines.push(cells.join("|"));
    }
    lines.join("\n")
}

/// A Term Card's head lines under its new name.
pub(crate) fn rewrite_head(body: &str, rename: &Rename) -> String {
    body.split('\n')
        .map(|line| {
            if let Some(rest) = line.strip_prefix("术语：").filter(|rest| rest.trim() == rename.old) {
                let _ = rest;
                format!("术语：{}", rename.canon)
            } else if let Some(rest) = line.strip_prefix("不用：") {
                let mut words: Vec<&str> = rest.split('、').map(str::trim).filter(|word| !word.is_empty() && *word != "无" && *word != rename.canon).collect();
                words.push(&rename.old);
                format!("不用：{}", words.join("、"))
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The terms a book keeps after its renames, longest first.
pub(crate) fn kept_terms(book: &Book, renames: &[Rename]) -> Vec<String> {
    let mut kept: Vec<String> = book
        .rows
        .iter()
        .map(|row| renames.iter().find(|rename| rename.old == row.term).map_or_else(|| row.term.clone(), |rename| rename.canon.clone()))
        .collect();
    kept.sort_by_key(|term| std::cmp::Reverse(term.len()));
    kept
}

/// One planned Card change.
struct Change {
    address: String,
    title: String,
    body: String,
    term: Option<String>,
    base_title: String,
    base_body: String,
}

fn changes_of(book: &Book, renames: &[Rename], kept: &[String]) -> (Vec<Change>, usize) {
    let mut changes = Vec::new();
    let mut replaced = 0;
    for card in &book.cards {
        let address = ps::s(&card["address"]);
        let kind = ps::s(&card["type"]);
        let (base_title, base_body) = (ps::s(&card["title"]), ps::s(&card["body"]));
        if kind == "cited" {
            let (body, count) = relabel(&base_body, renames);
            if count > 0 {
                replaced += count;
                changes.push(Change { address, title: base_title.clone(), body, term: None, base_title, base_body });
            }
            continue;
        }
        if address == book.list {
            let body = rewrite_table(&base_body, renames);
            if body != base_body {
                changes.push(Change { address, title: base_title.clone(), body, term: None, base_title, base_body });
            }
            continue;
        }
        let name = card["term"].as_str().unwrap_or_default().to_string();
        let own = renames.iter().find(|rename| rename.old == name);
        let (mut body, mut title) = (base_body.clone(), base_title.clone());
        let profile = Profile { own_term: if own.is_some() || name.is_empty() { None } else { Some(name.as_str()) }, term_card: kind == "term" };
        if let Some(rename) = own {
            body = rewrite_head(&body, rename);
            if title == rename.old {
                title = rename.canon.clone();
            }
        }
        let (next, count) = auto_edit(&body, renames, kept, profile);
        replaced += count;
        body = next;
        if !(own.is_some() && title == own.map_or("", |rename| rename.canon.as_str())) {
            let (next, count) = auto_edit(&title, renames, kept, Profile::default());
            replaced += count;
            title = next;
        }
        if body != base_body || title != base_title {
            changes.push(Change { address, title, body, term: own.map(|rename| rename.canon.clone()), base_title, base_body });
        }
    }
    (changes, replaced)
}

/// Read Cards fresh, in order, with the fields `authored_fingerprint` needs.
pub(crate) fn fresh_cards(zt: &mut Zt, addresses: &[String]) -> Result<HashMap<String, Value>> {
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
pub(crate) fn send(zt: &mut Zt, run: &str, phase: &str, items: Vec<Value>) -> Result<usize> {
    let mut sent = 0;
    let mut batch: Vec<Value> = Vec::new();
    let mut bytes = 0;
    let mut flush = |zt: &mut Zt, batch: &mut Vec<Value>| -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let reply = zt.request("apply_batch", json!({ "items": batch }))?;
        if let Some(message) = zt_check::mcp::reply_error(&reply) {
            bail!("apply_batch refused a term-align edit: {message}");
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

fn edit_item(change: &Change, card: &Value) -> Value {
    let mut arguments = json!({
        "at": change.address, "body": change.body,
        "base_fingerprint": crate::calls::authored_fingerprint(card),
    });
    if change.title != change.base_title {
        arguments["title"] = json!(change.title);
    }
    if let Some(term) = &change.term {
        arguments["term"] = json!(term);
    }
    json!({ "tool": "edit_card", "arguments": arguments })
}

/// Replace chosen byte ranges of one text, last first.
pub(crate) fn replace_at(text: &str, chosen: &[(usize, String, String)]) -> Option<String> {
    let mut out = text.to_string();
    let mut order: Vec<&(usize, String, String)> = chosen.iter().collect();
    order.sort_by_key(|(at, _, _)| std::cmp::Reverse(*at));
    for (at, word, term) in order {
        if !out.get(*at..).is_some_and(|rest| rest.starts_with(word.as_str())) {
            return None;
        }
        out.replace_range(*at..*at + word.len(), term);
    }
    Some(out)
}

/// A term of one book that another book of the shelf rejects, unless that book uses it too.
pub(crate) fn cross_faults(books: &[Book]) -> Vec<String> {
    let mut faults = Vec::new();
    for book in books {
        for other in books.iter().filter(|other| other.key != book.key) {
            let terms: HashSet<&String> = other.rows.iter().map(|row| &row.term).collect();
            for row in other.rows.iter() {
                for word in row.avoid.iter().filter(|word| !terms.contains(word)) {
                    if book.rows.iter().any(|mine| mine.term == *word) {
                        faults.push(format!("{}: `{word}` is rejected by {} for `{}`", book.key, other.key, row.term));
                    }
                }
            }
        }
    }
    faults
}

fn apply(p: &Params) -> Result<i32> {
    if p.relink.is_empty() {
        bail!("term-align: -Relink names term-relink.ps1");
    }
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
    let renames = term_standard::merge(&mut standard, &book.key, &book.rows, &sorted, &pairs, &examples);
    let kept = kept_terms(&book, &renames);
    let (mut changes, replaced) = changes_of(&book, &renames, &kept);
    // The list first: a Term Card's new name must find its row.
    changes.sort_by_key(|change| change.address != book.list);
    let addresses: Vec<String> = changes.iter().map(|change| change.address.clone()).collect();
    let fresh = fresh_cards(&mut zt, &addresses)?;
    let mut items = Vec::new();
    for change in &changes {
        let card = fresh.get(&change.address).with_context(|| format!("`{}` is gone", change.address))?;
        let (title, body) = (ps::s(&card["title"]), ps::s(&card["body"]));
        if title == change.title && body == change.body {
            continue; // an earlier apply wrote it
        }
        if title != change.base_title || body != change.base_body {
            util::print_last(&json!({"ok": false, "code": "plan_stale", "error": format!("`{}` changed since the start; run the start again", change.address)}))?;
            return Ok(0);
        }
        items.push(edit_item(change, card));
    }
    let edited = items.len();
    let run = format!("term-align-{}", &canon::sha256_hex(format!("{}|{}", book.key, ps::json_compact(&json!(pairs))).as_bytes())[..16]);
    send(&mut zt, &run, "apply", items)?;
    term_standard::save(&path, &standard)?;
    drop(zt);
    let links = relink(&p.relink, &book.key)?;
    util::print_last(&json!({
        "ok": true, "book": book.key, "aligned": pairs.len(), "renamed": renames.len(), "cards": edited, "replaced": replaced,
        "same": sorted.same.len(), "added": standard.entries.len() - before, "entries": standard.entries.len(), "links_added": links,
    }))?;
    Ok(0)
}

/// Bring a book's Term links up to date; answers the links added.
pub(crate) fn relink(script: &str, key: &str) -> Result<i64> {
    let output = std::process::Command::new("pwsh")
        .args(["-NoProfile", "-File", script, "-CitationKey", key])
        .output()
        .context("failed to start term-relink.ps1")?;
    let text = String::from_utf8_lossy(&output.stdout);
    let last = text.lines().rev().find(|line| line.trim_start().starts_with('{')).unwrap_or_default();
    let line: Value = serde_json::from_str(last).unwrap_or(json!({}));
    if line["ok"] != true {
        bail!("term-relink of {key} failed: {}", ps::s(&line["error"]));
    }
    Ok(line["links_added"].as_i64().unwrap_or(0))
}

fn check(p: &Params) -> Result<i32> {
    let mut zt = crate::zt::start()?;
    let mut books = Vec::new();
    for (key, title, _) in shelf_keys(&mut zt, &p.kind)? {
        if let Some(book) = read_rows(&mut zt, &key, &title)? {
            books.push(book);
        }
    }
    let mut lines = cross_faults(&books);
    let count = lines.len();
    lines.push(format!("faults: {count}"));
    util::print_lines(&lines)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rename(old: &str, canon: &str, head: &str) -> Rename {
        Rename { old: old.into(), canon: canon.into(), head: head.into() }
    }

    fn row(term: &str, avoid: &[&str]) -> Row {
        Row { term: term.into(), avoid: avoid.iter().map(ToString::to_string).collect(), ..Row::default() }
    }

    fn book(key: &str, rows: Vec<Row>) -> Book {
        Book { key: key.into(), title: key.into(), list: format!("{key}/1|a"), rows, cards: Vec::new() }
    }

    #[test]
    fn labels_and_long_words_change_and_source_lines_do_not() {
        let renames = [rename("萨拉森人", "撒拉森人", "b/1")];
        let kept = vec!["撒拉森人".to_string()];
        let body = "[[萨拉森人@b/1]]进入城市。后来萨拉森人离开。\n\n来源：[[b]] 萨拉森人史";
        let (text, count) = auto_edit(body, &renames, &kept, Profile::default());
        assert_eq!(text, "[[撒拉森人@b/1]]进入城市。后来撒拉森人离开。\n\n来源：[[b]] 萨拉森人史");
        assert_eq!(count, 2);
    }

    #[test]
    fn a_word_inside_a_longer_kept_term_stays_and_a_short_word_goes_to_a_reader() {
        let renames = [rename("类比", "类比推理", "b/1"), rename("人头税", "人丁税", "b/2")];
        let kept = vec!["类比推理".to_string(), "人头税额".to_string(), "人丁税".to_string()];
        let body = "类比推理很常见。这是类比。人头税额很高，人头税很重。";
        let (text, count) = auto_edit(body, &renames, &kept, Profile::default());
        assert_eq!(text, "类比推理很常见。这是类比。人头税额很高，人丁税很重。");
        assert_eq!(count, 1);
        let held = scan_held(&text, &[("类比".into(), "类比推理".into())], &kept, Profile::default());
        assert_eq!(held.len(), 1, "{held:?}");
        assert_eq!(&text[held[0].at..held[0].at + held[0].word.len()], "类比");
        assert!(text[..held[0].at].ends_with("这是"));
    }

    #[test]
    fn a_cited_card_takes_the_new_label_and_keeps_its_prose() {
        let (text, count) = relabel("皈依史。[[皈依@b/1]]见此。", &[rename("皈依", "改宗", "b/1")]);
        assert_eq!((text.as_str(), count), ("皈依史。[[改宗@b/1]]见此。", 1));
    }

    #[test]
    fn a_label_of_another_term_keeps_its_name() {
        let renames = [rename("皈依", "改宗", "b/1")];
        let (text, _) = auto_edit("[[皈依@a/9]]与[[皈依@b/1]]", &renames, &[], Profile::default());
        assert_eq!(text, "[[皈依@a/9]]与[[改宗@b/1]]");
    }

    #[test]
    fn an_address_with_an_escaped_bar_still_matches_its_head() {
        let renames = [rename("皈依", "改宗", "b/10|et")];
        let (text, _) = auto_edit(r"[[皈依@b/10\|et]]", &renames, &[], Profile::default());
        assert_eq!(text, r"[[改宗@b/10\|et]]");
    }

    #[test]
    fn the_table_row_takes_the_canon_and_keeps_the_old_name_as_a_rejected_word() {
        let table = "| 术语 | 不用 | 意思 | 首次定义 |\n|---|---|---|---|\n| 萨拉森人 | 撒拉森人、撒拉逊人 | 穆斯林的通称 | §1 |\n| 其他 | — | 别的 | §2 |";
        let text = rewrite_table(table, &[rename("萨拉森人", "撒拉森人", "b/1")]);
        assert!(text.contains("| 撒拉森人 | 撒拉逊人、萨拉森人 | 穆斯林的通称 | §1 |"), "{text}");
        assert!(text.contains("| 其他 | — | 别的 | §2 |"));
    }

    #[test]
    fn a_term_card_head_takes_the_new_name_and_rejects_the_old() {
        let body = "术语：萨拉森人\n不用：撒拉逊人\n类别：概念\n\n## 释义\n\n正文。";
        let text = rewrite_head(body, &rename("萨拉森人", "撒拉森人", "b/1"));
        assert!(text.starts_with("术语：撒拉森人\n不用：撒拉逊人、萨拉森人\n类别：概念"), "{text}");
        let none = rewrite_head("术语：甲\n不用：无", &rename("甲", "乙", "b/1"));
        assert_eq!(none, "术语：乙\n不用：甲");
    }

    #[test]
    fn a_chosen_occurrence_is_replaced_in_place_and_a_moved_one_refuses() {
        let text = "这是类比，那也是类比。";
        let second = text.rfind("类比").unwrap();
        assert_eq!(replace_at(text, &[(second, "类比".into(), "类比推理".into())]).unwrap(), "这是类比，那也是类比推理。");
        assert_eq!(replace_at(text, &[(1, "类比".into(), "甲".into())]), None);
    }

    #[test]
    fn the_check_names_a_term_another_book_rejects_unless_that_book_also_uses_it() {
        let shelf = vec![book("a", vec![row("甲", &[])]), book("b", vec![row("乙", &["甲"])])];
        assert_eq!(cross_faults(&shelf), vec!["a: `甲` is rejected by b for `乙`".to_string()]);
        let aligned = vec![book("a", vec![row("甲", &[])]), book("b", vec![row("甲", &["乙"])])];
        assert!(cross_faults(&aligned).is_empty());
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
