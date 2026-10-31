//! `zt-digest cited`: the Cited list of a digest (#219, docs/cited-literature-cards-proposal.md).
//! The tool parses the reference list of the whole source, indexes the lines that cite each work,
//! and writes the Cited list Card (type `cited`, a Side successor of `<key>/1`, beside the Term
//! list). An entry the parser cannot split goes to one DeepSeek normalize job, whose rows are
//! accepted only when the first author's surname and the year stand in the raw entry.
//!
//!   zt-digest cited -Step plan|precheck|apply -Staging <folder> [-Heading <h>] [-Source <file>]
//!                   [-Draft <file>] [-Token <n>] [-Apply]
//!
//! -Source names the whole bilingual source when manifest.json's `source` is not it: an adopted staging
//! names the staged Chinese-only `book.zh.md`, which holds no English reference list (pilot 2026-10-31).
//! The plan freezes the file it read as `source`; `cited-cards` reads that file.
//!
//!   plan      reads manifest.json's `source` (the whole bilingual file) and `groups` (the section
//!             headings and their anchors), writes cited\plan.json (one row per entry: its id,
//!             authors, year, title, venue, display entry, the Literature tree it matches in the
//!             box, and the candidate lines that cite it) and, for the entries it could not split,
//!             cited\normalize\jobs.json. Writes no Card. Prints {"ok","rows","unparsed","jobs","file"}.
//!   precheck  checks one normalize answer (-Draft): one fault per line, then `faults: N`.
//!   apply     merges the accepted normalize answers, then with -Apply creates or updates the Cited
//!             list Card, keeping the `引用处`/`关系` cells of an unchanged row, and writes
//!             cited\apply.json (the ledger's evidence). Prints {"ok","rows","card","refused"}.

use crate::args::Args;
use crate::organize::common::{self, pstr};
use crate::ps;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::LazyLock;

/// A year in parentheses: `(2017)`, `(2003a)`.
static PAREN_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\((1[5-9]\d\d|20\d\d)([a-z]?)\)").unwrap());
/// A year between commas: `, 1998,` (the IMF style of Ebrill et al.).
static COMMA_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r",\s*(1[5-9]\d\d|20\d\d)([a-z]?)\s*[,.]").unwrap());
/// A year between periods: `Daron. 1998. “Title.”` (the Chicago style of the Nobel lecture, the JPE and
/// Becker et al.; live run of the parser over the seven live books, 2026-10-31).
static PERIOD_YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\.\s+(1[5-9]\d\d|20\d\d)([a-z]?)\.(?:\s|$)").unwrap());
/// The start of an entry: a capitalised name followed by a comma (`Jones, K.`, `Ab Iorwerth, Aled`).
static AUTHOR_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\p{Lu}[\p{L}'’\-]*(?: \p{Lu}[\p{L}'’\-]*)*,\s").unwrap());
/// The leading run of `——`, `____` or `---` that stands for the previous entry's authors.
static DASHES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^((?:—{1,3}|_{2,}|-{3,})\s*)+").unwrap());
/// The page markers an Econometrica reference list ends with: `[1374, 1391]`.
static PAGE_MARKERS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*\[[\d,\s\-–]+\]\s*$").unwrap());
static HAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}]").unwrap());

/// The headings that open a reference list.
const REFERENCE_HEADINGS: &[&str] = &[
    "references",
    "reference",
    "bibliography",
    "references and notes",
    "works cited",
    "参考文献",
    "参考",
    "文献",
];

/// The most candidate lines one work keeps.
const MAX_CANDIDATES: usize = 60;
/// The most characters of one candidate line the plan keeps.
const CANDIDATE_CHARS: usize = 240;
/// The most entries one normalize job holds.
const NORMALIZE_PER_JOB: usize = 40;

pub const MODEL: &str = "deepseek-official/deepseek-flash";
const NORMALIZE_INSTRUCTION: &str = "dsh-cited-normalize-instructions.zh.md";
const NORMALIZE_PROMPT: &str = "按以上说明拆分这些参考文献条目，写出 JSON 文件。最后只回复一行 JSON。";

