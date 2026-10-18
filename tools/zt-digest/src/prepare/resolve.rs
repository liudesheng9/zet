//! `prepare -Book`: resolve a book from the Scholar Archive by its Work id or title.
//!
//! The resolution uses only the existing read-only bridge on `zt mcp`
//! (`scholar_archive_read` for `get_work`, `list_works` and `list_work_files`, and
//! `scholar_archive_file` for the document itself). It writes nothing to the archive: the
//! document lands in ZET's own cache, and the only file it writes is the caller's
//! `preparation.json`.
//!
//! A Work id resolves exactly. A title resolves only on one exact match (after the title is
//! normalised); otherwise the result lists candidate Works with their files for a selection.
//! The Work's present, linked Markdown files are ranked by their saved metadata
//! (`chinese_bilingual_markdown`, then a bilingual/Chinese file name, then `ocr_markdown`,
//! then any other Markdown), and several files of the best rank are a selection, never a
//! silent first pick. BibTeX is generated from the saved metadata; a supplied Citation key
//! overrides the entry key after the archive collision check. The resolved start, end and
//! skip headings, the source hash and every excluded section are persisted before any group
//! work.

use crate::calls;
use crate::canon::sha256_hex;
use crate::prepare::HEADING;
use crate::ps::{self, eq, join_path, len16, s};
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::path::Path;

/// The exit code of a preparation that stops on an unresolved choice. Only the preparation
/// result was written; no group work exists.
pub const NEEDS_SELECTION: i32 = 3;
/// The Works a selection result lists.
const MAX_CANDIDATES: usize = 10;
/// The pages of one Work's file list the resolution reads before it calls the list incomplete.
const MAX_FILE_PAGES: usize = 20;

/// Headings that open a book's front matter: the start heading is proposed after them.
const FRONT: &[&str] = &[
    "preface",
    "foreword",
    "contents",
    "table of contents",
    "cover",
    "title page",
    "copyright",
    "dedication",
    "abstract",
    "front matter",
    "前言",
    "序",
    "序言",
    "目录",
    "内容简介",
    "摘要",
    "出版说明",
    "编者的话",
    "译者的话",
];

/// Headings that open a book's back matter: the first one after the start heading is the end.
const BACK: &[&str] = &[
    "references",
    "bibliography",
    "reference",
    "works cited",
    "index",
    "appendix",
    "appendices",
    "notes",
    "glossary",
    "afterword",
    "epilogue",
    "参考",
    "参考文献",
    "引用文献",
    "索引",
    "附录",
    "注释",
    "术语表",
    "后记",
    "跋",
    "致谢",
    "鸣谢",
    "译后记",
];

/// Headings whose sections the digest leaves out.
const SKIP: &[&str] = &[
    "习题",
    "练习题",
    "练习",
    "思考题",
    "讨论题",
    "exercises",
    "problems",
    "questions",
    "further reading",
    "延伸阅读",
    "参考文献",
    "bibliography",
    "references",
    "索引",
    "index",
    "附录",
    "appendix",
    "appendices",
    "注释",
    "notes",
    "术语表",
    "glossary",
    "致谢",
    "acknowledgements",
    "acknowledgments",
    "译后记",
];

/// What the caller passed beside `-Book`. Every `*_given` flag says whether the caller named
/// that argument, so an explicit value and a resolved value are compared (a conflict is an
/// input error) instead of one silently overriding the other.
pub struct Explicit {
    pub source: String,
    pub source_given: bool,
    pub file_id: i64,
    pub citation_key: String,
    pub citation_key_given: bool,
    pub bibtex: String,
    pub bibtex_given: bool,
    pub book_title: String,
    pub book_title_given: bool,
    pub start: Option<String>,
    pub end: Option<String>,
    pub skip: Option<String>,
}

/// A frozen preparation: every choice the rest of `prepare` reads.
pub struct Resolved {
    pub source: String,
    pub file_id: i64,
    pub citation_key: String,
    pub bibtex: String,
    pub book_title: String,
    pub start: String,
    pub end: String,
    pub skip: String,
    /// `原书文档：…` from the resolved Work's shown files: no second archive read.
    pub documents: String,
    /// Path of the persisted read-only preparation result.
    pub preparation: String,
    /// The manifest's compact record of the resolution.
    pub summary: Value,
}

/// Either the frozen preparation or the choices that still need a caller.
pub enum Outcome {
    Ready(Box<Resolved>),
    NeedsSelection(Value),
}

/// One resolved Work: the fields the preparation needs, from `get_work`.
#[derive(Clone)]
struct Work {
    id: i64,
    title: String,
    authors: String,
    year: Option<i64>,
    venue: String,
    doi: String,
    /// The BibTeX field names the saved metadata actually gave.
    fields: Vec<String>,
}

/// One Work's file list, or the candidates for a selection.
enum Found<T> {
    One(T),
    Choose { message: String, candidates: Vec<Value> },
}

/// One heading of the resolved source, in reading order.
#[derive(Clone, Debug)]
struct Heading {
    title: String,
    level: usize,
    /// 1-based line of the resolved source file.
    line: usize,
    /// UTF-16 characters from this heading to the next heading of the same or a higher level.
    chars: usize,
}

/// What the heading scan proposes when the caller named no range.
struct Proposal {
    start: usize,
    end: Option<usize>,
    skip: Vec<usize>,
}

/// The frozen heading range: what the run reads and what it leaves out.
struct Range {
    start: usize,
    end: Option<usize>,
    skip: Vec<usize>,
    start_heading: String,
    end_heading: String,
    start_line: usize,
    end_line: usize,
    start_explicit: bool,
    end_explicit: bool,
    skip_explicit: bool,
    skip_join: String,
    included: usize,
}

