//! `zt-digest cited`: the Cited list of a digest (#219, docs/cited-literature-cards-proposal.md).
//! DeepSeek reads the book's reference material and writes one entry per work; the tool checks each
//! entry against the source lines it names, removes repeated works, indexes the lines that cite each
//! work, and writes the Cited list Card (type `cited`).
//!
//! Why reading jobs and no parser (2026-11-07, docs/cited-list-reading-proposal.md): a reference list
//! that came through OCR and the bilingual pipeline has no fixed shape. A rule-based parser merged
//! entries that do not start with `Surname, ` (`al-Azdī: …`, `‘Arīb: …`), took a humanities title for
//! the authors (`Author, Title (Place, Year)`), carried that error down every `——` entry, kept the
//! passages the OCR repeated (Atkinson 729 repeated rows, Morony 517), and kept preface prose as entries.
//!
//!   zt-digest cited -Step plan|precheck|apply -Staging <folder> [-Heading <h1,h2>] [-Source <file>]
//!                   [-Draft <file>] [-Token <n>] [-Apply]
//!
//! -Heading adds headings (comma-separated) that open reference material besides References,
//! Bibliography and 参考文献 (morony2005iraq: `AUTHOR INDEX`, whose works point to the full citations).
//! -Source names the whole bilingual source when manifest.json's `source` is not it: an adopted staging
//! names the staged Chinese-only `book.zh.md`, which holds no English reference list (pilot 2026-10-31).
//! The plan freezes the file it read as `source` and the headings as `heading`; `cited-cards` reads both.
//!
//!   plan      marks the reference material (the sections under the reference headings, without their
//!             translation), cuts it into parts of about [`PART_LINES`] lines, and writes one reading job
//!             per [`PARTS_PER_JOB`] parts (one conversation, one message per part) to cited\read\jobs.json.
//!             A job whose parts all hold a passing answer is not listed again. Writes no Card. Prints
//!             {"ok","rows" (material lines),"parts","jobs","file"}.
//!   precheck  checks one part's answer (-Draft): every entry is copied from the lines it names, its
//!             surname, year and title stand in it, and every line of the part is an entry's or skipped.
//!             One fault per line, then `faults: N`.
//!   apply     refuses while any part lacks a passing answer; otherwise orders the entries by their
//!             lines, removes a work listed twice, indexes the citing lines, writes cited\merged.json (the
//!             rows the Cited Cards read) and, with -Apply, creates or updates the Cited list Card,
//!             keeping the `引用处`/`关系` cells of an unchanged row, and writes cited\apply.json (the
//!             ledger's evidence). Prints {"ok","rows","card","repeated"}.


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

/// A year in any form, for telling a reference section from an index: `(Princeton, 1980)`, `A.H. 1352`.
static ANY_YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|[^\d])(1[0-9]\d\d|20\d\d)(?:[^\d]|$)").unwrap());
/// A four-digit year an entry may hold (`1866`, `(1920–1)`, `A.H. 1352` holds none in range).
static YEAR4: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|[^\d])(1[5-9]\d\d|20\d\d)(?:[^\d]|$)").unwrap());

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
    "select bibliography",
    "selected bibliography",
    "works cited",
    "参考文献",
    "参考文献选目",
    "参考",
    "文献",
];

/// The most candidate lines one work keeps.
const MAX_CANDIDATES: usize = 60;
/// The most characters of one candidate line the plan keeps.
const CANDIDATE_CHARS: usize = 240;


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
    REFERENCE_HEADINGS.contains(&lower.as_str())
        || custom.split(',').map(|h| h.trim().to_lowercase()).any(|h| !h.is_empty() && h == lower)
}

/// Whether the section after a heading goes on with a reference list: at least half of its lines
/// carry a year. A section with no line of its own goes on too: the bilingual source writes each
/// heading twice, and the first has only the second under it. A book's index and its appendices
/// carry no years, so the list ends there.
fn section_goes_on(lines: &[&str], from: usize) -> bool {
    let body: Vec<&str> = lines[from..]
        .iter()
        .take_while(|line| heading_text(line).is_none())
        .copied()
        .filter(|line| !line.trim().is_empty() && !translated(line))
        .collect();
    let dated = body.iter().filter(|line| ANY_YEAR.is_match(line)).count();
    dated * 2 >= body.len()
}

