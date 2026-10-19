//! `zt-check`: the Card language rules (R2 to R19) and the Card layer rules
//! (I, E, F, L) of the book digest, run in-process on Cards read straight from
//! the archive. It is the one Card quality check of the digest workflows and
//! the only source of truth: every agent and script calls it. It replaced the
//! PowerShell checkers card-style-check.ps1 and self-check.ps1 (deleted on
//! 2026-10-15) with the same faults, in the same order, in the same words;
//! comments below that name them describe where a behaviour came from.
//!
//! The checks keep PowerShell's text semantics where they differ from Rust's:
//! a length counts UTF-16 code units, `-match` and `-replace` ignore case, a
//! string comparison skips the characters the culture ignores, a hashtable key
//! ignores case, and `Sort-Object` / `Group-Object` order by the culture's
//! collation. Text is first put in a "UTF-16 form": each character outside the
//! Basic Multilingual Plane becomes two private-use stand-ins, one per surrogate,
//! so a regex class, a count, and a cut see what .NET sees. A fault line turns
//! the stand-ins back into the character.

use crate::mcp::Zt;
use anyhow::{Context, Result, bail};
use regex::{Captures, Regex};
use serde_json::{Map, Value, json};
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use std::path::Path;
use std::sync::LazyLock;

macro_rules! pattern {
    ($name:ident, $pattern:expr) => {
        static $name: LazyLock<Regex> =
            LazyLock::new(|| Regex::new($pattern).expect("a valid check pattern"));
    };
}

// Prose: what the rules apply to.
pattern!(DISPLAY_MATH, r"\$\$[\s\S]*?\$\$");
pattern!(INLINE_MATH, r"\$[^$\n]+\$");
pattern!(LINK, r"\[\[[^\]]+\]\]");
pattern!(LINK_TARGET, r"\[\[([^\]]+)\]\]");
pattern!(CODE, r"`[^`\n]*`");
pattern!(BOLD, r"\*\*|__");
pattern!(HIDE, r"\$\$[\s\S]*?\$\$|\$[^$\n]+\$|\[\[[^\]]+\]\]");
pattern!(HIDDEN, r"§(\d+)§");
pattern!(MEASURE_STRIP, r#"[\s“”"「」（）()《》]"#);
pattern!(PLACEHOLDERS, r"(?i)F|C");
pattern!(SPACE, r"\s");
pattern!(SPACES, r"\s+");
pattern!(CJK, r"[㐀-鿿豈-﫿]");
// .NET's \w: letters, nonspacing marks, decimal digits, connector punctuation.
pattern!(WORD, r"[\p{L}\p{Mn}\p{Nd}\p{Pc}]");
pattern!(FENCE, r"^(```|~~~)");
pattern!(HEADING, r"^#{1,6}\s");
pattern!(DISPLAY_LINE, r"^\$\$.*\$\$$");
pattern!(STEP, r"^\d+[.、)]");
pattern!(MARKER, r"^([-*+]|\d+[.、)])\s*");
// The rules.
pattern!(SEMICOLON, r"[；;]");
pattern!(LIGHT_VERB, r"(进行|加以|予以|作出|做出)[㐀-鿿豈-﫿]");
pattern!(
    NOUN_PHRASE,
    r"的[^，。、：；！？,.;:!?\s的]{0,7}的[^，。、：；！？,.;:!?\s的]{0,7}的"
);
pattern!(DERIVATION, r"(?i)^derivation");
// The standalone text rules (R20, R21). Not every digit and not every 第 is a
// local reference: 第一福利定理 is a name, and a label with the content beside
// it is a statement.
pattern!(LOCAL_EQUATION, r"式\s*[（(]\s*[0-9A-Za-z][0-9A-Za-z.]*\s*[)）]");
pattern!(
    LOCAL_NAMED_NUMBER,
    r"(命题|引理|定理|推论|模型)\s*[（(]?\s*\d+(?:[.．][0-9A-Za-z]+)*"
);
pattern!(LOCAL_CHAPTER, r"第\s*\d+\s*[章节]");
pattern!(LOCAL_THIS, r"本节|本章");
pattern!(LOCAL_SIGN, r"§\s*\d+(?:[.．][0-9A-Za-z]+)*");
pattern!(SECTION, r"^\*{0,2}(思路|起点|步骤|结果|解读)\*\*\s*$");
pattern!(POSITION, r"^第 \d+ 张，共 \d+ 张");
pattern!(NOTE, r"^说明\s*[：:]");
pattern!(NOTE_LABEL, r"^说明\s*[：:]\s*");
pattern!(SOURCE_LABEL, r"^来源\s*[：:]");
// The term table.
pattern!(TABLE_RULE, r"^-+$|^:?-+:?$");
pattern!(AVOID_SPLIT, r"[、,，/]");
pattern!(AVOID_NONE, r"^[-—无]+$");
// The Card layer rules.
pattern!(
    CLAIM_VERB,
    r"是|会|能|让|使|要|不|没|把|被|决定|导致|取决|依赖|提高|降低|减少|增加|等于|大于|小于|高于|低于|来自|需要|带来|引起|产生|改变|保证|排除|允许|阻止|证明|说明|表明"
);
pattern!(BOOK_CARD, r"^[^\d/|][^/|]*$");
pattern!(BOOK_ANCHOR, r"^[^\d/|][^/|]*/\d+$");
pattern!(SOURCE_LINE, r"(?i)^\s*(来源|出处|Source)\s*[：:]");
pattern!(REASON_STRIP, r"[\s\p{P}\p{S}]");
pattern!(NOT_LETTER_OR_NUMBER, r"[^\p{L}\p{N}]");
pattern!(
    DATE,
    r"(1[0-9]{3}|20[0-9]{2})|[0-9一二三四五六七八九十]+\s*(年|世纪)"
);
pattern!(YEAR, r"(1[0-9]{3}|20[0-9]{2})");
// self-check.ps1.
pattern!(FAULT_LINE, r"^\S+ ([RIELF]\d+) ");
pattern!(LIST_ITEM, r"^\s*([-*+]|\d+[.、)])\s+");
pattern!(LIST_SPLIT, r"[,\s]+");
// What ICU collation (and so a culture string comparison) skips: control and
// format characters, and variation selectors.
pattern!(
    IGNORABLE,
    r"[\x{0}-\x{8}\x{E}-\x{1F}\x{7F}-\x{84}\x{86}-\x{9F}\p{Cf}\x{FE00}-\x{FE0F}]"
);
pattern!(MARK, r"^\p{M}");

const HEDGES: [&str; 18] = [
    "值得注意的是",
    "需要注意的是",
    "在某种程度上",
    "某种意义上",
    "在一定程度上",
    "不难看出",
    "显而易见",
    "毋庸置疑",
    "众所周知",
    "可以说",
    "精妙",
    "巧妙",
    "深刻",
    "精彩",
    "极其重要",
    "非常重要",
    "十分重要",
    "优雅",
];

/// The labels of an Event Card (`时间：…`).
static EVENT_LABELS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    ["时间", "地点", "参与者", "经过", "来源"]
        .into_iter()
        .map(|label| {
            let pattern = format!(r"(?m)^\s*[-*]?\s*(\*\*)?{label}(\*\*)?\s*[：:]");
            (label, Regex::new(&pattern).expect("a label pattern"))
        })
        .collect()
});

/// The labels of a Finding Card (`测量：…`): what was measured, the data
/// (sample, period, place), the method, the result, its limits, the source.
static FINDING_LABELS: LazyLock<Vec<(&str, Regex)>> = LazyLock::new(|| {
    ["测量", "数据", "方法", "结果", "局限", "来源"]
        .into_iter()
        .map(|label| {
            let pattern = format!(r"^\s*[-*]?\s*(\*\*)?{label}(\*\*)?\s*[：:]");
            (label, Regex::new(&pattern).expect("a label pattern"))
        })
        .collect()
});

const SECTIONS: [&str; 5] = ["思路", "起点", "步骤", "结果", "解读"];

/// R20's named constant: the characters of content a sentence must carry beside
/// a local label. `式 (12) 的加成项来自需求弹性` states content; `式 (12) 有该项`
/// does not. It is calibrated on annotated examples before it can block (D294).
const LOCAL_REFERENCE_CONTENT: i64 = 5;

/// The deictic phrases R21 reports when no named Link is in the same sentence.
/// It is a detector of two things it can see, never a Chinese coreference engine.
const DEICTIC: [&str; 6] = ["那里", "那儿", "那张卡", "这张卡", "该卡", "该卡片"];

/// The rules self-check.ps1 reports by default: every hard rule but R6.
const DEFAULT_RULES: &str =
    "R2,R3,R4,R5,R7,R9,R12,R13,R14,R15,R16,R17,R20,R21,I1,I2,I3,I4,I7,L1,L2,E1,E2,F1,F2,F3";

const USAGE: &str = "usage:
  zt-check --json <file> [--setting <address>] [--level concept|formal] [--require-explain] [limits]
  zt-check --model <address> [--skip-shared] [--setting <address>] [--level concept|formal] [--require-explain] [limits]
  zt-check --addresses <a,b,...> [--setting <address>] [--level concept|formal] [--require-explain] [limits]
  zt-check --cards <a,b,...> [--log <file>] [--book <key>] [--rules R2,R3,...] [limits]
limits: --max-sentences 12 --max-chars 400 --max-steps 6 --max-explain-sentences 16
        --max-explain-chars 600 --min-idea-chars 40 --max-idea-chars 300 --min-reason 8";

// --- UTF-16 form ------------------------------------------------------------------------------

const HIGH_STAND_IN: u32 = 0xE000;
const LOW_STAND_IN: u32 = 0xE400;

fn is_high_stand_in(ch: char) -> bool {
    (HIGH_STAND_IN..LOW_STAND_IN).contains(&(ch as u32))
}

fn is_low_stand_in(ch: char) -> bool {
    (LOW_STAND_IN..LOW_STAND_IN + 0x400).contains(&(ch as u32))
}

/// The text with each character outside the BMP as two stand-ins, so one
/// `char` is one UTF-16 code unit.
fn utf16_form(text: &str) -> String {
    if text.chars().all(|ch| (ch as u32) < 0x10000) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let mut buffer = [0u16; 2];
        let units = ch.encode_utf16(&mut buffer);
        if units.len() == 1 {
            out.push(ch);
        } else {
            for unit in units.iter() {
                let stand_in = HIGH_STAND_IN + (u32::from(*unit) - 0xD800);
                out.push(char::from_u32(stand_in).expect("a private-use character"));
            }
        }
    }
    out
}

/// The text with each pair of stand-ins back as its character.
fn from_utf16_form(text: &str) -> String {
    if !text.chars().any(is_high_stand_in) {
        return text.to_string();
    }
    let chars = text.chars().collect::<Vec<_>>();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if is_high_stand_in(ch)
            && chars
                .get(index + 1)
                .is_some_and(|next| is_low_stand_in(*next))
        {
            let high = 0xD800 + (ch as u32 - HIGH_STAND_IN);
            let low = 0xDC00 + (chars[index + 1] as u32 - LOW_STAND_IN);
            let code = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
            index += 2;
        } else {
            out.push(ch);
            index += 1;
        }
    }
    out
}

/// A length as .NET counts it (the text is in UTF-16 form).
fn length(text: &str) -> i64 {
    text.chars().count() as i64
}

// --- PowerShell comparisons -------------------------------------------------------------------

/// The text without the characters a culture comparison ignores.
fn culture(text: &str) -> Cow<'_, str> {
    IGNORABLE.replace_all(text, "")
}

/// `-eq` on strings: case and ignorable characters do not count.
fn ps_eq(a: &str, b: &str) -> bool {
    a == b || culture(a).to_lowercase() == culture(b).to_lowercase()
}

fn ps_in(value: &str, list: &[impl AsRef<str>]) -> bool {
    list.iter().any(|item| ps_eq(value, item.as_ref()))
}

