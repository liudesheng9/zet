//! `zt-digest prepare` (the port of `scripts/book-digest/prepare-book-digest.ps1`) and
//! `zt-digest prepare-link` (the port of `prepare-link-pass.ps1`).
//!
//! prepare: reads a Chinese bilingual Markdown book and keeps the Chinese, keeps its figures in
//! ZET's artifact store (`figures::book_figures`), splits it at `#` and `##` headings from
//! -StartHeading to -EndHeading into units and groups, and writes to -Out: book.zh.md,
//! sections/sNN.md with sections/index.md, groups/gNN.md, figures.json, events.json,
//! shared-prefix.md and link-prefix.md (built from the instruction files of the scripts folder),
//! and manifest.json. -Apply creates the agent Literature tree with one anchor Card per group
//! (refused when the tree exists). Prints the manifest as compact JSON.
//!
//! prepare-link: reads manifest.json in -Staging and every tree; writes cards-all.md, and per
//! group link-gNN.md (the Cards it owns, with bodies and candidate targets) and allow-gNN.txt.
//! Prints `{pending, groups, unowned, cards_all}`.
//!
//! The instruction files come from ZT_DIGEST_SCRIPTS (the forwarder sets it to its own folder) or
//! the scripts\book-digest folder of the repository this binary was built from.

use crate::args::Args;
use crate::calls;
use crate::card::relatedness::Relatedness;
use crate::figures::{self, Figure};
use crate::ps::{self, CiMap, NL, culture_cmp, eq, is_in, join_path, key as ci, len16, s};
use crate::snapshot;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

static BLOCKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n").unwrap());
static CJK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}\x{F900}-\x{FAFF}]").unwrap());
static HAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}]").unwrap());
static KEEP_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(\$\$|\||<|```)").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(#{1,2})\s+(.+)$").unwrap());
static NOT_EVENT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(!\[|\||#|\d+\))").unwrap());
static CITATION_YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\?\[[^\]]*\d{4}[^\]]*\\?\]|[（(]\s*\d{4}[a-z]?\s*[)）]").unwrap());
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(1[5-9]\d\d|20[0-2]\d)\s*年").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static SECTION_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(Intro\.\d+|A\d+|\d+(\.\d+)?)\s").unwrap());
static CHAPTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^第(.)章").unwrap());
const SECTION: &str = r"(Intro\.\d+|A\d+|\d+(?:\.\d+)*|导论)";

/// The text of a UTF-8 file as `Get-Content -Raw -Encoding utf8` reads it (a BOM dropped).
fn read_raw(path: &str) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("Cannot find path '{path}' because it does not exist."))?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(text.strip_prefix('\u{FEFF}').map(str::to_string).unwrap_or(text))
}

fn write(path: &str, text: &str) -> Result<()> {
    std::fs::write(path, text).with_context(|| format!("failed to write {path}"))
}

/// Where the instruction files live.
fn scripts_dir() -> String {
    if let Ok(dir) = std::env::var("ZT_DIGEST_SCRIPTS")
        && !dir.is_empty()
    {
        return dir.trim_end_matches(['\\', '/']).to_string();
    }
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = crate_dir.parent().and_then(Path::parent).unwrap_or(crate_dir);
    repo.join("scripts").join("book-digest").to_string_lossy().into_owned()
}

/// A mandatory string parameter: given and not empty, as PowerShell's binding required.
fn mandatory(a: &Args, name: &str) -> Result<String> {
    let value = a.required(name)?;
    if value.is_empty() {
        bail!("Cannot bind argument to parameter '{name}' because it is an empty string.");
    }
    Ok(value)
}

/// `$text -match '(1[5-9]\d\d|20[0-2]\d)\s*年(?!代)'`.
fn names_a_year(text: &str) -> bool {
    let mut from = 0;
    while let Some(m) = YEAR.find_at(text, from) {
        if !text[m.end()..].starts_with('代') {
            return true;
        }
        from = m.start() + 1;
    }
    false
}

// --- the Chinese text and its units --------------------------------------------------------------------

/// The Chinese-only blocks: a block without CJK characters is dropped when the next block is its
/// translation; formulas, tables, and unpaired blocks stay. A kept figure's image line becomes its
/// `artifact:` line, once (its bilingual copy in the next blocks is dropped); other images go.
fn chinese_blocks(raw: &str, figures: &CiMap<Figure>) -> Vec<String> {
    let blocks: Vec<&str> = BLOCKS.split(raw).map(str::trim).filter(|b| !b.is_empty()).collect();
    let mut kept: Vec<String> = Vec::new();
    // Where each figure was last written: the bilingual copy right after it is not written again.
    let mut figure_at: HashMap<String, usize> = HashMap::new();
    for i in 0..blocks.len() {
        let mut lines: Vec<String> = Vec::new();
        let mut shown = false;
        for line in blocks[i].split('\n') {
            let Some(name) = figures::image_name(line).filter(|n| !n.is_empty()) else {
                lines.push(line.to_string());
                continue;
            };
            let Some(figure) = figures.get(&name) else { continue };
            if let Some(&at) = figure_at.get(&ci(&name))
                && kept.len() - at <= 3
            {
                continue;
            }
            figure_at.insert(ci(&name), kept.len());
            lines.push(figure.line.clone());
            shown = true;
        }
        let block = lines.join("\n").trim().to_string();
        if block.is_empty() {
            continue;
        }
        if shown || CJK.is_match(&block) || KEEP_BLOCK.is_match(&block) {
            kept.push(block);
            continue;
        }
        let next = blocks.get(i + 1).copied().unwrap_or_default();
        if CJK.is_match(next) {
            continue;
        }
        kept.push(block);
    }
    kept
}

#[derive(Clone, Debug)]
struct Unit {
    chapter: String,
    heading: String,
    headings: Vec<String>,
    text: String,
}

/// Units at `#` and `##` headings from -StartHeading up to -EndHeading; a unit under a heading of
/// `skip` is left out up to the next other heading.
fn split_units(kept: &[String], start: &str, end: &str, skip: &[String]) -> Vec<Unit> {
    let mut units: Vec<Unit> = Vec::new();
    let mut chapter = String::new();
    let mut current: Option<usize> = None;
    let mut inside = false;
    for block in kept {
        if !block.contains('\n')
            && let Some(caps) = HEADING.captures(block)
        {
            let level = caps[1].len();
            let heading = caps[2].trim().to_string();
            if !inside && eq(&heading, start) {
                inside = true;
            }
            if inside && eq(&heading, end) {
                break;
            }
            if !inside {
                continue;
            }
            if is_in(&heading, skip) {
                current = None;
                continue;
            }
            if level == 1 {
                chapter = heading.clone();
            }
            units.push(Unit { chapter: chapter.clone(), heading, headings: Vec::new(), text: format!("{block}{NL}{NL}") });
            current = Some(units.len() - 1);
            continue;
        }
        if inside && let Some(c) = current {
            units[c].text.push_str(&format!("{block}{NL}{NL}"));
        }
    }
    units
}

/// Units shorter than `min` characters (part titles, chapter lead-ins) go into the next unit.
fn merge_units(units: Vec<Unit>, min: i64) -> Result<Vec<Unit>> {
    let mut merged: Vec<Unit> = Vec::new();
    let mut carry: Option<Unit> = None;
    for mut unit in units {
        match carry.take() {
            Some(c) => {
                unit.text = format!("{}{}", c.text, unit.text);
                let mut headings = c.headings;
                headings.push(unit.heading.clone());
                unit.headings = headings;
            }
            None => unit.headings = vec![unit.heading.clone()],
        }
        if (len16(&unit.text) as i64) < min {
            carry = Some(unit);
            continue;
        }
        merged.push(unit);
    }
    if let Some(c) = carry {
        match merged.last_mut() {
            Some(last) => last.text.push_str(&c.text),
            None => bail!("Index was out of range. Must be non-negative and less than the size of the collection. (Parameter 'index')"),
        }
    }
    Ok(merged)
}

/// Groups of up to `max_units` consecutive units of one chapter, at most `max_chars` characters
/// unless one unit is larger: indexes into `merged`.
fn group_units(merged: &[Unit], max_units: i64, max_chars: i64) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < merged.len() {
        let mut members = vec![i];
        let mut size = len16(&merged[i].text) as i64;
        while (members.len() as i64) < max_units
            && i + 1 < merged.len()
            && eq(&merged[i + 1].chapter, &merged[i].chapter)
            && size + len16(&merged[i + 1].text) as i64 <= max_chars
        {
            i += 1;
            members.push(i);
            size += len16(&merged[i].text) as i64;
        }
        groups.push(members);
        i += 1;
    }
    groups
}

/// The event candidates of one unit: its lines that name a year, not a citation's year.
fn event_candidates(unit: &Unit, file: &str, n: usize, max: i64, into: &mut Vec<Value>) {
    let mut line_no = 0;
    for line in unit.text.split('\n') {
        line_no += 1;
        if into.len() as i64 >= max {
            break;
        }
        if !HAN.is_match(line) || NOT_EVENT_LINE.is_match(line) {
            continue;
        }
        // A citation's year (`Helper [1991]`, `Holmstrom (1979)`) names a paper, not an event.
        let plain = CITATION_YEAR.replace_all(line, "");
        if names_a_year(&plain) {
            let mut clip = SPACES.replace_all(line, " ").trim().to_string();
            if len16(&clip) > 120 {
                clip = format!("{}…", ps::cut16(&clip, 120));
            }
            into.push(json!({ "group": n, "src": format!("sections/{file}:{line_no}"), "text": clip }));
        }
    }
}

/// The shared prefix: the first instruction file, each further file inserted before its
/// `## 回复` section, the placeholders filled.
fn build_prefix(dir: &str, files: &str, fill: &[(&str, &str)]) -> Result<String> {
    let names: Vec<&str> = files.split(',').map(str::trim).filter(|f| !f.is_empty()).collect();
    let first = names.first().context("no instruction file")?;
    let mut template = read_raw(&join_path(dir, first))?;
    for extra in names.iter().skip(1) {
        let part = format!("{}\n\n", read_raw(&join_path(dir, extra))?.trim_end());
        template = match template.find("\n## 回复") {
            Some(at) => format!("{}{}{}", &template[..at + 1], part, &template[at + 1..]),
            None => format!("{}\n\n{}", template.trim_end(), part),
        };
    }
    for (from, to) in fill {
        template = template.replace(from, to);
    }
    Ok(template)
}