/// Resolve `-Book` and freeze every choice it settles.
///
/// `out` is the caller's staging folder: the read-only preparation result is written there as
/// soon as the Work and its file are known, before any group work exists.
pub fn run(zt: &mut Zt, out: &str, book: &str, x: &Explicit) -> Result<Outcome> {
    let staging = ps::full_name(out);
    std::fs::create_dir_all(&staging).with_context(|| format!("failed to make {staging}"))?;
    let preparation = join_path(&staging, "preparation.json");
    let mut result = Map::new();
    result.insert("schema_version".into(), json!(1));
    result.insert("status".into(), json!("needs_selection"));
    result.insert("requested".into(), json!({"book":book,"kind":requested_kind(book)}));
    result.insert("work".into(), Value::Null);
    result.insert("file".into(), Value::Null);
    result.insert("citation".into(), Value::Null);
    result.insert("headings".into(), Value::Null);
    result.insert("selections".into(), json!([]));
    result.insert("preparation".into(), json!(preparation));

    // --- the Work ---
    let work = match resolve_work(zt, book)? {
        Found::One(work) => work,
        Found::Choose { message, candidates } => {
            return selection(&preparation, result, "work", "ambiguous_title", &message, candidates);
        }
    };
    result.insert("work".into(), work_record(&work));

    // --- the Work's Markdown ---
    let files = work_files(zt, work.id)?;
    let (file, rank) = match choose_file(&work, &files, x.file_id)? {
        Found::One((file, rank)) => (file, rank),
        Found::Choose { message, candidates } => {
            return selection(&preparation, result, "file", "several_markdown_files", &message, candidates);
        }
    };
    let (path, sha256, bytes) = fetch_document(zt, &work, &file)?;
    let file_id = file["file_id"].as_i64().context("the chosen file has no id")?;
    result.insert(
        "file".into(),
        json!({"file_id":file_id,"name":file["name"].clone(),
            "relative_path":file["relative_path"].clone(),"display_role":file["display_role"].clone(),
            "shown":file["shown"].clone(),"preference":preference_name(rank),
            "path":path,"bytes":bytes,"sha256":sha256}),
    );

    // --- the Citation key, its BibTeX and the book title ---
    let (key, key_source) = citation_key(zt, &work, x)?;
    let bibtex = bibtex_text(&work, &key, x)?;
    let book_title = book_title(&work, x)?;
    result.insert(
        "citation".into(),
        json!({"key":key,"source":key_source,"entry_type":"book","fields":work.fields,"bibtex":bibtex}),
    );

    // --- the source and its frozen heading range ---
    if x.source_given {
        check_source(&work, &path, &sha256, x)?;
    }
    let raw = read_source(&path)?;
    let headings = scan_headings(&raw);
    let range = resolve_range(&work, &path, &headings, x)?;
    let excluded = excluded_sections(&headings, &range);
    result.insert(
        "headings".into(),
        json!({"source_sha256":sha256,
            "start":{"heading":range.start_heading,"line":range.start_line,"explicit":range.start_explicit},
            "end":if range.end_heading.is_empty() {Value::Null}
                else {json!({"heading":range.end_heading,"line":range.end_line,"explicit":range.end_explicit})},
            "skip":skip_titles(&headings, &range.skip),
            "skip_explicit":range.skip_explicit,
            "top":top_headings(&headings, &range),
            "included":range.included,
            "excluded":excluded,
            "preamble":preamble_of(&raw, &range)}),
    );
    result.insert("status".into(), json!("ready"));
    result.insert("selections".into(), json!([]));
    result.insert(
        "frozen".into(),
        json!({"source":path,"file_id":file_id,"source_sha256":sha256,
            "citation_key":key,"book_title":book_title,"start_heading":range.start_heading,
            "end_heading":range.end_heading,"skip_headings":range.skip_join}),
    );
    let result = Value::Object(result);
    util::write_atomic(&preparation, &ps::json_pretty(&result))?;

    let summary = json!({
        "requested":{"book":book,"kind":requested_kind(book)},
        "work":work_record(&work),
        "file":{"file_id":file_id,"name":file["name"].clone(),
            "relative_path":file["relative_path"].clone(),"display_role":file["display_role"].clone(),
            "preference":preference_name(rank),"bytes":bytes,"sha256":sha256},
        "citation":{"key":key,"source":key_source,"fields":work.fields},
        "headings":{"start":range.start_heading,"end":range.end_heading,
            "skip":skip_titles(&headings, &range.skip),
            "explicit":{"start":range.start_explicit,"end":range.end_explicit,"skip":range.skip_explicit},
            "included":range.included,"excluded":excluded.len(),"source_sha256":sha256},
        "preparation":preparation,
    });
    Ok(Outcome::Ready(Box::new(Resolved {
        source: path,
        file_id,
        citation_key: key,
        bibtex,
        book_title,
        start: range.start_heading.clone(),
        end: range.end_heading.clone(),
        skip: range.skip_join.clone(),
        documents: document_line(&files),
        preparation,
        summary,
    })))
}

fn requested_kind(book: &str) -> &'static str {
    if work_id_of(book).is_some() { "work_id" } else { "title" }
}

/// Write the read-only preparation result of an unresolved choice and hand it back.
fn selection(
    preparation: &str,
    mut result: Map<String, Value>,
    kind: &str,
    reason: &str,
    message: &str,
    candidates: Vec<Value>,
) -> Result<Outcome> {
    result.insert(
        "selections".into(),
        json!([{"kind":kind,"reason":reason,"message":message,"candidates":candidates}]),
    );
    let result = Value::Object(result);
    util::write_atomic(preparation, &ps::json_pretty(&result))?;
    Ok(Outcome::NeedsSelection(result))
}

fn work_id_of(book: &str) -> Option<i64> {
    let text = book.trim();
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok().filter(|id| *id > 0)
}

