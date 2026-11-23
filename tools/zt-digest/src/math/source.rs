//! P1a source identity (docs/math-digest-p1a-design.md §2, Milestone 1): the frozen leaf
//! inventory of a prepared mathematics book, its printed chapters as line ranges, the printed-label
//! scan, the check of one finder answer of schema 2 (packet-local fragments with typed keys), and
//! the two-pass join that makes the interim ownership projection: every source object with its
//! identity and primary destination, and every leaf with exactly one primary claim, or a fault.
//!
//! Milestone 1 is offline: nothing in the workflow calls this file yet. Milestone 2 wires it into
//! `index`, `index-check` and `index-apply`, and the consumers of design §3.
#![allow(dead_code)]

use super::index::{MIN_QUOTE, Section, UNIT_FORMS};
use crate::canon;
use crate::ps::{self, field, s};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

/// The kinds of a source object a finder may name (`role: object`).
pub const OBJECT_KINDS: &[&str] = &[
    "definition", "axiom", "theorem", "proposition", "lemma", "corollary", "example", "remark", "notation", "exercise", "figure", "editorial", "boilerplate",
];
/// The kinds that owe a proof status.
const RESULTS: &[&str] = &["theorem", "proposition", "lemma", "corollary"];
const PROOF_STATUS: &[&str] = &["inline", "deferred_in_slice", "source_omits", "external", "not_applicable"];
/// The kinds whose destination the finder chooses (`narrative`, or a unit they serve).
const CHOSEN: &[&str] = &["example", "remark", "notation"];

static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}(\s|$)").unwrap());
static FIGURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^!\[[^\]]*\]\(artifact:[^)]*\)\s*$").unwrap());
static LIST_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([-*+]|\d+[.)])\s").unwrap());
static CHAPTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^#{1,6}\s*(?:(?:Chapter|CHAPTER)\s+(\d+)|第\s*(\d+|[一二三四五六七八九十]+)\s*章)(?:\s|$|[.:：])").unwrap()
});
static EXERCISES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s*(?:EXERCISES|Exercises|习题|练习)\s*$").unwrap());
/// A main-text label at the start of a line: `1.19 Theorem.`, `**1.19** 定理`.
static MAIN_LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\**\s*(\d+\.\d+)\s*\**\s+\**\s*[^\s\d]").unwrap());
/// An exercise label at the start of a line in an exercise region: `1.`, `1.1.`.
static EXERCISE_LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\**\s*(\d+(?:\.\d+)?)\s*\**\s*\.(?:\s+\S|\s*$)").unwrap());
/// A labelled heading of an item: the label, then a kind word.
static ITEM_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*\**\s*\d+\.\d+\s*\**\s+\**\s*(?i:definitions?|theorems?|propositions?|lemmas?|corollar(?:y|ies)|examples?|remarks?|notations?|axioms?|定义|定理|命题|引理|推论|例|注|记号|公理)").unwrap()
});
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+(?:[.．]\d+)*").unwrap());

fn squeeze(text: &str) -> String {
    SPACE.replace_all(text, "").to_string()
}

fn non_space(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

// --- leaves ----------------------------------------------------------------------------------------

/// One leaf of a staged section: a run of lines, 1-based and inclusive. Nothing is dropped.
#[derive(Clone, Debug, PartialEq)]
pub struct Leaf {
    pub id: String,
    pub section: String,
    pub line: usize,
    pub end: usize,
    /// The physical kind: `paragraph`, `list`, `table`, `display`, `heading`, `figure`.
    pub kind: &'static str,
    /// `true` when the leaf must reach a primary claim (`obligation: write`).
    pub obligation: bool,
    /// Why an `obligation: none` leaf owes nothing.
    pub reason: &'static str,
    /// A display formula that coverage reads as part of the paragraph before it names that leaf.
    pub attached: Option<String>,
    pub text: String,
}

/// A heading the OCR made of a labelled item (`# 3.4 Theorem.`): its text after the `#`.
/// A section heading may carry a number too (`## 1.3 完备性`): only a kind word after the label
/// (`# 3.4 Theorem.`, `# 3.4 定理。`) makes the heading an item.
fn labelled_heading(line: &str) -> bool {
    HEADING.is_match(line) && ITEM_HEADING.is_match(line.trim_start_matches('#'))
}

fn make_leaf(section: &str, kind: &'static str, from: usize, to: usize, attached: Option<String>, lines: &[&str]) -> Leaf {
    // Only a plain heading owes nothing; a heading that opens with a printed label is the item.
    let plain = kind == "heading" && !labelled_heading(lines[from].trim());
    Leaf {
        id: format!("{section}:{}-{to}", from + 1),
        section: section.to_string(),
        line: from + 1,
        end: to,
        kind,
        obligation: !plain,
        reason: if plain { "heading" } else { "" },
        attached,
        text: lines[from..to].join("\n"),
    }
}

/// The leaves of one section: every non-blank block, in order. The block rules are the ones of
/// `coverage::units`, without its drops: a heading or a figure line is a leaf of its own, a display
/// formula (`$$…$$`) is a leaf that names the paragraph coverage attaches it to, a table is one leaf,
/// and a short block is a leaf. Only a plain heading owes no claim.
pub fn leaves(section: &str, lines: &[String]) -> Vec<Leaf> {
    let lines: Vec<&str> = lines.iter().map(|l| l.strip_suffix('\r').unwrap_or(l)).collect();
    let mut out: Vec<Leaf> = Vec::new();
    // The legacy paragraph unit a display may extend: its leaf index and its extended end.
    let mut last_unit: Option<(usize, usize)> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() {
            i += 1;
            continue;
        }
        if HEADING.is_match(line) || FIGURE.is_match(line) {
            let kind = if HEADING.is_match(line) { "heading" } else { "figure" };
            out.push(make_leaf(section, kind, i, i + 1, None, &lines));
            i += 1;
            continue;
        }
        if line.starts_with("$$") {
            let from = i;
            i += 1;
            if line == "$$" {
                while i < lines.len() {
                    let done = lines[i].trim().ends_with("$$");
                    i += 1;
                    if done {
                        break;
                    }
                }
            }
            let mut attached = None;
            if let Some((k, ext)) = last_unit
                && lines[ext..from].iter().all(|l| l.trim().is_empty())
            {
                attached = Some(out[k].id.clone());
                last_unit = Some((k, i));
            }
            out.push(make_leaf(section, "display", from, i, attached, &lines));
            continue;
        }
        let start = i;
        let kind = if line.starts_with('|') {
            while i < lines.len() && lines[i].trim().starts_with('|') {
                i += 1;
            }
            "table"
        } else {
            while i < lines.len() {
                let next = lines[i].trim();
                if next.is_empty() || HEADING.is_match(next) || FIGURE.is_match(next) || next.starts_with("$$") || next.starts_with('|') {
                    break;
                }
                i += 1;
            }
            if LIST_ITEM.is_match(line) { "list" } else { "paragraph" }
        };
        if non_space(&lines[start..i].join("\n")) >= 20 {
            last_unit = Some((out.len(), i));
        }
        out.push(make_leaf(section, kind, start, i, None, &lines));
    }
    out
}

/// The paragraph units today's coverage reads, as a projection of the leaves: the text leaves of at
/// least 20 non-whitespace characters, each extended by the displays attached to it.
pub fn legacy_units(leaves: &[Leaf]) -> Vec<crate::coverage::Unit> {
    let mut out: Vec<crate::coverage::Unit> = Vec::new();
    for l in leaves {
        match l.kind {
            "paragraph" | "list" | "table" if non_space(&l.text) >= 20 => {
                out.push(crate::coverage::Unit { section: l.section.clone(), start: l.line, end: l.end, text: l.text.clone() });
            }
            "display" if l.attached.is_some() => {
                if let Some(last) = out.last_mut() {
                    last.end = l.end;
                }
            }
            _ => {}
        }
    }
    out
}

// --- chapters --------------------------------------------------------------------------------------

/// A position in the book: section index, 1-based line, 0-based column in Unicode scalars.
type Pos = (usize, usize, usize);

/// A printed chapter: its id (`ch1`, or `front` for what comes before the first chapter), its
/// heading and its line ranges across the prepared sections.
#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {
    pub id: String,
    pub label: String,
    /// The heading right after the number heading (`# 第1章` then `# 实数系与复数系`), when there is one.
    pub title: String,
    pub ranges: Vec<(String, usize, usize)>,
}

/// The chapter's name as a reader sees it: its number heading and its title (`第1章 实数系与复数系`).
pub fn chapter_name(c: &Chapter) -> String {
    if c.title.is_empty() || c.label.contains(&c.title) { c.label.clone() } else { format!("{} {}", c.label, c.title) }
}

fn zh_number(text: &str) -> Option<usize> {
    if let Ok(n) = text.parse() {
        return Some(n);
    }
    let digit = |c: char| "一二三四五六七八九".chars().position(|d| d == c).map(|k| k + 1);
    let cs: Vec<char> = text.chars().collect();
    match cs.as_slice() {
        ['十'] => Some(10),
        ['十', b] => Some(10 + digit(*b)?),
        [a, '十'] => Some(10 * digit(*a)?),
        [a, '十', b] => Some(10 * digit(*a)? + digit(*b)?),
        [a] => digit(*a),
        _ => None,
    }
}

/// The chapter starts: (position, id, heading text without the `#`).
fn chapter_starts(sections: &[Section]) -> Vec<(Pos, String, String)> {
    let mut starts = Vec::new();
    for (si, sec) in sections.iter().enumerate() {
        for (k, l) in sec.lines.iter().enumerate() {
            let t = l.trim();
            if let Some(m) = CHAPTER.captures(t)
                && let Some(n) = m.get(1).or(m.get(2)).and_then(|x| zh_number(x.as_str()))
                // A second heading of the same chapter (a bilingual source) continues it.
                && starts.last().is_none_or(|x: &(Pos, String, String)| x.1 != format!("ch{n}"))
            {
                starts.push(((si, k + 1, 0), format!("ch{n}"), t.trim_start_matches('#').trim().to_string()));
            }
        }
    }
    starts
}

/// The printed chapters as line ranges. A chapter break may fall inside a staged section, and a
/// chapter may run across sections. Material before the first printed chapter is `front`.
pub fn chapters(sections: &[Section]) -> Vec<Chapter> {
    let mut starts = chapter_starts(sections);
    let before_first = |p: Pos| sections.iter().enumerate().any(|(si, sec)| sec.lines.iter().enumerate().any(|(k, l)| (si, k + 1, 0) < p && !l.trim().is_empty()));
    match starts.first() {
        None => starts.push(((0, 1, 0), "front".into(), String::new())),
        Some((p, _, _)) if before_first(*p) => starts.insert(0, ((0, 1, 0), "front".into(), String::new())),
        _ => {}
    }
    let Some(last) = sections.len().checked_sub(1) else { return Vec::new() };
    let mut out = Vec::new();
    for (k, (p, id, label)) in starts.iter().enumerate() {
        let stop: Pos = starts.get(k + 1).map(|x| x.0).unwrap_or((last, sections[last].lines.len() + 1, 0));
        let mut ranges = Vec::new();
        for (si, sec) in sections.iter().enumerate().take(stop.0.min(last) + 1).skip(p.0) {
            let total = sec.lines.len();
            let from = if si == p.0 { p.1 } else { 1 };
            let to = if si == stop.0 { stop.1 - 1 } else { total };
            if from <= to.min(total) {
                ranges.push((sec.file.clone(), from, to.min(total)));
            }
        }
        // The title: the next non-blank line of the same section, when it is a heading of its own.
        let title = sections[p.0].lines.iter().skip(p.1).map(|l| l.trim()).find(|l| !l.is_empty()).filter(|l| HEADING.is_match(l) && !CHAPTER.is_match(l)).map(|l| l.trim_start_matches('#').trim().to_string()).unwrap_or_default();
        out.push(Chapter { id: id.clone(), label: label.clone(), title, ranges });
    }
    out
}

/// The book's layout: chapter starts and the exercise headings.
struct Layout {
    starts: Vec<(Pos, String)>,
    exercises: Vec<Pos>,
}

impl Layout {
    fn new(sections: &[Section]) -> Layout {
        let mut starts = Vec::new();
        for c in chapters(sections) {
            if let Some((file, line, _)) = c.ranges.first() {
                let si = sections.iter().position(|x| &x.file == file).unwrap_or(0);
                starts.push(((si, *line, 0), c.id.clone()));
            }
        }
        let mut exercises = Vec::new();
        for (si, sec) in sections.iter().enumerate() {
            for (k, l) in sec.lines.iter().enumerate() {
                if EXERCISES.is_match(l.trim()) {
                    exercises.push((si, k + 1, 0));
                }
            }
        }
        Layout { starts, exercises }
    }

    fn chapter_index(&self, p: Pos) -> usize {
        self.starts.iter().rposition(|(s, _)| *s <= p).unwrap_or(0)
    }

    fn chapter(&self, p: Pos) -> String {
        self.starts.get(self.chapter_index(p)).map(|x| x.1.clone()).unwrap_or_else(|| "front".into())
    }

    fn has(&self, id: &str) -> bool {
        self.starts.iter().any(|x| x.1 == id)
    }

    /// Whether a position lies after an exercise heading of its own chapter.
    fn in_exercises(&self, p: Pos) -> bool {
        let from = self.starts.get(self.chapter_index(p)).map(|x| x.0).unwrap_or((0, 0, 0));
        self.exercises.iter().any(|e| *e >= from && *e <= p)
    }
}

// --- the label scan --------------------------------------------------------------------------------

/// A printed label the scan found: a report, never an object.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub chapter: String,
    pub exercise: bool,
    pub label: String,
    pub section: String,
    pub line: usize,
}

/// The printed labels per chapter: each line of a text leaf that opens with a main-text label
/// (`1.19 定理`; also a heading the OCR made of it), or, after the chapter's exercise heading, with an
/// exercise number (`19.`, alone on its line too). The
/// scan reports; it never creates an object or decides promotion.
pub fn candidates(sections: &[Section]) -> Vec<Candidate> {
    let layout = Layout::new(sections);
    let mut out = Vec::new();
    for (si, sec) in sections.iter().enumerate() {
        for l in leaves(&sec.file, &sec.lines) {
            if l.kind == "heading" {
                let text = sec.lines[l.line - 1].trim().trim_start_matches('#');
                if let Some(m) = MAIN_LABEL.captures(text).filter(|_| l.obligation && !layout.in_exercises((si, l.line, 0))) {
                    // Only a heading that is an item (see `labelled_heading`) reports its label.
                    out.push(Candidate { chapter: layout.chapter((si, l.line, 0)), exercise: false, label: m[1].to_string(), section: sec.file.clone(), line: l.line });
                }
                continue;
            }
            if !matches!(l.kind, "paragraph" | "list") {
                continue;
            }
            for n in l.line..=l.end {
                let p = (si, n, 0);
                let exercise = layout.in_exercises(p);
                let re = if exercise { &*EXERCISE_LABEL } else { &*MAIN_LABEL };
                if let Some(m) = re.captures(&sec.lines[n - 1]) {
                    out.push(Candidate { chapter: layout.chapter(p), exercise, label: m[1].to_string(), section: sec.file.clone(), line: n });
                }
            }
        }
    }
    out
}

// --- spans and fragments ---------------------------------------------------------------------------

/// The sections a fragment may cite: a packet's own sections inside the whole book.
struct Scope<'a> {
    all: &'a [Section],
    allowed: Vec<usize>,
}

impl<'a> Scope<'a> {
    fn of(all: &'a [Section], files: &[String]) -> Scope<'a> {
        Scope { all, allowed: (0..all.len()).filter(|&i| files.iter().any(|f| ps::eq(f, &all[i].file))).collect() }
    }

    fn find(&self, file: &str) -> Option<usize> {
        self.allowed.iter().copied().find(|&i| ps::eq(&self.all[i].file, file))
    }
}

#[derive(Clone, Debug)]
struct Span {
    section: String,
    start: Pos,
    end: Pos,
    col: Option<usize>,
    end_col: Option<usize>,
}

impl Span {
    fn json(&self) -> Value {
        let mut m = Map::new();
        m.insert("section".into(), json!(self.section));
        m.insert("line".into(), json!(self.start.1));
        m.insert("end".into(), json!(self.end.1));
        if let Some(c) = self.col {
            m.insert("col".into(), json!(c));
        }
        if let Some(c) = self.end_col {
            m.insert("end_col".into(), json!(c));
        }
        Value::Object(m)
    }

    fn overlaps(&self, o: &Span) -> bool {
        self.start < o.end && o.start < self.end
    }

    fn encloses(&self, o: &Span) -> bool {
        self.start <= o.start && o.end <= self.end
    }

