//! The chapter endnotes of a prepared book (implementation design 11.6, R310/R311).
//!
//! The digest reads the main range only (`prepare`'s start/end headings); a book's 注释 section
//! stands outside it and holds definitions a model needs (I30). This module finds that section,
//! maps its subsections to the digest's chapters by explicit headings and note labels, and writes
//! one file per chapter under `<staging>\notes\<chapter-id>.md`. Nothing here creates a digest
//! group or a Card: the notes are read-only reference text.
//!
//! Every fact keeps the original source: each chapter's record names the source lines it came
//! from, the note ids it holds, and the SHA-256 of the exact note text. A mapping that stays
//! ambiguous, or a note a chapter's text cites but the book never defines, becomes an explicit
//! fault, never a guess and never an invented formula. Notes larger than the declared prompt
//! budget are split into labelled files listed with their note ids, never truncated.

use crate::canon::sha256_hex;
use crate::prepare::resolve::{Heading, heading_matches, scan_headings};
use crate::ps::{eq, join_path, len16, s, NL};
use crate::util;
use anyhow::Result;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Headings that open a collected notes section: the names a Chinese book's back matter uses, and
/// the English ones.
const NOTES: &[&str] = &[
    "注释",
    "尾注",
    "章末注释",
    "注解",
    "原文注释",
    "notes",
    "endnotes",
    "chapter notes",
    "notes to the chapters",
    "notes to the chapter",
];

static CHAPTER_CN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^第\s*([0-9]+|[一二三四五六七八九十]+)\s*[章篇]").unwrap());
static CHAPTER_ARABIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d{1,3})(?:\s|[.、:：]|$)").unwrap());
/// A note label at the start of a block: `[1.1]`, `［1.1］`, `〔1〕`, `【2.3】`, or a circled digit.
static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(?:[\[［〔【]\s*(\d{1,3}(?:\.\d{1,3})?)\s*[\]］〕】]|([①-⑳]))").unwrap());
/// A note citation inside digest text: the same forms. A 4-digit year never matches.
static REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\[［〔【]\s*(\d{1,3}(?:\.\d{1,3})?)\s*[\]］〕】]|([①-⑳])").unwrap());

/// The declared prompt budget for one chapter's notes (`-NoteMaxChars`).
pub(crate) struct Options {
    pub title: String,
    pub max_chars: i64,
}

/// One written part of a chapter's notes: a whole number of note blocks.
pub(crate) struct Part {
    pub path: String,
    pub name: String,
    /// SHA-256 of this part's note text (the original text, header excluded).
    pub sha256: String,
    pub file_sha256: String,
    pub chars: i64,
    pub note_ids: Vec<String>,
    /// Original source line spans of the note text this part holds.
    pub spans: Vec<(usize, usize)>,
}

/// One chapter's notes.
pub(crate) struct ChapterNotes {
    pub chapter: String,
    pub chapter_id: String,
    pub number: Option<String>,
    /// The note text as the source writes it, all parts together.
    pub text: String,
    pub parts: Vec<Part>,
    /// Note ids the chapter's own text cites but the book never defines.
    pub missing: Vec<String>,
    pub present: bool,
}

/// The notes of one prepared book.
pub(crate) struct Notes {
    /// `manifest.notes`: the section record, every chapter's record and the faults.
    pub summary: Value,
    /// The preparation faults the notes raised (`notes_ambiguous`, `notes_missing`, ...).
    pub faults: Vec<Value>,
}

impl Notes {
    /// One chapter's record from the manifest, when the book has notes for it.
    pub fn record(&self, chapter: &str) -> Option<Value> {
        self.summary["chapters"]
            .as_array()?
            .iter()
            .find(|c| eq(&s(&c["chapter"]), chapter) && c["present"] == true)
            .cloned()
    }
}

/// One notes subsection and where it came from.
#[derive(Clone)]
struct Block {
    heading: String,
    line: usize,
    text: String,
    span: (usize, usize),
    note_ids: Vec<String>,
}

