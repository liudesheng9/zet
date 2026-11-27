//! Chart & intuition deterministic lint and model coverage checks (`zt-check --model <head> --charts`
//! and `zt-check --charts-json <file>`).
//!
//! Mirrors `lint.mjs`, `intuition.mjs` (lint & span), and the non-rendering checks of `verify.mjs`
//! from `zt-model-illustrate`.

use crate::mcp::Zt;
use crate::text::strip_term_links;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

const LABELS: [(&str, &str); 3] = [
    ("what", "是什么"),
    ("why", "为什么需要"),
    ("how", "怎样进入模型"),
];

const ASSUMP_CORE: &str = r"(?:[A-Za-z0-9]+(?:[.\-][A-Za-z0-9]+)*[′']*|[一二三四五六七八九十]+)";

static ASSUMP_RAW_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?:假设|假定)\s*(?:（({ASSUMP_CORE})）|\(({ASSUMP_CORE})\)|({ASSUMP_CORE}))"
    ))
    .expect("assump raw regex")
});
static ASSUMP_SELF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^(?:假设|假定)\s*(?:（{ASSUMP_CORE}）|\({ASSUMP_CORE}\)|{ASSUMP_CORE})(?:[：:\s]|$)"
    ))
    .expect("assump self regex")
});
static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<(/?)([a-zA-Z][\w-]*)([^>]*)>").expect("html tag regex"));
static DATA_PART_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\sdata-part\s*=\s*(?:"([^"]*)"|'([^']*)')"#).expect("data-part regex"));
static TEXT_EL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<text\b[^>]*>(.*?)</text>").expect("svg text regex"));
static TSPAN_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<tspan[\s\S]*?</tspan>").expect("tspan regex"));
static SVG_MATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z\u{03B1}-\u{03C9}\u{0391}-\u{03A9}\u{0303}\u{0304}\)\]][_^][A-Za-z0-9\{]")
        .expect("svg math regex")
});
static STYLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<style\b[^>]*>(.*?)</style>").expect("style regex"));
static CSS_SEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([^{}]+)\{").expect("css selector regex"));
static SVG_BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<svg\b[\s\S]*?</svg>").expect("svg block regex"));
static ANY_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"</?[a-zA-Z][^>]*>").expect("any tag regex"));
static TEX_MATH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\$\$[\s\S]*?\$\$|\$[^$\n]+\$").expect("tex math regex"));
static HTML_MATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[A-Za-z\u{03B1}-\u{03C9}\u{0391}-\u{03A9}\u{0303}][_^][A-Za-z0-9\{]+")
        .expect("html math regex")
});
static XML_COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<!--[\s\S]*?-->").expect("xml comment regex"));
static START_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<([a-zA-Z][\w:-]*)((?:[^>"']|"[^"]*"|'[^']*')*)/?>"#).expect("start tag regex"));
static LINE_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<line\b(?:[^>"']|"[^"]*"|'[^']*')*>"#).expect("svg line regex"));
static ARIA_LABEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\saria-label\s*=\s*(?:"([^"]*)"|'([^']*)')"#).expect("aria-label regex"));
static VIEWBOX_W_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<svg\b[^>]*\sviewBox\s*=\s*(?:"\s*0\s+0\s+(\d+)|'\s*0\s+0\s+(\d+))"#)
        .expect("viewbox width regex")
});
static ATTR_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\sid\s*=\s*(?:"([^"]+)"|'([^']+)')"#).expect("id attr regex"));
static CLASS_ZH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\sclass\s*=\s*(?:"[^"]*\bzh\b[^"]*"|'[^']*\bzh\b[^']*')"#)
        .expect("class zh regex")
});
static LIGHT_VERB_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(进行|加以|予以|作出|做出)[\u{4E00}-\u{9FFF}]").expect("light verb regex"));
static DEPENDS_LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"依赖\s*\[\[").expect("depends link regex"));
static INLINE_TEX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$[^$]+\$").expect("inline tex regex"));
static CHART_FENCE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)```(?:chart|html[^\n]*chart)[^\n]*\n(.*?)\n```").expect("chart fence regex")
});
static DATA_ZT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\sdata-zt\s*=\s*(?:"([^"]+)"|'([^']+)')"#).expect("data-zt regex"));

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartFault {
    pub rule: String,
    pub text: String,
}

impl ChartFault {
    pub fn to_json(&self) -> Value {
        json!({ "rule": self.rule, "text": self.text })
    }
}

fn is_assump_continuation_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "′'一二三四五六七八九十）)".contains(c)
}

/// Shared bare-assumption citation finder (mirrors `ASSUMP_CITE_RE` / `ASSUMP_BARE_RE` without lookahead).
pub(crate) fn find_assumption_citations(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for cap in ASSUMP_RAW_RE.captures_iter(text) {
        let Some(m) = cap.get(0) else {
            continue;
        };
        let rest = &text[m.end()..];
        let mut chars = rest.chars();
        if let Some(c1) = chars.next() {
            if is_assump_continuation_char(c1) {
                continue;
            }
            if (c1 == '.' || c1 == '-') && chars.next().is_some_and(|c2| c2.is_ascii_alphanumeric()) {
                continue;
            }
        }
        let key = cap
            .get(1)
            .or_else(|| cap.get(2))
            .or_else(|| cap.get(3))
            .map_or("", |g| g.as_str())
            .to_string();
        if !key.is_empty() {
            out.push((m.as_str().to_string(), key));
        }
    }
    out
}