fn work_record(work: &Work) -> Value {
    json!({"id":work.id,"title":work.title,"authors":work.authors,"year":work.year,
        "venue":work.venue,"doi":if work.doi.is_empty() {Value::Null} else {json!(work.doi)}})
}

/// The title as it is compared: whitespace runs collapse and case is ignored.
fn normalize_title(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn work_of(value: &Value) -> Result<Work> {
    let id = value["id"]
        .as_i64()
        .or_else(|| value["id"].as_str().and_then(|text| text.trim().parse().ok()))
        .with_context(|| format!("Scholar Archive returned a Work without an id: {value}"))?;
    let title = s(&value["title"]);
    if title.trim().is_empty() {
        bail!("Scholar Archive Work {id} has no title");
    }
    let authors = {
        let full = s(&value["source_fields"]["author"]);
        if full.trim().is_empty() { s(&value["authors"]) } else { full }
    };
    let mut work = Work {
        id,
        title,
        authors,
        year: value["year"].as_i64(),
        venue: s(&value["venue"]),
        doi: s(&value["doi"]),
        fields: Vec::new(),
    };
    work.fields = bibtex_fields(&work);
    Ok(work)
}

/// The BibTeX fields the saved metadata gave, in the order the entry writes them.
fn bibtex_fields(work: &Work) -> Vec<String> {
    let mut fields = Vec::new();
    if !work.authors.trim().is_empty() {
        fields.push("author".to_string());
    }
    fields.push("title".to_string());
    if work.year.is_some() {
        fields.push("year".to_string());
    }
    if !work.venue.trim().is_empty() {
        fields.push("publisher".to_string());
    }
    if !work.doi.trim().is_empty() {
        fields.push("doi".to_string());
    }
    fields
}

/// One bridge read: the tool's own result, with the bridge's failure made specific.
fn read(zt: &mut Zt, tool: &str, arguments: Value) -> Result<Value> {
    let answer = calls::read(zt, "scholar_archive_read", json!({"tool":tool,"arguments":arguments}))
        .with_context(|| format!("the Scholar Archive bridge cannot run `{tool}`"))?;
    Ok(answer["result"].clone())
}

/// The Work a `-Book` value names, or the Works a title must choose among.
fn resolve_work(zt: &mut Zt, book: &str) -> Result<Found<Work>> {
    if let Some(id) = work_id_of(book) {
        let record = read(zt, "get_work", json!({"work_id":id})).map_err(|error| {
            anyhow::anyhow!(
                "the Scholar Archive has no Work {id}: {error:#}; pass the book's Markdown with -Source (and -FileId) instead"
            )
        })?;
        let work = work_of(&record)?;
        if work.id != id {
            bail!("the Scholar Archive answered Work {} for Work {id}", work.id);
        }
        return Ok(Found::One(work));
    }
    let answer = read(zt, "list_works", json!({"query":book,"limit":50})).map_err(|error| {
        anyhow::anyhow!(
            "the Scholar Archive cannot search for the title `{book}`: {error:#}; pass the Work id as -Book, or the Markdown with -Source"
        )
    })?;
    let items = ps::each(&answer["items"]).into_iter().cloned().collect::<Vec<Value>>();
    let wanted = normalize_title(book);
    let exact = items
        .iter()
        .filter(|item| normalize_title(&s(&item["title"])) == wanted)
        .collect::<Vec<_>>();
    if let [only] = exact.as_slice() {
        let id = only["id"]
            .as_i64()
            .with_context(|| format!("Scholar Archive returned a Work without an id: {only}"))?;
        let record = read(zt, "get_work", json!({"work_id":id}))?;
        return Ok(Found::One(work_of(&record)?));
    }
    if items.is_empty() {
        bail!(
            "the Scholar Archive has no Work matching `{book}`; pass the Work id as -Book, or the Markdown with -Source"
        );
    }
    let mut candidates = Vec::new();
    for item in items.iter().take(MAX_CANDIDATES) {
        let mut candidate = item.clone();
        let id = item["id"].as_i64().unwrap_or_default();
        if id > 0 && let Ok(files) = work_files(zt, id) {
            candidate["files"] = json!(files.iter().map(file_record).collect::<Vec<Value>>());
        }
        candidates.push(candidate);
    }
    let message = if exact.len() > 1 {
        format!("{} Works are titled `{book}`; pass one of their ids as -Book", exact.len())
    } else {
        format!(
            "no Work is titled exactly `{book}`; {} Works match it; pass one of their ids as -Book",
            items.len()
        )
    };
    Ok(Found::Choose { message, candidates })
}

/// One file of a selection result.
fn file_record(file: &Value) -> Value {
    json!({"file_id":file["file_id"].clone(),"name":file["name"].clone(),
        "relative_path":file["relative_path"].clone(),"display_role":file["display_role"].clone(),
        "shown":file["shown"].clone(),"preference":preference(file).map(preference_name)})
}

/// Every page of one Work's linked, present files.
fn work_files(zt: &mut Zt, work_id: i64) -> Result<Vec<Value>> {
    let mut files: Vec<Value> = Vec::new();
    let mut after: i64 = 0;
    for _ in 0..MAX_FILE_PAGES {
        let answer = read(
            zt,
            "list_work_files",
            json!({"work_id":work_id,"link_state":"linked","availability":"present",
                "limit":200,"after_file_id":after}),
        )?;
        let page = ps::each(&answer["items"]).into_iter().cloned().collect::<Vec<Value>>();
        files.extend(page.iter().filter(|file| file["work_id"].as_i64() == Some(work_id)).cloned());
        match answer["next_after_file_id"].as_i64() {
            Some(next) if next > after => after = next,
            _ => return Ok(files),
        }
    }
    bail!("the Scholar Archive's file list of Work {work_id} is incomplete after {MAX_FILE_PAGES} pages")
}

/// How well a saved file suits the digest: 1 the Work's bilingual Markdown, 2 a bilingual or
/// Chinese file name, 3 its aligned OCR Markdown, 4 any other Markdown. `None` is no source.
fn preference(file: &Value) -> Option<u8> {
    if !eq(&s(&file["kind"]), "markdown") || !eq(&s(&file["availability"]), "present") {
        return None;
    }
    if file["work_id"].as_i64().is_none() {
        return None;
    }
    let role = s(&file["display_role"]);
    if eq(&role, "chinese_bilingual_markdown") {
        return Some(1);
    }
    if marks_bilingual(&(s(&file["relative_path"]) + "/" + &s(&file["name"]))) {
        return Some(2);
    }
    if eq(&role, "ocr_markdown") {
        return Some(3);
    }
    Some(4)
}

/// Whether a saved file name says the file is a Chinese or bilingual derivative.
fn marks_bilingual(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["zh-bilingual", "zh_bilingual", "bilingual", "chinese", "中英", "双语", "中文", ".zh.", "_zh.", "-zh."]
        .iter()
        .any(|mark| lower.contains(mark))
}

fn preference_name(rank: u8) -> &'static str {
    match rank {
        1 => "chinese_bilingual_markdown",
        2 => "bilingual_name",
        3 => "ocr_markdown",
        _ => "markdown",
    }
}