/// `String.StartsWith(string)`: a culture comparison, so an ignorable
/// character before the prefix does not count, and a combining mark after it
/// breaks the match.
fn culture_starts_with(text: &str, prefix: &str) -> bool {
    let text = culture(text);
    text.starts_with(prefix) && !MARK.is_match(&text[prefix.len()..])
}

/// A hashtable key: hashtables ignore case.
fn key(text: &str) -> String {
    text.to_lowercase()
}

/// The primary collation weight of a character (ICU root order for ASCII).
fn weight(ch: char) -> u32 {
    const PUNCTUATION: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
    if ch.is_whitespace() {
        1
    } else if let Some(at) = PUNCTUATION.find(ch) {
        10 + at as u32
    } else if ch.is_ascii_digit() {
        100 + (ch as u32 - '0' as u32)
    } else if ch.is_ascii_lowercase() {
        200 + (ch as u32 - 'a' as u32)
    } else {
        1000 + ch as u32
    }
}

/// `Sort-Object` order: the culture's collation, ignoring case.
fn culture_cmp(a: &str, b: &str) -> Ordering {
    let weights = |text: &str| {
        culture(text)
            .chars()
            .flat_map(char::to_lowercase)
            .map(weight)
            .collect::<Vec<_>>()
    };
    weights(a).cmp(&weights(b))
}

// --- JSON values as PowerShell reads them -----------------------------------------------------

/// A property of an entry; PowerShell property names ignore case.
fn prop<'a>(card: &'a Value, name: &str) -> Option<&'a Value> {
    let object = card.as_object()?;
    object.get(name).or_else(|| {
        object
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
    })
}

/// `[string]$value`.
fn ps_string(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(true)) => "True".to_string(),
        Some(Value::Bool(false)) => "False".to_string(),
        Some(Value::Number(number)) => {
            if let Some(int) = number.as_i64() {
                int.to_string()
            } else if let Some(int) = number.as_u64() {
                int.to_string()
            } else {
                let float = number.as_f64().unwrap_or_default();
                if float.fract() == 0.0 && float.abs() < 1e15 {
                    format!("{}", float as i64)
                } else {
                    format!("{float}")
                }
            }
        }
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| ps_string(Some(item)))
            .collect::<Vec<_>>()
            .join(" "),
        Some(Value::Object(_)) => String::new(),
    }
}

/// `[string]$value`, in UTF-16 form.
fn text(value: Option<&Value>) -> String {
    utf16_form(&ps_string(value))
}

/// Whether PowerShell reads a value as true.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|float| float != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => match items.as_slice() {
            [] => false,
            [one] => truthy(one),
            _ => true,
        },
        Value::Object(_) => true,
    }
}

/// `@($value | Where-Object { $_ } | ForEach-Object { [string]$_ })`.
fn strings(value: Option<&Value>) -> Vec<String> {
    let items = match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items.iter().collect(),
        Some(other) => vec![other],
    };
    items
        .into_iter()
        .filter(|item| truthy(item))
        .map(|item| text(Some(item)))
        .collect()
}

/// `[int]$value` (null is 0; a fraction rounds half to even).
fn ps_int(value: Option<&Value>) -> i64 {
    match value {
        None | Some(Value::Null) => 0,
        Some(Value::Bool(flag)) => i64::from(*flag),
        Some(Value::Number(number)) => number
            .as_i64()
            .unwrap_or_else(|| number.as_f64().unwrap_or_default().round_ties_even() as i64),
        Some(Value::String(text)) => text.trim().parse().unwrap_or(0),
        Some(other) => i64::from(truthy(other)),
    }
}

// --- prose ------------------------------------------------------------------------------------

/// The text without what the rules do not cover; a formula keeps a placeholder
/// so it still splits nothing.
fn plain(text: &str) -> String {
    let text = DISPLAY_MATH.replace_all(text, "F");
    let text = INLINE_MATH.replace_all(&text, "F");
    let text = LINK.replace_all(&text, "");
    let text = CODE.replace_all(&text, "C");
    BOLD.replace_all(&text, "").into_owned()
}

struct Measure {
    size: i64,
    words: bool,
}

impl Measure {
    fn unit(&self) -> &'static str {
        if self.words { "words" } else { "characters" }
    }
}

/// A sentence's size: characters when it holds Chinese, else words.
fn measure(sentence: &str) -> Measure {
    let plain = plain(sentence);
    if CJK.is_match(sentence) {
        let stripped = MEASURE_STRIP.replace_all(&plain, "");
        let stripped = PLACEHOLDERS.replace_all(&stripped, "");
        return Measure {
            size: length(&stripped),
            words: false,
        };
    }
    Measure {
        size: SPACES
            .split(&plain)
            .filter(|word| WORD.is_match(word))
            .count() as i64,
        words: true,
    }
}

struct ProseLine {
    text: String,
    step: bool,
}

/// The lines of prose in a body, each with whether it is a numbered step.
fn prose_lines(body: &str) -> Vec<ProseLine> {
    let mut out = Vec::new();
    let mut fence = false;
    let mut display = false;
    for raw in body.split('\n') {
        let trim = raw.trim_end().trim();
        if FENCE.is_match(trim) {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        if ps_eq(trim, "$$") {
            display = !display;
            continue;
        }
        if display {
            continue;
        }
        if trim.is_empty()
            || culture_starts_with(trim, "|")
            || culture_starts_with(trim, ">")
            || culture_starts_with(trim, "<")
            || culture_starts_with(trim, "![")
            || HEADING.is_match(trim)
            || DISPLAY_LINE.is_match(trim)
        {
            continue;
        }
        out.push(ProseLine {
            text: MARKER.replace(trim, "").into_owned(),
            step: STEP.is_match(trim),
        });
    }
    out
}

/// Exact structural checks retain authored text and original body line numbers.
/// They do not rewrite Chinese characters or compare paraphrases.
fn corruption(body: &str, title: &str, address: &str, faults: &mut Faults) {
    fn whole_quote(text: &str) -> bool {
        [
            ("“", "”"),
            ("「", "」"),
            ("『", "』"),
            ("«", "»"),
            ("\"", "\""),
            ("'", "'"),
        ]
        .iter()
        .any(|(open, close)| {
            text.starts_with(open) && text.ends_with(close) && text.len() > open.len() + close.len()
        })
    }
    fn heading(line: &str) -> Option<&str> {
        let rest = line.strip_prefix("# ")?.trim();
        let without = rest.trim_end_matches('#');
        if without.len() < rest.len() && without.chars().last().is_some_and(char::is_whitespace) {
            Some(without.trim_end())
        } else {
            Some(rest)
        }
    }
    fn flush(
        paragraph: &mut Vec<(usize, &str)>,
        seen: &mut HashMap<String, (usize, usize)>,
        address: &str,
        faults: &mut Faults,
    ) {
        if paragraph.is_empty() {
            return;
        }
        let start = paragraph.first().unwrap().0;
        let end = paragraph.last().unwrap().0;
        let text = paragraph
            .iter()
            .map(|(_, line)| *line)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        paragraph.clear();
        let probe = INLINE_MATH.replace_all(&text, "");
        let probe = LINK.replace_all(&probe, "");
        let probe = CODE.replace_all(&probe, "");
        let probe = BOLD.replace_all(&probe, "");
        if matches!(probe.trim(), "待写" | "待补" | "TODO" | "TBD" | "..." | "…")
            || (text.lines().count() == 1
                && (text.starts_with("来源：")
                    || text.starts_with("来源:")
                    || text.to_ascii_lowercase().starts_with("source:")))
            || !(CJK.is_match(&probe) || WORD.is_match(&probe))
        {
            return;
        }
        if let Some((first, last)) = seen.get(&text) {
            faults.add(
                address,
                "R18",
                &format!("repeated prose paragraph at lines {first}-{last} and {start}-{end}"),
            );
        } else {
            seen.insert(text, (start, end));
        }
    }
    let normalized = body.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<_> = normalized.split('\n').collect();
    let mut quoted = HashSet::new();
    let mut start = 0;
    for end in 0..=lines.len() {
        if end == lines.len() || lines[end].trim().is_empty() {
            if whole_quote(lines[start..end].join("\n").trim()) {
                quoted.extend(start..end);
            }
            start = end + 1;
        }
    }
    let mut paragraph = Vec::new();
    let mut seen = HashMap::new();
    let mut fence: Option<(char, usize)> = None;
    let mut math: Option<String> = None;
    let mut html_quote = 0usize;
    let mut lazy_quote = false;
    for (index, line) in lines.iter().enumerate() {
        let trim = line.trim();
        if let Some((mark, length)) = fence {
            let run = trim.chars().take_while(|c| *c == mark).count();
            if run >= length && trim.chars().skip(run).all(char::is_whitespace) {
                fence = None;
            }
            continue;
        }
        if let Some(end) = &math {
            if trim.contains(end.as_str()) {
                math = None;
            }
            continue;
        }
        let lower = trim.to_ascii_lowercase();
        if html_quote > 0 {
            html_quote = (html_quote + lower.matches("<blockquote").count())
                .saturating_sub(lower.matches("</blockquote").count());
            continue;
        }
        if trim.is_empty() {
            flush(&mut paragraph, &mut seen, address, faults);
            lazy_quote = false;
            continue;
        }
        if trim == "<!-- zt:reverse-links -->" {
            break;
        }
        if lazy_quote {
            continue;
        }
        if quoted.contains(&index) {
            flush(&mut paragraph, &mut seen, address, faults);
            continue;
        }
        if let Some(mark) = trim.chars().next().filter(|c| *c == '`' || *c == '~') {
            let length = trim.chars().take_while(|c| *c == mark).count();
            if length >= 3 {
                flush(&mut paragraph, &mut seen, address, faults);
                fence = Some((mark, length));
                continue;
            }
        }
        let ending = if trim.starts_with("$$") {
            Some(("$$".to_string(), 2))
        } else if trim.starts_with("\\[") {
            Some(("\\]".to_string(), 2))
        } else if let Some(rest) = trim.strip_prefix("\\begin{") {
            rest.split_once('}').and_then(|(env, _)| {
                [
                    "equation",
                    "equation*",
                    "align",
                    "align*",
                    "aligned",
                    "gather",
                    "gather*",
                    "displaymath",
                    "math",
                    "split",
                    "eqnarray",
                    "eqnarray*",
                ]
                .contains(&env)
                .then(|| (format!("\\end{{{env}}}"), env.len() + 8))
            })
        } else {
            None
        };
        if let Some((end, offset)) = ending {
            flush(&mut paragraph, &mut seen, address, faults);
            if !trim.get(offset..).unwrap_or_default().contains(&end) {
                math = Some(end);
            }
            continue;
        }
        if lower.starts_with("<blockquote") {
            flush(&mut paragraph, &mut seen, address, faults);
            html_quote = lower
                .matches("<blockquote")
                .count()
                .saturating_sub(lower.matches("</blockquote").count());
            continue;
        }
        if trim.starts_with('>') {
            flush(&mut paragraph, &mut seen, address, faults);
            lazy_quote = true;
            continue;
        }
        if line.starts_with("    ") || line.starts_with('\t') {
            if !paragraph.is_empty() {
                paragraph.push((index + 1, *line));
            }
            continue;
        }
        if trim.starts_with('|') || trim.starts_with("<!--") || trim.starts_with("![") {
            flush(&mut paragraph, &mut seen, address, faults);
            continue;
        }
        if !title.is_empty() && heading(trim) == Some(title) {
            flush(&mut paragraph, &mut seen, address, faults);
            faults.add(
                address,
                "R19",
                &format!("repeated Card title at body line {}", index + 1),
            );
            continue;
        }
        if HEADING.is_match(trim) || SECTION.is_match(trim) {
            flush(&mut paragraph, &mut seen, address, faults);
            continue;
        }
        paragraph.push((index + 1, *line));
    }
    flush(&mut paragraph, &mut seen, address, faults);
}
/// The sentences of a line; formulas and Links split nothing.
fn split_sentences(line: &str) -> Vec<String> {
    let mut hidden = Vec::new();
    let masked = HIDE.replace_all(line, |caps: &Captures| {
        hidden.push(caps[0].to_string());
        format!("§{}§", hidden.len() - 1)
    });
    // Split after 。！？!? and at the spaces after a period.
    let chars = masked.chars().collect::<Vec<_>>();
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        current.push(ch);
        index += 1;
        if matches!(ch, '。' | '！' | '？' | '!' | '?') {
            parts.push(std::mem::take(&mut current));
        } else if ch == '.' && chars.get(index).is_some_and(|next| next.is_whitespace()) {
            parts.push(std::mem::take(&mut current));
            while chars.get(index).is_some_and(|next| next.is_whitespace()) {
                index += 1;
            }
        }
    }
    parts.push(current);
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| {
            HIDDEN
                .replace_all(part, |caps: &Captures| {
                    caps[1]
                        .parse::<usize>()
                        .ok()
                        .and_then(|at| hidden.get(at))
                        .cloned()
                        .unwrap_or_default()
                })
                .into_owned()
        })
        .collect()
}

