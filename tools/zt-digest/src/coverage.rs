//! `zt-digest coverage`: the Literature notes' paragraph coverage (L2, #185; design
//! `docs/literature-notes-proposal.md` §3.3).
//!
//! A staging folder is split into paragraph units (each `sections/sNN.md`), the run's own source
//! logs and the Model plans give every Card's line spans, and the skips of the card layer are read
//! from `coverage-skips-gNN.json`. A unit with no covering span and no skip is fault C1; a span
//! that names only a file covers nothing and is fault C2; a number on a Literature note, Finding
//! or Event that its spans' text does not carry is the advisory C3; a skip whose reason is not
//! one of the five, or a `重复` that names no other unit, is fault C4.
//!
//! ```text
//! zt-digest coverage -Out <staging> [-Group N] [-Draft <file>] [-Step]
//! ```
//!
//! The command reads files only: it writes nothing but `coverage.json` with `-Step`, changes no
//! Card and no ledger row, and the same staging folder gives the same lines.

use crate::args::Args;
use crate::ps;
use crate::util;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// The five allowed skip reasons.
pub const REASONS: &[&str] = &["致谢", "参考文献", "目录", "版式", "重复"];

/// A paragraph unit of one staged section: a run of lines, 1-based and inclusive.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub section: String,
    pub start: usize,
    pub end: usize,
    pub text: String,
}

/// One source span of a Card: a section and, when the span is usable, its line range.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub section: String,
    /// 1-based first line; `None` when the span names only a file.
    pub start: Option<usize>,
    pub end: Option<usize>,
    /// The span as the log wrote it, for the C2 line.
    pub raw: String,
}

/// The spans of one covering source: a Card, a Model part or a group anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub address: String,
    pub spans: Vec<Span>,
}

/// A skip the card layer recorded, or one the draft proposes.
#[derive(Clone, Debug, PartialEq)]
pub struct Skip {
    pub span: Span,
    pub reason: String,
    /// The other unit a `重复` names.
    pub names: String,
}

static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}(\s|$)").unwrap());
static FIGURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^!\[[^\]]*\]\(artifact:[^)]*\)\s*$").unwrap());
static RANGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(.*):(\d+)(?:-(\d+))?$").unwrap());
static BARE_RANGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)(?:-(\d+))?$").unwrap());

/// The paragraph units of one section file: non-blank blocks of at least 20 non-whitespace
/// characters.
///
/// A heading line or a figure line (`![…](artifact:…)`) is not a unit. A display formula
/// (`$$…$$`) belongs to the paragraph before it and extends that paragraph's line range; with no
/// paragraph before it, it is no unit of its own. A table (consecutive `|` lines) is one unit.
pub fn units(section: &str, text: &str) -> Vec<Unit> {
    let lines: Vec<&str> = text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)).collect();
    let mut out: Vec<Unit> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() || HEADING.is_match(line) || FIGURE.is_match(line) {
            i += 1;
            continue;
        }
        if line.starts_with("$$") {
            let from = i;
            i += 1;
            // A one-line `$$…$$` is complete; a lone `$$` opens a block that its closing `$$` ends.
            if line == "$$" {
                while i < lines.len() {
                    let done = lines[i].trim().ends_with("$$");
                    i += 1;
                    if done {
                        break;
                    }
                }
            }
            if let Some(last) = out.last_mut()
                && lines[last.end..from].iter().all(|l| l.trim().is_empty())
            {
                last.end = i;
            }
            continue;
        }
        let start = i;
        if line.starts_with('|') {
            while i < lines.len() && lines[i].trim().starts_with('|') {
                i += 1;
            }
        } else {
            while i < lines.len() {
                let next = lines[i].trim();
                if next.is_empty()
                    || HEADING.is_match(next)
                    || FIGURE.is_match(next)
                    || next.starts_with("$$")
                    || next.starts_with('|')
                {
                    break;
                }
                i += 1;
            }
        }
        let joined = lines[start..i].join("\n");
        if joined.chars().filter(|c| !c.is_whitespace()).count() >= 20 {
            out.push(Unit { section: section.to_string(), start: start + 1, end: i, text: joined });
        }
    }
    out
}

/// The spans a source log or a Model plan writes: `sections/s05.md:120-160,171-175`, an absolute
/// path to a staged section, or a comma- or semicolon-separated list of them.
///
/// A span's section is its path's file name (`s05.md`), so a staged section is named by a relative
/// path, an absolute path or a `\\?\` path alike. A token with no line range is a file-only span.
pub fn parse_spans(raw: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut path: Option<String> = None;
    for piece in raw.split(';') {
        for token in piece.split(',') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            if let Some(c) = RANGE.captures(token) {
                let named = c[1].trim().to_string();
                let start: usize = c[2].parse().unwrap_or(0);
                let end: usize = c.get(3).map_or(start, |m| m.as_str().parse().unwrap_or(start));
                path = Some(named.clone());
                out.push(Span { section: file_name(&named), start: Some(start), end: Some(end), raw: token.to_string() });
            } else if BARE_RANGE.is_match(token) {
                let c = BARE_RANGE.captures(token).unwrap();
                let start: usize = c[1].parse().unwrap_or(0);
                let end: usize = c.get(2).map_or(start, |m| m.as_str().parse().unwrap_or(start));
                if let Some(named) = &path {
                    out.push(Span { section: file_name(named), start: Some(start), end: Some(end), raw: token.to_string() });
                }
            } else {
                path = Some(token.to_string());
                out.push(Span { section: file_name(token), start: None, end: None, raw: token.to_string() });
            }
        }
    }
    out
}

/// A path's last component, whichever separator the log used.
fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
}

/// One group of the prepared run: its number, id (`g01`) and section files.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub n: i64,
    pub id: String,
    pub sections: Vec<String>,
    pub anchor: String,
}

/// One written Card, Finding or Event as a snapshot holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct CardRecord {
    pub address: String,
    pub category: String,
    pub body: String,
}

/// Everything the report reads: the staged sections, the run's sources, the recorded skips and
/// the Cards whose numbers are checked.
#[derive(Clone, Debug, Default)]
pub struct Input {
    pub groups: Vec<Group>,
    /// Section file name (`s01.md`) and its text, in manifest order.
    pub sections: Vec<(String, String)>,
    pub sources: Vec<Source>,
    pub skips: Vec<Skip>,
    pub cards: Vec<CardRecord>,
    /// The scope: `Some(n)` limits the report to group `n`'s sections and Cards.
    pub group: Option<i64>,
}

/// One fault line: a location, a code and a message.
#[derive(Clone, Debug, PartialEq)]
pub struct Fault {
    pub section: String,
    pub start: usize,
    pub end: usize,
    pub code: &'static str,
    pub message: String,
    pub advisory: bool,
}