/// Top-level `[data-part]` titles as `chart-frame.js` finds them.
pub fn top_parts(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut open: Option<(String, i32)> = None;
    for cap in TAG_RE.captures_iter(src) {
        let close = !cap.get(1).map_or("", |m| m.as_str()).is_empty();
        let name = cap.get(2).map_or("", |m| m.as_str());
        let attrs = cap.get(3).map_or("", |m| m.as_str());
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "br" | "img" | "hr" | "input" | "meta" | "link" | "path" | "line" | "rect"
                | "circle" | "ellipse" | "polygon" | "polyline" | "use" | "stop"
        ) || attrs.ends_with('/')
        {
            continue;
        }
        if close {
            if let Some((title, open_depth)) = &open
                && *open_depth == depth
            {
                out.push(title.clone());
                open = None;
            }
            depth -= 1;
            continue;
        }
        depth += 1;
        if open.is_none()
            && let Some(part_cap) = DATA_PART_RE.captures(attrs)
        {
            let title = part_cap
                .get(1)
                .or_else(|| part_cap.get(2))
                .map_or("", |m| m.as_str())
                .to_string();
            open = Some((title, depth));
        }
    }
    out
}

fn selector_hits_bare_span(sel: &str) -> bool {
    let bytes = sel.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if &bytes[i..i + 4] == b"span" {
            let prev_ok = i == 0
                || matches!(bytes[i - 1], b' ' | b'\t' | b'\n' | b'\r' | b',' | b'>' | b'+' | b'~');
            let next_ok = i + 4 == bytes.len()
                || !(bytes[i + 4].is_ascii_alphanumeric()
                    || matches!(bytes[i + 4], b'_' | b'-' | b'.' | b'#' | b'['));
            if prev_ok && next_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Parses HTML/SVG start-tag attributes properly (skipping quoted attribute values so text such as
/// `aria-label="grid x1='145'"` or hyphen-prefixed `data-x1='145'` never spoofs an attribute, M10).
fn parse_tag_attrs(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = tag.as_bytes();
    let mut i = 0;
    // Skip leading `<tagname` when passed a full `<...>` tag.
    if bytes.first() == Some(&b'<') {
        i += 1;
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' && bytes[i] != b'/' {
            i += 1;
        }
    }
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'>' {
            break;
        }
        let name_start = i;
        while i < bytes.len()
            && !bytes[i].is_ascii_whitespace()
            && !matches!(bytes[i], b'=' | b'>' | b'/')
        {
            i += 1;
        }
        let name = tag[name_start..i].to_string();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
                let quote = bytes[i];
                i += 1;
                let val_start = i;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                let val = tag[val_start..i].to_string();
                if i < bytes.len() {
                    i += 1;
                }
                out.push((name, val));
            } else {
                let val_start = i;
                while i < bytes.len() && !bytes[i].is_ascii_whitespace() && !matches!(bytes[i], b'>' | b'/') {
                    i += 1;
                }
                out.push((name, tag[val_start..i].to_string()));
            }
        } else if !name.is_empty() {
            out.push((name, String::new()));
        }
    }
    out
}

fn svg_attr_num(attrs: &[(String, String)], name: &str) -> Option<f64> {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .and_then(|(_, v)| v.trim().parse::<f64>().ok())
}

