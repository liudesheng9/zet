//! The two guards against a DeepSeek read that cuts a line at 2,000 characters (proposal
//! docs/bilingual-long-line-cut-proposal.md, P1 and P3; the pdf-ocr-bilingual workflow holds the same rules in
//! `pdfocr.py`: `wrap_long`, `coverage_faults`).
//!
//! - `wrap_long`: a group file is what a digest job reads. Every line longer than `WRAP_AT` characters is broken
//!   with a single newline: an HTML table between rows (`</tr>`), prose at a sentence end, a list at a comma,
//!   else at a space. `prepare` refuses a group file that still has a line of `REFUSE_AT` characters or more.
//!   Donner 2026 `groups/g03.md` held a 3,862-character occupations list, Hickey eight tables of 2,257-6,414.
//! - `uncovered_pairs`: a translator that read only 2,000 characters of an English paragraph closes its Chinese
//!   at a clean full stop, so a check of how the Chinese ends misses the cut (donner2026industry blocks 277, 285,
//!   307, 401, 572). The coverage check compares what the English holds with what its Chinese holds: a note mark
//!   `$^{n}$` missing, a number of the English's last third missing, Han characters per English word below 0.75 x
//!   the book's median (paragraphs of 80 words or more), or a `……` the English does not have. `prepare` refuses
//!   a source with such a pair; a fill round of pdf-ocr-bilingual (`fillOnly`) retranslates the pairs it names.

use regex::Regex;
use std::sync::LazyLock;

/// A DeepSeek read shows this many characters of a line.
pub(crate) const READ_LIMIT: usize = 2000;
/// A staged file breaks longer lines.
pub(crate) const WRAP_AT: usize = 1500;
/// A staged file with a line this long is refused.
pub(crate) const REFUSE_AT: usize = 1800;

const MIN_WORDS: usize = 25;
const RATIO: f64 = 0.75;
const RATIO_WORDS: usize = 80;