/// A `被` that names no actor: not 被告, 被保险, 被动, 被子, or a party (被收购方).
fn passive(plain: &str) -> bool {
    let is_cjk = |ch: char| {
        ('\u{3400}'..='\u{9FFF}').contains(&ch) || ('\u{F900}'..='\u{FAFF}').contains(&ch)
    };
    plain.match_indices('被').any(|(at, _)| {
        let rest = &plain[at + '被'.len_utf8()..];
        if ["告", "保险", "动", "子"]
            .iter()
            .any(|word| rest.starts_with(word))
        {
            return false;
        }
        let next = rest.chars().take(3).collect::<Vec<_>>();
        let party =
            |n: usize| next.len() > n && next[..n].iter().all(|ch| is_cjk(*ch)) && next[n] == '方';
        !(party(1) || party(2))
    })
}

// --- the term table ---------------------------------------------------------------------------

type Terms = Vec<(String, Vec<String>)>;

/// The term table under a line that holds `术语表`: the term, and the words not to use.
fn read_terms(body: &str) -> Terms {
    let mut terms: Terms = Vec::new();
    let mut in_table = false;
    for line in body.split('\n') {
        if line.contains("术语表") {
            in_table = true;
            continue;
        }
        if !in_table {
            continue;
        }
        let t = line.trim();
        if t.is_empty() || !culture_starts_with(t, "|") {
            if terms.is_empty() {
                continue;
            }
            break;
        }
        let cells = t
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if cells.len() < 2 || TABLE_RULE.is_match(cells[0]) || ps_eq(cells[0], "术语") {
            continue;
        }
        let avoid = AVOID_SPLIT
            .split(cells[1])
            .map(|word| word.trim_matches(&[' ', '`', '“', '”'][..]))
            .filter(|word| !word.is_empty() && !AVOID_NONE.is_match(word))
            .map(str::to_string)
            .collect::<Vec<_>>();
        if avoid.is_empty() {
            continue;
        }
        // An ordered hashtable: a term that differs only in case replaces the earlier one.
        let term = cells[0].to_string();
        match terms.iter_mut().find(|(known, _)| key(known) == key(&term)) {
            Some(entry) => *entry = (term, avoid),
            None => terms.push((term, avoid)),
        }
    }
    terms
}

// --- the checks -------------------------------------------------------------------------------

/// The limits of card-style-check.ps1 and self-check.ps1.
#[derive(Clone, Debug)]
pub(crate) struct Limits {
    pub(crate) max_sentences: i64,
    pub(crate) max_chars: i64,
    pub(crate) max_steps: i64,
    pub(crate) max_explain_sentences: i64,
    pub(crate) max_explain_chars: i64,
    pub(crate) min_idea_chars: i64,
    pub(crate) max_idea_chars: i64,
    pub(crate) min_reason: i64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_sentences: 12,
            max_chars: 400,
            max_steps: 6,
            max_explain_sentences: 16,
            max_explain_chars: 600,
            min_idea_chars: 40,
            max_idea_chars: 300,
            min_reason: 8,
        }
    }
}

/// One run of card-style-check.ps1: the limits, the level, the term table, and
/// the shared parts that follow another model's term table.
pub(crate) struct Check {
    pub(crate) limits: Limits,
    pub(crate) require_explain: bool,
    pub(crate) level: String,
    terms: Terms,
    shared: Vec<String>,
}

impl Check {
    pub(crate) fn new(limits: Limits, require_explain: bool, level: &str) -> Self {
        Self {
            limits,
            require_explain,
            level: level.to_string(),
            terms: Vec::new(),
            shared: Vec::new(),
        }
    }

    /// The term table from the setting Card's body.
    fn with_terms(mut self, setting_body: Option<&str>) -> Self {
        self.terms = setting_body
            .map(|body| read_terms(&utf16_form(body)))
            .unwrap_or_default();
        self
    }

    fn with_shared(mut self, shared: Vec<String>) -> Self {
        self.shared = shared.iter().map(|address| utf16_form(address)).collect();
        self
    }

    /// Every fault of the Cards (`{"address","title","body","facet",...}`), in
    /// the order card-style-check.ps1 writes them.
    pub(crate) fn run(&self, cards: &[Value]) -> Vec<String> {
        let mut faults = Faults::default();
        let mut prose_chars = HashMap::new();
        for card in cards {
            let address = text(prop(card, "address"));
            let chars = self.language(card, &address, &mut faults);
            prose_chars.insert(key(&address), chars);
        }
        self.layer(cards, &prose_chars, &mut faults);
        faults.lines
    }

    /// R2 to R19 for one Card; returns its prose characters (I2 reads them).
    fn language(&self, card: &Value, address: &str, faults: &mut Faults) -> i64 {
        let title = text(prop(card, "title"));
        corruption(&text(prop(card,"body")),&title,address,faults);
        let facet = text(prop(card, "facet"));
        // A root, an anchor's section index, and a hub's navigation list are not claims; the
        // standalone rules read the authored prose that states one (D126's Literature prose
        // excludes a locator, and an index line is a locator).
        let navigation = ps_in(&text(prop(card, "category")), &["anchor", "root", "hub"]);
        let shared = ps_in(address, &self.shared);
        let derivation = culture_starts_with(&facet, "推导") || DERIVATION.is_match(&facet);
        if !title.is_empty() {
            let measure = measure(&title);
            let limit = if measure.words { 25 } else { 40 };
            if measure.size > limit {
                faults.add(
                    address,
                    "R12",
                    &format!("({} {} > {limit}) {title}", measure.size, measure.unit()),
                );
            }
            if SEMICOLON.is_match(&plain(&title)) {
                faults.add(address, "R12", &format!("two claims: {title}"));
            }
            // A title names its content, so a bare local number is the same fault there.
            local_reference(faults, address, &title);
            unnamed_referent(faults, address, &title);
        }
        let mut sentences = 0;
        let mut chars = 0;
        let mut steps = 0;
        // A Derivation part: its sections, the explanation (思路, 说明, 解读), and the 说明 lines per step.
        let mut section = String::new();
        let mut order = Vec::new();
        let mut explain_sentences = 0;
        let mut explain_chars = 0;
        let mut thought = 0;
        let mut reading = 0;
        let mut notes = BTreeMap::new();
        for line in prose_lines(&text(prop(card, "body"))) {
            if derivation && let Some(caps) = SECTION.captures(&line.text) {
                section = caps[1].to_string();
                order.push(section.clone());
                continue;
            }
            // The position line of 思路 (`第 2 张，共 3 张：…`) is the compiler's, not the writer's text.
            if derivation && section == "思路" && POSITION.is_match(&line.text) {
                continue;
            }
            let is_step = derivation && line.step;
            if is_step {
                steps += 1;
            }
            let is_note = derivation && !is_step && section == "步骤" && NOTE.is_match(&line.text);
            let line_text = if is_note {
                NOTE_LABEL.replace(&line.text, "").into_owned()
            } else {
                line.text.clone()
            };
            // The 来源 line after 解读 is not part of the explanation.
            let is_explain = is_note
                || (derivation
                    && (section == "思路" || section == "解读")
                    && !SOURCE_LABEL.is_match(&line.text));
            if is_note {
                if steps == 0 {
                    faults.add(
                        address,
                        "R17",
                        &format!("a 说明 line before the first step: {line_text}"),
                    );
                } else if notes.contains_key(&steps) {
                    faults.add(
                        address,
                        "R17",
                        &format!("a second 说明 line under step {steps}: {line_text}"),
                    );
                }
                notes.insert(steps, 0);
            }
            for sentence in split_sentences(&line_text) {
                self.sentence(faults, address, shared, &sentence, is_step);
                // A source locator (`来源：[[book]] §11.D`) may carry the number.
                if !navigation && !SOURCE_LINE.is_match(&line.text) {
                    local_reference(faults, address, &sentence);
                    unnamed_referent(faults, address, &sentence);
                }
                if is_step {
                    continue;
                }
                let measure = measure(&sentence);
                let size = if measure.words { 0 } else { measure.size };
                if is_explain {
                    explain_sentences += 1;
                    explain_chars += size;
                    if is_note {
                        *notes.entry(steps).or_insert(0) += 1;
                    } else if !ps_eq(sentence.trim(), "待写") {
                        if section == "思路" {
                            thought += 1;
                        } else {
                            reading += 1;
                        }
                    }
                } else {
                    sentences += 1;
                    chars += size;
                }
            }
        }
        // R15 to R17: a formal model's Derivation part with an explanation, or any one with --require-explain.
        if derivation && !ps_eq(&self.level, "concept") {
            let has = order.iter().any(|name| name == "思路" || name == "解读");
            if has || self.require_explain {
                if order.join(",") != SECTIONS.join(",") {
                    let found = if order.is_empty() {
                        "none".to_string()
                    } else {
                        order.join(", ")
                    };
                    faults.add(
                        address,
                        "R15",
                        &format!("sections {found}; want {}", SECTIONS.join(", ")),
                    );
                }
                if order.iter().any(|name| name == "思路") && thought > 5 {
                    faults.add(
                        address,
                        "R16",
                        &format!("(思路 has {thought} sentences > 5)"),
                    );
                }
                if order.iter().any(|name| name == "解读") && reading > 6 {
                    faults.add(
                        address,
                        "R16",
                        &format!("(解读 has {reading} sentences > 6)"),
                    );
                }
                for (step, count) in &notes {
                    if *count > 2 {
                        faults.add(
                            address,
                            "R17",
                            &format!("(说明 under step {step} has {count} sentences > 2)"),
                        );
                    }
                }
                let limits = &self.limits;
                if explain_sentences > limits.max_explain_sentences {
                    faults.add(
                        address,
                        "R17",
                        &format!(
                            "({explain_sentences} explanation sentences > {}) shorten the explanation",
                            limits.max_explain_sentences
                        ),
                    );
                } else if explain_chars > limits.max_explain_chars {
                    faults.add(
                        address,
                        "R17",
                        &format!(
                            "({explain_chars} explanation characters > {}) shorten the explanation",
                            limits.max_explain_chars
                        ),
                    );
                }
            }
        }
        // Card size: only a Model part of a formal model; the Model Card is the model's entry
        // point, and a concept model keeps its few parts whole.
        if ps_eq(&self.level, "concept") {
            return chars;
        }
        let limits = &self.limits;
        if !facet.is_empty() {
            if sentences > limits.max_sentences {
                faults.add(
                    address,
                    "R13",
                    &format!(
                        "({sentences} sentences > {}) split the part into two or more parts",
                        limits.max_sentences
                    ),
                );
            } else if chars > limits.max_chars {
                faults.add(
                    address,
                    "R13",
                    &format!(
                        "({chars} characters > {}) split the part into two or more parts",
                        limits.max_chars
                    ),
                );
            }
        }
        if steps > limits.max_steps {
            faults.add(
                address,
                "R14",
                &format!(
                    "({steps} steps > {}) split the derivation into a chain of Derivation parts",
                    limits.max_steps
                ),
            );
        }
        chars
    }

