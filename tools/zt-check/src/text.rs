//! Pure text operations for the Card check: the PowerShell text semantics
//! (UTF-16 lengths, culture comparisons, hashtable keys), the prose projection
//! the rules read, the term-table parse, the title comparison, and the lexical
//! Link and envelope operations.
//!
//! Nothing here reads a file, starts a process, or talks to an archive, so the
//! one implementation serves the `zt-check` CLI, zt-digest, and the zt service's
//! in-process check.

use regex::{Captures, Regex};
use serde_json::Value;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;

/// One compiled pattern: a rule or a text shape, built once per process.
macro_rules! pattern {
    ($name:ident, $pattern:expr) => {
        pub(crate) static $name: LazyLock<Regex> =
            LazyLock::new(|| Regex::new($pattern).expect("a valid check pattern"));
    };
}
pub(crate) use pattern;

// Prose: what the rules apply to.
pattern!(DISPLAY_MATH, r"\$\$[\s\S]*?\$\$");
pattern!(INLINE_MATH, r"\$[^$\n]+\$");
pattern!(LINK, r"\[\[[^\]]+\]\]");
pattern!(LINK_TARGET, r"\[\[([^\]]+)\]\]");
pattern!(CODE, r"`[^`\n]*`");
pattern!(FENCED, r"```[\s\S]*?```");
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

// The term table.
pattern!(TABLE_RULE, r"^-+$|^:?-+:?$");
pattern!(AVOID_SPLIT, r"[、,，/]");
pattern!(AVOID_NONE, r"^[-—无]+$");

pattern!(BOOK_CARD, r"^[^\d/|][^/|]*$");
pattern!(BOOK_ANCHOR, r"^[^\d/|][^/|]*/\d+$");
pattern!(SOURCE_LINE, r"(?i)^\s*(来源|出处|Source)\s*[：:]");
// The Literature Card's document line (`原书文档：[原书 PDF](scholar:78) · …`): a row of document links, a
// locator like the source line, never a prose sentence.
pattern!(DOCUMENT_LINE, r"^\s*原书文档\s*[：:]");
// A book title in 《…》 is one name: a long title of an English book must not make a short sentence long.
pattern!(BOOK_TITLE, r"《[^》\n]{1,200}》");
pattern!(REASON_STRIP, r"[\s\p{P}\p{S}]");
pattern!(NOT_LETTER_OR_NUMBER, r"[^\p{L}\p{N}]");

// What ICU collation (and so a culture string comparison) skips: control and
// format characters, and variation selectors.
pattern!(
    IGNORABLE,
    r"[\x{0}-\x{8}\x{E}-\x{1F}\x{7F}-\x{84}\x{86}-\x{9F}\p{Cf}\x{FE00}-\x{FE0F}]"
);
pattern!(MARK, r"^\p{M}");

// --- UTF-16 form ------------------------------------------------------------------------------

const HIGH_STAND_IN: u32 = 0xE000;
const LOW_STAND_IN: u32 = 0xE400;

pub(crate) fn is_high_stand_in(ch: char) -> bool {
    (HIGH_STAND_IN..LOW_STAND_IN).contains(&(ch as u32))
}

pub(crate) fn is_low_stand_in(ch: char) -> bool {
    (LOW_STAND_IN..LOW_STAND_IN + 0x400).contains(&(ch as u32))
}