static CJK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}\x{F900}-\x{FAFF}\x{3000}-\x{303F}\x{FF00}-\x{FFEF}]").unwrap());
static HAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{4E00}-\x{9FFF}]").unwrap());
static LATIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z]+").unwrap());
static WORD3: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z]{3,}").unwrap());
static ROMAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[ivxlcdm]+$").unwrap());
static LINK_TARGET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\]\([^)]*\)").unwrap());
static IMG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"!\[[^\]]*\]\(([^)\s]+)\)|<img[^>]*src="([^"]+)""#).unwrap());
static URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)<?(?:https?://|www\.)[^\s>)]+>?|\b(?:doi:\s*)?10\.\d{4,9}/[^\s)]+").unwrap());
static TAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static MATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)\$\$.*?\$\$|\$[^$\n]*\$").unwrap());
static NOTE_MARK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\s*\^\s*\{?\s*(\d{1,3})\s*\}?\s*\$|<sup>\s*(\d{1,3})\s*</sup>").unwrap());
static SUPERSCRIPT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[⁰¹²³⁴⁵⁶⁷⁸⁹]+").unwrap());
static EN_ELLIPSIS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\.\.\.|…|\. \. \.").unwrap());
static PERCENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bper\s?cent\b").unwrap());
static SCALE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(?:billion|million|thousand|trillion)").unwrap());
static BAD_START: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:\$\$|\$\^|```|~~~|<table|<details|</details|<summary|<img|<div|#|[-*+>|=](?:\s|$)|!\[|\d{1,3}[.)](?:\s|$)|$)").unwrap()
});
static LIST_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^#{1,6}\s*(?:(?:author|subject|name)\s+)?(?:index|references|bibliography)\s*$|^#{1,6}\s*(?:further|suggested|additional)\s+reading\s*$").unwrap()
});
static TAG_OPEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[A-Za-z/!]").unwrap());

// ---------------------------------------------------------------- the wrap guard

/// A break: the char index where the piece ends and the separator that the newline replaces.
fn breaks_at(chars: &[char], level: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let n = chars.len();
    let ends_with = |i: usize, s: &str| {
        let s: Vec<char> = s.chars().collect();
        i >= s.len() && chars[i - s.len()..i] == s[..]
    };
    for i in 1..n {
        match level {
            0 => {
                if ends_with(i, "</tr>") {
                    out.push((i, 0));
                }
                // after 。！？； (and closing quotes): no separator
                let mut j = i;
                while j > 0 && "”’」』）)".contains(chars[j - 1]) {
                    j -= 1;
                }
                if j > 0 && "。！？；".contains(chars[j - 1]) && (i == n || !"”’」』）)".contains(chars[i])) {
                    out.push((i, 0));
                }
                // ". " (a sentence end, maybe with a closing quote or a note mark): the space is the separator
                if chars[i] == ' ' {
                    let mut k = i;
                    let head: String = chars[..i].iter().collect();
                    if head.ends_with("}$")
                        && let Some(at) = head.rfind("$^{")
                    {
                        k = head[..at].chars().count();
                        while k > 0 && chars[k - 1] == ' ' {
                            k -= 1;
                        }
                    }
                    while k > 0 && "\"'”’)]".contains(chars[k - 1]) {
                        k -= 1;
                    }
                    if k > 0 && ".!?。！？；".contains(chars[k - 1]) {
                        out.push((i, 1));
                    }
                }
                // after a note mark that closes a Chinese sentence ("。$^{2}$"): no separator
                let head: String = chars[..i].iter().collect();
                if chars[i] != ' '
                    && head.ends_with("}$")
                    && let Some(at) = head.rfind("$^{")
                    && head[..at].trim_end().ends_with(['。', '！', '？', '；'])
                {
                    out.push((i, 0));
                }
            }
            1 => {
                if chars[i] == ' ' && i > 0 && ",;".contains(chars[i - 1]) {
                    out.push((i, 1));
                }
                if "，、".contains(chars[i - 1]) {
                    out.push((i, 0));
                }
            }
            2 => {
                if ends_with(i, "</td>") || ends_with(i, "</th>") {
                    out.push((i, 0));
                }
            }
            _ => {
                if chars[i] == ' ' {
                    out.push((i, 1));
                }
            }
        }
    }
    out
}

fn safe_cut(chars: &[char], cut: usize, sep: usize) -> bool {
    let rest: String = chars[cut + sep..].iter().collect();
    if rest.is_empty() || BAD_START.is_match(&rest) {
        return false; // the next line would open a list, heading, fence, table or image block
    }
    let head: String = chars[..cut].iter().collect();
    if head.ends_with("</tr>") || head.ends_with("</td>") || head.ends_with("</th>") {
        return true; // between table cells: no math spans them, and a "$" there is a currency sign
    }
    let dollars = head.char_indices().filter(|&(i, c)| c == '$' && !head[..i].ends_with('\\')).count();
    if dollars % 2 == 1 {
        return false; // inside inline math
    }
    let tag = TAG_OPEN.find_iter(&head).last().map(|m| m.start() as i64).unwrap_or(-1);
    if tag > head.rfind('>').map(|i| i as i64).unwrap_or(-1) {
        return false; // inside an HTML tag ("p<0.1" is no tag)
    }
    !head.trim().is_empty()
}

fn wrap_line(line: &str) -> Vec<String> {
    if line.trim_start().starts_with('|') {
        return vec![line.to_string()]; // a Markdown table row cannot break; the refusal catches a long one
    }
    let mut res = Vec::new();
    let mut chars: Vec<char> = line.chars().collect();
    while chars.len() > WRAP_AT {
        let mut cut = None;
        for level in 0..4 {
            let lo = if level < 2 { WRAP_AT / 2 } else { 1 };
            let best = breaks_at(&chars, level)
                .into_iter()
                .filter(|&(c, sep)| c >= lo && c <= WRAP_AT && c + sep < chars.len() && safe_cut(&chars, c, sep))
                .max_by_key(|&(c, _)| c);
            if best.is_some() {
                cut = best;
                break;
            }
        }
        let Some((c, sep)) = cut else { break };
        res.push(chars[..c].iter().collect());
        chars = chars[c + sep..].to_vec();
    }
    res.push(chars.into_iter().collect());
    res
}

/// `text` with every line longer than `WRAP_AT` characters broken by single newlines (code fences left alone).
pub(crate) fn wrap_long(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut fence = false;
    for line in text.split('\n') {
        if line.trim().starts_with("```") || line.trim().starts_with("~~~") {
            fence = !fence;
            out.push(line.to_string());
        } else if !fence && line.chars().count() > WRAP_AT {
            out.extend(wrap_line(line));
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// The longest line of `text`, in characters.
pub(crate) fn longest_line(text: &str) -> usize {
    text.split('\n').map(|l| l.chars().count()).max().unwrap_or(0)
}

// ---------------------------------------------------------------- the coverage check

fn plain(s: &str) -> String {
    let s = LINK_TARGET.replace_all(s, "] ");
    let s = IMG.replace_all(&s, " ");
    let s = URL.replace_all(&s, " ");
    let s = TAGS.replace_all(&s, " ");
    MATH.replace_all(&s, " ").into_owned()
}

fn note_marks(s: &str) -> std::collections::BTreeSet<u32> {
    let mut marks: std::collections::BTreeSet<u32> = NOTE_MARK
        .captures_iter(s)
        .filter_map(|c| c.get(1).or(c.get(2)).and_then(|m| m.as_str().parse().ok()))
        .collect();
    for m in SUPERSCRIPT.find_iter(s) {
        let digits: String = m.as_str().chars().map(|c| char::from_digit("⁰¹²³⁴⁵⁶⁷⁸⁹".chars().position(|x| x == c).unwrap() as u32, 10).unwrap()).collect();
        if let Ok(n) = digits.parse() {
            marks.insert(n);
        }
    }
    marks
}

/// The English words of a pair that the Chinese does not keep in Latin letters. A number is never a word.
fn english_words(en: &str, zh: &str) -> usize {
    let kept: std::collections::HashSet<&str> = LATIN.find_iter(zh).map(|m| m.as_str()).collect();
    LATIN.find_iter(&plain(en)).filter(|m| !kept.contains(m.as_str())).count()
}

/// The English words a translation must render in Han: [`english_words`] less "percent", which Chinese writes as
/// `%` (when the Chinese has one). Das & Grigorian 2026, line 473: five "percent" made a whole translation look
/// short.
fn pair_words(en: &str, zh: &str) -> usize {
    let en = once_each(en);
    if !zh.contains(['%', '％']) {
        return english_words(&en, zh);
    }
    english_words(&PERCENT.replace_all(&en, " "), zh)
}

/// `en` with a run the OCR repeated left out: an OCR line can hold a run of its own sentences twice (Sylla &
/// Cowen 2018, the letter to William Seton), and a translation renders them once. A sentence goes when it repeats
/// an earlier sentence of five words or more together with a different neighbour that repeats too; one sentence
/// said again and again is no OCR run.
fn once_each(en: &str) -> String {
    let mut sentences: Vec<&str> = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = en.char_indices().collect();
    for (k, &(i, c)) in chars.iter().enumerate() {
        let end = matches!(c, '.' | '!' | '?') && chars.get(k + 1).is_none_or(|&(_, n)| n.is_whitespace());
        if end || k + 1 == chars.len() {
            let stop = i + c.len_utf8();
            sentences.push(en[start..stop].trim());
            start = stop;
        }
    }
    let keys: Vec<Vec<String>> = sentences.iter().map(|s| LATIN.find_iter(s).map(|m| m.as_str().to_lowercase()).collect()).collect();
    let same = |a: usize, b: usize| keys[a] == keys[b];
    let repeated = |k: usize| {
        keys[k].len() >= 5
            && (0..k).any(|p| {
                same(p, k)
                    && ((k + 1 < keys.len() && p + 1 < k && same(p + 1, k + 1) && !same(k, k + 1))
                        || (p > 0 && same(p - 1, k - 1) && !same(k, k - 1)))
            })
    };
    sentences.iter().enumerate().filter(|&(k, _)| !repeated(k)).map(|(_, s)| *s).collect::<Vec<_>>().join(" ")
}

/// Han characters per English word of the pair, the words the translation must render in Han.
fn han_ratio(en: &str, zh: &str) -> Option<f64> {
    let w = pair_words(en, zh);
    (w >= MIN_WORDS).then(|| HAN.find_iter(zh).count() as f64 / w as f64)
}

/// The book's median is read on the plain word count, so the sign rule only raises a pair's own ratio and
/// never moves the line other pairs are measured against: it can clear a false alarm, never raise a new one.
/// (Read on the corrected count, the median of a percent-dense book rose and flagged whole pairs elsewhere,
/// for example Sylla & Cowen 2018 line 2560; a possessive-'s rule moved every median and flagged 14.)
fn median_ratio(en: &str, zh: &str) -> Option<f64> {
    let w = english_words(en, zh);
    (w >= MIN_WORDS).then(|| HAN.find_iter(zh).count() as f64 / w as f64)
}

/// The Chinese numeral of a whole number below 10,000 ("16" -> "十六"); empty otherwise.
fn zh_numeral(n: &str) -> String {
    let Ok(mut v) = n.parse::<u32>() else { return String::new() };
    if !(1..10000).contains(&v) {
        return String::new();
    }
    let d: Vec<char> = "零一二三四五六七八九".chars().collect();
    let (mut res, mut zero) = (String::new(), false);
    for (unit, size) in [("千", 1000), ("百", 100), ("十", 10), ("", 1)] {
        let q = v / size;
        v %= size;
        if q > 0 {
            if zero && !res.is_empty() {
                res.push('零');
            }
            res.push(d[q as usize]);
            res.push_str(unit);
            zero = false;
        } else if !res.is_empty() {
            zero = true;
        }
    }
    res.strip_prefix("一十").map(|r| format!("十{r}")).unwrap_or(res)
}

/// The numbers of `s` that a translation must keep: 2-4 digits, or a decimal; not part of a word ("COVID-19",
/// "2-digit"), not a grouped or scaled amount that Chinese writes in 万/亿.
fn numbers(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let word = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if !c[i].is_ascii_digit() || (i > 0 && (word(c[i - 1]) || ".,/-".contains(c[i - 1]))) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < c.len() && c[j].is_ascii_digit() {
            j += 1;
        }
        let int = j - i;
        let mut k = j;
        if k + 1 < c.len() && c[k] == '.' && c[k + 1].is_ascii_digit() {
            k += 1;
            while k < c.len() && c[k].is_ascii_digit() {
                k += 1;
            }
        }
        let decimal = k > j;
        // a dot-grouped amount ("2.000.000", "700.000"): thousands, as "300,000" is
        let grouped = decimal
            && k - j == 4
            && (c[j + 1..k].iter().all(|&d| d == '0') || (c.get(k) == Some(&'.') && c.get(k + 1).is_some_and(|ch| ch.is_ascii_digit())));
        if grouped {
            i = k;
            continue;
        }
        let fits = (2..=4).contains(&int) || (int == 1 && decimal);
        let next = c.get(k).copied();
        let after: String = c[k..].iter().collect();
        let blocked = next.is_some_and(word)
            || (next.is_some_and(|ch| ch == ',' || word(ch)) && c.get(k + 1).is_some_and(|ch| ch.is_ascii_digit()))
            || (next == Some('-') && c.get(k + 1).is_some_and(|ch| ch.is_ascii_alphabetic()))
            || SCALE.is_match(&after);
        if fits && !blocked {
            out.push(c[i..k].iter().collect());
        }
        i = k.max(i + 1);
    }
    out
}