/// `zt-digest prepare` (prepare-book-digest.ps1).
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = [
        "Source",
        "Out",
        "CitationKey",
        "BibTeX",
        "BookTitle",
        "StartHeading",
        "EndHeading",
        "SkipHeadings",
        "MaxChars",
        "MaxUnits",
        "MinUnitChars",
        "FileId",
        "MinFigureBytes",
        "MaxEventCandidates",
        "Instructions",
    ];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &["NoEventCandidates", "NoFigures", "Apply"])?;
    let source = mandatory(&a, "Source")?;
    let out = mandatory(&a, "Out")?;
    let key = mandatory(&a, "CitationKey")?;
    let bibtex = mandatory(&a, "BibTeX")?;
    let title = a.str("BookTitle");
    let start = a.str_or("StartHeading", "导论");
    let end = a.str_or("EndHeading", "参考");
    let skip_headings = a.str("SkipHeadings");
    let max_chars = a.int_or("MaxChars", 20000)?;
    let max_units = a.int_or("MaxUnits", 3)?;
    let min_unit_chars = a.int_or("MinUnitChars", 1000)?;
    let mut file_id = a.int_or("FileId", 0)?;
    let min_figure_bytes = a.int_or("MinFigureBytes", 3000)?;
    let max_event_candidates = a.int_or("MaxEventCandidates", 12)?;
    let instructions = a.str_or("Instructions", "dsh-instructions.zh.md");
    let no_event_candidates = a.flag("NoEventCandidates");
    let no_figures = a.flag("NoFigures");
    let apply = a.flag("Apply");
    let scripts = scripts_dir();

    // --- Chinese-only blocks ---
    let raw = read_raw(&source)?.replace("\r\n", "\n");
    if file_id == 0
        && let Some(id) = figures::file_id_in_path(&source)
    {
        file_id = id;
    }
    let mut figure_map: CiMap<Figure> = CiMap::new();
    if file_id != 0 && !no_figures {
        figure_map = figures::book_figures(&raw, file_id, min_figure_bytes)?;
    }
    let kept = chinese_blocks(&raw, &figure_map);

    // --- units and groups ---
    let skip: Vec<String> = skip_headings.split(',').map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect();
    let units = split_units(&kept, &start, &end, &skip);
    if units.is_empty() {
        bail!("no heading \"{start}\" found in {source}");
    }
    let merged = merge_units(units, min_unit_chars)?;
    let groups = group_units(&merged, max_units, max_chars);

    for dir in ["groups", "sections"] {
        let path = join_path(&out, dir);
        std::fs::create_dir_all(&path).with_context(|| format!("failed to make {path}"))?;
        for entry in std::fs::read_dir(&path)?.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if entry.file_type().is_ok_and(|t| t.is_file()) && name.ends_with(".md") {
                std::fs::remove_file(entry.path())?;
            }
        }
    }
    let staging = ps::full_name(&out);

    // The book stays on disk: one file per unit plus an index.
    let mut book = format!("# 全书正文（中文译本，{start} 至 {end} 之前）{NL}{NL}");
    let unit_file: Vec<String> = (0..merged.len()).map(|u| format!("s{:02}.md", u + 1)).collect();
    for (u, unit) in merged.iter().enumerate() {
        write(&join_path(&staging, &format!("sections\\{}", unit_file[u])), &unit.text)?;
        book.push_str(&unit.text);
    }
    write(&join_path(&staging, "book.zh.md"), &book)?;

    // Which other units each group should look at first, so its run needs fewer searches.
    let mut unit_group: HashMap<String, usize> = HashMap::new();
    for (g, members) in groups.iter().enumerate() {
        for &u in members {
            unit_group.insert(ci(&unit_file[u]), g + 1);
        }
    }
    let documents: Vec<(String, String)> = merged.iter().enumerate().map(|(u, unit)| (unit_file[u].clone(), unit.text.clone())).collect();
    let section_model = Relatedness::new(&documents, 150);

    let documents_line = if file_id != 0 { figures::document_line(file_id)? } else { String::new() };
    let index_path = join_path(&staging, "sections\\index.md");
    let mut manifest = Map::new();
    manifest.insert("citation_key".into(), json!(key));
    manifest.insert("book_title".into(), json!(title));
    manifest.insert("staging".into(), json!(staging));
    manifest.insert("shared_prefix".into(), json!(join_path(&staging, "shared-prefix.md")));
    manifest.insert("link_prefix".into(), json!(join_path(&staging, "link-prefix.md")));
    manifest.insert("book".into(), json!(join_path(&staging, "book.zh.md")));
    manifest.insert("index".into(), json!(index_path));
    manifest.insert("source".into(), json!(source));
    manifest.insert("groups".into(), json!([]));
    manifest.insert("file_id".into(), json!(file_id));
    manifest.insert("figures".into(), json!(figure_map.len()));
    manifest.insert("figures_file".into(), json!(join_path(&staging, "figures.json")));
    manifest.insert("documents".into(), json!(documents_line));

    // Every kept figure with its labels: the model compiler hands a part the figure its plan names.
    let mut listed: Vec<(String, Figure)> = figure_map.iter().map(|(k, f)| (k.clone(), f.clone())).collect();
    ps::ps_sort(&mut listed, |a, b| culture_cmp(&s(&a.1.path), &s(&b.1.path)));
    let figure_list: Vec<Value> = listed
        .iter()
        .map(|(name, f)| json!({ "name": name, "labels": f.labels, "alt": f.alt, "line": f.line, "sha256": f.sha256, "path": f.path }))
        .collect();
    write(&join_path(&staging, "figures.json"), &ps::json_pretty(&json!({ "file_id": file_id, "figures": figure_list })))?;

    let mut outline = String::new();
    let mut all_candidates: Vec<Value> = Vec::new();
    let mut index = String::new();
    index.push_str(&format!("# 全书索引：《{title}》{NL}{NL}"));
    index.push_str(&format!("每个小节文件位于 `{staging}\\sections\\`；全书合并文本为 `{staging}\\book.zh.md`；双语原文（只读）为 `{source}`。{NL}{NL}"));
    index.push_str(&format!("| 文件 | 字数 | 组 | 锚点卡片 | 章 | 小节 |{NL}|---|---|---|---|---|---|{NL}"));
    let mut group_entries: Vec<Value> = Vec::new();
    for (g, members) in groups.iter().enumerate() {
        let n = g + 1;
        let id = format!("g{n:02}");
        let headings: Vec<String> = members.iter().flat_map(|&u| merged[u].headings.clone()).collect();
        let text: String = members.iter().map(|&u| merged[u].text.as_str()).collect();
        let file = join_path(&staging, &format!("groups\\{id}.md"));
        let anchor = format!("{key}/{n}");
        let owned = join_path(&staging, &format!("owned-{id}.txt"));
        let sections: Vec<String> = members.iter().map(|&u| unit_file[u].clone()).collect();
        let chapter = merged[members[0]].chapter.clone();
        for &u in members {
            index.push_str(&format!(
                "| `{}` | {} | {n} | `{anchor}` | {} | {} |{NL}",
                unit_file[u],
                len16(&merged[u].text),
                merged[u].chapter,
                merged[u].headings.join("；")
            ));
        }
        let mut found: Vec<String> = Vec::new();
        for section in &sections {
            found.extend(section_model.find(section, 3, |other| unit_group.get(&ci(other)) == Some(&n)));
        }
        let related: Vec<String> = ps::select_unique(found)
            .into_iter()
            .take(4)
            .map(|f| {
                let u = unit_file.iter().position(|x| eq(x, &f)).unwrap_or(0);
                let in_group = unit_group.get(&ci(&f)).copied().unwrap_or(0);
                format!("`{f}`（第 {in_group} 组，锚点 `{key}/{in_group}`）{}", merged[u].headings.join("；"))
            })
            .collect();
        // Event candidates: the lines of this group's sections that name a year. The run decides
        // which are real events; a hypothetical example in a model is none.
        let mut candidates: Vec<Value> = Vec::new();
        for &u in members {
            event_candidates(&merged[u], &unit_file[u], n, max_event_candidates, &mut candidates);
        }
        let event_lines = if candidates.is_empty() {
            "  - （没有）".to_string()
        } else {
            candidates.iter().map(|c| format!("  - `{}` {}", s(&c["src"]), s(&c["text"]))).collect::<Vec<_>>().join("\n")
        };
        all_candidates.extend(candidates);
        let src_log = join_path(&staging, &format!("src-{id}.txt"));
        let gate = join_path(&staging, "topic-gate.lock");
        let events = if no_event_candidates { String::new() } else { format!("- 事件候选（本组小节中写出年份的句子；由你判断哪些是真实事件）：\n{event_lines}\n") };
        // The script's here-string: its lines end as the script file's did (CRLF).
        let head = [
            format!("# 你的组：第 {n} 组"),
            String::new(),
            format!("- 锚点卡片：`{anchor}`"),
            format!("- 章节：{chapter}；小节：{}（文件 {}，原文附在下面）", headings.join("；"), sections.join("、")),
            format!("- 用词最接近的其他小节（先看这些）：{}", related.join("；")),
            format!("- 写入命令：`... | zt-digest call - -Log \"{owned}\" -SrcLog \"{src_log}\" -TopicGate \"{gate}\"`"),
            format!("- SELF-CHECK: `zt-check --cards \"{anchor}\" --log \"{owned}\" --book {key}`"),
            format!("- 现有主题卡片快照：`{staging}\\snapshot-{id}.md`（本组启动时生成）"),
            format!("- 全书索引：`{index_path}`"),
            format!("- 本书术语表：`{staging}\\terms.md`（存在时先读它，按它用词）"),
            events,
            "## 你的小节原文".to_string(),
            String::new(),
        ]
        .join(NL);
        write(&file, &format!("{head}{text}"))?;
        outline.push_str(&format!("- 第 {n} 组 `{anchor}`：{chapter} | {}{NL}", headings.join("；")));
        group_entries.push(json!({
            "n": n, "id": id, "anchor": anchor, "chapter": chapter, "headings": headings,
            "sections": sections, "chars": len16(&text), "file": file,
        }));
    }
    manifest.insert("groups".into(), Value::Array(group_entries));
    write(&index_path, &index)?;
    write(&join_path(&staging, "events.json"), &ps::json_pretty(&Value::Array(all_candidates.clone())))?;
    manifest.insert("event_candidates".into(), json!(all_candidates.len()));

    let outline = outline.trim_end().to_string();
    for (files, target) in [(instructions.as_str(), "shared_prefix"), ("dsh-link-instructions.zh.md", "link_prefix")] {
        let prefix = build_prefix(
            &scripts,
            files,
            &[
                ("{{KEY}}", &key),
                ("{{TITLE}}", &title),
                ("{{HELPER}}", "zt-digest call"),
                ("{{STAGING}}", &staging),
                ("{{INDEX}}", &index_path),
                ("{{SOURCE}}", &source),
                ("{{OUTLINE}}", &outline),
            ],
        )?;
        write(&s(&manifest[target]), &prefix)?;
    }

    // --- agent Literature tree ---
    if apply {
        let mut zt = Zt::start()?;
        let z = &mut zt;
        let probe = calls::run(z,&[json!({ "tool": "get_card", "arguments": { "at": key } })], &mut calls::Options::new())?;
        if probe.1.is_none() {
            bail!("Literature tree {key} already exists; delete it or choose another key.");
        }
        let body = if documents_line.is_empty() { "（全书索引待整理。）".to_string() } else { format!("（全书索引待整理。）\n\n{documents_line}") };
        let mut setup = vec![json!({ "tool": "create_literature", "arguments": { "workspace": "agent", "bibtex": bibtex, "body": body } })];
        for group in ps::each(&manifest["groups"]) {
            let n = group["n"].as_u64().unwrap_or(0);
            let at = if n == 1 { key.clone() } else { format!("{key}/{}", n - 1) };
            let headings: Vec<String> = ps::each(&group["headings"]).iter().map(|h| s(h)).collect();
            setup.push(json!({ "tool": "create_successor", "arguments": {
                "at": at, "kind": "direct",
                "title": format!("第 {n} 组：{}", headings.join("；")),
                "body": format!("本卡对应原书 {} 中的：{}。负责该组的代理将把它改写为这一部分的核心论点与小节索引。", s(&group["chapter"]), headings.join("；")),
            } }));
        }
        write(&join_path(&staging, "setup-calls.json"), &ps::json_pretty(&Value::Array(setup.clone())))?;
        let (done, failure) = calls::run(z, &setup, &mut calls::Options::new())?;
        let mut lines: Vec<String> = done.into_iter().map(|d| d.line).collect();
        if let Some(failure) = failure {
            lines.extend(failure.lines());
            bail!("zt setup failed: {}", lines.join(" | "));
        }
        manifest.insert("created".into(), json!(lines));
    }

    let manifest = Value::Object(manifest);
    write(&join_path(&staging, "manifest.json"), &ps::json_pretty(&manifest))?;
    util::print_line(&ps::json_compact(&manifest))?;
    Ok(0)
}