    /// R2 to R9 for one sentence.
    fn sentence(
        &self,
        faults: &mut Faults,
        address: &str,
        shared: bool,
        sentence: &str,
        step: bool,
    ) {
        let plain = plain(sentence);
        let measure = measure(sentence);
        let limit = match (measure.words, step) {
            (false, true) => 30,
            (false, false) => 40,
            (true, true) => 20,
            (true, false) => 25,
        };
        if measure.size > limit {
            faults.add(
                address,
                "R2",
                &format!("({} {} > {limit}) {sentence}", measure.size, measure.unit()),
            );
        }
        if SEMICOLON.is_match(&plain) {
            faults.add(address, "R3", sentence);
        }
        // `被收购方`, `被投资方`: a party, not a passive (and a 被 that names the party has its actor).
        if passive(&plain) {
            faults.add(address, "R4", sentence);
        }
        if LIGHT_VERB.is_match(&plain) {
            faults.add(address, "R5", sentence);
        }
        if !shared {
            for (term, words) in &self.terms {
                // A rejected word inside the term itself is the term, not the word.
                let rest = plain.replace(term.as_str(), "");
                for word in words {
                    if rest.contains(word.as_str()) {
                        faults.add(
                            address,
                            "R6",
                            &format!("uses `{word}` for `{term}`: {sentence}"),
                        );
                    }
                }
            }
        }
        if NOUN_PHRASE.is_match(&plain) {
            faults.add(address, "R7", sentence);
        }
        if let Some(word) = HEDGES.iter().find(|word| plain.contains(*word)) {
            faults.add(address, "R9", &format!("`{word}`: {sentence}"));
        }
    }

    /// The Card layer rules (I, E, F, L): only for entries that name their category.
    fn layer(&self, cards: &[Value], prose_chars: &HashMap<String, i64>, faults: &mut Faults) {
        let layer = cards
            .iter()
            .filter(|card| prop(card, "category").is_some_and(truthy))
            .collect::<Vec<_>>();
        let limits = &self.limits;
        for card in &layer {
            let address = text(prop(card, "address"));
            let category = text(prop(card, "category"));
            let is = |names: &[&str]| ps_in(&category, names);
            if !is(&["idea", "anchor", "event", "finding"]) {
                continue;
            }
            let title = text(prop(card, "title"));
            let body = text(prop(card, "body"));
            let has = |name: &str| prop(card, name).is_some();
            let parent = text(prop(card, "parent"));
            let links = strings(prop(card, "links"));
            let kinds = strings(prop(card, "kinds"));
            let title_plain = SPACE
                .replace_all(&PLACEHOLDERS.replace_all(&plain(&title), ""), "")
                .into_owned();
            if is(&["idea", "anchor"])
                && length(&title_plain) <= 6
                && !CLAIM_VERB.is_match(&title_plain)
            {
                faults.add(
                    &address,
                    "I1",
                    &format!("(advisory) the title states no claim: {title}"),
                );
            }
            if is(&["idea"]) {
                let size = prose_chars.get(&key(&address)).copied().unwrap_or(0);
                if size > limits.max_idea_chars {
                    faults.add(
                        &address,
                        "I2",
                        &format!(
                            "({size} characters > {}) split candidate",
                            limits.max_idea_chars
                        ),
                    );
                } else if size < limits.min_idea_chars {
                    faults.add(
                        &address,
                        "I2",
                        &format!(
                            "({size} characters < {}) merge candidate",
                            limits.min_idea_chars
                        ),
                    );
                }
            }
            if is(&["idea", "event"])
                && has("book")
                && prop(card, "book").is_some_and(truthy)
                && has("links")
            {
                let book = text(prop(card, "book"));
                let anchor = Regex::new(&format!("(?i)^{}/\\d+$", regex::escape(&book)))
                    .expect("an escaped Citation key");
                if !links
                    .iter()
                    .any(|link| ps_eq(link, &book) || anchor.is_match(link))
                {
                    faults.add(
                        &address,
                        "I3",
                        &format!("no source line: no Link to [[{book}]] or one of its anchors"),
                    );
                }
            }
            if is(&["idea"]) && has("kinds") && !kinds.iter().any(|kind| !ps_eq(kind, &parent)) {
                faults.add(
                    &address,
                    "I4",
                    "no kinded Link to a Card other than the parent",
                );
            }
            if has("inbound") {
                let inbound = prop(card, "inbound")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                if crate::graph::is_orphan(&address, &text(prop(card, "category")), &inbound) {
                    faults.add(&address, "I5", "no other Card has an authored Link here");
                }
            }
            if is(&["event"]) {
                if !DATE.is_match(&title_plain) {
                    faults.add(&address, "E1", &format!("no date in the title: {title}"));
                }
                let missing = EVENT_LABELS
                    .iter()
                    .filter(|(_, pattern)| !pattern.is_match(&body))
                    .map(|(label, _)| *label)
                    .collect::<Vec<_>>();
                if !missing.is_empty() {
                    faults.add(
                        &address,
                        "E2",
                        &format!("missing labels: {}", missing.join("、")),
                    );
                }
                if has("inbound_kinded") && ps_int(prop(card, "inbound_kinded")) == 0 {
                    faults.add(&address, "E4", "no Idea Card links it with a kind word");
                }
            }
            if is(&["finding"]) {
                finding_rules(card, &address, &body, faults);
            }
            if has("links") {
                let mut refers: Vec<String> = Vec::new();
                for link in &links {
                    if !ps_in(link, &kinds)
                        && !book_place(link)
                        && !leads_to(&body, link)
                        && !refers.iter().any(|seen| culture(seen) == culture(link))
                    {
                        refers.push(link.clone());
                    }
                }
                if !refers.is_empty() {
                    let shown = refers
                        .iter()
                        .take(5)
                        .map(|link| format!("[[{link}]]"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    faults.add(
                        &address,
                        "L1",
                        &format!("({} Links read as refers) {shown}", refers.len()),
                    );
                }
            }
            // L2 and L3 read the body: the sentence of each Link, and the paragraphs it appears in.
            let mut paragraphs: Vec<(String, BTreeSet<usize>)> = Vec::new();
            let mut paragraph = 0;
            for raw in body.split('\n') {
                let trim = raw.trim();
                if trim.is_empty() {
                    paragraph += 1;
                    continue;
                }
                for caps in LINK_TARGET.captures_iter(trim) {
                    let target = caps[1].trim();
                    match paragraphs
                        .iter_mut()
                        .find(|(known, _)| key(known) == key(target))
                    {
                        Some((_, seen)) => {
                            seen.insert(paragraph);
                        }
                        None => paragraphs.push((target.to_string(), BTreeSet::from([paragraph]))),
                    }
                }
            }
            for line in prose_lines(&body) {
                for sentence in split_sentences(&line.text) {
                    if !sentence.contains("[[") || SOURCE_LINE.is_match(&sentence) {
                        continue;
                    }
                    let reason = PLACEHOLDERS.replace_all(&plain(&sentence), "").into_owned();
                    let reason = REASON_STRIP.replace_all(&reason, "");
                    // A formula states the reason too (`代入 [[…]] 的定义得 $MRTS=…$`).
                    if length(&reason) < limits.min_reason && !INLINE_MATH.is_match(&sentence) {
                        faults.add(
                            &address,
                            "L2",
                            &format!(
                                "({} characters < {}) a Link with no reason: {sentence}",
                                length(&reason),
                                limits.min_reason
                            ),
                        );
                    }
                }
            }
            paragraphs.sort_by(|a, b| culture_cmp(&a.0, &b.0));
            for (target, seen) in &paragraphs {
                if seen.len() >= 2 {
                    faults.add(
                        &address,
                        "L3",
                        &format!(
                            "(advisory) [[{target}]] is linked in {} paragraphs",
                            seen.len()
                        ),
                    );
                }
            }
            // I7 reads the relationship evidence, so only a Card that carries it is in scope.
            if is(&["idea"]) {
                own_source_dependency(card, &address, faults);
            }
        }
        // I6 and E5 compare Cards with each other. Group-Object sorts its groups.
        let mut groups: Vec<(String, Vec<(String, HashSet<String>)>)> = Vec::new();
        for card in &layer {
            if !ps_eq(&text(prop(card, "category")), "idea")
                || !prop(card, "tree").is_some_and(truthy)
            {
                continue;
            }
            let tree = text(prop(card, "tree"));
            let item = (
                text(prop(card, "address")),
                bigrams(&text(prop(card, "title"))),
            );
            match groups
                .iter_mut()
                .find(|(name, _)| culture(name).to_lowercase() == culture(&tree).to_lowercase())
            {
                Some((_, items)) => items.push(item),
                None => groups.push((tree, vec![item])),
            }
        }
        groups.sort_by(|a, b| culture_cmp(&a.0, &b.0));
        for (_, items) in &groups {
            for i in 0..items.len() {
                for j in i + 1..items.len() {
                    if similarity(&items[i].1, &items[j].1) >= 0.8 {
                        faults.add(
                            &items[j].0,
                            "I6",
                            &format!("(advisory) a title very similar to [[{}]]", items[i].0),
                        );
                    }
                }
            }
        }
        let events = layer
            .iter()
            .filter(|card| ps_eq(&text(prop(card, "category")), "event"))
            .map(|card| {
                let title = text(prop(card, "title"));
                let year = YEAR
                    .captures(&title)
                    .map(|caps| caps[1].to_string())
                    .unwrap_or_default();
                (text(prop(card, "address")), year, bigrams(&title))
            })
            .collect::<Vec<_>>();
        for i in 0..events.len() {
            for j in i + 1..events.len() {
                if !events[i].1.is_empty()
                    && ps_eq(&events[i].1, &events[j].1)
                    && similarity(&events[i].2, &events[j].2) >= 0.5
                {
                    faults.add(
                        &events[j].0,
                        "E5",
                        &format!("(advisory) the same event as [[{}]]?", events[i].0),
                    );
                }
            }
        }
    }
}

/// A source line or a place in the book: a Literature Card (`hart1995firms`)
/// or an anchor (`hart1995firms/4`). Topic trees are numbered, so `12/3` is no anchor.
fn book_place(target: &str) -> bool {
    BOOK_CARD.is_match(target) || BOOK_ANCHOR.is_match(target)
}

/// R20: a local label that stands in for its content (`式 (12) 有该项`). The
/// content is what the rest of the text says once the labels are taken out, so
/// `式 (12) 的加成项来自需求弹性` passes and a source locator is tested by its
/// caller only. Advice in the pilot: the label may stay as a secondary name.
fn local_reference(faults: &mut Faults, address: &str, text: &str) {
    let text_plain = plain(text);
    let mut labels: Vec<String> = Vec::new();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for pattern in [
        &*LOCAL_EQUATION,
        &*LOCAL_NAMED_NUMBER,
        &*LOCAL_CHAPTER,
        &*LOCAL_THIS,
        &*LOCAL_SIGN,
    ] {
        for found in pattern.find_iter(&text_plain) {
            if spans
                .iter()
                .any(|(start, end)| *start < found.end() && found.start() < *end)
            {
                continue;
            }
            spans.push((found.start(), found.end()));
            labels.push(found.as_str().to_string());
        }
    }
    if labels.is_empty() {
        return;
    }
    let mut order = (0..labels.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| spans[*index].0);
    let mut rest = String::new();
    let mut at = 0;
    for index in order {
        rest.push_str(&text_plain[at..spans[index].0]);
        at = spans[index].1;
    }
    rest.push_str(&text_plain[at..]);
    if length(&REASON_STRIP.replace_all(&rest, "")) >= LOCAL_REFERENCE_CONTENT {
        return;
    }
    for label in labels {
        faults.add(
            address,
            "R20",
            &format!("(advisory) local reference `{label}` names no content: {text}"),
        );
    }
}

/// R21: a deictic phrase with no named Link in its own sentence (`该卡` alone,
/// `那里` alone). A Link in the same sentence names the referent; a Link in the
/// sentence before does not. It decides only what it can see.
fn unnamed_referent(faults: &mut Faults, address: &str, text: &str) {
    let text_plain = plain(text);
    let Some(word) = DEICTIC.iter().find(|word| text_plain.contains(**word)) else {
        return;
    };
    if LINK.is_match(text) {
        return;
    }
    faults.add(
        address,
        "R21",
        &format!("(advisory) unnamed referent `{word}` with no named Link in its sentence: {text}"),
    );
}

/// The Literature tree a Link target names: the Citation key of a book Card or
/// of one of its anchors. A Topic address names no Literature tree.
fn literature_tree(target: &str) -> Option<String> {
    if BOOK_ANCHOR.is_match(target) {
        return target.split_once('/').map(|(key, _)| key.to_string());
    }
    BOOK_CARD.is_match(target).then(|| target.to_string())
}

/// The source evidence of one Card as the audit derives it: an explicit `sources`
/// set, else the Citation keys its `来源` lines name, else the Literature tree it
/// is filed in. Empty is unknown, which is not a pass.
fn source_evidence(card: &Value) -> Vec<String> {
    let explicit = strings(prop(card, "sources"));
    if !explicit.is_empty() {
        return explicit;
    }
    fn push(sources: &mut Vec<String>, tree: String) {
        if !sources.iter().any(|seen| key(seen) == key(&tree)) {
            sources.push(tree);
        }
    }
    let mut sources: Vec<String> = Vec::new();
    for line in text(prop(card, "body")).split('\n') {
        if !SOURCE_LINE.is_match(line.trim()) {
            continue;
        }
        for caps in LINK_TARGET.captures_iter(line) {
            if let Some(tree) = literature_tree(caps[1].trim()) {
                push(&mut sources, tree);
            }
        }
    }
    if sources.is_empty()
        && let Some(tree) = literature_tree(&text(prop(card, "tree")))
    {
        push(&mut sources, tree);
    }
    sources
}

/// I7: a Topic Idea Card whose `rests-on` Link enters its own source's Literature
/// tree. The claim has to carry its necessary conditions instead. A Card filed in
/// that tree rests on it by design, and an `illustrates`, `corresponds`, or
/// `contrasts` Link is no dependency.
fn own_source_dependency(card: &Value, address: &str, faults: &mut Faults) {
    let Some(relations) = card.get("relations").and_then(Value::as_object) else {
        return;
    };
    let tree = text(prop(card, "tree"));
    let sources = source_evidence(card);
    for (target, kind) in relations {
        if !ps_eq(&ps_string(Some(kind)), "rests-on") {
            continue;
        }
        let Some(book) = literature_tree(target) else {
            continue;
        };
        if !tree.is_empty() && key(&tree) == key(&book) {
            continue;
        }
        if sources.is_empty() {
            faults.add(
                address,
                "I7",
                &format!(
                    "(advisory) source evidence unknown, so [[{target}]] may be an own-source premise"
                ),
            );
        } else if sources.iter().any(|source| key(source) == key(&book)) {
            faults.add(
                address,
                "I7",
                &format!("`rests-on` [[{target}]] enters its own source `{book}`"),
            );
        }
    }
}

/// The Finding Card rules. F1: the fixed labels. F2: the `来源` line names its
/// source: a Link to the book or one of its anchors when the book is known,
/// else any text. F3: one result per Card: one `结果` label, with at most one
/// list item under it. F4: an Idea Card links it with a kind word.
fn finding_rules(card: &Value, address: &str, body: &str, faults: &mut Faults) {
    let lines = body
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>();
    let is_label = |line: &str| {
        FINDING_LABELS
            .iter()
            .any(|(_, pattern)| pattern.is_match(line))
    };
    let labelled = |name: &str| {
        let pattern = &FINDING_LABELS
            .iter()
            .find(|(label, _)| *label == name)
            .expect("a Finding label")
            .1;
        lines
            .iter()
            .enumerate()
            .filter(|(_, line)| pattern.is_match(line))
            .map(|(index, line)| (index, pattern.replace(line, "").trim().to_string()))
            .collect::<Vec<_>>()
    };
    let missing = FINDING_LABELS
        .iter()
        .filter(|(label, _)| labelled(label).is_empty())
        .map(|(label, _)| *label)
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        faults.add(
            address,
            "F1",
            &format!("missing labels: {}", missing.join("、")),
        );
    }
    let sources = labelled("来源");
    if !sources.is_empty() {
        if prop(card, "book").is_some_and(truthy) {
            let book = text(prop(card, "book"));
            let anchor = Regex::new(&format!("(?i)^{}/\\d+$", regex::escape(&book)))
                .expect("an escaped Citation key");
            let linked = sources.iter().any(|(_, rest)| {
                LINK_TARGET.captures_iter(rest).any(|caps| {
                    let target = caps[1].trim();
                    ps_eq(target, &book) || anchor.is_match(target)
                })
            });
            if !linked {
                faults.add(
                    address,
                    "F2",
                    &format!("the source line links neither [[{book}]] nor one of its anchors"),
                );
            }
        } else if sources.iter().all(|(_, rest)| rest.is_empty()) {
            faults.add(address, "F2", "the source line names no source");
        }
    }
    let results = labelled("结果");
    if results.len() > 1 {
        faults.add(
            address,
            "F3",
            &format!(
                "({} 结果 labels) one result per Card: split it",
                results.len()
            ),
        );
    } else if let Some((index, _)) = results.first() {
        let items = lines[index + 1..]
            .iter()
            .take_while(|line| !line.trim().is_empty() && !is_label(line))
            .filter(|line| LIST_ITEM.is_match(line))
            .count();
        if items > 1 {
            faults.add(
                address,
                "F3",
                &format!("({items} results under 结果) one result per Card: split it"),
            );
        }
    }
    if prop(card, "inbound_kinded").is_some() && ps_int(prop(card, "inbound_kinded")) == 0 {
        faults.add(address, "F4", "no Idea Card links it with a kind word");
    }
}

/// A Link that 引出 announces in its sentence: it points the other way.
fn leads_to(body: &str, target: &str) -> bool {
    let link = format!("[[{target}]]");
    body.match_indices(&link).any(|(at, _)| {
        let start = body[..at]
            .char_indices()
            .rev()
            .find(|(_, ch)| matches!(ch, '。' | '！' | '？' | '!' | '?' | '；' | ';' | '\n'))
            .map_or(0, |(index, ch)| index + ch.len_utf8());
        body[start..at].contains("引出")
    })
}

/// Character bigrams of a title without formulas and Links.
fn bigrams(title: &str) -> HashSet<String> {
    let plain = PLACEHOLDERS.replace_all(&plain(title), "").into_owned();
    let chars = NOT_LETTER_OR_NUMBER
        .replace_all(&plain, "")
        .chars()
        .collect::<Vec<_>>();
    chars.windows(2).map(|pair| pair.iter().collect()).collect()
}

fn similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let common = a.iter().filter(|bigram| b.contains(*bigram)).count();
    2.0 * common as f64 / (a.len() + b.len()) as f64
}