impl Fault {
    /// The line the command prints, e.g. `s01.md:120-128 C1 uncovered "…"`.
    pub fn line(&self) -> String {
        format!("{} {} {}{}", self.location(), self.code, self.message, if self.advisory { " (advisory)" } else { "" })
    }

    fn location(&self) -> String {
        if self.start == 0 {
            self.section.clone()
        } else if self.end > self.start {
            format!("{}:{}-{}", self.section, self.start, self.end)
        } else {
            format!("{}:{}", self.section, self.start)
        }
    }
}

/// One unit's outcome: who covers it, or what skip it carries.
#[derive(Clone, Debug, PartialEq)]
pub struct UnitReport {
    pub start: usize,
    pub end: usize,
    pub text: String,
    pub covered_by: Vec<String>,
    pub skip: Option<(String, String)>,
}

/// One section's outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct SectionReport {
    pub file: String,
    pub group: Option<i64>,
    pub units: usize,
    pub covered: usize,
    pub skipped: usize,
    pub uncovered: usize,
    pub detail: Vec<UnitReport>,
}

/// One Card's spans and its numbers that no span carries.
#[derive(Clone, Debug, PartialEq)]
pub struct CardReport {
    pub address: String,
    pub category: String,
    pub spans: Vec<String>,
    pub numbers: Vec<String>,
    pub missing: Vec<String>,
}

/// The whole coverage answer.
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    pub sections: Vec<SectionReport>,
    pub cards: Vec<CardReport>,
    pub faults: Vec<Fault>,
    pub units: usize,
    pub covered: usize,
    pub skipped: usize,
    pub uncovered: usize,
    pub faults_c1: usize,
    pub faults_c2: usize,
    pub faults_c3: usize,
    pub faults_c4: usize,
}

impl Coverage {
    /// The counted faults: C1, C2 and C4. C3 is advisory.
    pub fn counted(&self) -> usize {
        self.faults.iter().filter(|f| !f.advisory).count()
    }

    /// The covered share of the units, in whole percent, counting a skip as resolved: 372 covered
    /// and 6 skipped of 381 units is 99.
    pub fn pct(&self) -> u64 {
        if self.units == 0 {
            return 100;
        }
        (((self.covered + self.skipped) as f64 / self.units as f64) * 100.0).round() as u64
    }
}

/// The coverage of the input's sections.
pub fn report(input: &Input) -> Coverage {
    let mut out = Coverage::default();
    let groups: BTreeMap<String, i64> = input
        .groups
        .iter()
        .flat_map(|g| g.sections.iter().map(move |s| (s.clone(), g.n)))
        .collect();
    let text_of: BTreeMap<&str, &str> = input.sections.iter().map(|(name, text)| (name.as_str(), text.as_str())).collect();
    let in_scope = |file: &str| input.group.is_none_or(|wanted| groups.get(file) == Some(&wanted));
    let all_units: Vec<(String, Vec<Unit>)> = input
        .sections
        .iter()
        .filter(|(file, _)| in_scope(file))
        .map(|(file, text)| (file.clone(), units(file, text)))
        .collect();
    let mut faults = Vec::new();
    // C4: a skip reason must be one of the five, and `重复` must name another unit.
    for skip in &input.skips {
        let (Some(_), Some(_)) = (skip.span.start, skip.span.end) else {
            continue;
        };
        if !in_scope(&skip.span.section) || !text_of.contains_key(skip.span.section.as_str()) {
            continue;
        }
        if !REASONS.contains(&skip.reason.as_str()) {
            faults.push(Fault {
                section: skip.span.section.clone(),
                start: skip.span.start.unwrap_or(0),
                end: skip.span.end.unwrap_or(0),
                code: "C4",
                message: format!("skip reason {:?} is not allowed", skip.reason),
                advisory: false,
            });
            continue;
        }
        if skip.reason == "重复" {
            let own = skipped_unit(&all_units, skip);
            let names_another = parse_spans(&skip.names).iter().any(|named| {
                named.start.is_some()
                    && text_of.contains_key(named.section.as_str())
                    && all_units
                        .iter()
                        .find(|(file, _)| *file == named.section)
                        .is_some_and(|(_, units)| units.iter().any(|unit| overlaps(named, unit) && own != Some(unit_id(unit))))
            });
            if !names_another {
                faults.push(Fault {
                    section: skip.span.section.clone(),
                    start: skip.span.start.unwrap_or(0),
                    end: skip.span.end.unwrap_or(0),
                    code: "C4",
                    message: "重复 names no other unit".to_string(),
                    advisory: false,
                });
            }
        }
    }
    for (file, units) in &all_units {
        let group = groups.get(file).copied();
        let mut detail = Vec::new();
        for unit in units {
            let mut covering = BTreeSet::new();
            for source in &input.sources {
                if source.spans.iter().any(|span| overlaps(span, unit)) {
                    covering.insert(source.address.clone());
                }
            }
            let skip = input
                .skips
                .iter()
                .find(|skip| skip.span.section == *file && overlaps(&skip.span, unit))
                .map(|skip| (skip.reason.clone(), skip.names.clone()));
            let covered_by: Vec<String> = covering.into_iter().collect();
            let unit_report = UnitReport { start: unit.start, end: unit.end, text: unit.text.clone(), covered_by, skip: skip.clone() };
            if unit_report.covered_by.is_empty() && skip.is_none() {
                faults.push(Fault {
                    section: file.clone(),
                    start: unit.start,
                    end: unit.end,
                    code: "C1",
                    message: format!("uncovered {:?}", clip(&unit.text, 30)),
                    advisory: false,
                });
            }
            detail.push(unit_report);
        }
        let covered = detail.iter().filter(|u| !u.covered_by.is_empty()).count();
        let skipped = detail.iter().filter(|u| u.covered_by.is_empty() && u.skip.is_some()).count();
        let uncovered = detail.iter().filter(|u| u.covered_by.is_empty() && u.skip.is_none()).count();
        out.units += detail.len();
        out.covered += covered;
        out.skipped += skipped;
        out.uncovered += uncovered;
        out.sections.push(SectionReport { file: file.clone(), group, units: detail.len(), covered, skipped, uncovered, detail });
    }
    // C2: a span that names only a file covers nothing. Every such span is one fault, so the count
    // is the run's file-only span count, as the PRD measures it.
    for source in &input.sources {
        for span in &source.spans {
            if span.start.is_some() || !text_of.contains_key(span.section.as_str()) {
                continue;
            }
            faults.push(Fault {
                section: span.section.clone(),
                start: 0,
                end: 0,
                code: "C2",
                message: format!("file-only {}", source.address),
                advisory: false,
            });
        }
    }
    let order: BTreeMap<&str, usize> = input.sections.iter().enumerate().map(|(i, (name, _))| (name.as_str(), i)).collect();
    let (cards, mut advisories) = card_reports(input, &text_of);
    faults.append(&mut advisories);
    faults.sort_by(|a, b| {
        let section = |f: &Fault| order.get(f.section.as_str()).copied().unwrap_or(usize::MAX);
        section(a)
            .cmp(&section(b))
            .then(a.start.cmp(&b.start))
            .then(a.code.cmp(b.code))
            .then(a.section.cmp(&b.section))
            .then(a.message.cmp(&b.message))
    });
    out.faults_c1 = faults.iter().filter(|f| f.code == "C1").count();
    out.faults_c2 = faults.iter().filter(|f| f.code == "C2").count();
    out.faults_c3 = faults.iter().filter(|f| f.code == "C3").count();
    out.faults_c4 = faults.iter().filter(|f| f.code == "C4").count();
    out.faults = faults;
    out.cards = cards;
    out
}