    fn name(&self) -> String {
        format!("{}:{}-{}", self.section, self.start.1, self.end.1)
    }
}

/// Every span of `inner` lies inside one span of `outer`.
fn encloses(outer: &[Span], inner: &[Span]) -> bool {
    inner.iter().all(|i| outer.iter().any(|o| o.encloses(i)))
}

fn line_len(sections: &[Section], si: usize, line: usize) -> usize {
    sections[si].lines[line - 1].chars().count()
}

fn text_between(sections: &[Section], a: Pos, b: Pos) -> String {
    let mut out = Vec::new();
    for l in a.1..=b.1 {
        let chars: Vec<char> = sections[a.0].lines[l - 1].chars().collect();
        let from = if l == a.1 { a.2.min(chars.len()) } else { 0 };
        let to = if l == b.1 { b.2.min(chars.len()) } else { chars.len() };
        out.push(chars[from..to.max(from)].iter().collect::<String>());
    }
    out.join("\n")
}

/// One span of an answer, checked against the sections its packet may cite. The quote must lie
/// inside the span's own text (from `col` to `end_col`); a span whose text is shorter than a quote
/// is quoted whole.
fn parse_span(v: &Value, scope: &Scope) -> Result<Span, String> {
    let sections = scope.all;
    let file = field(v, "section");
    let si = scope.find(&file).ok_or_else(|| format!("section `{file}` is not a file of this job"))?;
    let total = sections[si].lines.len();
    let line = v["line"].as_u64().unwrap_or(0) as usize;
    let end = v["end"].as_u64().map(|x| x as usize).unwrap_or(line);
    if line < 1 || line > total {
        return Err(format!("line {line} is outside {file} (1-{total})"));
    }
    if end < line || end > total {
        return Err(format!("end {end} must be from line {line} to {total}"));
    }
    let col = v["col"].as_u64().map(|x| x as usize);
    let end_col = v["end_col"].as_u64().map(|x| x as usize);
    let (c0, c1) = (col.unwrap_or(0), end_col.unwrap_or_else(|| line_len(sections, si, end)));
    if c0 > line_len(sections, si, line) || c1 > line_len(sections, si, end) || (line == end && c0 >= c1) {
        return Err(format!("columns {c0}-{c1} do not mark text on lines {line}-{end} of {file}"));
    }
    let span = Span { section: sections[si].file.clone(), start: (si, line, c0), end: (si, end, c1), col, end_col };
    let body = squeeze(&text_between(sections, span.start, span.end));
    let quote = squeeze(&field(v, "quote"));
    if body.chars().count() < MIN_QUOTE {
        if quote != body {
            return Err(format!("quote must copy the whole short text of the span {}", span.name()));
        }
    } else if quote.chars().count() < MIN_QUOTE {
        return Err(format!("quote must copy at least {MIN_QUOTE} characters of the span {}", span.name()));
    } else if !body.contains(&quote) {
        return Err(format!("quote is not in the span {}", span.name()));
    }
    Ok(span)
}

fn parse_spans(v: &Value, scope: &Scope) -> Result<Vec<Span>, String> {
    let list = ps::list(v);
    if list.is_empty() {
        return Err("no spans".into());
    }
    list.into_iter().map(|x| parse_span(x, scope)).collect()
}

/// The printed label of a key: the number in it (`Theorem 1.19` → `1.19`), else the text as given.
/// A synthetic label (`after-1.21-1`) stays whole.
fn norm_label(raw: &str) -> String {
    let raw = raw.trim();
    if raw.starts_with("after-") || raw.starts_with("start-") {
        return raw.to_string();
    }
    match NUMBER.find(raw) {
        Some(m) => m.as_str().replace('．', "."),
        None => raw.replace('*', "").trim().trim_end_matches(['.', '．', '。']).trim().to_string(),
    }
}

/// Whether the `n` characters at `i` of `cs` stand as a whole label token: `1.1` is not a token
/// of `1.19`, nor `19` of `1.19`.
fn token_at(cs: &[char], i: usize, n: usize) -> bool {
    let numeric = |c: char| c.is_ascii_digit();
    let dot = |c: char| c == '.' || c == '．';
    let before = i.checked_sub(1).map(|k| cs[k]);
    let after = cs.get(i + n).copied();
    let bad_before = before.is_some_and(|c| numeric(c) || (dot(c) && i >= 2 && numeric(cs[i - 2])));
    let bad_after = after.is_some_and(|c| numeric(c) || (dot(c) && cs.get(i + n + 1).copied().is_some_and(numeric)));
    !bad_before && !bad_after
}

#[derive(Clone, Debug)]
struct Step {
    label: String,
    spans: Vec<Span>,
}

fn parse_steps(v: &Value, scope: &Scope) -> Result<Vec<Step>, String> {
    let mut out = Vec::new();
    for (k, st) in ps::list(v).into_iter().enumerate() {
        let spans = parse_spans(&st["spans"], scope).map_err(|e| format!("step {}: {e}", k + 1))?;
        out.push(Step { label: field(st, "label"), spans });
    }
    Ok(out)
}

fn steps_json(steps: &[Step]) -> Vec<Value> {
    steps.iter().map(|st| json!({"label": st.label, "spans": st.spans.iter().map(Span::json).collect::<Vec<_>>()})).collect()
}

/// One object fragment, parsed.
#[derive(Clone, Debug)]
struct ObjFrag {
    kind: String,
    label: String,
    sublabel: String,
    name: String,
    spans: Vec<Span>,
    status: String,
    reason: String,
    steps: Vec<Step>,
    dest: Value,
    relations: Vec<Value>,
}

/// A dependent fragment (a proof, hint or answer), parsed, with its key evidence judged against its
/// own packet.
#[derive(Clone, Debug)]
struct DepFrag {
    role: String,
    target: Value,
    evidence: Value,
    evidence_ok: bool,
    steps: Vec<Step>,
    spans: Vec<Span>,
    relations: Vec<Value>,
}

/// A fragment of one answer, parsed.
#[derive(Clone, Debug)]
enum Frag {
    Object(ObjFrag),
    Dependent(DepFrag),
    Exposition(Span),
    Dismiss { section: String, si: usize, line: usize, reason: String },
}

impl Frag {
    fn owned_spans(&self) -> Vec<&Span> {
        match self {
            Frag::Object(o) => o.spans.iter().chain(o.steps.iter().flat_map(|s| s.spans.iter())).collect(),
            Frag::Dependent(d) => d.spans.iter().chain(d.steps.iter().flat_map(|s| s.spans.iter())).collect(),
            Frag::Exposition(span) => vec![span],
            Frag::Dismiss { .. } => vec![],
        }
    }

    /// The source order of a fragment; a dismissal sorts by its locator.
    fn pos(&self) -> Pos {
        match self {
            Frag::Dismiss { si, line, .. } => (*si, *line, 0),
            f => f.owned_spans().first().map(|s| s.start).unwrap_or((usize::MAX, 0, 0)),
        }
    }

    /// The typed key a fragment may be contained by, when it has a printed label:
    /// (chapter, namespace, label, sublabel).
    fn key(&self, layout: &Layout) -> Option<Key> {
        match self {
            Frag::Object(o) if !o.label.is_empty() => Some((layout.chapter(self.pos()), o.kind.clone(), o.label.clone(), o.sublabel.clone())),
            Frag::Object(o) if !o.name.is_empty() => Some((layout.chapter(self.pos()), o.kind.clone(), format!("name:{}", o.name), o.sublabel.clone())),
            _ => None,
        }
    }