#[derive(Default)]
struct Faults {
    lines: Vec<String>,
}

impl Faults {
    /// `<address> <rule> <text>`, the text on one line and clipped to 90 characters.
    fn add(&mut self, address: &str, rule: &str, text: &str) {
        let collapsed = SPACES.replace_all(text, " ");
        let trimmed = collapsed.trim();
        let units = trimmed.chars().collect::<Vec<_>>();
        let clip = if units.len() > 90 {
            let mut kept = units[..89].iter().collect::<String>();
            // A cut through a surrogate pair leaves half a character; .NET writes U+FFFD for it.
            if is_high_stand_in(units[88]) && is_low_stand_in(units[89]) {
                kept.pop();
                kept.push('\u{FFFD}');
            }
            kept.push('…');
            kept
        } else {
            trimmed.to_string()
        };
        self.lines
            .push(from_utf16_form(&format!("{address} {rule} {clip}")));
    }
}

// --- inputs -----------------------------------------------------------------------------------

/// `--json`: a JSON array of entries (or one entry), as card-style-check.ps1 -Json reads it.
pub(crate) fn check_json(
    raw: &str,
    setting: &str,
    limits: Limits,
    require_explain: bool,
    level: &str,
) -> Result<Vec<String>> {
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);
    let cards = match serde_json::from_str::<Value>(raw).context("the file is not JSON")? {
        Value::Array(items) => items,
        other => vec![other],
    };
    let shared = cards
        .iter()
        .filter(|card| ps_eq(&text_of(card, "line"), "shared"))
        .map(|card| ps_string(prop(card, "address")))
        .collect();
    let setting = utf16_form(setting);
    let setting_body = (!setting.is_empty())
        .then(|| {
            cards
                .iter()
                .find(|card| ps_eq(&text(prop(card, "address")), &setting))
                .map(|card| ps_string(prop(card, "body")))
        })
        .flatten();
    let check = Check::new(limits, require_explain, level)
        .with_terms(setting_body.as_deref())
        .with_shared(shared);
    Ok(check.run(&cards))
}

fn text_of(card: &Value, name: &str) -> String {
    text(prop(card, name))
}

/// A Card as card-style-check.ps1 reads it from get_card: address, title, body, facet.
fn card_entry(card: &Value) -> Value {
    json!({
        "address": card.get("address").cloned().unwrap_or(Value::Null),
        "title": card.get("title").cloned().unwrap_or(Value::Null),
        "body": card.get("body").cloned().unwrap_or(Value::Null),
        "facet": card.get("facet").cloned().unwrap_or(Value::Null),
    })
}

/// The body of the setting Card: from the Cards checked, else from zt.
fn setting_body(zt: &mut Zt, cards: &[Value], setting: &str) -> Result<Option<String>> {
    if setting.is_empty() {
        return Ok(None);
    }
    let wanted = utf16_form(setting);
    if let Some(card) = cards
        .iter()
        .find(|card| ps_eq(&text(prop(card, "address")), &wanted))
    {
        return Ok(Some(ps_string(prop(card, "body"))));
    }
    let read = zt.cards(&[setting.to_string()], "text")?;
    Ok(Some(ps_string(read.cards[0].get("body"))))
}

/// `--model`: every Card of the complete model context, with its setting's
/// term table and level. An external setting uses a required lean read.
pub(crate) fn check_model(
    zt: &mut Zt,
    at: &str,
    setting: &str,
    level: &str,
    limits: Limits,
    require_explain: bool,
    skip_shared: bool,
) -> Result<Vec<String>> {
    let revision = zt.cards(&[], "text")?.archive_revision;
    let structure = zt.call("get_model", json!({ "at": at }))?;
    structure["head"]["address"]
        .as_str()
        .with_context(|| format!("`{at}` belongs to no model"))?;
    let mut cards = vec![card_entry(&structure["head"])];
    for part in structure["parts"].as_array().into_iter().flatten() {
        cards.push(card_entry(part));
    }
    let setting = if setting.is_empty() {
        ps_string(structure["build"].get("setting"))
    } else {
        setting.to_string()
    };
    let level = if level.is_empty() {
        ps_string(structure.get("level"))
    } else {
        level.to_string()
    };
    // A shared part follows the term table of its home model, not this one.
    let shared = structure["build"]["order"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| ps_eq(&ps_string(item.get("line")), "shared"))
        .map(|item| ps_string(item.get("address")))
        .collect::<Vec<_>>();
    if skip_shared {
        cards.retain(|card| !ps_in(&ps_string(prop(card, "address")), &shared));
    }
    let body = setting_body(zt, &cards, &setting)?;
    let check = Check::new(limits, require_explain, &level)
        .with_terms(body.as_deref())
        .with_shared(shared);
    if zt.cards(&[], "text")?.archive_revision != revision {
        bail!("incomplete checker inputs: changing read view");
    }
    Ok(check.run(&cards))
}