/// Why `zh` does not cover `en` (empty when it does), with `median` the book's Han characters per English word.
pub(crate) fn coverage_faults(en: &str, zh: &str, median: Option<f64>) -> Vec<String> {
    let mut faults = Vec::new();
    let lost: Vec<String> = note_marks(en).difference(&note_marks(zh)).map(|n| n.to_string()).collect();
    if !lost.is_empty() {
        faults.push(format!("note {}", lost.join(",")));
    }
    let text = plain(&SUPERSCRIPT.replace_all(&NOTE_MARK.replace_all(en, " "), " "));
    let zh_digits: String = zh.chars().filter(|c| !matches!(c, ',' | '，') && !c.is_whitespace()).collect();
    let chars: Vec<char> = text.chars().collect();
    let tail: String = chars[2 * chars.len() / 3..].iter().collect();
    let missing: Vec<String> = numbers(&tail)
        .into_iter()
        .filter(|n| !zh_digits.contains(n.as_str()) && !(!zh_numeral(n).is_empty() && zh_digits.contains(&zh_numeral(n))))
        .collect();
    if !missing.is_empty() {
        faults.push(format!("number {}", missing.iter().take(5).cloned().collect::<Vec<_>>().join(",")));
    }
    if let (Some(r), Some(m)) = (han_ratio(en, zh), median)
        && r < RATIO * m
        && pair_words(en, zh) >= RATIO_WORDS
    {
        faults.push(format!("short {r:.2}/{m:.2}"));
    }
    // a leading …… carries on a sentence that the block before began
    let body = zh.trim_start_matches(['“', '‘', '"', '\'', '（', '(', ' ']);
    let body = body.strip_prefix("……").unwrap_or(body);
    // a closing …… carries the sentence on into the next block when the English block itself stops mid-sentence
    // (a page break: "long the world's largest by")
    let open = plain(en).trim_end().chars().last().is_some_and(|c| c.is_alphanumeric() || matches!(c, ',' | '—' | '–' | '-'));
    let closed = body.trim_end_matches(['”', '’', '"', '\'', '）', ')', ' ']);
    let body = if open { closed.strip_suffix("……").unwrap_or(body) } else { body };
    if body.contains("……") && !EN_ELLIPSIS.is_match(en) {
        faults.push("ellipsis".into());
    }
    faults
}