struct Params {
    step: String,
    staging: String,
    heading: String,
    source: String,
    draft: String,
    token: String,
    apply: bool,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Step", "Staging", "Heading", "Source", "Draft", "Token"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &["Apply"])?;
    let p = Params {
        step: a.choice("Step", &["plan", "precheck", "apply"])?,
        staging: common::mandatory(&a, "Staging")?,
        heading: a.str("Heading"),
        source: a.str("Source"),
        draft: a.str("Draft"),
        token: a.str("Token"),
        apply: a.flag("Apply"),
    };
    let manifest = common::manifest(&p.staging)?;
    let key = pstr(&manifest, "citation_key");
    if key.is_empty() {
        bail!("the staging manifest names no citation_key");
    }
    let work = ps::join(&p.staging, "cited");
    std::fs::create_dir_all(&work)?;
    match p.step.as_str() {
        "precheck" => precheck_step(&p, &work),
        step => {
            let mut zt = crate::zt::start()?;
            if step == "plan" || p.apply {
                if let Some(refusal) = crate::lockcheck::preflight(&mut zt, "cited", step, &key)? {
                    return crate::lockcheck::stop("cited", &refusal);
                }
            }
            if step == "plan" {
                plan_step(&p, &manifest, &key, &work, &mut zt)
            } else {
                apply_step(&p, &manifest, &key, &work, &mut zt)
            }
        }
    }
}

// --- parsing -------------------------------------------------------------------------------------

/// One entry of a reference list, as the plan freezes it.
#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub row: usize,
    pub raw: String,
    pub entry: String,
    pub authors: String,
    pub surname: String,
    pub year: String,
    pub title: String,
    pub venue: String,
    pub parsed: bool,
    pub previous_authors: String,
}

/// The text of a heading line, without its `#` marks.
pub(crate) fn heading_text(line: &str) -> Option<String> {
    let trimmed = line.trim();
    trimmed
        .starts_with('#')
        .then(|| trimmed.trim_start_matches('#').trim().to_string())
}

fn is_reference_heading(text: &str, custom: &str) -> bool {
    let lower = text
        .trim_end_matches(['*', '\\', ':', '：'])
        .trim()
        .to_lowercase();
    REFERENCE_HEADINGS.contains(&lower.as_str()) || (!custom.is_empty() && lower == custom.trim().to_lowercase())
}

/// The source's lines, each marked when it stands in a reference list.
pub fn reference_lines(source: &str, custom: &str) -> Vec<(usize, String, bool)> {
    let mut inside = false;
    source
        .lines()
        .enumerate()
        .map(|(index, line)| {
            if let Some(text) = heading_text(line) {
                inside = is_reference_heading(&text, custom);
                return (index + 1, line.to_string(), false);
            }
            (index + 1, line.to_string(), inside)
        })
        .collect()
}

/// A raw reference line as text: the Markdown escapes and the line-break backslash undone, the
/// page markers cut.
fn clean(line: &str) -> String {
    let mut text = line.trim().trim_end_matches('\\').trim().to_string();
    for (from, to) in [("\\_", "_"), ("\\[", "["), ("\\]", "]"), ("\\*", "*"), ("\\&", "&")] {
        text = text.replace(from, to);
    }
    let text = text.trim_end_matches('\\').trim().to_string();
    PAGE_MARKERS.replace(&text, "").trim().to_string()
}

/// The year of an entry and the byte range of its match: the earliest of a parenthesised, a
/// period-delimited and a comma-delimited year.
fn year_of(text: &str) -> Option<(String, usize, usize)> {
    [&*PAREN_YEAR, &*PERIOD_YEAR, &*COMMA_YEAR]
        .iter()
        .filter_map(|pattern| pattern.captures(text))
        .min_by_key(|caps| caps.get(0).unwrap().start())
        .map(|caps| {
            let whole = caps.get(0).unwrap();
            (format!("{}{}", &caps[1], &caps[2]), whole.start(), whole.end())
        })
}

/// Whether a reference line is a translation of the list: more Han characters than Latin letters
/// (Hart 1995 repeats its whole list in Chinese under the same heading).
fn translated(line: &str) -> bool {
    let han = line.chars().filter(|c| HAN.is_match(&c.to_string())).count();
    let latin = line.chars().filter(char::is_ascii_alphabetic).count();
    han > latin
}