/// Extract the collected notes of the prepared book.
///
/// `chapters` are the digest's level-1 chapter titles in reading order; `start`/`end` are the
/// frozen range headings. `staging` receives `notes\`; nothing else is written.
pub(crate) fn extract(
    raw: &str,
    source: &str,
    staging: &str,
    chapters: &[String],
    start: &str,
    end: &str,
    options: &Options,
) -> Result<Notes> {
    let lines: Vec<&str> = raw.split('\n').collect();
    let headings = scan_headings(raw);
    let mut faults: Vec<Value> = Vec::new();

    // The notes section must stand outside the digest range, after the end heading.
    let start_at = headings.iter().position(|h| eq(&h.title, start));
    let end_at = headings.iter().position(|h| eq(&h.title, end) && !end.is_empty());
    let mut section = end_at.filter(|&at| is_notes(&headings[at].title));
    if section.is_none()
        && let Some(end) = end_at
    {
        section = ((end + 1)..headings.len()).find(|&at| is_notes(&headings[at].title));
    }
    if end_at.is_none()
        && let Some(start) = start_at
        && let Some(at) = ((start + 1)..headings.len()).find(|&i| is_notes(&headings[i].title))
    {
        // No end heading: the range runs to the end of the book, so these notes would be
        // digested as body text. Say so instead of extracting them twice.
        let (first, last) = section_span(&headings, &lines, at);
        faults.push(json!({
            "kind":"notes_inside_range",
            "heading":headings[at].title,
            "line":headings[at].line,
            "span":{"start_line":first,"end_line":last},
            "message":format!("the notes heading \"{}\" stands inside the digest range (no end heading was resolved); pass -EndHeading so the notes stay outside the digested text", headings[at].title),
        }));
        section = None;
    }

    let section_record = match section {
        None => json!({"present":false,"heading":Value::Null,"line":Value::Null,"chars":0,"outside_range":true}),
        Some(at) => {
            let heading = &headings[at];
            let (first, last) = section_span(&headings, &lines, at);
            let body_first = (heading.line + 1).min(last + 1);
            let body = if body_first <= last { lines[body_first - 1..last].join("\n") } else { String::new() };
            json!({
                "present":true,
                "heading":heading.title,
                "line":heading.line,
                "level":heading.level,
                "span":{"start_line":first,"end_line":last},
                "chars":len16(&body),
                "outside_range":true,
            })
        }
    };

    let mut records: Vec<ChapterNotes> = chapters
        .iter()
        .enumerate()
        .map(|(index, title)| ChapterNotes {
            chapter: title.clone(),
            chapter_id: format!("c{:02}", index + 1),
            number: chapter_number(title),
            text: String::new(),
            parts: Vec::new(),
            missing: Vec::new(),
            present: false,
        })
        .collect();
    let mut by_chapter: Vec<Vec<Block>> = vec![Vec::new(); chapters.len()];

    if let Some(at) = section {
        let blocks = notes_blocks(&lines, &headings, at);
        for block in blocks {
            if block.text.trim().is_empty() {
                continue;
            }
            match chapter_candidates(&block, chapters).as_slice() {
                [one] => by_chapter[*one].extend(note_units(&block)),
                candidates => faults.push(unmapped_fault(&block, candidates, chapters)),
            }
        }
        for (index, record) in records.iter_mut().enumerate() {
            let blocks = &by_chapter[index];
            if blocks.is_empty() {
                continue;
            }
            record.present = true;
            record.text = bodies(blocks);
            write_chapter(staging, source, options, record, blocks, &mut faults)?;
        }
    }

    // Notes the chapter's text cites but the book never defines stay visible.
    if section.is_some() {
        let digest = digest_text(raw, &headings, start, end);
        for record in records.iter_mut() {
            let chapter_text = digest.get(&record.chapter).cloned().unwrap_or_default();
            let defined: Vec<String> = record.parts.iter().flat_map(|p| p.note_ids.clone()).collect();
            let missing: Vec<String> = note_refs(&chapter_text).into_iter().filter(|id| !defined.contains(id)).collect();
            if !missing.is_empty() {
                faults.push(json!({
                    "kind":"notes_missing",
                    "chapter_id":record.chapter_id,
                    "chapter":record.chapter,
                    "note_ids":missing,
                    "message":format!(
                        "章节「{}」的正文引用了注释 {}，但原书的注释没有给出它们的定义；这是原书的缺失，不要替它补写内容",
                        record.chapter,
                        missing.join("、")
                    ),
                }));
                record.missing = missing;
            }
        }
    }

    let chapters_json: Vec<Value> = records.iter().map(|record| chapter_record(record, options)).collect();
    let summary = json!({
        "section": section_record,
        "chapters": chapters_json,
        "faults": faults,
        "split": records.iter().any(|r| r.parts.len() > 1),
    });
    Ok(Notes { summary, faults })
}