/// The text with each character outside the BMP as two stand-ins, so one
/// `char` is one UTF-16 code unit.
pub(crate) fn utf16_form(text: &str) -> String {
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
pub(crate) fn from_utf16_form(text: &str) -> String {
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
pub(crate) fn length(text: &str) -> i64 {
    text.chars().count() as i64
}

// --- PowerShell comparisons -------------------------------------------------------------------

/// The text without the characters a culture comparison ignores.
pub(crate) fn culture(text: &str) -> Cow<'_, str> {
    IGNORABLE.replace_all(text, "")
}

/// `-eq` on strings: case and ignorable characters do not count.
pub(crate) fn ps_eq(a: &str, b: &str) -> bool {
    a == b || culture(a).to_lowercase() == culture(b).to_lowercase()
}

pub(crate) fn ps_in(value: &str, list: &[impl AsRef<str>]) -> bool {
    list.iter().any(|item| ps_eq(value, item.as_ref()))
}

/// `String.StartsWith(string)`: a culture comparison, so an ignorable
/// character before the prefix does not count, and a combining mark after it
/// breaks the match.
pub(crate) fn culture_starts_with(text: &str, prefix: &str) -> bool {
    let text = culture(text);
    text.starts_with(prefix) && !MARK.is_match(&text[prefix.len()..])
}

/// A hashtable key: hashtables ignore case.
pub(crate) fn key(text: &str) -> String {
    text.to_lowercase()
}

/// The primary collation weight of a character (ICU root order for ASCII).
pub(crate) fn weight(ch: char) -> u32 {
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
pub(crate) fn culture_cmp(a: &str, b: &str) -> Ordering {
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
pub(crate) fn prop<'a>(card: &'a Value, name: &str) -> Option<&'a Value> {
    let object = card.as_object()?;
    object.get(name).or_else(|| {
        object
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
    })
}

/// `[string]$value`.
pub(crate) fn ps_string(value: Option<&Value>) -> String {
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
pub(crate) fn text(value: Option<&Value>) -> String {
    utf16_form(&ps_string(value))
}

/// Whether PowerShell reads a value as true.
pub(crate) fn truthy(value: &Value) -> bool {
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
pub(crate) fn strings(value: Option<&Value>) -> Vec<String> {
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
pub(crate) fn ps_int(value: Option<&Value>) -> i64 {
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
pub(crate) fn plain(text: &str) -> String {
    let text = DISPLAY_MATH.replace_all(text, "F");
    let text = INLINE_MATH.replace_all(&text, "F");
    let text = LINK.replace_all(&text, "");
    let text = CODE.replace_all(&text, "C");
    BOLD.replace_all(&text, "").into_owned()
}

pub(crate) struct Measure {
    pub(crate) size: i64,
    pub(crate) words: bool,
}

impl Measure {
    pub(crate) fn unit(&self) -> &'static str {
        if self.words { "words" } else { "characters" }
    }
}

/// A sentence's size: characters when it holds Chinese, else words.
pub(crate) fn measure(sentence: &str) -> Measure {
    let plain = plain(sentence);
    let plain = BOOK_TITLE.replace_all(&plain, "书").into_owned();
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

/// Whether a title keeps R12's length limit (40 characters, or 25 words).
pub fn title_fits(title: &str) -> bool {
    let m = measure(title);
    m.size <= if m.words { 25 } else { 40 }
}

/// Whether one prose sentence that is not a step keeps R2's length limit (40 characters, or 25 words).
pub fn sentence_fits(sentence: &str) -> bool {
    let m = measure(sentence);
    m.size <= if m.words { 25 } else { 40 }
}

pub(crate) struct ProseLine {
    pub(crate) text: String,
    pub(crate) step: bool,
}

/// The lines of prose in a body, each with whether it is a numbered step.
/// The list lines of a body's `**结构**` section (each with a Link), as `prose_lines` gives their text.
pub(crate) fn structure_lines(body: &str) -> HashSet<String> {
    let mut lines = HashSet::new();
    let mut inside = false;
    for raw in body.split('\n') {
        let trim = raw.trim();
        if trim.starts_with("**结构**") {
            inside = true;
            continue;
        }
        if !inside {
            continue;
        }
        if trim.starts_with("**") || HEADING.is_match(trim) {
            inside = false;
            continue;
        }
        if trim.contains("[[") {
            lines.insert(MARKER.replace(trim, "").trim().to_string());
        }
    }
    lines
}

pub(crate) fn prose_lines(body: &str) -> Vec<ProseLine> {
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
/// R22: whether a text leaves an inline formula open, an odd number of unescaped `$` once its
/// `$$` display blocks and its code are set aside. A writer that builds a Card in a PowerShell
/// double-quoted string loses `$A_I` (an unset variable) and leaves `当且仅当 <A_O$`: on
/// grossman2002integration trial 8 two titles of group 4 reached the archive that way.
pub(crate) fn open_math(text: &str) -> bool {
let probe = FENCED.replace_all(text, "");
let probe = DISPLAY_MATH.replace_all(&probe, "");
let probe = CODE.replace_all(&probe, "");
let mut count = 0usize;
let mut escaped = false;
for c in probe.chars() {
    if c == '$' && !escaped {
        count += 1;
    }
    escaped = c == '\\' && !escaped;
}
count % 2 == 1
}

/// The sentences of a line; formulas and Links split nothing.
pub(crate) fn split_sentences(line: &str) -> Vec<String> {
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
    // A book title in 《…》 is one name: its own ? or ! ends no sentence.
    let mut in_title = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        current.push(ch);
        index += 1;
        if ch == '《' {
            in_title += 1;
        } else if ch == '》' {
            in_title = in_title.saturating_sub(1);
        } else if in_title > 0 {
        } else if matches!(ch, '。' | '！' | '？' | '!' | '?') {
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
pub(crate) fn passive(plain: &str) -> bool {
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

pub(crate) type Terms = Vec<(String, Vec<String>)>;

/// The cells of one term-table row: `(术语, 不用, 意思, 首次定义)`. `None` for the header, the
/// rule, or a line that is not a row. `不用` keeps R6's split (`、,，/`), its trim of the marks
/// the table allows, and its drop of the words that mean "none" (`—`, `无`).
pub(crate) fn term_cells(t: &str) -> Option<(String, Vec<String>, String, String)> {
    let cells = t
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect::<Vec<_>>();
    if cells.len() < 2 || TABLE_RULE.is_match(cells[0]) || ps_eq(cells[0], "术语") {
        return None;
    }
    let avoid = AVOID_SPLIT
        .split(cells[1])
        .map(|word| word.trim_matches(&[' ', '`', '“', '”'][..]))
        .filter(|word| !word.is_empty() && !AVOID_NONE.is_match(word))
        .map(str::to_string)
        .collect::<Vec<_>>();
    Some((
        cells[0].to_string(),
        avoid,
        cells.get(2).map(|cell| cell.to_string()).unwrap_or_default(),
        cells.get(3).map(|cell| cell.to_string()).unwrap_or_default(),
    ))
}

/// One row of a book's term table: the four columns `术语 | 不用 | 意思 | 首次定义`
/// (docs/luhmann-comparison-implementation-design.md D063). The digest reads the meanings
/// and definition locations beside the term and its rejected words; R6 still reads only the
/// first two columns.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TermRow {
    /// `术语`: the preferred word of the book.
    pub term: String,
    /// `不用`: the words the book rejects for this term (its own naming evidence).
    pub avoid: Vec<String>,
    /// `意思`: the meaning the book gives the term.
    pub meaning: String,
    /// `首次定义`: where the book first defines it (`§2.1`).
    pub defined: String,
}

/// The term table of one book Card, with all four columns (D063). The digest uses this to
/// read a book's naming evidence; it applies the same row parse R6 applies, so the two never
/// disagree about what a row says. The table ends at the first line that is not a row once a
/// row has been read; a later row that names the same term (case ignored) replaces the earlier
/// one, as R6's ordered hashtable does.
pub fn term_table(body: &str) -> Vec<TermRow> {
    let mut rows: Vec<TermRow> = Vec::new();
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
            if rows.is_empty() {
                continue;
            }
            break;
        }
        let Some((term, avoid, meaning, defined)) = term_cells(t) else { continue };
        let row = TermRow { term, avoid, meaning, defined };
        match rows.iter_mut().find(|known| key(&known.term) == key(&row.term)) {
            Some(entry) => *entry = row,
            None => rows.push(row),
        }
    }
    rows
}

/// The term table under a line that holds `术语表`: the term, and the words not to use.
pub(crate) fn read_terms(body: &str) -> Terms {
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
        let Some((term, avoid, _, _)) = term_cells(t) else { continue };
        if avoid.is_empty() {
            continue;
        }
        // An ordered hashtable: a term that differs only in case replaces the earlier one.
        match terms.iter_mut().find(|(known, _)| key(known) == key(&term)) {
            Some(entry) => *entry = (term, avoid),
            None => terms.push((term, avoid)),
        }
    }
    terms
}

// --- the checks -------------------------------------------------------------------------------

/// One title projection of the Workspace context: a read-only comparison row.
#[derive(Clone, Debug)]
pub(crate) struct TitleRow {
    pub(crate) address: String,
    pub(crate) title: String,
    pub(crate) workspace: String,
    /// A restatement entry is navigation: its title is not a substantive claim, so it is neither
    /// checked nor offered as a duplicate-title candidate (§4.5, D086).
    pub(crate) restatement: bool,
}

/// The Workspace title context I6 compares with (D135): rows of one bulk title
/// projection, never an edit target. `complete` is false when the reader could not
/// supply the whole Workspace, so no run claims a Workspace-wide check from one tree.
/// `supplied` is false only when no context was passed at all; that documented mode
/// compares the entries the run holds and makes no Workspace-wide claim.
#[derive(Clone, Debug, Default)]
pub struct TitleContext {
    pub(crate) titles: Vec<TitleRow>,
    pub(crate) complete: bool,
    pub(crate) supplied: bool,
}

impl TitleContext {
    /// The context of one `export_titles` answer (or its rows).
    pub fn from_titles(titles: &[Value]) -> Self {
        let mut rows = Vec::new();
        for row in titles {
            let address = text(prop(row, "address"));
            if address.is_empty() {
                continue;
            }
            rows.push(TitleRow {
                address,
                title: text(prop(row, "title")),
                workspace: text(prop(row, "workspace")),
                restatement: text(prop(row, "restatement")).eq_ignore_ascii_case("true"),
            });
        }
        Self { titles: rows, complete: true, supplied: true }
    }

    /// The context of a `--json` bundle's `context` object: the envelope the CLI
    /// adapter reads, so only the `cli` build needs it.
    #[cfg(feature = "cli")]
    pub(crate) fn from_json(context: &Value) -> Self {
        let rows = context["titles"].as_array().cloned().unwrap_or_default();
        let mut parsed = Self::from_titles(&rows);
        parsed.complete = context["complete"] == true;
        parsed
    }
}

/// The Links and the list lines of a Card's `## 同述入口` section (§4.5, D084): the navigation
/// references an accepted restatement entry writes. A heading inside a code fence opens nothing.
pub(crate) fn entry_section(body: &str) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut inside = false;
    let mut fenced = false;
    let mut entry_links: Vec<String> = Vec::new();
    let mut outside_links: Vec<String> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for raw in body.lines() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(heading) = trimmed
            .strip_prefix("## ")
            .or_else(|| trimmed.strip_prefix("### "))
        {
            inside = heading.trim() == "同述入口";
            continue;
        }
        for caps in LINK_TARGET.captures_iter(trimmed) {
            let target = caps[1].trim().to_string();
            let known = if inside { &mut entry_links } else { &mut outside_links };
            if !target.is_empty() && !known.iter().any(|seen| ps_eq(seen, &target)) {
                known.push(target);
            }
        }
        if inside && LIST_LINE.is_match(trimmed) {
            lines.push(LIST_LINE.replace(trimmed, "").into_owned());
        }
    }
    (entry_links, lines, outside_links)
}

pub(crate) static LIST_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:[-*+]|\d+[.)])\s+").unwrap());
/// A source line or a place in the book: a Literature Card (`hart1995firms`)
/// or an anchor (`hart1995firms/4`). Topic trees are numbered, so `12/3` is no anchor.
pub(crate) fn book_place(target: &str) -> bool {
    BOOK_CARD.is_match(target) || BOOK_ANCHOR.is_match(target)
}

/// The Literature tree a Link target names: the Citation key of a book Card or
/// of one of its anchors. A Topic address names no Literature tree. The write
/// gate uses the same definition to name the book a proposed Card belongs to.
pub fn literature_tree(target: &str) -> Option<String> {
    if BOOK_ANCHOR.is_match(target) {
        return target.split_once('/').map(|(key, _)| key.to_string());
    }
    BOOK_CARD.is_match(target).then(|| target.to_string())
}

/// A Link that 引出 announces in its sentence: it points the other way.
pub(crate) fn leads_to(body: &str, target: &str) -> bool {
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
pub(crate) fn bigrams(title: &str) -> HashSet<String> {
    let plain = PLACEHOLDERS.replace_all(&plain(title), "").into_owned();
    let chars = NOT_LETTER_OR_NUMBER
        .replace_all(&plain, "")
        .chars()
        .collect::<Vec<_>>();
    chars.windows(2).map(|pair| pair.iter().collect()).collect()
}

pub(crate) fn similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let common = a.iter().filter(|bigram| b.contains(*bigram)).count();
    2.0 * common as f64 / (a.len() + b.len()) as f64
}

/// I6's near-title threshold (D129): the Dice coefficient of the two bigram sets.
pub const TITLE_SIMILARITY: f64 = 0.8;

/// A title's identity for an exact or near pair report (D361): its letters and digits, lower
/// cased, so case, spacing and punctuation never split two spellings of one title. The audit
/// and the checker read one normalization, so a report never introduces a second title rule.
pub fn title_key(title: &str) -> String {
    let plain = PLACEHOLDERS.replace_all(&plain(title), "").into_owned();
    NOT_LETTER_OR_NUMBER.replace_all(&plain, "").to_lowercase()
}

/// The bigrams of a title, as the checker's I6 compares them: for a candidate index in a
/// report so that a near-title scan is not a pairwise scan of the whole archive.
pub fn title_bigrams(title: &str) -> HashSet<String> {
    bigrams(title)
}

/// I6's near-title measure between two titles: the Dice coefficient of their bigrams, 0.0
/// when either title has no bigram. `TITLE_SIMILARITY` is the threshold I6 reports at.
pub fn title_similarity(a: &str, b: &str) -> f64 {
    similarity(&bigrams(a), &bigrams(b))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_identity_folds_case_and_punctuation() {
        assert_eq!(title_key("Alpha  Beta!"), "alphabeta");
        assert!(title_similarity("剩余控制权决定契约空白处", "剩余控制权决定契约的空白处") >= TITLE_SIMILARITY);
        assert_eq!(title_similarity("", "空白"), 0.0);
        assert!(title_bigrams("ab").contains("ab"));
    }


    /// D063: the digest reads the whole term row through the checker's own parse, while R6
    /// keeps reading only `术语` and `不用`. Adding the two columns changes no R6 fault, and a
    /// row without a rejected word is still not part of R6's table.
    #[test]
    fn the_term_table_exposes_all_four_columns_without_changing_r6() {
        let body = "本卡是《书》的术语表。本书的卡片用左列的术语。\n\n术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡的多重性 | 多重均衡、多重均衡性 | 同一经济有多个均衡的可能性 | §2.1 |\n| 学习（个体行为） | 学习 | 个体重复练习带来的技能变化 | §3.1 |\n| 学习（产业生产率） | 学习 | 产业生产率随累计产量提高 | §4.2 |\n| 剩余控制权 | — | 契约没写明时决定资产用法的权利 | §1 |\n\n来源：[[书]]";
        let rows = term_table(body);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0], TermRow {
            term: "均衡的多重性".into(),
            avoid: vec!["多重均衡".into(), "多重均衡性".into()],
            meaning: "同一经济有多个均衡的可能性".into(),
            defined: "§2.1".into(),
        });
        assert_eq!(rows[2].avoid, vec!["学习".to_string()]);
        assert_eq!(rows[2].meaning, "产业生产率随累计产量提高");
        assert_eq!(rows[3].defined, "§1");
        assert!(rows[3].avoid.is_empty());
        // R6 keeps only the rows with a rejected word, and the same two occurrences of 学习.
        assert_eq!(
            read_terms(body),
            vec![
                ("均衡的多重性".to_string(), vec!["多重均衡".to_string(), "多重均衡性".to_string()]),
                ("学习（个体行为）".to_string(), vec!["学习".to_string()]),
                ("学习（产业生产率）".to_string(), vec!["学习".to_string()]),
            ]
        );
        // A row that repeats a term (case ignored) replaces the earlier one in both readers.
        let repeated = "术语表\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| GDP | 国民生产总值 | 产出 | §1 |\n| gdp | 国内生产总值 | 产出 | §2 |\n";
        assert_eq!(term_table(repeated).len(), 1);
        assert_eq!(term_table(repeated)[0].meaning, "产出");
        assert_eq!(read_terms(repeated)[0].0, "gdp");
        // A table with no rejected words anywhere is empty for R6 and whole for the digest.
        let plain = "术语表\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 敲竹杠 | — | 机会主义 | §1 |\n";
        assert!(read_terms(plain).is_empty());
        assert_eq!(term_table(plain).len(), 1);
        assert!(term_table("没有术语表的卡").is_empty());
    }

    #[test]
    fn a_book_title_keeps_its_end_marks_inside_one_sentence() {
        assert_eq!(
            split_sentences("本卡是《When does labor scarcity encourage innovation?》的术语表。下一句。"),
            ["本卡是《When does labor scarcity encourage innovation?》的术语表。", "下一句。"]
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
}