/// Lint one in-Card chart HTML/SVG snippet (mirrors `lint.mjs`).
pub fn lint_chart(src: &str, setting: bool, model: bool) -> Vec<ChartFault> {
    let mut out = Vec::new();
    let mut add = |rule: &str, text: String| {
        out.push(ChartFault {
            rule: rule.to_string(),
            text,
        })
    };

    // svg-math / svg-tex
    for cap in TEXT_EL_RE.captures_iter(src) {
        let inner = cap.get(1).map_or("", |m| m.as_str());
        let bare = TSPAN_RE.replace_all(inner, "");
        let clip: String = inner.chars().take(60).collect();
        if SVG_MATH_RE.is_match(&bare) {
            add(
                "svg-math",
                format!("SVG text with raw _ or ^ (wrap it in m()): {clip}"),
            );
        }
        if inner.contains('$') {
            add("svg-tex", format!("TeX inside SVG text never renders: {clip}"));
        }
    }

    // span-css: scans every selector before `{` (including nested `@media { span { ... } }`).
    for cap in STYLE_RE.captures_iter(src) {
        let css = cap.get(1).map_or("", |m| m.as_str());
        for sel_cap in CSS_SEL_RE.captures_iter(css) {
            let trimmed = sel_cap.get(1).map_or("", |m| m.as_str()).trim();
            if selector_hits_bare_span(trimmed) {
                add(
                    "span-css",
                    format!(
                        "CSS selector \"{trimmed}\" styles every span, KaTeX's too: give the element a class"
                    ),
                );
            }
        }
    }

    // html-math
    let no_svg = SVG_BLOCK_RE.replace_all(src, " ");
    let no_style = STYLE_RE.replace_all(&no_svg, " ");
    let html = ANY_TAG_RE.replace_all(&no_style, " ");
    let prose = TEX_MATH_RE.replace_all(&html, " ");
    for m in HTML_MATH_RE.find_iter(&prose) {
        add(
            "html-math",
            format!("HTML text with math outside $…$: {}", m.as_str()),
        );
    }

    // assumption-name
    if setting {
        for (hit, _) in find_assumption_citations(src) {
            add(
                "assumption-name",
                format!(
                    "\"{hit}\" in a setting card's chart counts as a bare citation: name the content, or let the box's data-zt open the card"
                ),
            );
        }
    }

    // axis-label (strip XML comments and parse exact start-tag attributes so quoted text like
    // `aria-label="grid x1='145'"` and commented-out labels cannot spoof axis verdicts, M10).
    for m in SVG_BLOCK_RE.find_iter(src) {
        let s_clean = XML_COMMENT_RE.replace_all(m.as_str(), "");
        let s = s_clean.as_ref();
        let mut vert = Vec::new();
        let mut hor = Vec::new();
        for lm in LINE_TAG_RE.find_iter(s) {
            let attrs = parse_tag_attrs(lm.as_str());
            let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
                svg_attr_num(&attrs, "x1"),
                svg_attr_num(&attrs, "y1"),
                svg_attr_num(&attrs, "x2"),
                svg_attr_num(&attrs, "y2"),
            ) else {
                continue;
            };
            if (x1 - x2).abs() < 0.5 && (y1 - y2).abs() > 20.0 {
                vert.push((x1, y1.max(y2)));
            }
            if (y1 - y2).abs() < 0.5 && (x1 - x2).abs() > 20.0 {
                hor.push((x1.min(x2), y1));
            }
        }
        let plots = vert
            .iter()
            .filter(|&&(vx, vy)| hor.iter().any(|&(hx, hy)| (vx - hx).abs() < 1.0 && (vy - hy).abs() < 1.0))
            .count();
        if plots == 0 {
            continue;
        }
        let mut nx = 0usize;
        let mut ny = 0usize;
        for cap in START_TAG_RE.captures_iter(s) {
            for (k, v) in parse_tag_attrs(cap.get(0).map_or("", |g| g.as_str())) {
                if k == "data-axis" {
                    if v == "x" {
                        nx += 1;
                    } else if v == "y" {
                        ny += 1;
                    }
                }
            }
        }
        if nx < plots || ny < plots {
            let label = ARIA_LABEL_RE
                .captures(s)
                .and_then(|c| c.get(1).or_else(|| c.get(2)).map(|g| g.as_str()))
                .unwrap_or("svg");
            add(
                "axis-label",
                format!(
                    "a plot with {plots} axis corner(s) names {nx} x axis and {ny} y axis: label what each axis measures, with axes() or t(…, {{axis: 'x'|'y'}}) from draw.mjs ({label})"
                ),
            );
        }
    }

    // width
    for cap in VIEWBOX_W_RE.captures_iter(src) {
        if let Ok(w) = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map_or("0", |m| m.as_str())
            .parse::<u64>()
            && w > 760
        {
            add(
                "width",
                format!("viewBox width {w} > 760: text becomes small in the reader"),
            );
        }
    }

    // dup-id
    let mut id_counts: HashMap<String, usize> = HashMap::new();
    let mut id_order: Vec<String> = Vec::new();
    for cap in ATTR_ID_RE.captures_iter(src) {
        let id = cap
            .get(1)
            .or_else(|| cap.get(2))
            .map_or("", |m| m.as_str())
            .to_string();
        if !id_counts.contains_key(&id) {
            id_order.push(id.clone());
        }
        *id_counts.entry(id).or_insert(0) += 1;
    }
    for id in id_order {
        let n = id_counts[&id];
        if n > 1 {
            add(
                "dup-id",
                format!(
                    "id \"{id}\" is defined {n} times: on the chart sheet the copy in a hidden part stops drawing (arrowheads vanish); give each svg its own id (svg() in draw.mjs does)"
                ),
            );
        }
    }

    // parts
    let parts = top_parts(src);
    let heads = CLASS_ZH_RE.find_iter(src).count();
    if model && parts.len() < 3 {
        add(
            "parts",
            format!(
                "the Model Card figure has {} top-level data-part element(s): wrap the game, the causal structure and the changes each in part(title, html) from draw.mjs",
                parts.len()
            ),
        );
    } else if !model && heads >= 2 && parts.len() < heads {
        add(
            "parts",
            format!(
                "a chart with {heads} headed sections has {} top-level data-part element(s): wrap each section in part(title, html) from draw.mjs",
                parts.len()
            ),
        );
    }
    if parts.len() > 12 {
        add(
            "parts",
            format!("{} parts: the chart sheet shows at most 12 tabs", parts.len()),
        );
    }
    for title in &parts {
        if title.trim().is_empty() || title.chars().count() > 48 {
            add(
                "parts",
                format!("part title \"{title}\" is empty or longer than 48 characters (the tab cuts it)"),
            );
        }
    }

    out
}

/// Find the byte span `(start, end)` of the `**直觉**` section in `body`, stopping at any
/// subsequent bold heading (standalone or inline), code fence, or `来源：` line.
pub fn find_intuition_span(body: &str) -> Option<(usize, usize)> {
    let mut offset = 0usize;
    let mut start: Option<usize> = None;
    let bytes = body.as_bytes();
    let lines: Vec<&str> = body.split('\n').collect();
    for (i, line) in lines.iter().enumerate() {
        let next_offset = offset + line.len() + usize::from(i < lines.len() - 1);
        let trimmed = line.trim();
        if start.is_none() {
            if trimmed == "**直觉**" {
                let s = if offset > 0 && bytes[offset - 1] == b'\n' {
                    offset - 1
                } else {
                    offset
                };
                start = Some(s);
            }
        } else {
            let is_bold_heading = trimmed.starts_with("**")
                && trimmed[2..].find("**").is_some_and(|p| p > 0);
            let is_fence = trimmed.starts_with("```");
            let is_source = trimmed.starts_with("来源：") || trimmed.starts_with("来源:");
            if is_bold_heading || is_fence || is_source {
                return Some((start.unwrap(), offset));
            }
        }
        offset = next_offset;
    }
    start.map(|s| (s, body.len()))
}

fn norm_assump(v: &str) -> String {
    v.replacen("假定", "假设", 1)
        .replace('（', "(")
        .replace('）', ")")
        .split_whitespace()
        .collect::<String>()
}

