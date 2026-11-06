//! `zt-digest term-review`: the replacement review of one aligned book (docs/term-align-design.md). The skill
//! `zt-term-review` conducts it. It reads the standard list and the live Cards, so it depends on no alignment
//! run and can run later or again.
//!
//!   zt-digest term-review -Step start  -Book <key> -Work <folder> [-Type history] [-Model <m>]
//!   zt-digest term-review -Step check  -Work <folder> -Job <id>      a job's SELF-CHECK
//!   zt-digest term-review -Step finish -Work <folder> -Relink <term-relink.ps1>
//!
//!   start   finds every remaining use of a shared entry's variants in the book, and writes the jobs. A job
//!           answers with ids, so an agent never changes a sentence.
//!   finish  replaces the chosen ids where the text has not changed since the start, and relinks the book.

use crate::args::Args;
use crate::canon;
use crate::ps;
use crate::term_align::{self, Book, around, flat, kept_terms, replace_at, scan_held};
use crate::term_link::spans::Profile;
use crate::term_standard::{self, Standard};
use crate::util;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

const REVIEW_DOC: &str = include_str!("../../../scripts/book-digest/dsh-term-residual-instructions.zh.md");
/// Occurrences of one review job: 2 tool calls and 0.15 a job, measured on three history books.
const OCCURRENCES_PER_JOB: usize = 150;
const JOBS_PER_MANAGER: usize = 8;
/// Characters of context on each side of an occurrence.
const CONTEXT: usize = 36;

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
    let a = Args::parse(argv, &["Step", "Work", "Type", "Book", "Job", "Model", "Relink"], &[])?;
    let p = Params {
        step: a.str("Step"),
        work: a.str("Work").trim_end_matches(['\\', '/']).to_string(),
        kind: a.str_or("Type", "history"),
        book: a.str("Book"),
        job: a.str("Job"),
        model: a.str_or("Model", crate::cited::MODEL),
        relink: a.str("Relink"),
    };
    if p.work.is_empty() {
        bail!("term-review: -Work is required");
    }
    match p.step.as_str() {
        "start" => start(&p),
        "check" => check(&p),
        "finish" => finish(&p),
        other => bail!("term-review: unknown -Step `{other}`"),
    }
}

/// The words a reader judges in a book: the variants of each entry that this book shares with another.
/// A book's other 不用 words stay R6's advice, because no other book asks to change them.
pub(crate) fn scope(standard: &Standard, key: &str) -> Vec<(String, String)> {
    let mut words: BTreeMap<String, String> = BTreeMap::new();
    for entry in standard.entries.iter().filter(|entry| entry.books.len() >= 2 && entry.books.contains_key(key)) {
        for word in &entry.variants {
            words.entry(word.clone()).or_insert_with(|| entry.term.clone());
        }
    }
    words.into_iter().collect()
}

fn held_path(p: &Params) -> String {
    ps::join(&p.work, "held.json")
}

fn answer_path(p: &Params, job: &str) -> String {
    ps::join(&ps::join(&p.work, "out"), &format!("{job}.json"))
}

fn scan(book: &Book, variants: &[(String, String)]) -> Vec<Value> {
    let kept = kept_terms(book, &[]);
    let variants: Vec<(String, String)> = variants.iter().filter(|(word, _)| !kept.contains(word)).cloned().collect();
    let mut held = Vec::new();
    for card in &book.cards {
        let address = ps::s(&card["address"]);
        let kind = ps::s(&card["type"]);
        if kind == "cited" || address == book.list {
            continue;
        }
        let (title, body) = (ps::s(&card["title"]), ps::s(&card["body"]));
        let name = card["term"].as_str().unwrap_or_default().to_string();
        let profile = Profile { own_term: (!name.is_empty()).then_some(name.as_str()), term_card: kind == "term" };
        for (field, text, found) in [
            ("body", &body, scan_held(&body, &variants, &kept, profile)),
            ("title", &title, scan_held(&title, &variants, &kept, Profile::default())),
        ] {
            for item in found {
                let (before, rest) = around(text, item.at..item.at + item.word.len(), CONTEXT);
                held.push(json!({
                    "address": address, "field": field, "at": item.at, "word": item.word, "term": item.term,
                    "hash": canon::sha256_hex(text.as_bytes()), "before": flat(&before), "after": flat(&rest),
                }));
            }
        }
    }
    held.sort_by_key(|item| (ps::s(&item["word"]), ps::s(&item["address"])));
    for (index, item) in held.iter_mut().enumerate() {
        item["id"] = json!(format!("o{}", index + 1));
    }
    held
}