/// The first author of an author string: `Aghion, P., and Bolton, P.` → `Aghion, P.`.
fn first_author(authors: &str) -> String {
    let parts: Vec<&str> = authors.split(", ").collect();
    match parts.as_slice() {
        [one] => one.split(" and ").next().unwrap_or(one).trim().trim_end_matches(',').to_string(),
        [surname, given, ..] if !given.starts_with("and ") && !given.starts_with("AND ") => {
            let given = given.split(" and ").next().unwrap_or(given).split(" AND ").next().unwrap_or(given);
            format!("{surname}, {}", given.trim().trim_end_matches(','))
        }
        [surname, ..] => surname.to_string(),
        [] => String::new(),
    }
}

/// The first author's surname, in the case a text cites it (`ACEMOGLU` → `Acemoglu`).
fn surname_of(authors: &str) -> String {
    let first = authors
        .split(',')
        .next()
        .unwrap_or_default()
        .split(" and ")
        .next()
        .unwrap_or_default()
        .split(" AND ")
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    let letters: Vec<char> = first.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() > 1 && letters.iter().all(|c| c.is_uppercase()) {
        first
            .split(' ')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(head) => head.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        first
    }
}

/// The title and the venue after the year.
fn title_and_venue(rest: &str) -> (String, String) {
    let rest = rest.trim_start_matches([')', '.', ':', ',', ';', ' ']).trim();
    for (open, close) in [('‘', '’'), ('“', '”'), ('"', '"')] {
        if let Some(inner) = rest.strip_prefix(open) {
            if let Some(end) = inner.find(close) {
                let title = inner[..end].trim().trim_end_matches([',', '.']).to_string();
                let venue = inner[end + close.len_utf8()..]
                    .trim_start_matches(['.', ',', ';', ' '])
                    .trim()
                    .to_string();
                return (title, venue);
            }
        }
    }
    let end = [rest.find(". "), rest.find(" (")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(rest.len());
    let title = rest[..end].trim().trim_end_matches('.').to_string();
    let venue = rest[end..].trim_start_matches(['.', ' ']).trim().to_string();
    (title, venue)
}

/// ASCII folding for a reference id: `Custódio` → `custodio`.
pub(crate) fn fold(text: &str) -> String {
    text.chars()
        .filter_map(|c| {
            let base = match c {
                'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => 'a',
                'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
                'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' | 'Ø' => 'o',
                'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
                'ç' | 'Ç' => 'c',
                'ñ' | 'Ñ' => 'n',
                'ß' => 's',
                other => other,
            };
            base.is_ascii_alphanumeric().then(|| base.to_ascii_lowercase())
        })
        .collect()
}

/// Split one cleaned entry. `previous` is the entry before it, whose authors a dash run repeats.
fn split_entry(row: usize, raw: &str, previous: Option<&Entry>) -> Entry {
    let mut text = clean(raw);
    let mut previous_authors = String::new();
    if let Some(found) = DASHES.find(&text) {
        if let Some(prev) = previous.filter(|prev| !prev.authors.is_empty()) {
            // One dash run repeats the previous authors (`—— (1994)`, `——, 1980`, `——. 2002`); a run of
            // two or more (`—— ——`) repeats them all too. One run before `and X` repeats only the first
            // author, and one run before another name (`—— Gertner, R.`) starts with the first author.
            let runs = found.as_str().split_whitespace().count();
            let rest = text[found.end()..].trim_start().to_string();
            let joins = rest.starts_with("and ") || rest.starts_with("AND ") || rest.starts_with("& ");
            let names = rest.chars().next().is_some_and(char::is_uppercase);
            text = if runs == 1 && joins {
                format!("{} {rest}", first_author(&prev.authors))
            } else if runs == 1 && names {
                format!("{}, {rest}", first_author(&prev.authors))
            } else {
                let gap = if rest.starts_with([',', '.', ':']) { "" } else { " " };
                format!("{}{gap}{rest}", prev.authors)
            };
            previous_authors = prev.authors.clone();
        }
    }
    let mut entry = Entry { row, raw: raw.trim().to_string(), entry: text.clone(), previous_authors, ..Entry::default() };
    if let Some((year, start, end)) = year_of(&text) {
        // The QJE style quotes the title before the year (`Coles, Melvin G., and Eric Smith, “Title,” Economica,
        // LXIII (1996), 589–597.`): the authors end at the quote, and the title is the quoted text.
        let quote = text[..start].find(['“', '"']);
        let authors = match quote {
            Some(at) => text[..at].trim().trim_end_matches([',', ' ']).trim().to_string(),
            None => text[..start].trim().trim_end_matches([',', '(', ' ']).trim().to_string(),
        };
        if !authors.is_empty() {
            let (title, venue) = match quote {
                Some(at) => title_and_venue(&text[at..start]),
                None => title_and_venue(&text[end..]),
            };
            entry.surname = surname_of(&authors);
            entry.authors = authors;
            entry.year = year;
            entry.title = title;
            entry.venue = venue;
            entry.parsed = !entry.surname.is_empty() && !entry.title.is_empty();
        }
    }
    entry
}

/// Every entry of the source's reference lists, in order. A line in translation is no entry. A line
/// without a year that does not start like an entry (a dash run, or a name and a comma) continues
/// the entry before it: Hart wraps a long entry onto a line that starts in capitals, after a line
/// that ends with the same `\` as every entry.
pub fn entries(source: &str, custom: &str) -> Vec<Entry> {
    let lines = reference_lines(source, custom);
    // A list that holds both Latin entries and entries with Chinese in them is the original followed by
    // its translation (Grossman 2002 repeats its QJE list with Latin author names and Chinese titles): the
    // lines with Chinese in them are no entries. A list that is all Chinese is the book's own.
    let han = |line: &str| HAN.find_iter(line).count() >= 2;
    let listed: Vec<&String> = lines.iter().filter(|(_, l, inside)| *inside && !l.trim().is_empty()).map(|(_, l, _)| l).collect();
    let mixed = listed.iter().any(|l| han(l)) && listed.iter().any(|l| !han(l));
    let mut raws: Vec<String> = Vec::new();
    for (_, line, inside) in lines {
        if !inside || line.trim().is_empty() || translated(&line) || (mixed && han(&line)) {
            continue;
        }
        let text = clean(&line);
        let starts = DASHES.is_match(&text) || AUTHOR_START.is_match(&text);
        let continues = year_of(&text).is_none() && !starts && !raws.is_empty();
        if continues {
            let last = raws.last_mut().unwrap();
            *last = format!("{} {}", last.trim_end().trim_end_matches('\\'), line.trim());
        } else {
            raws.push(line.trim().to_string());
        }
    }
    let mut out: Vec<Entry> = Vec::new();
    for (index, raw) in raws.iter().enumerate() {
        let entry = split_entry(index + 1, raw, out.last());
        out.push(entry);
    }
    out
}

/// The reference id of each entry: the folded surname and the year; entries that share it get
/// `a`, `b`, … in order (an entry that names its own suffix keeps it). An unsplit entry is `refNNN`.
pub fn ids(entries: &[Entry]) -> Vec<String> {
    let base: Vec<String> = entries
        .iter()
        .map(|e| {
            if e.parsed {
                format!("{}{}", fold(&e.surname), e.year.chars().take(4).collect::<String>())
            } else {
                format!("ref{:03}", e.row)
            }
        })
        .collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for id in &base {
        *counts.entry(id.as_str()).or_default() += 1;
    }
    let mut seen: HashMap<&str, usize> = HashMap::new();
    entries
        .iter()
        .zip(&base)
        .map(|(e, id)| {
            let suffix: String = e.year.chars().skip(4).collect();
            if !suffix.is_empty() {
                return format!("{id}{suffix}");
            }
            if counts[id.as_str()] > 1 && e.parsed {
                let n = seen.entry(id.as_str()).or_default();
                let letter = (b'a' + (*n as u8)) as char;
                *n += 1;
                return format!("{id}{letter}");
            }
            id.clone()
        })
        .collect()
}

/// The candidate lines of one work: every line outside the reference lists that names its
/// surname and its year (with the year's own letter, when it has one).
fn candidates(
    lines: &[(usize, String, bool)],
    headings: &[(Option<String>, Option<String>)],
    entry: &Entry,
) -> Vec<Value> {
    if !entry.parsed {
        return Vec::new();
    }
    let surname = entry.surname.to_lowercase();
    let folded = fold(&entry.surname);
    let year = entry.year.clone();
    let mut found = Vec::new();
    for (index, (number, line, inside)) in lines.iter().enumerate() {
        if *inside || line.trim_start().starts_with('#') {
            continue;
        }
        let lower = line.to_lowercase();
        let names = lower.contains(&surname) || (!folded.is_empty() && fold(&lower).contains(&folded) && folded.len() > 3);
        if !names || !cites_year(line, &year) {
            continue;
        }
        let (heading, anchor) = &headings[index];
        let text: String = line.trim().chars().take(CANDIDATE_CHARS).collect();
        found.push(json!({ "line": number, "heading": heading, "anchor": anchor, "text": text }));
        if found.len() >= MAX_CANDIDATES {
            break;
        }
    }
    found
}

/// Whether a line cites `year`: the four digits, followed by the year's own letter when it has
/// one, and not by another letter when it has none.
fn cites_year(line: &str, year: &str) -> bool {
    let digits: String = year.chars().take(4).collect();
    let letter = year.chars().nth(4);
    let mut start = 0;
    while let Some(at) = line[start..].find(&digits) {
        let at = start + at;
        let before = line[..at].chars().next_back();
        let after = line[at + digits.len()..].chars().next();
        let bounded = !before.is_some_and(|c| c.is_ascii_digit());
        let matches = match letter {
            Some(letter) => after == Some(letter),
            None => !after.is_some_and(|c| c.is_ascii_alphanumeric()),
        };
        if bounded && matches {
            return true;
        }
        start = at + digits.len();
    }
    false
}

/// Each line's section heading (the last heading in the book's language, or the last heading)
/// and the anchor of the group whose headings name it.
pub(crate) fn line_headings(lines: &[(usize, String, bool)], groups: &Value) -> Vec<(Option<String>, Option<String>)> {
    let mut anchors: HashMap<String, String> = HashMap::new();
    for group in groups.as_array().into_iter().flatten() {
        let anchor = pstr(group, "anchor");
        for heading in group["headings"].as_array().into_iter().flatten() {
            if let Some(text) = heading.as_str() {
                anchors.entry(text.trim().to_string()).or_insert_with(|| anchor.clone());
            }
        }
    }
    let mut current: Option<String> = None;
    let mut any: Option<String> = None;
    lines
        .iter()
        .map(|(_, line, _)| {
            if let Some(text) = heading_text(line) {
                if HAN.is_match(&text) {
                    current = Some(text.clone());
                }
                any = Some(text);
            }
            let heading = current.clone().or_else(|| any.clone());
            let anchor = heading.as_ref().and_then(|h| anchors.get(h)).cloned();
            (heading, anchor)
        })
        .collect()
}

/// A title as the box match compares it: lower-case letters and digits only.
pub(crate) fn title_key(title: &str) -> String {
    fold(title)
}

// --- plan ----------------------------------------------------------------------------------------

fn plan_step(p: &Params, manifest: &Value, key: &str, work: &str, zt: &mut Zt) -> Result<i32> {
    let source_path = if p.source.is_empty() { pstr(manifest, "source") } else { p.source.clone() };
    if source_path.is_empty() {
        bail!("the staging manifest names no source; pass -Source");
    }
    let bytes = std::fs::read(&source_path).with_context(|| format!("failed to read {source_path}"))?;
    let source = String::from_utf8_lossy(&bytes).trim_start_matches('\u{FEFF}').to_string();
    let lines = reference_lines(&source, &p.heading);
    let headings = line_headings(&lines, &manifest["groups"]);
    let parsed = entries(&source, &p.heading);
    let ids = ids(&parsed);

    // The Literature trees of the box, for the works the box already holds.
    let listed = zt.call("list_trees", json!({ "zone": "literature" }))?;
    let mut box_titles: HashMap<String, String> = HashMap::new();
    for tree in listed["trees"].as_array().into_iter().flatten() {
        let root = pstr(tree, "root");
        let title = title_key(&pstr(tree, "title"));
        if root != key && title.len() >= 12 {
            box_titles.entry(title).or_insert(root);
        }
    }

    let rows: Vec<Value> = parsed
        .iter()
        .zip(&ids)
        .map(|(e, id)| {
            let in_box = (e.parsed && title_key(&e.title).len() >= 12)
                .then(|| box_titles.get(&title_key(&e.title)).cloned())
                .flatten();
            json!({
                "row": e.row,
                "id": id,
                "raw": e.raw,
                "entry": e.entry,
                "authors": e.authors,
                "surname": e.surname,
                "year": e.year,
                "title": e.title,
                "venue": e.venue,
                "parsed": e.parsed,
                "previous_authors": e.previous_authors,
                "box": in_box,
                "candidates": candidates(&lines, &headings, e),
            })
        })
        .collect();
    let plan_hash = crate::canon::hash_value(&json!(rows));
    let plan = json!({
        "schema_version": 1,
        "kind": "cited_plan",
        "book": key,
        "title": pstr(manifest, "book_title"),
        "source": source_path,
        "source_sha256": crate::canon::sha256_hex(&bytes),
        "plan_hash": plan_hash,
        "rows": rows,
    });
    let plan_path = ps::join(work, "plan.json");
    util::write_atomic(&plan_path, &ps::json_pretty(&plan))?;

    // One normalize job per forty unsplit entries.
    let unparsed: Vec<&Entry> = parsed.iter().filter(|e| !e.parsed).collect();
    let normalize = ps::join(work, "normalize");
    let mut jobs = Vec::new();
    if !unparsed.is_empty() {
        std::fs::create_dir_all(ps::join(&normalize, "out"))?;
        let doc = util::read_text(ps::join(&common::scripts_dir(), NORMALIZE_INSTRUCTION))?
            .replace("{{KEY}}", key);
        let doc_path = ps::join(&normalize, NORMALIZE_INSTRUCTION);
        util::write_atomic(&doc_path, &doc)?;
        for (n, chunk) in unparsed.chunks(NORMALIZE_PER_JOB).enumerate() {
            let id = format!("n{:02}", n + 1);
            let answer = ps::join(&ps::join(&normalize, "out"), &format!("{id}.json"));
            let mut text = format!("# 拆分任务 {id}\n\n输出文件：{answer}\n\n");
            for e in chunk {
                text.push_str(&format!("- row {}：{}\n", e.row, clean(&e.raw)));
                if !e.previous_authors.is_empty() {
                    text.push_str(&format!("  上一条的作者：{}\n", e.previous_authors));
                }
            }
            text.push_str(&format!(
                "\n样例：`[{{\"row\":{},\"authors\":\"Jones, K.\",\"year\":\"\",\"title\":\"…\",\"venue\":\"\"}}]`\n\nSELF-CHECK: zt-digest cited -Step precheck -Staging \"{}\" -Draft \"{answer}\"\n",
                chunk[0].row, p.staging
            ));
            let job_path = ps::join(&normalize, &format!("job-{id}.md"));
            util::write_atomic(&job_path, &text)?;
            jobs.push(json!({
                "id": id,
                "files": [doc_path, job_path],
                "prompt": NORMALIZE_PROMPT,
                "skills": ["asd-ste100-zh"],
                "model": MODEL,
                "effort": "high",
                "answer": answer,
            }));
        }
    }
    let jobs_path = ps::join(&normalize, "jobs.json");
    if !jobs.is_empty() {
        util::write_atomic(&jobs_path, &ps::json_pretty(&json!(jobs)))?;
    }

    if std::path::Path::new(&ps::join(&p.staging, "ledger.sqlite3")).is_file() {
        let prepared = crate::ledger::Prepared::read(&p.staging, zt)?;
        crate::ledger::cited_register(&prepared, key)?;
    }

    let mut line = json!({ "ok": true, "rows": parsed.len(), "plan": plan_path });
    if !unparsed.is_empty() {
        line["unparsed"] = json!(unparsed.len());
        line["jobs"] = json!(jobs.len());
        line["file"] = json!(jobs_path);
    }
    util::print_line(&util::compact(&line))?;
    Ok(0)
}

// --- precheck ------------------------------------------------------------------------------------

fn read_plan(work: &str) -> Result<Value> {
    let plan = util::read_json(ps::join(work, "plan.json")).context("no cited plan; run -Step plan first")?;
    if plan["kind"] != "cited_plan" {
        bail!("cited\\plan.json is not a cited plan");
    }
    Ok(plan)
}

/// The faults of one normalize answer against the plan's raw entries.
fn normalize_faults(plan: &Value, answer: &Value) -> Vec<String> {
    let rows = plan["rows"].as_array().cloned().unwrap_or_default();
    let mut faults = Vec::new();
    let Some(items) = answer.as_array() else {
        return vec!["answer N0 the answer is not a JSON array".to_string()];
    };
    for item in items {
        let number = item["row"].as_u64().unwrap_or(0) as usize;
        let Some(row) = rows.iter().find(|row| row["row"].as_u64() == Some(number as u64)) else {
            faults.push(format!("row{number} N1 no reference row {number} in the plan"));
            continue;
        };
        let raw = clean(&format!(
            "{} {}",
            pstr(row, "raw"),
            pstr(row, "previous_authors")
        ));
        let authors = pstr(item, "authors");
        let surname = surname_of(&authors);
        if surname.is_empty() || !raw.to_lowercase().contains(&surname.to_lowercase()) {
            faults.push(format!(
                "row{number} N1 the first author's surname `{surname}` is not in the raw entry"
            ));
        }
        let year = pstr(item, "year");
        if year.is_empty() {
            if year_of(&raw).is_some() || Regex::new(r"\b(1[5-9]\d\d|20\d\d)\b").unwrap().is_match(&raw) {
                faults.push(format!("row{number} N2 the raw entry has a year; write it"));
            }
        } else if !raw.contains(&year) {
            faults.push(format!("row{number} N2 the year `{year}` is not in the raw entry"));
        }
        if pstr(item, "title").trim().is_empty() {
            faults.push(format!("row{number} N3 the title is empty"));
        }
    }
    faults
}

fn precheck_step(p: &Params, work: &str) -> Result<i32> {
    if p.draft.is_empty() {
        bail!("-Draft is required for -Step precheck");
    }
    let plan = read_plan(work)?;
    let answer = util::read_json(&p.draft).unwrap_or(Value::Null);
    let faults = normalize_faults(&plan, &answer);
    let mut lines = faults.clone();
    lines.push(format!("faults: {}", faults.len()));
    util::print_lines(&lines)?;
    Ok(0)
}

// --- apply ---------------------------------------------------------------------------------------

/// A table cell: one line, no `|`.
pub(crate) fn cell(text: &str) -> String {
    text.replace('|', "／").replace(['\n', '\r'], " ").trim().to_string()
}

/// The list rows after the accepted normalize answers: (row, entry, year) per row, plus the
/// refused rows.
fn merged_rows(plan: &Value, work: &str) -> Result<(Vec<Value>, Vec<usize>)> {
    let mut rows = plan["rows"].as_array().cloned().unwrap_or_default();
    let mut refused = Vec::new();
    let out = ps::join(&ps::join(work, "normalize"), "out");
    if let Ok(dir) = std::fs::read_dir(&out) {
        let mut files: Vec<_> = dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        files.sort();
        for file in files {
            let answer = util::read_json(&file).unwrap_or(Value::Null);
            for item in answer.as_array().into_iter().flatten() {
                let number = item["row"].as_u64().unwrap_or(0);
                if !normalize_faults(plan, &json!([item])).is_empty() {
                    refused.push(number as usize);
                    continue;
                }
                if let Some(row) = rows.iter_mut().find(|row| row["row"].as_u64() == Some(number)) {
                    if row["parsed"] == true {
                        continue;
                    }
                    for field in ["authors", "year", "title", "venue"] {
                        row[field] = item[field].clone();
                    }
                    row["surname"] = json!(surname_of(&pstr(item, "authors")));
                    row["parsed"] = json!(true);
                }
            }
        }
    }
    refused.sort_unstable();
    refused.dedup();
    Ok((rows, refused))
}

/// The Cited list Card's title and body.
pub fn list_card(book_title: &str, key: &str, rows: &[(String, String, String, String)]) -> (String, String) {
    let title = format!("参考文献：《{book_title}》引用的 {} 篇文献", rows.len());
    let mut body = format!(
        "本卡是《{book_title}》的参考文献表。表中按原书顺序列出它的 {} 篇参考文献；每篇文献有一张文献引用卡，排在本卡之后。\n\n| 编号 | 文献 | 年份 | 引用处 | 关系 |\n| --- | --- | --- | --- | --- |\n",
        rows.len()
    );
    for (index, (entry, year, cited_at, relation)) in rows.iter().enumerate() {
        body.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            index + 1,
            cell(entry),
            cell(year),
            cell(cited_at),
            cell(relation)
        ));
    }
    body.push_str(&format!("\n来源：[[{key}]] 参考文献"));
    (title, body)
}