/// The audit's coverage finding (#188): one prepared run's counts, its uncovered units and its C3
/// numbers, ready to be listed in `audit.json` and `audit.md`. `null` when the folder is no
/// prepared staging folder; `{status:"unavailable", reason}` when it is one but cannot be read.
///
/// The audit writes its own `audit-cards.json` first, so the numbers are checked against the
/// Cards this audit just read. An uncovered unit and a number that no source line states are
/// findings, not faults: no count here fails the audit or the run.
pub fn audit_findings(staging: &Path) -> Value {
    if !staging.join("manifest.json").is_file() {
        return Value::Null;
    }
    let run = match read_run(staging, None) {
        Ok(run) => run,
        Err(error) => {
            let reason: String = format!("{error:#}").chars().take(400).collect();
            return json!({"status": "unavailable", "reason": reason});
        }
    };
    let coverage = report(&run.input);
    let line = |start: usize, end: usize| if end > start { format!("{start}-{end}") } else { start.to_string() };
    let mut uncovered = Vec::new();
    for section in &coverage.sections {
        for unit in &section.detail {
            if unit.covered_by.is_empty() && unit.skip.is_none() {
                uncovered.push(json!({
                    "section": section.file,
                    "line": line(unit.start, unit.end),
                    "text": clip(&unit.text, 60),
                }));
            }
        }
    }
    let c3: Vec<Value> = coverage
        .cards
        .iter()
        .flat_map(|card| {
            card.missing
                .iter()
                .map(move |number| json!({"address": card.address, "number": number}))
        })
        .collect();
    json!({
        "status": "complete",
        "book": run.book,
        "units": coverage.units,
        "covered": coverage.covered,
        "skipped": coverage.skipped,
        "uncovered": coverage.uncovered,
        "pct": coverage.pct(),
        "faults": {
            "c1": coverage.faults_c1, "c2": coverage.faults_c2,
            "c3": coverage.faults_c3, "c4": coverage.faults_c4,
            "counted": coverage.counted(),
        },
        "uncovered_units": uncovered,
        "c3": c3,
        "card_source": {"path": run.snapshot_path, "cards": run.snapshot_cards},
    })
}

/// Whether a span covers a unit: a line range covers each unit it overlaps.
fn overlaps(span: &Span, unit: &Unit) -> bool {
    match (span.start, span.end) {
        (Some(start), Some(end)) => span.section == unit.section && start <= unit.end && end >= unit.start,
        _ => false,
    }
}

/// A unit's identity, for comparing a `重复` skip with the unit it names.
fn unit_id(unit: &Unit) -> (String, usize, usize) {
    (unit.section.clone(), unit.start, unit.end)
}

/// The unit one skip names, when it overlaps one.
fn skipped_unit(all: &[(String, Vec<Unit>)], skip: &Skip) -> Option<(String, usize, usize)> {
    let (_, units) = all.iter().find(|(file, _)| *file == skip.span.section)?;
    units.iter().find(|unit| overlaps(&skip.span, unit)).map(unit_id)
}

/// The first `max` characters of a text, white space folded, with `…` when it is longer.
pub fn clip(text: &str, max: usize) -> String {
    let folded = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if folded.chars().count() > max {
        format!("{}…", folded.chars().take(max).collect::<String>())
    } else {
        folded
    }
}

/// The C3 and span facts of every Literature note, Finding and Event.
fn card_reports(input: &Input, text_of: &BTreeMap<&str, &str>) -> (Vec<CardReport>, Vec<Fault>) {
    let groups: BTreeMap<String, i64> = input.groups.iter().flat_map(|g| g.sections.iter().map(move |s| (s.clone(), g.n))).collect();
    let in_scope = |file: &str| input.group.is_none_or(|wanted| groups.get(file) == Some(&wanted));
    let sections: BTreeMap<&str, Vec<&str>> = text_of
        .iter()
        .map(|(name, text)| (*name, text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)).collect()))
        .collect();
    let mut out = Vec::new();
    for card in &input.cards {
        let category = card.category.as_str();
        if !matches!(category, "idea" | "anchor" | "event" | "finding") {
            continue;
        }
        let spans: Vec<Span> = input.sources.iter().filter(|s| s.address == card.address).flat_map(|s| s.spans.clone()).collect();
        if spans.is_empty() {
            continue;
        }
        if !spans.iter().any(|span| in_scope(&span.section)) {
            continue;
        }
        let ranged: Vec<&Span> = spans.iter().filter(|span| span.start.is_some() && in_scope(&span.section)).collect();
        let mut text = String::new();
        for span in &ranged {
            let Some(lines) = sections.get(span.section.as_str()) else {
                continue;
            };
            let start = span.start.unwrap_or(1).max(1);
            let end = span.end.unwrap_or(start).min(lines.len());
            for line in lines.iter().take(end).skip(start - 1) {
                text.push_str(line);
                text.push('\n');
            }
        }
        let numbers = numbers_in(&card.body);
        let missing: Vec<String> = if ranged.is_empty() {
            Vec::new()
        } else {
            numbers.iter().filter(|n| !contains_number(&text, n)).cloned().collect()
        };
        out.push(CardReport {
            address: card.address.clone(),
            category: category.to_string(),
            spans: spans.iter().map(|span| match (span.start, span.end) {
                (Some(start), Some(end)) if end > start => format!("{}:{start}-{end}", span.section),
                (Some(start), _) => format!("{}:{start}", span.section),
                _ => span.section.clone(),
            }).collect(),
            numbers,
            missing,
        });
    }
    let faults: Vec<Fault> = out
        .iter()
        .flat_map(|card| {
            card.missing.iter().map(move |number| Fault {
                section: card.address.clone(),
                start: 0,
                end: 0,
                code: "C3",
                message: format!("number {:?}", number),
                advisory: true,
            })
        })
        .collect();
    out.sort_by(|a, b| a.address.cmp(&b.address));
    (out, faults)
}