fn start(p: &Params) -> Result<i32> {
    if p.book.is_empty() {
        bail!("term-review: -Book names the book to review");
    }
    std::fs::create_dir_all(ps::join(&p.work, "out"))?;
    let drive = ps::join(&p.work, "drive");
    std::fs::create_dir_all(&drive)?;
    let mut zt = crate::zt::start()?;
    let standard = term_standard::load(&term_align::store_of(&mut zt, &p.kind)?, &p.kind)?;
    let book = match term_align::read_book(&mut zt, &p.kind, &p.book)? {
        Ok(book) => book,
        Err(refusal) => {
            util::print_last(&refusal)?;
            return Ok(0);
        }
    };
    let held = scan(&book, &scope(&standard, &book.key));
    util::write_atomic(held_path(p), &ps::json_compact(&json!({"book": book.key, "held": held})))?;
    let mut jobs: Vec<Value> = Vec::new();
    for (index, part) in held.chunks(OCCURRENCES_PER_JOB).enumerate() {
        let id = format!("r{:03}", index + 1);
        let answer = answer_path(p, &id);
        let command = format!("zt-digest term-review -Step check -Work \"{}\" -Job {id}", p.work);
        let mut text = REVIEW_DOC.replace("{{ANSWER}}", &answer).replace("{{CHECK}}", &command);
        text.push_str("\n## 全部出现处\n\n");
        for item in part {
            let word = ps::s(&item["word"]);
            text.push_str(&format!("{}\t{word}→{}\t{}【{word}】{}\n", ps::s(&item["id"]), ps::s(&item["term"]), ps::s(&item["before"]), ps::s(&item["after"])));
        }
        let task = ps::join(&drive, &format!("{id}.md"));
        util::write_atomic(&task, &text)?;
        let ids: Vec<String> = part.iter().map(|item| ps::s(&item["id"])).collect();
        util::write_atomic(ps::join(&drive, &format!("{id}.ids.json")), &ps::json_compact(&json!(ids)))?;
        jobs.push(json!({"id": id, "files": [task], "prompt": "按任务说明判断全部出现处，写出答案并运行 SELF-CHECK，最后只回复一行 JSON。", "output": answer, "model": p.model}));
    }
    let file = ps::join(&drive, "jobs.json");
    util::write_atomic(&file, &ps::json_compact(&Value::Array(jobs.clone())))?;
    let mut lists = 0;
    for part in jobs.chunks(JOBS_PER_MANAGER) {
        lists += 1;
        util::write_atomic(ps::join(&drive, &format!("jobs-{lists:03}.json")), &ps::json_compact(&json!(part)))?;
    }
    util::print_last(&json!({"ok": true, "book": book.key, "held": held.len(), "jobs": jobs.len(), "lists": lists, "file": file}))?;
    Ok(0)
}

fn answer_ids(p: &Params, job: &str) -> Result<Vec<String>> {
    let answer = util::read_json(answer_path(p, job))?;
    Ok(answer["replace"].as_array().context("the answer has no `replace` list")?.iter().map(ps::s).collect())
}

fn check(p: &Params) -> Result<i32> {
    let known: HashSet<String> = util::read_json(ps::join(&ps::join(&p.work, "drive"), &format!("{}.ids.json", p.job)))?
        .as_array()
        .into_iter()
        .flatten()
        .map(ps::s)
        .collect();
    let mut lines = Vec::new();
    match answer_ids(p, &p.job) {
        Ok(ids) => lines.extend(ids.iter().filter(|id| !known.contains(*id)).map(|id| format!("{id} is no occurrence of this job"))),
        Err(error) => lines.push(format!("the answer file is not readable: {error}")),
    }
    let count = lines.len();
    lines.push(format!("faults: {count}"));
    util::print_lines(&lines)?;
    Ok(0)
}