    /// The full typed keys this fragment's `contains` relations name.
    fn contains_keys(&self, layout: &Layout) -> Vec<Key> {
        let rels = match self {
            Frag::Object(o) => &o.relations,
            Frag::Dependent(d) => &d.relations,
            _ => return Vec::new(),
        };
        let own = layout.chapter(self.pos());
        rels.iter().filter(|r| field(r, "rel") == "contains").map(|r| typed_key(&r["target"], &own, layout)).collect()
    }
}

/// A typed key: (chapter, namespace, label, sublabel).
type Key = (String, String, String, String);

/// The full key a target names. A target with no chapter takes the chapter of an `N.M` label,
/// else the chapter it is read in.
fn typed_key(target: &Value, own_chapter: &str, layout: &Layout) -> Key {
    // An unnumbered object is named by its `name` (`name:开覆盖`): the finder cannot know the
    // ordinal the join gives it.
    let label = match (norm_label(&field(target, "label")), field(target, "name").trim().to_string()) {
        (l, n) if l.is_empty() && !n.is_empty() => format!("name:{n}"),
        (l, _) => l,
    };
    let chapter = match field(target, "chapter").trim() {
        "" => match label.split_once('.') {
            Some((n, _)) if !label.starts_with("after-") && layout.has(&format!("ch{n}")) => format!("ch{n}"),
            _ => own_chapter.to_string(),
        },
        c => c.to_string(),
    };
    (chapter, field(target, "namespace").to_lowercase(), label, field(target, "sublabel").trim().to_string())
}

fn frag_name(e: &Value) -> String {
    let label = field(e, "label").trim().to_string();
    if !label.is_empty() { label } else { field(e, "name").trim().to_string() }
}

/// The key evidence of a dependent fragment: a quote on a line of its own packet that names the
/// target's label as a whole token (a local citation, an enclosing heading or a forward pointer).
fn evidence_ok(evidence: &Value, label: &str, scope: &Scope) -> bool {
    let Some(si) = scope.find(&field(evidence, "section")) else { return false };
    let line = evidence["line"].as_u64().unwrap_or(0) as usize;
    if line < 1 || line > scope.all[si].lines.len() {
        return false;
    }
    let quote: Vec<char> = squeeze(&field(evidence, "quote")).chars().collect();
    // The squeezed line, each character with its place in the original line.
    let original: Vec<char> = scope.all[si].lines[line - 1].chars().collect();
    let (text, place): (Vec<char>, Vec<usize>) = original.iter().enumerate().filter(|(_, c)| !c.is_whitespace()).map(|(k, c)| (*c, k)).unzip();
    let ls: Vec<char> = label.chars().collect();
    if quote.is_empty() || ls.is_empty() || quote.len() > text.len() {
        return false;
    }
    // The label must be a whole token where the quote stands in the source line, not only inside
    // the quote: a clipped quote `定理 1.1` of `定理 1.19` does not name 1.1.
    (0..=text.len() - quote.len()).any(|i| {
        text[i..i + quote.len()] == quote[..]
            && (0..quote.len()).any(|j| {
                // The label stands whole and unbroken in the original line: `1.1 9` is no `1.19`.
                let p = i + j;
                j + ls.len() <= quote.len() && quote[j..j + ls.len()] == ls[..] && place[p + ls.len() - 1] - place[p] == ls.len() - 1 && token_at(&original, place[p], ls.len())
            })
    })
}

/// One fragment of an answer: its parse, or the fault that names what to change.
fn parse_fragment(e: &Value, scope: &Scope) -> Result<Frag, String> {
    match field(e, "role").as_str() {
        "object" => {
            let kind = field(e, "kind").to_lowercase();
            if !OBJECT_KINDS.contains(&kind.as_str()) {
                return Err(format!("kind `{kind}` is not one of {}", OBJECT_KINDS.join(", ")));
            }
            let spans = parse_spans(&e["spans"], scope)?;
            let status = field(&e["proof"], "status");
            let reason = field(&e["proof"], "reason");
            if !status.is_empty() && !PROOF_STATUS.contains(&status.as_str()) {
                return Err(format!("proof status `{status}` is not one of {}", PROOF_STATUS.join(", ")));
            }
            let steps = parse_steps(&e["proof"]["steps"], scope)?;
            Ok(Frag::Object(ObjFrag {
                label: norm_label(&field(e, "label")),
                sublabel: field(e, "sublabel").trim().to_string(),
                name: field(e, "name").trim().to_string(),
                kind,
                spans,
                status,
                reason,
                steps,
                dest: e["destination"].clone(),
                relations: ps::list(&e["relations"]).into_iter().cloned().collect(),
            }))
        }
        role @ ("proof" | "hint" | "answer") => {
            let label = norm_label(&field(&e["target"], "label"));
            if label.is_empty() {
                return Err(format!("a {role} needs a target label"));
            }
            let (steps, spans) = if role == "proof" {
                let steps = parse_steps(&e["steps"], scope)?;
                if steps.is_empty() {
                    return Err("a proof needs its steps".into());
                }
                (steps, Vec::new())
            } else {
                (Vec::new(), parse_spans(&e["spans"], scope)?)
            };
            Ok(Frag::Dependent(DepFrag {
                role: role.to_string(),
                target: e["target"].clone(),
                evidence: e["evidence"].clone(),
                evidence_ok: evidence_ok(&e["evidence"], &label, scope),
                steps,
                spans,
                relations: ps::list(&e["relations"]).into_iter().cloned().collect(),
            }))
        }
        "exposition" => {
            let spans = ps::list(&e["spans"]);
            if spans.len() != 1 {
                return Err("an exposition declares one contiguous span".into());
            }
            Ok(Frag::Exposition(parse_span(spans[0], scope)?))
        }
        "dismiss" => {
            if field(e, "reason").trim().is_empty() {
                return Err("a dismissal needs a `reason`".into());
            }
            let section = field(e, "section");
            let si = scope.find(&section).ok_or_else(|| format!("section `{section}` is not a file of this job"))?;
            Ok(Frag::Dismiss { section: scope.all[si].file.clone(), si, line: e["line"].as_u64().unwrap_or(0) as usize, reason: field(e, "reason") })
        }
        other => Err(format!("role `{other}` is not one of object, proof, hint, answer, exposition, dismiss")),
    }
}

/// Whether two overlapping fragments are a declared containment: the outer one names the inner
/// one's typed key in a `contains` relation, and every span of the inner one lies inside it.
fn contained(outer: &Frag, inner: &Frag, layout: &Layout) -> bool {
    let Some(key) = inner.key(layout) else { return false };
    let outer_spans: Vec<Span> = outer.owned_spans().into_iter().cloned().collect();
    let inner_spans: Vec<Span> = inner.owned_spans().into_iter().cloned().collect();
    outer.contains_keys(layout).contains(&key) && encloses(&outer_spans, &inner_spans)
}

/// The faults of one finder answer (schema 2): one line per fault, naming the fragment and what to
/// change. An empty list is a passing answer. `sections` is the whole prepared book, so chapters
/// come from the frozen book layout; `files` are the packet's own sections, the only ones a
/// fragment may cite.
pub fn check_answer(answer: &Value, sections: &[Section], files: &[&str]) -> Vec<String> {
    if answer["schema"].as_i64() != Some(2) {
        return vec!["the answer must declare \"schema\": 2".into()];
    }
    let Some(list) = answer.get("fragments").and_then(Value::as_array) else {
        return vec!["the answer has no `fragments` array".into()];
    };
    let files: Vec<String> = files.iter().map(|f| f.to_string()).collect();
    let scope = Scope::of(sections, &files);
    let layout = Layout::new(sections);
    let mut faults = Vec::new();
    let mut parsed: Vec<(usize, Frag)> = Vec::new();
    for (k, e) in list.iter().enumerate() {
        let n = k + 1;
        let name = frag_name(e);
        let tag = if name.is_empty() { format!("fragment {n}") } else { format!("fragment {n} ({name})") };
        match parse_fragment(e, &scope) {
            Err(f) => faults.push(format!("{tag}: {f}")),
            Ok(f) => {
                if let Frag::Object(o) = &f
                    && matches!(o.status.as_str(), "source_omits" | "external")
                    && o.reason.trim().is_empty()
                {
                    faults.push(format!("{tag}: proof status `{}` needs a `reason`", o.status));
                }
                if let Frag::Dependent(d) = &f
                    && !d.evidence_ok
                {
                    let label = norm_label(&field(&d.target, "label"));
                    faults.push(format!("{tag}: the evidence must quote a line of this packet that names `{label}`"));
                    continue;
                }
                parsed.push((n, f));
            }
        }
    }
    faults.extend(unclaimed_in_packet(&scope, &parsed));
    faults.extend(labels_in_packet(&scope, &layout, &parsed));
    for (i, (a, fa)) in parsed.iter().enumerate() {
        for (b, fb) in parsed.iter().skip(i + 1) {
            let overlap = fa.owned_spans().iter().any(|x| fb.owned_spans().iter().any(|y| x.overlaps(y)));
            if overlap && !contained(fa, fb, &layout) && !contained(fb, fa, &layout) {
                faults.push(format!("fragments {a} and {b} overlap; end the first before the second starts, or name the inner one in a `contains` relation of an outer span that encloses it"));
            }
        }
    }
    faults
}

/// The text of the packet that no fragment claims: one line per leaf, or per uncovered run inside a
/// leaf (`s03.md:4:17-4:30`), so the job sees what it left undone and claims it.
fn unclaimed_in_packet(scope: &Scope, parsed: &[(usize, Frag)]) -> Vec<String> {
    let spans: Vec<&Span> = parsed.iter().flat_map(|(_, f)| f.owned_spans()).collect();
    let covered = |p: Pos| spans.iter().any(|s| s.start <= p && p < s.end);
    let mut out = Vec::new();
    for &si in &scope.allowed {
        let sec = &scope.all[si];
        for l in leaves(&sec.file, &sec.lines) {
            if !l.obligation {
                continue;
            }
            let mut runs: Vec<(Pos, Pos)> = Vec::new();
            let mut open: Option<(Pos, Pos)> = None;
            let mut whole = true;
            for n in l.line..=l.end {
                for (c, ch) in sec.lines[n - 1].chars().enumerate() {
                    if ch.is_whitespace() {
                        continue;
                    }
                    let p = (si, n, c);
                    if covered(p) {
                        whole = false;
                        if let Some(r) = open.take() {
                            runs.push(r);
                        }
                    } else {
                        open = Some(match open {
                            Some((a, _)) => (a, (si, n, c + 1)),
                            None => (p, (si, n, c + 1)),
                        });
                    }
                }
            }
            runs.extend(open);
            if whole && !runs.is_empty() {
                out.push(format!("{} ({}) is claimed by no fragment: put it in an object, a proof step, a hint or answer, or an exposition", l.id, l.kind));
                continue;
            }
            for (a, b) in runs {
                out.push(format!("{}:{}:{}-{}:{} ({}) is claimed by no fragment: put it in an object, a proof step, a hint or answer, or an exposition", l.section, a.1, a.2, b.1, b.2, l.kind));
            }
        }
    }
    out
}

/// What a packet alone decides about the printed labels in its sections: each opens an object of
/// this answer with that label, or this answer dismisses its line; no exposition holds one; no two
/// objects of the answer share a key. The join checks the same across the packets.
fn labels_in_packet(scope: &Scope, layout: &Layout, parsed: &[(usize, Frag)]) -> Vec<String> {
    let mut out = Vec::new();
    let mine = |file: &str| scope.allowed.iter().any(|&i| ps::eq(&scope.all[i].file, file));
    let dismissed: Vec<(String, usize)> = parsed
        .iter()
        .filter_map(|(_, f)| match f {
            Frag::Dismiss { section, line, .. } => Some((section.clone(), *line)),
            _ => None,
        })
        .collect();
    let is_dismissed = |c: &Candidate| dismissed.iter().any(|(s, l)| ps::eq(s, &c.section) && *l == c.line);
    let cands: Vec<Candidate> = candidates(scope.all).into_iter().filter(|c| mine(&c.section) && !is_dismissed(c)).collect();
    for c in &cands {
        let opens = parsed.iter().any(|(_, f)| match f {
            Frag::Object(o) => o.label == c.label && (o.kind == "exercise") == c.exercise && o.spans.iter().any(|s| ps::eq(&s.section, &c.section) && s.start.1 == c.line),
            _ => false,
        });
        if !opens {
            out.push(format!("{}:{} prints {} and no object of this answer starts there with that label: claim it as an object that starts on this line, or dismiss the line with a reason", c.section, c.line, c.label));
        }
    }
    for (_, f) in parsed {
        if let Frag::Exposition(sp) = f {
            for c in cands.iter().filter(|c| ps::eq(&c.section, &sp.section) && c.line >= sp.start.1 && c.line <= sp.end.1) {
                out.push(format!("an exposition may not hold the printed label {} ({}:{})", c.label, c.section, c.line));
            }
        }
    }
    let keys: Vec<(usize, String)> = parsed
        .iter()
        .filter_map(|(n, f)| match f {
            Frag::Object(o) if !o.label.is_empty() => Some((*n, object_id(&layout.chapter(f.pos()), &o.kind, &o.label, &o.sublabel))),
            _ => None,
        })
        .collect();
    for (i, (a, ka)) in keys.iter().enumerate() {
        if let Some((b, _)) = keys.iter().skip(i + 1).find(|(_, kb)| kb == ka) {
            out.push(format!("fragments {a} and {b} both have the key {ka}: give each its own printed label, or merge them"));
        }
    }
    out
}

// --- the old record view ---------------------------------------------------------------------------

/// The statement index records the skeleton and the later steps read (`units`, `others`), as a
/// view of the projection: one unit per object whose destination is its own unit, one other per
/// example and exercise, in book order. The display id is the kind word and the printed label
/// (`定理 1.19`), or the kind word and the name for an unnumbered item (`推论「根的乘积」`).
/// `sections` carry the prepared group and its chapter text.
pub fn legacy_view(projection: &Value, sections: &[Section]) -> (Vec<Value>, Vec<Value>) {
    let place = |file: &str| sections.iter().find(|x| ps::eq(&x.file, file)).map(|x| (x.group, x.chapter.clone())).unwrap_or((0, String::new()));
    let objects = ps::list(&projection["objects"]);
    let mut used: BTreeMap<String, usize> = BTreeMap::new();
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    for o in &objects {
        let kind = field(o, "kind");
        let label = field(&o["key"], "label");
        let zh = super::index::kind_zh(&kind);
        let numeric = NUMBER.find(&label).is_some_and(|m| m.as_str() == label);
        let base = if label.starts_with("after-") || label.is_empty() {
            format!("{zh}「{}」", field(o, "name"))
        } else if numeric && !zh.is_empty() {
            format!("{zh} {label}")
        } else {
            label.clone()
        };
        let n = used.entry(base.clone()).or_insert(0);
        *n += 1;
        ids.insert(field(o, "id"), if *n == 1 { base } else { format!("{base}（{n}）") });
    }
    // One writer per chapter exercise set: the group of the chapter's first exercise writes them all.
    let mut set_group: BTreeMap<String, i64> = BTreeMap::new();
    for o in objects.iter().filter(|o| field(o, "kind") == "exercise") {
        let (g, _) = place(&field(&o["statement"][0], "section"));
        set_group.entry(field(&o["key"], "chapter")).or_insert(g);
    }
    let span_text = |sp: &Value| match (sp["col"].as_u64(), sp["end_col"].as_u64()) {
        (Some(c), Some(e)) => format!("sections/{}:{}-{}（第 {} 行第 {} 字起，至第 {} 行第 {} 字前）", field(sp, "section"), field(sp, "line"), field(sp, "end"), field(sp, "line"), c, field(sp, "end"), e),
        _ => format!("sections/{}:{}-{}", field(sp, "section"), field(sp, "line"), field(sp, "end")),
    };
    let mut units: Vec<Value> = Vec::new();
    let mut others: Vec<Value> = Vec::new();
    for o in &objects {
        let (id, kind) = (field(o, "id"), field(o, "kind"));
        let own = field(o, "destination") == format!("unit:{id}");
        if !own && !matches!(kind.as_str(), "example" | "exercise") {
            continue;
        }
        let st = &o["statement"][0];
        let section = field(st, "section");
        let (mut group, prepared) = place(&section);
        if kind == "exercise"
            && let Some(g) = set_group.get(&field(&o["key"], "chapter"))
        {
            group = *g;
        }
        // The printed chapter: its id and its name (`第1章 实数系与复数系`); front matter keeps the
        // prepared group's chapter text.
        let chapter_id = field(&o["key"], "chapter");
        let chapter = ps::list(&projection["chapters"]).into_iter().find(|c| field(c, "id") == chapter_id).map(|c| field(c, "name")).filter(|n| !n.is_empty()).unwrap_or(prepared);
        let label = field(&o["key"], "label");
        let printed = !label.starts_with("after-");
        let numeric = printed && NUMBER.find(&label).is_some_and(|m| m.as_str() == label);
        let word = if numeric { super::index::kind_zh(&kind).to_string() } else { String::new() };
        let mut rec = json!({
            "id": ids[&id], "object_id": id,
            "label": if printed { if numeric { format!("{word} {label}") } else { label.clone() } } else { String::new() },
            "word": word, "number": if numeric { label.clone() } else { String::new() },
            "name": field(o, "name"), "group": group, "chapter": chapter, "chapter_id": chapter_id,
            "section": section, "line": st["line"], "end": st["end"],
        });
        let map = rec.as_object_mut().unwrap();
        if own {
            let steps = ps::list(&o["proof"]["steps"]);
            let proof = steps.first().map(|s| &s["spans"][0]).filter(|s| ps::eq(&field(s, "section"), &section)).and_then(|s| s["line"].as_u64()).unwrap_or(0);
            let level = if RESULTS.contains(&kind.as_str()) && !steps.is_empty() { "formal" } else { "concept" };
            let cites: Vec<String> = ps::list(&o["relations"])
                .into_iter()
                .filter(|r| field(r, "rel") == "cites" && r["target"].is_string() && s(&r["target"]) != id)
                .filter_map(|r| objects.iter().find(|x| field(x, "id") == s(&r["target"]) && field(x, "destination") == format!("unit:{}", s(&r["target"]))).map(|x| ids[&field(x, "id")].clone()))
                .collect();
            map.insert("proof".into(), json!(proof));
            // The projection's proof status and where its first step stands, in any section (a deferred
            // proof's steps are in another section, so `proof` above is 0 for it).
            map.insert("proof_status".into(), json!(field(&o["proof"], "status")));
            let at = steps.first().map(|s| &s["spans"][0]).map(|s| format!("{}:{}", field(s, "section"), field(s, "line"))).unwrap_or_default();
            map.insert("proof_at".into(), json!(at));
            map.insert("form".into(), json!(kind));
            map.insert("level".into(), json!(level));
            map.insert("cites".into(), json!(ps::select_unique(cites)));
            units.push(rec);
        } else {
            // P1a M2.2: an example whose destination is a unit is written into that unit; every other
            // example and every exercise is the group writer's (an exercise belongs to its chapter's
            // exercise set, never to the unit before it), with its printed hints and answers.
            let dest = field(o, "destination");
            let belongs = dest.strip_prefix("unit:").and_then(|t| ids.get(t)).cloned().unwrap_or_default();
            let spans_of = |role: &str| -> Vec<String> {
                objects
                    .iter()
                    .filter(|x| field(x, "kind") == role && field(x, "destination") == format!("exercise:{id}"))
                    .flat_map(|x| ps::list(&x["statement"]).into_iter().map(span_text).collect::<Vec<_>>())
                    .collect()
            };
            map.insert("proof".into(), json!(0));
            map.insert("kind".into(), json!(kind));
            map.insert("belongs_to".into(), json!(belongs));
            map.insert("after".into(), json!(belongs));
            map.insert("writer".into(), json!(if belongs.is_empty() { "group" } else { "unit" }));
            if kind == "exercise" {
                map.insert("hints".into(), json!(spans_of("hint")));
                map.insert("answers".into(), json!(spans_of("answer")));
            }
            others.push(rec);
        }
    }
    (units, others)
}

// --- the join --------------------------------------------------------------------------------------

/// One finder answer with the sections its packet gave and the finder job that wrote it.
#[derive(Clone, Debug)]
pub struct Packet {
    pub id: String,
    pub answer: Value,
    pub sections: Vec<String>,
}

#[derive(Clone, Debug)]
struct Obj {
    id: String,
    kind: String,
    chapter: String,
    namespace: String,
    label: String,
    sublabel: String,
    name: String,
    spans: Vec<Span>,
    status: String,
    reason: String,
    steps: Vec<Step>,
    dest_raw: Value,
    dest: String,
    relations: Vec<Value>,
    open: bool,
}

impl Obj {
    fn claim_spans(&self) -> Vec<Span> {
        self.spans.iter().chain(self.steps.iter().flat_map(|s| s.spans.iter())).cloned().collect()
    }