static FENCED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)```.*?```").unwrap());
static DISPLAY_MATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)\$\$.*?\$\$").unwrap());
static INLINE_MATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$[^$\n]+\$").unwrap());
static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?:<|\(|href="|src=")https?://[^\s)>"\]]+"#).unwrap());
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9０-９]+(?:\.[0-9０-９]+)?").unwrap());
/// An artifact reference (`artifact:<sha256>`) is an address, not a statement.
static ARTIFACT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"artifact:[0-9a-fA-F]+").unwrap());
/// A scholar locator (`scholar:<file id>`), likewise.
static SCHOLAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"scholar:[0-9]+").unwrap());
/// A Markdown link target keeps its visible text only.
static LINK_TARGET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\]\([^)]*\)").unwrap());
/// A Link keeps its label only (`[[address|label]]` is `label`, `[[address]]` is empty).
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[[^\]]*\]\]").unwrap());

/// The numbers a text states outside formulas: integers, decimals, percentages and years.
///
/// Formula spans (`$…$`, `$$…$$`), code fences and URLs are not statements of the prose, so a
/// number inside one is not collected. Full-width digits are read as digits, and a thousands
/// separator between digits is dropped.
pub fn numbers_in(text: &str) -> Vec<String> {
    let mut body = text.to_string();
    for pattern in [&*FENCED, &*DISPLAY_MATH, &*INLINE_MATH, &*URL, &*ARTIFACT, &*SCHOLAR, &*LINK_TARGET, &*LINK] {
        body = pattern.replace_all(&body, " ").to_string();
    }
    let body = fold_digits(&body);
    let mut out: Vec<String> = Vec::new();
    for found in NUMBER.find_iter(&body) {
        let token = found.as_str();
        // A leading list ordinal (`1.`, `2、`, `3）`) numbers a list; it states no quantity.
        if is_list_ordinal(&body, found.start(), found.end()) {
            continue;
        }
        if !out.iter().any(|seen| seen == token) {
            out.push(token.to_string());
        }
    }
    out
}

/// Whether a number is a list ordinal: nothing but white space before it on its line, and a list
/// mark right after it.
fn is_list_ordinal(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].rsplit('\n').next().unwrap_or_default();
    let after = text[end..].chars().next();
    before.trim().is_empty() && matches!(after, Some('.' | '．' | '、' | ')' | '）'))
}

/// Full-width digits to ASCII digits.
fn fold_digits(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
            other => other,
        })
        .collect()
}

/// One number's normalized form: digits, one decimal point, and no thousands separator.
fn normalized(number: &str) -> String {
    number.replace([',', '，'], "")
}

/// Whether a text states a number: the token as a whole number, never a stretch of a longer one.
pub fn contains_number(text: &str, number: &str) -> bool {
    let number = normalized(&fold_digits(number));
    let haystack = comma_folded(&fold_digits(text));
    let needle: Vec<char> = number.chars().collect();
    if needle.is_empty() {
        return false;
    }
    let hay: Vec<char> = haystack.chars().collect();
    let mut at = 0;
    while at + needle.len() <= hay.len() {
        let window = &hay[at..at + needle.len()];
        let before_ok = at == 0 || !hay[at - 1].is_ascii_digit() && hay[at - 1] != '.';
        let end = at + needle.len();
        let after_ok = end == hay.len() || !hay[end].is_ascii_digit() && hay[end] != '.';
        if window == needle.as_slice() && before_ok && after_ok {
            return true;
        }
        at += 1;
    }
    false
}

/// A thousands separator between two digits is dropped, so `1,200` states `1200`. A comma is a
/// separator only before exactly three digits (`1,200`), so `2，3` stays two numbers.
fn comma_folded(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        if matches!(c, ',' | '，') && i > 0 && chars[i - 1].is_ascii_digit() {
            let count = chars[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
            if count == 3 {
                continue;
            }
        }
        out.push(*c);
    }
    out
}

/// One prepared run as the coverage command reads it: the manifest's groups and the files the
/// report is built from. The card cycle (#187) reads the same way, so a plan can tell a job what
/// the run's own coverage measurement says.
pub struct Run {
    pub book: String,
    pub input: Input,
    /// The Card snapshot the numbers are read from, and how many Cards it holds.
    pub snapshot_path: String,
    pub snapshot_cards: usize,
}

/// The manifest's groups of a staging folder.
pub fn staging_groups(staging: &Path) -> Result<Vec<Group>> {
    let manifest_path = staging.join("manifest.json");
    if !manifest_path.is_file() {
        bail!("{} holds no manifest.json: coverage reads a prepared staging folder", staging.display());
    }
    let manifest = util::read_json(&manifest_path)?;
    let groups = read_groups(&manifest)?;
    if groups.is_empty() {
        bail!("manifest.json names no group");
    }
    Ok(groups)
}

/// The staged sections, source logs, recorded skips and Cards of one prepared run.
pub fn read_run(staging: &Path, group: Option<i64>) -> Result<Run> {
    let manifest_path = staging.join("manifest.json");
    if !manifest_path.is_file() {
        bail!("{} holds no manifest.json: coverage reads a prepared staging folder", staging.display());
    }
    let manifest = util::read_json(&manifest_path)?;
    let book = ps::s(manifest.get("citation_key").unwrap_or(&Value::Null));
    let groups = read_groups(&manifest)?;
    if groups.is_empty() {
        bail!("manifest.json names no group");
    }
    let sections = read_sections(staging, &groups, group)?;
    let sources = read_sources(staging, &groups, group, &book)?;
    let skips = read_skips(staging, &groups, group)?;
    let snapshot = read_snapshot(staging)?;
    let snapshot_cards = snapshot.cards.len();
    Ok(Run {
        book,
        input: Input { groups, sections, sources, skips, cards: snapshot.cards, group },
        snapshot_path: snapshot.path,
        snapshot_cards,
    })
}

/// `zt-digest coverage`: the command line.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["Out", "Group", "Draft"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &["Step"])?;
    let out = a.required("Out")?;
    let staging = PathBuf::from(&out);
    let group = if a.has("Group") { Some(a.int_or("Group", 0)?) } else { None };
    let mut run = read_run(&staging, group)?;
    if let Some(wanted) = group
        && !run.input.groups.iter().any(|g| g.n == wanted)
    {
        bail!("-Group {wanted} names no group of this run");
    }
    let mut draft_record = Value::Null;
    if a.has("Draft") {
        let path = a.required("Draft")?;
        let draft = util::read_json(&path)?;
        let (adds, add_sources, draft_skips, record) = read_draft(&path, &draft)?;
        run.input.cards.extend(adds);
        run.input.sources.extend(add_sources);
        run.input.skips.extend(draft_skips);
        draft_record = record;
    }
    finish(&run, &staging, group, &draft_record, a.flag("Step"))
}

fn finish(run: &Run, staging: &Path, group: Option<i64>, draft: &Value, step: bool) -> Result<i32> {
    let input = &run.input;
    let book = run.book.as_str();
    let snapshot_path = run.snapshot_path.as_str();
    let snapshot_cards = run.snapshot_cards;
    let coverage = report(input);
    if step {
        let detail = staging.join("coverage.json");
        let record = coverage_json(&coverage, staging, group, book, snapshot_path, snapshot_cards, draft);
        util::write_atomic(&detail, &format!("{}\n", serde_json::to_string_pretty(&record)?))?;
        util::print_line(&util::compact(&step_line(&coverage, &detail)))?;
        return Ok(0);
    }
    let mut lines: Vec<String> = coverage.faults.iter().map(Fault::line).collect();
    lines.push(format!("faults: {}", coverage.counted()));
    util::print_lines(&lines)?;
    Ok(if coverage.counted() == 0 { 0 } else { 1 })
}

