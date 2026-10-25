//! `zt-digest prepare` (the port of `scripts/book-digest/prepare-book-digest.ps1`) and
//! `zt-digest prepare-link` (the port of `prepare-link-pass.ps1`).
//!
//! With `-Book <Work id|title>` the book is resolved through the read-only Scholar Archive
//! bridge first (`resolve`): the Work, its Chinese/bilingual Markdown, its BibTeX and the
//! heading range become the run's frozen choices, and the caller's explicit `-Source`,
//! `-FileId`, `-BibTeX`, `-BookTitle` and range arguments must agree with them.
//!
//! prepare: reads a Chinese bilingual Markdown book and keeps the Chinese, keeps its figures in
//! ZET's artifact store (`figures::book_figures`), splits it at `#` and `##` headings from
//! -StartHeading to -EndHeading into units and groups, and writes to -Out: book.zh.md,
//! sections/sNN.md with sections/index.md, groups/gNN.md, figures.json, events.json,
//! shared-prefix.md and link-prefix.md (built from the instruction files of the scripts folder),
//! and manifest.json. -Apply creates the agent Literature tree with one anchor Card per group
//! (refused when the tree exists). Prints the manifest as compact JSON.
//!
//! The book's collected notes (注释 / 尾注 / notes) stand outside that range: `notes::extract`
//! maps them to the digest's chapters and writes one read-only file per chapter under
//! `notes\<chapter-id>.md` (design 11.6, R310/R311). The manifest records each chapter's chapter
//! id, original source line spans, note ids and hashes; every group file and model context pack
//! carries its chapter's notes, inline or, when they exceed -NoteMaxChars, as the labelled part
//! files with their note ids. Notes never create a group or a Card, an ambiguous mapping is a
//! preparation fault, and the notes a chapter cites but the book never defines stay visible.
//! Those faults go into the run's retained `source-faults.json` and its `source-faults.md`.
//!
//! prepare-link: reads manifest.json in -Staging and every tree; writes cards-all.md, and per
//! group link-gNN.md (the Cards it owns, with bodies and candidate targets) and allow-gNN.txt.
//! Prints `{pending, blocked, groups, unowned, digest_state, cards_all}`.
//!
//! A group owns exactly the Cards its own receipts name: the helper's `owned-gNN.txt` log (which
//! `group-finish` republishes from the checked results), the group's anchor, and Cards of the
//! digested book's own Literature tree under that anchor. No other Card is assigned by body text,
//! so a `§1.2`-style reference in another book's Card never hands that Card to a link job (#92).
//! A group whose `digest/gNNN@R` unit is not `done` in the staging ledger gets no link job at all:
//! its allow-list is empty, it is reported in `blocked`, and the link stage names it. A folder
//! without a ledger (a preparation with no declared variant) falls back to its write receipts.
//!
//! The instruction files come from ZT_DIGEST_SCRIPTS (the forwarder sets it to its own folder) or
//! the scripts\book-digest folder of the repository this binary was built from.

use crate::args::Args;
use crate::calls;
use crate::card::relatedness::Relatedness;
use crate::figures::{self, Figure};
use crate::link_gap;
use crate::ps::{self, CiMap, NL, culture_cmp, eq, is_in, join_path, key as ci, len16, s};
use crate::snapshot;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

pub(crate) mod notes;
mod resolve;

static BLOCKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n").unwrap());
static CJK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}\x{F900}-\x{FAFF}]").unwrap());
static HAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}]").unwrap());
static KEEP_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(\$\$|\||<|```)").unwrap());
pub(crate) static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(#{1,2})\s+(.+)$").unwrap());
static NOT_EVENT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(!\[|\||#|\d+\))").unwrap());
static CITATION_YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\\?\[[^\]]*\d{4}[^\]]*\\?\]|[（(]\s*\d{4}[a-z]?\s*[)）]").unwrap());
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(1[5-9]\d\d|20[0-2]\d)\s*年").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// The text of a UTF-8 file as `Get-Content -Raw -Encoding utf8` reads it (a BOM dropped).
fn read_raw(path: &str) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("Cannot find path '{path}' because it does not exist."))?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(text.strip_prefix('\u{FEFF}').map(str::to_string).unwrap_or(text))
}

fn write(path: &str, text: &str) -> Result<()> {
    util::write_atomic(path,text)
}
/// Where the instruction files live.
pub(crate) fn scripts_dir() -> String {
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

/// Whether the text holds a Chinese character: a resolved heading with one survives the
/// digest's Chinese-only filter, an English-only heading may not.
pub(crate) fn has_cjk(text: &str) -> bool {
    CJK.is_match(text)
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
pub(crate) fn chinese_blocks(raw: &str, figures: &CiMap<Figure>) -> Vec<String> {
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
pub(crate) struct Unit {
    pub(crate) chapter: String,
    pub(crate) heading: String,
    pub(crate) headings: Vec<String>,
    pub(crate) text: String,
}

/// Units at `#` and `##` headings from -StartHeading up to -EndHeading; a unit under a heading of
/// `skip` is left out up to the next other heading.
fn split_units(kept: &[String], start: &str, end: &str, skip: &[String]) -> Vec<Unit> {
    // ATX headings do not require a blank line before/after them. Separate
    // them before selecting the range, while leaving fenced examples intact.
    let mut blocks = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    for block in kept {
        let mut prose = Vec::new();
        for line in block.split('\n') {
            let trim = line.trim_start_matches(' ');
            let indentation = line.len() - trim.len();
            let marker = trim
                .as_bytes()
                .first()
                .copied()
                .filter(|b| matches!(b, b'`' | b'~'));
            let run = marker
                .map(|m| trim.bytes().take_while(|b| *b == m).count())
                .unwrap_or(0);
            let was_fenced = fence.is_some();
            if indentation <= 3 && run >= 3 {
                if let Some((kind, size)) = fence {
                    if marker == Some(kind) && run >= size && trim[run..].trim().is_empty() {
                        fence = None;
                    }
                } else {
                    fence = Some((marker.unwrap(), run));
                }
            }
            if !was_fenced && fence.is_none() && HEADING.is_match(line) {
                if !prose.is_empty() {
                    blocks.push(prose.join("\n"));
                    prose.clear();
                }
                blocks.push(line.to_string());
            } else {
                prose.push(line);
            }
        }
        if !prose.is_empty() {
            blocks.push(prose.join("\n"));
        }
    }
    let mut units: Vec<Unit> = Vec::new();
    let mut chapter = String::new();
    let mut current: Option<usize> = None;
    let mut inside = false;
    for block in &blocks {
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
            units.push(Unit {
                chapter: chapter.clone(),
                heading,
                headings: Vec::new(),
                text: format!("{block}{NL}{NL}"),
            });
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

/// The Chinese text's source slices, computed without writing: the units after merging, the section
/// files `s01.md…` and the whole-book text. `prepare` writes them; the sections-only backfill branch
/// (S25, #168) recomputes them to reuse or reconstruct a prepared run's staged source.
pub(crate) struct Staged {
    pub(crate) merged: Vec<Unit>,
    pub(crate) files: Vec<(String, String)>,
    pub(crate) book: String,
}

/// Split the Chinese blocks into the selected units, merge tiny ones, and render the section files
/// and the whole-book text. `empty` is the caller's own sentence for a document whose start heading
/// selects no text (a resolved book and an explicit source word that fault differently).
pub(crate) fn staged_sections(
    kept: &[String],
    start: &str,
    end: &str,
    skip: &[String],
    min_unit_chars: i64,
    empty: &str,
) -> Result<Staged> {
    let units = split_units(kept, start, end, skip);
    if units.is_empty() {
        bail!("{empty}");
    }
    let merged = merge_units(units, min_unit_chars)?;
    let mut book = if end.is_empty() {
        format!("# 全书正文（中文译本，{start} 起）{NL}{NL}")
    } else {
        format!("# 全书正文（中文译本，{start} 至 {end} 之前）{NL}{NL}")
    };
    let mut files = Vec::new();
    for (index, unit) in merged.iter().enumerate() {
        files.push((format!("s{:02}.md", index + 1), unit.text.clone()));
        book.push_str(&unit.text);
    }
    Ok(Staged {
        merged,
        files,
        book,
    })
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
pub(crate) fn build_prefix(dir: &str, files: &str, fill: &[(&str, &str)]) -> Result<String> {
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
pub(crate) fn run_contract(
    manifest: &Map<String, Value>,
    variant: &str,
    source_hash: &str,
    options: Value,
    archive: Value,
) -> Result<Value> {
    let base: Value = serde_json::from_str(include_str!(
        "../../../scripts/book-digest/workflow/zt-digest-base.blueprint.json"
    ))?;
    let profile: Value = serde_json::from_str(match variant {
        "econ" => {
            include_str!("../../../scripts/book-digest/workflow/zt-econ-digest.blueprint.json")
        }
        "math" => {
            include_str!("../../../scripts/book-digest/workflow/zt-math-digest.blueprint.json")
        }
        _ => bail!("unsupported prepared-run variant `{variant}`"),
    })?;
    let file = |path: &str| -> Result<Value> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("incomplete preparation artifact `{path}`"))?;
        Ok(
            json!({"path":std::fs::canonicalize(path)?.to_string_lossy(),"sha256":crate::canon::sha256_hex(&bytes)}),
        )
    };
    let source = file(&s(&manifest["source"]))?;
    if source["sha256"] != source_hash {
        bail!("source changed during preparation; use a new output directory");
    }
    let mut source = source;
    source["file_id"] = manifest["file_id"].clone();
    let mut groups = Vec::new();
    for group in manifest["groups"]
        .as_array()
        .context("incomplete prepared groups")?
    {
        let mut record = file(&s(&group["file"]))?;
        // The mathematics skeleton appends its own unit list to a group file at the
        // frozen boundary. The length lets the ledger verify that frozen prefix exactly
        // while the declared extension marker names where the compiler's own text starts.
        record["bytes"] = json!(std::fs::metadata(s(&group["file"]))?.len());
        for key in ["n", "id", "anchor", "chapter", "headings", "chars"] {
            record[key] = group[key].clone();
        }
        let mut sections = Vec::new();
        for section in group["sections"]
            .as_array()
            .context("incomplete prepared sections")?
        {
            sections.push(file(&join_path(
                &s(&manifest["staging"]),
                &format!("sections/{}", s(section)),
            ))?);
        }
        record["sections"] = json!(sections);
        groups.push(record);
    }
    let group_manifest_hash =
        crate::canon::sha256_hex(crate::canon::canonical_json(&json!(groups)).as_bytes());
    let mut artifacts = Map::new();
    for key in [
        "book",
        "index",
        "shared_prefix",
        "link_prefix",
        "figures_file",
    ] {
        artifacts.insert(key.to_string(), file(&s(&manifest[key]))?);
    }
    artifacts.insert(
        "events".into(),
        file(&join_path(&s(&manifest["staging"]), "events.json"))?,
    );
    artifacts.insert(
        "setup_calls".into(),
        file(&join_path(&s(&manifest["staging"]), "setup-calls.json"))?,
    );
    Ok(
        json!({"schema_version":1,"variant":variant,"citation_key":manifest["citation_key"],"archive_identity":archive,"source":source,"options":options,"contracts":{"base_version":base["version"],"variant_version":profile["version"],"artifact_schema":1},"groups":groups,"group_manifest_hash":group_manifest_hash,"artifacts":artifacts,"group_extension":if variant == "math" {json!({"marker":crate::math::group_extension_marker(),"by":"math-skeleton"})} else {Value::Null}}),
    )
}
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
        "NoteMaxChars",
        "Instructions",
        "Variant",
        "RunOptions",
        "Book",
        "MaxAttempts",
        "DshRunCap",
    ];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &["NoEventCandidates", "NoFigures", "Apply", "Brief", "Validate", "Sections"])?;
    // S1 (docs/workflow-simplicity-proposal.md): -Brief ends with one short line a relay carries whole (the group
    // count and the key) instead of the whole manifest, which a Haiku agent had to count.
    let brief = a.flag("Brief");
    let emit = |manifest: &Value| -> Result<()> {
        if brief {
            util::print_line(&ps::json_compact(&json!({"ok": true, "groups": ps::ps_count(&manifest["groups"]), "citation_key": manifest["citation_key"], "out": manifest["out"]})))
        } else {
            util::print_line(&ps::json_compact(manifest))
        }
    };
    let out = mandatory(&a, "Out")?;
    // S25 (#168): the sections-only branch for one already digested book. It validates the book
    // and rebuilds or reuses the staged source slices; it allocates no group, creates no root,
    // anchor, group, model or narrative Card and never runs the preparation's Card setup.
    if a.flag("Validate") || a.flag("Sections") {
        if a.flag("Apply") {
            bail!("-Validate and -Sections prepare a backfill and write no Card; remove -Apply");
        }
        return crate::backfill::run(&a);
    }
    if Path::new(&out).join("ledger.sqlite3").exists() {
        bail!("output directory is already initialized; use ledger resume operations or a new output directory");
    }
    let book = a.str("Book");
    let mut held: Option<Zt> = None;
    let resolved = if book.trim().is_empty() {
        None
    } else {
        let mut zt = crate::zt::start()?;
        let explicit = resolve::Explicit {
            source: a.str("Source"),
            source_given: a.has("Source"),
            file_id: a.int_or("FileId", 0)?,
            citation_key: a.str("CitationKey"),
            citation_key_given: a.has("CitationKey"),
            bibtex: a.str("BibTeX"),
            bibtex_given: a.has("BibTeX"),
            book_title: a.str("BookTitle"),
            book_title_given: a.has("BookTitle"),
            start: a.has("StartHeading").then(|| a.str("StartHeading")),
            end: a.has("EndHeading").then(|| a.str("EndHeading")),
            skip: a.has("SkipHeadings").then(|| a.str("SkipHeadings")),
        };
        match resolve::run(&mut zt, &out, book.trim(), &explicit)? {
            resolve::Outcome::NeedsSelection(result) => {
                util::print_last(&result)?;
                return Ok(resolve::NEEDS_SELECTION);
            }
            resolve::Outcome::Ready(resolved) => {
                held = Some(zt);
                Some(resolved)
            }
        }
    };
    let (source, key, bibtex, title, start, end, skip_headings) = match &resolved {
        Some(resolved) => (
            resolved.source.clone(),
            resolved.citation_key.clone(),
            resolved.bibtex.clone(),
            resolved.book_title.clone(),
            resolved.start.clone(),
            resolved.end.clone(),
            resolved.skip.clone(),
        ),
        None => (
            mandatory(&a, "Source")?,
            mandatory(&a, "CitationKey")?,
            mandatory(&a, "BibTeX")?,
            a.str("BookTitle"),
            a.str_or("StartHeading", "导论"),
            a.str_or("EndHeading", "参考"),
            a.str("SkipHeadings"),
        ),
    };
    let max_chars = a.int_or("MaxChars", 20000)?;
    let max_units = a.int_or("MaxUnits", 3)?;
    let min_unit_chars = a.int_or("MinUnitChars", 1000)?;
    let mut file_id = match &resolved {
        Some(resolved) => resolved.file_id,
        None => a.int_or("FileId", 0)?,
    };
    let min_figure_bytes = a.int_or("MinFigureBytes", 3000)?;
    let max_event_candidates = a.int_or("MaxEventCandidates", 12)?;
    let note_max_chars = a.int_or("NoteMaxChars", 6000)?;
    let instructions = a.str_or("Instructions", "dsh-digest-base.zh.md,dsh-model-cards.zh.md");
    let no_event_candidates = a.flag("NoEventCandidates");
    let no_figures = a.flag("NoFigures");
    let apply = a.flag("Apply");
    let variant=if a.has("Variant") {a.choice("Variant",&["econ","math"])?} else {String::new()};
    if !variant.is_empty() && !apply {bail!("Variant requires Apply so the prepared run can bind its actual archive");}
    if a.has("RunOptions") && variant.is_empty() {bail!("RunOptions requires Variant and Apply");}
    if (a.has("MaxAttempts") || a.has("DshRunCap")) && variant.is_empty() {bail!("MaxAttempts and DshRunCap require Variant and Apply");}
    // R329 (§15.2): a full digest that would write this Literature tree reads the tree normally and
    // stops before the first skeleton or metadata write, and before it writes a group, a section or
    // a ledger. The tree's Citation key is known here, so this is the first write-side step.
    if apply {
        if held.is_none() {
            held = Some(crate::zt::start()?);
        }
        let z = held.as_mut().context("no zt mcp process for the Literature tree")?;
        if let Some(refusal) = crate::lockcheck::preflight(z, "prepare", "apply", &key)? {
            return crate::lockcheck::stop("prepare", &refusal);
        }
    }
    // R273: the managed run's bounded settings are validated here, at preparation,
    // and persisted in the immutable run contract. A resume reuses them.
    let settings = match crate::ledger::run_settings(
        a.has("MaxAttempts").then(|| a.int_or("MaxAttempts", 0)).transpose()?,
        a.has("DshRunCap").then(|| a.int_or("DshRunCap", 0)).transpose()?,
    ) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("zt-digest prepare: {error}");
            return Ok(2);
        }
    };
    // The workflow modules pass the switches as JSON text; a hand-run prepare may name a file.
    let declared=if a.has("RunOptions") {Some(crate::canon::read_json_arg(&a.str("RunOptions"))?)}else{None};
    let run_options=if variant.is_empty(){Value::Null}else{crate::ledger::run_options(&variant,no_figures,declared.as_ref())?};
    let scripts = scripts_dir();

    // --- Chinese-only blocks ---
    let source_bytes=std::fs::read(&source).with_context(||format!("Cannot find path '{source}' because it does not exist."))?;
    let source_file_hash=crate::canon::sha256_hex(&source_bytes);
    let source_hash=if variant.is_empty() {String::new()} else {source_file_hash.clone()};
    if !variant.is_empty() {std::str::from_utf8(&source_bytes).context("prepared-run source must be valid UTF-8")?;}
    let raw=String::from_utf8_lossy(&source_bytes);
    let raw=raw.strip_prefix('\u{FEFF}').unwrap_or(&raw).replace("\r\n","\n");
    drop(source_bytes);
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
    let empty = if resolved.is_some() {
        format!(
            "the start heading \"{start}\" is not in the Chinese text of {source} (the bilingual filter drops an English-only heading whose next block is its translation); pass -StartHeading with a heading of the Chinese text"
        )
    } else {
        format!("no heading \"{start}\" found in {source}")
    };
    let staged = staged_sections(&kept, &start, &end, &skip, min_unit_chars, &empty)?;
    let merged = staged.merged;
    let groups = group_units(&merged, max_units, max_chars);

    for dir in ["groups", "sections", "notes"] {
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

    // The chapter endnotes stand outside the digest range: read-only reference text for the
    // groups and model context packs, never a group or a Card (design 11.6, R310/R311).
    let chapter_titles: Vec<String> = merged.iter().map(|unit| unit.chapter.clone()).collect::<Vec<_>>().into_iter().fold(Vec::new(), |mut list, chapter| {
        if list.last() != Some(&chapter) {
            list.push(chapter);
        }
        list
    });
    let chapter_titles = chapter_titles.into_iter().filter(|chapter| !chapter.is_empty()).collect::<Vec<_>>();
    let notes = notes::extract(
        &raw,
        &source,
        &staging,
        &chapter_titles,
        &start,
        &end,
        &notes::Options { title: title.clone(), max_chars: note_max_chars },
    )?;
    // The preparation's own notes faults go into the same retained source-fault record the
    // architects add to, so the person who keeps the Scholar Archive reads one report.
    let identity = crate::faults::Identity {
        path: source.clone(),
        file_id,
        sha256: source_file_hash.clone(),
        lines: raw.split('\n').map(str::to_string).collect(),
    };
    let source_report = crate::faults::record_preparation(&staging, &identity, &notes.faults)?;

    // The book stays on disk: one file per unit plus an index. A resolved run that found no
    // back matter has no end heading, so its header names only the start.
    for (name, text) in &staged.files {
        write(&join_path(&staging, &format!("sections\\{name}")), text)?;
    }
    write(&join_path(&staging, "book.zh.md"), &staged.book)?;
    let unit_file: Vec<String> = (0..merged.len()).map(|u| format!("s{:02}.md", u + 1)).collect();

    // Which other units each group should look at first, so its run needs fewer searches.
    let mut unit_group: HashMap<String, usize> = HashMap::new();
    for (g, members) in groups.iter().enumerate() {
        for &u in members {
            unit_group.insert(ci(&unit_file[u]), g + 1);
        }
    }
    let documents: Vec<(String, String)> = merged.iter().enumerate().map(|(u, unit)| (unit_file[u].clone(), unit.text.clone())).collect();
    let section_model = Relatedness::new(&documents, 150);

    let documents_line = match &resolved {
        Some(resolved) => resolved.documents.clone(),
        None => if file_id != 0 { figures::document_line(file_id)? } else { String::new() },
    };
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
    manifest.insert("source_sha256".into(), json!(source_file_hash));
    manifest.insert("groups".into(), json!([]));
    manifest.insert("file_id".into(), json!(file_id));
    manifest.insert("figures".into(), json!(figure_map.len()));
    manifest.insert("figures_file".into(), json!(join_path(&staging, "figures.json")));
    manifest.insert("documents".into(), json!(documents_line));
    manifest.insert("notes".into(), notes.summary.clone());
    // The resolution was frozen (and persisted with every excluded section) before this
    // staging folder held any group work.
    if let Some(resolved) = &resolved {
        manifest.insert("preparation".into(), json!(resolved.preparation));
        manifest.insert("book_resolution".into(), resolved.summary.clone());
    }
    manifest.insert("source_faults".into(), json!({
        "store": source_report["store"].clone(),
        "report": source_report["report"].clone(),
    }));

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
        let events = if no_event_candidates { String::new() } else { format!("- 事件候选（本组小节中写出年份的句子；由你判断哪些是真实事件）：\n{event_lines}\n") };
        // The script's here-string: its lines end as the script file's did (CRLF).
        let head = [
            format!("# 你的组：第 {n} 组"),
            String::new(),
            format!("- 锚点卡片：`{anchor}`"),
            format!("- 章节：{chapter}；小节：{}（文件 {}，原文附在下面）", headings.join("；"), sections.join("、")),
            format!("- 用词最接近的其他小节（先看这些）：{}", related.join("；")),
            format!("- 写入命令：`... | zt-digest call - -Log \"{owned}\" -SrcLog \"{src_log}\"`（只写本书的文献树）"),
            format!("- SELF-CHECK: `zt-check --cards \"{anchor}\" --log \"{owned}\" --book {key}`"),
            format!("- 现有文献卡片快照：`{staging}\\snapshot-{id}.md`（本组启动时生成）"),
            format!("- 全书索引：`{index_path}`"),
            format!("- 本书术语表：`{staging}\\terms.md`（存在时先读它，按它用词）"),
            events,
            "## 你的小节原文".to_string(),
            String::new(),
        ]
        .join(NL);
        // The chapter's own endnotes: read-only reference text, and the chapter's missing notes
        // stay visible. A group of a chapter without notes carries no block.
        let notes_block = notes
            .record(&chapter)
            .and_then(|record| notes::reference_block(&record, &source))
            .unwrap_or_default();
        write(&file, &format!("{head}{text}{notes_block}"))?;
        outline.push_str(&format!("- 第 {n} 组 `{anchor}`：{chapter} | {}{NL}", headings.join("；")));
        group_entries.push(json!({
            "n": n, "id": id, "anchor": anchor, "chapter": chapter, "headings": headings,
            "sections": sections, "chars": len16(&text), "file": file,
            "notes": notes.record(&chapter).map(|record| json!({
                "chapter_id":record["chapter_id"].clone(),
                "files":record["files"].as_array().map(|files| files.iter().map(|f| f["path"].clone()).collect::<Vec<_>>()).unwrap_or_default(),
                "note_ids":record["note_ids"].clone(),
                "missing":record["missing"].clone(),
            })).unwrap_or(Value::Null),
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
        if held.is_none() {
            held = Some(crate::zt::start()?);
        }
        let z = held.as_mut().context("no zt mcp process for the Literature tree")?;
        let archive=if !variant.is_empty() {
            z.archive_view()?["archive_identity"].clone()
        } else {Value::Null};
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
        if !variant.is_empty() {
            let options=json!({"book_title":title,"bibtex_sha256":crate::canon::sha256_hex(bibtex.as_bytes()),"start_heading":start,"end_heading":end,"skip_headings":skip_headings,"max_chars":max_chars,"max_units":max_units,"min_unit_chars":min_unit_chars,"min_figure_bytes":min_figure_bytes,"max_event_candidates":max_event_candidates,"instructions":instructions,"no_event_candidates":no_event_candidates,"no_figures":no_figures,"run_options":run_options});
            let mut contract = run_contract(&manifest, &variant, &source_hash, options, archive)?;
            contract["settings"] = settings.clone();
            manifest.insert("run_contract".into(), contract);
            write(
                &join_path(&staging, "manifest.json"),
                &ps::json_pretty(&Value::Object(manifest)),
            )?;
            // The report was rendered before the manifest existed; render it once more so the
            // file the person reads names the book.
            crate::faults::render(&staging, &identity)?;
            crate::ledger::prepare_setup(&staging, z)?;
            let complete = crate::util::read_json(join_path(&staging, "manifest.json"))?;
            emit(&complete)?;
            return Ok(0);
        }
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
    // The report was rendered before the manifest existed; render it once more so the file the
    // person reads names the book.
    crate::faults::render(&staging, &identity)?;
    emit(&manifest)?;
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
    fn set(&mut self, k: &str, v: V) {
        self.0.insert(ci(k), v);
    }
}

/// The digest state of every prepared group, read from the staging folder's own ledger (#92).
/// `None` when the folder holds no ledger (a preparation with no declared variant): such a folder
/// can only offer its write receipts. A folder with a ledger names the exact state of each group's
/// `digest/gNNN@R` unit, so a group that never completed is refused instead of guessed.
fn digest_states(staging: &str) -> Result<Option<std::collections::BTreeMap<i64, String>>> {
    let path = join_path(staging, "ledger.sqlite3");
    if !Path::new(&path).exists() {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("open the prepared ledger {path}"))?;
    let mut statement = conn.prepare("SELECT id,state FROM units WHERE layer='digest'")?;
    let rows = statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut revisions: std::collections::BTreeMap<i64, (i64, String)> = std::collections::BTreeMap::new();
    for row in rows {
        let (id, state) = row?;
        let Some(rest) = id.strip_prefix("digest/g") else { continue };
        let Some((number, revision)) = rest.split_once('@') else { continue };
        let (Ok(number), Ok(revision)) = (number.parse::<i64>(), revision.parse::<i64>()) else { continue };
        let entry = revisions.entry(number).or_insert((revision, state.clone()));
        if revision > entry.0 {
            *entry = (revision, state);
        }
    }
    Ok(Some(revisions.into_iter().map(|(number, (_, state))| (number, state)).collect()))
}

/// `zt-digest prepare-link -Staging <folder> [-TitleChars 40]` (prepare-link-pass.ps1).
///
/// Reads the staging manifest and the archive, then computes each completed group's semantic gap
/// set from its own write receipts (§5.5; D118–D124, #110). Only the gap set becomes a Link job:
/// an Idea Card with no valid kinded Link to another group or earlier material, a substantive
/// orphan, and the anchor whose `在全书中的位置` paragraph needs writing. A group whose
/// `digest/gNNN@R` unit is not `done` gets no Link job and an empty allow-list (#92).
///
/// Ownership is receipt-based: a Card is in a group's scope exactly when its own
/// `owned-gNN.txt` names it, and the anchor. No body text, section number, older Card of the same
/// tree, or inferred address ever widens that scope (AC02, AC03). A Card two groups' receipts name
/// is a preparation fault, and so is a receipt that names a Card the archive does not hold.
pub fn link_cli(argv: &[String]) -> Result<i32> {
    let params = ["Staging", "TitleChars"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &["Brief"])?;
    let staging = mandatory(&a, "Staging")?;
    let title_chars = a.int_or("TitleChars", 40)?.max(0) as usize;
    let manifest = ps::parse_json(&read_raw(&join_path(&staging, "manifest.json"))?)?;
    let key = s(&manifest["citation_key"]);
    let groups = ps::each(&manifest["groups"]);

    // R328 (§15.2): the link pass allocates the Link jobs that write this book's Cards, so it stops
    // before writing its first file when the book's Literature tree is locked by the owner. It
    // still reads the locked tree while it asks, and it allocates nothing. One client does both that
    // ask and the pass itself.
    let mut zt = crate::zt::start()?;
    if let Some(refusal) = crate::lockcheck::preflight(&mut zt, "prepare-link", "plan", &key)? {
        return crate::lockcheck::stop("prepare-link", &refusal);
    }

    // --- every Literature tree, both Workspaces (docs/literature-only-digest-design.md: the Link
    // pass reads and links Literature trees only; Topic trees are zt-idea-grow's) ---
    let details = snapshot::read_literature_trees(&mut zt)?;
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

    // --- digest completion: the staging ledger is the authority, a folder without one has only its
    // write receipts (#92) ---
    let states = digest_states(&staging)?;
    let digest_state = if states.is_some() { "ledger" } else { "receipts" };

    // --- every Card's authored text facts, once: bodies, kind facts, and the candidate catalogue ---
    let addresses: Vec<String> = details.iter().flat_map(|tree| ps::ps_items(&tree["cards"]))
        .map(|c| s(&c["address"])).collect();
    let read = zt.cards(&addresses, "text")?;
    let title_revision = details.first().and_then(|tree| tree["archive_revision"].as_u64());
    if title_revision != Some(read.archive_revision) {
        bail!("incomplete prepare-link input: changing view between titles and Card bodies");
    }
    let mut bodies: BTreeMap<String, Value> = BTreeMap::new();
    for card in &read.cards {
        bodies.insert(ps::key(&s(&card["address"])), card.clone());
    }
    if bodies.len() != addresses.len() {
        bail!("read {} of {} required Cards", bodies.len(), addresses.len());
    }
    // The archive-wide facts the gap classifier reads: every authored Link, both registers, and the
    // Literature trees with their anchors.
    let links_view = zt.call("list_links", json!({}))?;
    let mut register_views = Vec::new();
    for workspace in ["agent", "human"] {
        register_views.push(zt.call(
            "get_register",
            json!({"workspace": workspace, "entry_offset": 0, "entry_limit": 1}),
        )?);
    }
    let archive = link_gap::Archive::from_views(&details, &links_view, &register_views)?;
    let facts = link_gap::Facts::new(&key, bodies, archive, read.archive_revision);
    if zt.cards(&[], "text")?.archive_revision != read.archive_revision {
        bail!("incomplete prepare-link input: changing view while the gap scope is read");
    }

    // --- ownership: the group's own receipts, and its anchor. Nothing is inferred ---
    let mut owners: Vec<link_gap::Group> = Vec::new();
    for group in &groups {
        let n = ps::int(&group["n"])?;
        let id = s(&group["id"]);
        let log = join_path(&staging, &format!("owned-{id}.txt"));
        let mut owned = vec![s(&group["anchor"])];
        if Path::new(&log).exists() {
            for line in ps::read_lines(&log)? {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if !facts.cards.contains_key(&ps::key(line)) {
                    bail!("incomplete prepare-link input: the receipt of group {n} names the missing Card `{line}`");
                }
                if !owned.iter().any(|at| eq(at, line)) {
                    owned.push(line.to_string());
                }
            }
        }
        owners.push(link_gap::Group { n, id, anchor: s(&group["anchor"]), owned });
    }
    let scopes = link_gap::classify(&facts, &owners)?;

    // --- candidate targets: the most similar Cards outside the gap Card's own group ---
    let mut titles: ByAddress<String> = ByAddress::new();
    let mut cards_by_address: ByAddress<Value> = ByAddress::new();
    let mut documents: Vec<(String, String)> = Vec::new();
    for tree in &details {
        for card in ps::ps_items(&tree["cards"]) {
            let address = s(&card["address"]);
            let title = s(&card["title"]);
            titles.set(&address, title.clone());
            let full = facts.cards.get(&ps::key(&address))
                .with_context(|| format!("incomplete candidate input: missing `{address}`"))?;
            let content = if eq(&s(&card["workspace"]), "agent") && !eq(&s(&card["kind"]), "topic") {
                s(&full["body"])
            } else {
                s(&full["excerpt"])
            };
            let text = format!("{title} {title} {content}");
            cards_by_address.set(&address, card);
            documents.push((address, text));
        }
    }
    let card_model = Relatedness::new(&documents, 80);
    let group_owns = |n: i64, target: &str| -> bool {
        owners.iter().any(|owner| owner.n == n && owner.owned.iter().any(|at| eq(at, target)))
    };
    let candidates_of = |address: &str, n: i64| -> Vec<String> {
        let mut linked: HashSet<String> = HashSet::new();
        if let Some(full) = facts.cards.get(&ps::key(address)) {
            for target in ps::each(&full["links"]) {
                linked.insert(ps::key(&s(&target)));
            }
        }
        card_model.find(address, 5, |id| {
            group_owns(n, id)
                || linked.contains(&ps::key(id))
                || ps::like(id, "*/0")
                || link_gap::is_book_place(id)
        })
    };

    // --- per-group files, and the public scope evidence ---
    let mut summary: Vec<Value> = Vec::new();
    let mut blocked: Vec<Value> = Vec::new();
    let mut pending: Vec<i64> = Vec::new();
    let mut no_gap: Vec<i64> = Vec::new();
    let (mut owned_total, mut gap_total, mut skipped_total) = (0usize, 0usize, 0usize);
    for group in &groups {
        let n = ps::int(&group["n"])?;
        let id = s(&group["id"]);
        let scope = scopes.iter().find(|scope| scope.n == n)
            .with_context(|| format!("the gap scope of group {n} is missing"))?;
        let done = Path::new(&join_path(&staging, &format!("linked-{id}.txt"))).exists();
        // #92: a group whose digest did not complete gets no link job. Its write fence is an empty
        // allow-list, so a job started by mistake could change no Card at all.
        let (completed, state) = match &states {
            Some(states) => {
                let state = states.get(&n).cloned().unwrap_or_else(|| "unregistered".to_string());
                (state == "done", state)
            }
            None => {
                let receipt = Path::new(&join_path(&staging, &format!("owned-{id}.txt"))).exists();
                (receipt, if receipt { "receipt".to_string() } else { "unverified".to_string() })
            }
        };
        owned_total += scope.owned.len();
        skipped_total += scope.skipped.len();
        let allow = join_path(&staging, &format!("allow-{id}.txt"));
        let headings = ps::each(&group["headings"]).iter().map(|h| s(h)).collect::<Vec<_>>().join("；");
        if !completed {
            write(&allow, "")?;
            blocked.push(json!({
                "n":n, "unit":format!("digest/{id}@1"), "state":state,
                "reason":format!("the digest unit digest/{id}@1 is {state}, not done; this group gets no link job"),
            }));
            let mut text = String::new();
            let mut line = |t: &str| {
                text.push_str(t);
                text.push_str(NL);
            };
            line(&format!("# 你的组：第 {n} 组（digest 未完成，本次不做连接）"));
            line("");
            line(&format!("- 状态：本组的 digest 单元 `digest/{id}@1` 是 `{state}`，不是 `done`。"));
            line("- 本次没有可连接的卡片：允许清单为空，写入命令不能改动任何卡片。等该组 digest 完成后重新准备连接。");
            write(&join_path(&staging, &format!("link-{id}.md")), &text)?;
            summary.push(json!({"n":n,"id":id,"owned":scope.owned.len(),"done":done,"digest":state,"blocked":true,"gaps":scope.gaps.len(),"skipped":scope.skipped.len(),"position":scope.position}));
            continue;
        }
        let gap_addresses: Vec<String> = scope.gaps.iter().map(|gap| gap.at.clone()).collect();
        gap_total += gap_addresses.len();
        // The write fence is the gap set: only these Cards receive a Link job.
        write(&allow, &format!("{}\n", gap_addresses.join("\n")))?;
        let mut candidates: Map<String, Value> = Map::new();
        for at in &gap_addresses {
            candidates.insert(at.clone(), json!(candidates_of(at, n)));
        }
        write(
            &join_path(&staging, &format!("link-gap-{id}.json")),
            &serde_json::to_string_pretty(&json!({
                "schema_version": 1,
                "book": key,
                "archive_revision": read.archive_revision,
                "group": scope.json(),
                "candidates": candidates,
            }))?,
        )?;
        let mut text = String::new();
        let mut line = |t: &str| {
            text.push_str(t);
            text.push_str(NL);
        };
        line(&format!("# 第 {n} 组的连接缺口（digest 已完成）"));
        line("");
        line(&format!("- 锚点卡片：`{}`；小节：{headings}", scope.anchor));
        line(&format!("- 本组拥有的卡片 {} 张，其中语义缺口 {} 张；已有跨部分关系的卡片 {} 张（不要动）。", scope.owned.len(), scope.gaps.len(), scope.skipped.len()));
        line(&format!("- 锚点的“在全书中的位置”段：{}。", match scope.position.as_str() {
            "missing" => "缺失，本组要写这一段",
            "duplicate" => "有不止一段，本次只记录原因，不追加第二段",
            "unlinked" => "只有一段但没有链接到其他部分，本次记录原因，不追加第二段",
            _ => "已有一段并链接到其他部分，不要追加",
        }));
        line(&format!("- 原文：{}；全书索引：`{}`；全部卡片索引：`{cards_all}`", ps::each(&group["sections"]).iter().map(|x| format!("`{}`", join_path(&staging, &format!("sections\\{}", s(x))))).collect::<Vec<_>>().join("、"), s(&manifest["index"])));
        line("");
        if gap_addresses.is_empty() {
            line("本组没有语义缺口：所有卡片都已有有效的跨部分关系。写入命令的允许清单为空，不需要模型任务。");
        } else {
            for gap in &scope.gaps {
                let card = facts.cards.get(&ps::key(&gap.at)).cloned().unwrap_or(Value::Null);
                line(&format!("### `{}` {}", gap.at, s(&card["title"])));
                line("");
                line(&format!("缺口：{}", gap.reasons.join("、")));
                line("");
                line(s(&card["body"]).trim());
                line("");
                let list: Vec<String> = ps::ps_items(&candidates.get(&gap.at).cloned().unwrap_or_else(|| json!([])))
                    .iter()
                    .map(|c| {
                        let address = s(c);
                        let mark = cards_by_address.get(&address).map(snapshot::mark).unwrap_or_default();
                        let title = titles.get(&address).cloned().unwrap_or_default();
                        format!("`{address}`{mark} {}", snapshot::cut(&title, title_chars))
                    })
                    .collect();
                if !list.is_empty() {
                    line(&format!("> 候选目标（用词最接近、尚未链接、属于其他部分）：{}", list.join("；")));
                    line("");
                }
            }
        }
        write(&join_path(&staging, &format!("link-{id}.md")), &text)?;
        if !done {
            if gap_addresses.is_empty() {
                no_gap.push(n);
            } else {
                pending.push(n);
            }
        }
        summary.push(json!({"n":n,"id":id,"owned":scope.owned.len(),"done":done,"digest":state,"blocked":false,"gaps":gap_addresses.len(),"skipped":scope.skipped.len(),"position":scope.position}));
    }
    let owned_keys: HashSet<String> = owners.iter().flat_map(|group| group.owned.iter().map(|at| ps::key(at))).collect();
    let unowned = details
        .iter()
        .filter(|tree| eq(&s(&tree["workspace"]), "agent"))
        .flat_map(|tree| ps::ps_items(&tree["cards"]))
        .filter(|card| !eq(&s(&card["kind"]), "topic"))
        .filter(|card| !owned_keys.contains(&ps::key(&s(&card["address"]))))
        .count();
    // S1: -Brief keeps the line short enough for a relay: the per-group summary becomes a map of gap counts.
    if a.flag("Brief") {
        let gaps: Map<String, Value> = summary.iter().map(|g| (s(&g["n"]), g["gaps"].clone())).collect();
        let blocked_brief: Vec<Value> = ps::ps_items(&json!(blocked)).iter().map(|b| json!({"n": b["n"], "unit": b["unit"], "state": b["state"]})).collect();
        util::print_line(&ps::json_compact(&json!({
            "pending": pending, "no_gap": no_gap, "blocked": blocked_brief, "gaps": gaps,
            "owned_total": owned_total, "gap_total": gap_total, "skipped_total": skipped_total, "unowned": unowned, "digest_state": digest_state,
        })))?;
        return Ok(0);
    }
    util::print_line(&ps::json_compact(&json!({
        "pending": pending,
        "no_gap": no_gap,
        "blocked": blocked,
        "groups": summary,
        "owned_total": owned_total,
        "gap_total": gap_total,
        "skipped_total": skipped_total,
        "pending_count": pending.len(),
        "no_gap_count": no_gap.len(),
        "scheduled": pending.len(),
        "unowned": unowned,
        "digest_state": digest_state,
        "cards_all": cards_all,
        "archive_revision": read.archive_revision,
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
    fn headings_without_blank_lines_still_bound_the_selected_range() {
        let units = units_of("# 导论\n正文。\n## 小节\n第二段。\n# 参考\n未选择。");
        assert_eq!(
            units.iter().map(|u| u.heading.as_str()).collect::<Vec<_>>(),
            vec!["导论", "小节"]
        );
        assert!(units[0].text.contains("正文。"));
        assert!(units[1].text.contains("第二段。"));
        assert!(units.iter().all(|u| !u.text.contains("未选择")));
        let code = units_of(
            "# 导论\n\n```markdown\n# 参考\n代码示例。\n```\n\n仍是正文。\n\n# 参考\n未选择。",
        );
        assert_eq!(code.len(), 1);
        assert!(code[0].text.contains("# 参考\n代码示例。"));
        assert!(code[0].text.contains("仍是正文。"));
        assert!(!code[0].text.contains("未选择"));
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