/// Whether a heading opens a collected notes section.
fn is_notes(title: &str) -> bool {
    heading_matches(title, NOTES)
}

/// The 1-based original line span of one heading's section.
fn section_span(headings: &[Heading], lines: &[&str], at: usize) -> (usize, usize) {
    let heading = &headings[at];
    let end = headings
        .iter()
        .skip(at + 1)
        .find(|other| other.level <= heading.level)
        .map(|other| other.line - 1)
        .unwrap_or(lines.len());
    (heading.line, end.max(heading.line))
}

/// The notes of one section as blocks: its subsections when it has any, its labelled paragraphs
/// when it does not.
fn notes_blocks(lines: &[&str], headings: &[Heading], at: usize) -> Vec<Block> {
    let section = &headings[at];
    let (first, last) = section_span(headings, lines, at);
    let children: Vec<&Heading> = headings
        .iter()
        .skip(at + 1)
        .take_while(|other| other.level > section.level && other.line <= last)
        .collect();
    if children.is_empty() {
        return paragraphs(&lines[first..last], first + 1);
    }
    children
        .into_iter()
        .map(|child| {
            let end = headings
                .iter()
                .find(|other| other.line > child.line && other.level <= child.level)
                .map(|other| other.line - 1)
                .unwrap_or(last)
                .min(last);
            let raw_body: Vec<&str> = if child.line + 1 <= end { lines[child.line..end].to_vec() } else { Vec::new() };
            // The span points at the note text itself, not at the blank lines around it.
            let lead = raw_body.iter().take_while(|line| line.trim().is_empty()).count();
            let tail = raw_body.iter().rev().take_while(|line| line.trim().is_empty()).count();
            let body = raw_body[lead..raw_body.len().saturating_sub(tail)].join("\n");
            let first = (child.line + 1 + lead).min(end.max(child.line + 1));
            let last = end.saturating_sub(tail).max(first);
            let text = body.trim().to_string();
            Block {
                heading: child.title.clone(),
                line: child.line,
                note_ids: note_ids(&text),
                text,
                span: (first, last),
            }
        })
        .collect()
}

/// Labelled paragraphs of a flat notes section.
fn paragraphs(lines: &[&str], offset: usize) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut start: Option<usize> = None;
    let mut buffer: Vec<&str> = Vec::new();
    let last_line = offset + lines.len().saturating_sub(1);
    for (index, line) in lines.iter().enumerate() {
        let number = offset + index;
        if line.trim().is_empty() || LABEL.is_match(line) {
            push_block(&mut blocks, start, &buffer, number.saturating_sub(1).max(offset.saturating_sub(1)));
            start = None;
            buffer.clear();
            if line.trim().is_empty() {
                continue;
            }
            start = Some(number);
        }
        if start.is_some() {
            buffer.push(line);
        }
    }
    push_block(&mut blocks, start, &buffer, last_line);
    blocks
}

fn push_block(blocks: &mut Vec<Block>, start: Option<usize>, buffer: &[&str], end: usize) {
    let Some(start) = start.filter(|_| !buffer.is_empty()) else { return };
    let text = buffer.join("\n").trim().to_string();
    if text.is_empty() {
        return;
    }
    let note_ids = note_ids(&text);
    blocks.push(Block { heading: String::new(), line: start, text, span: (start, end.max(start)), note_ids });
}

/// The individual notes of one subsection: a new note starts at its label, and a note's own
/// continuation lines (with their blanks) stay with it. Splitting here is what lets an oversized
/// subsection become labelled parts without ever cutting a note in half.
fn note_units(block: &Block) -> Vec<Block> {
    let lines: Vec<&str> = block.text.split('\n').collect();
    let mut units: Vec<Block> = Vec::new();
    let mut buffer: Vec<&str> = Vec::new();
    let mut from = 0usize;
    for (index, line) in lines.iter().enumerate() {
        if LABEL.is_match(line) && !buffer.is_empty() {
            flush_unit(&mut units, &mut buffer, block.span.0, from, &block.heading);
            from = index;
        }
        buffer.push(line);
    }
    flush_unit(&mut units, &mut buffer, block.span.0, from, &block.heading);
    if units.is_empty() { vec![block.clone()] } else { units }
}