/// The existing Cited list of the book, if any: its address and its rows.
pub fn existing_list(zt: &mut Zt, key: &str) -> Result<Option<Value>> {
    let reply = zt.request("get_cited", json!({ "at": key }))?;
    if reply["result"]["isError"] == true {
        let text = reply["result"]["content"][0]["text"].as_str().unwrap_or_default().to_string();
        if text.contains("has no Cited list") {
            return Ok(None);
        }
        bail!("get_cited {key}: {text}");
    }
    let text = reply["result"]["content"][0]["text"].as_str().unwrap_or("{}");
    Ok(Some(serde_json::from_str(text).context("get_cited: the answer is not JSON")?))
}

fn apply_step(p: &Params, manifest: &Value, key: &str, work: &str, zt: &mut Zt) -> Result<i32> {
    let plan = read_plan(work)?;
    let (rows, refused) = merged_rows(&plan, work)?;
    let existing = existing_list(zt, key)?;
    let kept = |entry: &str, field: &str| -> Option<String> {
        existing.as_ref().and_then(|list| {
            list["rows"].as_array().into_iter().flatten().find(|row| pstr(row, "entry") == cell(entry)).map(|row| pstr(row, field))
        })
    };
    let table: Vec<(String, String, String, String)> = rows
        .iter()
        .map(|row| {
            let entry = pstr(row, "entry");
            let year = pstr(row, "year");
            let cited_at = kept(&entry, "cited_at").filter(|v| !v.is_empty()).unwrap_or_else(|| "—".into());
            let relation = kept(&entry, "relation").filter(|v| !v.is_empty()).unwrap_or_else(|| "—".into());
            (entry, if year.is_empty() { "—".into() } else { year }, cited_at, relation)
        })
        .collect();
    let mut card = existing.as_ref().map(|list| pstr(list, "list"));
    let mut changed = false;
    if !table.is_empty() && p.apply {
        let (title, body) = list_card(&pstr(manifest, "book_title"), key, &table);
        match &card {
            Some(address) => {
                let current = zt.call("get_card", json!({ "at": address }))?;
                if pstr(&current, "title") != title || pstr(&current, "body").trim_end() != body.trim_end() {
                    zt.call("edit_card", json!({ "at": address, "title": title, "body": body }))?;
                    changed = true;
                }
            }
            None => {
                let created = zt.call(
                    "create_successor",
                    json!({ "at": format!("{key}/1"), "kind": "side", "type": "cited", "title": title, "body": body }),
                )?;
                card = Some(pstr(&created, "address"));
                changed = true;
            }
        }
    }
    let report = json!({
        "schema_version": 1,
        "kind": "cited_apply",
        "ok": true,
        "unit": "cited/book@1",
        "token": p.token,
        "book": key,
        "plan_hash": plan["plan_hash"],
        "applied": p.apply,
        "list": card,
        "rows": table.len(),
        "refused": refused,
        "changed": changed,
    });
    util::write_atomic(ps::join(work, "apply.json"), &ps::json_pretty(&report))?;
    // The rows after the accepted normalize answers: the Cited Cards read these (a normalized entry has
    // its surname and title only here; live backfill of acemoglu2025nobel, 2026-10-31).
    util::write_atomic(ps::join(work, "merged.json"), &ps::json_pretty(&json!({ "plan_hash": plan["plan_hash"], "rows": rows })))?;
    let mut line = json!({ "ok": true, "rows": table.len(), "card": card });
    if !refused.is_empty() {
        line["refused"] = json!(refused.len());
    }
    util::print_line(&util::compact(&line))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_year_is_cited_only_with_its_own_letter() {
        assert!(cites_year("Acemoglu (2003a) shows", "2003a"));
        assert!(!cites_year("Acemoglu (2003b) shows", "2003a"));
        assert!(cites_year("Deming（2017）", "2017"));
        assert!(!cites_year("in 2017a", "2017"));
        assert!(!cites_year("page 12017", "2017"));
    }

    #[test]
    fn the_first_author_and_the_surname_come_from_the_author_string() {
        assert_eq!(first_author("Aghion, P., and Bolton, P."), "Aghion, P.");
        assert_eq!(first_author("Atkinson, Anthony B., and Joseph E. Stiglitz"), "Atkinson, Anthony B.");
        assert_eq!(surname_of("ACEMOGLU, D., AND F. ZILIBOTTI"), "Acemoglu");
        assert_eq!(surname_of("Benmelech, E. and Frydman, C."), "Benmelech");
        assert_eq!(surname_of("Economist"), "Economist");
        assert_eq!(fold("Ab Iorwerth"), "abiorwerth");
    }
}