/// `原书文档：[原书 PDF](scholar:43) · [中英对照 Markdown](scholar:41)`, from the saved metadata
/// the file list already holds: the shown present files, the PDF first and the bilingual
/// Markdown before another one, as the digest's Literature Card writes them.
fn document_line(files: &[Value]) -> String {
    let mut shown = files
        .iter()
        .filter(|file| file["shown"].as_bool() == Some(true))
        .filter(|file| {
            let kind = s(&file["kind"]);
            eq(&kind, "pdf") || eq(&kind, "markdown")
        })
        .collect::<Vec<_>>();
    shown.sort_by(|a, b| {
        rank_of(a)
            .cmp(&rank_of(b))
            .then_with(|| a["file_id"].as_i64().unwrap_or_default().cmp(&b["file_id"].as_i64().unwrap_or_default()))
    });
    let links = shown
        .iter()
        .map(|file| {
            let label = if eq(&s(&file["kind"]), "pdf") {
                "原书 PDF"
            } else if matches!(preference(file), Some(1) | Some(2)) {
                "中英对照 Markdown"
            } else {
                "原文 Markdown"
            };
            format!("[{label}](scholar:{})", file["file_id"])
        })
        .collect::<Vec<_>>();
    if links.is_empty() {
        return String::new();
    }
    format!("原书文档：{}", links.join(" · "))
}

/// The order of the Document links: the PDF, then the bilingual Markdown, then another one.
fn rank_of(file: &Value) -> (u8, u8) {
    if eq(&s(&file["kind"]), "pdf") {
        (0, 0)
    } else if matches!(preference(file), Some(1) | Some(2)) {
        (1, 0)
    } else {
        (1, 1)
    }
}

/// The Work's file the digest reads: the explicit `-FileId`, or the only file of the best
/// preference; several of the best preference are a selection.
fn choose_file(work: &Work, files: &[Value], file_id: i64) -> Result<Found<(Value, u8)>> {
    if file_id != 0 {
        let Some(file) = files.iter().find(|file| file["file_id"].as_i64() == Some(file_id)) else {
            bail!(
                "file {file_id} is not one of Work {}'s linked present files ({}); pass the id of the Work's Markdown",
                work.id,
                file_summary(files)
            );
        };
        let Some(rank) = preference(file) else {
            bail!(
                "file {file_id} (`{}`, {}, {}) is not a present Markdown document; the digest reads the Work's Markdown",
                s(&file["relative_path"]),
                s(&file["kind"]),
                s(&file["availability"])
            );
        };
        return Ok(Found::One((file.clone(), rank)));
    }
    let suitable = files
        .iter()
        .filter_map(|file| preference(file).map(|rank| (rank, file)))
        .collect::<Vec<_>>();
    if suitable.is_empty() {
        bail!(
            "Work {} (`{}`) has no present Markdown document (its files are {}); pass the book's Markdown with -Source",
            work.id,
            work.title,
            file_summary(files)
        );
    }
    let best = suitable.iter().map(|(rank, _)| *rank).min().unwrap_or(4);
    let at_best = suitable
        .iter()
        .filter(|(rank, _)| *rank == best)
        .map(|(_, file)| (*file).clone())
        .collect::<Vec<Value>>();
    if let [only] = at_best.as_slice() {
        return Ok(Found::One((only.clone(), best)));
    }
    Ok(Found::Choose {
        message: format!(
            "Work {} (`{}`) has {} present Markdown documents of the same preference (`{}`); pass one id with -FileId",
            work.id,
            work.title,
            at_best.len(),
            preference_name(best)
        ),
        candidates: suitable
            .iter()
            .map(|(rank, file)| {
                let mut record = file_record(file);
                record["selected"] = json!(*rank == best);
                record
            })
            .collect(),
    })
}