/// One note: its label line and its own continuation lines, trimmed, with its source span.
fn flush_unit(units: &mut Vec<Block>, buffer: &mut Vec<&str>, base: usize, from: usize, heading: &str) {
    let mut lead = 0;
    while lead < buffer.len() && buffer[lead].trim().is_empty() {
        lead += 1;
    }
    let mut end = buffer.len();
    while end > lead && buffer[end - 1].trim().is_empty() {
        end -= 1;
    }
    if lead < end {
        let text = buffer[lead..end].join("\n").trim().to_string();
        let first = base + from + lead;
        let last = base + from + end - 1;
        units.push(Block { heading: heading.to_string(), line: first, note_ids: note_ids(&text), text, span: (first, last.max(first)) });
    }
    buffer.clear();
}

/// Every chapter a notes block names: by its heading title, by its chapter number, or by the
/// note id prefixes its labels carry. One chapter is a mapping, several are an ambiguity, and
/// none is an unmapped subsection.
fn chapter_candidates(block: &Block, chapters: &[String]) -> Vec<usize> {
    if !block.heading.is_empty() {
        let exact: Vec<usize> = (0..chapters.len()).filter(|&i| eq(&chapters[i], &block.heading)).collect();
        if !exact.is_empty() {
            return exact;
        }
        if let Some(number) = chapter_number(&block.heading) {
            let by_number: Vec<usize> = (0..chapters.len()).filter(|&i| chapter_number(&chapters[i]).as_deref() == Some(number.as_str())).collect();
            if !by_number.is_empty() {
                return by_number;
            }
        }
    }
    let mut found: Vec<usize> = Vec::new();
    for id in &block.note_ids {
        let head = id.split('.').next().unwrap_or_default().to_string();
        for (index, chapter) in chapters.iter().enumerate() {
            if chapter_number(chapter).as_deref() == Some(head.as_str()) && !found.contains(&index) {
                found.push(index);
            }
        }
    }
    found
}

fn unmapped_fault(block: &Block, candidates: &[usize], chapters: &[String]) -> Value {
    let kind = if candidates.len() > 1 { "notes_ambiguous" } else { "notes_unmapped" };
    let message = if candidates.len() > 1 {
        format!(
            "the notes subsection \"{}\" cites note ids of several chapters ({}); split it under explicit chapter headings",
            block.heading,
            candidates.iter().map(|&i| chapters[i].clone()).collect::<Vec<_>>().join("、")
        )
    } else {
        format!(
            "the notes subsection \"{}\" names no chapter of the digest; give it the chapter's heading or the chapter's note labels such as [1.1]",
            block.heading
        )
    };
    json!({
        "kind":kind,
        "heading":block.heading,
        "line":block.line,
        "span":{"start_line":block.span.0,"end_line":block.span.1},
        "observed":clip(&block.text,400),
        "candidates":candidates.iter().map(|&i| chapters[i].clone()).collect::<Vec<_>>(),
        "message":message,
    })
}