    /// An object a printed label may name: not a hint, answer, exposition or editorial piece.
    fn printed(&self) -> bool {
        !matches!(self.kind.as_str(), "hint" | "answer" | "exposition")
    }
}

struct Faults(Vec<Value>);

impl Faults {
    fn add(&mut self, code: &str, detail: String) {
        self.0.push(json!({"code": code, "detail": detail}));
    }
}

fn locator(sp: &Span) -> String {
    format!("{}:{}", sp.section, sp.start.1)
}

fn object_id(chapter: &str, namespace: &str, label: &str, sublabel: &str) -> String {
    if sublabel.is_empty() { format!("{chapter}:{namespace}:{label}") } else { format!("{chapter}:{namespace}:{label}({sublabel})") }
}

/// The object a typed key names, by its exact id. Only a key with no namespace may take the one
/// main-text object of that chapter and label (a book that numbers its kinds in one sequence). A key
/// with no chapter takes the chapter of an `N.M` label, else the chapter it is read in.
fn resolve(target: &Value, own_chapter: &str, layout: &Layout, objects: &[Obj]) -> Result<usize, String> {
    let (chapter, ns, label, sub) = typed_key(target, own_chapter, layout);
    if let Some(name) = label.strip_prefix("name:") {
        let hits: Vec<usize> = (0..objects.len())
            .filter(|&k| {
                let o = &objects[k];
                !o.open && o.printed() && o.chapter == chapter && o.label.starts_with("after-") && o.name == name && (ns.is_empty() || o.namespace == ns)
            })
            .collect();
        return match hits.as_slice() {
            [k] => Ok(*k),
            _ => Err(format!("no one unnumbered object is named {name} in {chapter}")),
        };
    }
    if ns.is_empty() {
        let hits: Vec<usize> = (0..objects.len())
            .filter(|&k| {
                let o = &objects[k];
                !o.open && o.printed() && o.chapter == chapter && o.label == label && o.sublabel == sub && o.namespace != "exercise"
            })
            .collect();
        return match hits.as_slice() {
            [k] => Ok(*k),
            _ => Err(format!("no one main-text object has the label {label} in {chapter}")),
        };
    }
    let id = object_id(&chapter, &ns, &label, &sub);
    objects.iter().position(|o| !o.open && o.id == id).ok_or_else(|| format!("no accepted object has the key {id}"))
}

/// A piece of a leaf: the whole leaf, or the part between two span boundaries inside it.
struct Piece {
    id: String,
    leaf: usize,
    start: Pos,
    end: Pos,
    whole: bool,
}

struct Claim {
    owner: usize,
    span: Span,
}

/// The interim ownership projection (`math/index.json` schema 2) from every finder answer of a
/// book, each read against its own packet's sections, in two passes: the objects first
/// (statements, exercises, figures, declared exposition and the like), then the fragments that
/// depend on them (proofs, hints, answers). `expected` is the frozen expected set of a pilot
/// (`keys`, each with an optional `section` and `line`, and `unnumbered`); every printed label the
/// scan finds must also start an accepted object or carry an approved dismissal. `edition` is the
/// frozen hash of the bilingual source. The projection carries its faults; it settles only with none.
pub fn join(packets: &[Packet], sections: &[Section], expected: Option<&Value>, edition: &str) -> Value {
    let layout = Layout::new(sections);
    let mut faults = Faults(Vec::new());
    let mut frags: Vec<Frag> = Vec::new();
    for (a, packet) in packets.iter().enumerate() {
        if packet.answer["schema"].as_i64() != Some(2) {
            faults.add("bad_fragment", format!("answer {}: the answer must declare \"schema\": 2", a + 1));
            continue;
        }
        let Some(list) = packet.answer["fragments"].as_array() else {
            faults.add("bad_fragment", format!("answer {}: the answer has no `fragments` array", a + 1));
            continue;
        };
        let scope = Scope::of(sections, &packet.sections);
        for (k, e) in list.iter().enumerate() {
            match parse_fragment(e, &scope) {
                Ok(f) => frags.push(f),
                Err(f) => faults.add("bad_fragment", format!("answer {} fragment {}: {f}", a + 1, k + 1)),
            }
        }
    }
    frags.sort_by_key(|f| f.pos());
    let mut objects = pass_one(&frags, &layout, &mut faults);
    let unresolved = pass_two(&frags, &layout, &mut objects, &mut faults);
    settle_objects(&mut objects, &layout, &mut faults);

    let cands = candidates(sections);
    let dismissed: Vec<Value> = frags
        .iter()
        .filter_map(|f| match f {
            Frag::Dismiss { section, line, reason, .. } => Some(json!({"section": section, "line": line, "reason": reason})),
            _ => None,
        })
        .collect();
    for (k, d) in dismissed.iter().enumerate() {
        if dismissed[..k].iter().any(|e| ps::eq(&field(e, "section"), &field(d, "section")) && e["line"] == d["line"]) {
            faults.add("dismissal_conflict", format!("{}:{}: two dismissals of one printed label", field(d, "section"), d["line"]));
        }
    }
    let is_dismissed = |c: &Candidate| dismissed.iter().any(|d| ps::eq(&field(d, "section"), &c.section) && d["line"].as_u64() == Some(c.line as u64));
    for o in objects.iter().filter(|o| o.kind == "exposition" && !o.open) {
        let sp = &o.spans[0];
        if layout.chapter(sp.end) != o.chapter {
            faults.add("exposition_crosses_chapter", format!("{}: {}", o.id, sp.name()));
        }
        for c in cands.iter().filter(|c| ps::eq(&c.section, &sp.section) && c.line >= sp.start.1 && c.line <= sp.end.1 && !is_dismissed(c)) {
            faults.add("exposition_hides_label", format!("{}:{} opens with the printed label {}", c.section, c.line, c.label));
        }
    }

    let leaf_rows = claim_leaves(sections, &layout, &objects, &mut faults);

    // Every printed occurrence the scan finds starts an accepted object of its label, or carries an
    // approved dismissal (a running head, an OCR duplicate).
    for c in cands.iter().filter(|c| !is_dismissed(c)) {
        let hit = objects.iter().any(|o| {
            !o.open && o.printed() && o.chapter == c.chapter && o.label == c.label && (o.namespace == "exercise") == c.exercise && o.spans.iter().any(|sp| ps::eq(&sp.section, &c.section) && sp.start.1 == c.line)
        });
        if !hit {
            faults.add("candidate_unmatched", format!("{}:{} prints {} and no accepted object of that label starts there", c.section, c.line, c.label));
        }
    }
    // A pilot's frozen expected set: exact typed keys (and their locators when given), and the
    // unnumbered items by kind and locator.
    if let Some(exp) = expected {
        for k in ps::list(&exp["keys"]) {
            let (ns, ch, label, sub) = (field(k, "namespace").to_lowercase(), field(k, "chapter"), norm_label(&field(k, "label")), field(k, "sublabel"));
            let id = object_id(&ch, &ns, &label, &sub);
            let (file, line) = (field(k, "section"), k["line"].as_u64());
            if file.is_empty() != line.is_none() || (!k["line"].is_null() && line.is_none()) {
                faults.add("expected_bad", format!("{id}: a locator needs both `section` and a numeric `line`"));
                continue;
            }
            // An approved disposition of a frozen key stands for its object.
            if !field(k, "disposition").trim().is_empty() {
                continue;
            }
            let at = |o: &Obj| line.is_none_or(|l| o.spans.iter().any(|sp| ps::eq(&sp.section, &file) && sp.start.1 as u64 == l));
            if !objects.iter().any(|o| !o.open && o.id == id && at(o)) {
                faults.add("expected_missing", id);
            }
        }
        for u in ps::list(&exp["unnumbered"]) {
            let (kind, file, line) = (field(u, "kind").to_lowercase(), field(u, "section"), u["line"].as_u64().unwrap_or(0) as usize);
            if !objects.iter().any(|o| !o.open && o.kind == kind && ps::eq(&o.spans[0].section, &file) && o.spans[0].start.1 == line) {
                faults.add("expected_missing", format!("{kind} at {file}:{line}"));
            }
        }
    }

    let accepted: Vec<&Obj> = objects.iter().filter(|o| !o.open).collect();
    let object_rows: Vec<Value> = accepted
        .iter()
        .map(|o| {
            json!({
                "id": o.id, "kind": o.kind, "name": o.name,
                "key": {"chapter": o.chapter, "namespace": o.namespace, "label": o.label, "sublabel": o.sublabel},
                "statement": o.spans.iter().map(Span::json).collect::<Vec<_>>(),
                "proof": {"status": o.status, "reason": o.reason, "steps": steps_json(&o.steps)},
                "destination": o.dest,
                "relations": o.relations,
            })
        })
        .collect();
    // Each chapter's exercise set, in book order, with its heading leaf: the binding its one writer
    // receives in Milestone 2.
    let mut sets: Vec<Value> = Vec::new();
    for c in chapters(sections) {
        let ex: Vec<&&Obj> = accepted.iter().filter(|o| o.chapter == c.id && o.kind == "exercise").collect();
        if ex.is_empty() {
            continue;
        }
        let heading = layout.exercises.iter().find(|p| layout.chapter(**p) == c.id).and_then(|p| {
            let sec = &sections[p.0];
            leaves(&sec.file, &sec.lines).into_iter().find(|l| l.line == p.1).map(|l| l.id)
        });
        let of = |kind: &str| accepted.iter().filter(|o| o.chapter == c.id && o.kind == kind).map(|o| o.id.clone()).collect::<Vec<_>>();
        sets.push(json!({"chapter": c.id, "heading": heading, "exercises": ex.iter().map(|o| o.id.clone()).collect::<Vec<_>>(), "hints": of("hint"), "answers": of("answer")}));
    }
    let mut source = Map::new();
    for sec in sections {
        source.insert(sec.file.clone(), json!(canon::sha256_hex(sec.lines.join("\n").as_bytes())));
    }
    let chapter_rows: Vec<Value> = chapters(sections)
        .iter()
        .map(|c| json!({"id": c.id, "label": c.label, "title": c.title, "name": chapter_name(c), "ranges": c.ranges.iter().map(|(f, a, b)| json!({"section": f, "line": a, "end": b})).collect::<Vec<_>>()}))
        .collect();
    let mut p = json!({
        "schema_version": 2,
        "source": {"edition": edition, "sections": source},
        "chapters": chapter_rows,
        "leaves": leaf_rows,
        "objects": object_rows,
        "exercise_sets": sets,
        "dismissed": dismissed,
        "candidates": cands.iter().map(|c| json!({"chapter": c.chapter, "exercise": c.exercise, "label": c.label, "section": c.section, "line": c.line})).collect::<Vec<_>>(),
        "unresolved": unresolved,
    });
    // Each fault names the finder jobs whose answers it touches, so a repair round can send it back
    // to exactly those jobs.
    let mut annotated = faults.0;
    for f in annotated.iter_mut() {
        let jobs = fault_jobs(&field(f, "detail"), &field(f, "code"), packets, &objects, sections);
        f["jobs"] = json!(jobs);
    }
    p["faults"] = json!(annotated);
    let hash = projection_hash(&p);
    p["hash"] = json!(hash);
    p
}

/// The finder jobs a fault touches: the packets whose sections its locators name, the packets of the
/// objects it names, and for a frozen key that no object has, the packets of that chapter.
fn fault_jobs(detail: &str, code: &str, packets: &[Packet], objects: &[Obj], sections: &[Section]) -> Vec<String> {
    let mut jobs: BTreeSet<String> = BTreeSet::new();
    let job_of = |file: &str| packets.iter().find(|p| p.sections.iter().any(|f| ps::eq(f, file))).map(|p| p.id.clone());
    for sec in sections {
        if detail.contains(&format!("{}:", sec.file))
            && let Some(j) = job_of(&sec.file)
        {
            jobs.insert(j);
        }
    }
    let ends_id = |rest: &str| rest.chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '.' || c == '-' || c == '('));
    for o in objects {
        if let Some(i) = detail.find(&o.id)
            && ends_id(&detail[i + o.id.len()..])
            && let Some(j) = job_of(&o.spans[0].section)
        {
            jobs.insert(j);
        }
    }
    if let Some(rest) = detail.strip_prefix("answer ")
        && let Some(n) = rest.split(|c: char| !c.is_ascii_digit()).next().and_then(|n| n.parse::<usize>().ok())
        && let Some(p) = packets.get(n.wrapping_sub(1))
    {
        jobs.insert(p.id.clone());
    }
    if jobs.is_empty() && code == "expected_missing" {
        let chapter = detail.split(':').next().unwrap_or("");
        for c in chapters(sections).iter().filter(|c| c.id == chapter) {
            for (file, _, _) in &c.ranges {
                if let Some(j) = job_of(file) {
                    jobs.insert(j);
                }
            }
        }
    }
    jobs.into_iter().collect()
}

/// Pass one: the independently identified objects and the declared exposition, in book order, with
/// their typed keys. An unnumbered object takes an ordinal after the last printed label of its
/// chapter. Two objects with one key both stay open.
fn pass_one(frags: &[Frag], layout: &Layout, faults: &mut Faults) -> Vec<Obj> {
    let mut objects: Vec<Obj> = Vec::new();
    let mut last_label: BTreeMap<(String, bool), String> = BTreeMap::new();
    let mut ordinals: BTreeMap<(String, String, String), usize> = BTreeMap::new();
    let mut expositions: BTreeMap<String, usize> = BTreeMap::new();
    for f in frags {
        if let Frag::Exposition(span) = f {
            let chapter = layout.chapter(span.start);
            let n = expositions.entry(chapter.clone()).or_insert(0);
            *n += 1;
            objects.push(Obj {
                id: object_id(&chapter, "exposition", &n.to_string(), ""),
                kind: "exposition".into(),
                namespace: "exposition".into(),
                label: n.to_string(),
                sublabel: String::new(),
                name: String::new(),
                spans: vec![span.clone()],
                status: "not_applicable".into(),
                reason: String::new(),
                steps: Vec::new(),
                dest_raw: Value::Null,
                dest: format!("narrative:{chapter}"),
                relations: Vec::new(),
                chapter,
                open: false,
            });
            continue;
        }
        let Frag::Object(o) = f else { continue };
        let chapter = layout.chapter(o.spans[0].start);
        let exercise = o.kind == "exercise";
        let label = if o.label.is_empty() {
            let after = last_label.get(&(chapter.clone(), exercise)).cloned().unwrap_or_else(|| "start".into());
            let n = ordinals.entry((chapter.clone(), o.kind.clone(), after.clone())).or_insert(0);
            *n += 1;
            format!("after-{after}-{n}")
        } else {
            last_label.insert((chapter.clone(), exercise), o.label.clone());
            o.label.clone()
        };
        let id = object_id(&chapter, &o.kind, &label, &o.sublabel);
        let mut open = false;
        if let Some(k) = objects.iter().position(|x| x.id == id) {
            objects[k].open = true;
            open = true;
            faults.add("duplicate_key", format!("{id} at {} and {}", locator(&objects[k].spans[0]), locator(&o.spans[0])));
        }
        objects.push(Obj {
            id,
            kind: o.kind.clone(),
            chapter,
            namespace: o.kind.clone(),
            label,
            sublabel: o.sublabel.clone(),
            name: o.name.clone(),
            spans: o.spans.clone(),
            status: o.status.clone(),
            reason: o.reason.clone(),
            steps: o.steps.clone(),
            dest_raw: o.dest.clone(),
            dest: String::new(),
            relations: o.relations.clone(),
            open,
        });
    }
    for k in 0..objects.len() {
        let o = &objects[k];
        let parent = object_id(&o.chapter, &o.namespace, &o.label, "");
        if !o.sublabel.is_empty() && !objects.iter().any(|p| !p.open && p.id == parent) {
            let id = o.id.clone();
            objects[k].open = true;
            faults.add("no_parent", format!("{id}: no accepted object for its whole label"));
        }
    }
    objects
}

/// Pass two: proofs, hints and answers bind to the accepted objects of pass one, each by its typed
/// key and the quote of its own packet that names it. The fragments of one proof, inline or not,
/// join in source order; a fragment that does not bind is kept, with its spans and the reason, and
/// blocks.
fn pass_two(frags: &[Frag], layout: &Layout, objects: &mut Vec<Obj>, faults: &mut Faults) -> Vec<Value> {
    let mut unresolved: Vec<Value> = Vec::new();
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for f in frags {
        let Frag::Dependent(d) = f else { continue };
        let label = norm_label(&field(&d.target, "label"));
        let at = locator(f.owned_spans()[0]);
        let mut keep = |reason: String, faults: &mut Faults, code: &str| {
            faults.add(code, format!("the {} at {at}: {reason}", d.role));
            unresolved.push(json!({"role": d.role, "target": d.target, "evidence": d.evidence, "steps": steps_json(&d.steps), "spans": d.spans.iter().map(Span::json).collect::<Vec<_>>(), "reason": reason}));
        };
        if !d.evidence_ok {
            keep(format!("it names `{label}` without a quote of its own packet that names it"), faults, "no_key_evidence");
            continue;
        }
        let k = match resolve(&d.target, &layout.chapter(f.pos()), layout, objects) {
            Ok(k) => k,
            Err(e) => {
                keep(e, faults, "unresolved_target");
                continue;
            }
        };
        if d.role == "proof" {
            // A continuation joins the steps the object already has, inline or joined, in source
            // order; two fragments that claim the same text fault as `proof_overlap`.
            objects[k].steps.extend(d.steps.iter().cloned());
            objects[k].relations.extend(d.relations.iter().cloned());
            continue;
        }
        if objects[k].namespace != "exercise" {
            let id = objects[k].id.clone();
            keep(format!("{id} is not an exercise"), faults, "unresolved_target");
            continue;
        }
        let tid = objects[k].id.clone();
        let n = counts.entry((tid.clone(), d.role.clone())).or_insert(0);
        *n += 1;
        let suffix = if *n == 1 { d.role.clone() } else { format!("{}-{n}", d.role) };
        let (chapter, olabel) = (objects[k].chapter.clone(), objects[k].label.clone());
        objects.push(Obj {
            id: format!("{tid}:{suffix}"),
            kind: d.role.clone(),
            chapter,
            namespace: d.role.clone(),
            label: olabel,
            sublabel: String::new(),
            name: String::new(),
            spans: d.spans.clone(),
            status: "not_applicable".into(),
            reason: String::new(),
            steps: Vec::new(),
            dest_raw: Value::Null,
            dest: format!("exercise:{tid}"),
            relations: Vec::new(),
            open: false,
        });
    }
    unresolved
}

/// The proof status, destination and relations of every accepted object.
fn settle_objects(objects: &mut [Obj], layout: &Layout, faults: &mut Faults) {
    let snapshot = objects.to_vec();
    for o in objects.iter_mut().filter(|o| !o.open) {
        let result = RESULTS.contains(&o.kind.as_str());
        if o.status.is_empty() {
            if result {
                faults.add("no_proof_status", format!("{}: a result needs its proof status", o.id));
            } else {
                o.status = "not_applicable".into();
            }
        }
        match o.status.as_str() {
            "inline" | "deferred_in_slice" if o.steps.is_empty() => faults.add(
                "proof_missing",
                format!("{}: a {} proof has no steps in these chapters: write its steps where the proof stands, or, when the book proves it in a chapter these files do not hold, use `external` with that reason", o.id, o.status),
            ),
            "source_omits" | "external" if o.reason.trim().is_empty() => faults.add("proof_reason", format!("{}: `{}` needs its reason", o.id, o.status)),
            "not_applicable" if result => faults.add("proof_status_wrong", format!("{}: a result owes a source treatment (inline, deferred_in_slice, source_omits or external)", o.id)),
            "source_omits" | "external" | "not_applicable" if !o.steps.is_empty() => faults.add("proof_conflict", format!("{}: `{}` with proof steps", o.id, o.status)),
            _ => {}
        }
        // Every proof publishes in source order, inline or joined: spans within a step, then steps.
        for st in o.steps.iter_mut() {
            st.spans.sort_by_key(|s| s.start);
        }
        o.steps.sort_by_key(|s| s.spans[0].start);
        // The statement and every step of one object claim disjoint text.
        let all = o.claim_spans();
        if all.iter().enumerate().any(|(i, a)| all.iter().skip(i + 1).any(|b| a.overlaps(b))) {
            faults.add("proof_overlap", format!("{}: two of its statement and proof spans claim the same text", o.id));
        }
        if o.dest.is_empty() {
            o.dest = match o.kind.as_str() {
                k if UNIT_FORMS.contains(&k) => format!("unit:{}", o.id),
                "exercise" => format!("exercise:{}", o.id),
                "figure" => format!("figure:{}", o.id),
                "editorial" => "provenance".into(),
                "boilerplate" => "boilerplate_skip".into(),
                _ => match field(&o.dest_raw, "to").as_str() {
                    "narrative" => format!("narrative:{}", o.chapter),
                    "unit" => match resolve(&o.dest_raw["target"], &o.chapter, layout, &snapshot) {
                        Ok(k) if UNIT_FORMS.contains(&snapshot[k].kind.as_str()) => format!("unit:{}", snapshot[k].id),
                        _ => String::new(),
                    },
                    _ => String::new(),
                },
            };
            if o.dest.is_empty() && CHOSEN.contains(&o.kind.as_str()) {
                faults.add("open_destination", format!("{}: name `narrative` or a unit it serves", o.id));
            }
        }
        let mut rels = Vec::new();
        for r in &o.relations {
            let rel = field(r, "rel");
            let (target, status, reason) = match resolve(&r["target"], &o.chapter, layout, &snapshot) {
                Ok(k) => (json!(snapshot[k].id), "resolved", String::new()),
                Err(e) => (Value::Null, "unresolved", e),
            };
            // An unknown citation is kept with its reason; an open containment or illustration blocks.
            if status == "unresolved" && rel != "cites" {
                faults.add("open_relation", format!("{}: {rel}: {reason}", o.id));
            }
            rels.push(json!({"rel": rel, "target": target, "key": r["target"].clone(), "raw": field(r, "raw"), "status": status, "reason": reason}));
        }
        o.relations = rels;
    }
}