/// Lint the three lines of a setting/definition card's `**直觉**` section (mirrors `intuition.mjs`).
pub fn lint_intuition(
    what: &str,
    why: &str,
    how: &str,
    self_assump: Option<&str>,
) -> Vec<ChartFault> {
    let mut out = Vec::new();
    let mut add = |rule: &str, text: String| {
        out.push(ChartFault {
            rule: rule.to_string(),
            text,
        })
    };
    let self_norm = self_assump.map(norm_assump);
    let values = [what, why, how];
    for ((_, label), raw) in LABELS.iter().zip(values.iter()) {
        let s = raw.trim();
        if s.is_empty() {
            add("intuition-missing", format!("no {label} line"));
            continue;
        }
        if s.contains('；') {
            add(
                "intuition-semicolon",
                format!("{label}: a semicolon (R3): write two sentences"),
            );
        }
        if let Some(m) = LIGHT_VERB_RE.find(s) {
            add(
                "intuition-light-verb",
                format!("{label}: a light verb (R5): {}", m.as_str()),
            );
        }
        if DEPENDS_LINK_RE.is_match(s) {
            add(
                "intuition-depends",
                format!("{label}: a 依赖 Link belongs in **起点**"),
            );
        }
        if s.contains("[[") {
            add(
                "intuition-link",
                format!("{label}: no Link in 直觉 (a Link to a later card reads as a forward premise): name the chapter in words"),
            );
        }
        if s.contains("$$") || s.contains("```") {
            add(
                "intuition-display",
                format!("{label}: no display formula or chart in 直觉"),
            );
        }
        for (hit, _) in find_assumption_citations(s) {
            if self_norm.as_deref() != Some(norm_assump(&hit).as_str()) {
                add(
                    "intuition-assumption",
                    format!("{label}: \"{hit}\" is a bare citation (check-settings: cite): name the content"),
                );
            }
        }
        for sentence in s.split(['。', '！', '？']) {
            let plain = INLINE_TEX_RE.replace_all(sentence, "x");
            let trimmed = plain.trim();
            if !trimmed.is_empty() && trimmed.chars().count() > 80 {
                let clip: String = trimmed.chars().take(30).collect();
                add(
                    "intuition-long",
                    format!("{label}: a sentence over 80 characters (R2): {clip}…"),
                );
            }
        }
    }
    out
}

/// Offline chart + intuition coverage and content-lint audit (used by live `--model <head> --charts`).
/// `cards`: `(address, facet, title, body)`.
pub fn audit_charts_offline(
    head: &str,
    cards: &[(String, String, String, String)],
    target_exists: &HashMap<String, bool>,
) -> Value {
    let mut charts = 0usize;
    let mut lint_faults: Vec<Value> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut no_intuition: Vec<String> = Vec::new();

    for (at, facet, title, body) in cards {
        let fences: Vec<&str> = CHART_FENCE_RE
            .captures_iter(body)
            .filter_map(|c| c.get(1).map(|m| m.as_str()))
            .collect();
        let is_head = at == head;
        let setting = !is_head
            && (facet.starts_with("设定") || facet.starts_with("假设") || facet.starts_with("假定"));
        let definition = !is_head && facet.starts_with("定义");
        let setting_like = setting
            || definition
            || (!is_head
                && (facet.starts_with("判据")
                    || facet.starts_with("均衡定义")
                    || facet.starts_with("解概念")));

        if !head.is_empty() && fences.is_empty() && (is_head || setting) {
            missing.push(at.clone());
        }
        if !head.is_empty() && (setting || definition) {
            let sec = find_intuition_span(body)
                .map(|(s, e)| &body[s..e])
                .unwrap_or("");
            let mut extracted = [String::new(), String::new(), String::new()];
            let mut lacks: Vec<&str> = Vec::new();
            for (idx, (_, label)) in LABELS.iter().enumerate() {
                let prefix = format!("- {label}：");
                if let Some(rest) = sec.lines().find_map(|l| {
                    l.strip_prefix(&prefix)
                        .filter(|r| r.chars().next().is_some_and(|c| !c.is_whitespace()))
                }) {
                    // Strip post-authoring Term links (`[[label@addr]]` -> `label`) before linting
                    // so Term links added by term-link don't trigger false `intuition-link` faults,
                    // while ordinary links, semicolons, light verbs, and bare assumptions still trigger.
                    extracted[idx] = strip_term_links(rest);
                } else {
                    lacks.push(*label);
                }
            }
            if !lacks.is_empty() {
                no_intuition.push(format!("{at} ({})", lacks.join("、")));
            }
            let self_label = ASSUMP_SELF_RE
                .find(title)
                .map(|m| m.as_str().trim_end_matches(['：', ':', ' ', '\t']));
            for f in lint_intuition(&extracted[0], &extracted[1], &extracted[2], self_label) {
                if f.rule != "intuition-missing" {
                    lint_faults.push(json!({ "card": at, "rule": f.rule, "text": f.text }));
                }
            }
        }

        for src in fences {
            charts += 1;
            for f in lint_chart(src, setting_like, is_head) {
                lint_faults.push(json!({ "card": at, "rule": f.rule, "text": f.text }));
            }
            let mut seen_targets = HashSet::new();
            for cap in DATA_ZT_RE.captures_iter(src) {
                let target = cap
                    .get(1)
                    .or_else(|| cap.get(2))
                    .map_or("", |m| m.as_str());
                if !seen_targets.insert(target.to_string()) {
                    continue;
                }
                if !target_exists.get(target).copied().unwrap_or(false) {
                    lint_faults.push(json!({
                        "card": at,
                        "rule": "dead-target",
                        "text": format!("data-zt=\"{target}\" opens no Card (renumbered by a later move?)"),
                    }));
                }
            }
        }
    }

    let ok = lint_faults.is_empty() && missing.is_empty() && no_intuition.is_empty();
    json!({
        "head": head,
        "ok": ok,
        "cards": cards.len(),
        "charts": charts,
        "faults": lint_faults,
        "missing": missing,
        "no_intuition": no_intuition,
    })
}