// --- prepare-link ---------------------------------------------------------------------------------

/// A hashtable keyed by address (`@{}`: case ignored).
struct ByAddress<V>(HashMap<String, V>);

impl<V> ByAddress<V> {
    fn new() -> Self {
        Self(HashMap::new())
    }
    fn get(&self, k: &str) -> Option<&V> {
        self.0.get(&ci(k))
    }
    fn has(&self, k: &str) -> bool {
        self.0.contains_key(&ci(k))
    }
    fn set(&mut self, k: &str, v: V) {
        self.0.insert(ci(k), v);
    }
}

/// `Group-OfSection`: the group of a section id (`2.3.1` falls back to `2.3`, then `2`).
fn group_of_section(section_group: &CiMap<i64>, id: &str) -> Option<i64> {
    if let Some(&n) = section_group.get(id) {
        return Some(n);
    }
    let parts: Vec<&str> = id.split('.').collect();
    if parts.len() > 2
        && let Some(&n) = section_group.get(&format!("{}.{}", parts[0], parts[1]))
    {
        return Some(n);
    }
    if ps::like(id, "Intro*")
        && let Some(&n) = section_group.get("导论")
    {
        return Some(n);
    }
    section_group.get(parts[0]).copied()
}

/// The section id a heading names (`2.3 …`, `A1 …`, `Intro.2 …`, `第三章…`, `导论`).
fn section_id(heading: &str) -> Option<String> {
    if let Some(c) = SECTION_ID.captures(heading) {
        return Some(c[1].to_string());
    }
    if let Some(c) = CHAPTER.captures(heading) {
        let numerals = ["一", "二", "三", "四", "五", "六", "七", "八", "九"];
        return numerals.iter().position(|x| *x == &c[1]).map(|i| (i + 1).to_string());
    }
    if eq(heading, "导论") { Some("导论".to_string()) } else { None }
}