/// `--cards`: self-check.ps1. Each Card as digest-audit.ps1 sees it: a Model
/// part or Model Card with its model (setting, level, and every Derivation
/// part with an explanation); an Idea Card, an anchor, an Event Card, or a
/// Finding Card with
/// its layer rules; any other Card with the R rules only. Returns every line,
/// before the rule filter.
pub(crate) fn check_cards(
    zt: &mut Zt,
    addresses: &[String],
    book: &str,
    limits: Limits,
) -> Result<Vec<String>> {
    let batch = zt.cards(addresses, "text")?;
    let catalogue = zt.call("export_titles", json!({}))?;
    let titles = catalogue["titles"]
        .as_array()
        .context("incomplete checker classification catalogue")?;
    if catalogue["complete"] != true
        || catalogue["count"].as_u64() != Some(titles.len() as u64)
        || catalogue["archive_revision"].as_u64() != Some(batch.archive_revision)
    {
        bail!("incomplete checker classification: changing or partial title view");
    }
    let literature: HashSet<String> = titles
        .iter()
        .filter_map(|card| card["tree"].as_str())
        .filter(|tree| BOOK_CARD.is_match(tree))
        .map(str::to_string)
        .collect();
    let mut anchors: HashMap<String, usize> = HashMap::new();
    for title in titles {
        let address = title["address"]
            .as_str()
            .context("incomplete checker classification address")?;
        if BOOK_ANCHOR.is_match(address) {
            *anchors
                .entry(
                    title["tree"]
                        .as_str()
                        .context("incomplete checker classification tree")?
                        .to_string(),
                )
                .or_default() += 1;
        }
    }
    let mut cards = batch.cards;
    let mut owners: HashMap<String, Option<String>> = HashMap::new();
    let mut models: HashMap<String, Value> = HashMap::new();
    for card in &mut cards {
        if card["type"] == "model" {
            let at = ps_string(card.get("address"));
            if !owners.contains_key(&at) {
                let model = zt.call("get_model", json!({"at":at}))?;
                let head = model["head"]["address"].as_str().map(str::to_string);
                owners.insert(at.clone(), head.clone());
                if let Some(head) = head {
                    owners.insert(head.clone(), Some(head.clone()));
                    // A shared part may have a different primary owner. Only
                    // filed parts are known to have this model as their home.
                    for part in model["parts"].as_array().into_iter().flatten() {
                        if part["filed"] == true {
                            owners.insert(ps_string(part.get("address")), Some(head.clone()));
                        }
                    }
                    models.insert(head, model);
                }
            }
            if let Some(Some(head)) = owners.get(&at) {
                let model = &models[head];
                card["model"] = json!({"head":{"address":head},"level":model["level"],"build":{"setting":model["build"]["setting"]}});
            }
        }
    }
    let mut by_model: Vec<(String, Vec<Value>)> = Vec::new();
    let mut plain = Vec::new();
    for card in &cards {
        let address = ps_string(card.get("address"));
        let tree = ps_string(card.get("tree"));
        let title = ps_string(card.get("title"));
        let body = ps_string(card.get("body"));
        // Valid Topic tree ids are numeric; Literature Citation keys start with a letter.
        let in_book = card["workspace"] == "agent" && BOOK_CARD.is_match(&tree);
        let category = crate::graph::category(card, &literature, &anchors);
        let category = category.as_str();
        let head = card["model"]["head"]["address"]
            .as_str()
            .map(str::to_string);
        if matches!(category, "part" | "model")
            && let Some(head) = head
        {
            match by_model
                .iter_mut()
                .find(|(known, _)| key(known) == key(&head))
            {
                Some((_, members)) => members.push(card.clone()),
                None => by_model.push((head, vec![card.clone()])),
            }
            continue;
        }
        let mut item = Map::new();
        item.insert("address".into(), json!(address));
        item.insert("title".into(), json!(title));
        item.insert("body".into(), json!(body));
        item.insert("facet".into(), Value::Null);
        if matches!(category, "idea" | "anchor" | "event" | "finding") {
            let parent = ps_string(card.get("parent_address"));
            let links = card["links"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|link| !link.is_empty())
                .collect::<Vec<_>>();
            let kinds = card["kinds"]
                .as_object()
                .map(|kinds| kinds.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            item.insert("category".into(), json!(category));
            item.insert("tree".into(), json!(tree));
            item.insert("parent".into(), json!(parent));
            item.insert("links".into(), json!(links));
            item.insert("kinds".into(), json!(kinds));
            // The relationship evidence of I7: the kind word of every kinded Link.
            item.insert("relations".into(), card["kinds"].clone());
            let key = if !book.is_empty() {
                book.to_string()
            } else if in_book {
                tree.clone()
            } else {
                String::new()
            };
            if !key.is_empty() {
                item.insert("book".into(), json!(key));
            }
        }
        plain.push(Value::Object(item));
    }
    let mut lines = Vec::new();
    // Model parts and Model Cards: per model, with its setting (term table) and
    // level, as card-style-check.ps1 -Addresses reads them from the first Card.
    for (_, members) in &by_model {
        let model = &members[0]["model"];
        let setting = ps_string(model["build"].get("setting"));
        let level = ps_string(model.get("level"));
        let checked = members.iter().map(card_entry).collect::<Vec<_>>();
        let body = setting_body(zt, &checked, &setting)?;
        let check = Check::new(limits.clone(), true, &level).with_terms(body.as_deref());
        lines.extend(check.run(&checked));
    }
    if !plain.is_empty() {
        lines.extend(Check::new(limits, false, "").run(&plain));
    }
    if zt.cards(&[], "text")?.archive_revision != batch.archive_revision {
        bail!("incomplete checker inputs: changing read view");
    }
    Ok(lines)
}
/// The lines of the rules asked for; advisory lines are shown but not counted.
fn filter_rules(lines: &[String], rules: &str) -> (Vec<String>, usize) {
    let allowed = LIST_SPLIT
        .split(rules)
        .filter(|rule| !rule.is_empty())
        .collect::<Vec<_>>();
    let mut kept = Vec::new();
    let mut faults = 0;
    for line in lines {
        let Some(caps) = FAULT_LINE.captures(line) else {
            continue;
        };
        if !ps_in(&caps[1], &allowed) {
            continue;
        }
        kept.push(line.clone());
        if !line.contains("(advisory)") {
            faults += 1;
        }
    }
    (kept, faults)
}

// --- the command ------------------------------------------------------------------------------

#[derive(Default)]
struct Options {
    json: Option<String>,
    model: Option<String>,
    addresses: Option<String>,
    cards: Option<String>,
    log: Option<String>,
    book: Option<String>,
    rules: Option<String>,
    setting: Option<String>,
    level: Option<String>,
    require_explain: bool,
    skip_shared: bool,
    limits: Limits,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Options::default();
        let mut index = 0;
        while index < args.len() {
            let flag = args[index].as_str();
            let mut value = || -> Result<String> {
                index += 1;
                args.get(index)
                    .cloned()
                    .with_context(|| format!("{flag} needs a value\n{USAGE}"))
            };
            let number = |text: String| -> Result<i64> {
                text.trim()
                    .parse::<i64>()
                    .with_context(|| format!("{flag} needs a whole number, not `{text}`"))
            };
            match flag {
                "--json" => options.json = Some(value()?),
                "--model" => options.model = Some(value()?),
                "--addresses" => options.addresses = Some(value()?),
                "--cards" => options.cards = Some(value()?),
                "--log" => options.log = Some(value()?),
                "--book" => options.book = Some(value()?),
                "--rules" => options.rules = Some(value()?),
                "--setting" => options.setting = Some(value()?),
                "--level" => {
                    let level = value()?;
                    if !ps_in(&level, &["", "concept", "formal"]) {
                        bail!("--level is concept or formal, not `{level}`");
                    }
                    options.level = Some(level.to_lowercase());
                }
                "--require-explain" => options.require_explain = true,
                "--skip-shared" => options.skip_shared = true,
                "--max-sentences" => options.limits.max_sentences = number(value()?)?,
                "--max-chars" => options.limits.max_chars = number(value()?)?,
                "--max-steps" => options.limits.max_steps = number(value()?)?,
                "--max-explain-sentences" => {
                    options.limits.max_explain_sentences = number(value()?)?
                }
                "--max-explain-chars" => options.limits.max_explain_chars = number(value()?)?,
                "--min-idea-chars" => options.limits.min_idea_chars = number(value()?)?,
                "--max-idea-chars" => options.limits.max_idea_chars = number(value()?)?,
                "--min-reason" => options.limits.min_reason = number(value()?)?,
                "-h" | "--help" => bail!("{USAGE}"),
                other => bail!("unknown option `{other}`\n{USAGE}"),
            }
            index += 1;
        }
        let modes = [
            options.json.is_some(),
            options.model.is_some(),
            options.addresses.is_some(),
            options.cards.is_some() || options.log.is_some(),
        ]
        .into_iter()
        .filter(|on| *on)
        .count();
        if modes != 1 {
            bail!("name one input: --json, --model, --addresses, or --cards/--log\n{USAGE}");
        }
        let cards_mode = options.cards.is_some() || options.log.is_some();
        if cards_mode
            && (options.setting.is_some()
                || options.level.is_some()
                || options.require_explain
                || options.skip_shared)
        {
            bail!(
                "--cards takes each model's own setting and level; --setting, --level, --require-explain, and --skip-shared do not apply"
            );
        }
        if !cards_mode && (options.book.is_some() || options.rules.is_some()) {
            bail!("--book and --rules apply to --cards only");
        }
        if options.skip_shared && options.model.is_none() {
            bail!("--skip-shared applies to --model only");
        }
        Ok(options)
    }
}

/// `--addresses`: card-style-check.ps1 -Addresses. The Cards from lean batches;
/// the term table from --setting or the first Card's model setting; the level
/// from --level or the first Card's model.
pub(crate) fn check_addresses(
    zt: &mut Zt,
    addresses: &[String],
    setting: &str,
    level: &str,
    limits: Limits,
    require_explain: bool,
) -> Result<Vec<String>> {
    let batch = zt.cards(addresses, "text")?;
    let read = batch.cards;
    let first = match read.first().filter(|card| card["type"] == "model") {
        Some(card) => zt.call("get_model", json!({"at":card["address"]}))?,
        None => Value::Null,
    };
    let setting = if setting.is_empty() {
        ps_string(first["build"].get("setting"))
    } else {
        setting.to_string()
    };
    let level = if level.is_empty() {
        ps_string(first.get("level"))
    } else {
        level.to_string()
    };
    let cards = read.iter().map(card_entry).collect::<Vec<_>>();
    let body = setting_body(zt, &cards, &setting)?;
    if zt.cards(&[], "text")?.archive_revision != batch.archive_revision {
        bail!("incomplete checker inputs: changing read view");
    }
    Ok(Check::new(limits, require_explain, &level)
        .with_terms(body.as_deref())
        .run(&cards))
}

/// `zt-check …`.
pub fn run_cli(args: &[String]) -> Result<()> {
    write_lines(&run_lines(args, None)?)
}

