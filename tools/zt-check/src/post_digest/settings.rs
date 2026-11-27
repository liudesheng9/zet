//! Model settings & empirical data-tree / specification-ladder audit
//! (`zt-check --model <head> --settings [--symbols] [--format json]`).
//!
//! Mirrors `skills/zt-model-settings/scripts/check-settings.mjs`.

use crate::mcp::Zt;
use crate::post_digest::charts::find_assumption_citations;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

const DECOR: [&str; 16] = [
    "bar", "hat", "tilde", "mathcal", "mathbb", "mathbf", "mathscr", "mathfrak", "boldsymbol",
    "overline", "widetilde", "widehat", "check", "dot", "ddot", "vec",
];

const GREEK: &[&str] = &[
    "alpha", "beta", "gamma", "delta", "epsilon", "varepsilon", "zeta", "eta", "theta", "vartheta",
    "iota", "kappa", "lambda", "mu", "nu", "xi", "pi", "varpi", "rho", "varrho", "sigma",
    "varsigma", "tau", "upsilon", "phi", "varphi", "chi", "psi", "omega", "Gamma", "Delta", "Theta",
    "Lambda", "Xi", "Pi", "Sigma", "Upsilon", "Phi", "Psi", "Omega", "ell",
];

const PROSE_GREEK: [(char, &str); 35] = [
    ('α', "alpha"), ('β', "beta"), ('γ', "gamma"), ('δ', "delta"), ('ε', "epsilon"),
    ('ζ', "zeta"), ('η', "eta"), ('θ', "theta"), ('κ', "kappa"), ('λ', "lambda"),
    ('μ', "mu"), ('ν', "nu"), ('ξ', "xi"), ('π', "pi"), ('ρ', "rho"), ('σ', "sigma"),
    ('τ', "tau"), ('φ', "phi"), ('χ', "chi"), ('ψ', "psi"), ('ω', "omega"),
    ('Γ', "Gamma"), ('Δ', "Delta"), ('Θ', "Theta"), ('Λ', "Lambda"), ('Π', "Pi"),
    ('Σ', "Sigma"), ('Φ', "Phi"), ('Ψ', "Psi"), ('Ω', "Omega"),
    ('ϵ', "epsilon"), ('ϑ', "theta"), ('ϖ', "pi"), ('ϱ', "rho"), ('ς', "sigma"),
];

const ASSUMP_CORE: &str = r"(?:[A-Za-z0-9]+(?:[.\-][A-Za-z0-9]+)*[′']*|[一二三四五六七八九十]+)";

static ASSUMP_TITLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?:假设|假定)(?:\s*(?:（({ASSUMP_CORE})）|\(({ASSUMP_CORE})\)|({ASSUMP_CORE})))?[：:]"
    ))
    .expect("assump title regex")
});
static ASSUMP_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?:假设|假定)\s*(?:（({ASSUMP_CORE})）|\(({ASSUMP_CORE})\)|({ASSUMP_CORE}))(?:[：:\s]|$)"
    ))
    .expect("assump name regex")
});
static INLINE_DOLLAR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$([^$]+)\$").expect("inline dollar regex"));
static FORMULA_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\$?([^$]+)\$\$?").expect("formula regex"));
static DISPLAY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\$\$[\s\S]*?\$\$").expect("display regex"));
static TEX_ENV_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\begin\s*\{[^{}]*\}(?:\s*\{[^{}]*\})?|\\end\s*\{[^{}]*\}").expect("tex env regex")
});
static TEX_TEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\\(?:text|textrm|textit|mathrm|operatorname|mbox|tag|label)\s*\{[^{}]*\}")
        .expect("tex text regex")
});
static TEX_TOK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\[A-Za-z]+|\\.|[^\s]").expect("tex tok regex"));
static TEX_SKIP_TOK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\\[ ,;:!]$|^\\(?:left|right|big|Big|bigg|Bigg|quad|qquad|displaystyle|limits)$")
        .expect("tex skip tok regex")
});
static TEX_SPACE_CMD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\[ ,;:!]").expect("tex space cmd regex"));
static SYM_CUT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"=|>|<|\\in\b|\\le\b|\\ge\b|\\equiv").expect("sym cut regex"));
static LINK_PLAIN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]@]+)\]\]").expect("plain link regex"));
static LINK_TERM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\]@]*)@[^\]]+\]\]").expect("term link regex"));
static LINK_ANY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[[^\]]+\]\]").expect("any link regex"));
static BINDS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"沿用\s*\[\[[^\]]+\]\]\s*的全部符号").expect("binds regex"));
static DEFINES_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[\s。；，：:、(（])记(?:号[:：])?\s*\$([^$]+)\$").expect("defines regex")
});
static DEFINES_CUT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":=|=|\\equiv|\\coloneqq").expect("defines cut regex"));
static HEADER_STATED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\*\*(本章设定|本阶段的设定|设定|假设|假定|记号|符号|符号表)\*\*")
        .expect("stated header regex")
});
static SYM_TABLE_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\|\s*符号\s*\|").expect("sym table line regex"));
static SYM_OR_TERM_TABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\|\s*(符号|术语)\s*\|").expect("sym or term table regex"));
static HEADING_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\*\*([^*]+)\*\*").expect("heading line regex"));
static DEPENDS_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"依赖\s*\[\[").expect("depends link regex"));
static DEPENDS_TARGET_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"依赖\s*\[\[([^\]@]+?)\]\]").expect("depends target regex"));
static FIRST_CELL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\|\s*([^|]+?)\s*\|").expect("first cell regex"));
static TEX_CMD_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\([a-zA-Z]+)").expect("tex cmd name regex"));

#[derive(Clone, Debug, Default)]
pub struct SettingCard {
    pub facet: String,
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotationRow {
    pub symbols: Vec<String>,
    pub link: Option<String>,
    pub has_third_col: bool,
}

fn is_setting_prefix(s: &str) -> bool {
    s.starts_with("设定")
        || s.starts_with("假设")
        || s.starts_with("假定")
        || s.starts_with("本章设定")
        || s.starts_with("本阶段的设定")
}

fn is_criterion_prefix(s: &str) -> bool {
    s.starts_with("判据") || s.starts_with("均衡定义") || s.starts_with("解概念")
}

fn is_definition_prefix(s: &str) -> bool {
    s.starts_with("定义")
}

fn is_assumption_prefix(s: &str) -> bool {
    s.starts_with("假设") || s.starts_with("假定")
}

fn is_notation_prefix(s: &str) -> bool {
    s.starts_with("符号")
}

fn is_setting(facet: &str, title: &str) -> bool {
    is_setting_prefix(facet) || is_setting_prefix(title)
}

fn is_criterion(facet: &str, title: &str) -> bool {
    is_criterion_prefix(facet) || is_criterion_prefix(title)
}

fn is_definition(facet: &str, title: &str) -> bool {
    is_definition_prefix(facet) || is_definition_prefix(title)
}

fn is_assumption(facet: &str, title: &str) -> bool {
    is_assumption_prefix(facet) || is_assumption_prefix(title)
}

fn is_setting_member(facet: &str, title: &str) -> bool {
    is_setting(facet, title) || is_definition(facet, title) || is_criterion(facet, title)
}

fn is_setting_tree_allowed(facet: &str, title: &str) -> bool {
    is_setting_member(facet, title)
        || facet.starts_with("基准")
        || title.starts_with("基准")
        || facet.starts_with("边界")
        || title.starts_with("边界")
}

fn is_notation(facet: &str, title: &str) -> bool {
    is_notation_prefix(facet) || is_notation_prefix(title)
}

fn strip_list_prefix(line: &str) -> (&str, bool) {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix('-').or_else(|| trimmed.strip_prefix('*'))
        && rest.starts_with(|c: char| c.is_whitespace())
    {
        return (rest.trim_start(), true);
    }
    let digits = trimmed.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digits > 0 {
        let after = &trimmed[digits..];
        if let Some(rest) = after.strip_prefix('.').or_else(|| after.strip_prefix('、')) {
            return (rest.trim_start(), true);
        }
    }
    (trimmed, false)
}

fn split_table_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let inner = t.strip_suffix('|').unwrap_or(t);
    let bytes = inner.as_bytes();
    let mut cells = Vec::new();
    let mut start = 0usize;
    let mut in_link = false;
    let mut i = 0usize;
    while i < bytes.len() {
        if !in_link && bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'|' {
            i += 2;
            continue;
        }
        if !in_link && bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            in_link = true;
            i += 2;
            continue;
        }
        if in_link && bytes[i] == b']' && i + 1 < bytes.len() && bytes[i + 1] == b']' {
            in_link = false;
            i += 2;
            continue;
        }
        if !in_link && bytes[i] == b'|' {
            cells.push(inner[start..i].trim().to_string());
            start = i + 1;
        }
        i += 1;
    }
    cells.push(inner[start..].trim().to_string());
    cells
}

fn split_top(tex: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut depth: i32 = 0;
    for ch in tex.chars() {
        if matches!(ch, '(' | '{' | '[') {
            depth += 1;
        } else if matches!(ch, ')' | '}' | ']') {
            depth -= 1;
        }
        if depth == 0 && matches!(ch, ',' | '，' | '；') {
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(ch);
        }
    }
    out
}

/// Normalize spaces in a notation symbol: drop whitespace except a single space after a `\command`
/// when immediately followed by an ASCII letter (e.g. `\bar Z`, `\theta Z`).
fn compact_tex_symbol(raw: &str) -> String {
    let no_sp = TEX_SPACE_CMD_RE.replace_all(raw, "");
    let cut = match SYM_CUT_RE.find(&no_sp) {
        Some(m) => &no_sp[..m.start()],
        None => &no_sp,
    };
    let s = cut.trim();
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            out.push('\\');
            i += 1;
            let cmd_start = i;
            while i < chars.len() && chars[i].is_ascii_alphabetic() {
                out.push(chars[i]);
                i += 1;
            }
            if i > cmd_start {
                let ws_start = i;
                while i < chars.len() && chars[i].is_whitespace() {
                    i += 1;
                }
                if i > ws_start && i < chars.len() && chars[i].is_ascii_alphabetic() {
                    out.push(' ');
                }
                continue;
            }
            if i >= chars.len() {
                break;
            }
        }
        if !chars[i].is_whitespace() {
            out.push(chars[i]);
        }
        i += 1;
    }
    out
}

