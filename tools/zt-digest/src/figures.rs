//! `zt-digest figures`: the port of `scripts/book-digest/attach-figures.ps1`, with the functions it
//! dot-sourced from `figures.ps1` (the figures of a Scholar Archive book). `prepare` uses the same
//! functions.
//!
//! For a digested book (Literature tree -CitationKey; its Scholar Archive Markdown -FileId, or the
//! digest's -Staging folder, whose manifest names both):
//!   1. keeps every figure of the Markdown in ZET's artifact store and reads its labels
//!      (`2.C.1`, `IV`/`四`) from the caption next to the image;
//!   2. finds the agent Cards of this book's Literature tree that name a figure (`图 2.C.1`, `图2.C.1`,
//!      `Figure 2.C.1`; never `图 2.C.10`);
//!   3. puts each figure's `![图 2.C.1](artifact:<sha256>)` line into the Card, as its own
//!      paragraph after the paragraph that first names it;
//!   4. adds `原书文档：[原书 PDF](scholar:…) · …` to the Literature Card when it has no Document link.
//!
//! The Scholar Archive: `zt scholar keep-figures`, `zt scholar sql`, and `zt scholar file` run
//! once per call, as the script ran them (`zt` on PATH). No MCP tool keeps all of a document's
//! figures in one call, and each runs once per tool run, so the process costs little; the Card
//! reads and writes go through the tool's one `zt mcp`.

use crate::args::Args;
use crate::calls;
use crate::model::read_calls;
use crate::ps::{self, CiMap, eq, len16, s};
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;


const CHINESE: [&str; 21] = [
    "", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十", "十一", "十二", "十三", "十四", "十五", "十六", "十七", "十八", "十九", "二十",
];
const ROMAN: [&str; 21] = [
    "", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII", "XIII", "XIV", "XV", "XVI", "XVII", "XVIII", "XIX", "XX",
];