/// Every leaf, cut at the span boundaries inside it, with its one primary claim. When several
/// objects claim a piece, the one innermost object wins: every other claimant names it in a
/// `contains` relation and has a span that encloses its span. Anything else is a double claim.
fn claim_leaves(sections: &[Section], layout: &Layout, objects: &[Obj], faults: &mut Faults) -> Vec<Value> {
    let mut claims: Vec<Claim> = Vec::new();
    for (k, o) in objects.iter().enumerate().filter(|(_, o)| !o.open) {
        for sp in o.claim_spans() {
            claims.push(Claim { owner: k, span: sp });
        }
    }
    // A containment holds when the outer object names the inner one and its spans enclose every
    // span of the inner object, not only the spans of the piece at hand.
    let mut contains: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (k, o) in objects.iter().enumerate().filter(|(_, o)| !o.open) {
        for r in o.relations.iter().filter(|r| field(r, "rel") == "contains" && r["target"].is_string()) {
            if let Some(i) = objects.iter().position(|x| !x.open && x.id == s(&r["target"])) {
                if encloses(&o.claim_spans(), &objects[i].claim_spans()) {
                    contains.insert((k, i));
                } else {
                    faults.add("containment_not_enclosed", format!("{} contains {}, but its spans do not enclose it", o.id, objects[i].id));
                }
            }
        }
    }
    let cuts: BTreeSet<Pos> = claims.iter().flat_map(|c| [c.span.start, c.span.end]).collect();
    let mut all_leaves: Vec<(usize, Leaf)> = Vec::new();
    for (si, sec) in sections.iter().enumerate() {
        for l in leaves(&sec.file, &sec.lines) {
            all_leaves.push((si, l));
        }
    }
    let mut pieces: Vec<Piece> = Vec::new();
    for (k, (si, l)) in all_leaves.iter().enumerate() {
        let a: Pos = (*si, l.line, 0);
        let b: Pos = (*si, l.end, line_len(sections, *si, l.end));
        let mut bounds = vec![a];
        bounds.extend(cuts.range(a..b).filter(|c| **c > a).copied());
        bounds.push(b);
        for w in bounds.windows(2) {
            let (p, q) = (w[0], w[1]);
            if squeeze(&text_between(sections, p, q)).is_empty() {
                continue;
            }
            let whole = p == a && q == b;
            let id = if whole { l.id.clone() } else { format!("{}:{}:{}-{}:{}", l.section, p.1, p.2, q.1, q.2) };
            pieces.push(Piece { id, leaf: k, start: p, end: q, whole });
        }
    }
    let mut rows: Vec<Value> = Vec::new();
    for pc in &pieces {
        let l = &all_leaves[pc.leaf].1;
        let covering: Vec<&Claim> = claims.iter().filter(|c| c.span.start <= pc.start && pc.end <= c.span.end).collect();
        let mut owners: Vec<usize> = covering.iter().map(|c| c.owner).collect();
        owners.dedup();
        owners.sort();
        owners.dedup();
        let inner = |i: usize| owners.iter().all(|&o| o == i || contains.contains(&(o, i)));
        let winners: Vec<usize> = owners.iter().copied().filter(|&i| inner(i)).collect();
        let winner = match (owners.len(), winners.as_slice()) {
            (0, _) => None,
            (_, [w]) => Some(*w),
            _ => {
                let names: Vec<&str> = owners.iter().map(|&o| objects[o].id.as_str()).collect();
                faults.add("double_claim", format!("{}: {}", pc.id, names.join(", ")));
                None
            }
        };
        if owners.is_empty() && l.obligation {
            faults.add("unclaimed", format!("{} ({})", pc.id, l.kind));
        }
        let mut row = Map::new();
        row.insert("id".into(), json!(pc.id));
        row.insert("leaf".into(), json!(l.id));
        row.insert("section".into(), json!(l.section));
        row.insert("line".into(), json!(pc.start.1));
        row.insert("end".into(), json!(pc.end.1));
        if !pc.whole {
            row.insert("col".into(), json!(pc.start.2));
            row.insert("end_col".into(), json!(pc.end.2));
        }
        row.insert("kind".into(), json!(l.kind));
        row.insert("chapter".into(), json!(layout.chapter(pc.start)));
        row.insert("obligation".into(), json!(if l.obligation { "write" } else { "none" }));
        row.insert("reason".into(), json!(l.reason));
        row.insert("attached".into(), if pc.whole { json!(l.attached) } else { Value::Null });
        row.insert("object_id".into(), winner.map(|w| json!(objects[w].id)).unwrap_or(Value::Null));
        row.insert("destination".into(), winner.map(|w| json!(objects[w].dest)).unwrap_or(Value::Null));
        rows.push(Value::Object(row));
    }
    rows
}

/// The index receipt's hash: source identities, spans, destinations, obligations and the faults
/// (so a projection's validation outcome cannot change under its hash); never the hash itself, or
/// anything a later step binds.
pub fn projection_hash(projection: &Value) -> String {
    canon::content_hash(projection, &["hash"])
}