fn finish(p: &Params) -> Result<i32> {
    if p.relink.is_empty() {
        bail!("term-review: -Relink names term-relink.ps1");
    }
    let saved = util::read_json(held_path(p)).context("term-review: run -Step start first")?;
    let key = ps::s(&saved["book"]);
    let held = saved["held"].as_array().cloned().unwrap_or_default();
    let jobs = held.len().div_ceil(OCCURRENCES_PER_JOB);
    let mut chosen: HashSet<String> = HashSet::new();
    let mut answered = 0;
    for index in 1..=jobs {
        if let Ok(ids) = answer_ids(p, &format!("r{index:03}")) {
            answered += 1;
            chosen.extend(ids);
        }
    }
    let mut by_text: BTreeMap<(String, String), Vec<&Value>> = BTreeMap::new();
    for item in held.iter().filter(|item| chosen.contains(&ps::s(&item["id"]))) {
        by_text.entry((ps::s(&item["address"]), ps::s(&item["field"]))).or_default().push(item);
    }
    let mut zt = crate::zt::start()?;
    let addresses: Vec<String> = by_text.keys().map(|(address, _)| address.clone()).collect::<HashSet<_>>().into_iter().collect();
    let fresh = term_align::fresh_cards(&mut zt, &addresses)?;
    let mut planned: BTreeMap<String, Value> = BTreeMap::new();
    let (mut stale, mut replaced) = (0, 0);
    for ((address, field), items) in &by_text {
        let Some(card) = fresh.get(address) else { continue };
        let text = ps::s(&card[field.as_str()]);
        if canon::sha256_hex(text.as_bytes()) != ps::s(&items[0]["hash"]) {
            stale += items.len();
            continue;
        }
        let edits: Vec<(usize, String, String)> = items
            .iter()
            .map(|item| (item["at"].as_u64().unwrap_or(0) as usize, ps::s(&item["word"]), ps::s(&item["term"])))
            .collect();
        let Some(next) = replace_at(&text, &edits) else {
            stale += items.len();
            continue;
        };
        replaced += items.len();
        let entry = planned.entry(address.clone()).or_insert_with(|| {
            json!({ "tool": "edit_card", "arguments": { "at": address, "base_fingerprint": crate::calls::authored_fingerprint(card) } })
        });
        entry["arguments"][field.as_str()] = json!(next);
    }
    let run = format!("term-review-{}", &canon::sha256_hex(format!("{}|{}", p.work, held.len()).as_bytes())[..16]);
    term_align::send(&mut zt, &run, "review", planned.into_values().collect())?;
    drop(zt);
    let links = term_align::relink(&p.relink, &key)?;
    util::print_last(&json!({
        "ok": answered == jobs, "book": key, "held": held.len(), "replaced": replaced, "stale": stale, "open": jobs - answered, "links_added": links,
        "error": if answered == jobs { String::new() } else { format!("{} review jobs gave no answer", jobs - answered) },
    }))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term_standard::Entry;

    fn entry(term: &str, variants: &[&str], books: &[&str]) -> Entry {
        Entry {
            term: term.into(),
            variants: variants.iter().map(ToString::to_string).collect(),
            books: books.iter().map(|book| ((*book).to_string(), term.to_string())).collect(),
            ..Entry::default()
        }
    }

    #[test]
    fn only_an_entry_a_second_book_shares_is_in_scope() {
        let standard = Standard {
            entries: vec![entry("改宗", &["皈依"], &["a", "b"]), entry("尊号", &["称号"], &["a"]), entry("圣训", &["言行"], &["b", "c"])],
            ..Standard::default()
        };
        // 称号 is a's own 不用 word and no other book asks to change it; 圣训 is not in book a.
        assert_eq!(scope(&standard, "a"), vec![("皈依".to_string(), "改宗".to_string())]);
    }

    fn book(rows: &[&str], body: &str) -> Book {
        let rows = rows.iter().enumerate().map(|(n, term)| term_standard::Row { id: format!("R{n}"), term: (*term).into(), ..term_standard::Row::default() }).collect();
        Book { key: "a".into(), title: "a".into(), list: "a/1|a".into(), rows, cards: vec![json!({"address": "a/1", "type": "idea", "title": "题", "body": body})] }
    }

    #[test]
    fn a_word_inside_a_kept_term_is_left_alone_and_each_use_gets_an_id() {
        let found = scan(&book(&["改宗", "皈依者"], "他皈依了。皈依者很多。他又皈依。"), &[("皈依".to_string(), "改宗".to_string())]);
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found.iter().map(|item| ps::s(&item["id"])).collect::<Vec<_>>(), ["o1", "o2"]);
        assert_eq!(found[0]["before"], "他");
    }

    #[test]
    fn a_variant_that_is_a_term_of_this_book_is_not_scanned() {
        let found = scan(&book(&["皈依"], "他皈依了。"), &[("皈依".to_string(), "改宗".to_string())]);
        assert!(found.is_empty(), "{found:?}");
    }
}