static FIGURE_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:FIGURE|Figure|Fig\.?|FIG\.?)\s+([0-9]+(?:[.\-][0-9A-Za-z]+)*|[IVXLC]+)\b|^\s*图\s*([0-9]+(?:[.\-][0-9A-Za-z]+)*|[一二三四五六七八九十]+|[IVXLC]+)").unwrap()
});
static IMAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*!\[[^\]]*\]\(([^)\s]+)").unwrap());
static QUERY_PART: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[?#]").unwrap());
static BLOCKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n").unwrap());
static DETAILS_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</details>").unwrap());
static DETAILS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^<details").unwrap());
static CHINESE_ONLY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[一二三四五六七八九十]+$").unwrap());
static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(```|~~~)").unwrap());
static ALT_OR_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)!\[[^\]]*\]\([^)]*\)").unwrap());
static WIKI_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\[\[[^\]]*\]\]").unwrap());
static SCHOLAR_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\]\(scholar:\d+").unwrap());
static ARCHIVE_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)[\\/]scholar-archive[\\/]files[\\/](\d+)[\\/]").unwrap());

/// One figure of a book, as `Get-BookFigures` (or a figures.json) gives it.
#[derive(Clone, Debug, Default)]
pub(crate) struct Figure {
    pub sha256: Value,
    pub path: Value,
    pub labels: Vec<String>,
    pub alt: String,
    pub line: String,
    /// Where the image first appears in the book's text (0, 1, …); an image the text never holds sorts last.
    pub position: usize,
}

/// `Get-FigureImageName`: the file name an image line refers to, or None for a line that is not
/// an image.
pub(crate) fn image_name(line: &str) -> Option<String> {
    let caps = IMAGE.captures(line)?;
    let target = QUERY_PART.split(&caps[1]).next().unwrap_or_default();
    let target = unescape_data_string(target);
    Some(target.rsplit(['\\', '/']).next().unwrap_or_default().to_string())
}

/// `[uri]::UnescapeDataString`: %XX runs decode as UTF-8; a run that is not UTF-8 stays as written.
fn unescape_data_string(text: &str) -> String {
    let bytes = text.as_bytes();
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            let ch = text[i..].chars().next().unwrap_or_default();
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        // A run of %XX escapes.
        let start = i;
        let mut run = Vec::new();
        while i + 2 < bytes.len() && bytes[i] == b'%' {
            match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(a), Some(b)) => {
                    run.push(a * 16 + b);
                    i += 3;
                }
                _ => break,
            }
        }
        if run.is_empty() {
            out.push('%');
            i = start + 1;
            continue;
        }
        match String::from_utf8(run) {
            Ok(decoded) => out.push_str(&decoded),
            Err(_) => out.push_str(&text[start..i]),
        }
    }
    out
}

/// `Get-FigureLabels`: the labels a caption line gives, with the other numeral spelling of a
/// Roman or Chinese number.
pub(crate) fn labels(line: &str) -> Vec<String> {
    let Some(caps) = FIGURE_LABEL.captures(line) else { return Vec::new() };
    let label = caps.get(1).or_else(|| caps.get(2)).map(|m| m.as_str().to_string()).unwrap_or_default();
    let mut out = vec![label.clone()];
    let roman = ROMAN.iter().position(|r| *r == label).unwrap_or(0);
    let chinese = CHINESE.iter().position(|c| *c == label).unwrap_or(0);
    if roman > 0 {
        out.push(CHINESE[roman].to_string());
    }
    if chinese > 0 {
        out.push(ROMAN[chinese].to_string());
    }
    out
}

/// `Get-FigureMentionPattern`: finds a mention of one label in Card text (`图 2.C.1`, `图2.C.1`,
/// `Figure 2.C.1`), never a longer number (`图 2.C.10`). The script's pattern ended in a negative
/// lookahead `(?!\.?[0-9A-Za-z一二三四五六七八九十])`, which this checks by hand; `-match`
/// ignored case.
#[derive(Clone)]
pub(crate) struct Mention(Regex);

impl Mention {
    pub fn new(label: &str) -> Self {
        Self(Regex::new(&format!(r"(?i)(?:图|Figure|Fig\.)\s*{}", regex::escape(label))).unwrap())
    }

    pub fn is_in(&self, text: &str) -> bool {
        let longer = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || "一二三四五六七八九十".contains(c));
        let mut from = 0;
        while from <= text.len() {
            let Some(m) = self.0.find_at(text, from) else { return false };
            let mut rest = text[m.end()..].chars();
            let first = rest.next();
            let blocked = longer(first) || (first == Some('.') && longer(rest.next()));
            if !blocked {
                return true;
            }
            from = m.start() + text[m.start()..].chars().next().map_or(1, char::len_utf8);
        }
        false
    }
}

/// `zt scholar keep-figures <file id> --min-bytes N --out <temp file>`: the figures kept, as JSON.
fn keep_figures(file_id: i64, min_bytes: i64) -> Result<Value> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let out = std::env::temp_dir().join(format!("zt-digest-figures-{}-{nanos}.tmp", std::process::id()));
    let out_text = out.display().to_string();
    let result = (|| -> Result<Value> {
        let (ok, said) = zt_scholar(&["keep-figures", &file_id.to_string(), "--min-bytes", &min_bytes.to_string(), "--out", &out_text])?;
        if !ok {
            bail!("zt scholar keep-figures {file_id} failed: {}", said.join(" "));
        }
        let raw = util::read_text(&out)?;
        ps::parse_json(&raw).with_context(|| format!("{out_text} is not JSON"))
    })();
    let _ = std::fs::remove_file(&out);
    result
}

/// `Get-BookFigures`: keeps every figure the book's Markdown shows (`zt scholar keep-figures`)
/// and reads each figure's labels from the text next to the image. Keyed by the image's file name
/// (case ignored, as the script's `@{}`).
pub(crate) fn book_figures(text: &str, file_id: i64, min_bytes: i64) -> Result<CiMap<Figure>> {
    let kept = keep_figures(file_id, min_bytes)?;
    figures_from_text(text, &kept)
}

/// The labels part of `Get-BookFigures`, on the answer of `zt scholar keep-figures`.
pub(crate) fn figures_from_text(text: &str, kept: &Value) -> Result<CiMap<Figure>> {
    let mut figures: CiMap<Figure> = CiMap::new();
    for figure in ps::each(&kept["figures"]) {
        figures.insert(
            &s(&figure["name"]),
            Figure {
                sha256: figure["sha256"].clone(),
                path: figure["path"].clone(),
                position: usize::MAX,
                ..Figure::default()
            },
        );
    }
    let text = text.replace("\r\n", "\n");
    let blocks: Vec<&str> = BLOCKS.split(&text).collect();
    let mut images_seen = 0usize;
    for i in 0..blocks.len() {
        let lines: Vec<&str> = blocks[i].split('\n').collect();
        for k in 0..lines.len() {
            let Some(name) = image_name(lines[k]) else { continue };
            if name.is_empty() || !figures.contains(&name) {
                continue;
            }
            // The caption: the rest of the image's own block, then the next short blocks.
            let mut nearby: Vec<String> = Vec::new();
            for line in &lines[k + 1..] {
                if !line.is_empty() && image_name(line).is_none() {
                    nearby.push(line.to_string());
                }
            }
            let mut in_details = false;
            for block in blocks.iter().take((i + 8).min(blocks.len())).skip(i + 1) {
                let block = block.trim();
                if in_details {
                    if DETAILS_END.is_match(block) {
                        in_details = false;
                    }
                    continue;
                }
                if DETAILS.is_match(block) {
                    if !DETAILS_END.is_match(block) {
                        in_details = true;
                    }
                    continue;
                }
                if block.starts_with('#') || image_name(block).is_some() || len16(block) > 400 {
                    break;
                }
                nearby.extend(block.split('\n').map(str::to_string));
            }
            let mut found: Vec<String> = Vec::new();
            for line in &nearby {
                for label in labels(line) {
                    found.push(label);
                }
            }
            // No caption below: a figure whose title is the heading above it (`# 图 1.1. …`, then its
            // text and panels) takes that heading's label. The nearest heading decides, however far up:
            // the image belongs to that section, and one that names no figure gives none
            // (ebrill2001modern: the second map of 图 1.1 sat more than eight blocks below its heading).
            if found.is_empty() {
                let above = lines[..k].iter().rev().copied().chain(blocks[..i].iter().rev().flat_map(|block| block.trim().split('\n').rev()));
                if let Some(heading) = above.map(str::trim).find(|line| line.starts_with('#')) {
                    found = labels(heading.trim_start_matches('#'));
                }
            }
            let mut figure = figures.get(&name).cloned().unwrap_or_default();
            figure.position = figure.position.min(images_seen);
            images_seen += 1;
            for label in found {
                if !figure.labels.contains(&label) {
                    figure.labels.push(label);
                }
            }
            figures.insert(&name, figure);
        }
    }
    let names = figures.keys();
    for name in names {
        let mut figure = figures.get(&name).cloned().unwrap_or_default();
        set_alt(&mut figure);
        figures.insert(&name, figure);
    }
    Ok(figures)
}