/// The allocation record of a settled projection: one unit per `unit:` destination of its own
/// object, in book order, bound to the projection's hash. A projection with faults, or one changed
/// after its hash was taken, is refused. The projection itself is never rewritten: the Card address
/// is filled in this record.
pub fn allocate(projection: &Value) -> Result<Value, String> {
    let faults = ps::list(&projection["faults"]).len();
    if faults > 0 {
        return Err(format!("the projection has {faults} faults; it is not settled"));
    }
    let hash = projection_hash(projection);
    if field(projection, "hash") != hash {
        return Err("the projection changed after its hash was taken".into());
    }
    let units: Vec<Value> = ps::list(&projection["objects"])
        .into_iter()
        .filter(|o| field(o, "destination") == format!("unit:{}", field(o, "id")))
        .enumerate()
        .map(|(k, o)| json!({"object_id": field(o, "id"), "unit": format!("u{:04}", k + 1), "card": null}))
        .collect();
    Ok(json!({"index_hash": hash, "units": units}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ps::{field, s};

    fn sec(file: &str, text: &str) -> Section {
        Section { file: file.into(), group: 1, chapter: String::new(), lines: text.split('\n').map(String::from).collect() }
    }

    fn line_of(sections: &[Section], file: &str, n: usize) -> String {
        sections.iter().find(|x| x.file == file).unwrap().lines[n - 1].clone()
    }

    fn codes(p: &Value) -> Vec<String> {
        p["faults"].as_array().unwrap().iter().map(|f| field(f, "code")).collect()
    }

    fn fault_text(p: &Value) -> String {
        p["faults"].as_array().unwrap().iter().map(|f| format!("{}: {}", field(f, "code"), field(f, "detail"))).collect::<Vec<_>>().join("\n")
    }

    fn object<'a>(p: &'a Value, id: &str) -> &'a Value {
        p["objects"].as_array().unwrap().iter().find(|o| field(o, "id") == id).unwrap_or_else(|| panic!("no object {id} in {}", p["objects"]))
    }

    fn leaf<'a>(p: &'a Value, id: &str) -> &'a Value {
        p["leaves"].as_array().unwrap().iter().find(|o| field(o, "id") == id).unwrap_or_else(|| panic!("no leaf {id} in {}", p["leaves"]))
    }

    fn sp(file: &str, line: usize, end: usize, quote: &str) -> Value {
        json!({"section": file, "line": line, "end": end, "quote": quote})
    }

    // --- the book ---------------------------------------------------------------------------------
    //
    // A synthetic Chapter 1 in the shape of baby Rudin's (own words, not the book's text): a
    // theorem whose proof is the Appendix, two theorems with inline proofs, an unnumbered corollary,
    // a named entry, the Appendix in nine steps, and the exercises with a hint on its question's line.

    fn book() -> Vec<Section> {
        let s01 = [
            "# 第 1 章 实数系",
            "",
            "我们先回顾有理数的不足之处，并说明为什么需要一个更大的数系来讨论极限。",
            "",
            "1.19 定理。存在一个具有最小上界性质的有序域，它包含有理数域作为它的一个子域。",
            "",
            "证明放在本章附录。",
            "",
            "1.20 定理。若 x 与 y 为实数且 x 大于零，则存在正整数 n 使得 n 乘 x 大于 y。",
            "",
            "证明。设结论不成立，则这个集合有上界，取它的最小上界即可推出矛盾。",
            "",
            "1.21 定理。对每个正实数 x 与正整数 n，恰有一个正实数 y 使得 y 的 n 次方等于 x。",
            "",
            "证明。令 E 为满足条件的正实数之集，并证明它的上确界就是所求的那个数。",
            "",
            "推论。若 a 与 b 为正实数而 n 为正整数，则 ab 的 n 次方根等于两者 n 次方根之积。",
            "",
            "证明。由上面定理中的唯一性直接可以得到这个等式。",
            "",
            "1.22 小数。我们说明实数与十进制小数展开之间的关系，作为本节的结束语。",
        ]
        .join("\n");
        let mut s02 = vec!["# 附录".to_string(), String::new(), "本附录给出定理 1.19 的证明，分九步从有理数构造出实数。".to_string()];
        for k in 1..=9 {
            s02.push(String::new());
            s02.push(format!("第 {k} 步 这一步完成构造中的第 {k} 个部分，并核对所需的性质成立。"));
        }
        let s03 = [
            "# 习题",
            "",
            "1. 若 r 为非零有理数而 x 为无理数，证明 r 加 x 是无理数。",
            "2. 证明不存在平方等于十二的有理数。提示：仿照前面例子中的论证。",
            "19. 设 a 与 b 属于 k 维空间，求所有到 a 的距离为到 b 距离两倍的点。",
        ]
        .join("\n");
        vec![sec("s01.md", &s01), sec("s02.md", &s02.join("\n")), sec("s03.md", &s03)]
    }

    fn inline(status: &str, file: &str, line: usize, quote: &str) -> Value {
        json!({"status": status, "steps": [{"label": "", "spans": [sp(file, line, line, quote)]}]})
    }

    /// Packet A: the chapter text. Packet B: the Appendix. Packet C: the exercises.
    fn packet_a() -> Value {
        json!({"schema": 2, "fragments": [
            {"role": "exposition", "spans": [sp("s01.md", 3, 3, "我们先回顾有理数")]},
            {"role": "object", "kind": "theorem", "label": "1.19", "name": "实数域的存在", "spans": [sp("s01.md", 5, 7, "1.19 定理")], "proof": {"status": "deferred_in_slice"}},
            {"role": "object", "kind": "theorem", "label": "1.20", "name": "阿基米德性质", "spans": [sp("s01.md", 9, 9, "1.20 定理")], "proof": inline("inline", "s01.md", 11, "证明。设结论")},
            {"role": "object", "kind": "theorem", "label": "1.21", "name": "n 次方根", "spans": [sp("s01.md", 13, 13, "1.21 定理")], "proof": inline("inline", "s01.md", 15, "证明。令 E"), "relations": [{"rel": "cites", "target": {"namespace": "theorem", "label": "1.20"}, "raw": "定理 1.20"}]},
            {"role": "object", "kind": "corollary", "label": "", "name": "根的乘积", "spans": [sp("s01.md", 17, 17, "推论。若 a")], "proof": inline("inline", "s01.md", 19, "证明。由上面")},
            {"role": "object", "kind": "remark", "label": "1.22", "name": "小数", "spans": [sp("s01.md", 21, 21, "1.22 小数")], "destination": {"to": "narrative"}}
        ]})
    }

    fn packet_b() -> Value {
        let steps: Vec<Value> = (1..=9)
            .map(|k| {
                let line = 3 + 2 * k;
                let mut spans = vec![sp("s02.md", line, line, &format!("第 {k} 步 这一步"))];
                if k == 1 {
                    spans.insert(0, sp("s02.md", 3, 3, "本附录给出"));
                }
                json!({"label": format!("第 {k} 步"), "spans": spans})
            })
            .collect();
        json!({"schema": 2, "fragments": [
            {"role": "proof", "target": {"namespace": "theorem", "label": "1.19"}, "evidence": {"section": "s02.md", "line": 3, "quote": "定理 1.19 的证明"}, "steps": steps}
        ]})
    }

    fn packet_c(b: &[Section]) -> Value {
        let l4 = line_of(b, "s03.md", 4);
        let k = l4.split("提示").next().unwrap().chars().count();
        let n = l4.chars().count();
        json!({"schema": 2, "fragments": [
            {"role": "object", "kind": "exercise", "label": "1", "name": "有理加无理", "spans": [sp("s03.md", 3, 3, "1. 若 r")]},
            {"role": "object", "kind": "exercise", "label": "2", "name": "根号十二", "spans": [{"section": "s03.md", "line": 4, "end": 4, "end_col": k, "quote": "2. 证明不存在"}]},
            {"role": "hint", "target": {"namespace": "exercise", "label": "2"}, "evidence": {"section": "s03.md", "line": 4, "quote": "2. 证明不存在"}, "spans": [{"section": "s03.md", "line": 4, "end": 4, "col": k, "end_col": n, "quote": "提示：仿照"}]},
            {"role": "object", "kind": "exercise", "label": "19", "name": "阿波罗尼斯球", "spans": [sp("s03.md", 5, 5, "19. 设 a")]}
        ]})
    }

    const ED: &str = "test-edition";

    fn pk(answer: Value, files: &[&str]) -> Packet {
        Packet { id: files.join(","), answer, sections: files.iter().map(|f| f.to_string()).collect() }
    }

    fn pa(a: Value) -> Packet {
        pk(a, &["s01.md"])
    }

    fn pb(a: Value) -> Packet {
        pk(a, &["s02.md"])
    }

    fn pc(a: Value) -> Packet {
        pk(a, &["s03.md"])
    }

    fn jn(packets: &[Packet], b: &[Section], expected: Option<&Value>) -> Value {
        join(packets, b, expected, ED)
    }

    fn answers(b: &[Section]) -> Vec<Packet> {
        vec![pa(packet_a()), pb(packet_b()), pc(packet_c(b))]
    }

    // --- 1. leaf inventory ------------------------------------------------------------------------

    #[test]
    fn every_block_is_a_leaf_and_short_text_is_kept() {
        let lines: Vec<String> = "# 标题\n\n证明。\n\n![图](artifact:f1.png)\n\n| a | b |\n| - | - |\n\n- 第一项内容足够长足够长足够长足够长足够长".split('\n').map(String::from).collect();
        let ls = leaves("s09.md", &lines);
        let kinds: Vec<(&str, usize, usize, bool)> = ls.iter().map(|l| (l.kind, l.line, l.end, l.obligation)).collect();
        assert_eq!(kinds, [("heading", 1, 1, false), ("paragraph", 3, 3, true), ("figure", 5, 5, true), ("table", 7, 8, true), ("list", 10, 10, true)]);
        assert_eq!(ls[0].reason, "heading");
        assert_eq!(ls[1].id, "s09.md:3-3");
        assert_eq!(ls[1].text, "证明。");
    }

    #[test]
    fn an_attached_display_and_an_isolated_display_are_separate_cases() {
        let text = "这是一段足够长的文字，后面紧跟着一个公式块，没有空的段落。\n\n$$\nx^2\n$$\n\n# 小节\n\n$$ y $$";
        let lines: Vec<String> = text.split('\n').map(String::from).collect();
        let ls = leaves("s01.md", &lines);
        let displays: Vec<(usize, Option<String>)> = ls.iter().filter(|l| l.kind == "display").map(|l| (l.line, l.attached.clone())).collect();
        assert_eq!(displays, [(3, Some("s01.md:1-1".to_string())), (9, None)]);
        assert!(ls.iter().all(|l| l.kind == "heading" || l.obligation), "a display, attached or isolated, owes a claim");
    }

    #[test]
    fn the_legacy_paragraph_units_are_a_projection_of_the_leaves() {
        let text = "第一段文字足够长，一共超过二十个非空白的字符，作为普通段落。\n\n$$\na=b\n$$\n\n短句。\n\n$$ c $$\n\n# 标题\n\n| x | y |\n| 1 | 2 |\n\n![f](artifact:a.png)\n\n最后一段也足够长，超过二十个非空白字符，用来检查。\n$$ d $$";
        let lines: Vec<String> = text.split('\n').map(String::from).collect();
        assert_eq!(legacy_units(&leaves("s05.md", &lines)), crate::coverage::units("s05.md", text));
        for s in book() {
            assert_eq!(legacy_units(&leaves(&s.file, &s.lines)), crate::coverage::units(&s.file, &s.lines.join("\n")));
        }
    }

    #[test]
    fn chapter_ranges_may_split_a_staged_section() {
        let b = vec![
            sec("s01.md", "# 前言\n\n本书的目的。\n\n# 第 1 章 实数\n\n正文。\n\n# Chapter 2\n\n正文二。"),
            sec("s02.md", "更多第二章的正文。\n\n# 第三章 数列\n\n正文三。"),
        ];
        let ch = chapters(&b);
        let got: Vec<(&str, Vec<(String, usize, usize)>)> = ch.iter().map(|c| (c.id.as_str(), c.ranges.clone())).collect();
        assert_eq!(
            got,
            [
                ("front", vec![("s01.md".into(), 1, 4)]),
                ("ch1", vec![("s01.md".into(), 5, 8)]),
                ("ch2", vec![("s01.md".into(), 9, 11), ("s02.md".into(), 1, 2)]),
                ("ch3", vec![("s02.md".into(), 3, 5)]),
            ]
        );
        assert_eq!(ch[2].label, "Chapter 2");
        // A bilingual source heads a chapter twice: the second heading continues the chapter.
        let b = vec![sec("s01.md", "# Chapter 1\n\n# 第1章\n\n正文。\n\n# Chapter 2\n\n# 第 2 章\n\n正文二。")];
        let got: Vec<(String, Vec<(String, usize, usize)>)> = chapters(&b).into_iter().map(|c| (c.id, c.ranges)).collect();
        assert_eq!(got, [("ch1".to_string(), vec![("s01.md".to_string(), 1, 6)]), ("ch2".to_string(), vec![("s01.md".to_string(), 7, 11)])]);
    }

    #[test]
    fn the_label_scan_reports_main_text_and_exercise_labels_by_chapter() {
        let c = candidates(&book());
        let main: Vec<String> = c.iter().filter(|x| !x.exercise).map(|x| x.label.clone()).collect();
        let ex: Vec<(String, usize)> = c.iter().filter(|x| x.exercise).map(|x| (x.label.clone(), x.line)).collect();
        assert_eq!(main, ["1.19", "1.20", "1.21", "1.22"]);
        assert_eq!(ex, [("1".to_string(), 3), ("2".to_string(), 4), ("19".to_string(), 5)]);
        assert!(c.iter().all(|x| x.chapter == "ch1"), "the Appendix and the exercises stay in Chapter 1");
    }

    #[test]
    fn a_label_the_ocr_made_a_heading_owes_a_claim_and_a_bare_exercise_number_is_a_label() {
        // Rudin's OCR prints `# 3.4 Theorem.` as a heading and `2.19.` alone on its line.
        let b = vec![sec("s01.md", "# Chapter 3\n\n# 3.4 Theorem.\n\n(a) A bounded sequence in R^k has a convergent subsequence.\n\n# EXERCISES\n\n3.18.\n3.19. Replace the recursion by a fixed point.")];
        let ls = leaves("s01.md", &b[0].lines);
        assert_eq!((ls[1].kind, ls[1].obligation, ls[1].reason), ("heading", true, ""), "a labelled heading is instructional");
        assert_eq!((ls[0].obligation, ls[3].obligation), (false, false), "a plain heading owes nothing");
        let c: Vec<(bool, String)> = candidates(&b).into_iter().map(|x| (x.exercise, x.label)).collect();
        assert_eq!(c, [(false, "3.4".to_string()), (true, "3.18".to_string()), (true, "3.19".to_string())]);
    }

    #[test]
    fn a_hint_on_its_questions_line_is_a_leaf_of_its_own() {
        let b = book();
        let p = jn(&answers(&b), &b, None);
        assert!(codes(&p).is_empty(), "{}", fault_text(&p));
        let l4 = line_of(&b, "s03.md", 4);
        let k = l4.split("提示").next().unwrap().chars().count();
        let n = l4.chars().count();
        let q = leaf(&p, &format!("s03.md:4:0-4:{k}"));
        let h = leaf(&p, &format!("s03.md:4:{k}-4:{n}"));
        assert_ne!(field(q, "id"), field(h, "id"));
        assert_eq!(field(q, "object_id"), "ch1:exercise:2");
        assert_eq!(field(h, "object_id"), "ch1:exercise:2:hint");
        assert_eq!(field(h, "destination"), "exercise:ch1:exercise:2");
        let n3 = line_of(&b, "s03.md", 3).chars().count();
        assert_eq!(field(leaf(&p, &format!("s03.md:3:0-3:{n3}")), "object_id"), "ch1:exercise:1", "the exercise block splits at its spans");
    }

    // --- 2. identity and joining ------------------------------------------------------------------

    #[test]
    fn a_deferred_proof_in_another_packet_joins_its_theorem_and_exercise_1_19_stays_distinct() {
        // The exercise prints the label of the theorem, as Rudin's exercises do in the OCR (`1.19.`).
        let mut b = book();
        b[2].lines[4] = "1.19. 设 a 与 b 属于 k 维空间，求所有到 a 的距离为到 b 距离两倍的点。".into();
        let mut c = packet_c(&b);
        c["fragments"].as_array_mut().unwrap().push(json!({"role": "object", "kind": "exercise", "label": "1.19", "name": "同号习题", "spans": [sp("s03.md", 5, 5, "1.19. 设 a")]}));
        c["fragments"].as_array_mut().unwrap().remove(3);
        // The dependent packet comes first: the statement pass still runs before the binding pass.
        let p = jn(&[pb(packet_b()), pa(packet_a()), pc(c)], &b, None);
        assert!(codes(&p).is_empty(), "{}", fault_text(&p));
        let t = object(&p, "ch1:theorem:1.19");
        assert_eq!(field(&t["proof"], "status"), "deferred_in_slice");
        let labels: Vec<String> = t["proof"]["steps"].as_array().unwrap().iter().map(|s| field(s, "label")).collect();
        assert_eq!(labels, (1..=9).map(|k| format!("第 {k} 步")).collect::<Vec<_>>());
        assert_eq!(field(t, "destination"), "unit:ch1:theorem:1.19");
        let e = object(&p, "ch1:exercise:1.19");
        assert_eq!(field(e, "destination"), "exercise:ch1:exercise:1.19");
        assert_eq!(field(leaf(&p, "s01.md:9-9"), "object_id"), "ch1:theorem:1.20", "the theorems between statement and proof keep their own claims");
        assert_eq!(field(leaf(&p, "s02.md:21-21"), "object_id"), "ch1:theorem:1.19");
        assert_eq!(field(leaf(&p, "s02.md:21-21"), "destination"), "unit:ch1:theorem:1.19");
    }

    #[test]
    fn a_proof_with_no_key_evidence_stays_unresolved_and_blocks() {
        let b = book();
        let mut bb = packet_b();
        bb["fragments"][0]["evidence"] = json!({"section": "s02.md", "line": 5, "quote": "第 1 步"});
        let p = jn(&[pa(packet_a()), pb(bb), pc(packet_c(&b))], &b, None);
        let c = codes(&p);
        assert!(c.contains(&"no_key_evidence".to_string()), "{}", fault_text(&p));
        assert!(c.contains(&"proof_missing".to_string()), "{}", fault_text(&p));
        assert!(c.contains(&"unclaimed".to_string()), "the Appendix is nobody's");
    }

    #[test]
    fn an_unnumbered_item_gets_an_ordinal_after_the_last_printed_label() {
        let b = book();
        let p = jn(&answers(&b), &b, None);
        let o = object(&p, "ch1:corollary:after-1.21-1");
        assert_eq!(o["key"], json!({"chapter": "ch1", "namespace": "corollary", "label": "after-1.21-1", "sublabel": ""}));
        assert_eq!(field(o, "destination"), "unit:ch1:corollary:after-1.21-1");
        assert_eq!(field(object(&p, "ch1:remark:1.22"), "destination"), "narrative:ch1");
        assert_eq!(object(&p, "ch1:theorem:1.21")["relations"][0]["status"], json!("resolved"));
    }

    // --- 3. relations and reconciliation ----------------------------------------------------------

    fn with_nested_example(contains: bool) -> (Vec<Section>, Vec<Packet>) {
        let b = vec![sec(
            "s01.md",
            "# 第 2 章 拓扑\n\n2.41 定理。欧氏空间的子集紧当且仅当它有界且闭，这里只讨论欧氏空间。\n\n证明。先证明闭区间套的交非空，再用它推出结论。\n\n2.42 例。取一列开区间覆盖单位区间，说明有限子覆盖的选取方法。\n\n回到证明：由例子中的构造，有界闭集的任一开覆盖都有有限子覆盖。",
        )];
        let mut t = json!({"role": "object", "kind": "theorem", "label": "2.41", "name": "海涅-博雷尔", "spans": [sp("s01.md", 3, 3, "2.41 定理")],
            "proof": {"status": "inline", "steps": [{"label": "", "spans": [sp("s01.md", 5, 9, "证明。先证明")]}]}});
        if contains {
            t["relations"] = json!([{"rel": "contains", "target": {"namespace": "example", "label": "2.42"}}]);
        }
        let a = json!({"schema": 2, "fragments": [t,
            {"role": "object", "kind": "example", "label": "2.42", "name": "开覆盖", "spans": [sp("s01.md", 7, 7, "2.42 例")], "destination": {"to": "unit", "target": {"namespace": "theorem", "label": "2.41"}},
             "relations": [{"rel": "illustrates", "target": {"namespace": "theorem", "label": "2.41"}}, {"rel": "cites", "target": {"namespace": "theorem", "label": "9.9"}, "raw": "定理 9.9"}]}
        ]});
        (b, vec![pa(a)])
    }

    #[test]
    fn a_nested_example_keeps_its_own_destination_inside_the_proof_that_contains_it() {
        let (b, a) = with_nested_example(true);
        let p = jn(&a, &b, None);
        assert!(codes(&p).is_empty(), "{}", fault_text(&p));
        assert_eq!(field(leaf(&p, "s01.md:7-7"), "object_id"), "ch2:example:2.42");
        assert_eq!(field(leaf(&p, "s01.md:9-9"), "object_id"), "ch2:theorem:2.41");
        let e = object(&p, "ch2:example:2.42");
        assert_eq!(field(e, "destination"), "unit:ch2:theorem:2.41");
        let cite = &e["relations"][1];
        assert_eq!(cite["status"], json!("unresolved"));
        assert!(!field(cite, "reason").is_empty(), "an unknown reference is kept with its reason");
        let (b, a) = with_nested_example(false);
        let p = jn(&a, &b, None);
        assert_eq!(codes(&p), ["double_claim"], "{}", fault_text(&p));
    }

    #[test]
    fn a_forward_reference_resolves_and_an_unresolved_illustration_blocks() {
        let b = book();
        let mut a = packet_a();
        a["fragments"][1]["relations"] = json!([{"rel": "cites", "target": {"namespace": "exercise", "label": "19"}, "raw": "习题 19"}]);
        a["fragments"][5]["relations"] = json!([{"rel": "illustrates", "target": {"namespace": "theorem", "label": "1.99"}}]);
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        assert_eq!(object(&p, "ch1:theorem:1.19")["relations"][0]["target"], json!("ch1:exercise:19"));
        assert_eq!(codes(&p), ["open_relation"], "{}", fault_text(&p));
    }

    fn expected_keys() -> Value {
        json!({"keys": [
            {"chapter": "ch1", "namespace": "theorem", "label": "1.19"}, {"chapter": "ch1", "namespace": "theorem", "label": "1.20"},
            {"chapter": "ch1", "namespace": "theorem", "label": "1.21"}, {"chapter": "ch1", "namespace": "remark", "label": "1.22"},
            {"chapter": "ch1", "namespace": "exercise", "label": "1"}, {"chapter": "ch1", "namespace": "exercise", "label": "2"}, {"chapter": "ch1", "namespace": "exercise", "label": "19"}],
            "unnumbered": [{"kind": "corollary", "section": "s01.md", "line": 17}]})
    }

    #[test]
    fn a_duplicate_cannot_mask_a_missing_theorem() {
        let b = book();
        let mut a = packet_a();
        // 1.20 is read as a second 1.19: the count of theorems is still three.
        a["fragments"][2]["label"] = json!("1.19");
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, Some(&expected_keys()));
        let c = codes(&p);
        assert!(c.contains(&"duplicate_key".to_string()), "{}", fault_text(&p));
        assert!(fault_text(&p).contains("expected_missing: ch1:theorem:1.20"), "{}", fault_text(&p));
        let p = jn(&answers(&b), &b, Some(&expected_keys()));
        assert!(codes(&p).is_empty(), "{}", fault_text(&p));
    }

    #[test]
    fn residual_prose_passes_only_when_declared() {
        let b = book();
        let p = jn(&answers(&b), &b, None);
        assert_eq!(field(leaf(&p, "s01.md:3-3"), "destination"), "narrative:ch1", "declared prose passes");
        let mut a = packet_a();
        a["fragments"].as_array_mut().unwrap().remove(0);
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        assert_eq!(codes(&p), ["unclaimed"], "silently omitted prose blocks: {}", fault_text(&p));
        assert!(fault_text(&p).contains("s01.md:3-3"));
    }

    #[test]
    fn an_omitted_unnumbered_corollary_blocks_although_every_number_reconciles() {
        let b = book();
        let mut a = packet_a();
        a["fragments"].as_array_mut().unwrap().remove(4);
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        let t = fault_text(&p);
        assert!(!codes(&p).contains(&"candidate_unmatched".to_string()), "every printed number has its object: {t}");
        assert!(t.contains("unclaimed: s01.md:17-17") && t.contains("unclaimed: s01.md:19-19"), "{t}");
        let mut a = packet_a();
        a["fragments"].as_array_mut().unwrap().remove(4);
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, Some(&expected_keys()));
        assert!(fault_text(&p).contains("expected_missing: corollary at s01.md:17"), "{}", fault_text(&p));
    }

    #[test]
    fn declared_prose_may_not_hide_a_printed_label() {
        let b = book();
        let mut a = packet_a();
        a["fragments"].as_array_mut().unwrap().remove(5);
        a["fragments"].as_array_mut().unwrap().push(json!({"role": "exposition", "spans": [sp("s01.md", 21, 21, "1.22 小数")]}));
        let p = jn(&[pa(a.clone()), pb(packet_b()), pc(packet_c(&b))], &b, None);
        assert!(codes(&p).contains(&"exposition_hides_label".to_string()), "{}", fault_text(&p));
        a["fragments"].as_array_mut().unwrap().push(json!({"role": "dismiss", "section": "s01.md", "line": 21, "reason": "a closing remark, not a numbered result"}));
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        assert!(codes(&p).is_empty(), "an approved dismissal is a disposition: {}", fault_text(&p));
    }

    #[test]
    fn a_proof_status_that_owes_steps_or_a_reason_blocks() {
        let b = book();
        let mut a = packet_a();
        a["fragments"][2]["proof"] = json!({"status": "external"});
        a["fragments"][3]["proof"].as_object_mut().unwrap().remove("status");
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        let t = fault_text(&p);
        assert!(t.contains("proof_reason: ch1:theorem:1.20"), "{t}");
        assert!(t.contains("no_proof_status: ch1:theorem:1.21"), "{t}");
    }

    // --- the packet self-check --------------------------------------------------------------------

    #[test]
    fn the_packet_self_check_passes_good_answers_and_names_each_fault() {
        let b = book();
        assert!(check_answer(&packet_a(), &b, &["s01.md"]).is_empty(), "{:?}", check_answer(&packet_a(), &b, &["s01.md"]));
        assert!(check_answer(&packet_b(), &b, &["s02.md"]).is_empty(), "{:?}", check_answer(&packet_b(), &b, &["s02.md"]));
        assert!(check_answer(&packet_c(&b), &b, &["s03.md"]).is_empty(), "{:?}", check_answer(&packet_c(&b), &b, &["s03.md"]));
        let bad = json!({"schema": 2, "fragments": [
            {"role": "object", "kind": "rule", "label": "", "spans": [sp("s01.md", 3, 3, "我们先回顾")]},
            {"role": "object", "kind": "theorem", "label": "1.19", "spans": [sp("s01.md", 5, 5, "不在这一行")], "proof": {"status": "inline"}},
            {"role": "object", "kind": "theorem", "label": "1.20", "spans": [sp("s01.md", 9, 99, "1.20 定理")], "proof": {"status": "external"}},
            {"role": "proof", "target": {"namespace": "theorem", "label": ""}, "steps": []},
            {"role": "exposition", "spans": [sp("s01.md", 3, 3, "我们先回顾"), sp("s01.md", 11, 11, "证明。设")]},
            {"role": "object", "kind": "theorem", "label": "1.21", "spans": [sp("s01.md", 13, 15, "1.21 定理")], "proof": {"status": "not_applicable"}},
            {"role": "object", "kind": "corollary", "label": "", "spans": [sp("s01.md", 15, 17, "证明。令 E")], "proof": {"status": "not_applicable"}}
        ]});
        let f = check_answer(&bad, &b, &["s01.md"]).join("\n");
        assert!(f.contains("fragment 1: kind `rule`"), "{f}");
        assert!(f.contains("fragment 2 (1.19): quote is not in the span s01.md:5-5"), "{f}");
        assert!(f.contains("fragment 3 (1.20): end 99"), "{f}");
        assert!(f.contains("fragment 4: a proof needs a target label"), "{f}");
        assert!(f.contains("fragment 5: an exposition declares one contiguous span"), "{f}");
        assert!(f.contains("fragments 6 and 7 overlap"), "{f}");
        assert_eq!(check_answer(&json!({"schema": 2}), &b, &["s01.md"]), ["the answer has no `fragments` array"]);
    }

    // --- 4. immutability --------------------------------------------------------------------------

    #[test]
    fn allocation_binds_the_index_hash_and_never_rewrites_the_index() {
        let b = book();
        let p = jn(&answers(&b), &b, None);
        let before = projection_hash(&p);
        assert_eq!(field(&p, "hash"), before);
        let text = p.to_string();
        assert!(!text.contains("\"card\"") && !text.contains("\"writer\"") && !text.contains("\"job\""), "the index holds no writer binding");
        let alloc = allocate(&p).unwrap();
        assert_eq!(projection_hash(&p), before);
        assert_eq!(field(&alloc, "index_hash"), before);
        let ids: Vec<String> = alloc["units"].as_array().unwrap().iter().map(|u| field(u, "object_id")).collect();
        assert_eq!(ids, ["ch1:theorem:1.19", "ch1:theorem:1.20", "ch1:theorem:1.21", "ch1:corollary:after-1.21-1"]);
        assert_eq!(alloc["units"][0]["card"], Value::Null);
        // The same answers make the same projection.
        assert_eq!(projection_hash(&jn(&[pc(packet_c(&b)), pb(packet_b()), pa(packet_a())], &b, None)), before);
    }

    // --- round 13: the faults the first tests did not reach ---------------------------------------

    /// The book with its Appendix in two staged sections: the second opens with a running head that
    /// names the theorem, as a page break in the OCR may leave it.
    fn book_split() -> Vec<Section> {
        let b = book();
        let s02 = &b[1].lines;
        let mut second = vec!["（定理 1.19 的证明，续）".to_string(), String::new()];
        second.extend(s02[12..].iter().cloned());
        vec![b[0].clone(), sec("s02.md", &s02[..12].join("\n")), sec("s02b.md", &second.join("\n")), b[2].clone()]
    }

    #[test]
    fn a_proof_split_across_packets_joins_in_source_order() {
        let b = book_split();
        let steps = packet_b()["fragments"][0]["steps"].as_array().unwrap().clone();
        let first = json!({"schema": 2, "fragments": [{"role": "proof", "target": {"namespace": "theorem", "label": "1.19"}, "evidence": {"section": "s02.md", "line": 3, "quote": "定理 1.19 的证明"}, "steps": steps[..4].to_vec()}]});
        let mut rest: Vec<Value> = steps[4..].to_vec();
        for st in rest.iter_mut() {
            let line = st["spans"][0]["line"].as_u64().unwrap() - 10;
            st["spans"][0]["line"] = json!(line);
            st["spans"][0]["end"] = json!(line);
            st["spans"][0]["section"] = json!("s02b.md");
        }
        let mut second = json!({"schema": 2, "fragments": [
            {"role": "proof", "target": {"namespace": "theorem", "label": "1.19"}, "evidence": {"section": "s02b.md", "line": 1, "quote": "定理 1.19 的证明，续"}, "steps": rest}]});
        // The running head belongs to no step: without a claim it blocks.
        let p = jn(&[pk(second.clone(), &["s02b.md"]), pk(first.clone(), &["s02.md"]), pa(packet_a()), pk(packet_c(&b), &["s03.md"])], &b, None);
        assert!(fault_text(&p).contains("unclaimed: s02b.md:1-1"), "{}", fault_text(&p));
        second["fragments"].as_array_mut().unwrap().push(json!({"role": "object", "kind": "editorial", "label": "", "spans": [sp("s02b.md", 1, 1, "定理 1.19 的证明")]}));
        // The second packet arrives first; the steps still join in source order.
        let p = jn(&[pk(second, &["s02b.md"]), pk(first, &["s02.md"]), pa(packet_a()), pk(packet_c(&b), &["s03.md"])], &b, None);
        assert!(codes(&p).is_empty(), "{}", fault_text(&p));
        let labels: Vec<String> = object(&p, "ch1:theorem:1.19")["proof"]["steps"].as_array().unwrap().iter().map(|s| field(s, "label")).collect();
        assert_eq!(labels, (1..=9).map(|k| format!("第 {k} 步")).collect::<Vec<_>>());
    }

    #[test]
    fn a_result_owes_a_source_treatment_and_two_proofs_may_not_overlap() {
        let b = book();
        let mut a = packet_a();
        a["fragments"][2]["proof"] = json!({"status": "not_applicable"});
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        assert!(fault_text(&p).contains("proof_status_wrong: ch1:theorem:1.20"), "{}", fault_text(&p));
        // An inline step on the statement's own line overlaps the statement.
        let mut a = packet_a();
        a["fragments"][2]["proof"] = inline("inline", "s01.md", 9, "1.20 定理");
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        assert!(codes(&p).contains(&"proof_overlap".to_string()), "{}", fault_text(&p));
    }

    #[test]
    fn identity_matches_exact_kinds_and_every_printed_occurrence() {
        let b = book();
        // A remark labelled 1.22 does not satisfy an expected theorem 1.22.
        let exp = json!({"keys": [{"chapter": "ch1", "namespace": "theorem", "label": "1.22"}]});
        let p = jn(&answers(&b), &b, Some(&exp));
        assert!(fault_text(&p).contains("expected_missing: ch1:theorem:1.22"), "{}", fault_text(&p));
        // An explicit namespace that names another kind does not resolve.
        let mut bb = packet_b();
        bb["fragments"][0]["target"]["namespace"] = json!("lemma");
        let p = jn(&[pa(packet_a()), pb(bb), pc(packet_c(&b))], &b, None);
        assert!(codes(&p).contains(&"unresolved_target".to_string()), "{}", fault_text(&p));
        // A synthetic label stays whole: the corollary, not Theorem 1.21.
        let mut a = packet_a();
        a["fragments"][5]["relations"] = json!([{"rel": "cites", "target": {"namespace": "corollary", "label": "after-1.21-1"}}]);
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        assert_eq!(object(&p, "ch1:remark:1.22")["relations"][0]["target"], json!("ch1:corollary:after-1.21-1"));
        // A label the OCR printed twice needs a disposition for its second occurrence.
        let mut b2 = book();
        b2[0].lines.push(String::new());
        b2[0].lines.push("1.21 定理。对每个正实数 x 与正整数 n，恰有一个正实数 y 使得 y 的 n 次方等于 x。".into());
        let mut a = packet_a();
        a["fragments"].as_array_mut().unwrap().push(json!({"role": "object", "kind": "editorial", "label": "", "spans": [sp("s01.md", 23, 23, "1.21 定理")]}));
        let p = jn(&[pa(a.clone()), pb(packet_b()), pc(packet_c(&b2))], &b2, None);
        assert!(fault_text(&p).contains("candidate_unmatched: s01.md:23"), "{}", fault_text(&p));
        a["fragments"].as_array_mut().unwrap().push(json!({"role": "dismiss", "section": "s01.md", "line": 23, "reason": "the OCR printed 1.21 twice"}));
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b2))], &b2, None);
        assert!(codes(&p).is_empty(), "{}", fault_text(&p));
    }

    #[test]
    fn a_quote_must_lie_in_its_span_and_evidence_names_a_whole_label() {
        let b = book();
        let l4 = line_of(&b, "s03.md", 4);
        let k = l4.split("提示").next().unwrap().chars().count();
        let bad = json!({"schema": 2, "fragments": [
            {"role": "object", "kind": "exercise", "label": "2", "spans": [{"section": "s03.md", "line": 4, "end": 4, "end_col": k, "quote": "提示：仿照"}]}
        ]});
        let f = check_answer(&bad, &b, &["s03.md"]).join("\n");
        assert!(f.contains("fragment 1 (2): quote is not in the span s03.md:4-4"), "{f}");
        let ev = json!({"schema": 2, "fragments": [{"role": "proof", "target": {"namespace": "theorem", "label": "1.1"}, "evidence": {"section": "s02.md", "line": 3, "quote": "定理 1.19 的证明"}, "steps": [{"label": "", "spans": [sp("s02.md", 5, 5, "第 1 步 这一步")]}]}]});
        assert!(check_answer(&ev, &b, &["s02.md"]).join("\n").contains("names `1.1`"), "1.1 is not 1.19");
        assert_eq!(check_answer(&json!({"fragments": []}), &b, &["s01.md"]), ["the answer must declare \"schema\": 2"]);
        // A display span opens with `$$`: its quote may come from a later line of the span.
        let d = vec![sec("s01.md", "# 第 1 章\n\n$$\nx^{2} + y^{2} = 1\n$$")];
        let ok = json!({"schema": 2, "fragments": [{"role": "object", "kind": "notation", "label": "", "spans": [sp("s01.md", 3, 5, "x^{2}+y^{2}")], "destination": {"to": "narrative"}}]});
        assert!(check_answer(&ok, &d, &["s01.md"]).is_empty(), "{:?}", check_answer(&ok, &d, &["s01.md"]));
    }

    #[test]
    fn the_join_reads_each_packet_against_its_own_sections() {
        let b = book();
        let mut bb = packet_b();
        bb["fragments"][0]["evidence"] = json!({"section": "s01.md", "line": 5, "quote": "1.19 定理"});
        let p = jn(&[pa(packet_a()), pb(bb), pc(packet_c(&b))], &b, None);
        assert!(codes(&p).contains(&"no_key_evidence".to_string()), "evidence from another packet: {}", fault_text(&p));
        let u = &p["unresolved"][0];
        assert_eq!((field(u, "role"), u["steps"].as_array().map(|s| s.len())), ("proof".to_string(), Some(9)), "an unresolved fragment is kept with its spans");
        assert!(!field(u, "reason").is_empty());
        let p = jn(&[pa(packet_a()), pk(packet_b(), &["s03.md"]), pc(packet_c(&b))], &b, None);
        assert!(codes(&p).contains(&"bad_fragment".to_string()), "a span outside its packet: {}", fault_text(&p));
    }

    #[test]
    fn every_owner_a_leaf_names_is_an_object_and_cut_pieces_keep_their_leaf() {
        let b = book();
        let p = jn(&answers(&b), &b, None);
        let ids: Vec<String> = p["objects"].as_array().unwrap().iter().map(|o| field(o, "id")).collect();
        for l in p["leaves"].as_array().unwrap() {
            if let Some(o) = l["object_id"].as_str() {
                assert!(ids.iter().any(|i| i == o), "leaf {} names {o}, which is no object", field(l, "id"));
            }
        }
        assert_eq!(field(object(&p, "ch1:exposition:1"), "destination"), "narrative:ch1");
        let n3 = line_of(&b, "s03.md", 3).chars().count();
        assert_eq!(field(leaf(&p, &format!("s03.md:3:0-3:{n3}")), "leaf"), "s03.md:3-5");
        assert_eq!(field(leaf(&p, "s01.md:9-9"), "leaf"), "s01.md:9-9");
        let set = &p["exercise_sets"][0];
        assert_eq!(field(set, "chapter"), "ch1");
        assert_eq!(field(set, "heading"), "s03.md:1-1");
        assert_eq!(set["exercises"], json!(["ch1:exercise:1", "ch1:exercise:2", "ch1:exercise:19"]));
        assert_eq!(set["hints"], json!(["ch1:exercise:2:hint"]));
    }

    #[test]
    fn containment_needs_real_enclosure() {
        let (b, mut a) = with_nested_example(true);
        // The proof step now stops at the example's line, and the example runs past it.
        a[0].answer["fragments"][0]["proof"]["steps"][0]["spans"] = json!([sp("s01.md", 5, 7, "证明。先证明")]);
        a[0].answer["fragments"][1]["spans"] = json!([sp("s01.md", 7, 9, "2.42 例")]);
        let p = jn(&a, &b, None);
        assert!(codes(&p).contains(&"double_claim".to_string()), "{}", fault_text(&p));
        let f = check_answer(&a[0].answer, &b, &["s01.md"]).join("\n");
        assert!(f.contains("fragments 1 and 2 overlap"), "{f}");
    }

    #[test]
    fn allocation_refuses_an_unsettled_or_altered_projection_and_the_hash_ignores_packet_order() {
        let b = book();
        let mut a = packet_a();
        a["fragments"].as_array_mut().unwrap().remove(0);
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        assert!(allocate(&p).is_err(), "a projection with faults is not allocated");
        let mut p = jn(&answers(&b), &b, None);
        assert_eq!(p["source"]["edition"], json!(ED));
        p["objects"][1]["destination"] = json!("narrative:ch1");
        assert!(allocate(&p).is_err(), "a projection changed after its hash is not allocated");
        let with = |v: Value, file: &str, reason: &str| {
            let mut v = v;
            v["fragments"].as_array_mut().unwrap().push(json!({"role": "dismiss", "section": file, "line": 3, "reason": reason}));
            v
        };
        let h1 = projection_hash(&jn(&[pa(with(packet_a(), "s01.md", "one")), pb(packet_b()), pc(with(packet_c(&b), "s03.md", "three"))], &b, None));
        let h2 = projection_hash(&jn(&[pc(with(packet_c(&b), "s03.md", "three")), pb(packet_b()), pa(with(packet_a(), "s01.md", "one"))], &b, None));
        assert_eq!(h1, h2);
    }

    // --- round 14 -----------------------------------------------------------------------------------

    #[test]
    fn an_inline_proof_start_takes_its_continuation_from_another_packet() {
        let b = book_split();
        let steps = packet_b()["fragments"][0]["steps"].as_array().unwrap().clone();
        let mut a = packet_a();
        a["fragments"][1]["proof"] = json!({"status": "deferred_in_slice", "steps": steps[..4].to_vec()});
        let mut rest: Vec<Value> = steps[4..].to_vec();
        for st in rest.iter_mut() {
            let line = st["spans"][0]["line"].as_u64().unwrap() - 10;
            st["spans"][0] = json!({"section": "s02b.md", "line": line, "end": line, "quote": st["spans"][0]["quote"].clone()});
        }
        let second = json!({"schema": 2, "fragments": [
            {"role": "proof", "target": {"namespace": "theorem", "label": "1.19"}, "evidence": {"section": "s02b.md", "line": 1, "quote": "定理 1.19 的证明，续"}, "steps": rest},
            {"role": "object", "kind": "editorial", "label": "", "spans": [sp("s02b.md", 1, 1, "定理 1.19 的证明")]}]});
        let p = jn(&[pk(second, &["s02b.md"]), pk(a, &["s01.md", "s02.md"]), pk(packet_c(&b), &["s03.md"])], &b, None);
        assert!(codes(&p).is_empty(), "{}", fault_text(&p));
        assert_eq!(object(&p, "ch1:theorem:1.19")["proof"]["steps"].as_array().unwrap().len(), 9);
    }

    #[test]
    fn clipped_evidence_does_not_name_a_shorter_label() {
        let b = book();
        let ev = json!({"schema": 2, "fragments": [{"role": "proof", "target": {"namespace": "theorem", "label": "1.1"}, "evidence": {"section": "s02.md", "line": 3, "quote": "定理 1.1"}, "steps": [{"label": "", "spans": [sp("s02.md", 5, 5, "第 1 步 这一步")]}]}]});
        assert!(check_answer(&ev, &b, &["s02.md"]).join("\n").contains("names `1.1`"), "the source line says 1.19");
        let p = jn(&[pa(packet_a()), pb(json!({"schema": 2, "fragments": "none"})), pc(packet_c(&b))], &b, None);
        assert!(fault_text(&p).contains("bad_fragment: answer 2: the answer has no `fragments` array"), "{}", fault_text(&p));
    }

    #[test]
    fn printed_answers_and_hints_are_listed_apart() {
        let b = book();
        let p = jn(&answers(&b), &b, None);
        assert_eq!(p["exercise_sets"][0]["hints"], json!(["ch1:exercise:2:hint"]));
        assert_eq!(p["exercise_sets"][0]["answers"], json!([]));
    }

    #[test]
    fn containment_holds_only_when_the_whole_inner_object_is_enclosed() {
        let (mut b, mut a) = with_nested_example(true);
        b[0].lines.push(String::new());
        b[0].lines.push("例 2.42 的补充：这一构造对任意有界闭集都可以照样进行。".into());
        a[0].answer["fragments"][1]["spans"] = json!([sp("s01.md", 7, 7, "2.42 例"), sp("s01.md", 11, 11, "例 2.42 的补充")]);
        let p = jn(&a, &b, None);
        assert!(codes(&p).contains(&"double_claim".to_string()), "a second inner span outside the proof: {}", fault_text(&p));
    }

    #[test]
    fn the_hash_binds_the_faults_and_dismissals_may_not_conflict() {
        let b = book();
        let mut a = packet_a();
        a["fragments"].as_array_mut().unwrap().remove(0);
        let mut p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        p["faults"] = json!([]);
        assert!(allocate(&p).is_err(), "emptying the faults changes what the hash describes");
        let mut a = packet_a();
        a["fragments"].as_array_mut().unwrap().push(json!({"role": "dismiss", "section": "s01.md", "line": 21, "reason": "one"}));
        a["fragments"].as_array_mut().unwrap().push(json!({"role": "dismiss", "section": "s01.md", "line": 21, "reason": "two"}));
        let p = jn(&[pa(a), pb(packet_b()), pc(packet_c(&b))], &b, None);
        assert!(codes(&p).contains(&"dismissal_conflict".to_string()), "{}", fault_text(&p));
    }

    #[test]
    fn an_expected_key_needs_a_whole_locator_or_none_and_may_carry_a_disposition() {
        let b = book();
        let exp = json!({"keys": [{"chapter": "ch1", "namespace": "theorem", "label": "1.19", "section": "s01.md"}]});
        assert!(codes(&jn(&answers(&b), &b, Some(&exp))).contains(&"expected_bad".to_string()));
        let exp = json!({"keys": [{"chapter": "ch1", "namespace": "theorem", "label": "1.19", "section": "s01.md", "line": 9}]});
        assert!(codes(&jn(&answers(&b), &b, Some(&exp))).contains(&"expected_missing".to_string()), "1.19 starts on line 5");
        let exp = json!({"keys": [{"chapter": "ch1", "namespace": "theorem", "label": "1.99", "disposition": "printed in the second edition only"}]});
        assert!(codes(&jn(&answers(&b), &b, Some(&exp))).is_empty());
    }

    // --- round 15 -----------------------------------------------------------------------------------

    #[test]
    fn steps_given_on_the_statement_publish_in_source_order() {
        let b = book();
        let mut steps = packet_b()["fragments"][0]["steps"].as_array().unwrap().clone();
        steps.reverse();
        // The first step's two spans also come in reverse.
        let last = steps.len() - 1;
        let spans = steps[last]["spans"].as_array_mut().unwrap();
        spans.reverse();
        let mut a = packet_a();
        a["fragments"][1]["proof"] = json!({"status": "deferred_in_slice", "steps": steps});
        let p = jn(&[pk(a, &["s01.md", "s02.md"]), pc(packet_c(&b))], &b, None);
        assert!(codes(&p).is_empty(), "{}", fault_text(&p));
        let st = &object(&p, "ch1:theorem:1.19")["proof"]["steps"];
        let labels: Vec<String> = st.as_array().unwrap().iter().map(|s| field(s, "label")).collect();
        assert_eq!(labels, (1..=9).map(|k| format!("第 {k} 步")).collect::<Vec<_>>());
        assert_eq!(st[0]["spans"][0]["line"], json!(3));
    }

    #[test]
    fn a_dispositioned_key_still_needs_a_whole_locator() {
        let b = book();
        let exp = json!({"keys": [{"chapter": "ch1", "namespace": "theorem", "label": "1.99", "section": "s01.md", "disposition": "second edition only"}]});
        assert!(codes(&jn(&answers(&b), &b, Some(&exp))).contains(&"expected_bad".to_string()));
    }

    #[test]
    fn whitespace_inside_a_label_does_not_make_the_label() {
        // The OCR split a number: the line prints `1.1 9`, which is no `1.19`.
        let b = vec![sec("s01.md", "# 第 1 章\n\n本附录给出定理 1.1 9 的证明，分九步完成。\n\n第 1 步 这一步完成构造中的第一个部分。")];
        let ev = json!({"schema": 2, "fragments": [{"role": "proof", "target": {"namespace": "theorem", "label": "1.19"}, "evidence": {"section": "s01.md", "line": 3, "quote": "定理 1.19 的证明"}, "steps": [{"label": "", "spans": [sp("s01.md", 5, 5, "第 1 步 这一步")]}]}]});
        assert!(check_answer(&ev, &b, &["s01.md"]).join("\n").contains("names `1.19`"));
    }

    #[test]
    fn the_self_check_reads_chapters_from_the_whole_book() {
        // The packet holds only the second section: its chapter heading is in the first.
        let b = vec![
            sec("s01.md", "# 第 2 章 拓扑\n\n本章讨论紧性，以及它在欧氏空间里的刻画方法。"),
            sec("s02.md", "2.41 定理。欧氏空间的子集紧当且仅当它有界且闭，这里只讨论欧氏空间。\n\n证明。先证明闭区间套的交非空，再用它推出结论。\n\n2.42 例。取一列开区间覆盖单位区间，说明有限子覆盖的选取方法。\n\n回到证明：由例子中的构造，有界闭集的任一开覆盖都有有限子覆盖。"),
        ];
        let a = json!({"schema": 2, "fragments": [
            {"role": "object", "kind": "theorem", "label": "2.41", "spans": [sp("s02.md", 1, 1, "2.41 定理")],
             "proof": {"status": "inline", "steps": [{"label": "", "spans": [sp("s02.md", 3, 7, "证明。先证明")]}]},
             "relations": [{"rel": "contains", "target": {"chapter": "ch2", "namespace": "example", "label": "2.42"}}]},
            {"role": "object", "kind": "example", "label": "2.42", "spans": [sp("s02.md", 5, 5, "2.42 例")], "destination": {"to": "narrative"}}
        ]});
        assert!(check_answer(&a, &b, &["s02.md"]).is_empty(), "{:?}", check_answer(&a, &b, &["s02.md"]));
        let mut away = a.clone();
        away["fragments"][1]["spans"] = json!([sp("s01.md", 3, 3, "本章讨论紧性")]);
        let f = check_answer(&away, &b, &["s02.md"]).join("\n");
        assert!(f.contains("section `s01.md` is not a file of this job"), "reads stay inside the packet: {f}");
    }

    #[test]
    fn a_declared_containment_that_encloses_nothing_blocks() {
        let (b, mut a) = with_nested_example(true);
        // The proof stops before the example: the spans are disjoint, so no leaf is shared.
        a[0].answer["fragments"][0]["proof"]["steps"][0]["spans"] = json!([sp("s01.md", 5, 5, "证明。先证明")]);
        a[0].answer["fragments"].as_array_mut().unwrap().push(json!({"role": "exposition", "spans": [sp("s01.md", 9, 9, "回到证明")]}));
        let p = jn(&a, &b, None);
        assert!(codes(&p).contains(&"containment_not_enclosed".to_string()), "{}", fault_text(&p));
    }

    // --- M2.1 ---------------------------------------------------------------------------------------

    #[test]
    fn a_label_followed_by_a_space_and_another_number_is_still_the_label() {
        let b = vec![sec("s01.md", "# 第 1 章\n\n由定理 1.1 9 个点都在圆上，这是下面证明的出发点。\n\n第 1 步 这一步完成构造中的第一个部分。")];
        let ev = json!({"schema": 2, "fragments": [
            {"role": "proof", "target": {"namespace": "theorem", "label": "1.1"}, "evidence": {"section": "s01.md", "line": 3, "quote": "由定理 1.1"}, "steps": [{"label": "", "spans": [sp("s01.md", 3, 5, "由定理 1.1 9 个点")]}]}]});
        assert!(check_answer(&ev, &b, &["s01.md"]).is_empty(), "{:?}", check_answer(&ev, &b, &["s01.md"]));
    }

    #[test]
    fn the_self_check_lists_what_no_fragment_claims() {
        let b = book();
        let mut a = packet_a();
        a["fragments"].as_array_mut().unwrap().remove(0);
        let f = check_answer(&a, &b, &["s01.md"]);
        assert_eq!(f, ["s01.md:3-3 (paragraph) is claimed by no fragment: put it in an object, a proof step, a hint or answer, or an exposition"]);
        // Part of a line: the hint left out of the exercise packet.
        let mut c = packet_c(&b);
        c["fragments"].as_array_mut().unwrap().remove(2);
        let l4 = line_of(&b, "s03.md", 4);
        let (k, n) = (l4.split("提示").next().unwrap().chars().count(), l4.chars().count());
        assert_eq!(check_answer(&c, &b, &["s03.md"]), [format!("s03.md:4:{k}-4:{n} (list) is claimed by no fragment: put it in an object, a proof step, a hint or answer, or an exposition")]);
    }

    #[test]
    fn the_old_unit_records_are_a_view_of_the_projection() {
        let mut b = book();
        for (k, s) in b.iter_mut().enumerate() {
            s.group = k as i64 + 1;
            s.chapter = "第 1 章".into();
        }
        let p = jn(&answers(&b), &b, None);
        let (units, others) = legacy_view(&p, &b);
        let ids: Vec<String> = units.iter().map(|u| field(u, "id")).collect();
        assert_eq!(ids, ["定理 1.19", "定理 1.20", "定理 1.21", "推论「根的乘积」"]);
        assert_eq!(field(&units[0], "object_id"), "ch1:theorem:1.19");
        assert_eq!((field(&units[0], "chapter"), field(&units[0], "chapter_id")), ("第 1 章 实数系".to_string(), "ch1".to_string()), "the printed chapter, not the prepared group's");
        assert_eq!((field(&units[0], "form"), field(&units[0], "level")), ("theorem".to_string(), "formal".to_string()));
        assert_eq!((units[0]["section"].clone(), units[0]["line"].clone(), units[0]["end"].clone(), units[0]["group"].clone()), (json!("s01.md"), json!(5), json!(7), json!(1)));
        assert_eq!(units[2]["cites"], json!(["定理 1.20"]));
        assert_eq!(units[1]["proof"], json!(11));
        assert_eq!(units[0]["proof"], json!(0), "a proof in another section has no line in the statement's section");
        let oids: Vec<(String, String)> = others.iter().map(|o| (field(o, "id"), field(o, "kind"))).collect();
        assert_eq!(oids, [("习题 1".to_string(), "exercise".to_string()), ("习题 2".to_string(), "exercise".to_string()), ("习题 19".to_string(), "exercise".to_string())]);
        assert_eq!(others[0]["group"], json!(3));
        assert_eq!((field(&others[1], "after"), field(&others[1], "writer")), (String::new(), "group".to_string()), "an exercise is its chapter set's, not the unit before it");
        // The hint starts on its question's line: its columns travel with it.
        assert!(s(&others[1]["hints"][0]).starts_with("sections/s03.md:4-4（第 4 行第 "), "{}", others[1]["hints"]);
        assert_eq!(others[1]["answers"], json!([]));
    }

    #[test]
    fn an_unnumbered_nested_example_is_contained_by_its_name() {
        let (b, mut a) = with_nested_example(true);
        a[0].answer["fragments"][0]["relations"] = json!([{"rel": "contains", "target": {"namespace": "example", "name": "开覆盖"}}]);
        a[0].answer["fragments"][1]["label"] = json!("");
        a[0].answer["fragments"].as_array_mut().unwrap().push(json!({"role": "dismiss", "section": "s01.md", "line": 7, "reason": "the example is printed with a number this test drops"}));
        assert!(check_answer(&a[0].answer, &b, &["s01.md"]).is_empty(), "{:?}", check_answer(&a[0].answer, &b, &["s01.md"]));
        let p = jn(&a, &b, None);
        assert!(codes(&p).is_empty(), "{}", fault_text(&p));
        assert_eq!(field(leaf(&p, "s01.md:7-7"), "object_id"), "ch2:example:after-2.41-1");
        assert_eq!(object(&p, "ch2:theorem:2.41")["relations"][0]["target"], json!("ch2:example:after-2.41-1"));
    }

    #[test]
    fn a_numbered_section_heading_is_not_an_item() {
        // Tao numbers his sections (`## 1.3 完备性`) and his items (`**公理 1.3.3**`) alike.
        let b = vec![sec("s01.md", "# 第 1 章 实数\n\n## 1.3 完备性\n\n**公理 1.3.3** 每个有上界的非空集都有上确界。\n\n# 3.4 定理。\n\n(a) 有界数列有收敛的子列。")];
        let ls = leaves("s01.md", &b[0].lines);
        assert_eq!((ls[1].kind, ls[1].obligation), ("heading", false), "a section heading");
        assert_eq!((ls[3].kind, ls[3].obligation), ("heading", true), "an item the OCR made a heading");
        let c: Vec<String> = candidates(&b).into_iter().map(|x| x.label).collect();
        assert_eq!(c, ["3.4"]);
    }

    #[test]
    fn the_self_check_decides_what_its_packet_alone_can_decide() {
        let b = book();
        // A printed label nothing in the answer starts: the remark 1.22 left out, its line declared prose.
        let mut a = packet_a();
        a["fragments"].as_array_mut().unwrap().remove(5);
        a["fragments"].as_array_mut().unwrap().push(json!({"role": "exposition", "spans": [sp("s01.md", 21, 21, "1.22 小数")]}));
        let f = check_answer(&a, &b, &["s01.md"]).join("\n");
        assert!(f.contains("s01.md:21 prints 1.22 and no object of this answer starts there"), "{f}");
        assert!(f.contains("an exposition may not hold the printed label 1.22 (s01.md:21)"), "{f}");
        // A dismissal is the answer's own disposition of that line.
        a["fragments"].as_array_mut().unwrap().push(json!({"role": "dismiss", "section": "s01.md", "line": 21, "reason": "a closing remark"}));
        assert!(check_answer(&a, &b, &["s01.md"]).is_empty(), "{:?}", check_answer(&a, &b, &["s01.md"]));
        // Two objects with one key in one answer.
        let mut a = packet_a();
        a["fragments"][2]["label"] = json!("1.19");
        let f = check_answer(&a, &b, &["s01.md"]).join("\n");
        assert!(f.contains("fragments 2 and 3 both have the key ch1:theorem:1.19"), "{f}");
    }

    #[test]
    fn each_join_fault_names_the_finder_jobs_it_touches() {
        let b = book();
        let mut bb = packet_b();
        bb["fragments"][0]["target"]["namespace"] = json!("lemma");
        let p = jn(&[pa(packet_a()), pb(bb), pc(packet_c(&b))], &b, Some(&json!({"keys": [{"chapter": "ch1", "namespace": "theorem", "label": "1.99"}]})));
        let jobs_of = |code: &str| -> Vec<Value> { p["faults"].as_array().unwrap().iter().filter(|f| field(f, "code") == code).map(|f| f["jobs"].clone()).collect() };
        assert_eq!(jobs_of("unresolved_target"), [json!(["s02.md"])], "{}", fault_text(&p));
        assert_eq!(jobs_of("proof_missing"), [json!(["s01.md"])], "the theorem's own packet: {}", fault_text(&p));
        assert_eq!(jobs_of("expected_missing"), [json!(["s01.md", "s02.md", "s03.md"])], "a missing key: every packet of its chapter");
    }

    // --- M2.2 ---------------------------------------------------------------------------------------

    #[test]
    fn a_chapter_takes_the_heading_after_its_number_as_its_title() {
        let b = vec![sec("s01.md", "# 第1章\n\n# 实数系与复数系\n\n# 引言\n\n正文。\n\n# 第 2 章 基本拓扑\n\n正文二。")];
        let t: Vec<(String, String, String)> = chapters(&b).into_iter().map(|c| (c.id, c.label, c.title)).collect();
        assert_eq!(t, [("ch1".to_string(), "第1章".to_string(), "实数系与复数系".to_string()), ("ch2".to_string(), "第 2 章 基本拓扑".to_string(), String::new())]);
        assert_eq!(chapter_name(&chapters(&b)[0]), "第1章 实数系与复数系");
        assert_eq!(chapter_name(&chapters(&b)[1]), "第 2 章 基本拓扑");
    }
}