/// `zt-digest prepare-link -Staging <folder> [-TitleChars 40]` (prepare-link-pass.ps1).
pub fn link_cli(argv: &[String]) -> Result<i32> {
    let params = ["Staging", "TitleChars"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &[])?;
    let staging = mandatory(&a, "Staging")?;
    let title_chars = a.int_or("TitleChars", 40)?.max(0) as usize;
    let manifest = ps::parse_json(&read_raw(&join_path(&staging, "manifest.json"))?)?;
    let key = s(&manifest["citation_key"]);
    let groups = ps::each(&manifest["groups"]);

    // --- every tree, both Workspaces ---
    let mut zt = Zt::start()?;
    let details = snapshot::read_title_trees(&mut zt)?;
    let mut text = String::new();
    text.push_str("# 全部卡片快照（地址与标题；[模型] 为模型卡，[模型·假设] 等为模型分卡，[事件] 为事件卡）");
    text.push_str(NL);
    text.push_str(NL);
    for workspace in ["agent", "human"] {
        text.push_str(if workspace == "agent" { "## 代理工作区" } else { "## 人类工作区（只读，可链接）" });
        text.push_str(NL);
        let mut trees: Vec<&Value> = details.iter().filter(|t| eq(&s(&t["workspace"]), workspace)).collect();
        ps::ps_sort(&mut trees, |a, b| culture_cmp(&s(&a["zone"]), &s(&b["zone"])).then_with(|| culture_cmp(&s(&a["tree"]), &s(&b["tree"]))));
        for tree in trees {
            for card in snapshot::sorted_cards(tree) {
                text.push_str(&snapshot::card_line(&card, title_chars));
                text.push_str(NL);
            }
        }
        text.push_str(NL);
    }
    let cards_all = join_path(&staging, "cards-all.md");
    write(&cards_all, &text)?;

    // --- section id -> group ---
    let mut section_group: CiMap<i64> = CiMap::new();
    for group in &groups {
        for heading in ps::each(&group["headings"]) {
            if let Some(id) = section_id(&s(heading))
                && !section_group.contains(&id)
            {
                section_group.insert(&id, ps::int(&group["n"])?);
            }
        }
    }

    // --- ownership ---
    let agent_cards: Vec<Value> = details
        .iter()
        .filter(|t| eq(&s(&t["workspace"]), "agent"))
        .flat_map(|t| ps::ps_items(&t["cards"]))
        .filter(|c| !eq(&s(&c["kind"]), "topic"))
        .collect();
    // Ranking uses complete agent bodies, but the previous compact excerpts
    // for human Cards and Topic roots. All are bulk inputs to the same process.
    let addresses: Vec<String> = details.iter().flat_map(|tree|ps::ps_items(&tree["cards"]))
        .map(|c|s(&c["address"])).collect();
    let read=zt.cards(&addresses,"text")?;
    let title_revision=details.first().and_then(|tree|tree["archive_revision"].as_u64());
    if title_revision!=Some(read.archive_revision) {bail!("incomplete prepare-link input: changing view between titles and Card bodies");}
    let mut bodies: ByAddress<Value> = ByAddress::new();
    for card in read.cards {
        bodies.set(&s(&card["address"]),card);
    }
    if bodies.0.len() != addresses.len() {
        bail!("read {} of {} required Cards", bodies.0.len(), addresses.len());
    }
    let mut owner: ByAddress<i64> = ByAddress::new();
    let mut owner_keys: Vec<String> = Vec::new();
    let mut set_owner = |owner: &mut ByAddress<i64>, address: &str, n: i64| {
        if !owner.has(address) {
            owner_keys.push(address.to_string());
        }
        owner.set(address, n);
    };
    let mut logged: HashSet<i64> = HashSet::new();
    for group in &groups {
        let n = ps::int(&group["n"])?;
        let log = join_path(&staging, &format!("owned-{}.txt", s(&group["id"])));
        if Path::new(&log).exists() {
            logged.insert(n);
            for line in ps::read_lines(&log)? {
                let line = line.trim();
                if !line.is_empty() && bodies.has(line) {
                    set_owner(&mut owner, line, n);
                }
            }
        }
        set_owner(&mut owner, &s(&group["anchor"]), n);
    }
    let in_anchor = Regex::new(&format!(r"(?i)^{}/(\d+)\|", regex::escape(&key)))?;
    let cites_section = Regex::new(&format!(r"(?i)来源[：:]\s*\[\[{}\]\]\s*§?\s*{SECTION}", regex::escape(&key)))?;
    let names_section = Regex::new(&format!(r"(?i)§\s*{SECTION}"))?;
    let source_mark = Regex::new(r"(?i)§|来源")?;
    for card in &agent_cards {
        let address = s(&card["address"]);
        if owner.has(&address) {
            continue;
        }
        let full = bodies.get(&address).cloned().unwrap_or(Value::Null);
        let body = s(&full["body"]);
        // Hub Cards belong to the hub stage, and structure Cards (long lists of Links) to the
        // coordinator, not to a group.
        if eq(&s(&full["type"]), "hub") {
            continue;
        }
        let outbound=match full["outbound_count"].as_u64() {
            Some(count)=>count,
            None=>bail!("incomplete prepare-link input: missing outbound count for `{address}`"),
        };
        if outbound >= 8 {
            continue;
        }
        let mut n: Option<i64> = None;
        if let Some(c) = in_anchor.captures(&address) {
            let anchor_group = ps::int(&json!(c[1].to_string()))?;
            if source_mark.is_match(&body) {
                n = Some(anchor_group);
            }
        } else if let Some(c) = cites_section.captures(&body) {
            n = group_of_section(&section_group, &c[1]);
        } else if let Some(c) = names_section.captures(&body) {
            n = group_of_section(&section_group, &c[1]);
        }
        if let Some(n) = n
            && n != 0
            && !logged.contains(&n)
        {
            set_owner(&mut owner, &address, n);
        }
    }

    // --- candidate targets: the most similar Cards outside the owner's group ---
    let mut titles: ByAddress<String> = ByAddress::new();
    let mut cards_by_address: ByAddress<Value> = ByAddress::new();
    let mut documents: Vec<(String, String)> = Vec::new();
    for tree in &details {
        for card in ps::ps_items(&tree["cards"]) {
            let address = s(&card["address"]);
            let title = s(&card["title"]);
            titles.set(&address, title.clone());
            let full=bodies.get(&address).with_context(||format!("incomplete candidate input: missing `{address}`"))?;
            let content=if eq(&s(&card["workspace"]),"agent") && !eq(&s(&card["kind"]),"topic") {s(&full["body"])} else {s(&full["excerpt"])};
            let text=format!("{title} {title} {content}");
            cards_by_address.set(&address, card);
            documents.push((address, text));
        }
    }
    let card_model = Relatedness::new(&documents, 80);
    let candidates_of = |address: &str, n: i64| -> Vec<String> {
        let mut linked: HashSet<String> = HashSet::new();
        if let Some(full) = bodies.get(address) {
            for target in ps::each(&full["links"]) {
                linked.insert(ci(&s(&target)));
            }
        }
        card_model.find(address, 5, |id| owner.get(id) == Some(&n) || linked.contains(&ci(id)) || ps::like(id, "*/0"))
    };

    // --- per-group files ---
    let mut summary: Vec<Value> = Vec::new();
    for group in &groups {
        let n = ps::int(&group["n"])?;
        let id = s(&group["id"]);
        let mut mine: Vec<String> = owner_keys.iter().filter(|k| owner.get(k) == Some(&n)).cloned().collect();
        ps::ps_sort(&mut mine, |a, b| culture_cmp(a, b));
        // The workflow writes linked-gNN.txt when a group's link pass succeeds; a rerun skips it.
        let done = Path::new(&join_path(&staging, &format!("linked-{id}.txt"))).exists();
        let allow = join_path(&staging, &format!("allow-{id}.txt"));
        write(&allow, &format!("{}\n", mine.join("\n")))?;
        let section_list = ps::each(&group["sections"])
            .iter()
            .map(|x| format!("`{}`", join_path(&staging, &format!("sections\\{}", s(x)))))
            .collect::<Vec<_>>()
            .join("、");
        let headings = ps::each(&group["headings"]).iter().map(|h| s(h)).collect::<Vec<_>>().join("；");
        let mut text = String::new();
        let mut line = |t: &str| {
            text.push_str(t);
            text.push_str(NL);
        };
        line(&format!("# 你的组：第 {n} 组"));
        line("");
        line(&format!("- 锚点卡片：`{}`；小节：{headings}", s(&group["anchor"])));
        line(&format!("- 你这一部分的原文：{section_list}"));
        line(&format!("- 全书索引：`{}`；全部卡片索引：`{cards_all}`", s(&manifest["index"])));
        line(&format!("- 写入命令：`... | zt-digest call - -Allow \"{allow}\"`"));
        // The link writer answers for its new Links and their sentences, not for the rest of the Card.
        line(&format!("- SELF-CHECK: `zt-check --log \"{allow}\" --cards \"\" --book {key} --rules L1,L2,R2,R3,R4,R5,R7,R9`"));
        line(&format!("- 你负责的卡片（{} 张）：", mine.len()));
        line("");
        for address in &mine {
            let card = bodies.get(address).cloned().unwrap_or(Value::Null);
            line(&format!("### `{address}` {}", s(&card["title"])));
            line("");
            line(s(&card["body"]).trim());
            line("");
            let candidates: Vec<String> = candidates_of(address, n)
                .iter()
                .map(|c| {
                    let mark = cards_by_address.get(c).map(snapshot::mark).unwrap_or_default();
                    let title = titles.get(c).cloned().unwrap_or_default();
                    format!("`{c}`{mark} {}", snapshot::cut(&title, title_chars))
                })
                .collect();
            if !candidates.is_empty() {
                line(&format!("> 候选目标（用词最接近、尚未链接、属于其他部分）：{}", candidates.join("；")));
                line("");
            }
        }
        write(&join_path(&staging, &format!("link-{id}.md")), &text)?;
        summary.push(json!({ "n": n, "owned": mine.len(), "done": done }));
    }
    let unowned = agent_cards.iter().filter(|c| !owner.has(&s(&c["address"]))).count();
    let pending: Vec<Value> = summary.iter().filter(|g| g["done"] != json!(true)).map(|g| g["n"].clone()).collect();
    util::print_line(&ps::json_compact(&json!({
        "pending": pending,
        "groups": summary,
        "unowned": unowned,
        "cards_all": cards_all,
    })))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units_of(text: &str) -> Vec<Unit> {
        let kept = chinese_blocks(text, &CiMap::new());
        split_units(&kept, "导论", "参考", &["习题".to_string()])
    }

    #[test]
    fn keeps_the_chinese_of_each_pair() {
        let kept = chinese_blocks("# Intro\n\n# 导论\n\nEnglish text.\n\n中文译文。\n\n$$x$$\n\nLone English.\n\n# End", &CiMap::new());
        assert_eq!(kept, vec!["# 导论", "中文译文。", "$$x$$", "Lone English.", "# End"]);
    }

    #[test]
    fn a_figure_is_written_once_and_other_images_go() {
        let mut figures: CiMap<Figure> = CiMap::new();
        figures.insert("a.jpg", Figure { line: "![图 1](artifact:aa)".into(), ..Figure::default() });
        let kept = chinese_blocks("![](x/A.jpg)\nFigure 1\n\n![](x/a.jpg)\n图 1\n\n![](x/b.jpg)\n\n正文。", &figures);
        assert_eq!(kept, vec!["![图 1](artifact:aa)\nFigure 1", "图 1", "正文。"]);
    }

    #[test]
    fn units_merge_and_group() {
        let units = units_of("# 前言\n\n不算。\n\n# 导论\n\n导论正文。\n\n## 习题\n\n跳过。\n\n# 第一章\n\n## 1.1 甲\n\n甲文。\n\n## 1.2 乙\n\n乙文。\n\n# 参考\n\n不算。");
        let headings: Vec<&str> = units.iter().map(|u| u.heading.as_str()).collect();
        assert_eq!(headings, vec!["导论", "第一章", "1.1 甲", "1.2 乙"]);
        assert_eq!(units[0].text, format!("# 导论{NL}{NL}导论正文。{NL}{NL}"));
        assert_eq!(units[2].chapter, "第一章");
        let merged = merge_units(units, 12).unwrap();
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[1].headings, vec!["第一章", "1.1 甲"]);
        assert_eq!(group_units(&merged, 3, 20000), vec![vec![0], vec![1, 2]]);
        assert_eq!(group_units(&merged, 1, 20000), vec![vec![0], vec![1], vec![2]]);
        assert!(merge_units(units_of("# 导论\n\n短。"), 1000).is_err());
    }

    #[test]
    fn event_candidates_skip_citations_and_decades() {
        let unit = Unit {
            chapter: String::new(),
            heading: String::new(),
            headings: Vec::new(),
            text: format!("# 标题{NL}{NL}1990 年代的变化。{NL}Holmstrom (1979) 证明了。{NL}通用汽车在 1926 年收购了费雪。{NL}| 1950 年 |"),
        };
        let mut found = Vec::new();
        event_candidates(&unit, "s01.md", 2, 12, &mut found);
        assert_eq!(found, vec![json!({ "group": 2, "src": "sections/s01.md:5", "text": "通用汽车在 1926 年收购了费雪。" })]);
        assert!(names_a_year("1990年代与1991年") && !names_a_year("1990年代"));
    }

    #[test]
    fn sections_name_groups() {
        assert_eq!(section_id("2.3 契约").as_deref(), Some("2.3"));
        assert_eq!(section_id("a1 附录").as_deref(), Some("a1"));
        assert_eq!(section_id("第三章 企业").as_deref(), Some("3"));
        assert_eq!(section_id("第十章"), None);
        let mut map: CiMap<i64> = CiMap::new();
        map.insert("2.3", 4);
        map.insert("A1", 7);
        map.insert("导论", 1);
        map.insert("5", 9);
        assert_eq!(group_of_section(&map, "2.3.1"), Some(4));
        assert_eq!(group_of_section(&map, "a1"), Some(7));
        assert_eq!(group_of_section(&map, "Intro.2"), Some(1));
        assert_eq!(group_of_section(&map, "5.2"), Some(9));
        assert_eq!(group_of_section(&map, "6"), None);
    }

    #[test]
    fn prefixes_insert_files_before_the_reply_section() {
        let dir = std::env::temp_dir().join(format!("zt-digest-prefix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "\u{FEFF}头 {{KEY}}\r\n\r\n## 回复\r\n回复。\r\n").unwrap();
        std::fs::write(dir.join("b.md"), "插入。\r\n\r\n").unwrap();
        let d = dir.to_string_lossy().into_owned();
        let text = build_prefix(&d, "a.md, b.md", &[("{{KEY}}", "K")]).unwrap();
        assert_eq!(text, "头 K\r\n\r\n插入。\n\n## 回复\r\n回复。\r\n");
        std::fs::write(dir.join("c.md"), "无回复\r\n").unwrap();
        assert_eq!(build_prefix(&d, "c.md,b.md", &[]).unwrap(), "无回复\n\n插入。\n\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