fn file_summary(files: &[Value]) -> String {
    if files.is_empty() {
        return "none".to_string();
    }
    files
        .iter()
        .map(|file| {
            format!(
                "{} `{}` ({}, {}, {})",
                file["file_id"],
                s(&file["relative_path"]),
                s(&file["kind"]),
                s(&file["availability"]),
                s(&file["display_role"])
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Fetch the chosen Markdown through the bridge into ZET's document cache and verify it: its
/// path, its SHA-256 and its byte count.
fn fetch_document(zt: &mut Zt, work: &Work, file: &Value) -> Result<(String, String, u64)> {
    let file_id = file["file_id"].as_i64().context("the chosen file has no id")?;
    let copy = calls::read(zt, "scholar_archive_file", json!({"file_id":file_id,"max_text_bytes":0}))
        .map_err(|error| {
            anyhow::anyhow!(
                "the Scholar Archive cannot deliver file {file_id} of Work {} (`{}`): {error:#}; pass the book's Markdown with -Source",
                work.id,
                work.title
            )
        })?;
    let path = s(&copy["path"]);
    if path.trim().is_empty() {
        bail!("the Scholar Archive delivered no path for file {file_id} of Work {}", work.id);
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("the Scholar Archive's copy of file {file_id} is unreadable at {path}"))?;
    let sha256 = sha256_hex(&bytes);
    let declared = s(&copy["sha256"]);
    if !declared.is_empty() && !eq(&declared, &sha256) {
        bail!("document {file_id} in ZET's cache does not hash to the delivered SHA-256; fetch it again");
    }
    Ok((path, sha256, bytes.len() as u64))
}

/// The archive's own Citation-key rule: `^[A-Za-z][A-Za-z0-9_-]{0,127}$`.
fn valid_citation_key(key: &str) -> bool {
    (1..=128).contains(&key.len())
        && key.bytes().next().is_some_and(|first| first.is_ascii_alphabetic())
        && key.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// The entry key generated from saved metadata: the first author's surname and the year.
fn generated_key(work: &Work) -> String {
    let first = work
        .authors
        .split([',', ';'])
        .flat_map(|part| part.split(" and "))
        .map(str::trim)
        .find(|part| !part.is_empty())
        .unwrap_or_default();
    let surname = first
        .split_whitespace()
        .next_back()
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(100)
        .collect::<String>();
    let mut key = if surname.bytes().next().is_some_and(|first| first.is_ascii_alphabetic()) {
        surname
    } else {
        format!("Work{}", work.id)
    };
    if let Some(year) = work.year {
        key.push_str(&year.to_string());
    }
    if !valid_citation_key(&key) {
        key = format!("Work{}", work.id);
    }
    key
}

/// The selected key: the caller's, or one generated from the saved metadata; both are checked
/// against the archive's existing Cards.
fn citation_key(zt: &mut Zt, work: &Work, x: &Explicit) -> Result<(String, &'static str)> {
    let (key, source) = if x.citation_key_given {
        (x.citation_key.trim().to_string(), "given")
    } else {
        (generated_key(work), "generated")
    };
    if !valid_citation_key(&key) {
        bail!("invalid Citation key `{key}`; expected ^[A-Za-z][A-Za-z0-9_-]{{0,127}}$");
    }
    let (_, failure) = calls::run(zt, &[json!({"tool":"get_card","arguments":{"at":key}})], &mut calls::Options::new())?;
    if failure.is_none() {
        bail!("Citation key `{key}` already names a Card; pass another -CitationKey");
    }
    Ok((key, source))
}

/// The BibTeX entry from the saved metadata, or the caller's entry after the conflict check.
fn bibtex_text(work: &Work, key: &str, x: &Explicit) -> Result<String> {
    let generated = render_bibtex(work, key);
    if !x.bibtex_given {
        return Ok(generated);
    }
    if squash(&x.bibtex) != squash(&generated) {
        bail!(
            "the given -BibTeX differs from the entry Work {} (`{}`) generates; drop -BibTeX to use the resolved entry",
            work.id,
            work.title
        );
    }
    Ok(x.bibtex.clone())
}

/// Whether two BibTeX texts say the same thing: whitespace never matters.
fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// `@book{key, author, title, year, publisher, doi}` in a fixed field order. Braces inside a
/// value are dropped so the entry keeps one balanced level, and the title stays a literal.
fn render_bibtex(work: &Work, key: &str) -> String {
    let mut values: Vec<(&str, String)> = Vec::new();
    for name in &work.fields {
        let value = match name.as_str() {
            "author" => work.authors.clone(),
            "title" => work.title.clone(),
            "year" => work.year.map(|year| year.to_string()).unwrap_or_default(),
            "publisher" => work.venue.clone(),
            "doi" => work.doi.clone(),
            _ => continue,
        };
        values.push((name.as_str(), value));
    }
    let mut lines = vec![format!("@book{{{key},")];
    for (index, (name, value)) in values.iter().enumerate() {
        lines.push(format!(
            "  {name} = {{{}}}{}",
            value.replace(['{', '}'], ""),
            if index + 1 < values.len() { "," } else { "" }
        ));
    }
    lines.push("}".to_string());
    lines.join("\n")
}

/// The title the run records: the caller's (after the conflict check) or the Work's.
fn book_title(work: &Work, x: &Explicit) -> Result<String> {
    if !x.book_title_given || x.book_title.trim().is_empty() {
        return Ok(work.title.clone());
    }
    if normalize_title(&x.book_title) != normalize_title(&work.title) {
        bail!(
            "the given -BookTitle `{}` differs from Work {}'s title `{}`; drop -BookTitle to use the resolved title",
            x.book_title,
            work.id,
            work.title
        );
    }
    Ok(x.book_title.clone())
}

/// A given `-Source` must be the resolved document: the same file, or the same bytes.
fn check_source(work: &Work, resolved: &str, sha256: &str, x: &Explicit) -> Result<()> {
    let given = Path::new(&ps::full_name(&x.source)).to_path_buf();
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(&given), std::fs::canonicalize(resolved))
        && a == b
    {
        return Ok(());
    }
    let bytes = std::fs::read(&x.source)
        .with_context(|| format!("Cannot find path '{}' because it does not exist.", x.source))?;
    if sha256_hex(&bytes) != sha256 {
        bail!(
            "the given -Source `{}` is not the Markdown of Work {} ({resolved}); drop -Source to use the resolved document",
            x.source,
            work.id
        );
    }
    Ok(())
}

/// The resolved source text as the digest reads it: UTF-8 (lossy), no BOM, `\n` line ends.
fn read_source(path: &str) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("Cannot find path '{path}' because it does not exist."))?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(text.strip_prefix('\u{FEFF}').unwrap_or(&text).replace("\r\n", "\n"))
}

/// `#` and `##` headings of the source in reading order, with the characters of each section.
/// A heading line inside a fenced code block is text, as in the digest's own split.
fn scan_headings(raw: &str) -> Vec<Heading> {
    let lines = raw.split('\n').collect::<Vec<_>>();
    let mut positions: Vec<(usize, usize, String)> = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    for (index, line) in lines.iter().enumerate() {
        let trim = line.trim_start_matches(' ');
        let indentation = line.len() - trim.len();
        let marker = trim.as_bytes().first().copied().filter(|b| matches!(b, b'`' | b'~'));
        let run = marker.map(|m| trim.bytes().take_while(|b| *b == m).count()).unwrap_or(0);
        let was_fenced = fence.is_some();
        if indentation <= 3 && run >= 3 {
            if let Some((kind, size)) = fence {
                if marker == Some(kind) && run >= size && trim[run..].trim().is_empty() {
                    fence = None;
                }
            } else {
                fence = Some((marker.unwrap(), run));
            }
        }
        if was_fenced || fence.is_some() {
            continue;
        }
        if let Some(caps) = HEADING.captures(line) {
            positions.push((index, caps[1].len(), caps[2].trim().to_string()));
        }
    }
    let mut headings: Vec<Heading> = Vec::new();
    for (index, level, title) in &positions {
        let span_end = positions
            .iter()
            .find(|(at, other, _)| at > index && *other <= *level)
            .map(|(at, _, _)| *at)
            .unwrap_or(lines.len());
        let chars = lines[*index..span_end].iter().map(|line| len16(line)).sum::<usize>() + span_end - *index - 1;
        headings.push(Heading { title: title.clone(), level: *level, line: index + 1, chars });
    }
    headings
}

/// Whether a heading opens front matter.
fn is_front(title: &str) -> bool {
    heading_matches(title, FRONT)
}

/// Whether a heading opens back matter.
fn is_back(title: &str) -> bool {
    heading_matches(title, BACK)
}

/// Whether a heading's section is left out.
fn is_skippable(title: &str) -> bool {
    heading_matches(title, SKIP)
}

/// A heading matches a list exactly, or opens it with a qualifier: `Appendix A`,
/// `References and Notes`, `参考文献与索引`. A qualifier is a numbering or a connector, so
/// `Reference prices in a market` is no back matter. A Chinese heading is also matched with
/// its spaces removed (`前 言` is `前言`), as Chinese headings are often typeset spaced.
fn heading_matches(title: &str, list: &[&str]) -> bool {
    let title = normalize_title(title);
    let tight = title.split_whitespace().collect::<String>();
    list.iter().any(|entry| {
        let entry = normalize_title(entry);
        if title == entry {
            return true;
        }
        if entry.bytes().any(|byte| !byte.is_ascii()) {
            return tight
                .strip_prefix(&entry.split_whitespace().collect::<String>())
                .is_some_and(qualifier);
        }
        title.strip_prefix(&entry).is_some_and(qualifier)
    })
}

fn qualifier(rest: &str) -> bool {
    let rest = rest.trim();
    if rest.is_empty() {
        return true;
    }
    if ["and ", "与", "及", "、", "，", ",", ":", "：", "(", "（", "-", "—"].iter().any(|mark| rest.starts_with(mark)) {
        return true;
    }
    if rest
        .chars()
        .all(|c| c.is_ascii_digit() || "ivxlcdm".contains(c.to_ascii_lowercase()) || c.is_whitespace())
    {
        return true;
    }
    if rest.chars().count() == 1 && rest.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return true;
    }
    let mut chars = rest.chars();
    matches!(chars.next(), Some(c) if "一二三四五六七八九十".contains(c))
        && chars.next().is_none()
}

fn index_of(headings: &[Heading], title: &str) -> Option<usize> {
    headings.iter().position(|heading| eq(&heading.title, title))
}

/// The start/end/skip headings: the caller's when given, the proposal otherwise.
fn resolve_range(work: &Work, path: &str, headings: &[Heading], x: &Explicit) -> Result<Range> {
    if headings.is_empty() {
        bail!("Work {}'s document at {path} has no `#` or `##` heading to split; pass the book's Markdown with -Source", work.id);
    }
    let proposal = propose(headings);
    let (start, start_explicit) = match x.start.as_deref().filter(|value| !value.trim().is_empty()) {
        Some(title) => match index_of(headings, title) {
            Some(index) => (index, true),
            None => bail!("no heading \"{title}\" found in {path}"),
        },
        None => (proposal.start, false),
    };
    let (end, end_explicit) = match x.end.as_deref().filter(|value| !value.trim().is_empty()) {
        Some(title) => match index_of(headings, title) {
            Some(index) if index > start => (Some(index), true),
            Some(_) => bail!("the end heading \"{title}\" does not follow the start heading in {path}"),
            None => bail!("no heading \"{title}\" found in {path}"),
        },
        None => (proposal.end, false),
    };
    if end.is_some_and(|index| index <= start) {
        bail!("the end heading does not follow the start heading in {path}; pass -StartHeading and -EndHeading");
    }
    let (skip, skip_explicit) = match x.skip.as_deref().filter(|value| !value.trim().is_empty()) {
        Some(list) => {
            let mut indices = Vec::new();
            for title in split_skip(list) {
                let index = index_of(headings, &title).with_context(|| format!("no heading \"{title}\" found in {path}"))?;
                indices.push(index);
            }
            (indices, true)
        }
        None => (proposal.skip, false),
    };
    let limit = end.unwrap_or(headings.len());
    let skip = skip
        .into_iter()
        .filter(|index| *index > start && *index < limit)
        .collect::<Vec<usize>>();
    let included = (start..limit).filter(|index| !skip.contains(index)).count();
    let skip_join = skip_titles(headings, &skip).join(",");
    Ok(Range {
        start,
        end,
        skip,
        start_heading: headings[start].title.clone(),
        end_heading: end.map(|index| headings[index].title.clone()).unwrap_or_default(),
        start_line: headings[start].line,
        end_line: end.map(|index| headings[index].line).unwrap_or(0),
        start_explicit,
        end_explicit,
        skip_explicit,
        skip_join,
        included,
    })
}

fn split_skip(list: &str) -> Vec<String> {
    list.split(',').map(str::trim).filter(|title| !title.is_empty()).map(str::to_string).collect()
}

fn skip_titles(headings: &[Heading], skip: &[usize]) -> Vec<String> {
    skip.iter().map(|index| headings[*index].title.clone()).collect()
}

/// The proposal: start after the front matter, end at the back matter, and skip the sections
/// a digest never digests (exercises, references, indexes, appendices). The digest reads the
/// Chinese text, where the bilingual filter drops an English-only heading whose next block is
/// its translation, so a heading with Chinese characters is preferred for the start and the
/// end; an English-only one is a last resort and `prepare` then reports it precisely.
fn propose(headings: &[Heading]) -> Proposal {
    let top = (0..headings.len()).filter(|index| headings[*index].level == 1).collect::<Vec<usize>>();
    let chinese = |index: &usize| crate::prepare::has_cjk(&headings[*index].title);
    let start = top
        .iter()
        .copied()
        .find(|index| chinese(index) && !is_front(&headings[*index].title))
        .or_else(|| top.iter().copied().find(|index| !is_front(&headings[*index].title)))
        .or_else(|| top.iter().copied().find(chinese))
        .or_else(|| top.first().copied())
        .unwrap_or(0);
    let end = (start + 1..headings.len())
        .find(|index| headings[*index].level == 1 && chinese(index) && is_back(&headings[*index].title))
        .or_else(|| (start + 1..headings.len()).find(|index| headings[*index].level == 1 && is_back(&headings[*index].title)));
    let limit = end.unwrap_or(headings.len());
    let skip = (start + 1..limit)
        .filter(|index| headings[*index].level <= 2 && is_skippable(&headings[*index].title))
        .collect();
    Proposal { start, end, skip }
}

/// Every heading the range leaves out, with the reason, in reading order.
fn excluded_sections(headings: &[Heading], range: &Range) -> Vec<Value> {
    let mut excluded = Vec::new();
    for (index, heading) in headings.iter().enumerate() {
        let disposition = if index < range.start {
            "before_start"
        } else if range.end.is_some_and(|end| index >= end) {
            "after_end"
        } else if range.skip.contains(&index) {
            "skipped"
        } else {
            continue;
        };
        excluded.push(json!({"heading":heading.title,"level":heading.level,"line":heading.line,
            "chars":heading.chars,"disposition":disposition}));
    }
    excluded
}

/// The text before the start heading, so nothing outside the kept range is unaccounted for.
fn preamble_of(raw: &str, range: &Range) -> Value {
    let lines = raw.split('\n').collect::<Vec<_>>();
    let end = range.start_line.saturating_sub(1).min(lines.len());
    let text = lines[..end].join("\n");
    json!({"lines":end,"chars":len16(&text),"kept":false})
}

/// The level-1 headings of the range with their subheadings and characters: the "top
/// headings" a caller sees before choosing a range.
fn top_headings(headings: &[Heading], range: &Range) -> Vec<Value> {
    let limit = range.end.unwrap_or(headings.len());
    (range.start..limit)
        .filter(|index| headings[*index].level == 1)
        .map(|index| {
            let subheadings = (index + 1..limit).take_while(|next| headings[*next].level > 1).count();
            json!({"heading":headings[index].title,"line":headings[index].line,
                "chars":headings[index].chars,"subheadings":subheadings,
                "skipped":range.skip.contains(&index)})
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work(authors: &str, title: &str, year: Option<i64>, venue: &str, doi: &str) -> Work {
        let mut work = Work {
            id: 20,
            title: title.to_string(),
            authors: authors.to_string(),
            year,
            venue: venue.to_string(),
            doi: doi.to_string(),
            fields: Vec::new(),
        };
        work.fields = bibtex_fields(&work);
        work
    }

    fn explicit() -> Explicit {
        Explicit {
            source: String::new(),
            source_given: false,
            file_id: 0,
            citation_key: String::new(),
            citation_key_given: false,
            bibtex: String::new(),
            bibtex_given: false,
            book_title: String::new(),
            book_title_given: false,
            start: None,
            end: None,
            skip: None,
        }
    }

    #[test]
    fn a_title_is_compared_without_whitespace_or_case() {
        assert_eq!(normalize_title("  Democracy   Does\nCause Growth "), "democracy does cause growth");
        assert_ne!(normalize_title("Democracy does cause growth"), normalize_title("Democracy and growth"));
    }

    #[test]
    fn a_work_id_is_a_positive_integer_only() {
        assert_eq!(work_id_of("20"), Some(20));
        assert_eq!(work_id_of(" 7 "), Some(7));
        assert_eq!(work_id_of("0"), None);
        assert_eq!(work_id_of("Democracy 20"), None);
        assert_eq!(work_id_of("2.0"), None);
    }

    #[test]
    fn the_generated_key_is_the_first_surname_and_the_year() {
        assert_eq!(generated_key(&work("D Acemoglu, S Naidu", "T", Some(2019), "", "")), "Acemoglu2019");
        assert_eq!(generated_key(&work("Daron Acemoglu and Suresh Naidu", "T", Some(2019), "", "")), "Acemoglu2019");
        assert_eq!(generated_key(&work("Persson, Tabellini", "T", None, "", "")), "Persson");
        assert_eq!(generated_key(&work("", "T", Some(2001), "", "")), "Work202001");
        assert_eq!(generated_key(&work("李 稻葵", "T", Some(2020), "", "")), "Work202020");
        assert!(valid_citation_key(&generated_key(&work("Acemoglu", "T", Some(2019), "", ""))));
    }

    #[test]
    fn bibtex_lists_the_saved_fields_in_a_fixed_order() {
        let text = render_bibtex(
            &work("D Acemoglu, S Naidu", "Democracy does cause growth", Some(2019), "NBER Working Papers", "10.1/x"),
            "Acemoglu2019",
        );
        assert_eq!(
            text,
            "@book{Acemoglu2019,\n  author = {D Acemoglu, S Naidu},\n  title = {Democracy does cause growth},\n  year = {2019},\n  publisher = {NBER Working Papers},\n  doi = {10.1/x}\n}"
        );
        assert_eq!(render_bibtex(&work("", "Only {Braces}", None, "", ""), "Work1"), "@book{Work1,\n  title = {Only Braces}\n}");
        assert_eq!(squash("@book{K,\n title={T}}"), squash("@book{K, title={T}}"));
    }

    #[test]
    fn a_file_is_ranked_by_its_saved_metadata() {
        let file = |kind: &str, availability: &str, role: &str, path: &str| {
            json!({"file_id":1,"kind":kind,"availability":availability,"display_role":role,
                "relative_path":path,"name":path,"work_id":20,"shown":true})
        };
        assert_eq!(preference(&file("markdown", "present", "chinese_bilingual_markdown", "a/b.md")), Some(1));
        assert_eq!(preference(&file("markdown", "present", "ocr_markdown", "a/b.zh-bilingual.md")), Some(2));
        assert_eq!(preference(&file("markdown", "present", "ocr_markdown", "a/b.md")), Some(3));
        assert_eq!(preference(&file("markdown", "present", "", "a/b.md")), Some(4));
        assert_eq!(preference(&file("pdf", "present", "original_pdf", "a/b.pdf")), None);
        assert_eq!(preference(&file("markdown", "missing", "ocr_markdown", "a/b.md")), None);
        let mut unlinked = file("markdown", "present", "ocr_markdown", "a/b.md");
        unlinked["work_id"] = Value::Null;
        assert_eq!(preference(&unlinked), None);
    }

    #[test]
    fn a_section_remembers_where_it_starts_and_how_long_it_is() {
        let raw = "# Preface\n\nfront matter.\n\n# 导论\n\n## Intro.1 Why firms\n\nFirms exist.\n\n# 第一章 契约\n\n## 1.1 不完全契约\n\n剩余控制权。\n\n# 参考文献\n\n文献。\n";
        let headings = scan_headings(raw);
        assert_eq!(
            headings.iter().map(|h| (h.level, h.title.as_str(), h.line)).collect::<Vec<_>>(),
            vec![
                (1, "Preface", 1),
                (1, "导论", 5),
                (2, "Intro.1 Why firms", 7),
                (1, "第一章 契约", 11),
                (2, "1.1 不完全契约", 13),
                (1, "参考文献", 17)
            ]
        );
        assert!(headings[3].chars > headings[4].chars);
        assert!(headings[1].chars > 0);
        let fenced = scan_headings("# 第一章\n\n```\n# not a heading\n```\n\n## 1.1 节\n\n正文。\n");
        assert_eq!(fenced.iter().map(|h| h.title.as_str()).collect::<Vec<_>>(), vec!["第一章", "1.1 节"]);
    }

    #[test]
    fn the_proposal_starts_after_the_front_matter_and_stops_at_the_back_matter() {
        let raw = "# Preface\n\nx\n\n# 导论\n\n## Intro.1\n\nx\n\n# 第一章 契约\n\n## 1.1 不完全契约\n\nx\n\n## 习题\n\nx\n\n# 第二章 市场\n\nx\n\n# 参考文献\n\nx\n";
        let headings = scan_headings(raw);
        let proposal = propose(&headings);
        assert_eq!(headings[proposal.start].title, "导论");
        assert_eq!(proposal.end.map(|end| headings[end].title.clone()), Some("参考文献".to_string()));
        assert_eq!(skip_titles(&headings, &proposal.skip), vec!["习题".to_string()]);
        let range = resolve_range(&work("", "T", None, "", ""), "book.md", &headings, &explicit()).unwrap();
        assert_eq!(
            excluded_sections(&headings, &range)
                .iter()
                .map(|entry| (s(&entry["heading"]), s(&entry["disposition"])))
                .collect::<Vec<_>>(),
            vec![
                ("Preface".to_string(), "before_start".to_string()),
                ("习题".to_string(), "skipped".to_string()),
                ("参考文献".to_string(), "after_end".to_string())
            ]
        );
        assert_eq!(range.start_heading, "导论");
        assert_eq!(range.end_heading, "参考文献");
        assert_eq!(range.skip_join, "习题");
        assert!(range.start_line > 0 && range.end_line > range.start_line);
        assert!(range.included >= 4);
        assert_eq!(top_headings(&headings, &range).len(), 3);
    }

    #[test]
    fn a_heading_matches_a_list_at_a_word_boundary() {
        assert!(is_front("Preface"));
        assert!(is_front("  前 言 "));
        assert!(!is_front("Preface to the second edition of the book about many things"));
        assert!(is_back("References and Notes"));
        assert!(is_back("参考文献"));
        assert!(is_back("参考文献与索引"));
        assert!(is_back("Appendix A"));
        assert!(!is_back("Reference prices in a market"));
        assert!(is_skippable("Exercises"));
        assert!(is_skippable("习题"));
        assert!(!is_skippable("Exercise of power"));
    }
}