/// The source's lines, each marked when it stands in a reference list. A list runs on through its
/// own subheadings (hoyland2015path, donner1981conquests: a bibliography split by kind or region).
pub fn reference_lines(source: &str, custom: &str) -> Vec<(usize, String, bool)> {
    let all: Vec<&str> = source.lines().collect();
    let mut inside = false;
    all.iter()
        .enumerate()
        .map(|(index, line)| {
            if let Some(text) = heading_text(line) {
                inside = is_reference_heading(&text, custom) || (inside && section_goes_on(&all, index + 1));
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

/// Whether a reference line is a translation of the list: more Han characters than Latin letters
/// (Hart 1995 repeats its whole list in Chinese under the same heading).
fn translated(line: &str) -> bool {
    let han = line.chars().filter(|c| HAN.is_match(&c.to_string())).count();
    let latin = line.chars().filter(char::is_ascii_alphabetic).count();
    han > latin
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

/// ASCII folding for a reference id: `Custódio` → `custodio`.
pub(crate) fn fold(text: &str) -> String {
    text.chars()
        .filter_map(|c| {
            let base = match c {
                'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' | 'ā' | 'ă' | 'ą' | 'Ā' | 'Ă' | 'Ą' => 'a',
                'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' | 'ē' | 'ĕ' | 'ę' | 'ė' | 'ě' | 'Ē' | 'Ě' => 'e',
                'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' | 'ī' | 'ĭ' | 'į' | 'ı' | 'Ī' | 'İ' => 'i',
                'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' | 'Ø' | 'ō' | 'ŏ' | 'ő' | 'Ō' => 'o',
                'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'Ū' => 'u',
                'ç' | 'Ç' | 'ć' | 'č' | 'Ć' | 'Č' => 'c',
                'ñ' | 'Ñ' | 'ń' | 'ň' | 'ṇ' | 'Ṇ' => 'n',
                'ß' | 'ś' | 'š' | 'ş' | 'ṣ' | 'Ś' | 'Š' | 'Ş' | 'Ṣ' => 's',
                // The letters of Arabic and Persian transliteration (scheiner2022baghdad): `Ṭāhir` folded to `hir`.
                'ḍ' | 'ď' | 'đ' | 'Ḍ' | 'Ď' | 'Đ' => 'd',
                'ġ' | 'ğ' | 'ǧ' | 'Ġ' | 'Ğ' => 'g',
                'ḥ' | 'ḫ' | 'ħ' | 'Ḥ' | 'Ḫ' => 'h',
                'ḳ' | 'ķ' | 'Ḳ' => 'k',
                'ḷ' | 'ł' | 'Ḷ' | 'Ł' => 'l',
                'ṛ' | 'ř' | 'Ṛ' => 'r',
                'ṭ' | 'ť' | 'ţ' | 'Ṭ' | 'Ť' => 't',
                'ẓ' | 'ž' | 'ż' | 'ź' | 'Ẓ' | 'Ž' => 'z',
                'ý' | 'ÿ' | 'Ý' => 'y',
                other => other,
            };
            base.is_ascii_alphanumeric().then(|| base.to_ascii_lowercase())
        })
        .collect()
}

/// A folded name with its doubled letters squeezed: transliteration writes a doubled consonant in different places
/// (`Ibn Batūṭṭa` in the list, `Ibn Baṭṭūṭa` in the text).
pub(crate) fn squeeze(folded: &str) -> String {
    let mut letters: Vec<char> = folded.chars().collect();
    letters.dedup();
    letters.into_iter().collect()
}

/// Whether a Term hit (`uses `X` for `Y`: <sentence>`) stands in a sentence the reference itself holds: the `文献：` line
/// quotes the reference verbatim, and a title with a full stop in it splits into sentences that lose the line's prefix.
pub(crate) fn quotes_the_reference(message: &str, entry: &str) -> bool {
    message.split_once(": ").is_some_and(|(_, sentence)| entry.contains(sentence.trim()))
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

/// The candidate lines of one work: every line outside the reference lists that names its surname and its
/// year (with the year's own letter, when it has one); a work without a year (an author index) needs its
/// surname and the first long word of its title. `folded_lines` holds each line lower-cased and folded, computed once.
fn candidates(
    lines: &[(usize, String, bool)],
    headings: &[(Option<String>, Option<String>)],
    folded_lines: &[(String, String)],
    entry: &Entry,
) -> Vec<Value> {
    if !entry.parsed {
        return Vec::new();
    }
    let surname = entry.surname.to_lowercase();
    let folded = fold(&entry.surname);
    let year = entry.year.clone();
    let word = entry.title.split(|c: char| !c.is_alphanumeric()).map(fold).find(|w| w.len() >= 5);
    let mut found = Vec::new();
    for (index, (number, line, inside)) in lines.iter().enumerate() {
        if *inside || line.trim_start().starts_with('#') {
            continue;
        }
        let (lower, flat) = &folded_lines[index];
        let names = lower.contains(&surname) || (folded.len() > 3 && flat.contains(&folded));
        let dated = if year.is_empty() { word.as_ref().is_some_and(|w| flat.contains(w.as_str())) } else { cites_year(line, &year) };
        if !names || !dated {
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

/// About this many material lines make one part (one message of a reading job).
const PART_LINES: usize = 60;
/// A part closes early when its text passes this many characters.
const PART_CHARS: usize = 12_000;
/// The parts one reading job (one conversation) holds.
const PARTS_PER_JOB: usize = 4;

pub const MODEL: &str = "deepseek-official/deepseek-flash";
const READ_INSTRUCTION: &str = "dsh-cited-read-instructions.zh.md";
const READ_PROMPT: &str = "按以上说明读本段的参考文献，写出 JSON 文件并运行自查。最后只回复一行 JSON。";
const NEXT_PROMPT: &str = "继续：按同一份说明读下一段，写出 JSON 文件并运行自查。最后只回复一行 JSON。";

/// One line of the reference material: its source line number and its cleaned text.
#[derive(Clone, Debug)]
pub struct Material {
    pub line: usize,
    pub text: String,
}

/// The reference material of a source: every non-empty line of its reference sections, cleaned, without
/// the lines in translation. A list that holds both Latin entries and entries with Chinese in them is the
/// original followed by its translation (Grossman 2002, Hart 1995): its Chinese lines are left out. A list
/// that is all Chinese is the book's own.
pub fn material(source: &str, custom: &str) -> Vec<Material> {
    let lines = reference_lines(source, custom);
    let han = |line: &str| HAN.find_iter(line).count() >= 2;
    let listed: Vec<&String> = lines.iter().filter(|(_, l, inside)| *inside && !l.trim().is_empty()).map(|(_, l, _)| l).collect();
    let mixed = listed.iter().any(|l| han(l)) && listed.iter().any(|l| !han(l));
    lines
        .into_iter()
        .filter(|(_, line, inside)| *inside && !line.trim().is_empty() && !translated(line) && !(mixed && han(line)))
        .map(|(n, line, _)| Material { line: n, text: clean(&line) })
        .filter(|m| !m.text.is_empty())
        .collect()
}

/// Cut the material into parts of about [`PART_LINES`] lines or [`PART_CHARS`] characters. A part ends,
/// where it can, after a line that closes an entry (`.`, `)` or a line-break `\`), so an entry is not cut.
fn parts(material: &[Material], raw: &[&str]) -> Vec<Vec<Material>> {
    let closes = |m: &Material| {
        let line = raw.get(m.line - 1).map(|l| l.trim_end()).unwrap_or_default();
        line.ends_with('\\') || line.ends_with('.') || line.ends_with(')') || line.ends_with('。')
    };
    let mut out: Vec<Vec<Material>> = Vec::new();
    let mut current: Vec<Material> = Vec::new();
    let mut chars = 0usize;
    for m in material {
        chars += m.text.chars().count();
        current.push(m.clone());
        let full = current.len() >= PART_LINES || chars >= PART_CHARS;
        let hard = current.len() >= PART_LINES + 15 || chars >= PART_CHARS + 3_000;
        if (full && closes(m)) || hard {
            out.push(std::mem::take(&mut current));
            chars = 0;
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Text as the entry check compares it: one space for any run of spaces, one kind of quote mark, lower case.
fn norm(text: &str) -> String {
    let mapped: String = text
        .chars()
        .map(|c| match c {
            '‘' | '’' | 'ʼ' | '`' | '′' => '\'',
            '“' | '”' | '″' => '"',
            '\u{a0}' => ' ',
            other => other,
        })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn part_id(job: usize, part: usize) -> String {
    format!("r{job:02}-{part}")
}

fn read_dir(work: &str) -> String {
    ps::join(work, "read")
}

fn answer_path(work: &str, id: &str) -> String {
    ps::join(&ps::join(&read_dir(work), "out"), &format!("{id}.json"))
}

// --- plan ----------------------------------------------------------------------------------------

fn plan_step(p: &Params, manifest: &Value, key: &str, work: &str, zt: &mut Zt) -> Result<i32> {
    let source_path = if p.source.is_empty() { pstr(manifest, "source") } else { p.source.clone() };
    if source_path.is_empty() {
        bail!("the staging manifest names no source; pass -Source");
    }
    let bytes = std::fs::read(&source_path).with_context(|| format!("failed to read {source_path}"))?;
    let source = String::from_utf8_lossy(&bytes).trim_start_matches('\u{FEFF}').to_string();
    let raw: Vec<&str> = source.lines().collect();
    let found = material(&source, &p.heading);
    let cut = parts(&found, &raw);

    // One job per PARTS_PER_JOB parts; part (job, k) is `rJJ-K`.
    let mut plan_parts = Vec::new();
    for (index, part) in cut.iter().enumerate() {
        let (job, k) = (index / PARTS_PER_JOB + 1, index % PARTS_PER_JOB + 1);
        plan_parts.push(json!({
            "id": part_id(job, k),
            "job": job,
            "lines": part.iter().map(|m| m.line).collect::<Vec<_>>(),
        }));
    }
    let plan_hash = crate::canon::hash_value(&json!({ "source": crate::canon::sha256_hex(&bytes), "heading": p.heading, "parts": plan_parts }));
    let plan = json!({
        "schema_version": 2,
        "kind": "cited_plan",
        "book": key,
        "title": pstr(manifest, "book_title"),
        "source": source_path,
        "heading": p.heading,
        "source_sha256": crate::canon::sha256_hex(&bytes),
        "plan_hash": plan_hash,
        "parts": plan_parts,
        "rows": [],
    });
    let plan_path = ps::join(work, "plan.json");
    // A new plan of other parts makes every old answer stale.
    let old = util::read_json(&plan_path).ok();
    let read = read_dir(work);
    if old.as_ref().is_some_and(|old| old["plan_hash"] != json!(plan_hash)) && std::path::Path::new(&read).is_dir() {
        std::fs::remove_dir_all(&read)?;
    }
    util::write_atomic(&plan_path, &ps::json_pretty(&plan))?;

    std::fs::create_dir_all(ps::join(&read, "out"))?;
    let doc = util::read_text(ps::join(&common::scripts_dir(), READ_INSTRUCTION))?.replace("{{KEY}}", key);
    let doc_path = ps::join(&read, READ_INSTRUCTION);
    util::write_atomic(&doc_path, &doc)?;
    let text_of: HashMap<usize, &str> = found.iter().map(|m| (m.line, m.text.as_str())).collect();
    let mut jobs = Vec::new();
    let mut settled = 0usize;
    let jobs_total = cut.len().div_ceil(PARTS_PER_JOB);
    for job in 1..=jobs_total {
        let own: Vec<&Value> = plan["parts"].as_array().into_iter().flatten().filter(|x| x["job"].as_u64() == Some(job as u64)).collect();
        let mut files = Vec::new();
        for (k, part) in own.iter().enumerate() {
            let id = pstr(part, "id");
            let answer = answer_path(work, &id);
            let mut text = format!("# 参考文献读取任务 {id}（第 {} 段，共 {} 段）\n\n输出文件：{answer}\n\n## 本段的原文行\n\n", k + 1, own.len());
            for n in part["lines"].as_array().into_iter().flatten().filter_map(Value::as_u64) {
                text.push_str(&format!("L{n}: {}\n", text_of.get(&(n as usize)).copied().unwrap_or_default()));
            }
            text.push_str(&format!("\nSELF-CHECK: zt-digest cited -Step precheck -Staging \"{}\" -Draft \"{answer}\"\n", p.staging));
            let path = ps::join(&read, &format!("part-{id}.md"));
            util::write_atomic(&path, &text)?;
            files.push((id, path, answer));
        }
        let passed = files.iter().all(|(_, _, answer)| answer_faults_for(&plan, &found, answer).is_empty());
        if passed {
            settled += 1;
            continue;
        }
        let (first, rest) = files.split_first().context("a job without parts")?;
        jobs.push(json!({
            "id": format!("r{job:02}"),
            "files": [doc_path, first.1],
            "prompt": READ_PROMPT,
            "then": rest.iter().map(|(_, path, _)| json!({ "files": [path], "prompt": NEXT_PROMPT })).collect::<Vec<_>>(),
            "skills": ["asd-ste100-zh"],
            "model": MODEL,
            "effort": "high",
            "answer": files.last().map(|f| f.2.clone()),
        }));
    }
    let jobs_path = ps::join(&read, "jobs.json");
    util::write_atomic(&jobs_path, &ps::json_pretty(&json!(jobs)))?;

    if std::path::Path::new(&ps::join(&p.staging, "ledger.sqlite3")).is_file() {
        let prepared = crate::ledger::Prepared::read(&p.staging, zt)?;
        crate::ledger::cited_register(&prepared, key)?;
    }
    let mut line = json!({ "ok": true, "rows": found.len(), "parts": cut.len(), "jobs": jobs.len(), "plan": plan_path });
    if settled > 0 {
        line["settled"] = json!(settled);
    }
    if !jobs.is_empty() {
        line["file"] = json!(jobs_path);
    }
    util::print_line(&util::compact(&line))?;
    Ok(0)
}

// --- precheck ------------------------------------------------------------------------------------

fn read_plan(work: &str) -> Result<Value> {
    let plan = util::read_json(ps::join(work, "plan.json")).context("no cited plan; run -Step plan first")?;
    if plan["kind"] != "cited_plan" || plan["schema_version"] != 2 {
        bail!("cited\\plan.json is not a reading plan; run -Step plan");
    }
    Ok(plan)
}

/// The plan's material, read again from its frozen source.
fn plan_material(plan: &Value) -> Result<Vec<Material>> {
    let path = pstr(plan, "source");
    let bytes = std::fs::read(&path).with_context(|| format!("failed to read {path}"))?;
    let source = String::from_utf8_lossy(&bytes).trim_start_matches('\u{FEFF}').to_string();
    Ok(material(&source, &pstr(plan, "heading")))
}

/// The part an answer file belongs to: the one whose answer path it is.
fn part_of<'a>(plan: &'a Value, answer: &str) -> Option<&'a Value> {
    let stem = std::path::Path::new(answer).file_stem()?.to_str()?.to_string();
    plan["parts"].as_array()?.iter().find(|part| pstr(part, "id") == stem)
}

fn answer_faults_for(plan: &Value, material: &[Material], answer: &str) -> Vec<String> {
    match util::read_json(answer) {
        Ok(value) => part_faults(plan, material, &value, answer),
        Err(_) => vec![format!("answer R0 no readable JSON answer at {answer}")],
    }
}

/// The lines of a part's job (an entry may name a line of an earlier part of its job, when it starts there).
fn job_lines(plan: &Value, job: u64) -> std::collections::HashSet<usize> {
    plan["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|part| part["job"].as_u64() == Some(job))
        .flat_map(|part| part["lines"].as_array().cloned().unwrap_or_default())
        .filter_map(|n| n.as_u64().map(|n| n as usize))
        .collect()
}

fn line_numbers(value: &Value) -> Vec<usize> {
    value.as_array().into_iter().flatten().filter_map(|n| n.as_u64().map(|n| n as usize)).collect()
}

/// The faults of one part's answer. `answer_file` names the part; an empty name takes the part from the
/// answer's own `part` field (plan time). Each fault is one line: `<where> <rule> <what>`.
fn part_faults(plan: &Value, material: &[Material], answer: &Value, answer_file: &str) -> Vec<String> {
    let part = if answer_file.is_empty() { None } else { part_of(plan, answer_file) };
    let Some(part) = part else {
        return vec![format!("answer R0 `{answer_file}` is no part's answer file")];
    };
    let text_of: HashMap<usize, &str> = material.iter().map(|m| (m.line, m.text.as_str())).collect();
    let allowed = job_lines(plan, part["job"].as_u64().unwrap_or(0));
    let own: Vec<usize> = line_numbers(&part["lines"]);
    let Some(entries) = answer["entries"].as_array() else {
        return vec!["answer R0 the answer has no `entries` array".to_string()];
    };
    let mut faults = Vec::new();
    let mut covered: std::collections::HashSet<usize> = line_numbers(&answer["skip"]).into_iter().collect();
    for (index, item) in entries.iter().enumerate() {
        let at = format!("entry{}", index + 1);
        let lines = line_numbers(&item["lines"]);
        if lines.is_empty() {
            faults.push(format!("{at} R1 `lines` is empty"));
            continue;
        }
        if let Some(bad) = lines.iter().find(|n| !allowed.contains(n)) {
            faults.push(format!("{at} R1 L{bad} is not a line of this job"));
            continue;
        }
        covered.extend(lines.iter().copied());
        let mut sorted = lines.clone();
        sorted.sort_unstable();
        let joined = norm(&sorted.iter().map(|n| text_of.get(n).copied().unwrap_or_default()).collect::<Vec<_>>().join(" "));
        let entry = pstr(item, "entry");
        let entry_n = norm(&entry);
        if entry_n.is_empty() || !joined.contains(&entry_n) {
            faults.push(format!("{at} R2 `entry` is not copied from its lines: copy it letter by letter from L{}", sorted[0]));
            continue;
        }
        let dashed = DASHES.is_match(entry.trim());
        let authors = pstr(item, "authors");
        let surname = surname_of(&authors);
        if !authors.trim().is_empty() && !dashed && !entry_n.contains(&norm(&surname)) {
            faults.push(format!("{at} R3 the first author's surname `{surname}` is not in `entry`"));
        }
        if dashed && authors.trim().is_empty() {
            faults.push(format!("{at} R3 `entry` starts with a dash run: write the authors it repeats in `authors`"));
        }
        let year = pstr(item, "year");
        if year.is_empty() {
            if let Some(caps) = YEAR4.captures(&entry) {
                faults.push(format!("{at} R4 `entry` holds the year {}; write it in `year`", &caps[1]));
            }
        } else if !entry.contains(&year) {
            faults.push(format!("{at} R4 the year `{year}` is not in `entry`"));
        }
        let title = pstr(item, "title");
        if title.trim().is_empty() {
            faults.push(format!("{at} R5 `title` is empty"));
        } else if !entry_n.contains(&norm(&title)) {
            faults.push(format!("{at} R5 `title` is not copied from `entry`"));
        }
    }
    for n in own.iter().filter(|n| !covered.contains(n)) {
        let text: String = text_of.get(n).copied().unwrap_or_default().chars().take(120).collect();
        faults.push(format!("L{n} R6 this line is in no entry and not in `skip`: {text}"));
    }
    faults
}

fn precheck_step(p: &Params, work: &str) -> Result<i32> {
    if p.draft.is_empty() {
        bail!("-Draft is required for -Step precheck");
    }
    let plan = read_plan(work)?;
    let found = plan_material(&plan)?;
    let faults = answer_faults_for(&plan, &found, &p.draft);
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

/// The entry as the list shows it: a leading dash run is replaced by the authors it repeats.
fn display(entry: &str, authors: &str) -> String {
    let entry = entry.trim();
    match DASHES.find(entry) {
        Some(found) if !authors.trim().is_empty() => {
            let rest = entry[found.end()..].trim_start();
            // `——Futūḥ: al-Balādhurī, …` names its author after the short title: the dash run only goes.
            if norm(rest).contains(&norm(&surname_of(authors))) {
                return rest.to_string();
            }
            let gap = if rest.starts_with([',', '.', ':', ';']) { "" } else if rest.starts_with('(') { " " } else { ", " };
            format!("{}{gap}{rest}", authors.trim().trim_end_matches(','))
        }
        _ => entry.to_string(),
    }
}

/// Every passing entry of the plan, in source order, with the repeated works removed, as list rows; or the
/// parts whose answer is missing or faulty.
fn read_rows(plan: &Value, work: &str) -> Result<(Vec<Entry>, usize, Vec<String>)> {
    let found = plan_material(plan)?;
    let text_of: HashMap<usize, &str> = found.iter().map(|m| (m.line, m.text.as_str())).collect();
    let mut open = Vec::new();
    let mut gathered: Vec<(usize, usize, Value)> = Vec::new();
    for part in plan["parts"].as_array().into_iter().flatten() {
        let id = pstr(part, "id");
        let answer = answer_path(work, &id);
        let faults = answer_faults_for(plan, &found, &answer);
        if !faults.is_empty() {
            open.push(format!("{id} ({})", faults.len()));
            continue;
        }
        let value = util::read_json(&answer)?;
        for (index, item) in value["entries"].as_array().into_iter().flatten().enumerate() {
            let first = line_numbers(&item["lines"]).into_iter().min().unwrap_or(0);
            gathered.push((first, index, item.clone()));
        }
    }
    gathered.sort_by_key(|(line, index, _)| (*line, *index));
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut rows = Vec::new();
    let mut repeated = 0usize;
    for (_, _, item) in gathered {
        let authors = pstr(&item, "authors").trim().to_string();
        let title = pstr(&item, "title").trim().to_string();
        let year = pstr(&item, "year").trim().to_string();
        let surname = surname_of(&authors);
        let entry = display(&pstr(&item, "entry"), &authors);
        let identity = format!("{}|{}|{}", fold(&surname), year, title_key(if title.is_empty() { &entry } else { &title }).chars().take(60).collect::<String>());
        if !seen.insert(identity) {
            repeated += 1;
            continue;
        }
        let mut lines = line_numbers(&item["lines"]);
        lines.sort_unstable();
        rows.push(Entry {
            row: rows.len() + 1,
            raw: lines.iter().map(|n| text_of.get(n).copied().unwrap_or_default()).collect::<Vec<_>>().join(" "),
            entry,
            parsed: !surname.is_empty(),
            surname,
            authors,
            year,
            title,
            venue: pstr(&item, "venue").trim().to_string(),
        });
    }
    Ok((rows, repeated, open))
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
    let (parsed, repeated, open) = read_rows(&plan, work)?;
    if !open.is_empty() {
        bail!("{} part(s) have no passing answer: {}", open.len(), open.join(", "));
    }
    let ids = ids(&parsed);
    let bytes = std::fs::read(pstr(&plan, "source"))?;
    let source = String::from_utf8_lossy(&bytes).trim_start_matches('\u{FEFF}').to_string();
    let lines = reference_lines(&source, &pstr(&plan, "heading"));
    let headings = line_headings(&lines, &manifest["groups"]);
    let folded_lines: Vec<(String, String)> = lines.iter().map(|(_, l, _)| { let lower = l.to_lowercase(); let flat = fold(&lower); (lower, flat) }).collect();

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
            let in_box = (title_key(&e.title).len() >= 12).then(|| box_titles.get(&title_key(&e.title)).cloned()).flatten();
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
                "box": in_box,
                "candidates": candidates(&lines, &headings, &folded_lines, e),
            })
        })
        .collect();

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
        "repeated": repeated,
        "refused": [],
        "changed": changed,
    });
    util::write_atomic(ps::join(work, "apply.json"), &ps::json_pretty(&report))?;
    // The rows the Cited Cards read (cited-cards reads them when their plan_hash is the plan's).
    util::write_atomic(ps::join(work, "merged.json"), &ps::json_pretty(&json!({ "plan_hash": plan["plan_hash"], "rows": rows })))?;
    util::print_line(&util::compact(&json!({ "ok": true, "rows": table.len(), "card": card, "repeated": repeated })))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transliterated_name_folds_to_its_plain_letters() {
        assert_eq!(fold("Ṭāhir"), "tahir");
        assert_eq!(fold("Tāhir"), "tahir");
        assert_eq!(fold("al-Aṣīl"), fold("Al-Asil"));
        assert_eq!(fold("Mīrkhwānd"), fold("Mirkhwānd"));
        assert_eq!(fold("Ḥasan ibn Ẓāhir ʿAlī"), "hasanibnzahiràli".replace('à', "a").as_str());
        assert_eq!(fold("Ab Iorwerth"), "abiorwerth");
    }

    #[test]
    fn a_doubled_consonant_may_stand_anywhere_in_a_name() {
        assert_eq!(squeeze(&fold("Ibn Batūṭṭa")), squeeze(&fold("Ibn Baṭṭūṭa")));
        assert_ne!(squeeze(&fold("Ibn Baṭṭūṭa")), squeeze(&fold("Ibn Jubayr")));
    }

    #[test]
    fn a_term_hit_inside_the_quoted_reference_is_no_fault() {
        let entry = "Messina, V., L'area di al-Madā'in. Dal declino di Seleucia alla Fondazione di Veh Ardashir, in: Genito, B. and Caterina, L. (eds.), Naples 2015.";
        assert!(quotes_the_reference("uses `Veh Ardashir` for `韦赫·阿尔达希尔`: Dal declino di Seleucia alla Fondazione di Veh Ardashir, in: Genito, B.", entry));
        assert!(!quotes_the_reference("uses `Veh Ardashir` for `韦赫·阿尔达希尔`: 本书在这里写 Veh Ardashir。", entry));
    }

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
        assert_eq!(surname_of("ACEMOGLU, D., AND F. ZILIBOTTI"), "Acemoglu");
        assert_eq!(surname_of("Benmelech, E. and Frydman, C."), "Benmelech");
        assert_eq!(surname_of("Economist"), "Economist");
        assert_eq!(fold("Ab Iorwerth"), "abiorwerth");
    }

    #[test]
    fn a_reference_list_runs_on_through_its_own_subheadings_and_stops_at_the_index() {
        // hoyland2015path and donner1981conquests: a book's bibliography is split by subheadings at the
        // list's own level, and the bilingual source writes each heading twice. The plan stopped at the
        // first subheading, so Hoyland's list had no row and Donner's lost most of its rows.
        let text = "# 正文\n\n正文。\n\n# SELECT BIBLIOGRAPHY\n\n# 参考文献选目\n\n# General Studies\n\n# 一般研究\n\nDonner, F. The Early Islamic Conquests (Princeton, 1980).\\\nKaegi, W. Byzantium and the Early Islamic Conquests (Cambridge, 1992).\\\n\n# Regional Studies\n\n# 区域研究\n\n## Egypt\n\n## 埃及\n\nBell, H. Egypt from Alexander the Great (Oxford, 1948).\n\n# INDEX\n\n# 索引\n\nAbbasids, 12, 45\n\nYarmuk, 45, 66, 95\n\nZiyad, 112\n";
        let marked: Vec<String> = reference_lines(text, "")
            .into_iter()
            .filter(|(_, line, inside)| *inside && !line.trim().is_empty())
            .map(|(_, line, _)| line)
            .collect();
        assert_eq!(marked.len(), 3, "{marked:?}");
        assert!(marked.iter().all(|line| line.contains('(') && !line.contains("Yarmuk")), "{marked:?}");
    }
}