fn translatable(block: &str) -> bool {
    let s = block.trim();
    if CJK.is_match(s) || ["$$", "```", "<table", "![", "<img", "<details", "</details", "<summary"].iter().any(|p| s.starts_with(p)) {
        return false;
    }
    let s = TAGS.replace_all(&MATH.replace_all(&IMG.replace_all(s, " "), " "), " ").into_owned();
    WORD3.find_iter(&s).any(|w| !ROMAN.is_match(w.as_str()))
}

/// `(line of the Chinese block, faults)` of every English block of a bilingual text whose Chinese does not cover it.
pub(crate) fn uncovered_pairs(raw: &str) -> Vec<(usize, Vec<String>)> {
    // blocks with their first line number
    let mut blocks: Vec<(usize, String)> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut start = 0;
    for (n, line) in raw.split('\n').enumerate() {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                blocks.push((start, cur.join("\n")));
                cur.clear();
            }
        } else {
            if cur.is_empty() {
                start = n + 1;
            }
            cur.push(line);
        }
    }
    if !cur.is_empty() {
        blocks.push((start, cur.join("\n")));
    }
    let mut pairs: Vec<(usize, &str, String)> = Vec::new();
    // as pdfocr's mark_blocks: the OCR text inside <details> and the entries under an Index / References /
    // Bibliography / Further Reading heading never need Chinese
    let (mut inside, mut listing) = (false, false);
    for i in 0..blocks.len() {
        let b = blocks[i].1.trim_start();
        if b.starts_with("<details") {
            inside = true;
        }
        if b.starts_with('#') && !CJK.is_match(b) {
            listing = LIST_HEADING.is_match(b);
        }
        let skip = inside || listing;
        if blocks[i].1.contains("</details>") {
            inside = false;
        }
        if skip || !translatable(&blocks[i].1) {
            continue;
        }
        let mut j = i + 1;
        while j < blocks.len() && CJK.is_match(&blocks[j].1) {
            j += 1;
        }
        if j > i + 1 {
            let zh: Vec<&str> = blocks[i + 1..j].iter().map(|b| b.1.as_str()).collect();
            pairs.push((blocks[i + 1].0, &blocks[i].1, zh.join("\n\n")));
        }
    }
    let mut ratios: Vec<f64> = pairs.iter().filter_map(|(_, en, zh)| median_ratio(en, zh)).collect();
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = ratios.get(ratios.len() / 2).copied();
    pairs
        .iter()
        .filter_map(|(line, en, zh)| {
            let f = coverage_faults(en, zh, median);
            (!f.is_empty()).then(|| (*line, f))
        })
        .collect()
}