/// The alt text names each figure once: an Arabic or Roman label, else the Chinese one.
fn set_alt(figure: &mut Figure) {
    let mut shown: Vec<&String> = figure.labels.iter().filter(|l| !CHINESE_ONLY.is_match(l)).collect();
    if shown.is_empty() {
        shown = figure.labels.iter().collect();
    }
    figure.alt = if shown.is_empty() { String::new() } else { format!("图 {}", shown.iter().map(|l| l.as_str()).collect::<Vec<_>>().join("、")) };
    figure.line = format!("![{}](artifact:{})", figure.alt, s(&figure.sha256));
}

/// `zt scholar <args>`: whether it succeeded, and its output lines (stdout, then stderr), as
/// `$said = & zt scholar … 2>&1` saw them.
fn zt_scholar(args: &[&str]) -> Result<(bool, Vec<String>)> {
    let said = std::process::Command::new("zt").arg("scholar").args(args).output().context("zt did not start")?;
    let mut lines: Vec<String> = String::from_utf8_lossy(&said.stdout).lines().map(str::to_string).collect();
    if !said.status.success() {
        lines.extend(String::from_utf8_lossy(&said.stderr).lines().map(str::to_string));
    }
    Ok((said.status.success(), lines))
}

/// `Get-BookDocumentLine`: the Work's own documents as Document links, for a Literature Card:
/// `原书文档：[原书 PDF](scholar:85) · [中英对照 Markdown](scholar:83) · …`; '' when it has none.
/// Runs `zt scholar sql` once, as the script did.
pub(crate) fn document_line(file_id: i64) -> Result<String> {
    let sql = format!(
        "SELECT id, kind, chinese_bilingual FROM work_files WHERE work_id = (SELECT work_id FROM work_files WHERE id = {file_id}) \
         AND availability = 'present' AND kind IN ('pdf', 'markdown') AND shown = 1 ORDER BY kind DESC, chinese_bilingual DESC, id"
    );
    let (ok, said) = zt_scholar(&["sql", &sql])?;
    if !ok {
        bail!("zt scholar sql failed: {}", said.join(" "));
    }
    let answer = ps::parse_json(&said.join("\n")).context("zt scholar sql printed no JSON")?;
    let mut links = Vec::new();
    for row in ps::each(&answer["rows"]) {
        let label = if eq(&s(&row[1]), "pdf") {
            "原书 PDF"
        } else if row[2].as_f64() == Some(1.0) || (row[2].is_string() && eq(&s(&row[2]), "1")) {
            "中英对照 Markdown"
        } else {
            "原文 Markdown"
        };
        links.push(format!("[{label}](scholar:{})", s(&row[0])));
    }
    if links.is_empty() {
        return Ok(String::new());
    }
    Ok(format!("原书文档：{}", links.join(" · ")))
}

/// The cached copy of a Scholar Archive document (`zt scholar file <id>`): its path.
fn document_path(file_id: i64) -> Result<String> {
    let (ok, said) = zt_scholar(&["file", &file_id.to_string()])?;
    if !ok {
        bail!("zt scholar file {file_id} failed");
    }
    let copy = ps::parse_json(&said.join("\n")).with_context(|| format!("zt scholar file {file_id} failed"))?;
    Ok(s(&copy["path"]))
}

/// The Scholar Archive file id in a ZET cache path such as `...\scholar-archive\files\83\...`.
pub(crate) fn file_id_in_path(source: &str) -> Option<i64> {
    ARCHIVE_PATH.captures(source).and_then(|c| c[1].parse().ok())
}