/// Write one chapter's notes, split at whole note blocks when they exceed the prompt budget.
fn write_chapter(
    staging: &str,
    source: &str,
    options: &Options,
    record: &mut ChapterNotes,
    blocks: &[Block],
    faults: &mut Vec<Value>,
) -> Result<()> {
    // Parts at block boundaries: a note is never cut in half, however large it is.
    let mut groups: Vec<Vec<&Block>> = Vec::new();
    let mut current: Vec<&Block> = Vec::new();
    let mut size = 0i64;
    for block in blocks {
        let chars = len16(&block.text) as i64;
        if !current.is_empty() && size + chars > options.max_chars {
            groups.push(std::mem::take(&mut current));
            size = 0;
        }
        current.push(block);
        size += chars;
    }
    if !current.is_empty() {
        groups.push(current);
    }
    let total = groups.len();
    for (index, part) in groups.iter().enumerate() {
        let text = bodies(&part.iter().map(|b| (*b).clone()).collect::<Vec<_>>());
        let ids = unique(part.iter().flat_map(|b| b.note_ids.clone()).collect());
        let part_spans: Vec<(usize, usize)> = part.iter().map(|b| b.span).collect();
        let name = if total == 1 { format!("{}.md", record.chapter_id) } else { format!("{}-{}.md", record.chapter_id, index + 1) };
        let path = join_path(&join_path(staging, "notes"), &name);
        let span_line = part_spans.iter().map(|(a, b)| format!("{a}–{b}")).collect::<Vec<_>>().join("、");
        let label = if ids.is_empty() { "（没有可识别的注释编号）".to_string() } else { ids.join("、") };
        let part_note = if total == 1 { String::new() } else { format!("- 分段：第 {}/{} 部分", index + 1, total) };
        let header = format!(
            "# 《{}》章节注释：{}（只读参考文本）{NL}{NL}- 原书：`{source}`{NL}- 原文行：{span_line}{NL}- 注释编号：{label}{NL}{part_note}{NL}- 注释原文 SHA-256：`{}`{NL}- 用途：本章注释的只读参考文本；不是正文，不写成卡片。{NL}{NL}## 注释原文{NL}{NL}",
            options.title,
            record.chapter,
            sha256_hex(text.as_bytes()),
        );
        util::write_atomic(&path, &format!("{header}{text}{NL}"))?;
        let bytes = std::fs::read(&path)?;
        record.parts.push(Part {
            path,
            name,
            sha256: sha256_hex(text.as_bytes()),
            file_sha256: sha256_hex(&bytes),
            chars: len16(&text) as i64,
            note_ids: ids,
            spans: part_spans,
        });
    }
    if total > 1 {
        // Splitting is a declared limitation, not a silent truncation: the user of the notes
        // lists and reads each part with its note ids, so no definition is lost.
        faults.push(json!({
            "kind":"notes_split",
            "chapter_id":record.chapter_id,
            "chapter":record.chapter,
            "parts":total,
            "note_ids":record.parts.iter().flat_map(|p| p.note_ids.clone()).collect::<Vec<_>>(),
            "message":format!("章节「{}」的注释超过提示词预算 {} 字，分成 {} 个带编号的文件；使用方必须列出并读取这些文件，不要猜测内容", record.chapter, options.max_chars, total),
        }));
    }
    Ok(())
}

/// The manifest's record of one chapter's notes.
fn chapter_record(record: &ChapterNotes, options: &Options) -> Value {
    let files: Vec<Value> = record
        .parts
        .iter()
        .map(|part| {
            json!({
                "path":part.path,
                "name":part.name,
                "sha256":part.sha256,
                "file_sha256":part.file_sha256,
                "chars":part.chars,
                "note_ids":part.note_ids,
                "spans":part.spans.iter().map(|(a,b)| json!({"start_line":a,"end_line":b})).collect::<Vec<_>>(),
            })
        })
        .collect();
    let spans: Vec<Value> = record
        .parts
        .iter()
        .flat_map(|p| p.spans.iter())
        .map(|(a, b)| json!({"start_line":a,"end_line":b}))
        .collect();
    json!({
        "chapter_id":record.chapter_id,
        "chapter":record.chapter,
        "number":record.number,
        "present":record.present,
        "files":files,
        "note_ids":record.parts.iter().flat_map(|p| p.note_ids.clone()).collect::<Vec<_>>(),
        "spans":spans,
        "sha256":if record.present {sha256_hex(record.text.as_bytes())} else {String::new()},
        "chars":if record.present {len16(&record.text) as i64} else {0},
        "text":if record.present {record.text.clone()} else {String::new()},
        "split":record.parts.len() > 1,
        "missing":record.missing,
        "budget":options.max_chars,
    })
}