fn format_audit_text(report: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for f in report["faults"].as_array().into_iter().flatten() {
        out.push(format!(
            "lint {}: {}: {}",
            f["rule"].as_str().unwrap_or(""),
            f["card"].as_str().unwrap_or(""),
            f["text"].as_str().unwrap_or("")
        ));
    }
    for at in report["missing"].as_array().into_iter().flatten() {
        if let Some(a) = at.as_str() {
            out.push(format!(
                "missing: {a} has no figure (the Model Card and every setting card need one)"
            ));
        }
    }
    for item in report["no_intuition"].as_array().into_iter().flatten() {
        if let Some(s) = item.as_str() {
            out.push(format!(
                "intuition: {s} lacks 直觉 lines (every setting, assumption, and definition card needs 是什么, 为什么需要, 怎样进入模型)"
            ));
        }
    }
    out.push(format!(
        "{} chart(s) in {} card(s); {} lint fault(s); {} card(s) without a figure; {} card(s) without a full 直觉",
        report["charts"].as_u64().unwrap_or(0),
        report["cards"].as_u64().unwrap_or(0),
        report["faults"].as_array().map_or(0, Vec::len),
        report["missing"].as_array().map_or(0, Vec::len),
        report["no_intuition"].as_array().map_or(0, Vec::len),
    ));
    out
}

pub fn run_live(zt: &mut Zt, head: &str, json_out: bool) -> Result<(Vec<String>, u8, usize)> {
    let rev_before = zt.cards(&[], "text")?.archive_revision;
    let model = zt.call("get_model", json!({ "at": head }))?;
    let got_head = model["head"]["address"].as_str().unwrap_or("");
    if got_head != head {
        if got_head.is_empty() {
            bail!("{head} is not a Model Card");
        }
        bail!("{head} is not a Model Card: it belongs to the model {got_head}; give that address");
    }
    let parts = model["parts"]
        .as_array()
        .with_context(|| format!("get_model answer for {head} lacks `parts` array"))?;
    let mut addresses = vec![head.to_string()];
    for (idx, p) in parts.iter().enumerate() {
        let Some(filed) = p["filed"].as_bool() else {
            bail!("get_model answer for {head} parts[{idx}] lacks boolean `filed`");
        };
        let Some(a) = p["address"].as_str().filter(|s| !s.is_empty()) else {
            bail!("get_model answer for {head} parts[{idx}] lacks non-empty string `address`");
        };
        if filed && a != head {
            addresses.push(a.to_string());
        }
    }
    let fetched = zt.cards(&addresses, "text")?;
    if fetched.archive_revision != rev_before {
        bail!("incomplete checker inputs: changing read view");
    }
    let by_addr: HashMap<String, Value> = fetched
        .cards
        .into_iter()
        .filter_map(|c| {
            let a = c.get("address").and_then(Value::as_str)?.to_string();
            Some((a, c))
        })
        .collect();

    let mut cards = Vec::new();
    let mut data_zt_targets: Vec<String> = Vec::new();
    let mut seen_targets: HashSet<String> = HashSet::new();
    for at in &addresses {
        let Some(c) = by_addr.get(at) else {
            bail!("read-failed: could not load card at {at}");
        };
        let facet = c.get("facet").and_then(Value::as_str).unwrap_or("").to_string();
        let title = c.get("title").and_then(Value::as_str).unwrap_or("").to_string();
        let body = c.get("body").and_then(Value::as_str).unwrap_or("").to_string();
        // Collect data-zt targets ONLY inside chart fences (never prose examples outside charts).
        for fence_cap in CHART_FENCE_RE.captures_iter(&body) {
            let src = fence_cap.get(1).map_or("", |m| m.as_str());
            for cap in DATA_ZT_RE.captures_iter(src) {
                let t = cap
                    .get(1)
                    .or_else(|| cap.get(2))
                    .map_or("", |m| m.as_str());
                if seen_targets.insert(t.to_string()) {
                    data_zt_targets.push(t.to_string());
                }
            }
        }
        cards.push((at.clone(), facet, title, body));
    }
    let mut target_exists: HashMap<String, bool> = HashMap::new();
    if !data_zt_targets.is_empty() {
        let checked = zt.cards_optional(&data_zt_targets, "text")?;
        if checked.archive_revision != rev_before {
            bail!("incomplete checker inputs: changing read view");
        }
        for (req, c) in checked.resolved {
            target_exists.insert(req, true);
            if let Some(a) = c.get("address").and_then(Value::as_str) {
                target_exists.insert(a.to_string(), true);
            }
        }
    }
    if zt.cards(&[], "text")?.archive_revision != rev_before {
        bail!("incomplete checker inputs: changing read view");
    }

    let report = audit_charts_offline(head, &cards, &target_exists);
    let fault_count = report["faults"].as_array().map_or(0, Vec::len)
        + report["missing"].as_array().map_or(0, Vec::len)
        + report["no_intuition"].as_array().map_or(0, Vec::len);
    let exit = if report["ok"].as_bool() == Some(true) { 0 } else { 1 };
    let lines = if json_out {
        vec![serde_json::to_string_pretty(&report)?]
    } else {
        format_audit_text(&report)
    };
    Ok((lines, exit, fault_count))
}