/// The `-Step` line: at most 400 characters, whatever the project path is.
fn step_line(coverage: &Coverage, detail: &Path) -> Value {
    json!({
        "ok": true,
        "units": coverage.units,
        "covered": coverage.covered,
        "skipped": coverage.skipped,
        "uncovered": coverage.uncovered,
        "pct": coverage.pct(),
        "detail": detail.display().to_string(),
    })
}

/// `coverage.json`: the per-section and per-Card facts of the run.
fn coverage_json(coverage: &Coverage, staging: &Path, group: Option<i64>, book: &str, snapshot_path: &str, snapshot_cards: usize, draft: &Value) -> Value {
    let sections: Vec<Value> = coverage
        .sections
        .iter()
        .map(|section| {
            json!({
                "file": section.file,
                "group": section.group,
                "units": section.units,
                "covered": section.covered,
                "skipped": section.skipped,
                "uncovered": section.uncovered,
                "spans": section.detail.iter().map(|unit| json!({
                    "line": if unit.end > unit.start { format!("{}-{}", unit.start, unit.end) } else { unit.start.to_string() },
                    "text": clip(&unit.text, 60),
                    "covered_by": unit.covered_by,
                    "skip": unit.skip.as_ref().map(|(reason, names)| json!({"reason": reason, "names": names})),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let cards: Vec<Value> = coverage
        .cards
        .iter()
        .map(|card| json!({"address": card.address, "category": card.category, "spans": card.spans, "numbers": card.numbers, "c3": card.missing}))
        .collect();
    json!({
        "schema_version": 1,
        "out": staging.display().to_string(),
        "book": book,
        "group": group,
        "units": coverage.units,
        "covered": coverage.covered,
        "skipped": coverage.skipped,
        "uncovered": coverage.uncovered,
        "pct": coverage.pct(),
        "faults": {"c1": coverage.faults_c1, "c2": coverage.faults_c2, "c3": coverage.faults_c3, "c4": coverage.faults_c4, "counted": coverage.counted()},
        "sections": sections,
        "cards": cards,
        "card_source": {"path": snapshot_path, "cards": snapshot_cards},
        "draft": draft,
    })
}

/// The manifest's groups.
fn read_groups(manifest: &Value) -> Result<Vec<Group>> {
    let mut out = Vec::new();
    for group in ps::ps_items(&manifest["groups"]) {
        let n = ps::int(&group["n"])?;
        let sections: Vec<String> = ps::list(&group["sections"]).into_iter().map(ps::s).collect();
        out.push(Group { n, id: ps::field(&group, "id"), sections, anchor: ps::field(&group, "anchor") });
    }
    Ok(out)
}

/// The staged section files of the groups in scope, in manifest order.
fn read_sections(staging: &Path, groups: &[Group], group: Option<i64>) -> Result<Vec<(String, String)>> {
    let mut out: Vec<(String, String)> = Vec::new();
    for g in groups.iter().filter(|g| group.is_none_or(|wanted| g.n == wanted)) {
        for name in &g.sections {
            if out.iter().any(|(seen, _)| seen == name) {
                continue;
            }
            let path = staging.join("sections").join(name);
            let text = util::read_text(&path).with_context(|| format!("missing staged section {}", path.display()))?;
            out.push((name.clone(), text));
        }
    }
    Ok(out)
}

/// The run's source logs and Model plans, as one source per address.
fn read_sources(staging: &Path, groups: &[Group], group: Option<i64>, book: &str) -> Result<Vec<Source>> {
    let mut out: Vec<Source> = Vec::new();
    let mut add = |address: String, raw: &str| {
        let spans = parse_spans(raw);
        if spans.is_empty() {
            return;
        }
        match out.iter_mut().find(|source| source.address == address) {
            Some(source) => source.spans.extend(spans),
            None => out.push(Source { address, spans }),
        }
    };
    for g in groups.iter().filter(|g| group.is_none_or(|wanted| g.n == wanted)) {
        let log = staging.join(format!("src-{}.txt", g.id));
        if log.is_file() {
            for line in util::read_text(&log)?.lines() {
                if let Some((address, src)) = line.split_once('\t') {
                    add(address.trim().to_string(), src.trim());
                }
            }
        }
        for (prefix, plan_name) in [("events", "events"), ("findings", "findings")] {
            let log = staging.join(format!("{prefix}-src-{}.txt", g.id));
            if !log.is_file() {
                continue;
            }
            let plan = read_layer_plan(staging, plan_name, &g.id)?;
            for line in util::read_text(&log)?.lines() {
                let Some((address, candidate)) = line.split_once('\t') else {
                    continue;
                };
                let Some(src) = plan.get(candidate.trim()) else {
                    continue;
                };
                add(address.trim().to_string(), src);
            }
        }
    }
    // Every Model plan of the run: each part names its own source lines.
    let models = staging.join("models");
    if models.is_dir() {
        for plan in model_plans(&models)? {
            let model = ps::field(&plan, "model");
            for part in ps::list(&plan["parts"]) {
                let id = ps::field(part, "id");
                let src: Vec<String> = ps::list(&part["src"]).into_iter().map(ps::s).collect();
                let address = format!("{model}#{id}");
                for raw in src {
                    add(address.clone(), &raw);
                }
            }
        }
    }
    let _ = book;
    Ok(out)
}

/// One layer's candidate id to source line map: `events/plan.json`'s `candidates` or the group's
/// `events-list-gNN.json`. A missing plan leaves the layer's spans unknown.
fn read_layer_plan(staging: &Path, layer: &str, id: &str) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let plan_path = staging.join(layer).join("plan.json");
    if plan_path.is_file() {
        let plan = util::read_json(&plan_path)?;
        if let Some(candidates) = plan["candidates"].as_object() {
            for list in candidates.values() {
                for candidate in ps::ps_items(list) {
                    let key = ps::field(&candidate, "id");
                    if key.is_empty() {
                        continue;
                    }
                    out.insert(key, ps::field(&candidate, "src"));
                }
            }
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    let list_path = staging.join(format!("{layer}-list-{id}.json"));
    if list_path.is_file() {
        let list = util::read_json(&list_path)?;
        for candidate in ps::ps_items(&list["candidates"]) {
            let key = ps::field(&candidate, "id");
            if key.is_empty() {
                continue;
            }
            out.insert(key, ps::field(&candidate, "src"));
        }
    }
    Ok(out)
}

/// Every `plan.json` below the Models folder that carries parts.
fn model_plans(dir: &Path) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|name| name == "plan.json") {
                let Ok(value) = util::read_json(&path) else {
                    continue;
                };
                if value["parts"].is_array() && !ps::field(&value, "model").is_empty() {
                    out.push(value);
                }
            }
        }
    }
    out.sort_by_key(|plan| ps::field(plan, "model"));
    Ok(out)
}

/// The skips the card layer recorded, one file per group.
fn read_skips(staging: &Path, groups: &[Group], group: Option<i64>) -> Result<Vec<Skip>> {
    let mut out = Vec::new();
    for g in groups.iter().filter(|g| group.is_none_or(|wanted| g.n == wanted)) {
        let path = staging.join(format!("coverage-skips-{}.json", g.id));
        if !path.is_file() {
            continue;
        }
        let value = util::read_json(&path)?;
        let list = if value.is_array() { ps::ps_items(&value) } else { ps::ps_items(&value["skips"]) };
        for entry in list {
            if let Some(skip) = parse_skip(&entry) {
                out.push(skip);
            }
        }
    }
    Ok(out)
}

fn parse_skip(entry: &Value) -> Option<Skip> {
    let raw = ["src", "span", "unit"]
        .iter()
        .map(|key| ps::field(entry, key))
        .find(|value| !value.is_empty())?;
    let span = parse_spans(&raw).into_iter().next()?;
    let names = ["names", "other", "duplicate_of"].iter().map(|key| ps::field(entry, key)).find(|value| !value.is_empty()).unwrap_or_default();
    Some(Skip { span, reason: ps::field(entry, "reason"), names })
}

/// The run's Card snapshot: the audit's whole-book dump when it exists, else the cards layer's.
#[derive(Clone, Debug, Default)]
struct Snapshot {
    path: String,
    cards: Vec<CardRecord>,
}

fn read_snapshot(staging: &Path) -> Result<Snapshot> {
    let candidates = [staging.join("audit-cards.json"), staging.join("cards/after/audit-cards.json"), staging.join("cards/before/audit-cards.json")];
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let value = util::read_json(&path)?;
        let mut cards = Vec::new();
        for card in ps::ps_items(&value) {
            let address = ps::field(&card, "address");
            if address.is_empty() {
                continue;
            }
            let mut category = ps::field(&card, "category");
            if category.is_empty() {
                category = ps::field(&card, "type");
            }
            cards.push(CardRecord { address, category, body: ps::field(&card, "body") });
        }
        if !cards.is_empty() {
            return Ok(Snapshot { path: path.display().to_string(), cards });
        }
    }
    Ok(Snapshot::default())
}

/// A group or review draft: the Cards it adds, their spans, and the skips it proposes.
fn read_draft(path: &str, draft: &Value) -> Result<(Vec<CardRecord>, Vec<Source>, Vec<Skip>, Value)> {
    let mut cards = Vec::new();
    let mut sources = Vec::new();
    let mut skips = Vec::new();
    let mut index = 0usize;
    let calls = ps::ps_items(&draft["calls"]);
    for call in &calls {
        let tool = ps::field(call, "tool");
        if !tool.starts_with("create_") {
            continue;
        }
        let arguments = &call["arguments"];
        let body = ps::field(arguments, "body");
        if body.trim().is_empty() {
            continue;
        }
        index += 1;
        let address = format!("draft#{index}");
        let category = match ps::field(arguments, "type").as_str() {
            "" => "idea".to_string(),
            other => other.to_string(),
        };
        cards.push(CardRecord { address: address.clone(), category, body });
        let src = ps::field(arguments, "src");
        if !src.is_empty() {
            sources.push(Source { address: address.clone(), spans: parse_spans(&src) });
        }
    }
    for add in ps::ps_items(&draft["adds"]) {
        let body = ps::field(&add, "body");
        if body.trim().is_empty() {
            continue;
        }
        index += 1;
        let address = format!("draft#{index}");
        let category = match ps::field(&add, "category").as_str() {
            "" => "idea".to_string(),
            other => other.to_string(),
        };
        cards.push(CardRecord { address: address.clone(), category, body });
        let raw = ["span", "src", "unit"].iter().map(|key| ps::field(&add, key)).find(|value| !value.is_empty()).unwrap_or_default();
        if !raw.is_empty() {
            sources.push(Source { address: address.clone(), spans: parse_spans(&raw) });
        }
    }
    for entry in ps::ps_items(&draft["skips"]) {
        if let Some(skip) = parse_skip(&entry) {
            skips.push(skip);
        }
    }
    let record = json!({"path": path, "cards": cards.len(), "skips": skips.len()});
    Ok((cards, sources, skips, record))
}
#[cfg(test)]
mod tests {
    use super::*;

    fn lines(names: &[(&str, usize, usize)]) -> Vec<(String, usize, usize)> {
        names.iter().map(|(s, a, b)| (s.to_string(), *a, *b)).collect()
    }

    fn spans(units: &[Unit]) -> Vec<(String, usize, usize)> {
        lines(&units.iter().map(|u| (u.section.as_str(), u.start, u.end)).collect::<Vec<_>>())
    }

    fn parsed(raw: &str) -> Vec<(String, Option<usize>, Option<usize>)> {
        parse_spans(raw).into_iter().map(|s| (s.section, s.start, s.end)).collect()
    }

    #[test]
    fn a_heading_a_figure_and_a_short_block_are_not_units() {
        let text = [
            "# I. 一个标题行",                // 1 heading
            "",
            "这是第一段正文，足够二十个字以上，因此是一个段落单元。", // 3 unit
            "",
            "![图 1](artifact:abc)",
            "",
            "短块",                            // 7 too short
        ]
        .join("\n");
        let names = spans(&units("s01.md", &text));
        assert_eq!(names, lines(&[("s01.md", 3, 3)]));
        assert_eq!(units("s01.md", &text)[0].text, "这是第一段正文，足够二十个字以上，因此是一个段落单元。");
    }

    #[test]
    fn a_paragraph_of_several_lines_is_one_unit_and_keeps_its_lines() {
        let text = ["第一行开头，后面还有", "第二行的内容，合起来够二十个字。", "", "另一段也够二十个字以上的长度，单独算一个单元。"].join("\n");
        assert_eq!(spans(&units("s02.md", &text)), lines(&[("s02.md", 1, 2), ("s02.md", 4, 4)]));
    }

    #[test]
    fn a_display_formula_belongs_to_the_paragraph_before_it() {
        let text = ["这一段说明公式的含义，长度已经超过二十个字。", "", "$$", "a = b + c", "$$", "", "下一段也超过二十个字的长度，另算一个单元。"].join("\n");
        assert_eq!(spans(&units("s03.md", &text)), lines(&[("s03.md", 1, 5), ("s03.md", 7, 7)]));
    }

    #[test]
    fn a_table_is_one_unit() {
        let text = ["| 国家 | 数值 |", "| --- | --- |", "| 甲 | 12 |", "", "表后的说明文字也超过二十个字，单独算一个单元。"].join("\n");
        let units = units("s04.md", &text);
        assert_eq!(spans(&units), lines(&[("s04.md", 1, 3), ("s04.md", 5, 5)]));
        assert!(units[0].text.starts_with("| 国家 |"));
    }

    #[test]
    fn a_log_span_names_its_section_and_its_lines() {
        assert_eq!(
            parsed("sections/s05.md:120-160,171-175"),
            vec![("s05.md".into(), Some(120), Some(160)), ("s05.md".into(), Some(171), Some(175))]
        );
        // A single line, and a comma that starts the next path.
        assert_eq!(
            parsed("sections/s03.md:42, sections/s04.md:7"),
            vec![("s03.md".into(), Some(42), Some(42)), ("s04.md".into(), Some(7), Some(7))]
        );
    }

    #[test]
    fn an_absolute_or_prefixed_path_names_the_same_section() {
        assert_eq!(parsed(r"\\?\D:\zt-digest\b\sections\s01.md"), vec![("s01.md".into(), None, None)]);
        assert_eq!(parsed(r"C:\work\sections\s02.md:5-7"), vec![("s02.md".into(), Some(5), Some(7))]);
        // The run's own log shape: the sections, then the writer's note.
        assert_eq!(
            parsed(r"\\?\D:\b\sections\s03.md; \\?\D:\b\sections\s04.md; sections/s03.md:11-13,38-40"),
            vec![
                ("s03.md".into(), None, None),
                ("s04.md".into(), None, None),
                ("s03.md".into(), Some(11), Some(13)),
                ("s03.md".into(), Some(38), Some(40)),
            ]
        );
    }

    /// Ten short paragraphs, one per line, so a span's overlap is easy to read.
    fn section(name: &str, paragraphs: &[&str]) -> (String, String) {
        let mut text = String::new();
        for paragraph in paragraphs {
            text.push_str(paragraph);
            text.push_str("\n\n");
        }
        (name.to_string(), text)
    }

    fn group(n: i64, sections: &[&str]) -> Group {
        Group { n, id: format!("g{n:02}"), sections: sections.iter().map(|s| s.to_string()).collect(), anchor: format!("K/{n}") }
    }

    fn source(address: &str, raw: &str) -> Source {
        Source { address: address.to_string(), spans: parse_spans(raw) }
    }

    /// One section of ten two-line units: paragraph `i` is at line `2i+1`.
    fn ten_paragraphs() -> (String, String) {
        let paragraphs: Vec<String> = (1..=10).map(|i| format!("第 {i} 段的正文内容，长度超过二十个字，是一个段落单元。")).collect();
        let refs: Vec<&str> = paragraphs.iter().map(String::as_str).collect();
        section("s01.md", &refs)
    }

    fn setup() -> Input {
        let (name, text) = ten_paragraphs();
        Input { groups: vec![group(1, &["s01.md"])], sections: vec![(name, text)], ..Default::default() }
    }

    #[test]
    fn a_unit_is_covered_by_each_span_it_overlaps_and_uncovered_ones_are_c1() {
        let mut input = setup();
        input.sources = vec![source("K/1|a", "sections/s01.md:1-4"), source("K/1|b", "sections/s01.md:3-5")];
        let coverage = report(&input);
        assert_eq!((coverage.units, coverage.covered, coverage.skipped, coverage.uncovered), (10, 3, 0, 7));
        assert_eq!(coverage.pct(), 30);
        let detail = &coverage.sections[0].detail;
        assert_eq!(detail[0].covered_by, ["K/1|a"]);
        assert_eq!(detail[1].covered_by, ["K/1|a", "K/1|b"]);
        assert_eq!(detail[2].covered_by, ["K/1|b"]);
        assert!(detail[3].covered_by.is_empty());
        let c1: Vec<String> = coverage.faults.iter().filter(|f| f.code == "C1").map(Fault::line).collect();
        assert_eq!(c1[0], "s01.md:7 C1 uncovered \"第 4 段的正文内容，长度超过二十个字，是一个段落单元。\"");
        assert_eq!(c1.len(), 7);
        assert_eq!(coverage.counted(), 7);
    }

    #[test]
    fn a_file_only_span_covers_nothing_and_is_c2() {
        let mut input = setup();
        input.sources = vec![
            source("K/1", r"\\?\D:\b\sections\s01.md"),
            source("K/1|a", r"\\?\D:\b\sections\s01.md; sections/s01.md:1-2"),
            source("K/1|b", "sections/other.md"),
        ];
        let coverage = report(&input);
        assert_eq!(coverage.covered, 1);
        let c2: Vec<String> = coverage.faults.iter().filter(|f| f.code == "C2").map(Fault::line).collect();
        assert_eq!(c2, ["s01.md C2 file-only K/1", "s01.md C2 file-only K/1|a"]);
        assert_eq!(coverage.faults_c2, 2);
        // A span naming a file this run has no section for covers nothing and faults nothing.
        assert!(!coverage.faults.iter().any(|f| f.message.contains("K/1|b")));
    }

    #[test]
    fn a_group_limits_the_sections_it_reports() {
        let (a, at) = ten_paragraphs();
        let (b, bt) = section("s02.md", &["另一节的第一段正文，长度也超过二十个字。", "另一节的第二段正文，长度也超过二十个字。"]);
        let mut input = Input { groups: vec![group(1, &["s01.md"]), group(2, &["s02.md"])], sections: vec![(a, at), (b, bt)], ..Default::default() };
        input.sources = vec![source("K/1|a", "sections/s01.md:1-20")];
        let all = report(&input);
        assert_eq!(all.units, 12);
        input.group = Some(1);
        let one = report(&input);
        assert_eq!(one.units, 10);
        assert_eq!(one.faults_c1, 0);
        input.group = Some(2);
        let two = report(&input);
        assert_eq!(two.units, 2);
        assert_eq!(two.faults_c1, 2);
        assert_eq!(two.sections[0].group, Some(2));
    }

    fn skip(section: &str, from: usize, to: usize, reason: &str, names: &str) -> Skip {
        Skip {
            span: Span { section: section.to_string(), start: Some(from), end: Some(to), raw: format!("{section}:{from}-{to}") },
            reason: reason.to_string(),
            names: names.to_string(),
        }
    }

    #[test]
    fn each_allowed_skip_reason_resolves_a_unit() {
        let mut input = setup();
        input.skips = vec![skip("s01.md", 1, 2, "致谢", ""), skip("s01.md", 3, 4, "参考文献", ""), skip("s01.md", 5, 6, "目录", ""), skip("s01.md", 7, 8, "版式", "")];
        let coverage = report(&input);
        assert_eq!((coverage.units, coverage.covered, coverage.skipped, coverage.uncovered), (10, 0, 4, 6));
        assert_eq!(coverage.faults_c4, 0);
        assert_eq!(coverage.sections[0].detail[0].skip, Some(("致谢".to_string(), String::new())));
        assert_eq!(coverage.pct(), 40);
    }

    #[test]
    fn a_repeated_unit_skip_must_name_another_unit() {
        let mut input = setup();
        input.skips = vec![skip("s01.md", 3, 4, "重复", "sections/s01.md:1-2")];
        assert_eq!(report(&input).faults_c4, 0);
        // The same unit, or a unit this run does not have, is no other unit.
        input.skips = vec![skip("s01.md", 3, 4, "重复", "sections/s01.md:3-4")];
        let same = report(&input);
        assert_eq!(same.faults_c4, 1);
        assert_eq!(
            same.faults.iter().find(|f| f.code == "C4").map(Fault::line).unwrap(),
            "s01.md:3-4 C4 重复 names no other unit"
        );
        input.skips = vec![skip("s01.md", 3, 4, "重复", "sections/s09.md:1-2")];
        assert_eq!(report(&input).faults_c4, 1);
        input.skips = vec![skip("s01.md", 3, 4, "重复", "")];
        assert_eq!(report(&input).faults_c4, 1);
    }

    #[test]
    fn a_number_the_spans_carry_is_checked_against_the_lines() {
        let (name, text) = section("s01.md", &["最富的 12 个国家人均 GDP 约为最穷的 11 个国家的 63 倍。"]);
        let mut input = Input {
            groups: vec![group(1, &["s01.md"])],
            sections: vec![(name, text)],
            sources: vec![source("K/1|b", "sections/s01.md:1")],
            ..Default::default()
        };
        input.cards = vec![CardRecord {
            address: "K/1|b".into(),
            category: "idea".into(),
            body: "最富的 12 个国家人均 GDP 约为最穷的 11 个国家的 63 倍。\n公式 $R^2 = 0.51$ 不算数字。\n来源：<https://doi.org/10.1257/aer.115.6.1709>".into(),
        }];
        let coverage = report(&input);
        assert_eq!(coverage.cards[0].numbers, ["12", "11", "63"]);
        assert!(coverage.cards[0].missing.is_empty());
        assert_eq!(coverage.faults_c3, 0);
        // A derived number that no line states is the advisory C3, and it counts nothing.
        input.cards[0].body = "前殖民地当前制度变异的 41% 可能源自殖民时期，第一阶段 R² 为 0.41。".into();
        let coverage = report(&input);
        assert_eq!(coverage.cards[0].missing, ["41", "0.41"]);
        assert_eq!(coverage.faults_c3, 2);
        assert_eq!(coverage.counted(), 0);
        let lines: Vec<String> = coverage.faults.iter().map(Fault::line).collect();
        assert_eq!(lines, ["K/1|b C3 number \"0.41\" (advisory)", "K/1|b C3 number \"41\" (advisory)"]);
    }

    #[test]
    fn a_draft_adds_its_cards_and_skips() {
        let mut input = setup();
        // The draft's first add covers the first two units; the skip carries the third.
        input.sources = vec![source("draft#1", "sections/s01.md:1-4")];
        input.skips = vec![skip("s01.md", 5, 6, "版式", "")];
        let coverage = report(&input);
        assert_eq!((coverage.covered, coverage.skipped, coverage.uncovered), (2, 1, 7));
    }

    #[test]
    fn the_audit_finding_lists_uncovered_units_and_c3_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("sections")).unwrap();
        let manifest = json!({"citation_key": "K", "groups": [{"n": 1, "id": "g01", "anchor": "K/1", "sections": ["s01.md"]}]});
        std::fs::write(root.join("manifest.json"), manifest.to_string()).unwrap();
        let text = [
            "第一段的正文，长度超过二十个字，这里没有数字。",
            "",
            "第二段的正文，长度超过二十个字，也没有数字。",
            "",
            "第三段的正文，长度超过二十个字，是一个段落单元。",
            "",
        ]
        .join("\n");
        std::fs::write(root.join("sections/s01.md"), text).unwrap();
        std::fs::write(root.join("src-g01.txt"), "K/1|a\tsections/s01.md:1\n").unwrap();
        std::fs::write(
            root.join("audit-cards.json"),
            json!([{"address": "K/1|a", "category": "idea", "body": "作者给出 63 倍的结果。"}]).to_string(),
        )
        .unwrap();
        let findings = audit_findings(root);
        assert_eq!(findings["status"], json!("complete"));
        assert_eq!(findings["book"], json!("K"));
        assert_eq!(findings["units"], json!(3));
        assert_eq!((findings["covered"].as_u64(), findings["uncovered"].as_u64()), (Some(1), Some(2)));
        assert_eq!(findings["pct"], json!(33));
        let uncovered = findings["uncovered_units"].as_array().unwrap();
        assert_eq!(uncovered.len(), 2);
        assert_eq!(uncovered[0]["section"], json!("s01.md"));
        assert_eq!(uncovered[0]["line"], json!("3"));
        assert!(uncovered[0]["text"].as_str().unwrap().contains("第二段"));
        assert_eq!(findings["c3"], json!([{"address": "K/1|a", "number": "63"}]));
        assert_eq!(findings["card_source"]["cards"], json!(1));
        // Not a prepared run: no coverage finding at all, not a zero.
        let plain = tempfile::tempdir().unwrap();
        assert_eq!(audit_findings(plain.path()), Value::Null);
        // A prepared run whose section cannot be read names why, never a successful count.
        std::fs::remove_file(root.join("sections/s01.md")).unwrap();
        let unavailable = audit_findings(root);
        assert_eq!(unavailable["status"], json!("unavailable"));
        assert!(unavailable["reason"].as_str().unwrap().contains("s01.md"));
    }

    #[test]
    fn the_step_line_stays_inside_four_hundred_characters() {
        let coverage = Coverage { units: 381, covered: 372, skipped: 6, uncovered: 3, ..Default::default() };
        // A staging path of 240 characters is already the Windows path budget; a run names ~40.
        let long = std::path::Path::new("D:\\").join("x".repeat(240)).join("coverage.json");
        let line = util::compact(&step_line(&coverage, &long));
        assert!(line.len() <= 400, "{} characters", line.len());
        assert!(line.starts_with("{\"ok\":true,\"units\":381,\"covered\":372,\"skipped\":6,\"uncovered\":3,\"pct\":99,"));
    }
    #[test]
    fn a_reason_outside_the_list_is_c4() {
        let mut input = setup();
        input.skips = vec![skip("s01.md", 3, 4, "背景", "")];
        let coverage = report(&input);
        assert_eq!(coverage.faults_c4, 1);
        assert_eq!(
            coverage.faults.iter().find(|f| f.code == "C4").map(Fault::line).unwrap(),
            "s01.md:3-4 C4 skip reason \"背景\" is not allowed"
        );
        assert_eq!(coverage.uncovered, 9, "an invalid skip resolves nothing");
        assert_eq!(coverage.counted(), 10);
    }
}