// --- attach-figures ---------------------------------------------------------------------------------

struct Card {
    address: String,
    title: String,
    body: String,
    text: String,
}

/// The figures in the book's order (their first appearance in the text, then their path), and one entry per
/// label and image: a figure of several panels (图 1.1 as two maps) names every panel under its label, in order.
/// The attach step had kept one image per label, the first by file path (a hash), so later panels were lost.
fn attach_order(figures: &CiMap<Figure>) -> (Vec<(Figure, Vec<Mention>)>, Vec<(String, usize)>) {
    let mut ordered: Vec<(Figure, Vec<Mention>)> = figures
        .iter()
        .map(|(_, f)| {
            let patterns = f.labels.iter().map(|l| Mention::new(l)).collect();
            (f.clone(), patterns)
        })
        .collect();
    ps::ps_sort(&mut ordered, |a, b| a.0.position.cmp(&b.0.position).then_with(|| ps::culture_cmp(&s(&a.0.path), &s(&b.0.path))));
    let mut by_label: Vec<(String, usize)> = Vec::new();
    for (i, (figure, _)) in ordered.iter().enumerate() {
        for label in &figure.labels {
            by_label.push((label.clone(), i));
        }
    }
    (ordered, by_label)
}

/// `Get-MentionText`: image alt text and Link targets are not mentions.
fn mention_text(block: &str) -> String {
    let t = ALT_OR_LINK.replace_all(block, "");
    WIKI_LINK.replace_all(&t, "").into_owned()
}