fn is_setting_like_facet(facet: &str) -> bool {
    facet.starts_with("设定")
        || facet.starts_with("假设")
        || facet.starts_with("假定")
        || facet.starts_with("定义")
        || facet.starts_with("判据")
        || facet.starts_with("均衡定义")
        || facet.starts_with("解概念")
}

/// Unwraps ```` ```chart ... ``` ```` fence(s) if present; otherwise treats `raw` as HTML.
fn unwrap_chart_fences(raw: &str) -> Vec<&str> {
    let fenced: Vec<&str> = CHART_FENCE_RE
        .captures_iter(raw)
        .filter_map(|c| c.get(1).map(|m| m.as_str()))
        .collect();
    if !fenced.is_empty() {
        fenced
    } else {
        vec![raw]
    }
}

/// Offline lint of a JSON payload exported from an illustration module:
/// `{ "head"?: "...", "figures"?: { "<addr>": "<fence or html>" }, "intuitions"?: { "<addr>": { "what", "why", "how", "self"? } }, "settings"?: ["<addr>", ...], "facets"?: { "<addr>": "<facet>" } }`.
/// Omitting `settings` and `facets` defaults setting-card membership to empty (never silently flips verdicts).
pub fn run_json_file(path: &str, json_out: bool) -> Result<(Vec<String>, u8, usize)> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    let doc: Value = serde_json::from_str(raw).with_context(|| format!("{path} is not valid JSON"))?;
    let Some(obj) = doc.as_object() else {
        bail!("{path} must be a JSON object with `figures` and/or `intuitions`");
    };
    let has_figures = obj.contains_key("figures");
    let has_intuitions = obj.contains_key("intuitions");
    if !has_figures && !has_intuitions {
        bail!("{path} must contain at least one of `figures` or `intuitions`");
    }
    let head = match obj.get("head") {
        None | Some(Value::Null) => "",
        Some(Value::String(s)) => s.as_str(),
        Some(_) => bail!("{path}: `head` must be a string"),
    };
    let explicit_settings: HashSet<&str> = match obj.get("settings") {
        None | Some(Value::Null) => HashSet::new(),
        Some(Value::Array(arr)) => {
            let mut set = HashSet::new();
            for item in arr {
                let s = item
                    .as_str()
                    .with_context(|| format!("{path}: every entry of `settings` must be a string"))?;
                set.insert(s);
            }
            set
        }
        Some(_) => bail!("{path}: `settings` must be an array of strings"),
    };
    let facets_map: HashMap<&str, &str> = match obj.get("facets") {
        None | Some(Value::Null) => HashMap::new(),
        Some(Value::Object(map)) => {
            let mut out = HashMap::new();
            for (k, v) in map {
                let s = v
                    .as_str()
                    .with_context(|| format!("{path}: `facets[{k}]` must be a string"))?;
                out.insert(k.as_str(), s);
            }
            out
        }
        Some(_) => bail!("{path}: `facets` must be an object of strings"),
    };

    let mut faults = Vec::new();
    if let Some(figs_val) = obj.get("figures") {
        let figs = figs_val
            .as_object()
            .with_context(|| format!("{path}: `figures` must be an object"))?;
        for (at, val) in figs {
            let raw_src = val
                .as_str()
                .with_context(|| format!("{path}: `figures[{at}]` must be a string"))?;
            let is_model = !head.is_empty() && at == head;
            let is_setting = !is_model
                && (explicit_settings.contains(at.as_str())
                    || facets_map
                        .get(at.as_str())
                        .is_some_and(|f| is_setting_like_facet(f)));
            for src in unwrap_chart_fences(raw_src) {
                for f in lint_chart(src, is_setting, is_model) {
                    faults.push(json!({ "card": at, "rule": f.rule, "text": f.text }));
                }
            }
        }
    }
    if let Some(ints_val) = obj.get("intuitions") {
        let ints = ints_val
            .as_object()
            .with_context(|| format!("{path}: `intuitions` must be an object"))?;
        for (at, val) in ints {
            let entry = val
                .as_object()
                .with_context(|| format!("{path}: `intuitions[{at}]` must be an object"))?;
            let what = entry.get("what").and_then(Value::as_str).unwrap_or("");
            let why = entry.get("why").and_then(Value::as_str).unwrap_or("");
            let how = entry.get("how").and_then(Value::as_str).unwrap_or("");
            let self_assump = entry.get("self").and_then(Value::as_str);
            for f in lint_intuition(what, why, how, self_assump) {
                faults.push(json!({ "card": at, "rule": f.rule, "text": f.text }));
            }
        }
    }
    let fault_count = faults.len();
    let ok = faults.is_empty();
    let report = json!({ "head": head, "ok": ok, "faults": faults });
    let exit = if ok { 0 } else { 1 };
    let lines = if json_out {
        vec![serde_json::to_string_pretty(&report)?]
    } else {
        let mut out: Vec<String> = report["faults"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|f| {
                format!(
                    "lint {}: {}: {}",
                    f["rule"].as_str().unwrap_or(""),
                    f["card"].as_str().unwrap_or(""),
                    f["text"].as_str().unwrap_or("")
                )
            })
            .collect();
        out.push(format!("faults: {fault_count}"));
        out
    };
    Ok((lines, exit, fault_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_chart_catches_svg_math_tex_span_css_nested_media_html_math_assump_axis_width_dupid_and_parts() {
        let bad = r#"
<style>@media print { .box span { font-size: 10px; } }</style>
<svg viewBox="0 0 820 200" aria-label="plot">
  <defs><marker id="za"></marker><marker id="za"></marker></defs>
  <line x1="40" y1="20" x2="40" y2="160"/>
  <line x1="40" y1="160" x2="200" y2="160"/>
  <text x="10" y="10">e_R and $x$</text>
</svg>
<p>Plain y_R outside math and 假设 (B) plus 假设 1.2</p>
"#;
        let faults = lint_chart(bad, true, true);
        let rules: Vec<&str> = faults.iter().map(|f| f.rule.as_str()).collect();
        for expected in [
            "svg-math",
            "svg-tex",
            "span-css",
            "html-math",
            "assumption-name",
            "axis-label",
            "width",
            "dup-id",
            "parts",
        ] {
            assert!(rules.contains(&expected), "missing {expected} in {rules:?}");
        }
        assert_eq!(faults.iter().filter(|f| f.rule == "assumption-name").count(), 2);
    }

    #[test]
    fn lint_intuition_catches_semicolon_light_verb_links_and_bare_assumptions_respecting_self() {
        let faults = lint_intuition(
            "在假设 (B) 与假设 1.2 下成立；并进行计算。",
            "保证假设 B. 满足且假设一 成立，见 [[K/1]]。",
            "进入第 1 章。",
            Some("假设 (B)"),
        );
        let texts: Vec<&str> = faults.iter().map(|f| f.text.as_str()).collect();
        assert!(!texts.iter().any(|t| t.contains("假设 (B)")), "self assumption allowed: {texts:?}");
        assert!(texts.iter().any(|t| t.contains("假设 1.2")));
        assert!(texts.iter().any(|t| t.contains("假设 B")));
        assert!(texts.iter().any(|t| t.contains("假设一")));
        assert!(faults.iter().any(|f| f.rule == "intuition-semicolon"));
        assert!(faults.iter().any(|f| f.rule == "intuition-light-verb"));
        assert!(faults.iter().any(|f| f.rule == "intuition-link"));
    }

    #[test]
    fn find_intuition_span_and_live_audit_lint_intuition_while_allowing_term_links() {
        let body = "**起点**\n原始设定。\n\n**直觉**\n- 是什么：[[均衡@K/10]]；分号违规。\n- 为什么需要：理由。\n- 怎样进入模型：进入第 1 章。\n\n**说明** inline note.\n```chart\n<svg viewBox=\"0 0 400 100\"></svg>\n```\n";
        let (start, end) = find_intuition_span(body).expect("span found");
        let slice = &body[start..end];
        assert!(slice.contains("- 是什么：[[均衡@K/10]]"));
        assert!(!slice.contains("inline note"));
        assert!(body[end..].starts_with("**说明** inline note."));

        let report = audit_charts_offline(
            "K/2|a",
            &[
                (
                    "K/2|a".into(),
                    "".into(),
                    "模型".into(),
                    "```chart\n<section data-part=\"一\"></section><section data-part=\"二\"></section><section data-part=\"三\"></section>\n```".into(),
                ),
                ("K/2|a|a".into(), "设定".into(), "设定：环境".into(), body.into()),
            ],
            &HashMap::new(),
        );
        let rules: Vec<&str> = report["faults"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|f| f["rule"].as_str())
            .collect();
        assert!(rules.contains(&"intuition-semicolon"), "live audit must lint intuition: {rules:?}");
        assert!(!rules.contains(&"intuition-link"), "post-authoring Term link must not fail live intuition lint: {rules:?}");
    }

    #[test]
    fn charts_json_rejects_wrong_shaped_json_and_respects_explicit_empty_settings() {
        let bad = std::env::temp_dir().join(format!("zt-charts-bad-{}.json", std::process::id()));
        std::fs::write(&bad, r#"{"cards": []}"#).unwrap();
        assert!(run_json_file(bad.to_str().unwrap(), true).is_err());
        let _ = std::fs::remove_file(&bad);

        let empty_set = std::env::temp_dir().join(format!("zt-charts-empty-set-{}.json", std::process::id()));
        std::fs::write(
            &empty_set,
            json!({
                "head": "K/2|a",
                "settings": [],
                "figures": { "K/2|a|a": "<p>由假设 1 得出</p>" }
            })
            .to_string(),
        )
        .unwrap();
        let (_, exit, _) = run_json_file(empty_set.to_str().unwrap(), true).unwrap();
        let _ = std::fs::remove_file(&empty_set);
        assert_eq!(exit, 0, "explicit settings: [] means K/2|a|a is not linted as a setting card");

        // P0-8: omitting `settings` must match explicit empty `settings: []` (never silently flip
        // exit 0 <-> 1), while explicit `settings` or `facets` enables setting-card lint, and
        // ```chart ... ``` fences in figures values are unwrapped before linting.
        let omitted_set = std::env::temp_dir().join(format!("zt-charts-omitted-set-{}.json", std::process::id()));
        std::fs::write(
            &omitted_set,
            json!({
                "head": "K/2|a",
                "figures": { "K/2|a|a": "```chart\n<p>由假设 1 得出</p>\n```" }
            })
            .to_string(),
        )
        .unwrap();
        let (_, exit_omitted, _) = run_json_file(omitted_set.to_str().unwrap(), true).unwrap();
        let _ = std::fs::remove_file(&omitted_set);
        assert_eq!(exit_omitted, 0, "omitting settings must not silently treat card as setting");

        let fenced_bad = std::env::temp_dir().join(format!("zt-charts-fenced-bad-{}.json", std::process::id()));
        std::fs::write(
            &fenced_bad,
            json!({
                "head": "K/2|a",
                "settings": ["K/2|a|a"],
                "figures": {
                    "K/2|a|a": "```chart\n<svg viewBox=\"0 0 400 100\"><text>ok</text></svg><p>由假设 1 得出</p>\n```"
                }
            })
            .to_string(),
        )
        .unwrap();
        let (lines_fenced, exit_fenced, _) = run_json_file(fenced_bad.to_str().unwrap(), true).unwrap();
        let _ = std::fs::remove_file(&fenced_bad);
        assert_eq!(exit_fenced, 1);
        let parsed: Value = serde_json::from_str(&lines_fenced.join("\n")).unwrap();
        let rules: Vec<&str> = parsed["faults"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|f| f["rule"].as_str())
            .collect();
        assert_eq!(rules, vec!["assumption-name"], "fenced chart should unwrap cleanly: {parsed}");

        // P1-4: single-quoted data-zt='...' is checked, and present intuition lines are still linted
        // even when another intuition line is missing.
        let partial_body = "**直觉**\n- 是什么：含分号；违规。\n- 为什么需要：理由。\n\n```chart\n<div data-zt='K/dead'></div>\n```";
        let rep = audit_charts_offline(
            "K/2|a",
            &[
                ("K/2|a".into(), "".into(), "模型".into(), "```chart\n<section data-part=\"一\"></section><section data-part=\"二\"></section><section data-part=\"三\"></section>\n```".into()),
                ("K/2|a|a".into(), "设定".into(), "设定".into(), partial_body.into()),
            ],
            &HashMap::new(),
        );
        assert_eq!(rep["no_intuition"].as_array().unwrap().len(), 1);
        let rep_rules: Vec<&str> = rep["faults"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|f| f["rule"].as_str())
            .collect();
        assert!(rep_rules.contains(&"dead-target"), "single-quoted data-zt must be checked: {rep_rules:?}");
        assert!(rep_rules.contains(&"intuition-semicolon"), "present intuition lines must still be linted when another is missing: {rep_rules:?}");

        // M10: single-quoted viewBox, id, data-part, and spaced data-zt = '...' must be linted identically.
        let sq_bad = "<svg viewBox = '0 0 990 120'><marker id='m1'></marker><marker id = 'm1'></marker></svg><section data-part = '一'></section><section data-part='二'></section><section data-part='三'></section>";
        let sq_faults = lint_chart(sq_bad, false, true);
        let sq_rules: Vec<&str> = sq_faults.iter().map(|f| f.rule.as_str()).collect();
        assert!(sq_rules.contains(&"width"), "single-quoted viewBox width must be caught: {sq_rules:?}");
        assert!(sq_rules.contains(&"dup-id"), "single-quoted dup id must be caught: {sq_rules:?}");
        assert!(!sq_rules.contains(&"parts"), "3 single-quoted data-part elements must satisfy model chart parts: {sq_rules:?}");

        let spaced_zt = audit_charts_offline(
            "K/2|a",
            &[
                ("K/2|a".into(), "".into(), "模型".into(), "```chart\n<section data-part='一'></section><section data-part='二'></section><section data-part='三'></section><div data-zt = 'K/dead'></div>\n```".into()),
            ],
            &HashMap::new(),
        );
        assert!(spaced_zt["faults"].as_array().unwrap().iter().any(|f| f["rule"] == "dead-target"));

        // M10: whitespace around `=` in <line x1 = '0' ...> and data-axis = 'x'/'y' must match
        // unspaced syntax in both directions.
        let unlabeled_spaced = "<svg viewBox='0 0 400 200'><line x1 = '40' y1 = '20' x2 = '40' y2 = '180'/><line x1 = '40' y1 = '180' x2 = '360' y2 = '180'/></svg>";
        let unlabeled = lint_chart(unlabeled_spaced, false, false);
        let unlabeled_rules: Vec<&str> = unlabeled.iter().map(|f| f.rule.as_str()).collect();
        assert!(unlabeled_rules.contains(&"axis-label"), "spaced <line> coords must still flag unlabeled axes: {unlabeled_rules:?}");

        let labeled_spaced = "<svg viewBox='0 0 400 200'><line x1 = '40' y1 = '20' x2 = '40' y2 = '180'/><line x1 = '40' y1 = '180' x2 = '360' y2 = '180'/><text data-axis = 'x'>q</text><text data-axis = 'y'>p</text></svg>";
        let labeled_faults = lint_chart(labeled_spaced, false, false);
        assert!(labeled_faults.is_empty(), "spaced data-axis = 'x'/'y' must satisfy axis-label: {labeled_faults:?}");

        // M10: hyphen-prefixed attributes (`data-x1='145'`, `data-data-axis='x'`) must never shadow
        // real `x1` or satisfy `data-axis`.
        let prefixed_attr = "<svg viewBox='0 0 400 200'><line data-x1='145' x1='40' y1='20' x2='40' y2='180'/><line x1='40' y1='180' x2='360' y2='180'/><text data-data-axis='x'>q</text><text data-data-axis='y'>p</text></svg>";
        let prefixed_faults = lint_chart(prefixed_attr, false, false);
        assert!(
            prefixed_faults.iter().any(|f| f.rule == "axis-label"),
            "data-x1 and data-data-axis must not spoof axis geometry/labels: {prefixed_faults:?}"
        );

        // M10: quoted prose inside aria-label (`aria-label="grid x1='145'"`) and XML comments
        // (`<!-- <text data-axis='x'>... -->`) must NOT spoof line coordinates or axis labels.
        let quoted_and_commented = "<svg viewBox='0 0 400 200'><line aria-label=\"grid x1='145'\" x1='40' y1='20' x2='40' y2='180'/><line x1='40' y1='180' x2='360' y2='180'/><!-- <text data-axis='x'>q</text><text data-axis='y'>p</text> --></svg>";
        let qc_faults = lint_chart(quoted_and_commented, false, false);
        assert!(
            qc_faults.iter().any(|f| f.rule == "axis-label"),
            "quoted aria-label text and XML comments must not spoof axis-label: {qc_faults:?}"
        );
    }
}