pub fn notation_rows(body: &str) -> Vec<NotationRow> {
    let mut rows = Vec::new();
    let mut in_terms = false;
    for line in body.split('\n') {
        let t = line.trim();
        if t.contains("统一用语") || t.contains("术语表") {
            in_terms = true;
            continue;
        }
        if in_terms && HEADING_LINE_RE.is_match(t) {
            in_terms = false;
        }
        if in_terms || !t.starts_with('|') {
            continue;
        }
        let cells = split_table_cells(t);
        if cells.is_empty()
            || cells[0].contains("符号")
            || cells[0].contains("术语")
            || cells[0].contains("---")
        {
            continue;
        }
        let mut symbols = Vec::new();
        for cap in INLINE_DOLLAR_RE.captures_iter(&cells[0]) {
            let tex = cap.get(1).map_or("", |m| m.as_str());
            for part in split_top(tex) {
                let sym = compact_tex_symbol(&part);
                if !sym.is_empty() && !symbols.contains(&sym) {
                    symbols.push(sym);
                }
            }
        }
        let has_third_col = cells.len() >= 3;
        let first_cell = if has_third_col { cells[2].as_str() } else { "" };
        let link = LINK_PLAIN_RE
            .captures(first_cell)
            .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()));
        if !symbols.is_empty() {
            rows.push(NotationRow {
                symbols,
                link,
                has_third_col,
            });
        }
    }
    rows
}

pub fn notation_symbols(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for r in notation_rows(body) {
        for s in r.symbols {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    }
    out
}

fn is_decor(t: &str) -> bool {
    t.strip_prefix('\\').is_some_and(|name| DECOR.contains(&name))
}

fn is_letter(t: &str) -> bool {
    (t.len() == 1 && t.as_bytes()[0].is_ascii_alphabetic())
        || t.strip_prefix('\\').is_some_and(|name| GREEK.contains(&name))
}

fn formulas(body: &str) -> Vec<&str> {
    FORMULA_RE
        .captures_iter(body)
        .filter_map(|c| c.get(1).map(|m| m.as_str()))
        .collect()
}

pub fn tokens(tex: &str) -> Vec<String> {
    let s1 = TEX_ENV_RE.replace_all(tex, " ");
    let s2 = TEX_TEXT_RE.replace_all(&s1, " ");
    let mut out: Vec<String> = TEX_TOK_RE
        .find_iter(&s2)
        .map(|m| m.as_str().to_string())
        .filter(|t| !TEX_SKIP_TOK_RE.is_match(t))
        .collect();
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i + 2 < out.len() {
            if out[i] == "{" && out[i + 2] == "}" && out[i + 1] != "{" && out[i + 1] != "}" {
                let inner = out[i + 1].clone();
                out.splice(i..=i + 2, [inner]);
                changed = true;
            }
            i += 1;
        }
    }
    out
}

fn skip_group(t: &[String], mut i: usize) -> usize {
    if i >= t.len() || t[i] != "{" {
        return i;
    }
    let mut depth = 0i32;
    while i < t.len() {
        if t[i] == "{" {
            depth += 1;
        } else if t[i] == "}" {
            depth -= 1;
            if depth == 0 {
                return i;
            }
        }
        i += 1;
    }
    i
}

pub fn base_symbols(tex: &str) -> Vec<String> {
    let t = tokens(tex);
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        if t[i] == "_" || t[i] == "^" {
            i = skip_group(&t, i + 1) + 1;
            continue;
        }
        let mut sym: Option<String> = None;
        if is_decor(&t[i]) && i + 1 < t.len() {
            let next = if t[i + 1] == "{" {
                t.get(i + 2).cloned().unwrap_or_default()
            } else {
                t[i + 1].clone()
            };
            sym = Some(format!("{} {next}", t[i]));
            i = if t[i + 1] == "{" {
                skip_group(&t, i + 1)
            } else {
                i + 1
            };
        } else if is_letter(&t[i]) {
            sym = Some(t[i].clone());
        }
        if let Some(s) = sym {
            if s != "\\mathbb R" {
                let is_diff = s == "d"
                    && t.get(i + 1)
                        .is_some_and(|nxt| is_letter(nxt) || is_decor(nxt));
                let is_real_space = s == "R" && t.get(i + 1).is_some_and(|nxt| nxt == "^" || nxt == "_");
                if !is_diff && !is_real_space && !out.contains(&s) {
                    out.push(s);
                }
            }
        }
        i += 1;
    }
    out
}

fn symbol_tokens(sym: &str) -> Vec<String> {
    let t = tokens(sym);
    if let Some(open) = t.iter().position(|x| x == "(")
        && open > 0
    {
        return t[..open].to_vec();
    }
    t
}

fn scripted(t: &[String]) -> Vec<bool> {
    let mut mark = vec![false; t.len()];
    let mut i = 0;
    while i < t.len() {
        if is_decor(&t[i]) {
            let end = skip_group(t, i + 1).min(t.len().saturating_sub(1));
            for slot in mark.iter_mut().take(end + 1).skip(i + 1) {
                *slot = true;
            }
        }
        i += 1;
    }
    mark
}

fn uses_symbol(body: &str, sym: &str, longer: &[String]) -> bool {
    let want = symbol_tokens(sym);
    if want.is_empty() || (want.len() == 1 && is_decor(&want[0])) {
        return false;
    }
    let over: Vec<Vec<String>> = longer
        .iter()
        .map(|s| symbol_tokens(s))
        .filter(|o| o.len() > want.len() && o[..want.len()] == want[..])
        .collect();
    for f in formulas(body) {
        let t = tokens(f);
        let mark = scripted(&t);
        if want.len() > t.len() {
            continue;
        }
        for i in 0..=t.len() - want.len() {
            if mark[i] || t[i..i + want.len()] != want[..] {
                continue;
            }
            if over
                .iter()
                .any(|o| i + o.len() <= t.len() && t[i..i + o.len()] == o[..])
            {
                continue;
            }
            return true;
        }
    }
    false
}

fn start_end_idx(lines: &[&str], start_idx: usize) -> usize {
    for (idx, l) in lines.iter().enumerate().skip(start_idx + 1) {
        let t = l.trim_start();
        if HEADING_LINE_RE.is_match(l) || t.starts_with("来源：") || t.starts_with("来源:") {
            return idx;
        }
    }
    let mut k = start_idx + 1;
    while k < lines.len() {
        let l = lines[k];
        let is_list = strip_list_prefix(l).1;
        let blank_before_list =
            l.trim().is_empty() && k + 1 < lines.len() && strip_list_prefix(lines[k + 1]).1;
        let indented = l.starts_with("  ") && !l.trim().is_empty();
        if is_list || blank_before_list || indented {
            k += 1;
        } else {
            break;
        }
    }
    if k == start_idx + 1 && k < lines.len() && !lines[k].trim().is_empty() {
        k += 1;
    }
    k
}

fn outside_start(body: &str) -> String {
    let clean = body.replace('\r', "");
    let lines: Vec<&str> = clean.split('\n').collect();
    let Some(start_idx) = lines.iter().position(|l| l.trim_start().starts_with("**起点**")) else {
        return body.to_string();
    };
    let end_idx = start_end_idx(&lines, start_idx);
    let mut out = Vec::with_capacity(lines.len());
    out.extend_from_slice(&lines[..start_idx]);
    out.extend_from_slice(&lines[end_idx..]);
    out.join("\n")
}

fn after_start(body: &str) -> String {
    let clean = body.replace('\r', "");
    let lines: Vec<&str> = clean.split('\n').collect();
    let Some(start_idx) = lines.iter().position(|l| l.trim_start().starts_with("**起点**")) else {
        return body.to_string();
    };
    let end_idx = start_end_idx(&lines, start_idx);
    lines[end_idx..].join("\n")
}