/// `Add-FiguresToBody`: the Card's new body and the alt texts of the figures it gains, or None.
fn add_figures(card: &Card, by_label: &[(String, usize)], figures: &[(Figure, Vec<Mention>)]) -> Option<(String, Vec<String>)> {
    let body = card.body.trim();
    let blocks: Vec<&str> = BLOCKS.split(body).filter(|b| !b.trim().is_empty()).collect();
    // Whether a chart or code fence is still open after each block.
    let mut open = vec![false; blocks.len()];
    let mut inside = false;
    for (b, block) in blocks.iter().enumerate() {
        for line in block.split('\n') {
            if FENCE.is_match(line) {
                inside = !inside;
            }
        }
        open[b] = inside;
    }
    let mut after: HashMap<i64, Vec<String>> = HashMap::new();
    let mut added: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (_, at_figure) in by_label {
        let (figure, patterns) = &figures[*at_figure];
        let sha = s(&figure.sha256);
        if !seen.insert(sha.clone()) {
            continue;
        }
        if body.contains(&format!("artifact:{sha}")) {
            continue;
        }
        let named = |t: &str| patterns.iter().any(|p| p.is_in(t));
        // A later panel of a figure the Card already shows goes right after the last earlier panel there.
        let earlier: Vec<String> = figures[..*at_figure]
            .iter()
            .filter(|(other, _)| other.labels.iter().any(|label| figure.labels.contains(label)))
            .map(|(other, _)| format!("artifact:{}", s(&other.sha256)))
            .collect();
        let mut at: Option<i64> = (0..blocks.len()).rev().find(|b| earlier.iter().any(|artifact| blocks[*b].contains(artifact.as_str()))).map(|b| b as i64);
        for b in 0..blocks.len() {
            if at.is_some() {
                break;
            }
            let open_before = b > 0 && open[b - 1];
            if !open_before && named(&mention_text(blocks[b])) {
                at = Some(b as i64);
                break;
            }
        }
        let mut at = match at {
            Some(at) => at,
            None => {
                if !named(&card.title) {
                    continue;
                }
                -1
            }
        };
        // A mention in a block that opens a fence goes after the fence closes.
        while at >= 0 && open[at as usize] && ((at + 1) as usize) < blocks.len() {
            at += 1;
        }
        after.entry(at).or_default().push(figure.line.clone());
        added.push(figure.alt.clone());
    }
    if added.is_empty() {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    if let Some(lines) = after.get(&-1) {
        out.extend(lines.iter().cloned());
    }
    for (b, block) in blocks.iter().enumerate() {
        out.push(block.to_string());
        if let Some(lines) = after.get(&(b as i64)) {
            out.extend(lines.iter().cloned());
        }
    }
    Some((out.join("\n\n"), added))
}

/// Writes through the helper semantics in one batch; a failure ends the run as `Invoke-Zt` did.
fn write_batch(zt: &mut Zt, edits: &[Value]) -> Result<()> {
    let (done, failure) = calls::run(zt, edits, &mut calls::Options::new())?;
    if let Some(failure) = failure {
        let mut lines: Vec<String> = done.into_iter().map(|d| d.line).collect();
        lines.extend(failure.lines());
        bail!("zt-mcp failed: {}", lines.join(" | "));
    }
    Ok(())
}

/// The figures a Card body shows (its artifact sha256 appears there), and the figures the digested text
/// never holds (when the run's digested text is known), each by sha256 in the figures' order.
fn figure_presence(figures: &CiMap<Figure>, bodies: &[String], digested: Option<&str>) -> (Vec<String>, Vec<String>) {
    let mut shown = Vec::new();
    let mut outside = Vec::new();
    for (_, figure) in figures.iter() {
        let sha = s(&figure.sha256);
        if sha.is_empty() {
            continue;
        }
        let artifact = format!("artifact:{sha}");
        if bodies.iter().any(|body| body.contains(&artifact)) {
            shown.push(sha);
        } else if digested.is_some_and(|text| !text.contains(&artifact)) {
            outside.push(sha);
        }
    }
    (shown, outside)
}

/// `zt-digest figures` (attach-figures.ps1).
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["CitationKey", "FileId", "Staging", "Source", "MinFigureBytes", "Report", "Batch", "FiguresFile"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &["NoDocuments", "Apply"])?;
    let key = a.required("CitationKey")?;
    if key.is_empty() {
        bail!("-CitationKey is required");
    }
    let mut file_id = a.int_or("FileId", 0)?;
    let staging = a.str("Staging");
    let mut source = a.str("Source");
    let min_bytes = a.int_or("MinFigureBytes", 3000)?;
    let report = a.str("Report");
    let batch = a.int_or("Batch", 40)?.max(1) as usize;
    let figures_file = a.str("FiguresFile");
    let no_documents = a.flag("NoDocuments");
    let apply = a.flag("Apply");

    if !staging.is_empty() {
        let manifest = ps::parse_json(&util::read_text(ps::join_path(&staging, "manifest.json"))?)?;
        if source.is_empty() {
            source = s(&manifest["source"]);
        }
        if file_id == 0 && ps::truthy(&manifest["file_id"]) {
            file_id = ps::int(&manifest["file_id"])?;
        }
    }
    if file_id == 0
        && let Some(id) = file_id_in_path(&source)
    {
        file_id = id;
    }
    let mut figures: CiMap<Figure> = CiMap::new();
    if !figures_file.is_empty() {
        let listed = ps::parse_json(&util::read_text(&figures_file)?)?;
        for (index, f) in ps::each(&listed["figures"]).into_iter().enumerate() {
            figures.insert(
                &s(&f["name"]),
                Figure {
                    sha256: f["sha256"].clone(),
                    path: f["path"].clone(),
                    labels: ps::ps_items(&f["labels"]).iter().map(s).collect(),
                    alt: s(&f["alt"]),
                    line: s(&f["line"]),
                    // A figures file keeps its list order unless it names each figure's place in the text.
                    position: f["position"].as_u64().map(|n| n as usize).unwrap_or(index),
                },
            );
        }
    } else {
        if file_id == 0 {
            bail!("give -FileId (the book Markdown's Scholar Archive file id), or a -Staging or -Source that names it");
        }
        if source.is_empty() || !ps::exists(&source) {
            source = document_path(file_id)?;
        }
        let text = util::read_text(&source)?;
        figures = book_figures(&text, file_id, min_bytes)?;
    }

    let mut zt = crate::zt::start()?;
    // R328 (§15.2): attaching figures edits the book's Cards. The plan (no -Apply) writes only its
    // report file; `-Apply` stops before the first Card when the book's Literature tree is locked.
    if apply
        && let Some(refusal) = crate::lockcheck::preflight(&mut zt, "figures", "apply", &key)?
    {
        return crate::lockcheck::stop("figures", &refusal);
    }
    let (ordered, by_label) = attach_order(&figures);

    // --- the Cards of this book that name a figure ---
    let mut queries: Vec<(&str, Value)> = Vec::new();
    let mut asked: HashSet<String> = HashSet::new();
    for (label, _) in &by_label {
        if !asked.insert(label.to_lowercase()) {
            continue;
        }
        for q in [format!("图 {label}"), format!("图{label}")] {
            queries.push(("search_cards", json!({ "query": q, "workspace": "agent", "limit": 200 })));
        }
    }
    let mut candidates: Vec<String> = Vec::new();
    let mut candidate_set: HashSet<String> = HashSet::new();
    for chunk in queries.chunks(60) {
        for found in read_calls(&mut zt, chunk)? {
            for hit in ps::each(&found["results"]) {
                let address = s(&hit["address"]);
                if candidate_set.insert(address.clone()) {
                    candidates.push(address);
                }
            }
        }
    }
    let anchor = Regex::new(&format!("(?i)^{}/\\d+$", regex::escape(&key)))?;
    let in_tree = Regex::new(&format!("(?i)^{}/", regex::escape(&key)))?;
    let addresses: Vec<&String> = candidates.iter().filter(|a| !eq(a, &key) && !anchor.is_match(a)).collect();
    let mut cards: Vec<Card> = Vec::new();
    for chunk in addresses.chunks(batch) {
        let reads: Vec<(&str, Value)> = chunk.iter().map(|a| ("get_card", json!({ "at": a }))).collect();
        for card in read_calls(&mut zt, &reads)? {
            if !eq(&s(&card["workspace"]), "agent") || eq(&s(&card["type"]), "hub") {
                continue;
            }
            let address = s(&card["address"]);
            let body = s(&card["body"]);
            // docs/literature-only-digest-design.md: the book's Cards are the Cards of its Literature
            // tree; a Card elsewhere that cites the book (a Topic Card, another book) is not its to edit.
            if !in_tree.is_match(&address) {
                continue;
            }
            cards.push(Card { address, title: s(&card["title"]), body, text: s(&card["text"]) });
        }
    }

    let mut plan: Vec<Value> = Vec::new();
    for card in &cards {
        if let Some((body, added)) = add_figures(card, &by_label, &ordered) {
            let mut entry = Map::new();
            entry.insert("address".into(), json!(card.address));
            entry.insert("title".into(), json!(card.title));
            entry.insert("figures".into(), json!(added));
            entry.insert("body".into(), json!(body));
            entry.insert("base".into(), json!(card.text));
            plan.push(Value::Object(entry));
        }
    }

    // --- the Literature Card's documents ---
    let documents = if no_documents || file_id == 0 { String::new() } else { document_line(file_id)? };
    let root = read_calls(&mut zt, &[("get_card", json!({ "at": key }))])?.into_iter().next().unwrap_or(Value::Null);
    let mut root_change = Value::Null;
    if !documents.is_empty() && !SCHOLAR_LINK.is_match(&s(&root["body"])) {
        root_change = json!({
            "address": key,
            "body": format!("{}\n\n{documents}", s(&root["body"]).trim_end()),
            "base": root["text"].clone(),
        });
    }

    if apply {
        let mut edits: Vec<Value> = plan
            .iter()
            .map(|p| json!({ "tool": "edit_card", "arguments": { "at": p["address"], "body": p["body"], "base": p["base"] } }))
            .collect();
        if !root_change.is_null() {
            edits.push(json!({ "tool": "edit_card", "arguments": { "at": root_change["address"], "body": root_change["body"], "base": root_change["base"] } }));
        }
        for chunk in edits.chunks(batch) {
            write_batch(&mut zt, chunk)?;
        }
    }

    // Which figures a Card of the book already shows (a Card the writer gave the image, or an earlier
    // attach), and which lie outside the digested range: the stage publication counts neither as a gap.
    // ebrill2001modern: 图 12.1 sat in 45|d and the two maps of 图 1.1 in 2|b, and the back cover's barcode
    // lies after the end heading, yet all four were reported unmatched.
    let tree = read_calls(&mut zt, &[("get_tree", json!({ "tree": key, "bodies": true }))])?.into_iter().next().unwrap_or(Value::Null);
    let mut bodies: Vec<String> = ps::each(&tree["cards"]).into_iter().map(|card| s(&card["body"])).collect();
    bodies.extend(plan.iter().map(|p| s(&p["body"])));
    let digested = if staging.is_empty() { None } else { util::read_text(ps::join_path(&staging, "book.zh.md")).ok() };
    let (already, outside) = figure_presence(&figures, &bodies, digested.as_deref());
    if !report.is_empty() {
        let value = json!({ "citation_key": key, "file_id": file_id, "cards": plan, "literature": root_change, "shown": already, "outside": outside });
        std::fs::write(&report, ps::json_pretty(&value)).with_context(|| format!("failed to write {report}"))?;
    }
    let labelled = figures.iter().filter(|(_, f)| !f.labels.is_empty()).count();
    let shown: usize = plan.iter().map(|p| ps::ps_count(&p["figures"])).sum();
    let sample: Vec<String> = plan
        .iter()
        .take(8)
        .map(|p| format!("{} <- {}", s(&p["address"]), ps::ps_items(&p["figures"]).iter().map(s).collect::<Vec<_>>().join(", ")))
        .collect();
    util::print_line(&ps::json_compact(&json!({
        "citation_key": key,
        "file_id": file_id,
        "applied": apply,
        "figures": figures.len(),
        "labelled": labelled,
        "candidates": candidates.len(),
        "book_cards": cards.len(),
        "cards": plan.len(),
        "shown": shown,
        "already_shown": already.len(),
        "outside": outside.len(),
        "documents": documents,
        "literature": !root_change.is_null(),
        "sample": sample,
    })))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_figure_is_shown_by_any_card_body_and_outside_when_the_digest_never_holds_it() {
        let mut figures: CiMap<Figure> = CiMap::new();
        for (name, sha) in [("map.jpg", "aa"), ("chart.jpg", "bb"), ("barcode.jpg", "cc"), ("new.jpg", "dd")] {
            figures.insert(name, Figure { sha256: json!(sha), ..Figure::default() });
        }
        let bodies = vec!["图 1.1 ![图 1.1](artifact:aa)".to_string(), "planned ![图 2](artifact:bb)".to_string()];
        let digested = "![](artifact:aa)\n![](artifact:bb)\n![](artifact:dd)";
        let (shown, outside) = figure_presence(&figures, &bodies, Some(digested));
        assert_eq!(shown, vec!["aa", "bb"]);
        assert_eq!(outside, vec!["cc"], "the barcode after the end heading is outside; the unshown in-range figure is not");
        let (_, unknown) = figure_presence(&figures, &bodies, None);
        assert!(unknown.is_empty(), "with no digested text nothing is called outside");
    }

    #[test]
    fn caption_labels() {
        // The assertions of attach-figures.tests.ps1 on figures.ps1.
        assert_eq!(labels("Figure 2.C.1 (left)\\"), vec!["2.C.1"]);
        assert_eq!(labels("FIGURE IV"), vec!["IV", "四"]);
        assert_eq!(labels("图四\\"), vec!["四", "IV"]);
        assert!(labels("The figure shows a curve.").is_empty());
        assert_eq!(labels("  Fig. 3-2 the curve"), vec!["3-2"]);
        // -cnotmatch: case counts.
        assert!(labels("figure 2").is_empty());
    }

    #[test]
    fn image_names() {
        assert_eq!(image_name("![](../Assets/G%20H/images/abc.jpg)\\").as_deref(), Some("abc.jpg"));
        assert_eq!(image_name("  ![x](a/b/%E5%9B%BE.png?x=1)").as_deref(), Some("图.png"));
        assert_eq!(image_name("text ![](a.png)"), None);
        assert_eq!(unescape_data_string("a%2"), "a%2");
        assert_eq!(unescape_data_string("%FF%41"), "%FF%41");
    }

    #[test]
    fn mentions_never_match_a_longer_number() {
        let p = Mention::new("2.C.1");
        assert!(p.is_in("如图 2.C.1 所示") && p.is_in("图2.C.1。") && p.is_in("see figure 2.c.1"));
        assert!(!p.is_in("图 2.C.10") && !p.is_in("图 2.C.1.a"));
        // A rejected mention does not hide a later one.
        assert!(p.is_in("图 2.C.10 与图 2.C.1"));
        let i = Mention::new("I");
        assert!(i.is_in("见图 I 中") && !i.is_in("图 II"));
        assert!(!Mention::new("四").is_in("图四十"));
    }

    #[test]
    fn labels_come_from_the_caption_next_to_the_image() {
        let kept = json!({ "figures": [
            { "name": "a.jpg", "sha256": "aa", "path": "B/images/a.jpg" },
            { "name": "B.JPG", "sha256": "bb", "path": "B/images/b.jpg" },
        ]});
        let text = "![](images/a.jpg)\nFigure 2.C.1 (left)\r\n\r\n<details>\n\n描述 FIGURE 9\n\n</details>\n\n图 2.C.2\n\n![](images/b.jpg)\n\n# 标题\n\n图 3";
        let figures = figures_from_text(text, &kept).unwrap();
        let a = figures.get("A.jpg").unwrap();
        assert_eq!(a.labels, vec!["2.C.1", "2.C.2"]);
        assert_eq!(a.line, "![图 2.C.1、2.C.2](artifact:aa)");
        let b = figures.get("b.jpg").unwrap();
        assert!(b.labels.is_empty());
        assert_eq!(b.line, "![](artifact:bb)");
    }

    /// ebrill2001modern: 图 1.1 has its title as a heading above its two map panels and no caption below
    /// them, so neither map was labelled and no Card showed them.
    #[test]
    fn a_figure_without_a_caption_takes_the_figure_heading_above_it() {
        let kept = json!({ "figures": [
            { "name": "m1.jpg", "sha256": "11", "path": "B/images/m1.jpg" },
            { "name": "m2.jpg", "sha256": "22", "path": "B/images/m2.jpg" },
            { "name": "c.jpg", "sha256": "33", "path": "B/images/c.jpg" },
        ]});
        let text = "# 图 1.1. 增值税的传播\n\n增值税这一税种在不到 50 年前首次引入。\n\n![](images/m1.jpg)\n\n<details>\n<summary>natural_image</summary>\n\n世界地图\n\n</details>\n\n本图由地图设计部制作。\n\n![](images/m2.jpg)\n\n无增值税\n\n# 第二章 设计\n\n正文。\n\n![](images/c.jpg)\n\n正文继续。";
        let figures = figures_from_text(text, &kept).unwrap();
        assert_eq!(figures.get("m1.jpg").unwrap().labels, vec!["1.1"]);
        assert_eq!(figures.get("m2.jpg").unwrap().labels, vec!["1.1"], "a later panel under the same heading");
        assert!(figures.get("c.jpg").unwrap().labels.is_empty(), "a heading that names no figure gives no label");
        // In the bilingual source the second map sits more than eight blocks below its heading (English and
        // Chinese text, an image description); the nearest heading still decides.
        let filler = (0..12).map(|n| format!("Paragraph {n}.\n\n段落 {n}。")).collect::<Vec<_>>().join("\n\n");
        let far = format!("# Figure 1.1. The Spread of the VAT\n\n# 图 1.1. 增值税的传播\n\n![](images/m1.jpg)\n\n{filler}\n\n![](images/m2.jpg)\n\n# 第二章\n\n![](images/c.jpg)");
        let figures = figures_from_text(&far, &kept).unwrap();
        assert_eq!(figures.get("m2.jpg").unwrap().labels, vec!["1.1"], "a panel far below its figure heading");
        assert!(figures.get("c.jpg").unwrap().labels.is_empty());
    }

    /// ebrill2001modern: 图 1.1 is two map panels under one heading. The attach step kept one image per label
    /// (the first by file path, a hash), so the second map was never shown.
    #[test]
    fn every_panel_of_a_figure_is_attached_in_source_order() {
        // The panels' file names sort the other way round from the book's order.
        let kept = json!({ "figures": [
            { "name": "zz-first.jpg", "sha256": "11", "path": "B/images/zz-first.jpg" },
            { "name": "aa-second.jpg", "sha256": "22", "path": "B/images/aa-second.jpg" },
            { "name": "chart.jpg", "sha256": "33", "path": "B/images/chart.jpg" },
        ]});
        let text = "# 图 1.1. 增值税的传播\n\n正文。\n\n![](images/zz-first.jpg)\n\n说明。\n\n![](images/aa-second.jpg)\n\n# 第二章\n\n![](images/chart.jpg)\n图 2.1 税率";
        let figures = figures_from_text(text, &kept).unwrap();
        let (ordered, by_label) = attach_order(&figures);
        let names: Vec<String> = by_label.iter().map(|(label, i)| format!("{label}:{}", s(&ordered[*i].0.sha256))).collect();
        assert_eq!(names, vec!["1.1:11", "1.1:22", "2.1:33"], "every image of a label, in the book's order");
        let card = Card { address: "K/2|b".into(), title: "增值税的传播".into(), body: "图 1.1 标出各国引入增值税的年代。\n\n来源：[[K]]".into(), text: String::new() };
        let (body, added) = add_figures(&card, &by_label, &ordered).unwrap();
        assert_eq!(added.len(), 2, "both panels are attached");
        let first = body.find("artifact:11").unwrap();
        let second = body.find("artifact:22").unwrap();
        assert!(body.find("图 1.1 标出").unwrap() < first && first < second && second < body.find("来源").unwrap(), "{body}");
        assert!(!body.contains("artifact:33"), "a figure the Card does not name is not attached");
        // A Card that already shows the first panel gains only the second.
        let shown = Card { body: "图 1.1 见下。\n\n![图 1.1](artifact:11)".into(), ..card };
        let (body, added) = add_figures(&shown, &by_label, &ordered).unwrap();
        assert_eq!(added.len(), 1);
        assert!(body.find("artifact:11").unwrap() < body.find("artifact:22").unwrap());
    }

    #[test]
    fn a_figure_follows_the_paragraph_or_fence_that_names_it() {
        let figure = Figure { sha256: json!("aa"), labels: vec!["2.C.1".into()], alt: "图 2.C.1".into(), line: "![图 2.C.1](artifact:aa)".into(), ..Figure::default() };
        let figures = vec![(figure.clone(), vec![Mention::new("2.C.1")])];
        let by_label = vec![("2.C.1".to_string(), 0)];
        let card = |title: &str, body: &str| Card { address: "x".into(), title: title.into(), body: body.into(), text: String::new() };
        let (body, added) = add_figures(&card("t", "图 2.C.1 画出。\n\n第二段。"), &by_label, &figures).unwrap();
        assert_eq!(body, "图 2.C.1 画出。\n\n![图 2.C.1](artifact:aa)\n\n第二段。");
        assert_eq!(added, vec!["图 2.C.1"]);
        let (body, _) = add_figures(&card("t", "```chart\n图 2.C.1\n\n```\n\n后文。"), &by_label, &figures).unwrap();
        assert_eq!(body, "```chart\n图 2.C.1\n\n```\n\n![图 2.C.1](artifact:aa)\n\n后文。");
        let (body, _) = add_figures(&card("图 2.C.1 的含义", "正文。"), &by_label, &figures).unwrap();
        assert_eq!(body, "![图 2.C.1](artifact:aa)\n\n正文。");
        assert!(add_figures(&card("t", "图 2.C.1 ![图 2.C.1](artifact:aa)"), &by_label, &figures).is_none());
        assert!(add_figures(&card("t", "见 [[图 2.C.1]]。"), &by_label, &figures).is_none());
    }
}