/// The lines `zt-check …` prints: one per fault, then `faults: N`. With `zt`,
/// the reads go to that `zt mcp`; without it, a new one starts when needed.
pub fn run_lines(args: &[String], zt: Option<&mut Zt>) -> Result<Vec<String>> {
    fn server<'a>(shared: Option<&'a mut Zt>, own: &'a mut Option<Zt>) -> Result<&'a mut Zt> {
        match shared {
            Some(zt) => Ok(zt),
            None => Ok(own.insert(Zt::start()?)),
        }
    }
    let mut own: Option<Zt> = None;
    let options = Options::parse(args)?;
    let setting = options.setting.clone().unwrap_or_default();
    let level = options.level.clone().unwrap_or_default();
    let (mut out, count) = if let Some(path) = &options.json {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
        let lines = check_json(
            &raw,
            &setting,
            options.limits.clone(),
            options.require_explain,
            &level,
        )?;
        let count = lines.len();
        (lines, count)
    } else if let Some(list) = &options.addresses {
        let list = LIST_SPLIT
            .split(list)
            .filter(|at| !at.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let zt = server(zt, &mut own)?;
        let lines = check_addresses(
            zt,
            &list,
            &setting,
            &level,
            options.limits.clone(),
            options.require_explain,
        )?;
        let count = lines.len();
        (lines, count)
    } else if let Some(at) = &options.model {
        let zt = server(zt, &mut own)?;
        let lines = check_model(
            zt,
            at,
            &setting,
            &level,
            options.limits.clone(),
            options.require_explain,
            options.skip_shared,
        )?;
        let count = lines.len();
        (lines, count)
    } else {
        let mut list = LIST_SPLIT
            .split(options.cards.as_deref().unwrap_or_default())
            .filter(|at| !at.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if let Some(log) = options.log.as_deref().filter(|log| Path::new(log).exists()) {
            let raw =
                std::fs::read_to_string(log).with_context(|| format!("failed to read {log}"))?;
            let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
            list.extend(
                raw.split(['\r', '\n'])
                    .map(str::trim)
                    .filter(|at| !at.is_empty())
                    .map(str::to_string),
            );
        }
        // Select-Object -Unique: case counts, ignorable characters do not.
        let mut unique: Vec<String> = Vec::new();
        for at in list {
            if !unique.iter().any(|seen| culture(seen) == culture(&at)) {
                unique.push(at);
            }
        }
        if unique.is_empty() {
            (vec!["no Cards to check".to_string()], 0)
        } else {
            let zt = server(zt, &mut own)?;
            let lines = check_cards(
                zt,
                &unique,
                options.book.as_deref().unwrap_or_default(),
                options.limits.clone(),
            )?;
            filter_rules(&lines, options.rules.as_deref().unwrap_or(DEFAULT_RULES))
        }
    };
    out.push(format!("faults: {count}"));
    Ok(out)
}

/// One line each, UTF-8 whatever the console code page, with the line ends
/// PowerShell writes (CRLF on Windows).
fn write_lines(lines: &[String]) -> Result<()> {
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push_str(newline);
    }
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(text.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str =
        include_str!("../../../scripts/book-digest/tests/card-style-fixture.json");

    fn run_json(
        cards: &Value,
        setting: &str,
        limits: Limits,
        require_explain: bool,
        level: &str,
    ) -> Vec<String> {
        let mut lines =
            check_json(&cards.to_string(), setting, limits, require_explain, level).unwrap();
        let count = lines.len();
        lines.push(format!("faults: {count}"));
        lines
    }

    fn assert_prefixes(lines: &[String], expected: &[&str]) {
        assert_eq!(lines.len(), expected.len(), "lines: {lines:#?}");
        for (line, want) in lines.iter().zip(expected) {
            assert!(line.starts_with(want), "expected `{want}`, got `{line}`");
        }
    }

    /// card-style-check.tests.ps1: one fault for each rule, and text the checks leave alone.
    #[test]
    fn the_fixture_raises_one_fault_for_each_rule() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let lines = run_json(&fixture, "K/1|a|a", Limits::default(), false, "");
        assert_prefixes(
            &lines,
            &[
                "K/1|a|a|1 R12 (49 characters > 40) 清算比例：E 付不出 P 时",
                "K/1|a|a|1 R3 E 在时点 1 付不出去；E 能部分付出。",
                "K/1|a|a|1 R4 资产被清算。",
                "K/1|a|a|1 R5 双方进行谈判。",
                "K/1|a|a|1 R7 E 在时点 1 的用来还债的现金的上限是 $P$。",
                "K/1|a|a|1 R6 uses `卖资产` for `清算`: 值得注意的是，C 会卖资产。",
                "K/1|a|a|1 R9 `值得注意的是`: 值得注意的是，C 会卖资产。",
                "K/1|a|a|1 R6 uses `还款额` for `可承诺的还款`: 可承诺的还款等于 $B$，还款额不会更高。",
                "K/1|a|a|1 R2 (50 characters > 40) 这一句很长很长",
                "K/1|a|a|2 R2 (34 characters > 30) 代入 $P=B$ 以后",
                "K/1|a|a|3 R3 The creditor cannot verify the return; it cannot enforce a payment from it either.",
                "K/1|a|a|4 R13 (13 sentences > 12) split the part",
                "K/1|a|a|5 R14 (7 steps > 6) split the derivation into a chain of Derivation parts",
                // The card-style fixture also holds a bare deictic, which the pilot reports as advice.
                "Other/1|a|a R21 (advisory) unnamed referent `这张卡` with no named Link in its sentence: 这张卡属于另一个模型。",
                "faults: 14",
            ],
        );
        // The setting's own tables, the quotation, chart, and formulas, and a shared part's words
        // under another model's term table raise nothing. The shared part's one bare deictic is
        // the pilot's advice, which its own text earns.
        let skipped = Regex::new(r"引文|图里|清算价值；|^Other/").unwrap();
        for line in &lines {
            let shared_advice = line.starts_with("Other/") && line.contains(" R21 (advisory) ");
            assert!(
                !skipped.is_match(line) || shared_advice,
                "checked text it must skip: {line}"
            );
        }
    }

    fn small() -> Value {
        json!([
            { "address": "S/1|a|a", "facet": "设定", "title": "设定", "body": "E 有一个项目，项目在时点 2 产生收益。C 出资，换取时点 1 的还款。E 可以挪用一部分收益。" },
            { "address": "S/1|a|a|1", "facet": "推导", "title": "推导", "body": "1. E 在时点 1 还款以后，手里剩下项目的收益。\n2. E 在时点 2 挪用一部分收益，留下其余部分。\n3. 两边加上初始财富，得到参与约束。" },
        ])
    }

    /// R13 counts prose characters too; a Derivation part's steps count under R14 only.
    #[test]
    fn card_size_counts_characters_and_steps() {
        let limits = Limits {
            max_chars: 30,
            max_steps: 2,
            ..Limits::default()
        };
        let size = run_json(&small(), "", limits.clone(), false, "");
        assert_prefixes(
            &size,
            &["S/1|a|a R13 (", "S/1|a|a|1 R14 (3 steps > 2)", "faults: 2"],
        );
        assert!(
            size[0].contains("characters > 30"),
            "R13 must count characters: {}",
            size[0]
        );
        // A concept model keeps its few parts whole: no size faults.
        let concept = run_json(&small(), "", limits, false, "concept");
        assert_eq!(concept, ["faults: 0"]);
    }

    fn explain_cards() -> Value {
        let steps = "**步骤**\n1. 参与约束：E 的所得高于 $w$，即 $R+fy_2>w$。\n2. 情形 $f=1$：$R=c-P$。\n   说明：不清算时，现金够还款，E 留下差额。\n3. 两边加 $K-w$，得 $y_1+fy_2>K$。";
        let good = format!(
            "**思路**\nE 只在还款后所得超过 $w$ 时参与。先求 E 还款后剩下的现金 $R$。\n\n**起点**\n- 依赖设定 [[X/1|a|a]]：E 参与，须所得高于 $w$。\n\n{steps}\n\n**结果**\n$$y_1+fy_2>K$$\n\n**解读**\n$w$ 在两边相消，所以 E 的财富不影响参与。检验：$f=1$ 时它是普通的净现值条件。"
        );
        let long = (1..=17)
            .map(|n| format!("第 {n} 句解释推导的一步。"))
            .collect::<String>();
        let without_reading = good[..good.find("**解读**").unwrap()].trim().to_string();
        json!([
            { "address": "X/1|a|a|1", "facet": "推导：参与约束", "title": "推导：参与约束", "body": good },
            { "address": "X/1|a|a|2", "facet": "推导：缺解读", "title": "推导：缺解读", "body": without_reading },
            { "address": "X/1|a|a|3", "facet": "推导：两行说明", "title": "推导：两行说明", "body": good.replace("留下差额。", "留下差额。\n   说明：这一行多余。") },
            { "address": "X/1|a|a|4", "facet": "推导：太长", "title": "推导：太长", "body": good.replace("**解读**\n", &format!("**解读**\n{long}\n")) },
            { "address": "X/1|a|a|5", "facet": "推导：旧格式", "title": "推导：旧格式", "body": format!("**起点**\n- 依赖设定 [[X/1|a|a]]。\n\n{steps}\n\n**结果**\n$$y_1+fy_2>K$$") },
        ])
    }

    /// R15 to R17: the explanation of a Derivation part.
    #[test]
    fn a_derivation_part_holds_its_explanation_sections() {
        let cards = explain_cards();
        let plain_run = run_json(&cards, "", Limits::default(), false, "");
        assert_prefixes(
            &plain_run,
            &[
                "X/1|a|a|2 R15 sections 思路, 起点, 步骤, 结果; want 思路, 起点, 步骤, 结果, 解读",
                "X/1|a|a|3 R17 a second 说明 line under step 2",
                "X/1|a|a|4 R16 (解读 has 19 sentences > 6)",
                "X/1|a|a|4 R17 (22 explanation sentences > 16)",
                "faults: 4",
            ],
        );
        // --require-explain flags the older part too.
        let strict = run_json(&cards, "", Limits::default(), true, "");
        assert!(
            strict
                .iter()
                .any(|line| line.starts_with("X/1|a|a|5 R15 sections 起点, 步骤, 结果")),
            "{strict:#?}"
        );
        assert_eq!(strict.last().unwrap(), "faults: 5");
        // The explanation does not count under R13: 12 sentences of 思路 and 解读 fit a 3-sentence part.
        let limits = Limits {
            max_sentences: 3,
            max_explain_sentences: 40,
            max_explain_chars: 2000,
            ..Limits::default()
        };
        let r13 = run_json(&cards, "", limits, false, "");
        let part = Regex::new(r"^X/1\|a\|a\|[1-4] R13 ").unwrap();
        assert!(!r13.iter().any(|line| part.is_match(line)), "{r13:#?}");
    }

    #[test]
    fn the_position_line_of_a_chain_is_not_the_writers_text() {
        let body = "**思路**\n第 2 张，共 3 张：这一行很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长。\n本张求出剩下的现金。\n\n**起点**\n- 依赖设定 [[X/1|a|a]]。\n\n**步骤**\n1. 代入 $P=B$。\n\n**结果**\n$$R=c$$\n\n**解读**\n现金决定参与。";
        let lines = run_json(
            &json!([{ "address": "X/1|a|a|1", "facet": "推导", "title": "推导：现金", "body": body }]),
            "",
            Limits::default(),
            true,
            "",
        );
        assert_eq!(lines, ["faults: 0"]);
    }

    /// The Card layer rules read the entry's category, Links, kinds, and tree
    /// (the expected lines are card-style-check.ps1's own output on these entries).
    #[test]
    fn the_layer_rules_read_category_links_and_kinds() {
        let cards = json!([
            { "address": "5/1", "title": "税", "body": "引出 [[5/2]]。参见 [[5/3]]。\n\n依赖 [[5/3]] 的结论，税负落在买方。", "category": "idea", "parent": "", "links": ["5/2", "5/3"], "kinds": ["5/3"], "book": "Hart1995", "tree": "5" },
            { "address": "5/2", "title": "1999 年的亚洲金融危机", "body": "时间：1999 年。\n地点：亚洲。", "category": "event", "links": [], "inbound": ["5/1"], "parent": "5/1", "inbound_kinded": 0 },
            { "address": "5/3", "title": "1999 年亚洲的金融危机", "body": "- **时间**：1999 年。\n- 地点：亚洲。\n参与者：银行。\n经过：挤兑。\n来源：[[Hart1995/2]]", "category": "event", "links": ["Hart1995/2"], "kinds": [], "inbound": [], "inbound_kinded": 1, "book": "Hart1995" },
            { "address": "5/4", "title": "商品税的负担落在买方身上", "body": "商品税的负担落在买方身上，因为需求缺乏弹性。来源：[[Hart1995]]。", "category": "idea", "parent": "5/3", "links": ["Hart1995"], "kinds": [], "tree": "5" },
            { "address": "5/5", "title": "商品税的负担落在买方身上了", "body": "同上。", "category": "idea", "parent": "5/4", "links": [], "kinds": [], "tree": "5" },
        ]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        assert_eq!(
            lines,
            [
                "5/1 I1 (advisory) the title states no claim: 税",
                "5/1 I2 (19 characters < 40) merge candidate",
                "5/1 I3 no source line: no Link to [[Hart1995]] or one of its anchors",
                "5/1 L2 (2 characters < 8) a Link with no reason: 引出 [[5/2]]。",
                "5/1 L2 (2 characters < 8) a Link with no reason: 参见 [[5/3]]。",
                "5/1 L3 (advisory) [[5/3]] is linked in 2 paragraphs",
                "5/2 E2 missing labels: 参与者、经过、来源",
                "5/2 E4 no Idea Card links it with a kind word",
                "5/3 I5 no other Card has an authored Link here",
                "5/4 I2 (26 characters < 40) merge candidate",
                "5/4 I4 no kinded Link to a Card other than the parent",
                "5/5 I2 (3 characters < 40) merge candidate",
                "5/5 I4 no kinded Link to a Card other than the parent",
                "5/5 I6 (advisory) a title very similar to [[5/4]]",
                "5/3 E5 (advisory) the same event as [[5/2]]?",
                "faults: 15",
            ]
        );
    }

    /// F1 to F4: the labels, the source line, one result per Card, and a kinded Link in.
    #[test]
    fn the_finding_rules_read_labels_the_source_line_and_one_result() {
        let good = "测量：企业是否因为缺少保险市场而推迟投资。\n数据：48 家企业的访谈，2024 年，美国。\n方法：半结构化访谈。\n结果：31 家企业推迟了投资。\n局限：样本不是随机样本。\n来源：[[Smith2024/3]] §3.2\n\n这一结果印证 [[5/1]] 的判断：缺少市场会压低投资。";
        let cards = json!([
            { "address": "5/2", "title": "31 家受访企业因为缺少保险市场推迟了投资", "body": good, "category": "finding", "links": ["Smith2024/3", "5/1"], "kinds": ["5/1"], "inbound": ["5/1"], "parent": "5/0", "inbound_kinded": 1, "book": "Smith2024" },
            { "address": "5/3", "title": "受访企业推迟了投资", "body": "- **测量**：投资时点。\n- 数据：48 家企业。\n- 结果：\n  - 31 家推迟投资。\n  - 12 家取消投资。\n- 局限：自报原因。\n来源：见第三章", "category": "finding", "links": [], "kinds": [], "inbound": ["5/1"], "parent": "5/0", "inbound_kinded": 0, "book": "Smith2024" },
            { "address": "5/4", "title": "两项结果写在一张卡里", "body": "测量：投资。\n数据：企业。\n方法：访谈。\n结果：推迟。\n结果：取消。\n局限：小样本。\n来源：", "category": "finding" },
        ]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        let finding = lines
            .iter()
            .filter(|line| {
                line.split(' ')
                    .nth(1)
                    .is_some_and(|rule| rule.starts_with('F'))
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            finding,
            [
                "5/3 F1 missing labels: 方法",
                "5/3 F2 the source line links neither [[Smith2024]] nor one of its anchors",
                "5/3 F3 (2 results under 结果) one result per Card: split it",
                "5/3 F4 no Idea Card links it with a kind word",
                "5/4 F2 the source line names no source",
                "5/4 F3 (2 结果 labels) one result per Card: split it",
            ]
        );
        // A Finding Card takes the shared layer rules too, but not I3: F2 checks its source line.
        assert!(
            !lines.iter().any(|line| line.starts_with("5/2 ")),
            "{lines:#?}"
        );
        assert!(
            !lines.iter().any(|line| line.contains(" I3 ")),
            "{lines:#?}"
        );
        let (kept, count) = filter_rules(&lines, DEFAULT_RULES);
        assert!(kept.iter().any(|line| line.starts_with("5/3 F1 ")));
        assert!(
            !kept.iter().any(|line| line.contains(" F4 ")),
            "F4 is not a default rule"
        );
        assert!(count >= 5);
    }

    fn of_rule<'a>(lines: &'a [String], rule: &str) -> Vec<&'a str> {
        lines
            .iter()
            .map(String::as_str)
            .filter(|line| line.split(' ').nth(1) == Some(rule))
            .collect()
    }

    /// R20 and R21: a local label that stands in for its content, and a deictic
    /// with no named Link in its sentence. Both are advice, not blocking faults.
    #[test]
    fn local_references_and_referents_are_advice() {
        let cards = json!([
            { "address": "T/1|a", "title": "式 (12) 有该项", "body": "这一张只给了编号。" },
            { "address": "T/1|b", "title": "加成项来自需求弹性",
              "body": "式 (12) 的加成项来自需求弹性。\n\n命题 3.G.3 说明均衡唯一。\n\n来源：[[book]] §11.D" },
            { "address": "T/1|c", "title": "第一福利定理成立", "body": "第一福利定理在这里成立。\n\n§11.D 有该项。" },
            { "address": "T/1|d", "title": "代码里的编号", "body": "```\n式 (12) 有该项\n```\n\n> 式 (12) 有该项\n\n`式 (12)` 有该项。" },
            { "address": "T/1|e", "title": "该卡说明了均衡", "body": "那里给出了答案。" },
            { "address": "T/1|f", "title": "均衡的存在性", "body": "该卡 [[T/1|a]] 说明了均衡。\n\n先看 [[T/1|a]] 的方法。该卡给出了结果。" },
            { "address": "T/1|g", "title": "平均分成三份", "body": "第 6 章给出了结论。\n\n本章。" },
            { "address": "T/1|h", "category": "anchor", "title": "第 3 组", "body": "§3.1 稀缺与配置\n\n§3.2 市场均衡" },
        ]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "R20"),
            [
                "T/1|a R20 (advisory) local reference `式 (12)` names no content: 式 (12) 有该项",
                "T/1|c R20 (advisory) local reference `§11.D` names no content: §11.D 有该项。",
                "T/1|g R20 (advisory) local reference `本章` names no content: 本章。",
            ]
        );
        assert_eq!(
            of_rule(&lines, "R21"),
            [
                "T/1|e R21 (advisory) unnamed referent `该卡` with no named Link in its sentence: 该卡说明了均衡",
                "T/1|e R21 (advisory) unnamed referent `那里` with no named Link in its sentence: 那里给出了答案。",
                "T/1|f R21 (advisory) unnamed referent `该卡` with no named Link in its sentence: 该卡给出了结果。",
            ]
        );
        // Advice: a default self-check reports the lines but counts no fault for them.
        let (kept, faults) = filter_rules(&lines, DEFAULT_RULES);
        assert!(kept.iter().any(|line| line.contains(" R20 (advisory) ")));
        assert_eq!(faults, 0);
    }

    /// I7: a Topic Idea Card whose `rests-on` Link enters its own source's
    /// Literature tree. A source Link, `illustrates`, another book, and a Card
    /// in that Literature tree are no fault; missing source evidence is unknown.
    #[test]
    fn own_source_dependencies_are_reported() {
        let idea = |address: &str, tree: &str, body: &str, target: &str, kind: &str| {
            json!({
                "address": address, "category": "idea", "tree": tree, "parent": format!("{tree}/1"),
                "title": format!("{} 的前提", address), "body": body,
                "links": [target], "kinds": [target], "relations": { target: kind },
            })
        };
        let mut own = idea("12/3/1|a", "12/3", "依赖 [[book/4]] 的定义：所有者能拒绝合作。", "book/4", "rests-on");
        own["sources"] = json!(["book"]);
        let mut illustrates = idea("12/3/1|b", "12/3", "例示 [[book/5]] 的两期模型。", "book/5", "illustrates");
        illustrates["sources"] = json!(["book"]);
        let mut other = idea("12/3/1|c", "12/3", "依赖 [[other/2]] 的前提：投资不可转用。", "other/2", "rests-on");
        other["sources"] = json!(["book"]);
        let mut in_book = idea("book/4|a", "book", "依赖 [[book/4]] 的定义：控制权随状态转移。", "book/4", "rests-on");
        in_book["sources"] = json!(["book"]);
        let unknown = idea("12/3/1|d", "12/3", "依赖 [[book/6]] 的结论：事后分配由地位决定。", "book/6", "rests-on");
        let mut from_body = idea("12/3/1|e", "12/3", "依赖 [[book/7]] 的设定。\n\n来源：[[book]] §3.2", "book/7", "rests-on");
        from_body["links"] = json!(["book/7", "book"]);
        let cards = json!([own, illustrates, other, in_book, unknown, from_body]);
        let lines = run_json(&cards, "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "I7"),
            [
                "12/3/1|a I7 `rests-on` [[book/4]] enters its own source `book`",
                "12/3/1|d I7 (advisory) source evidence unknown, so [[book/6]] may be an own-source premise",
                "12/3/1|e I7 `rests-on` [[book/7]] enters its own source `book`",
            ]
        );
    }

    #[test]
    fn lengths_count_utf16_units_and_a_clip_ends_with_an_ellipsis() {
        assert_eq!(length(&utf16_form("a😀b")), 4);
        assert_eq!(from_utf16_form(&utf16_form("a😀b")), "a😀b");
        let mut faults = Faults::default();
        let long = format!("{}😀tail", "x".repeat(88));
        faults.add("A/1", "R2", &utf16_form(&long));
        assert_eq!(
            faults.lines[0],
            format!("A/1 R2 {}\u{FFFD}…", "x".repeat(88))
        );
    }

    #[test]
    fn sentences_split_after_end_marks_and_keep_formulas_whole() {
        assert_eq!(
            split_sentences("甲。乙！ 丙 $a. b$ 丁. Next one? [[x.y]] end"),
            ["甲。", "乙！", "丙 $a. b$ 丁.", "Next one?", "[[x.y]] end"]
        );
    }

    #[test]
    fn culture_order_puts_punctuation_before_digits_and_ignores_case() {
        let mut keys = vec!["3/10", "3/1|a", "b", "A", "3/1"];
        keys.sort_by(|a, b| culture_cmp(a, b));
        assert_eq!(keys, ["3/1", "3/1|a", "3/10", "A", "b"]);
        assert!(culture_starts_with("\u{200B}| a |", "|"));
        assert!(!culture_starts_with("|\u{0301}", "|"));
    }

    #[test]
    fn the_rule_filter_keeps_asked_rules_and_counts_hard_faults() {
        let lines = vec![
            "a/1 R6 uses x".to_string(),
            "a/1 R3 x；y".to_string(),
            "a/1 I1 (advisory) the title states no claim: x".to_string(),
        ];
        let (kept, count) = filter_rules(&lines, DEFAULT_RULES);
        assert_eq!(
            kept,
            [
                "a/1 R3 x；y",
                "a/1 I1 (advisory) the title states no claim: x"
            ]
        );
        assert_eq!(count, 1);
    }
}