/// The note text of several blocks, as the source writes them.
fn bodies(blocks: &[Block]) -> String {
    blocks.iter().map(|b| b.text.clone()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(&format!("{NL}{NL}"))
}

/// The note ids of one notes block, in reading order.
fn note_ids(text: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let Some(c) = LABEL.captures(line) else { continue };
        let Some(id) = label_of(&c) else { continue };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

fn label_of(c: &regex::Captures<'_>) -> Option<String> {
    match (c.get(1), c.get(2)) {
        (Some(number), _) => Some(number.as_str().to_string()),
        (_, Some(circled)) => Some(circled.as_str().to_string()),
        _ => None,
    }
}

/// The note ids one chapter's digest text cites, in reading order.
fn note_refs(text: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for c in REF.captures_iter(text) {
        // A Markdown link target is not a note citation.
        let end = c.get(0).map(|m| m.end()).unwrap_or(0);
        if text[end..].starts_with('(') {
            continue;
        }
        let Some(id) = label_of(&c) else { continue };
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// The chapter number a heading states: `第3章`, `第三章`, `3 契约`.
fn chapter_number(title: &str) -> Option<String> {
    if let Some(c) = CHAPTER_CN.captures(title) {
        let value = c[1].to_string();
        if let Ok(n) = value.parse::<i64>() {
            return Some(n.to_string());
        }
        if let Some(n) = chinese_number(&value) {
            return Some(n.to_string());
        }
    }
    CHAPTER_ARABIC.captures(title).map(|c| c[1].to_string())
}

/// `一` to `九十九` as a number, in the forms a chapter heading uses.
fn chinese_number(text: &str) -> Option<i64> {
    let digit = |ch: char| "零一二三四五六七八九".chars().position(|c| c == ch).map(|n| n as i64);
    if text.is_empty() {
        return None;
    }
    if let Some((tens, ones)) = text.split_once('十') {
        let tens = if tens.is_empty() { 1 } else { digit(tens.chars().next()?)? };
        let ones = if ones.is_empty() { 0 } else { digit(ones.chars().next()?)? };
        if ones > 9 {
            return None;
        }
        return Some(tens * 10 + ones);
    }
    let mut value = 0;
    let mut count = 0;
    for ch in text.chars() {
        value = value * 10 + digit(ch)?;
        count += 1;
    }
    if count > 1 { None } else { Some(value) }
}

/// The digest text per chapter, from the raw source between the range headings.
fn digest_text(raw: &str, headings: &[Heading], start: &str, end: &str) -> HashMap<String, String> {
    let lines: Vec<&str> = raw.split('\n').collect();
    let first = headings.iter().position(|h| eq(&h.title, start)).map(|i| headings[i].line).unwrap_or(1);
    let last = headings
        .iter()
        .position(|h| eq(&h.title, end) && !end.is_empty())
        .map(|i| headings[i].line)
        .unwrap_or(lines.len() + 1)
        .max(first);
    let mut out: HashMap<String, String> = HashMap::new();
    let mut chapter = String::new();
    let mut text = String::new();
    let mut fence = false;
    let stop = last.saturating_sub(1).min(lines.len());
    for line in &lines[first.saturating_sub(1)..stop] {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        if !fence
            && let Some(c) = crate::prepare::HEADING.captures(line)
            && c[1].len() == 1
        {
            if !chapter.is_empty() {
                out.entry(chapter.clone()).or_default().push_str(&text);
            }
            text.clear();
            chapter = c[2].trim().to_string();
        }
        text.push_str(line);
        text.push('\n');
    }
    if !chapter.is_empty() {
        out.entry(chapter).or_default().push_str(&text);
    }
    out
}

fn unique(mut list: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in list.drain(..) {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if len16(text) <= max {
        return text.to_string();
    }
    let mut cut = String::new();
    for ch in text.chars() {
        if len16(&format!("{cut}{ch}")) > max {
            break;
        }
        cut.push(ch);
    }
    format!("{cut}……")
}

/// Which note ids a chapter's record holds, in reading order.
fn record_ids(record: &Value) -> Vec<String> {
    record["note_ids"].as_array().map(|ids| ids.iter().map(s).collect()).unwrap_or_default()
}

/// The read-only reference block a group file and a model context pack carry for one chapter,
/// from the manifest's record. Inline when the notes fit the declared prompt budget; otherwise
/// the labelled part files, each with its note ids, so nothing is truncated or guessed. `None`
/// when the chapter has no notes.
pub(crate) fn reference_block(record: &Value, source: &str) -> Option<String> {
    if record["present"] != true {
        return None;
    }
    let parts: Vec<Value> = record["files"].as_array().cloned().unwrap_or_default();
    if parts.is_empty() {
        return None;
    }
    let mut block = String::new();
    block.push_str(&format!("{NL}{NL}## 本章注释（只读参考文本：读它，但不要为注释写卡片）{NL}{NL}"));
    let spans = record["spans"]
        .as_array()
        .map(|list| list.iter().map(|span| format!("{}–{}", s(&span["start_line"]), s(&span["end_line"]))).collect::<Vec<_>>().join("、"))
        .unwrap_or_default();
    block.push_str(&format!(
        "- 出处：`{source}` 第 {spans} 行；本章注释原文的 SHA-256：`{}`。{NL}",
        s(&record["sha256"])
    ));
    let ids = record_ids(record);
    if !ids.is_empty() {
        block.push_str(&format!("- 注释编号：{}。{NL}", ids.join("、")));
    }
    let missing: Vec<String> = record["missing"].as_array().map(|list| list.iter().map(s).collect()).unwrap_or_default();
    if !missing.is_empty() {
        block.push_str(&format!(
            "- 缺失的注释：本章正文引用了注释 {}，但原书的注释没有给出它们的定义。这是原书的缺失，不要替它编造内容。{NL}",
            missing.join("、")
        ));
    }
    if parts.len() == 1 {
        block.push_str(&format!("- 注释文件：`{}`。{NL}", s(&parts[0]["path"])));
        block.push_str(&format!("{NL}{}{NL}", s(&record["text"]).trim()));
    } else {
        block.push_str(&format!(
            "- 本章注释超过提示词预算，分成 {} 个带编号的文件；逐个读这些文件，不要猜测或截断内容：{NL}",
            parts.len()
        ));
        for part in &parts {
            let part_ids: Vec<String> = part["note_ids"].as_array().map(|list| list.iter().map(s).collect()).unwrap_or_default();
            let label = if part_ids.is_empty() { "（没有可识别的编号）".to_string() } else { part_ids.join("、") };
            let spans = part["spans"]
                .as_array()
                .map(|list| list.iter().map(|span| format!("{}–{}", s(&span["start_line"]), s(&span["end_line"]))).collect::<Vec<_>>().join("、"))
                .unwrap_or_default();
            block.push_str(&format!("  - `{}`：注释 {label}，原文第 {spans} 行{NL}", s(&part["path"])));
        }
    }
    Some(block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_numbers_read_chinese_and_arabic_headings() {
        assert_eq!(chapter_number("第一章 契约").as_deref(), Some("1"));
        assert_eq!(chapter_number("第十二章 结论").as_deref(), Some("12"));
        assert_eq!(chapter_number("第二十一章 附录").as_deref(), Some("21"));
        assert_eq!(chapter_number("第十章 结论").as_deref(), Some("10"));
        assert_eq!(chapter_number("第3章 契约").as_deref(), Some("3"));
        assert_eq!(chapter_number("3 契约").as_deref(), Some("3"));
        assert_eq!(chapter_number("导论"), None);
        assert_eq!(chapter_number("Intro.1 为什么有企业"), None);
    }

    #[test]
    fn note_labels_and_citations_are_read_without_years_or_links() {
        assert_eq!(note_ids("[1.1] 契约不完全：见 Hart 1995 第二章。\n\n[1.3] 敲竹杠。"), vec!["1.1", "1.3"]);
        assert_eq!(note_refs("参见注 [1.4]，另见 [1991]（不是注释）与 [链接](x)，还有①。"), vec!["1.4", "①"]);
        assert_eq!(note_ids("① 第一条\n② 第二条"), vec!["①", "②"]);
    }

    #[test]
    fn a_subsection_maps_by_its_heading_then_by_its_labels() {
        let chapters = vec!["第一章 契约".to_string(), "第二章 企业边界".to_string()];
        let block = |heading: &str, text: &str| Block {
            heading: heading.into(),
            line: 1,
            text: text.into(),
            span: (2, 2),
            note_ids: note_ids(text),
        };
        assert_eq!(chapter_candidates(&block("第二章 企业边界", "[2.1] 资产互补性。"), &chapters), vec![1]);
        assert_eq!(chapter_candidates(&block("集中注释", "[1.5] 一。\n\n[2.3] 二。"), &chapters), vec![0, 1]);
        assert_eq!(chapter_candidates(&block("一", "[1.2] 三。"), &chapters), vec![0]);
        assert!(chapter_candidates(&block("附录 A", "没有编号。"), &chapters).is_empty());
    }

    #[test]
    fn a_block_cut_from_the_notes_section_keeps_its_own_lines() {
        let raw = "# 注释\n\n## 第一章 契约\n\n[1.1] 契约不完全。\n\n[1.2] 剩余控制权。\n\n## 第二章 边界\n\n[2.1] 资产互补性。\n\n# 参考文献\n";
        let headings = scan_headings(raw);
        let lines: Vec<&str> = raw.split('\n').collect();
        let blocks = notes_blocks(&lines, &headings, 0);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].note_ids, vec!["1.1", "1.2"]);
        assert!(blocks[0].text.contains("[1.1] 契约不完全。"));
        assert_eq!(blocks[0].span, (5, 7));
        assert_eq!(blocks[1].note_ids, vec!["2.1"]);
    }
}