fn sections_of(body: &str) -> Vec<String> {
    body.split('\n')
        .filter_map(|l| {
            HEADING_LINE_RE
                .captures(l)
                .and_then(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        })
        .collect()
}

fn section_text(body: &str, name: &str) -> String {
    let clean = body.replace('\r', "");
    let mut in_sec = false;
    let mut out = Vec::new();
    for line in clean.split('\n') {
        if let Some(cap) = HEADING_LINE_RE.captures(line) {
            let h = cap.get(1).map_or("", |m| m.as_str().trim());
            if in_sec {
                break;
            }
            if h == name {
                in_sec = true;
                let tail = &line[cap.get(0).unwrap().end()..];
                if !tail.trim().is_empty() {
                    out.push(tail);
                }
            }
            continue;
        }
        if in_sec {
            out.push(line);
        }
    }
    out.join("\n")
}

fn plain_statement(s: &str) -> String {
    let step1 = LINK_TERM_RE.replace_all(s, "$1");
    let step2 = LINK_ANY_RE.replace_all(&step1, "");
    step2
        .chars()
        .filter(|c| {
            !c.is_whitespace()
                && !matches!(
                    c,
                    '，' | '。' | '；' | '：' | '、' | ',' | '.' | ';' | ':' | '(' | ')' | '（' | '）'
                )
        })
        .collect()
}

fn bigrams(s: &str) -> HashSet<(char, char)> {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}

fn dice(a: &str, b: &str) -> f64 {
    let ga = bigrams(a);
    let gb = bigrams(b);
    let common = ga.iter().filter(|x| gb.contains(x)).count();
    let denom = ga.len() + gb.len();
    if denom == 0 {
        0.0
    } else {
        (2.0 * common as f64) / (denom as f64)
    }
}

fn symbol_of_def(tex: &str) -> String {
    let head = match DEFINES_CUT_RE.find(tex) {
        Some(m) => &tex[..m.start()],
        None => tex,
    };
    head.split_whitespace().collect::<String>()
}

/// Audit a formal model's setting part and notation card (mirrors `check-settings.mjs::audit`).
pub fn audit_formal(
    m: &Value,
    cards: &HashMap<String, SettingCard>,
    parents: &HashMap<String, String>,
) -> Value {
    let setting_addr = m["build"]["setting"].as_str().unwrap_or("");
    let order = m["build"]["order"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let in_part = |a: &str| -> bool {
        if a == setting_addr {
            return true;
        }
        a.strip_prefix(setting_addr)
            .and_then(|r| r.strip_prefix('|'))
            .and_then(|r| r.as_bytes().first())
            .is_some_and(|b| b.is_ascii_lowercase())
    };
    let mains: Vec<&str> = order
        .iter()
        .filter(|o| o["line"].as_str() == Some("main"))
        .filter_map(|o| o["address"].as_str())
        .collect();
    let chapter_of = |a: &str| -> Option<String> {
        let mut matches: Vec<&str> = mains
            .iter()
            .copied()
            .filter(|h| a == *h || a.starts_with(&format!("{h}|")))
            .collect();
        matches.sort_by_key(|h| std::cmp::Reverse(h.len()));
        matches.first().map(|s| (*s).to_string())
    };

    #[derive(Clone)]
    struct Row {
        address: String,
        facet: String,
        title: String,
        body: String,
        line: String,
        has_import: bool,
    }
    let mut all: Vec<Row> = Vec::with_capacity(order.len());
    let mut by_addr: HashMap<String, usize> = HashMap::new();
    for o in order {
        let addr = o["address"].as_str().unwrap_or("").to_string();
        let card = cards.get(&addr);
        let facet = card
            .filter(|c| !c.facet.is_empty())
            .map(|c| c.facet.clone())
            .unwrap_or_else(|| o["facet"].as_str().unwrap_or("").to_string());
        let title = card
            .filter(|c| !c.title.is_empty())
            .map(|c| c.title.clone())
            .unwrap_or_else(|| o["title"].as_str().unwrap_or("").to_string());
        let body = card.map(|c| c.body.clone()).unwrap_or_default();
        let line = o["line"].as_str().unwrap_or("").to_string();
        let has_import = o
            .get("import")
            .is_some_and(|v| !v.is_null() && v.as_bool() != Some(false) && v.as_str() != Some(""));
        by_addr.insert(addr.clone(), all.len());
        all.push(Row {
            address: addr,
            facet,
            title,
            body,
            line,
            has_import,
        });
    }

    let level = m["level"].as_str().unwrap_or("");
    let has_form = m
        .get("form")
        .is_some_and(|v| !v.is_null() && v.as_str() != Some(""));
    let is_formal_v2 = level == "formal" && !has_form;

    let notes: Vec<&Row> = all
        .iter()
        .filter(|c| {
            (in_part(&c.address) || (c.line == "shared" && c.has_import))
                && is_notation(&c.facet, &c.title)
        })
        .collect();
    let note_addrs: HashSet<&str> = notes.iter().map(|c| c.address.as_str()).collect();

    let binds = if is_formal_v2 {
        notes.iter().any(|c| BINDS_RE.is_match(&c.body))
    } else {
        all.iter()
            .any(|c| in_part(&c.address) && BINDS_RE.is_match(&c.body))
    };

    let pending_notation_import = m["build"]["closure"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|f| {
            let Some(c) = f.get("card").filter(|v| v.is_object()) else {
                return false;
            };
            c["type"].as_str() == Some("model")
                && c["models"].as_array().is_some_and(|a| !a.is_empty())
                && crate::post_digest::stage::is_notation_card(c)
        });

    let missing_raw: Vec<Value> = if !notes.is_empty() || binds {
        Vec::new()
    } else {
        vec![json!({
            "fix": format!(
                "the setting part of {setting_addr} has no notation card (facet 符号表与读法): add one at {setting_addr}|a (or import one via **引入**), optionally writing 沿用 [[<lender notation card>]] 的全部符号 inside {setting_addr}|a"
            )
        })]
    };
    let (missing, deferred_missing) = if pending_notation_import {
        (Vec::new(), missing_raw)
    } else {
        (missing_raw, Vec::new())
    };

    let mut borrowed_bodies: Vec<String> = Vec::new();
    let mut wrong_notation_targets: Vec<Value> = Vec::new();
    let borrow_sources: Vec<&Row> = if is_formal_v2 {
        notes.iter().copied().filter(|c| in_part(&c.address)).collect()
    } else {
        all.iter().filter(|c| in_part(&c.address)).collect()
    };
    for c in borrow_sources {
        for line in c.body.split('\n') {
            for clause in line.split(['。', '；', ';']) {
                let Some(pos) = clause.find("沿用") else {
                    continue;
                };
                let tail = &clause[pos..];
                if !tail.contains("符号") {
                    continue;
                }
                for cap in LINK_PLAIN_RE.captures_iter(tail) {
                    let target_addr = cap.get(1).map_or("", |m| m.as_str().trim());
                    let target = cards.get(target_addr);
                    let target_is_canonical_note = target.is_some_and(|t| {
                        crate::formal::role_of(&t.facet) == Some(crate::formal::FormalRole::Notation)
                            && !t.body.trim().is_empty()
                    });
                    let target_is_note = target.is_some_and(|t| {
                        is_notation(&t.facet, &t.title) && !t.body.trim().is_empty()
                    });
                    if is_formal_v2 && !target_is_canonical_note {
                        let desc = target
                            .map(|t| {
                                if !t.facet.is_empty() {
                                    t.facet.as_str()
                                } else if !t.title.is_empty() {
                                    t.title.as_str()
                                } else {
                                    "unknown"
                                }
                            })
                            .unwrap_or("missing");
                        wrong_notation_targets.push(json!({
                            "card": c.address,
                            "what": "notation_target",
                            "fix": format!("{} binds 沿用 [[{target_addr}]] 的全部符号, but {target_addr} ({desc}) is not a non-empty 符号表与读法 card: link the lender's notation card (<lender_head>|a|a) directly", c.address),
                        }));
                        continue;
                    }
                    let Some(t) = target else {
                        wrong_notation_targets.push(json!({
                            "card": c.address,
                            "what": "notation_target",
                            "fix": format!("{} binds 沿用 [[{target_addr}]] 的全部符号, but {target_addr} was not found in the archive", c.address),
                        }));
                        continue;
                    };
                    let has_sym_table = t.body.lines().any(|l| SYM_TABLE_LINE_RE.is_match(l));
                    if (!target_is_note && !has_sym_table) || t.body.trim().is_empty() {
                        let desc = if !t.facet.is_empty() {
                            t.facet.as_str()
                        } else if !t.title.is_empty() {
                            t.title.as_str()
                        } else {
                            "unknown"
                        };
                        wrong_notation_targets.push(json!({
                            "card": c.address,
                            "what": "notation_target",
                            "fix": format!("{} binds 沿用 [[{target_addr}]] 的全部符号, but {target_addr} ({desc}) is not a non-empty 符号表与读法 card: link the lender's notation card (<lender_head>|a|a) directly", c.address),
                        }));
                        continue;
                    }
                    if !t.body.is_empty() && !borrowed_bodies.contains(&t.body) {
                        borrowed_bodies.push(t.body.clone());
                    }
                }
            }
        }
    }

    let mut note_combined_parts: Vec<&str> = notes.iter().map(|c| c.body.as_str()).collect();
    note_combined_parts.extend(borrowed_bodies.iter().map(String::as_str));
    let note_combined = note_combined_parts.join("\n");
    let note_text: String = note_combined.chars().filter(|c| !c.is_whitespace()).collect();

    // Outside
    let placed: Vec<&Row> = all
        .iter()
        .filter(|c| {
            !in_part(&c.address)
                && c.line != "shared"
                && (is_setting(&c.facet, &c.title) || is_notation(&c.facet, &c.title))
        })
        .collect();
    let mut outside = Vec::new();
    for c in &placed {
        if placed
            .iter()
            .any(|p| p.address != c.address && c.address.starts_with(&format!("{}|", p.address)))
        {
            continue;
        }
        let leave: Vec<String> = all
            .iter()
            .filter(|d| {
                d.address.starts_with(&format!("{}|", c.address))
                    && !is_setting(&d.facet, &d.title)
                    && !is_notation(&d.facet, &d.title)
            })
            .map(|d| d.address.clone())
            .collect();
        let ch = chapter_of(&c.address);
        let ch_disp = ch.as_deref().unwrap_or("null");
        let kind_word = if is_notation(&c.facet, &c.title) {
            "notation card"
        } else {
            "setting"
        };
        let mut fix = format!(
            "{} ({}) is a {kind_word} filed in {ch_disp}: move it into the setting part, as a Side successor of {setting_addr} or of the setting it refines",
            c.address, c.facet
        );
        if !leave.is_empty() {
            fix.push_str(&format!(
                "; first file its own derivations {} in {ch_disp} (under the card that uses them), they stay in the chapter",
                leave.join(", ")
            ));
        }
        outside.push(json!({
            "card": c.address,
            "facet": c.facet,
            "chapter": ch,
            "leave": leave,
            "fix": fix,
        }));
    }

    // Inside
    let inside: Vec<Value> = all
        .iter()
        .filter(|c| {
            in_part(&c.address)
                && c.address != setting_addr
                && !is_setting_tree_allowed(&c.facet, &c.title)
                && !is_notation(&c.facet, &c.title)
        })
        .map(|c| {
            json!({
                "card": c.address,
                "facet": c.facet,
                "fix": format!("{} ({}) is not a setting-tree role: move it out of the setting part, after its latest premise (check-network names the chapter)", c.address, c.facet),
            })
        })
        .collect();

    // Stated, uses, local notation
    let mut stated = Vec::new();
    let mut uses = Vec::new();
    let mut notation = Vec::new();
    let first_note_addr = notes.first().map(|c| c.address.as_str());
    for c in &all {
        if c.line == "shared" {
            continue;
        }
        if !in_part(&c.address)
            && !is_setting(&c.facet, &c.title)
            && !is_notation(&c.facet, &c.title)
        {
            for l in c.body.split('\n') {
                let (t, _) = strip_list_prefix(l);
                let t = t.trim();
                if let Some(m) = HEADER_STATED_RE.find(t) {
                    let what = m.as_str();
                    let clip: String = what.chars().take(60).collect();
                    stated.push(json!({
                        "card": c.address,
                        "what": what,
                        "fix": format!("{} states an assumption itself ({clip}): write it in a setting card and let {} rest on it (依赖 [[setting]])", c.address, c.address),
                    }));
                } else if SYM_TABLE_LINE_RE.is_match(t) {
                    stated.push(json!({
                        "card": c.address,
                        "what": "symbol table",
                        "fix": format!("{} states an assumption itself (symbol table): write it in a setting card and let {} rest on it (依赖 [[setting]])", c.address, c.address),
                    }));
                } else if t.starts_with("假设") || t.starts_with("假定") {
                    let clip: String = t.chars().take(60).collect();
                    stated.push(json!({
                        "card": c.address,
                        "what": t,
                        "fix": format!("{} states an assumption itself ({clip}): write it in a setting card and let {} rest on it (依赖 [[setting]])", c.address, c.address),
                    }));
                } else if t.contains("假设") || t.contains("假定") {
                    uses.push(json!({ "card": c.address, "line": t }));
                }
            }
        }
        if is_notation(&c.facet, &c.title) && note_addrs.contains(c.address.as_str()) {
            continue;
        }
        let mut seen = HashSet::new();
        for cap in DEFINES_RE.captures_iter(&c.body) {
            let sym = symbol_of_def(cap.get(1).map_or("", |m| m.as_str()));
            if sym.is_empty() || !seen.insert(sym.clone()) {
                continue;
            }
            let pat = format!(r"\${}(\$|=|\\equiv|,|，)", regex::escape(&sym));
            let found = Regex::new(&pat).is_ok_and(|re| re.is_match(&note_text));
            if !found {
                let suffix = first_note_addr
                    .map(|a| format!(" {a}"))
                    .unwrap_or_default();
                notation.push(json!({
                    "card": c.address,
                    "symbol": sym,
                    "fix": format!("{} introduces ${sym}$: define it in the notation card{suffix}", c.address),
                }));
            }
        }
    }

    // Dependency tree of setting part
    let pos: HashMap<&str, usize> = all
        .iter()
        .enumerate()
        .map(|(i, o)| (o.address.as_str(), i))
        .collect();
    let settings: Vec<&Row> = all
        .iter()
        .filter(|c| {
            in_part(&c.address)
                && (is_setting_member(&c.facet, &c.title) || c.address == setting_addr)
                && !is_notation(&c.facet, &c.title)
        })
        .collect();

    let rests: Vec<(&str, &str)> = m["relations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["kind"].as_str() == Some("rests-on"))
        .filter_map(|r| Some((r["from"].as_str()?, r["to"].as_str()?)))
        .collect();

    let premises_of = |a: &str| -> Vec<String> {
        let mut out = Vec::new();
        for &(from, to) in &rests {
            if from == a && !out.iter().any(|x| x == to) {
                out.push(to.to_string());
            }
        }
        out
    };

    let start: Vec<Value> = settings
        .iter()
        .filter(|c| !c.body.lines().any(|l| l.starts_with("**起点**")))
        .map(|c| {
            json!({
                "card": c.address,
                "fix": format!("{} ({}) has no **起点** section: list the settings it builds on (依赖 [[…]] 的设定：…), or write 原始设定 for a primitive", c.address, c.facet),
            })
        })
        .collect();

    let mut misfiled = Vec::new();
    let mut order_faults = Vec::new();
    for c in &settings {
        let prem = premises_of(&c.address);
        let my_pos = pos.get(c.address.as_str()).copied().unwrap_or(0);
        for p in &prem {
            if !pos.contains_key(p.as_str()) || pos[p.as_str()] > my_pos {
                order_faults.push(json!({
                    "card": c.address,
                    "premise": p,
                    "fix": format!("{} ({}) rests on {p}, which a reader meets later: move the setting after it, or turn the relation (the later card rests on the setting)", c.address, c.facet),
                }));
            }
        }
        if let Some(parent) = parents.get(&c.address)
            && !parent.is_empty()
            && parent != setting_addr
            && in_part(parent)
        {
            let parent_is_note = by_addr
                .get(parent)
                .map(|&idx| is_notation(&all[idx].facet, &all[idx].title))
                .unwrap_or(false);
            if !parent_is_note && !prem.iter().any(|p| p == parent) {
                let mut earlier: Vec<&String> = prem
                    .iter()
                    .filter(|p| in_part(p) && pos.get(p.as_str()).is_some_and(|&pi| pi < my_pos))
                    .collect();
                earlier.sort_by_key(|p| std::cmp::Reverse(pos[p.as_str()]));
                let main_hint = earlier
                    .first()
                    .map(|m| format!("its main premise {m}"))
                    .unwrap_or_else(|| "the setting it builds on".to_string());
                misfiled.push(json!({
                    "card": c.address,
                    "parent": parent,
                    "fix": format!("{} ({}) is filed under {parent} but does not build on it: file it under {main_hint}, or as a top-level Side of {setting_addr}; if it does build on {parent}, say so in its **起点**", c.address, c.facet),
                }));
            }
        }
    }

    let syms = if !notes.is_empty() || !borrowed_bodies.is_empty() {
        notation_symbols(&note_combined)
    } else {
        Vec::new()
    };
    let mut known: HashSet<String> = HashSet::new();
    let mut introduces = Vec::new();
    for c in &settings {
        let fresh: Vec<String> = syms
            .iter()
            .filter(|x| !known.contains(*x) && uses_symbol(&c.body, x, &syms))
            .cloned()
            .collect();
        for x in &fresh {
            known.insert(x.clone());
        }
        if !fresh.is_empty() {
            introduces.push(json!({ "card": c.address, "symbols": fresh }));
        }
    }

    // Placement
    let mut place = Vec::new();
    let head = m["head"]["address"].as_str().unwrap_or("");
    if !head.is_empty() && setting_addr != format!("{head}|a") {
        place.push(json!({
            "card": setting_addr,
            "fix": format!("the setting card {setting_addr} is not the first Side successor of the Model Card: move it to {head}|a, the chapters stay its Direct successors"),
        }));
    }
    if let Some(own) = notes.iter().find(|c| in_part(&c.address))
        && own.address != format!("{setting_addr}|a")
    {
        place.push(json!({
            "card": own.address,
            "fix": format!("the notation card {} is not the first Side successor of the setting card: move it to {setting_addr}|a", own.address),
        }));
    }

    // Symbols: uncovered, unused, first
    let own_card = |c: &Row| c.line != "shared" && !(is_notation(&c.facet, &c.title) && note_addrs.contains(c.address.as_str()));
    let covered: HashSet<String> = syms.iter().flat_map(|x| base_symbols(x)).collect();
    let mut uncovered = Vec::new();
    if is_formal_v2 && let Some(list) = m["build"]["symbol_uncovered"].as_array() {
        for f in list {
            let sym_vec: Vec<Value> = f
                .get("symbol")
                .filter(|v| !v.is_null())
                .cloned()
                .into_iter()
                .collect();
            uncovered.push(json!({
                "card": f["address"],
                "symbols": sym_vec,
                "fix": f["fix"],
            }));
        }
    } else if !notes.is_empty() || !borrowed_bodies.is_empty() {
        for c in &settings {
            let mut lack = Vec::new();
            for f in formulas(&c.body) {
                for s in base_symbols(f) {
                    if !covered.contains(&s) && !lack.contains(&s) {
                        lack.push(s);
                    }
                }
            }
            if !lack.is_empty() {
                let list_s = lack.iter().map(|x| format!("${x}$")).collect::<Vec<_>>().join(" ");
                uncovered.push(json!({
                    "card": c.address,
                    "symbols": lack,
                    "fix": format!("{} uses {list_s}, which the notation card does not list: add a row for each (its meaning and the card that defines it), or write the symbol as the notation card does", c.address),
                }));
            }
        }
    }

    let mut reading: Vec<&Row> = all.iter().filter(|c| in_part(&c.address) && own_card(c)).collect();
    reading.extend(all.iter().filter(|c| !in_part(&c.address) && own_card(c)));

    let rows_all: Vec<NotationRow> = notes
        .iter()
        .filter(|c| in_part(&c.address))
        .flat_map(|c| notation_rows(&c.body))
        .collect();
    let all_syms: Vec<String> = rows_all.iter().flat_map(|r| r.symbols.iter().cloned()).collect();
    let mut unused = Vec::new();
    let mut first = Vec::new();
    for r in &rows_all {
        for sym in &r.symbols {
            let meet = reading.iter().find(|c| uses_symbol(&c.body, sym, &all_syms));
            let Some(meet) = meet else {
                unused.push(json!({
                    "symbol": sym,
                    "fix": format!("the notation card lists ${sym}$, which no card of the model uses: drop the row, or write the symbol as the cards do"),
                }));
                continue;
            };
            if is_formal_v2 && r.link.is_none() {
                first.push(json!({
                    "symbol": sym,
                    "listed": null,
                    "reads": meet.address,
                    "fix": format!("the notation row for ${sym}$ lacks a 首次出现 Card Link in column 3: point it to [[{}]]", meet.address),
                }));
            } else if let Some(link) = &r.link
                && link != &meet.address
            {
                first.push(json!({
                    "symbol": sym,
                    "listed": link,
                    "reads": meet.address,
                    "fix": format!("the notation card says ${sym}$ is first met in {link}, but a reader meets it first in {}: define it there and point the row to it", meet.address),
                }));
            }
        }
    }

    // Format
    let kind_of = |c: &Row| -> &'static str {
        if is_assumption(&c.facet, &c.title) {
            return "假设";
        }
        if is_criterion(&c.facet, &c.title) {
            let src = if !c.facet.is_empty() { &c.facet } else { &c.title };
            if src.starts_with("均衡定义") {
                return "均衡定义";
            }
            if src.starts_with("解概念") {
                return "解概念";
            }
            return "判据";
        }
        if is_definition(&c.facet, &c.title) {
            return "定义";
        }
        "设定"
    };
    let title_ok = |t: &str, kind: &str| -> bool {
        match kind {
            "设定" => t.starts_with("设定：") || t.starts_with("设定:"),
            "假设" => ASSUMP_TITLE_RE.is_match(t),
            "定义" => t.starts_with("定义：") || t.starts_with("定义:"),
            _ => {
                t.starts_with("判据：")
                    || t.starts_with("判据:")
                    || t.starts_with("均衡定义：")
                    || t.starts_with("均衡定义:")
                    || t.starts_with("解概念：")
                    || t.starts_with("解概念:")
            }
        }
    };
    let allowed_tails = HashSet::from([
        "",
        "直觉",
        "说明",
        "直觉,说明",
        "作用域",
        "作用域,说明",
        "作用域,直觉",
        "作用域,直觉,说明",
    ]);
    let criterion_tails = HashSet::from([
        "作用域",
        "作用域,说明",
        "作用域,直觉",
        "作用域,直觉,说明",
    ]);

    let mut format = wrong_notation_targets;
    for c in &settings {
        let kind = kind_of(c);
        let mut bad = |what: &str, fix: String| {
            format.push(json!({
                "card": c.address,
                "what": what,
                "fix": format!("{} ({}): {fix}", c.address, c.facet),
            }));
        };
        if !title_ok(&c.title, kind) {
            let msg = match kind {
                "设定" => "the title opens with 设定：".to_string(),
                "假设" => "the title opens with 假设 <name>： (or 假设 (B)： / 假设：)".to_string(),
                _ => format!("the title opens with {kind}："),
            };
            bad("title", msg);
        }
        let sec = sections_of(&c.body);
        let tail = if sec.len() > 2 {
            sec[2..].join(",")
        } else {
            String::new()
        };
        let is_crit = is_criterion(&c.facet, &c.title);
        let ok_tail = if is_crit {
            criterion_tails.contains(tail.as_str())
        } else {
            allowed_tails.contains(tail.as_str())
        };
        if sec.first().map(String::as_str) != Some("起点")
            || sec.get(1).map(String::as_str) != Some(kind)
            || !ok_tail
        {
            let now_s = if sec.is_empty() {
                "none".to_string()
            } else {
                sec.iter().map(|x| format!("**{x}**")).collect::<Vec<_>>().join(", ")
            };
            let msg = if is_crit {
                format!("a criterion card requires **起点**, **{kind}**, **作用域**, then optional **直觉**/**说明** (now {now_s})")
            } else {
                format!("the sections are **起点**, **{kind}**, then optional **作用域**/**直觉**/**说明** (now {now_s})")
            };
            bad("section", msg);
        }
        if DEPENDS_LINK_RE.is_match(&outside_start(&c.body)) {
            bad(
                "depends",
                "a 依赖 line outside **起点**: move the premise into **起点**".to_string(),
            );
        }
        if SYM_OR_TERM_TABLE_RE.is_match(&c.body) {
            bad(
                "table",
                "a symbol or term table: its rows belong in the notation card".to_string(),
            );
        }
    }

    // Repeat
    let name_of = |c: &Row| -> Option<String> {
        if !is_assumption(&c.facet, &c.title) {
            return None;
        }
        let cap = ASSUMP_NAME_RE.captures(&c.title)?;
        cap.get(1)
            .or_else(|| cap.get(2))
            .or_else(|| cap.get(3))
            .map(|m| m.as_str().to_string())
    };
    let statements_of = |c: &Row| -> Vec<String> {
        let sec = section_text(&c.body, kind_of(c));
        let no_disp = DISPLAY_RE.replace_all(&sec, "");
        let mut out = Vec::new();
        for l in no_disp.split('\n') {
            if l.trim_start().starts_with("$$") {
                continue;
            }
            let (stripped, _) = strip_list_prefix(l);
            for sent in stripped.split('。') {
                let p = plain_statement(sent);
                if p.chars().count() >= 12 {
                    out.push(p);
                }
            }
        }
        out
    };
    let mut repeat = Vec::new();
    let mut said: Vec<(&Row, String)> = Vec::new();
    for c in &settings {
        let mine = statements_of(c);
        let my_name = name_of(c);
        let hit = said.iter().find(|(other, s)| {
            let apart = my_name.is_some()
                && name_of(other).is_some()
                && my_name != name_of(other);
            !apart && mine.iter().any(|x| dice(x, s) >= 0.8)
        });
        if let Some((other, _)) = hit {
            repeat.push(json!({
                "card": c.address,
                "other": other.address,
                "fix": format!("{} restates what {} states: keep the statement in {}; {} names it in **起点** and states only what it changes", c.address, other.address, other.address, c.address),
            }));
        }
        for s in mine {
            said.push((c, s));
        }
    }

    // Cite
    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    for c in settings.iter().filter(|c| is_assumption(&c.facet, &c.title)) {
        if let Some(n) = name_of(c) {
            named.entry(n).or_default().push(c.address.clone());
        }
    }
    let mut cite = Vec::new();
    for c in all.iter().filter(|c| own_card(c)) {
        let mut seen = HashSet::new();
        for line in c.body.split('\n') {
            let bare = LINK_ANY_RE.replace_all(line, "");
            for (_, key) in find_assumption_citations(&bare) {
                let Some(addrs) = named.get(&key) else {
                    continue;
                };
                if addrs.contains(&c.address) || seen.contains(&key) {
                    continue;
                }
                if addrs
                    .iter()
                    .any(|a| line.contains(&format!("@{a}]]")) || line.contains(&format!("[[{a}]]")))
                {
                    continue;
                }
                seen.insert(key.clone());
                cite.push(json!({
                    "card": c.address,
                    "name": format!("假设 {key}"),
                    "line": line.trim(),
                    "fix": format!("{} names 假设 {key} without a Link: write [[{}]] in its place (the GUI shows a Link to a model part by its facet, 假设 {key})", c.address, addrs[0]),
                }));
            }
        }
    }

    // Balance (advice)
    let mut balance = Vec::new();
    for c in &settings {
        let sec = section_text(&c.body, kind_of(c));
        let text = if !sec.is_empty() {
            sec
        } else {
            after_start(&c.body)
                .split('\n')
                .filter(|l| !l.starts_with('|'))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let items = text
            .split('\n')
            .filter(|l| {
                let (rest, is_list) = strip_list_prefix(l);
                is_list && !rest.trim().is_empty()
            })
            .count();
        let count = if items > 0 {
            items
        } else {
            text.split('\n')
                .filter(|l| !l.trim_start().starts_with("$$"))
                .collect::<Vec<_>>()
                .join("")
                .split('。')
                .filter(|s| !s.trim().is_empty())
                .count()
        };
        if count > 10 {
            balance.push(json!({
                "card": c.address,
                "what": "heavy",
                "fix": format!("{} states {count} things: split it into settings that each set up one piece", c.address),
            }));
        } else if c.address != setting_addr
            && !is_assumption(&c.facet, &c.title)
            && !is_definition(&c.facet, &c.title)
            && !is_criterion(&c.facet, &c.title)
            && count <= 1
            && !c.body.contains("$$")
        {
            balance.push(json!({
                "card": c.address,
                "what": "light",
                "fix": format!("{} states one thing: consider joining it to the setting it builds on", c.address),
            }));
        }
        let kids = settings
            .iter()
            .filter(|d| parents.get(&d.address).map(String::as_str) == Some(c.address.as_str()))
            .count();
        if kids > 7 {
            balance.push(json!({
                "card": c.address,
                "what": "wide",
                "fix": format!("{} has {kids} settings filed under it: group them in blocks", c.address),
            }));
        }
    }

    let ok = missing.is_empty()
        && outside.is_empty()
        && inside.is_empty()
        && stated.is_empty()
        && notation.is_empty()
        && start.is_empty()
        && misfiled.is_empty()
        && order_faults.is_empty()
        && place.is_empty()
        && uncovered.is_empty()
        && unused.is_empty()
        && first.is_empty()
        && format.is_empty()
        && repeat.is_empty()
        && cite.is_empty();

    json!({
        "setting": setting_addr,
        "notation_card": first_note_addr,
        "outside": outside,
        "inside": inside,
        "stated": stated,
        "notation": notation,
        "missing": missing,
        "deferred_missing": deferred_missing,
        "start": start,
        "misfiled": misfiled,
        "order": order_faults,
        "place": place,
        "uncovered": uncovered,
        "unused": unused,
        "first": first,
        "format": format,
        "repeat": repeat,
        "cite": cite,
        "balance": balance,
        "uses": uses,
        "introduces": introduces,
        "ok": ok,
    })
}

pub fn variable_symbols(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.split('\n') {
        let Some(cap) = FIRST_CELL_RE.captures(line) else {
            continue;
        };
        let cell = cap.get(1).map_or("", |m| m.as_str()).trim();
        if cell == "变量" || cell.contains("---") {
            continue;
        }
        for icap in INLINE_DOLLAR_RE.captures_iter(cell) {
            let tex = icap.get(1).map_or("", |m| m.as_str());
            for s in base_symbols(tex) {
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        }
    }
    out
}

fn parent_of_addr(a: &str) -> &str {
    a.rsplit_once('|').map_or(a, |(p, _)| p)
}

fn ends_with_direct_num(a: &str) -> bool {
    a.rsplit_once('|')
        .is_some_and(|(_, tail)| !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()))
}

/// Audit an empirical model's data tree and specification ladder (mirrors `check-settings.mjs::auditEmpirical`).
pub fn audit_empirical(m: &Value, cards: &HashMap<String, SettingCard>) -> Value {
    let order: Vec<&Value> = m["build"]["order"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|o| o["line"].as_str() != Some("shared"))
        .collect();
    let head = m["head"]["address"].as_str().unwrap_or("");
    let body = |a: &str| -> &str { cards.get(a).map_or("", |c| c.body.as_str()) };
    fn role_of(o: &Value) -> &str {
        o["role"].as_str().unwrap_or("")
    }

    let data = order
        .iter()
        .copied()
        .find(|o| role_of(o) == "data" && o["line"].as_str() == Some("main"));
    let data_addr = data.and_then(|d| d["address"].as_str());
    let mut place = Vec::new();
    if data_addr != Some(&format!("{head}|a")) {
        place.push(json!({
            "card": data_addr.unwrap_or(head),
            "fix": format!("the data card (数据) is the first Side successor of {head} ({head}|a)"),
        }));
    }
    let table = order.iter().copied().find(|o| role_of(o) == "variables");
    let table_addr = table.and_then(|t| t["address"].as_str());
    if let Some(da) = data_addr
        && table_addr != Some(&format!("{da}|a"))
    {
        place.push(json!({
            "card": table_addr.unwrap_or(da),
            "fix": format!("the 变量表 is the first Side successor of the data card {da} ({da}|a)"),
        }));
    }

    let uncovered: Vec<Value> = m["build"]["variable_uncovered"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|f| json!({ "card": f["address"], "symbol": f["symbol"], "fix": f["fix"] }))
        .collect();

    let mut used: HashSet<String> = HashSet::new();
    let mut prose: HashSet<&'static str> = HashSet::new();
    for o in &order {
        let addr = o["address"].as_str().unwrap_or("");
        if Some(addr) == table_addr {
            continue;
        }
        let b = body(addr);
        for tex in formulas(b) {
            for s in base_symbols(tex) {
                used.insert(s);
            }
        }
        for ch in b.chars() {
            if let Some(&(_, name)) = PROSE_GREEK.iter().find(|&&(g, _)| g == ch) {
                prose.insert(name);
            }
        }
    }
    let in_prose = |s: &str| -> bool {
        TEX_CMD_NAME_RE
            .captures_iter(s)
            .any(|c| c.get(1).is_some_and(|m| prose.contains(m.as_str())))
    };
    let unused: Vec<Value> = match table_addr {
        Some(ta) => variable_symbols(body(ta))
            .into_iter()
            .filter(|s| !used.contains(s) && !in_prose(s))
            .map(|s| {
                json!({
                    "card": ta,
                    "symbol": s,
                    "fix": format!("the 变量表 row ${s}$ names a symbol no card of the model uses: drop the row"),
                })
            })
            .collect(),
        None => Vec::new(),
    };

    let chapters: Vec<&str> = order
        .iter()
        .filter(|o| role_of(o) == "chapter" && o["line"].as_str() == Some("main"))
        .filter_map(|o| o["address"].as_str())
        .collect();
    let specs: Vec<&Value> = order.iter().copied().filter(|o| role_of(o) == "spec").collect();

    let variant_change: Vec<Value> = specs
        .iter()
        .filter(|o| {
            let a = o["address"].as_str().unwrap_or("");
            o["line"].as_str() != Some("main")
                && !body(a).lines().any(|l| l.trim_start().starts_with("**相对基准的改动**"))
        })
        .map(|o| {
            let a = o["address"].as_str().unwrap_or("");
            json!({
                "card": a,
                "fix": format!("{a} is a specification variant without **相对基准的改动**: state the one change against the specification it is filed under"),
            })
        })
        .collect();

    let spec_in_chapter: Vec<Value> = specs
        .iter()
        .filter(|o| {
            let a = o["address"].as_str().unwrap_or("");
            chapters.iter().any(|c| a.starts_with(&format!("{c}|")))
        })
        .map(|o| {
            let a = o["address"].as_str().unwrap_or("");
            json!({
                "card": a,
                "fix": format!("{a} is a specification filed in a chapter: move it onto the ladder, under the specification it changes (a chapter holds the estimates of a variant, never the variant)"),
            })
        })
        .collect();

    let mut equation = Vec::new();
    for o in &specs {
        let a = o["address"].as_str().unwrap_or("");
        let text = body(a);
        let eq_sec = crate::empirical::section(text, "估计式");
        let has_eq = eq_sec.as_deref().is_some_and(|sec| {
            let mut rest = sec;
            while let Some(open) = rest.find("$$") {
                let after_open = &rest[open + 2..];
                let Some(close) = after_open.find("$$") else {
                    break;
                };
                if !after_open[..close].trim().is_empty() {
                    return true;
                }
                rest = &after_open[close + 2..];
            }
            false
        });
        if has_eq {
            continue;
        }
        let sec_says_missing = eq_sec.as_deref().is_some_and(|s| {
            s.contains("书中没有写出估计式") || s.contains("书中未写出估计式") || s.contains("书中未写")
        });
        if sec_says_missing || text.contains("书中没有写出估计式") {
            equation.push(json!({
                "card": a,
                "fix": format!("{a} says the book writes no equation: compose it in **估计式** and open with 原书没有写出本规格的估计式。本卡按……补写："),
            }));
        } else {
            equation.push(json!({
                "card": a,
                "fix": format!("{a} is a specification without a display equation in **估计式**: write it (from the source, or as 补写 opening with 原书没有写出本规格的估计式。本卡按……补写：)"),
            }));
        }
    }

    let design_addr = order
        .iter()
        .find(|o| role_of(o) == "design" && o["line"].as_str() == Some("main"))
        .and_then(|o| o["address"].as_str());
    let assumption: Vec<Value> = order
        .iter()
        .filter(|o| {
            if role_of(o) != "assumption" {
                return false;
            }
            let a = o["address"].as_str().unwrap_or("");
            design_addr.is_none_or(|d| parent_of_addr(a) != d || ends_with_direct_num(a))
        })
        .map(|o| {
            let a = o["address"].as_str().unwrap_or("");
            let suffix = design_addr.map(|d| format!(" {d}")).unwrap_or_default();
            json!({
                "card": a,
                "fix": format!("{a} is an identifying assumption that is not a Side successor of the design card{suffix}: file it as the design's next Side successor"),
            })
        })
        .collect();

    let data_prefix = data_addr.map(|d| format!("{d}|"));
    let tree: Vec<&Value> = order
        .iter()
        .copied()
        .filter(|o| {
            matches!(role_of(o), "sample" | "measure" | "descriptive")
                && data_prefix
                    .as_deref()
                    .is_some_and(|dp| o["address"].as_str().unwrap_or("").starts_with(dp))
        })
        .collect();
    let start: Vec<Value> = tree
        .iter()
        .filter(|o| {
            let a = o["address"].as_str().unwrap_or("");
            !body(a).lines().any(|l| l.trim_start().starts_with("**起点**"))
        })
        .map(|o| {
            let a = o["address"].as_str().unwrap_or("");
            json!({
                "card": a,
                "fix": format!("{a} has no **起点**: after its one-line summary, list what it builds on (依赖 [[…]]：…) under **起点**"),
            })
        })
        .collect();

    let at: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .filter_map(|(i, o)| o["address"].as_str().map(|a| (a, i)))
        .collect();
    let mut data_tree: HashSet<&str> = tree
        .iter()
        .filter_map(|o| o["address"].as_str())
        .collect();
    if let Some(da) = data_addr {
        data_tree.insert(da);
    }
    let mut order_faults = Vec::new();
    for o in &tree {
        let a = o["address"].as_str().unwrap_or("");
        let my_i = at.get(a).copied().unwrap_or(0);
        for cap in DEPENDS_TARGET_RE.captures_iter(body(a)) {
            let p = cap.get(1).map_or("", |m| m.as_str());
            if data_tree.contains(p) && at.get(p).is_some_and(|&pi| pi > my_i) {
                order_faults.push(json!({
                    "card": a,
                    "premise": p,
                    "fix": format!("{a} rests on {p}, which a reader meets later: reorder the data card's Side branches so {p} comes first"),
                }));
            }
        }
    }

    let is_stage3_shape = |f: &Value| -> bool { crate::post_digest::stage::is_stage3_empirical_shape(f) };
    let mut roles = Vec::new();
    let mut deferred_roles = Vec::new();
    for f in m["build"]["empirical_shape"].as_array().into_iter().flatten() {
        let entry = json!({ "card": f["address"], "kind": "empirical_shape", "fix": f["fix"] });
        if is_stage3_shape(f) {
            deferred_roles.push(entry);
        } else {
            roles.push(entry);
        }
    }
    for f in m["build"]["spec_unbound"].as_array().into_iter().flatten() {
        roles.push(json!({ "card": f["address"], "kind": "spec_unbound", "fix": f["fix"] }));
    }
    for k in ["estimate_unbound", "test_unbound"] {
        for f in m["build"][k].as_array().into_iter().flatten() {
            deferred_roles.push(json!({ "card": f["address"], "kind": k, "fix": f["fix"] }));
        }
    }

    let ok = place.is_empty()
        && uncovered.is_empty()
        && unused.is_empty()
        && variant_change.is_empty()
        && spec_in_chapter.is_empty()
        && equation.is_empty()
        && assumption.is_empty()
        && start.is_empty()
        && order_faults.is_empty()
        && roles.is_empty();

    json!({
        "level": "empirical",
        "data": data_addr,
        "variables": table_addr,
        "place": place,
        "uncovered": uncovered,
        "unused": unused,
        "variant_change": variant_change,
        "spec_in_chapter": spec_in_chapter,
        "equation": equation,
        "assumption": assumption,
        "start": start,
        "order": order_faults,
        "roles": roles,
        "deferred_roles": deferred_roles,
        "ok": ok,
    })
}

fn format_text(head: &str, r: &Value, is_empirical: bool, show_symbols: bool) -> Vec<String> {
    let mut out = Vec::new();
    if is_empirical {
        out.push(format!(
            "data {}  variables {}",
            r["data"].as_str().unwrap_or("none"),
            r["variables"].as_str().unwrap_or("none")
        ));
        for k in [
            "place",
            "uncovered",
            "unused",
            "variant_change",
            "spec_in_chapter",
            "equation",
            "assumption",
            "start",
            "order",
            "roles",
        ] {
            for f in r[k].as_array().into_iter().flatten() {
                out.push(format!("{k}: {}", f["fix"].as_str().unwrap_or("")));
            }
        }
        for f in r["deferred_roles"].as_array().into_iter().flatten() {
            out.push(format!(
                "[deferred -> Stage 3] {}: {}",
                f["kind"].as_str().unwrap_or("role"),
                f["fix"].as_str().unwrap_or("")
            ));
        }
        out.push(if r["ok"].as_bool() == Some(true) {
            "settings: data tree and specification ladder are clean".to_string()
        } else {
            "settings: faults above".to_string()
        });
        return out;
    }

    out.push(format!(
        "setting {}  notation {}",
        r["setting"].as_str().unwrap_or("none"),
        r["notation_card"].as_str().unwrap_or("none")
    ));
    for k in [
        "missing",
        "place",
        "outside",
        "inside",
        "stated",
        "notation",
        "uncovered",
        "unused",
        "first",
        "start",
        "misfiled",
        "order",
        "format",
        "repeat",
        "cite",
    ] {
        for f in r[k].as_array().into_iter().flatten() {
            out.push(format!("{k}: {}", f["fix"].as_str().unwrap_or("")));
        }
    }
    for f in r["deferred_missing"].as_array().into_iter().flatten() {
        out.push(format!(
            "[deferred -> Stage 4] missing: {}",
            f["fix"].as_str().unwrap_or("")
        ));
    }
    for b in r["balance"].as_array().into_iter().flatten() {
        out.push(format!("balance: {}", b["fix"].as_str().unwrap_or("")));
    }
    let uses_len = r["uses"].as_array().map_or(0, Vec::len);
    if uses_len > 0 {
        out.push(format!(
            "uses (advice, {uses_len} mid-sentence mention(s) of 假设/假定; pass --format json to inspect)"
        ));
    }
    if show_symbols {
        let sh = |a: &str| -> String {
            a.strip_prefix(head)
                .map(|rest| format!("H{rest}"))
                .unwrap_or_else(|| a.to_string())
        };
        for item in r["introduces"].as_array().into_iter().flatten() {
            let card = item["card"].as_str().unwrap_or("");
            let syms = item["symbols"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|s| format!("${s}$"))
                .collect::<Vec<_>>()
                .join(" ");
            out.push(format!("reads: {} introduces {syms}", sh(card)));
        }
    }
    out.push(if r["ok"].as_bool() == Some(true) {
        "settings: all in the setting part".to_string()
    } else {
        "settings: faults above".to_string()
    });
    out
}

/// Validates the live `get_model` payload consumed by `--settings` (M7):
/// requires non-empty string `address` on every `build.order` row, consistent `level`/`form`
/// fields, and required fault arrays for empirical/formal models. Returns the validated order
/// addresses.
pub fn validate_settings_payload(model: &Value, head: &str) -> Result<Vec<String>> {
    let Some(build) = model.get("build").and_then(Value::as_object) else {
        bail!("get_model answer for {head} lacks `build` object");
    };
    let Some(order_arr) = build.get("order").and_then(Value::as_array) else {
        bail!("get_model answer for {head} lacks `build.order` array");
    };
    let mut order_addrs = Vec::with_capacity(order_arr.len());
    for (idx, row) in order_arr.iter().enumerate() {
        let Some(addr) = row
            .as_object()
            .and_then(|o| o.get("address"))
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            bail!("get_model answer for {head} build.order[{idx}] lacks non-empty string `address`");
        };
        order_addrs.push(addr.to_string());
    }
    let (level, has_form) = crate::post_digest::stage::validate_level_and_form(model, build, head)?;
    if level == "empirical" {
        for key in [
            "empirical_shape",
            "spec_unbound",
            "estimate_unbound",
            "test_unbound",
            "variable_uncovered",
        ] {
            if !build.get(key).is_some_and(Value::is_array) {
                bail!("get_model answer for {head} lacks required empirical `build.{key}` array");
            }
        }
    } else if level == "formal" && !has_form {
        for key in ["closure", "symbol_uncovered"] {
            if !build.get(key).is_some_and(Value::is_array) {
                bail!("get_model answer for {head} lacks required formal `build.{key}` array");
            }
        }
        let Some(relations) = model.get("relations").and_then(Value::as_array) else {
            bail!("get_model answer for {head} lacks required `relations` array");
        };
        crate::post_digest::premises::validate_relations(relations, head)?;
    } else if let Some(relations) = model.get("relations") {
        let Some(arr) = relations.as_array() else {
            bail!("get_model answer for {head} has non-array `relations`");
        };
        crate::post_digest::premises::validate_relations(arr, head)?;
    }
    Ok(order_addrs)
}

pub fn run_live(
    zt: &mut Zt,
    head: &str,
    show_symbols: bool,
    json_out: bool,
) -> Result<(Vec<String>, u8, usize)> {
    let rev_before = zt.cards(&[], "text")?.archive_revision;
    let model = zt.call("get_model", json!({ "at": head }))?;
    let got_head = model["head"]["address"].as_str().unwrap_or("");
    if got_head != head {
        if got_head.is_empty() {
            bail!("{head} is not a Model Card");
        }
        bail!("{head} is not a Model Card: it belongs to the model {got_head}; give that address");
    }
    let order_addrs = validate_settings_payload(&model, head)?;

    let fetched = zt.cards(&order_addrs, "text")?;
    if fetched.archive_revision != rev_before {
        bail!("incomplete checker inputs: changing read view");
    }
    let mut cards: HashMap<String, SettingCard> = HashMap::new();
    let mut extra_targets: Vec<String> = Vec::new();
    let mut seen_extra: HashSet<String> = HashSet::new();

    for c in fetched.cards {
        let addr = c["address"].as_str().unwrap_or("").to_string();
        if addr.is_empty() {
            continue;
        }
        let body = c["body"].as_str().unwrap_or("").to_string();
        for line in body.split('\n') {
            for clause in line.split(['。', '；', ';']) {
                if let Some(pos) = clause.find("沿用")
                    && clause[pos..].contains("符号")
                {
                    for cap in LINK_PLAIN_RE.captures_iter(&clause[pos..]) {
                        let t = cap.get(1).map_or("", |m| m.as_str().trim());
                        if !t.is_empty()
                            && !order_addrs.iter().any(|a| a == t)
                            && seen_extra.insert(t.to_string())
                        {
                            extra_targets.push(t.to_string());
                        }
                    }
                }
            }
        }
        cards.insert(
            addr,
            SettingCard {
                facet: c["facet"].as_str().unwrap_or("").to_string(),
                title: c["title"].as_str().unwrap_or("").to_string(),
                body,
            },
        );
    }
    if !extra_targets.is_empty() {
        let extra = zt.cards_optional(&extra_targets, "text")?;
        if extra.archive_revision != rev_before {
            bail!("incomplete checker inputs: changing read view");
        }
        for (req, c) in extra.resolved {
            let addr = c["address"].as_str().unwrap_or("").to_string();
            let card = SettingCard {
                facet: c["facet"].as_str().unwrap_or("").to_string(),
                title: c["title"].as_str().unwrap_or("").to_string(),
                body: c["body"].as_str().unwrap_or("").to_string(),
            };
            if !addr.is_empty() {
                cards.insert(addr, card.clone());
            }
            // Retain requested handle/alias lookup alongside canonical address (M6).
            cards.entry(req).or_insert(card);
        }
    }

    let is_empirical = model["level"].as_str() == Some("empirical");
    let report = if is_empirical {
        audit_empirical(&model, &cards)
    } else {
        let key = head.split('/').next().unwrap_or(head);
        let tree_reply = zt.call("get_tree", json!({ "tree": key }))?;
        let rev = tree_reply["archive_revision"]
            .as_u64()
            .context("get_tree answer lacks `archive_revision`")?;
        if rev != rev_before {
            bail!("incomplete checker inputs: changing read view");
        }
        let tree_cards = tree_reply["cards"]
            .as_array()
            .context("get_tree answer lacks `cards` array")?;
        let mut parents: HashMap<String, String> = HashMap::new();
        for (idx, c) in tree_cards.iter().enumerate() {
            let Some(a) = c["address"].as_str().filter(|s| !s.is_empty()) else {
                bail!("get_tree answer for `{key}` cards[{idx}] lacks non-empty string `address`");
            };
            if let Some(p) = c["parent"].as_str() {
                parents.insert(a.to_string(), p.to_string());
            }
        }
        audit_formal(&model, &cards, &parents)
    };
    if zt.cards(&[], "text")?.archive_revision != rev_before {
        bail!("incomplete checker inputs: changing read view");
    }
    let fault_count: usize = if is_empirical {
        [
            "place",
            "uncovered",
            "unused",
            "variant_change",
            "spec_in_chapter",
            "equation",
            "assumption",
            "start",
            "order",
            "roles",
        ]
        .iter()
        .map(|k| report[*k].as_array().map_or(0, Vec::len))
        .sum()
    } else {
        [
            "missing",
            "place",
            "outside",
            "inside",
            "stated",
            "notation",
            "uncovered",
            "unused",
            "first",
            "start",
            "misfiled",
            "order",
            "format",
            "repeat",
            "cite",
        ]
        .iter()
        .map(|k| report[*k].as_array().map_or(0, Vec::len))
        .sum()
    };
    let ok = report["ok"].as_bool() == Some(true);
    let exit = if ok { 0 } else { 1 };
    let lines = if json_out {
        vec![serde_json::to_string_pretty(&report)?]
    } else {
        format_text(head, &report, is_empirical, show_symbols)
    };
    Ok((lines, exit, fault_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn card(facet: &str, title: &str, body: &str) -> SettingCard {
        SettingCard {
            facet: facet.into(),
            title: title.into(),
            body: body.into(),
        }
    }

    #[test]
    fn notation_symbols_and_base_symbols_handle_tex_arrays_and_decorations() {
        assert_eq!(
            notation_symbols("| 符号 | 含义 |\n| --- | --- |\n| $L,\\ Z=(Z_1,\\dots,Z_N)$ | 要素 |\n| $\\lambda(i,\\omega)$、$\\theta\\in\\Theta$ | x |"),
            vec!["L", "Z", "\\lambda(i,\\omega)", "\\theta"]
        );
        assert_eq!(
            base_symbols(r"y = \left\{ \begin{array}{l l} x & \text {if} I = 1 \\ w & \text {if} I = 0 \end{array} \right."),
            vec!["y", "x", "I", "w"]
        );
        assert_eq!(
            base_symbols(r"\left( \begin{array}{c c c} a & b \\ c & d \end{array} \right)"),
            vec!["a", "b", "c", "d"]
        );
    }

    #[test]
    fn formal_settings_audit_catches_placement_symbols_format_repeat_and_cite_and_passes_clean() {
        let h = "K/2|a";
        let p = "K/2|a|a";
        let note = format!(
            "| 符号 | 含义 | 首次出现 |\n| --- | --- | --- |\n| $L$ | 工人数 | [[{p}]] |\n| $\\bar Z$ | 资本总量 | [[{p}|b]] |\n| $\\theta,\\Theta$ | 技术 | [[{p}|b]] |"
        );
        let order = json!([
            { "address": p, "facet": "设定", "title": "设定：工人与资本", "line": "main" },
            { "address": format!("{p}|a"), "facet": "符号表与读法", "title": "符号表与读法", "line": "side" },
            { "address": format!("{p}|b"), "facet": "设定：技术", "title": "设定：技术与资本", "line": "side" },
            { "address": format!("{p}|c"), "facet": "假设 1", "title": "假设 1：凹性", "line": "side" },
            { "address": format!("{p}|1"), "facet": "第 1 章：直觉", "title": "第 1 章：直觉", "line": "main" },
        ]);
        let relations = json!([
            { "from": format!("{p}|b"), "to": p, "kind": "rests-on" },
            { "from": format!("{p}|c"), "to": format!("{p}|b"), "kind": "rests-on" },
        ]);
        let m = json!({
            "head": { "address": h },
            "level": "formal",
            "build": { "setting": p, "order": order },
            "relations": relations,
        });
        let mut cards = HashMap::from([
            (p.into(), card("设定", "设定：工人与资本", "**起点**\n- 原始设定：x。\n\n**设定**\n1. 有 $L$ 个工人。")),
            (format!("{p}|a"), card("符号表与读法", "符号表与读法", &note)),
            (format!("{p}|b"), card("设定：技术", "设定：技术与资本", &format!("**起点**\n- 依赖 [[{p}]] 的设定：工人。\n\n**设定**\n1. 资本总量为 $\\bar Z$。\n2. 技术 $\\theta\\in\\Theta$。\n\n**说明**\n技术由企业选。"))),
            (format!("{p}|c"), card("假设 1", "假设 1：凹性", &format!("**起点**\n- 依赖 [[{p}|b]] 的设定：技术。\n\n**假设**\n1. $L$ 与 $\\theta$ 上严格凹。"))),
            (format!("{p}|1"), card("第 1 章：直觉", "第 1 章：直觉", &format!("在[[假设 1@{p}|c]]下，解唯一。"))),
        ]);
        let parents = HashMap::from([
            (p.into(), h.into()),
            (format!("{p}|a"), p.into()),
            (format!("{p}|b"), p.into()),
            (format!("{p}|c"), p.into()),
            (format!("{p}|1"), p.into()),
        ]);

        let clean = audit_formal(&m, &cards, &parents);
        assert_eq!(clean["ok"], true, "clean strict model should pass: {clean}");

        cards.get_mut(&format!("{p}|1")).unwrap().body = "在假设 1 下，解唯一。".into();
        cards.get_mut(&format!("{p}|b")).unwrap().body.push_str("\n3. 产出 $Y = A \\bar Z^{\\alpha}$。");
        let dirty = audit_formal(&m, &cards, &parents);
        assert_eq!(dirty["ok"], false);
        assert_eq!(dirty["cite"].as_array().unwrap().len(), 1);
        assert_eq!(dirty["uncovered"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn empirical_settings_audit_checks_place_equation_spec_ladder_data_tree_and_defers_stage3() {
        let h = "K/5|c";
        let o = |addr: String, role: &str, line: &str| json!({ "address": addr, "role": role, "line": line, "facet": role, "title": role });
        let m = json!({
            "level": "empirical",
            "head": { "address": h },
            "build": {
                "order": [
                    o(format!("{h}|a"), "data", "main"),
                    o(format!("{h}|a|a"), "variables", "side"),
                    o(format!("{h}|a|1"), "design", "main"),
                    o(format!("{h}|a|1|a"), "assumption", "side"),
                    o(format!("{h}|a|2"), "spec", "main"),
                    o(format!("{h}|a|2|a"), "spec", "side"),
                    o(format!("{h}|a|3"), "chapter", "main"),
                    o(format!("{h}|a|3|a"), "estimate", "side"),
                    o(format!("{h}|a|3|b"), "spec", "side"),
                    o(format!("{h}|a|4"), "assumption", "side"),
                ],
                "variable_uncovered": [{ "address": format!("{h}|a|2"), "symbol": "Z", "fix": "Z has no row" }],
                "empirical_shape": [{ "address": h, "lacks": "估计", "fix": "zero 估计" }],
                "spec_unbound": [],
                "estimate_unbound": [{ "address": format!("{h}|a|3|a"), "fix": "stage 3" }],
                "test_unbound": [],
            }
        });
        let cards = HashMap::from([
            (format!("{h}|a|a"), card("变量表", "变量表", "| 变量 | 含义 |\n| --- | --- |\n| $y_{it}$ | 结果 |\n| $\\beta$ | 系数 |\n| $W$ | 未用 |")),
            (format!("{h}|a|2"), card("规格", "规格", "**估计式**\n$$ y_{it} = \\beta X_{it} + Z $$")),
            (format!("{h}|a|2|a"), card("规格", "规格", "**相对基准的改动**\n加入控制变量。\n\n**估计式**\n$$y_{it}=\\beta X_{it}$$")),
            (format!("{h}|a|3|b"), card("规格", "规格", "**估计式**\n书中没有写出估计式。")),
        ]);
        let r = audit_empirical(&m, &cards);
        assert_eq!(r["place"].as_array().unwrap().len(), 0);
        assert_eq!(r["uncovered"].as_array().unwrap().len(), 1);
        assert_eq!(r["unused"].as_array().unwrap().len(), 1);
        assert_eq!(r["variant_change"].as_array().unwrap().len(), 1);
        assert_eq!(r["spec_in_chapter"].as_array().unwrap().len(), 1);
        assert_eq!(r["equation"].as_array().unwrap().len(), 1);
        assert_eq!(r["assumption"].as_array().unwrap().len(), 1);
        assert_eq!(r["deferred_roles"].as_array().unwrap().len(), 2);
        assert_eq!(r["ok"], false);
    }

    #[test]
    fn trailing_backslash_never_panics_and_empirical_equation_uses_core_em3_rules() {
        // Regression for Astra audit: trailing literal `\` in TeX must not panic.
        assert_eq!(compact_tex_symbol(r"\bar\"), r"\bar\");
        assert_eq!(compact_tex_symbol(r"\"), r"\");
        assert_eq!(base_symbols(r"x + \"), vec!["x"]);

        // Empirical **估计式** accepts display math on the same line as the heading,
        // and rejects inline-only `$...$` math (matching EM3).
        let h = "K/6|a";
        let o = |addr: String, role: &str, line: &str| json!({ "address": addr, "role": role, "line": line, "facet": role, "title": role });
        let m = json!({
            "level": "empirical",
            "head": { "address": h },
            "build": {
                "order": [
                    o(format!("{h}|a"), "data", "main"),
                    o(format!("{h}|a|a"), "variables", "side"),
                    o(format!("{h}|a|1"), "design", "main"),
                    o(format!("{h}|a|2"), "spec", "main"),
                    o(format!("{h}|a|2|a"), "spec", "side"),
                ],
                "variable_uncovered": [],
                "empirical_shape": [],
                "spec_unbound": [],
                "estimate_unbound": [],
                "test_unbound": [],
            }
        });
        let cards = HashMap::from([
            (format!("{h}|a|a"), card("变量表", "变量表", "| 变量 | 含义 |\n| --- | --- |\n| $y$ | 结果 |\n| $x$ | 自变量 |")),
            (format!("{h}|a|2"), card("规格", "规格", "**估计式** $$y = x$$")),
            (format!("{h}|a|2|a"), card("规格", "规格", "**相对基准的改动**\n仅内联。\n\n**估计式**\n$y = x$")),
        ]);
        let r = audit_empirical(&m, &cards);
        let eq_cards: Vec<&str> = r["equation"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["card"].as_str())
            .collect();
        assert_eq!(eq_cards, vec![format!("{h}|a|2|a")]);

        // P1-3: empty `$$$$` and unclosed lone `$$` in **估计式** must NOT count as a valid equation.
        let mut cards_bad_eq = cards.clone();
        cards_bad_eq.insert(format!("{h}|a|2"), card("规格", "规格", "**估计式**\n$$$$"));
        cards_bad_eq.insert(format!("{h}|a|2|a"), card("规格", "规格", "**相对基准的改动**\n改。\n\n**估计式**\n$$ y = x"));
        let r_bad = audit_empirical(&m, &cards_bad_eq);
        assert_eq!(r_bad["equation"].as_array().unwrap().len(), 2, "{:?}", r_bad["equation"]);

        // P1-3: formal_v2 沿用 lender must have FormalRole::Notation facet AND non-empty body
        // (title prefix `符号` alone or empty-body notation card must be rejected).
        let fh = "K/7|a";
        let fm = json!({
            "level": "formal",
            "form": null,
            "head": { "address": fh },
            "build": {
                "setting": format!("{fh}|a"),
                "order": [
                    { "address": format!("{fh}|a"), "line": "main", "facet": "设定", "title": "设定" },
                    { "address": format!("{fh}|a|a"), "line": "side", "facet": "符号表与读法", "title": "符号表与读法" },
                ],
                "closure": [],
                "symbol_uncovered": [],
            }
        });
        let fcards_title_only = HashMap::from([
            (format!("{fh}|a"), card("设定", "设定", "**起点**\n原始设定。\n\n**设定**\n无符号。\n\n**直觉**\n直觉。\n\n**说明**\n说明。")),
            (format!("{fh}|a|a"), card("符号表与读法", "符号表与读法", "沿用 [[K/1|a|a]] 的全部符号。")),
            ("K/1|a|a".into(), card("设定：其他", "符号表：标题冒充", "| $x$ | 值 |")),
        ]);
        let r_title = audit_formal(&fm, &fcards_title_only, &HashMap::new());
        assert!(
            r_title["format"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["what"] == "notation_target"),
            "title-only lender must fail: {r_title}"
        );

        let fcards_empty_lender = HashMap::from([
            (format!("{fh}|a"), card("设定", "设定", "**起点**\n原始设定。\n\n**设定**\n无符号。\n\n**直觉**\n直觉。\n\n**说明**\n说明。")),
            (format!("{fh}|a|a"), card("符号表与读法", "符号表与读法", "沿用 [[K/1|a|a]] 的全部符号。")),
            ("K/1|a|a".into(), card("符号表与读法", "符号表与读法", "   \n  ")),
        ]);
        let r_empty = audit_formal(&fm, &fcards_empty_lender, &HashMap::new());
        assert!(
            r_empty["format"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["what"] == "notation_target"),
            "empty-body lender must fail: {r_empty}"
        );

        // M11: prose saying "书中未写标准误" outside **估计式** must NOT invalidate a valid display equation.
        let mut cards_prose_note = cards.clone();
        cards_prose_note.insert(
            format!("{h}|a|2"),
            card("规格", "规格", "**估计式**\n$$y = x$$\n\n**说明**\n书中未写标准误的自由度调整。"),
        );
        let r_prose = audit_empirical(&m, &cards_prose_note);
        assert!(
            !r_prose["equation"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["card"] == format!("{h}|a|2")),
            "valid display equation must not fail because of prose 书中未写标准误: {r_prose}"
        );

        // M6: missing lender on legacy (non-formal_v2) model must still emit a notation_target fault.
        let mut legacy_m = fm.clone();
        legacy_m["level"] = json!("concept");
        let fcards_missing_lender = HashMap::from([
            (format!("{fh}|a"), card("设定", "设定", "**起点**\n原始设定。\n\n**设定**\n无符号。\n\n**直觉**\n直觉。\n\n**说明**\n说明。")),
            (format!("{fh}|a|a"), card("符号表与读法", "符号表与读法", "沿用 [[K/missing]] 的全部符号。")),
        ]);
        let r_missing = audit_formal(&legacy_m, &fcards_missing_lender, &HashMap::new());
        assert!(
            r_missing["format"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f["what"] == "notation_target"),
            "unresolved legacy 沿用 lender must fail: {r_missing}"
        );

        // M7: validate_settings_payload rejects malformed build.order rows (`{}`) and omitted
        // empirical/formal fault arrays instead of silently dropping them.
        let bad_order = json!({
            "level": "empirical",
            "form": null,
            "head": { "address": "K/5|a" },
            "build": {
                "order": [{ "address": "K/5|a|a" }, {}],
                "empirical_shape": [],
                "spec_unbound": [],
                "estimate_unbound": [],
                "test_unbound": []
            }
        });
        assert!(validate_settings_payload(&bad_order, "K/5|a").is_err());
        let missing_emp_faults = json!({
            "level": "empirical",
            "form": null,
            "head": { "address": "K/5|a" },
            "build": {
                "order": [{ "address": "K/5|a|a" }]
            }
        });
        assert!(validate_settings_payload(&missing_emp_faults, "K/5|a").is_err());
        let missing_var_uncovered = json!({
            "level": "empirical",
            "form": null,
            "head": { "address": "K/5|a" },
            "build": {
                "order": [{ "address": "K/5|a|a" }],
                "empirical_shape": [],
                "spec_unbound": [],
                "estimate_unbound": [],
                "test_unbound": []
            }
        });
        assert!(validate_settings_payload(&missing_var_uncovered, "K/5|a").is_err());
        let valid_emp = json!({
            "level": "empirical",
            "form": null,
            "head": { "address": "K/5|a" },
            "build": {
                "order": [{ "address": "K/5|a|a" }],
                "empirical_shape": [],
                "spec_unbound": [],
                "estimate_unbound": [],
                "test_unbound": [],
                "variable_uncovered": []
            }
        });
        assert_eq!(validate_settings_payload(&valid_emp, "K/5|a").unwrap(), vec!["K/5|a|a"]);

        // M7: formal settings payload must require `relations` array with valid rows.
        let formal_no_relations = json!({
            "level": "formal",
            "form": null,
            "head": { "address": "K/7|a" },
            "build": {
                "order": [{ "address": "K/7|a|a" }],
                "closure": [],
                "symbol_uncovered": []
            }
        });
        assert!(validate_settings_payload(&formal_no_relations, "K/7|a").is_err());
        let mut formal_ok = formal_no_relations.clone();
        formal_ok["relations"] = json!([{ "kind": "rests-on", "from": "K/7|a|a|b", "to": "K/7|a|a" }]);
        assert!(validate_settings_payload(&formal_ok, "K/7|a").is_ok());
    }
}