/// The folder of the hand judgements of coverage flags: `ZT_COVERAGE_JUDGED`, else `%APPDATA%\zt\coverage-judged`.
pub(crate) fn judged_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("ZT_COVERAGE_JUDGED")
        && !d.trim().is_empty()
    {
        return d.into();
    }
    std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or_else(|_| ".".into())).join("zt").join("coverage-judged")
}

/// The flags an operator has not judged `original`, and the lines of those dismissed. The judgement file is
/// `<dir>/<source sha256>.json`: `{"source_sha256", "judged": [{"line", "verdict": "original", "reason"}]}`;
/// each entry is the operator's reading of the pair against its English (a cut is fixed in the source, never
/// judged). No file: every flag stays.
pub(crate) fn dismiss_judged(
    flags: Vec<(usize, Vec<String>)>,
    dir: &std::path::Path,
    source_sha256: &str,
) -> anyhow::Result<(Vec<(usize, Vec<String>)>, Vec<usize>)> {
    let path = dir.join(format!("{source_sha256}.json"));
    let Ok(text) = std::fs::read_to_string(&path) else { return Ok((flags, Vec::new())) };
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{FEFF}')).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    if v["source_sha256"].as_str() != Some(source_sha256) {
        anyhow::bail!("{}: source_sha256 does not name this source ({source_sha256})", path.display());
    }
    let mut lines = std::collections::BTreeSet::new();
    for j in v["judged"].as_array().map(|a| a.as_slice()).unwrap_or_default() {
        let line = j["line"].as_u64().ok_or_else(|| anyhow::anyhow!("{}: an entry has no line", path.display()))? as usize;
        if j["verdict"].as_str() != Some("original") {
            anyhow::bail!("{}: line {line}: the verdict must be \"original\" (a cut is fixed in the source)", path.display());
        }
        if j["reason"].as_str().is_none_or(|r| r.trim().is_empty()) {
            anyhow::bail!("{}: line {line}: a judgement needs its reason", path.display());
        }
        lines.insert(line);
    }
    let (dismissed, kept): (Vec<_>, Vec<_>) = flags.into_iter().partition(|(line, _)| lines.contains(line));
    Ok((kept, dismissed.into_iter().map(|(l, _)| l).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn long() -> String {
        (0..48)
            .map(|i| format!("In the year {} the trade was cut by the rise of Islam, as Pirenne argued at length. ", 600 + i))
            .collect::<String>()
            + "$^{5}$ The end."
    }

    #[test]
    fn a_long_line_breaks_at_sentence_ends_and_keeps_every_character() {
        let w = wrap_long(&long());
        assert!(w.contains('\n') && longest_line(&w) <= WRAP_AT && !w.contains("\n\n"));
        assert_eq!(w.replace('\n', " "), long());
        assert!(w.split('\n').rev().skip(1).all(|l| l.ends_with('.')));
        assert_eq!(wrap_long("short"), "short");
    }

    #[test]
    fn a_break_keeps_a_note_mark_with_its_sentence() {
        let text = format!("{}the end of a sentence. $^{{2}}$ Rather, the next sentence {}", "word ".repeat(280), "word ".repeat(200));
        let w = wrap_long(&text);
        let lines: Vec<&str> = w.split('\n').collect();
        assert!(lines.len() > 1 && lines[0].ends_with("sentence. $^{2}$") && !lines.iter().any(|l| l.starts_with("$^")));
        let zh = format!("{}这是一句话。$^{{3}}${}", "字".repeat(1300), "字".repeat(600));
        let w = wrap_long(&zh);
        assert!(w.split('\n').next().unwrap().ends_with("。$^{3}$") && w.replace('\n', "") == zh);
    }

    #[test]
    fn a_long_list_breaks_at_commas_and_a_table_between_rows() {
        let list = (0..400).map(|i| format!("item{i}")).collect::<Vec<_>>().join(", ");
        let w = wrap_long(&list);
        assert!(longest_line(&w) <= WRAP_AT && w.split('\n').rev().skip(1).all(|l| l.ends_with(',')));
        assert_eq!(w.replace('\n', " "), list);
        let zh = "农民、工匠、商人".repeat(300);
        assert!(longest_line(&wrap_long(&zh)) <= WRAP_AT && wrap_long(&zh).replace('\n', "") == zh);
        let table = format!("<table>{}</table>", (0..80).map(|i| format!("<tr><td>${i}</td><td>{}</td></tr>", "x".repeat(40))).collect::<String>());
        let w = wrap_long(&table);
        assert!(longest_line(&w) <= WRAP_AT && w.split('\n').rev().skip(1).all(|l| l.ends_with("</tr>")));
        assert_eq!(w.replace('\n', ""), table);
    }

    #[test]
    fn a_line_without_a_safe_break_stays_and_is_seen() {
        let w = wrap_long(&"x".repeat(1900));
        assert!(longest_line(&w) >= REFUSE_AT);
        let text = format!("{}- not a list {}", "word ".repeat(290), "word ".repeat(300));
        assert!(wrap_long(&text).split('\n').skip(1).all(|l| !l.starts_with("- ")));
    }

    #[test]
    fn numbers_and_numerals() {
        assert_eq!(numbers("in 1929, p. 270–271, COVID-19, 2-digit, $12.3 billion, 300,000 and 4.5 here"), vec!["1929", "270", "271", "4.5"]);
        assert_eq!(zh_numeral("16"), "十六");
        assert_eq!(zh_numeral("105"), "一百零五");
        assert_eq!(zh_numeral("2014"), "二千零一十四");
        assert_eq!(zh_numeral("20000"), "");
    }

    #[test]
    fn the_coverage_rules_one_by_one() {
        let en = "The tax was raised in many provinces of the empire and the governors of the time wrote long letters about \
                  it to the caliph, asking for more troops and more money, and the caliph answered in 1929 with a decree. $^{7}$";
        let full = "这项税在帝国的许多行省征收，当时的总督们为此给哈里发写了很长的信，请求更多的军队和更多的钱，哈里发在1929年以一道法令作答。$^{7}$";
        assert!(coverage_faults(en, full, Some(1.6)).is_empty());
        assert_eq!(coverage_faults(en, &full.replace("$^{7}$", ""), Some(1.6)), vec!["note 7"]);
        assert!(coverage_faults(en, &full.replace("1929", "一九二九"), Some(1.6))[0].starts_with("number"));
        assert_eq!(coverage_faults(en, &full.replace("当时的总督们", "……"), Some(1.6)), vec!["ellipsis"]);
        assert!(coverage_faults(en, &format!("……{full}"), Some(1.6)).is_empty());
        assert!(coverage_faults(en, &full.replace("$^{7}$", "⁷"), Some(1.6)).is_empty());
        assert!(coverage_faults("In Chapter 16 we study the model.", "在第十六章中我们研究该模型。", Some(1.6)).is_empty());
        let para = vec!["The governors of the provinces wrote to the caliph about the tax and the troops."; 8].join(" ");
        let half = "各行省的总督们就税收和军队问题写信给哈里发。".repeat(3);
        assert!(coverage_faults(&para, &half, Some(1.6)).iter().any(|f| f.starts_with("short")));
        assert!(!coverage_faults(&para, &half.repeat(3), Some(1.6)).iter().any(|f| f.starts_with("short")));
    }

    /// Chinese writes "percent" as `%`, with no Han of its own, so it does not count as an English word in the
    /// length ratio; a Chinese that leaves half the paragraph out is still short.
    #[test]
    fn a_word_that_chinese_writes_as_a_sign_does_not_count_in_the_ratio() {
        let en = vec!["The country's exports fell by 60 percent and its remittances by 33 percent in the first half of the year."; 6].join(" ");
        let zh = "该国出口下降60%，侨汇在上半年下降33%。".repeat(6);
        // 14 Han for 19 words is short against 0.75 × 1.0; for the 17 words a translation renders in Han it is not.
        assert!(!coverage_faults(&en, &zh, Some(1.0)).iter().any(|f| f.starts_with("short")), "{:?}", coverage_faults(&en, &zh, Some(1.0)));
        let cut = "该国出口下降60%，侨汇在上半年下降33%。".repeat(3);
        assert!(coverage_faults(&en, &cut, Some(1.0)).iter().any(|f| f.starts_with("short")));
        // Without the sign in the Chinese, "percent" is a word the translation had to render.
        assert_eq!(pair_words("fell by 60 percent", "下降了"), 3);
        assert_eq!(pair_words("fell by 60 percent", "下降60%"), 2);
        assert_eq!(pair_words("fell by 60 per cent", "下降60%"), 2);
    }

    /// The Das & Grigorian (CGD 2026) pair at line 473 that `prepare` refused: its Chinese is whole, and the
    /// English is dense with "percent", which the Chinese writes as `%`.
    #[test]
    fn the_das_jamaica_paragraph_is_covered() {
        // book text, so not in the repository: the Scholar Archive mirror holds it
        let path = "C:/Users/chenx/AppData/Roaming/zt/scholar-archive/files/1910/Das_Grigorian_2026_Domestic_Debt_Restructuring_Central_Bank_EMDEs_CGD.zh-bilingual.md";
        let Ok(raw) = std::fs::read_to_string(path) else {
            eprintln!("skipped: no {path}");
            return;
        };
        let found = uncovered_pairs(&raw);
        assert!(found.is_empty(), "{found:?}");
    }

    /// A `……` that closes the Chinese of an English block cut at a page break mid-sentence carries the sentence on
    /// into the next block, as a leading one does; after an English sentence that ends, it is still a cut.
    #[test]
    fn a_closing_ellipsis_after_an_english_page_break_carries_on() {
        let en = "The New York Stock Exchange, long the world's largest by";
        let zh = "纽约证券交易所长期以来一直是世界上最大的……";
        // too short for the ratio, so only the ellipsis rule speaks
        assert!(coverage_faults(en, zh, Some(1.5)).is_empty(), "{:?}", coverage_faults(en, zh, Some(1.5)));
        assert!(coverage_faults("Some were for a division of the power—others were for ", "有些人主张划分征税权——另一些人则主张……", Some(1.5)).is_empty());
        assert_eq!(coverage_faults("The exchange was founded in New York.", "该交易所……", Some(1.5)), vec!["ellipsis"]);
        assert_eq!(coverage_faults("The exchange, long the largest by", "该交易所……成立于纽约。", Some(1.5)), vec!["ellipsis"]);
    }

    /// Hamilton writes thousands with dots ("2.000.000 of Dollars", "700.000 Dollars"); the Chinese writes
    /// 2,000,000 or 70万. A grouped amount is no number a translation must copy, but a decimal still is.
    #[test]
    fn a_dot_grouped_amount_is_no_decimal() {
        assert_eq!(numbers("will amount to 2.000.000 of Dollars, 700.000 Dollars and 50.000 Men"), Vec::<String>::new());
        assert_eq!(numbers("a rate of 1.500 and 4.5 here"), vec!["1.500", "4.5"]);
    }

    /// An OCR line that holds its own sentences twice (Sylla & Cowen 2018, the letter to William Seton) is short
    /// against a whole translation of it; each sentence counts once.
    #[test]
    fn a_sentence_the_ocr_repeated_counts_once() {
        let once = "I am under a necessity of authorizing the Treasurer to draw upon you for One hundred thousand Dollars. \
                    It is a necessary aid to the Bank which feels more than you do the effects of certain machinations.";
        let en = format!("{once} {once} I may be compelled to go further; but it will be on the same conditions of the loan.");
        let zh = "我不得不授权司库向您开出十万美元的汇票。这是对合众国银行的一项必要援助，该行比您更深切地感受到某些阴谋的影响。我可能被迫采取进一步行动；但条件将与这笔借款相同。";
        assert_eq!(pair_words(&en, zh), english_words(&format!("{once} I may be compelled to go further; but it will be on the same conditions of the loan."), zh));
    }

    /// The Sylla & Cowen 2018 source (Scholar Archive file 1893) that `prepare` refused on 2026-11-17 with 11
    /// pairs, every one whole against its English: the rules clear five (three page-break ellipses, two
    /// dot-grouped amounts) and the OCR-repeated run; the five legal pairs below the ratio are left to the hand.
    #[test]
    fn the_sylla_cowen_hamilton_book_leaves_only_its_legal_pairs() {
        let path = "C:/Users/chenx/AppData/Roaming/zt/scholar-archive/files/1893/Sylla_Cowen_2018_Hamilton_on_Finance_Credit_and_Debt_CUP.zh-bilingual.md";
        let Ok(raw) = std::fs::read_to_string(path) else {
            eprintln!("skipped: no {path}");
            return;
        };
        let lines: Vec<usize> = uncovered_pairs(&raw).into_iter().map(|(l, _)| l).collect();
        assert_eq!(lines, vec![2438, 2600, 3354, 3394, 7373]);
    }

    /// An operator judges each flag by hand against its English: a cut is fixed in the source, a pair that only
    /// renders the original text is recorded as `original` with its reason in the judgement file named after the
    /// source's SHA-256, and only those flags are dismissed. A changed source has another hash, so no judgement
    /// carries over to it.
    #[test]
    fn a_flag_judged_original_by_hand_is_dismissed() {
        let dir = std::env::temp_dir().join(format!("zt-judged-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let flags = vec![(10, vec!["short 1.0/1.5".to_string()]), (20, vec!["ellipsis".to_string()]), (30, vec!["number 12".to_string()])];
        // no file: every flag stays
        let (kept, dismissed) = dismiss_judged(flags.clone(), &dir, "abc").unwrap();
        assert_eq!((kept.len(), dismissed.len()), (3, 0));
        std::fs::write(
            dir.join("abc.json"),
            r#"{"source_sha256":"abc","judged":[{"line":10,"verdict":"original","reason":"legal English, whole Chinese"},{"line":20,"verdict":"original","reason":"page break"},{"line":99,"verdict":"original","reason":"stale"}]}"#,
        )
        .unwrap();
        let (kept, dismissed) = dismiss_judged(flags.clone(), &dir, "abc").unwrap();
        assert_eq!(kept, vec![(30, vec!["number 12".to_string()])]);
        assert_eq!(dismissed, vec![10, 20]);
        // another source hash reads no judgement
        assert_eq!(dismiss_judged(flags.clone(), &dir, "def").unwrap().0.len(), 3);
        // a judgement without a reason, of an unknown verdict or of another source is refused
        std::fs::write(dir.join("abc.json"), r#"{"source_sha256":"abc","judged":[{"line":10,"verdict":"original","reason":" "}]}"#).unwrap();
        assert!(dismiss_judged(flags.clone(), &dir, "abc").is_err());
        std::fs::write(dir.join("abc.json"), r#"{"source_sha256":"abc","judged":[{"line":10,"verdict":"fine","reason":"x"}]}"#).unwrap();
        assert!(dismiss_judged(flags.clone(), &dir, "abc").is_err());
        std::fs::write(dir.join("abc.json"), r#"{"source_sha256":"zzz","judged":[]}"#).unwrap();
        assert!(dismiss_judged(flags, &dir, "abc").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Every flag of the books the cut repairs read (2026-11-12, `D:/zt/backups/cut-repair-2026-11-12`), written
    /// to `ZT_COVERAGE_OUT` so two builds can be compared: `cargo test -- --ignored coverage_flags_of_the_cut_repair_books`.
    #[test]
    #[ignore]
    fn coverage_flags_of_the_cut_repair_books() {
        let dir = "D:/zt/backups/cut-repair-2026-11-12";
        let out = std::env::var("ZT_COVERAGE_OUT").expect("ZT_COVERAGE_OUT");
        let mut names: Vec<_> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".zh-bilingual.md.before-cut-repair-")).collect();
        names.sort();
        let mut lines = Vec::new();
        for name in names {
            let raw = std::fs::read_to_string(format!("{dir}/{name}")).unwrap();
            for (line, faults) in uncovered_pairs(&raw) {
                lines.push(format!("{name}\t{line}\t{}", faults.join(";")));
            }
        }
        std::fs::write(out, lines.join("\n")).unwrap();
    }

    #[test]
    fn the_five_donner_cuts_are_found_and_nothing_else_on_their_pages() {
        // book text, so not in the repository: the pdf-ocr-bilingual tests keep the excerpt beside them
        let path = "E:/google_scholar_archive/workflows/pdf-ocr-bilingual/tests/data/donner2026_cuts.md";
        let Ok(raw) = std::fs::read_to_string(path) else {
            eprintln!("skipped: no {path}");
            return;
        };
        let raw = raw.as_str();
        let found = uncovered_pairs(raw);
        let lines: Vec<&str> = raw.split('\n').collect();
        let english_of = |zh_line: usize| {
            // the English block ends on the last non-blank line before the Chinese block
            let mut k = zh_line - 2;
            while lines[k].trim().is_empty() {
                k -= 1;
            }
            lines[k].to_string()
        };
        let starts = [
            ("Economic history as a field", "note 3"),
            ("It was from a controversy within", "note 5,6"),
            ("The study of the Near East since", "note 16"),
            ("Economic historians of ancient", "note 11,12"),
            ("exceeded urbanization rates", "ellipsis"),
        ];
        assert_eq!(found.len(), 5, "{found:?}");
        for (start, fault) in starts {
            let hit = found.iter().find(|(line, _)| english_of(*line).starts_with(start)).unwrap_or_else(|| panic!("{start}: {found:?}"));
            assert!(hit.1.iter().any(|f| f == fault), "{start}: {:?}", hit.1);
        }
    }
}

