//! R410–R417 (#88): explicit adoption of a legacy digest staging folder.
//!
//! A folder that predates the run ledger keeps its artifacts but has no prepared manifest,
//! so `ledger init` must not fabricate its history (R410) and `ledger resume` refuses it.
//! The operator runs `ledger adopt` instead. It reads the legacy preparation index, the
//! frozen source and group files, the owned and source logs, the compiler plans and step
//! state, the actual Cards and every retained command/provider output (R411), and it
//! resolves the archive/run identity and the frozen source inputs (R412). The result is a
//! read-only plan that classifies each obligation as verified, known, incomplete or unknown.
//!
//! `-Apply` is the separate, authorized step (R416). It writes the prepared manifest and the
//! run ledger from that plan: verified work is imported with `adopted` events that name the
//! evidence and a new read-only verification with its date and tool version, never a
//! pretended historic SELF-CHECK (R413); verified models stay built and only their missing
//! derive/review/explanation work becomes targeted units (R414); every missing or unknown
//! group job, note suggestion and downstream obligation is registered (R415). A folder
//! without enough source/group evidence keeps its artifacts and names the missing scope
//! instead of silently redigesting the whole book (R417).
//!
//! A skeleton anchor is never evidence of digest success: a group is verified only from the
//! substantive Cards the group actually produced, or from its owned log together with them.

use super::*;
use crate::prepare;
use crate::ps::{self, s};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The skeleton anchor text `prepare` writes: a group whose anchor still carries it has not
/// been digested, whatever else the folder holds (R411/T14).
const SKELETON_ANCHOR: &str = "负责该组的代理将把它改写为";

/// One legacy group: the file that was prepared for it and the evidence of what it produced.
struct LegacyGroup {
    n: u64,
    anchor: String,
    file: PathBuf,
    sha256: String,
    sections: Vec<(String, PathBuf)>,
    chapter: String,
    headings: Vec<String>,
    chars: i64,
    owned_log: Option<PathBuf>,
    src_log: Option<PathBuf>,
}

/// One retained model compiler state entry, when the folder kept its plans.
struct RetainedModel {
    address: String,
    status: String,
    plan: Option<(PathBuf, String)>,
    notes: Vec<String>,
    has_review: bool,
}

struct Legacy {
    out: PathBuf,
    variant: String,
    key: String,
    title: String,
    source: PathBuf,
    source_sha256: String,
    index: PathBuf,
    index_sha256: String,
    groups: Vec<LegacyGroup>,
    terms: Option<(PathBuf, String)>,
    figures: Option<(PathBuf, String)>,
    retained: Vec<RetainedModel>,
    model_states: usize,
    /// The command/provider outputs the folder still holds: Events/Findings job state and the
    /// Bridge report. They name obligations adoption records but does not invent a completion for.
    retained_outputs: Value,
    /// Artifacts the legacy folder does not retain: derived read-only at apply (R415).
    derived: Vec<Value>,
    /// Scope this folder cannot supply: adoption needs an explicit preparation (R417).
    missing: Vec<Value>,
}

/// One group's verified or missing output, from the actual Cards.
struct GroupFact {
    n: u64,
    anchor: String,
    state: &'static str,
    reason: String,
    anchor_fingerprint: String,
    cards: Vec<(String, String)>, // (address, authored fingerprint)
    lost_log_entries: Vec<String>,
    owned_source: &'static str,
    log_entries: usize,
    src_entries: usize,
}

/// One Model Card of the book, verified read-only from the archive.
struct ModelFact {
    address: String,
    unit: String,
    state: &'static str,
    reason: String,
    status: String,
    level: String,
    parts: usize,
    placeholders: Vec<String>,
    faults: BTreeMap<String, usize>,
    order: Vec<String>,
    derivations: Vec<String>,
    needs: Vec<Value>,
    chains: Value,
    plan_hash: String,
    plan_identity: &'static str,
    notes: Vec<String>,
    review_retained: bool,
    fingerprints: Vec<(String, String)>,
}

struct IndexRow {
    file: String,
    chars: i64,
    group: u64,
    anchor: String,
    chapter: String,
    headings: String,
}

fn file_record(path: &Path) -> Result<Value> {
    let path = std::fs::canonicalize(path)?;
    let bytes = std::fs::read(&path)?;
    Ok(json!({"path":path.to_string_lossy(),"sha256":canon::sha256_hex(&bytes),"bytes":bytes.len()}))
}

fn hash_file(path: &Path) -> Result<String> {
    Ok(canon::sha256_hex(&std::fs::read(path)?))
}

fn canonical(path: impl AsRef<Path>) -> Result<PathBuf> {
    Ok(std::fs::canonicalize(path)?)
}

/// A group file in the spellings `prepare` writes: `g01.md`, and `g100.md` above 99.
fn group_file(staging: &Path, n: u64) -> Result<Option<PathBuf>> {
    for name in [format!("g{n:02}.md"), format!("g{n:03}.md"), format!("g{n}.md")] {
        let path = staging.join("groups").join(&name);
        if path.is_file() {
            return Ok(Some(canonical(path)?));
        }
    }
    Ok(None)
}

fn optional_log(staging: &Path, prefix: &str, n: u64) -> Option<PathBuf> {
    [format!("{prefix}-g{n:02}.txt"), format!("{prefix}-g{n:03}.txt")]
        .iter()
        .map(|name| staging.join(name))
        .find(|path| path.is_file())
        .and_then(|path| canonical(path).ok())
}

/// The legacy preparation index: the book title, the frozen source it names, and one row per
/// section file with its group and anchor. It is the folder's only preparation record.
fn parse_index(text: &str) -> Result<(String, Option<String>, Vec<IndexRow>)> {
    let mut title = String::new();
    let mut source = None;
    let mut rows = Vec::new();
    for line in text.replace('\r', "").lines() {
        if title.is_empty() && let Some(rest) = line.trim().strip_prefix("# 全书索引：《") {
            title = rest.trim_end_matches('》').trim().to_string();
            continue;
        }
        if source.is_none() && let Some(at) = line.find("全书合并文本为 `") {
            let rest = &line[at + "全书合并文本为 `".len()..];
            if let Some(end) = rest.find('`') {
                source = Some(rest[..end].to_string());
            }
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // | `s01.md` | 3164 | 1 | `Key/1` | chapter | sections |
        if cells.len() != 8 || !cells[0].is_empty() || !cells[7].is_empty() {
            continue;
        }
        let file = cells[1].trim_matches('`').trim().to_string();
        let anchor = cells[4].trim_matches('`').trim().to_string();
        let (Ok(chars), Ok(group)) = (cells[2].parse::<i64>(), cells[3].parse::<u64>()) else {
            continue;
        };
        if !file.ends_with(".md") || !anchor.contains('/') {
            continue;
        }
        rows.push(IndexRow {
            file,
            chars,
            group,
            anchor,
            chapter: cells[5].to_string(),
            headings: cells[6].to_string(),
        });
    }
    if rows.is_empty() {
        bail!("the legacy preparation index has no section rows");
    }
    Ok((title, source, rows))
}

fn citation_key_of(anchor: &str) -> String {
    anchor.rsplit_once('/').map(|(key, _)| key.to_string()).unwrap_or_default()
}

/// The variant blueprint's profile: which instruction files the adopted run binds.
fn profile_of(variant: &str) -> Result<Value> {
    let blueprint: Value = serde_json::from_str(match variant {
        "econ" => include_str!("../../../../scripts/book-digest/workflow/zt-econ-digest.blueprint.json"),
        "math" => include_str!("../../../../scripts/book-digest/workflow/zt-math-digest.blueprint.json"),
        _ => bail!("unknown adoption variant `{variant}`"),
    })?;
    let profile = blueprint["consts"]["profile"].clone();
    if !profile.is_object() {
        bail!("the {variant} blueprint declares no profile");
    }
    Ok(profile)
}

fn blueprint_version(variant: &str) -> Result<(String, String)> {
    let base: Value = serde_json::from_str(include_str!(
        "../../../../scripts/book-digest/workflow/zt-digest-base.blueprint.json"
    ))?;
    let profile: Value = serde_json::from_str(match variant {
        "econ" => include_str!("../../../../scripts/book-digest/workflow/zt-econ-digest.blueprint.json"),
        "math" => include_str!("../../../../scripts/book-digest/workflow/zt-math-digest.blueprint.json"),
        _ => bail!("unknown adoption variant `{variant}`"),
    })?;
    Ok((s(&base["version"]), s(&profile["version"])))
}

/// The prefix artifact names the adopted run binds; the mathematics variant keeps its own.
fn prefix_artifacts(variant: &str) -> (&'static str, &'static str, &'static str) {
    match variant {
        "math" => ("math-prefix.md", "math-link-prefix.md", "dsh-math-link.zh.md"),
        _ => ("shared-prefix.md", "link-prefix.md", "dsh-link-instructions.zh.md"),
    }
}

// --- resolution ------------------------------------------------------------------------------------

/// A folder that cannot be reconciled from its own retained artifacts: the adoption names the
/// missing scope and writes no ledger (R417).
#[derive(Debug)]
struct MissingScope {
    what: String,
    reason: String,
}

impl std::fmt::Display for MissingScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.what, self.reason)
    }
}

impl std::error::Error for MissingScope {}

fn missing_scope(what: impl Into<String>, reason: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(MissingScope {
        what: what.into(),
        reason: reason.into(),
    })
}

fn resolve(out: &str, variant: &str) -> Result<Legacy> {
    let out = canonical(out).context("legacy staging folder does not exist")?;
    let index = out.join("sections").join("index.md");
    if !index.is_file() {
        return Err(missing_scope(
            "legacy preparation index",
            format!(
                "{} holds no sections/index.md, so the Citation key, the frozen source and the group structure cannot be resolved from the retained artifacts",
                out.display()
            ),
        ));
    }
    let (title, named_source, rows) = parse_index(&util::read_text(&index)?)?;
    let keys: BTreeSet<String> = rows.iter().map(|row| citation_key_of(&row.anchor)).collect();
    if keys.len() != 1 {
        return Err(missing_scope(
            "one Citation key in the legacy preparation index",
            format!("the index mixes Citation keys: {}", keys.into_iter().collect::<Vec<_>>().join(", ")),
        ));
    }
    let key = keys.into_iter().next().unwrap_or_default();
    if key.is_empty() || key.len() > 200 {
        return Err(missing_scope(
            "Citation key",
            "the legacy preparation index names no usable Citation key",
        ));
    }
    if !key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        return Err(missing_scope(
            "Citation key",
            format!("the legacy preparation index names an unusable Citation key `{key}`"),
        ));
    }
    let mut by_group: BTreeMap<u64, Vec<&IndexRow>> = BTreeMap::new();
    for row in &rows {
        by_group.entry(row.group).or_default().push(row);
    }
    let source = named_source
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| Some(out.join("book.zh.md")).filter(|path| path.is_file()))
        .ok_or_else(|| {
            missing_scope(
                "frozen source text",
                format!(
                    "{} retains no book.zh.md and the preparation index names no source that exists",
                    out.display()
                ),
            )
        })?;
    let source = canonical(source)?;
    let source_sha256 = hash_file(&source)?;
    let mut groups = Vec::new();
    let mut missing = Vec::new();
    for (n, members) in &by_group {
        let Some(file) = group_file(&out, *n)? else {
            missing.push(json!({
                "what":format!("group {n} file"),
                "reason":"the prepared group file is not in the staging folder, so this group cannot be re-run and its scope must be prepared explicitly",
            }));
            continue;
        };
        let mut sections = Vec::new();
        for row in members {
            let path = out.join("sections").join(&row.file);
            if !path.is_file() {
                return Err(missing_scope(
                    format!("group {n} section file {}", row.file),
                    "the prepared section the group file names is not in the staging folder",
                ));
            }
            sections.push((row.file.clone(), canonical(path)?));
        }
        let mut headings: Vec<String> = Vec::new();
        for row in members {
            for heading in row.headings.split('；') {
                let heading = heading.trim();
                if !heading.is_empty() && !headings.iter().any(|h| h == heading) {
                    headings.push(heading.to_string());
                }
            }
        }
        groups.push(LegacyGroup {
            n: *n,
            anchor: format!("{key}/{n}"),
            sha256: hash_file(&file)?,
            file,
            sections,
            chapter: members[0].chapter.clone(),
            headings,
            chars: members.iter().map(|row| row.chars).sum(),
            owned_log: optional_log(&out, "owned", *n),
            src_log: optional_log(&out, "src", *n),
        });
    }
    if groups.is_empty() {
        return Err(missing_scope("prepared group files", "the staging folder holds no prepared group file"));
    }
    let numbers: Vec<u64> = groups.iter().map(|group| group.n).collect();
    let expected: Vec<u64> = (1..=*numbers.iter().max().unwrap_or(&0)).collect();
    if numbers != expected {
        missing.push(json!({
            "what":"contiguous prepared groups",
            "reason":"the preparation index does not number its groups 1..N, so a missing group cannot be named or re-run",
        }));
    }
    let artifact = |name: &str| -> Result<Option<(PathBuf, String)>> {
        let path = out.join(name);
        if !path.is_file() {
            return Ok(None);
        }
        let path = canonical(path)?;
        let sha = hash_file(&path)?;
        Ok(Some((path, sha)))
    };
    let terms = artifact("terms.md")?;
    let figures = artifact("figures.json")?;
    let (shared, link, link_instructions) = prefix_artifacts(variant);
    let mut derived = Vec::new();
    for (name, reason) in [
        (
            shared,
            "the legacy folder retained no shared instruction prefix; the adopted run derives it from the frozen instruction files of its variant",
        ),
        (
            link,
            "the legacy folder retained no Link instruction prefix; the adopted run derives it from the frozen Link instruction file",
        ),
        (
            "events.json",
            "the legacy folder retained no Event candidate list; the adopted run records an empty candidate set and schedules no per-group Event enumeration",
        ),
        (
            "setup-calls.json",
            "the legacy folder retained no setup calls; the adopted run derives the list its Literature tree needs and writes no Card from it",
        ),
    ] {
        if !out.join(name).exists() {
            derived.push(json!({"artifact":name,"reason":reason}));
        }
    }
    if figures.is_none() {
        derived.push(json!({
            "artifact":"figures.json",
            "reason":"the legacy folder retained no figure list; the adopted run records an empty one and the Figures publication stays an open obligation",
        }));
    }
    let _ = link_instructions;
    let (retained, model_states) = retained_models(&out)?;
    let retained_outputs = retained_outputs(&out)?;
    Ok(Legacy {
        out,
        variant: variant.to_string(),
        key,
        title,
        source,
        source_sha256,
        index_sha256: hash_file(&index)?,
        index,
        groups,
        terms,
        figures,
        retained,
        model_states,
        retained_outputs,
        derived,
        missing,
    })
}

/// The retained model compiler state: every `models/<batch>/state.json` and the entry of
/// each Model Card it built (`<batch>/<slug>/plan.json` is that model's accepted plan).
fn retained_models(out: &Path) -> Result<(Vec<RetainedModel>, usize)> {
    let root = out.join("models");
    let mut batches = Vec::new();
    if root.join("state.json").is_file() {
        batches.push(canonical(&root)?);
    }
    if root.is_dir() {
        for entry in std::fs::read_dir(&root)? {
            let dir = entry?.path();
            if dir.is_dir() && dir.join("state.json").is_file() {
                batches.push(canonical(&dir)?);
            }
        }
    }
    batches.sort();
    let mut retained = Vec::new();
    for batch in &batches {
        let state = util::read_json(batch.join("state.json"))?;
        let Some(models) = state["models"].as_object() else {
            continue;
        };
        let mut addresses: Vec<&String> = models.keys().collect();
        addresses.sort();
        for address in addresses {
            let entry = &models[address];
            let slug = crate::model::compiler::slug_of(address);
            let plan_path = batch.join(&slug).join("plan.json");
            let plan = if plan_path.is_file() {
                let path = canonical(&plan_path)?;
                let sha = hash_file(&path)?;
                Some((path, sha))
            } else {
                None
            };
            let mut notes = Vec::new();
            let mut push = |text: &str| {
                if !text.trim().is_empty() && !notes.iter().any(|note| note == text) {
                    notes.push(text.to_string());
                }
            };
            for note in entry["notes"].as_array().into_iter().flatten() {
                if let Some(text) = note.as_str() {
                    push(text);
                }
            }
            if let Some(plan) = &plan {
                for note in util::read_json(&plan.0)?["notes"].as_array().into_iter().flatten() {
                    if let Some(text) = note.as_str() {
                        push(text);
                    }
                }
            }
            retained.push(RetainedModel {
                address: address.clone(),
                status: s(&entry["status"]),
                plan,
                notes,
                has_review: batch.join(&slug).join("source-review.json").is_file()
                    || entry["source_review"].is_object(),
            });
        }
    }
    let count = batches.len();
    Ok((retained, count))
}

// --- classification ---------------------------------------------------------------------------------

fn is_skeleton(body: &str) -> bool {
    body.contains(SKELETON_ANCHOR)
}

fn is_placeholder(body: &str) -> bool {
    body.replace('\r', "").split('\n').any(|line| line.trim() == "待写")
}

/// The Events/Findings compiler state a layer keeps beside its accepted plan. A job the plan
/// lists but the state neither applied nor marked partial never settled: adoption records that
/// fact and registers no completion for it (R415).
fn layer_jobs(out: &Path, name: &str) -> Result<Option<Value>> {
    let work = out.join(name);
    let plan_path = work.join("plan.json");
    let state_path = work.join("state.json");
    if !plan_path.is_file() && !state_path.is_file() {
        return Ok(None);
    }
    let planned = if plan_path.is_file() {
        util::read_json(&plan_path)?["jobs"].as_array().map(Vec::len).unwrap_or(0)
    } else {
        0
    };
    let state = if state_path.is_file() {
        util::read_json(&state_path)?
    } else {
        Value::Null
    };
    let applied = state["applied"].as_object().map(|jobs| jobs.len()).unwrap_or(0);
    let partial = state["partial"].as_object().map(|jobs| jobs.len()).unwrap_or(0);
    let settled = applied + partial;
    Ok(Some(json!({
        "work":plain(&work),
        "plan":if plan_path.is_file() { json!(true) } else { Value::Null },
        "state":if state_path.is_file() { json!(true) } else { Value::Null },
        "planned_jobs":planned,
        "applied_jobs":applied,
        "partial_jobs":partial,
        "pending_jobs":planned.saturating_sub(settled),
    })))
}

/// The command/provider outputs the folder still holds, as facts: the Event and Finding job
/// counts and the retained Bridge report. Adoption names them and never applies a Bridge
/// proposal or claims a settled job (R411/R415).
fn retained_outputs(out: &Path) -> Result<Value> {
    let bridge = out.join("bridge");
    let report = bridge.join("report.md");
    let proposal_plan = bridge.join("plan.json");
    let proposals = bridge.join("proposals.json");
    let mut outputs = Map::new();
    for name in ["events", "findings"] {
        outputs.insert(name.into(), layer_jobs(out, name)?.unwrap_or(Value::Null));
    }
    outputs.insert(
        "bridge".into(),
        if report.is_file() || proposal_plan.is_file() {
            json!({
                "work":plain(&bridge),
                "report":if report.is_file() {
                    let path = canonical(&report)?;
                    json!({"path":plain(&path),"sha256":hash_file(&path)?})
                } else { Value::Null },
                "proposals":if proposals.is_file() {
                    let path = canonical(&proposals)?;
                    json!({"path":plain(&path),"sha256":hash_file(&path)?})
                } else { Value::Null },
            })
        } else {
            Value::Null
        },
    );
    Ok(Value::Object(outputs))
}

fn log_lines(path: &Option<PathBuf>) -> Vec<String> {
    path.as_ref()
        .and_then(|path| util::read_text(path).ok())
        .map(|text| {
            text.replace('\r', "")
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// The group's written Cards, from the Cards the group actually produced. A skeleton anchor
/// is never one of them: the anchor alone proves no digest success (R411/T14).
fn group_fact(
    group: &LegacyGroup,
    cards: &HashMap<String, Value>,
    literature: &HashSet<String>,
    anchors: &HashMap<String, usize>,
) -> Result<GroupFact> {
    let anchor_card = cards.get(&group.anchor);
    let scope_prefix = format!("{}|", group.anchor);
    let mut written = Vec::new();
    for (address, card) in cards {
        if address == &group.anchor || !address.starts_with(&scope_prefix) {
            continue;
        }
        let category = zt_check::graph::category(card, literature, anchors);
        if !zt_check::graph::SUBSTANTIVE.contains(&category.as_str()) {
            continue;
        }
        let body = s(&card["body"]);
        if body.trim().is_empty() || is_placeholder(&body) {
            continue;
        }
        written.push((address.clone(), crate::calls::authored_fingerprint(card)));
    }
    written.sort();
    let entries = log_lines(&group.owned_log);
    let mut lost_log_entries = Vec::new();
    let mut logged_written = 0usize;
    for address in &entries {
        if !cards.contains_key(address) {
            lost_log_entries.push(address.clone());
        } else if written.iter().any(|(at, _)| at == address) {
            logged_written += 1;
        }
    }
    let src_entries = log_lines(&group.src_log).len();
    let anchor_body = anchor_card.map(|card| s(&card["body"])).unwrap_or_default();
    let anchor_present = anchor_card.is_some_and(|card| card["workspace"] == json!("agent"));
    let anchor_fingerprint = anchor_card
        .map(crate::calls::authored_fingerprint)
        .unwrap_or_default();
    let (state, reason, owned_source) = if !anchor_present {
        (
            "missing",
            "the group anchor is not an agent Card of the book".to_string(),
            "absent",
        )
    } else if !entries.is_empty() && !entries.iter().any(|at| at == &group.anchor) {
        (
            "incomplete",
            format!(
                "the owned log names {} Card(s) but not the group anchor, so the group never reached its checked completion",
                entries.len()
            ),
            "owned_log",
        )
    } else if written.is_empty() {
        (
            "incomplete",
            if is_skeleton(&anchor_body) {
                "a skeleton anchor exists and no substantive Card does: the group was prepared and never digested"
                    .to_string()
            } else {
                "the anchor carries no substantive Card of its group".to_string()
            },
            if entries.is_empty() { "absent" } else { "owned_log" },
        )
    } else if !lost_log_entries.is_empty() {
        (
            "incomplete",
            format!(
                "{} Card(s) named by the owned log no longer exist: {}",
                lost_log_entries.len(),
                lost_log_entries.join(", ")
            ),
            "owned_log",
        )
    } else {
        (
            "verified",
            format!(
                "{} substantive Card(s) of the group exist now{}",
                written.len(),
                if logged_written > 0 {
                    format!(", {logged_written} of them named by the owned log")
                } else {
                    String::new()
                }
            ),
            if entries.is_empty() { "reconstructed" } else { "owned_log" },
        )
    };
    Ok(GroupFact {
        n: group.n,
        anchor: group.anchor.clone(),
        state,
        reason,
        anchor_fingerprint,
        cards: written,
        lost_log_entries,
        owned_source,
        log_entries: entries.len(),
        src_entries,
    })
}

/// A readable unit key for one adopted Model Card. Adoption gives an imported legacy model
/// a fixed key (design §7.3).
fn model_key(address: &str) -> String {
    let mut key = String::from("adopted-");
    for c in address.chars() {
        key.push(if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            c
        } else {
            '-'
        });
    }
    key.truncate(150);
    key
}

/// The read-only verification of one Model Card: its actual parts, placeholders, build
/// faults, level and explanation need (R414). A retained plan supplies the plan identity;
/// without one the identity is the verified build itself, named as such.
fn model_fact(
    header: &str,
    model: &Value,
    body_of: &HashMap<String, String>,
    fingerprint_of: &HashMap<String, String>,
    retained: Option<&RetainedModel>,
) -> Result<ModelFact> {
    let order: Vec<Value> = model["build"]["order"].as_array().cloned().unwrap_or_default();
    let head_body = body_of.get(header).cloned().unwrap_or_else(|| s(&model["head"]["body"]));
    let mut placeholders = Vec::new();
    let head_written = !is_placeholder(&head_body) && !head_body.trim().is_empty();
    if !head_written {
        placeholders.push(header.to_string());
    }
    let mut members = vec![header.to_string()];
    let mut derivations = Vec::new();
    let mut needs = Vec::new();
    let mut parts = 0usize;
    for entry in &order {
        let address = s(&entry["address"]);
        if address.is_empty() || address == header || members.contains(&address) {
            continue;
        }
        members.push(address.clone());
        if s(&entry["line"]) != "shared" {
            parts += 1;
        }
        if entry["derivation"] == json!(true) && entry["line"] != json!("shared") {
            derivations.push(address.clone());
            let sections = crate::model::text::split_sections(body_of.get(&address).map(String::as_str).unwrap_or_default());
            let missing: Vec<&str> = ["思路", "解读"]
                .into_iter()
                .filter(|name| crate::model::text::section_text(&sections, name).is_empty())
                .collect();
            if !missing.is_empty() {
                needs.push(json!({"address":address,"missing":missing}));
            }
        }
    }
    // Placeholders come from the actual part bodies, never from the part count (I15).
    for address in members.iter().skip(1) {
        let body = body_of.get(address).map(String::as_str).unwrap_or_default();
        if is_placeholder(body) || body.trim().is_empty() {
            placeholders.push(address.clone());
        }
    }
    let mut faults = BTreeMap::new();
    for name in ["closure", "forward", "unanchored", "beyond_level", "reach_in"] {
        let count = model["build"][name].as_array().map(Vec::len).unwrap_or(0);
        if count > 0 {
            faults.insert(name.to_string(), count);
        }
    }
    let status = retained.map(|entry| entry.status.clone()).unwrap_or_default();
    let built_status = matches!(status.as_str(), "built" | "derived" | "reviewed");
    let (state, reason) = if !head_written && parts == 0 {
        ("incomplete", "the Model Card is still a skeleton and has no part".to_string())
    } else if parts == 0 {
        ("incomplete", "the Model Card has no part yet".to_string())
    } else if !placeholders.is_empty() {
        (
            "incomplete",
            format!(
                "{} Card(s) of the model still hold 待写: {}",
                placeholders.len(),
                placeholders.join(", ")
            ),
        )
    } else if !faults.is_empty() {
        (
            "incomplete",
            format!(
                "the build report names faults: {}",
                faults.iter().map(|(name, count)| format!("{name} {count}")).collect::<Vec<_>>().join(", ")
            ),
        )
    } else if retained.is_some() && !built_status {
        (
            "incomplete",
            format!("the retained compiler state records `{status}`, not a completed build"),
        )
    } else {
        (
            "verified",
            format!("{parts} part(s) of the model are built and none is 待写"),
        )
    };
    let (plan_hash, plan_identity) = match retained.and_then(|entry| entry.plan.as_ref()) {
        Some((_, sha)) => (sha.clone(), "retained_plan"),
        None => (
            canon::hash_value(&json!({
                "adopted_model":header,
                "build_order":order.iter().map(|entry| s(&entry["address"])).collect::<Vec<_>>(),
            })),
            "verified_build",
        ),
    };
    let fingerprints = members
        .iter()
        .filter_map(|address| fingerprint_of.get(address).map(|fingerprint| (address.clone(), fingerprint.clone())))
        .collect();
    Ok(ModelFact {
        address: header.to_string(),
        unit: format!("models/{}@1", model_key(header)),
        state,
        reason,
        status,
        level: s(&model["level"]),
        parts,
        placeholders,
        faults,
        order: order.iter().map(|entry| s(&entry["address"])).collect(),
        derivations,
        needs,
        chains: json!(explain_chains(model)?),
        plan_hash,
        plan_identity,
        notes: retained.map(|entry| entry.notes.clone()).unwrap_or_default(),
        review_retained: retained.is_some_and(|entry| entry.has_review),
        fingerprints,
    })
}

/// The note suggestions a retained plan or state really carries: each one must name the Link
/// it proposes, so a prose note stays a recorded fact instead of an invented Link (R415).
fn note_suggestions(facts: &[ModelFact]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for fact in facts {
        for (index, note) in fact.notes.iter().enumerate() {
            if !note.contains("[[") {
                continue;
            }
            let mut id = String::from("m-");
            for c in model_key(&fact.address).trim_start_matches("adopted-").chars() {
                id.push(if c.is_ascii_alphanumeric() || matches!(c, '.' | '_') { c } else { '-' });
            }
            id.push_str(&format!("-n{:02}", index + 1));
            if !seen.insert(id.clone()) {
                continue;
            }
            out.push(json!({"id":id,"card":fact.address,"note":note}));
        }
    }
    out.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    out
}

// --- the plan ---------------------------------------------------------------------------------------

fn plan_document(
    legacy: &Legacy,
    archive: &Value,
    revision: u64,
    literature: &Value,
    group_facts: &[GroupFact],
    model_facts: &[ModelFact],
    notes: &[Value],
) -> Result<Value> {
    let profile = profile_of(&legacy.variant)?;
    let (base_version, variant_version) = blueprint_version(&legacy.variant)?;
    let mut missing = legacy.missing.clone();
    let mut verified = Vec::new();
    let mut incomplete = Vec::new();
    let mut unknown = Vec::new();
    for fact in group_facts {
        let unit = format!("digest/g{:03}@1", fact.n);
        match fact.state {
            "verified" => verified.push(json!({"unit":unit,"kind":"group","cards":fact.cards.len()})),
            "missing" => {
                missing.push(json!({
                    "what":format!("group {} anchor", fact.n),
                    "unit":unit,
                    "reason":format!("the book tree holds no agent Card at `{}`: the group cannot be resumed under a guessed anchor", fact.anchor),
                }));
                incomplete.push(json!({"unit":unit,"kind":"group","reason":fact.reason}));
            }
            _ => incomplete.push(json!({"unit":unit,"kind":"group","reason":fact.reason})),
        }
        if !fact.lost_log_entries.is_empty() {
            unknown.push(json!({
                "what":"owned Card named by a retained log",
                "unit":unit,
                "detail":fact.lost_log_entries,
                "reason":"the log names a Card the archive no longer holds; adoption adds no completion for it",
            }));
        }
        if fact.log_entries == 0 {
            unknown.push(json!({
                "what":"owned log",
                "unit":unit,
                "reason":"the folder retained no owned log for this group; adoption reconstructed the group's output read-only from what the anchor's scope holds now",
            }));
        }
    }
    for fact in model_facts {
        match fact.state {
            "verified" => verified.push(json!({"unit":fact.unit,"kind":"model","address":fact.address})),
            "unknown" => {
                missing.push(json!({
                    "what":format!("Model Card {}", fact.address),
                    "unit":fact.unit,
                    "reason":fact.reason,
                }));
                incomplete.push(json!({"unit":fact.unit,"kind":"model","address":fact.address,"reason":fact.reason}));
            }
            _ => incomplete.push(json!({"unit":fact.unit,"kind":"model","address":fact.address,"reason":fact.reason})),
        }
        if !fact.review_retained {
            unknown.push(json!({
                "what":"Model source review",
                "unit":fact.unit,
                "address":fact.address,
                "reason":"the folder retained no checked source-review artifact for this model; adoption claims no review and the whole-book Card cycle owns the review",
            }));
        }
        if fact.state == "verified" && !fact.needs.is_empty() {
            incomplete.push(json!({
                "unit":format!("explain/{}@1", model_key(&fact.address)),
                "kind":"explain",
                "address":fact.address,
                "reason":format!("{} own Derivation part(s) still lack 思路 or 解读", fact.needs.len()),
            }));
        }
        if fact.state == "verified" && fact.plan_identity != "retained_plan" {
            unknown.push(json!({
                "what":"accepted model plan",
                "unit":fact.unit,
                "address":fact.address,
                "reason":"the folder retained no accepted plan for this model; the verification's plan identity is the verified build itself, named as such",
            }));
        }
    }
    let prose_notes = model_facts
        .iter()
        .map(|fact| fact.notes.iter().filter(|note| !note.contains("[[")).count())
        .sum::<usize>();
    if prose_notes > 0 {
        unknown.push(json!({
            "what":"retained model note",
            "count":prose_notes,
            "reason":"a retained note that names no Link cannot become a note-suggestion unit; it stays a recorded fact instead of an invented Link",
        }));
    }
    if legacy.terms.is_none() {
        unknown.push(json!({"what":"term table","unit":"terms/book@1","reason":"the folder retained no terms.md"}));
    }
    if legacy.model_states == 0 {
        unknown.push(json!({
            "what":"model compiler state",
            "reason":"the folder retained no model compiler state, so no retained model plan, note suggestion or historic review exists",
        }));
    }
    // R415: the retained Event/Finding job state and Bridge report are named facts. Adoption
    // registers no completion for them and applies no Bridge proposal.
    for name in ["events", "findings"] {
        let layer = &legacy.retained_outputs[name];
        if layer["pending_jobs"].as_u64().is_some_and(|pending| pending > 0) {
            unknown.push(json!({
                "what":format!("pending {name} job"),
                "detail":{"planned_jobs":layer["planned_jobs"],"applied_jobs":layer["applied_jobs"],"partial_jobs":layer["partial_jobs"],"pending_jobs":layer["pending_jobs"]},
                "reason":format!("the retained {name} plan lists jobs whose state is neither applied nor partial; adoption registers no completion for them, and the whole-book Card cycle owns the Cards they would have written"),
            }));
        }
    }
    if legacy.retained_outputs["bridge"]["report"].is_object() {
        unknown.push(json!({
            "what":"retained Bridge report",
            "unit":"bridge/book@1",
            "detail":legacy.retained_outputs["bridge"]["report"],
            "reason":"the Bridge publication is registered open; adoption applies no proposal without its explicit acceptance, so the immutable report stays a review decision",
        }));
    }
    let mut proposal = Vec::new();
    for fact in group_facts {
        proposal.push(json!({
            "unit":format!("digest/g{:03}@1", fact.n),
            "layer":"digest",
            "state":if fact.state == "verified" { "done" } else { "registered" },
            "reason":fact.reason,
        }));
    }
    for fact in model_facts {
        proposal.push(json!({
            "unit":fact.unit,
            "layer":"models",
            "state":if fact.state == "verified" { "done" } else { "registered" },
            "reason":fact.reason,
        }));
        if fact.state == "verified" && !fact.needs.is_empty() {
            proposal.push(json!({
                "unit":format!("explain/{}@1", model_key(&fact.address)),
                "layer":"explain",
                "state":"registered",
                "reason":format!("explanation of {} Derivation part(s)", fact.needs.len()),
            }));
        }
    }
    for note in notes {
        proposal.push(json!({
            "unit":format!("notes/{}@1", s(&note["id"])),
            "layer":"notes",
            "state":"registered",
            "reason":"recovered note suggestion from a retained model plan",
        }));
    }
    let adopted = group_facts.iter().filter(|fact| fact.state == "verified").count()
        + model_facts.iter().filter(|fact| fact.state == "verified").count();
    let unresolved = proposal.iter().filter(|unit| unit["state"] == "registered").count();
    let mut by_layer: BTreeMap<String, u64> = BTreeMap::new();
    for unit in &proposal {
        *by_layer.entry(s(&unit["layer"])).or_default() += 1;
    }
    let ready = missing.is_empty();
    Ok(json!({
        "schema_version":1,
        "kind":"adoption_plan",
        "status":if ready {"ready"} else {"insufficient_evidence"},
        "out":legacy.out.to_string_lossy(),
        "variant":legacy.variant,
        "created_at":canon::utc_now_o(),
        "tool":format!("zt-digest {}", env!("CARGO_PKG_VERSION")),
        "archive":{
            "id":archive["archive_identity"]["id"],
            "path":archive["archive_identity"]["path"],
            "revision":revision,
        },
        "citation_key":legacy.key,
        "title":legacy.title,
        "identity":{
            "citation_key":legacy.key,
            "citation_key_source":"sections/index.md",
            "variant":legacy.variant,
            "variant_source":"explicit adoption argument",
            "archive_identity":archive["archive_identity"],
            "literature_card":{
                "address":literature["address"],
                "kind":literature["kind"],
                "workspace":literature["workspace"],
                "bibtex_sha256":canon::sha256_hex(s(&literature["bibtex"]).as_bytes()),
            },
            "contract":{
                "origin":"adopted",
                "base_version":base_version,
                "variant_version":variant_version,
                "artifact_schema":1,
                "instructions":profile["instructions"],
            },
        },
        "evidence":{
            "index":file_record(&legacy.index)?,
            "index_sha256":legacy.index_sha256,
            "source":file_record(&legacy.source)?,
            "source_sha256":legacy.source_sha256,
            "groups":legacy.groups.len(),
            "model_states":legacy.model_states,
            "retained_outputs":legacy.retained_outputs,
            "retained_models":legacy.retained.len(),
            "terms":legacy.terms.as_ref().map(|(path, sha)| json!({"path":plain(path),"sha256":sha})).unwrap_or(Value::Null),
            "figures":legacy.figures.as_ref().map(|(path, sha)| json!({"path":plain(path),"sha256":sha})).unwrap_or(Value::Null),
        },
        "groups":group_facts.iter().map(|fact| {
            let group = legacy.groups.iter().find(|group| group.n == fact.n).expect("group evidence");
            json!({
                "n":fact.n,
                "anchor":fact.anchor,
                "file":group.file.to_string_lossy(),
                "sha256":group.sha256,
                "chapter":group.chapter,
                "headings":group.headings,
                "chars":group.chars,
                "sections":group.sections.iter().map(|(name, path)| json!({"name":name,"path":path.to_string_lossy()})).collect::<Vec<_>>(),
                "state":fact.state,
                "reason":fact.reason,
                "cards":fact.cards.iter().map(|(address, _)| address).collect::<Vec<_>>(),
                "substantive":fact.cards.len(),
                "owned_log":group.owned_log.as_ref().map(|path| path.to_string_lossy().to_string()),
                "source_log":group.src_log.as_ref().map(|path| path.to_string_lossy().to_string()),
                "log_entries":fact.log_entries,
                "source_entries":fact.src_entries,
                "owned_source":fact.owned_source,
            })
        }).collect::<Vec<_>>(),
        "models":model_facts.iter().map(|fact| json!({
            "address":fact.address,
            "unit":fact.unit,
            "state":fact.state,
            "reason":fact.reason,
            "status":fact.status,
            "level":fact.level,
            "parts":fact.parts,
            "placeholders":fact.placeholders,
            "faults":fact.faults,
            "derivations":fact.derivations,
            "explanation_needs":fact.needs,
            "plan_hash":fact.plan_hash,
            "plan_identity":fact.plan_identity,
            "review_retained":fact.review_retained,
            "note_suggestions":fact.notes.iter().filter(|note| note.contains("[[")).count(),
            "retained_notes":fact.notes.len(),
        })).collect::<Vec<_>>(),
        "notes":notes,
        "targeted":proposal.iter().filter(|unit| unit["state"] == "registered").cloned().collect::<Vec<_>>(),
        "derived":legacy.derived,
        "missing":missing,
        "classification":{"verified":verified,"incomplete":incomplete,"unknown":unknown},
        "proposal":{
            "units":proposal,
            "adopted":adopted,
            "unresolved":unresolved,
            "by_layer":by_layer,
        },
        "apply":{
            "allowed":ready,
            "reason":if ready {String::new()} else {"the plan names missing scope: adoption writes no ledger until that scope is prepared explicitly".to_string()},
        },
    }))
}

/// The proposal's identity: the units and the identity facts, so `-Apply` refuses a plan the
/// evidence no longer supports.
fn proposal_hash(plan: &Value) -> String {
    canon::hash_value(&json!({
        "identity":plan["identity"],
        "evidence":plan["evidence"],
        "groups":plan["groups"],
        "models":plan["models"],
        "notes":plan["notes"],
        "proposal":plan["proposal"],
        "derived":plan["derived"],
        "missing":plan["missing"],
    }))
}

// --- reading the archive -----------------------------------------------------------------------------

struct Snapshot {
    archive: Value,
    revision: u64,
    literature: Value,
    cards: HashMap<String, Value>,
}

fn read_archive(legacy: &Legacy, zt: &mut Zt) -> Result<Snapshot> {
    let archive = zt.archive_view()?;
    let literature = zt
        .call("get_card", json!({"at":legacy.key,"view":"text"}))
        .map_err(|error| {
            missing_scope(
                format!("Literature Card `{}`", legacy.key),
                format!(
                    "the archive holds no Literature Card for the Citation key the legacy preparation index names ({error:#})"
                ),
            )
        })?;
    if literature["kind"] != json!("literature") || literature["workspace"] != json!("agent") {
        return Err(missing_scope(
            format!("agent Literature Card `{}`", legacy.key),
            "the legacy folder names another book: the Card at that key is not an agent Literature Card",
        ));
    }
    let tree = zt.call("get_tree", json!({"tree":legacy.key}))?;
    let revision = tree["archive_revision"]
        .as_u64()
        .context("incomplete tree view: no archive revision")?;
    let addresses: Vec<String> = tree["cards"]
        .as_array()
        .context("incomplete tree view")?
        .iter()
        .map(|card| s(&card["address"]))
        .filter(|address| !address.is_empty())
        .collect();
    let read = zt.cards(&addresses, "text")?;
    if read.archive_revision != revision {
        bail!("archive changed while reading the book; retry");
    }
    let mut cards = HashMap::new();
    for card in read.cards {
        let address = s(&card["address"]);
        if card["workspace"] != json!("agent") {
            bail!("the book tree `{}` holds a Card outside the agent Workspace: {address}", legacy.key);
        }
        cards.insert(address, card);
    }
    Ok(Snapshot { archive, revision, literature, cards })
}

fn body_map(cards: &HashMap<String, Value>) -> HashMap<String, String> {
    cards.iter().map(|(address, card)| (address.clone(), s(&card["body"]))).collect()
}

fn fingerprint_map(cards: &HashMap<String, Value>) -> HashMap<String, String> {
    cards
        .iter()
        .map(|(address, card)| (address.clone(), crate::calls::authored_fingerprint(card)))
        .collect()
}

/// Every Model Card of the book, in address order: its parts and their bodies may lie
/// outside the book tree, so they are read in one more bounded batch.
fn model_facts(legacy: &Legacy, snapshot: &Snapshot, zt: &mut Zt) -> Result<Vec<ModelFact>> {
    let literature: HashSet<String> = HashSet::from([legacy.key.clone()]);
    let anchors: HashMap<String, usize> = HashMap::from([(legacy.key.clone(), legacy.groups.len())]);
    let mut headers: Vec<String> = snapshot
        .cards
        .iter()
        .filter(|(_, card)| zt_check::graph::category(card, &literature, &anchors) == "model")
        .map(|(address, _)| address.clone())
        .collect();
    headers.sort();
    let mut models = Vec::new();
    let mut bodies = body_map(&snapshot.cards);
    let mut fingerprints = fingerprint_map(&snapshot.cards);
    let mut extra: Vec<String> = Vec::new();
    for header in &headers {
        let model = zt.call("get_model", json!({"at":header}))?;
        for entry in model["build"]["order"].as_array().into_iter().flatten() {
            let address = s(&entry["address"]);
            if !address.is_empty() && !bodies.contains_key(&address) && !extra.contains(&address) {
                extra.push(address);
            }
        }
        models.push((header.clone(), model));
    }
    if !extra.is_empty() {
        let read = zt.cards(&extra, "text")?;
        let mut cards = HashMap::new();
        for card in read.cards {
            cards.insert(s(&card["address"]), card);
        }
        bodies.extend(body_map(&cards));
        fingerprints.extend(fingerprint_map(&cards));
    }
    let mut facts = Vec::new();
    for (header, model) in models {
        let retained = legacy.retained.iter().find(|entry| entry.address == header);
        facts.push(model_fact(&header, &model, &bodies, &fingerprints, retained)?);
    }
    Ok(facts)
}

fn analyse(
    legacy: &Legacy,
    zt: &mut Zt,
) -> Result<(Value, Snapshot, Vec<GroupFact>, Vec<ModelFact>, Vec<Value>)> {
    let snapshot = read_archive(legacy, zt)?;
    let literature_set: HashSet<String> = HashSet::from([legacy.key.clone()]);
    let anchors: HashMap<String, usize> = HashMap::from([(legacy.key.clone(), legacy.groups.len())]);
    let mut group_facts = Vec::new();
    for group in &legacy.groups {
        group_facts.push(group_fact(group, &snapshot.cards, &literature_set, &anchors)?);
    }
    let model_facts = model_facts(legacy, &snapshot, zt)?;
    let notes = note_suggestions(&model_facts);
    let plan = plan_document(
        legacy,
        &snapshot.archive,
        snapshot.revision,
        &snapshot.literature,
        &group_facts,
        &model_facts,
        &notes,
    )?;
    Ok((plan, snapshot, group_facts, model_facts, notes))
}

// --- the prepared manifest ---------------------------------------------------------------------------

/// The artifacts the legacy folder does not retain, derived read-only from what it does
/// retain. The plan lists each one, so a reader can tell an adopted artifact from a
/// prepared one.
fn derive_artifacts(legacy: &Legacy) -> Result<()> {
    let profile = profile_of(&legacy.variant)?;
    let (shared, link, link_instructions) = prefix_artifacts(&legacy.variant);
    let staging = legacy.out.to_string_lossy().to_string();
    let index = legacy.index.to_string_lossy().to_string();
    let source = legacy.source.to_string_lossy().to_string();
    let title = if legacy.title.trim().is_empty() {
        legacy.key.clone()
    } else {
        legacy.title.clone()
    };
    let mut outline = String::new();
    for group in &legacy.groups {
        outline.push_str(&format!(
            "- 第 {} 组 `{}`：{} | {}\n",
            group.n,
            group.anchor,
            group.chapter,
            group.headings.join("；")
        ));
    }
    let outline = outline.trim_end().to_string();
    let fill: [(&str, &str); 7] = [
        ("{{KEY}}", &legacy.key),
        ("{{TITLE}}", &title),
        ("{{HELPER}}", "zt-digest call"),
        ("{{STAGING}}", &staging),
        ("{{INDEX}}", &index),
        ("{{SOURCE}}", &source),
        ("{{OUTLINE}}", &outline),
    ];
    let scripts = prepare::scripts_dir();
    for (artifact, files) in [
        (shared, s(&profile["instructions"])),
        (link, link_instructions.to_string()),
    ] {
        let path = legacy.out.join(artifact);
        if path.exists() {
            continue;
        }
        let prefix = prepare::build_prefix(&scripts, &files, &fill)?;
        util::write_atomic(&path, &prefix)?;
    }
    let events = legacy.out.join("events.json");
    if !events.exists() {
        util::write_atomic(&events, &ps::json_pretty(&json!([])))?;
    }
    let figures = legacy.out.join("figures.json");
    if !figures.exists() {
        util::write_atomic(&figures, &ps::json_pretty(&json!({"file_id":0,"figures":[]})))?;
    }
    let setup = legacy.out.join("setup-calls.json");
    if !setup.exists() {
        let mut calls = vec![json!({"tool":"create_literature","arguments":{
            "workspace":"agent",
            "bibtex":"（adopted: the Literature tree exists）",
            "body":"（全书索引待整理。）",
        }})];
        for group in &legacy.groups {
            let at = if group.n == 1 {
                legacy.key.clone()
            } else {
                format!("{}/{}", legacy.key, group.n - 1)
            };
            calls.push(json!({"tool":"create_successor","arguments":{
                "at":at,
                "kind":"direct",
                "title":format!("第 {} 组：{}", group.n, group.headings.join("；")),
                "body":"（adopted: the group anchor exists）",
            }}));
        }
        util::write_atomic(&setup, &ps::json_pretty(&Value::Array(calls)))?;
    }
    Ok(())
}

/// A path as a manifest states it: without the extended-length prefix, so the tooling that
/// joins `sections/<name>` onto it keeps one spelling Windows accepts.
fn plain(path: &Path) -> String {
    crate::report::plain_path(&path.to_string_lossy())
}

/// The prepared manifest the adopted run binds. Its contract is the current workflow
/// contract, recorded as adopted: the legacy folder carries no contract of its own, and
/// adoption never pretends it did.
fn prepared_manifest(legacy: &Legacy, snapshot: &Snapshot) -> Result<Value> {
    let profile = profile_of(&legacy.variant)?;
    let staging = plain(&legacy.out);
    let (shared, link, _) = prefix_artifacts(&legacy.variant);
    let mut groups = Vec::new();
    for group in &legacy.groups {
        groups.push(json!({
            "n":group.n,
            "id":format!("g{:02}", group.n),
            "anchor":group.anchor,
            "chapter":group.chapter,
            "headings":group.headings,
            "chars":group.chars,
            "file":plain(&group.file),
            "sections":group.sections.iter().map(|(name, _)| json!(name)).collect::<Vec<_>>(),
        }));
    }
    let mut manifest = Map::new();
    manifest.insert("citation_key".into(), json!(legacy.key));
    manifest.insert("book_title".into(), json!(legacy.title));
    manifest.insert("staging".into(), json!(staging));
    manifest.insert("source".into(), json!(plain(&legacy.source)));
    manifest.insert("source_sha256".into(), json!(legacy.source_sha256));
    manifest.insert("file_id".into(), json!(0));
    manifest.insert("book".into(), json!(plain(&legacy.out.join("book.zh.md"))));
    manifest.insert("index".into(), json!(plain(&legacy.index)));
    manifest.insert("shared_prefix".into(), json!(plain(&legacy.out.join(shared))));
    manifest.insert("link_prefix".into(), json!(plain(&legacy.out.join(link))));
    manifest.insert("figures_file".into(), json!(plain(&legacy.out.join("figures.json"))));
    manifest.insert("groups".into(), Value::Array(groups));
    manifest.insert(
        "options".into(),
        json!({
            "book_title":legacy.title,
            "bibtex_sha256":canon::sha256_hex(s(&snapshot.literature["bibtex"]).as_bytes()),
            "start_heading":"",
            "end_heading":"",
            "skip_headings":[],
            "max_chars":20000,
            "max_units":3,
            "min_unit_chars":1000,
            "min_figure_bytes":3000,
            "max_event_candidates":12,
            "instructions":profile["instructions"],
            "no_event_candidates":true,
            "no_figures":false,
            "run_options":run_options(&legacy.variant, false, None)?,
        }),
    );
    manifest.insert(
        "adoption".into(),
        json!({
            "schema_version":1,
            "origin":"legacy_staging",
            "variant":legacy.variant,
            "index":plain(&legacy.index),
            "index_sha256":legacy.index_sha256,
            "source_sha256":legacy.source_sha256,
            "derived":legacy.derived,
            "missing":legacy.missing,
        }),
    );
    Ok(Value::Object(manifest))
}

// --- apply -------------------------------------------------------------------------------------------

fn adopted_group_result(fact: &GroupFact, plan: &Value, revision: u64) -> Value {
    let mut fingerprints: BTreeMap<String, String> = BTreeMap::new();
    fingerprints.insert(fact.anchor.clone(), fact.anchor_fingerprint.clone());
    for (address, fingerprint) in &fact.cards {
        fingerprints.insert(address.clone(), fingerprint.clone());
    }
    let mut outputs = vec![json!("adoption")];
    for address in fingerprints.keys() {
        outputs.push(json!(format!("card/{address}")));
    }
    json!({
        "schema_version":1,
        "kind":"group_adoption",
        "unit":format!("digest/g{:03}@1", fact.n),
        "state":"done",
        "disposition":"adopted",
        "complete":true,
        "applied":false,
        "reused":false,
        "emitted":[],
        "outputs":outputs,
        "anchor":{"address":fact.anchor,"fingerprint":fact.anchor_fingerprint},
        "owned":fact.cards.iter().map(|(address, _)| address).collect::<Vec<_>>(),
        "substantive":fact.cards.iter().map(|(address, _)| address).collect::<Vec<_>>(),
        "card_fingerprints":fingerprints,
        "trees":[plan["citation_key"]],
        "evidence":{
            "owned_source":fact.owned_source,
            "owned_log_entries":fact.log_entries,
            "source_log_entries":fact.src_entries,
            "reason":fact.reason,
        },
        "verification":{
            "mode":"read_only",
            "at":plan["created_at"],
            "tool":plan["tool"],
            "archive_revision":revision,
            "note":"adoption verified the current Cards; no historic command or SELF-CHECK result is claimed",
        },
    })
}

fn adopted_model_result(
    fact: &ModelFact,
    unit: &str,
    input_hash: &str,
    emitted: Vec<Value>,
    plan: &Value,
    revision: u64,
) -> Value {
    let mut outputs = vec![json!("adoption")];
    for (address, _) in &fact.fingerprints {
        outputs.push(json!(format!("card/{address}")));
    }
    json!({
        "schema_version":1,
        "kind":"model_adoption",
        "unit":unit,
        "state":"done",
        "disposition":"adopted",
        "complete":true,
        "emitted":emitted,
        "outputs":outputs,
        "model":fact.address,
        "plan_hash":fact.plan_hash,
        "plan_identity":fact.plan_identity,
        "unit_input_hash":input_hash,
        "plan_parts":fact.parts,
        "level":fact.level,
        "parts":fact.parts,
        "build":{"order":fact.order.iter().map(|address| json!({"address":address})).collect::<Vec<_>>(),"faults":fact.faults},
        "explanation":{
            "status":if fact.needs.is_empty() {"explained"} else {"required"},
            "derivations":fact.derivations.len(),
            "needs":fact.needs,
            "chains":fact.chains,
        },
        "source_review":{"retained":fact.review_retained},
        "card_fingerprints":fact.fingerprints.iter().cloned().collect::<BTreeMap<_,_>>(),
        "trees":[plan["citation_key"]],
        "evidence":{"status":fact.status,"reason":fact.reason},
        "verification":{
            "mode":"read_only",
            "at":plan["created_at"],
            "tool":plan["tool"],
            "archive_revision":revision,
            "note":"adoption verified the model's actual parts; historic compiler command evidence is retained separately and is not claimed here",
        },
    })
}

fn group_of_address(address: &str) -> u64 {
    let after = address.split_once('/').map(|(_, rest)| rest).unwrap_or(address);
    let number: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    number.parse().unwrap_or(0)
}

fn apply_plan(
    p: &Prepared,
    plan: &Value,
    group_facts: &[GroupFact],
    model_facts: &[ModelFact],
    notes: &[Value],
    plan_record: &Value,
) -> Result<Value> {
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let run_id = meta(&tx, "run_id")?;
    let now = canon::utc_now_o();
    let revision = plan["archive"]["revision"].as_u64().unwrap_or(0);
    let plan_hash = s(&plan_record["sha256"]);
    let mut adopted = Vec::new();
    let mut targeted = Vec::new();
    let mut events = 0usize;
    for fact in group_facts.iter().filter(|fact| fact.state == "verified") {
        let unit = format!("digest/g{:03}@1", fact.n);
        let result = adopted_group_result(fact, plan, revision);
        tx.execute(
            "UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL,owner=NULL WHERE id=?1",
            params![unit, canon::canonical_json(&result), now],
        )?;
        output(
            &tx,
            &unit,
            "adoption",
            "file",
            field(plan_record, "path")?,
            &plan_hash,
            &json!({"kind":"adoption_plan"}),
        )?;
        for (address, fingerprint) in &fact.cards {
            output(
                &tx,
                &unit,
                &format!("card/{address}"),
                "card",
                address,
                fingerprint,
                &json!({"address":address}),
            )?;
        }
        event(
            &tx,
            &unit,
            None,
            None,
            "adopted",
            &json!({
                "evidence":result["evidence"],
                "cards":fact.cards.len(),
                "verification":result["verification"],
                "plan":plan_record,
            }),
        )?;
        events += 1;
        adopted.push(json!({"unit":unit,"kind":"group","cards":fact.cards.len()}));
    }
    if let Some((path, sha)) = &prepare_terms(p) {
        let unit = "terms/book@1".to_string();
        let result = json!({
            "schema_version":1,
            "kind":"terms_adoption",
            "unit":unit,
            "state":"done",
            "disposition":"adopted",
            "complete":true,
            "emitted":[],
            "outputs":["adoption"],
            "term_table":{"path":plain(path),"sha256":sha},
            "verification":{"mode":"read_only","at":plan["created_at"],"tool":plan["tool"],"archive_revision":revision},
        });
        tx.execute(
            "UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",
            params![unit, canon::canonical_json(&result), now],
        )?;
        output(
            &tx,
            &unit,
            "adoption",
            "file",
            field(plan_record, "path")?,
            &plan_hash,
            &json!({"kind":"adoption_plan"}),
        )?;
        event(
            &tx,
            &unit,
            None,
            None,
            "adopted",
            &json!({"evidence":{"term_table":{"path":path.to_string_lossy(),"sha256":sha}},"verification":result["verification"],"plan":plan_record}),
        )?;
        events += 1;
        adopted.push(json!({"unit":unit,"kind":"terms"}));
    }
    for fact in model_facts {
        let inputs = if fact.state == "verified" {
            json!({
                "schema_version":1,
                "kind":"model_adopted",
                "model":fact.address,
                "level":fact.level,
                "plan_hash":fact.plan_hash,
                "plan_identity":fact.plan_identity,
                "build_order":fact.order,
                "explanation_needs":fact.needs,
            })
        } else {
            json!({
                "schema_version":1,
                "kind":"model_adoption_need",
                "model":fact.address,
                "status":fact.status,
                "parts":fact.parts,
                "placeholders":fact.placeholders,
                "faults":fact.faults,
                "reason":fact.reason,
            })
        };
        let key = model_key(&fact.address);
        let registered = register(&tx, "models", &key, 1, &inputs, None, false)?;
        if registered != fact.unit {
            bail!("the registered Model unit id changed: {registered}");
        }
        let group_unit = format!("digest/g{:03}@1", group_of_address(&fact.address));
        let known: Option<i64> = tx
            .query_row("SELECT 1 FROM units WHERE id=?1", [&group_unit], |row| row.get(0))
            .optional()?;
        dependency(
            &tx,
            &fact.unit,
            if known.is_some() { &group_unit } else { "prepare/book@1" },
            "success",
        )?;
        let input_hash: String = tx.query_row(
            "SELECT input_hash FROM units WHERE id=?1",
            [&fact.unit],
            |row| row.get(0),
        )?;
        if fact.state == "verified" {
            // The explanation obligation is the adopted model's own emitted child, so the
            // record declares it exactly as a Model completion declares its Explain unit.
            let mut emitted = Vec::new();
            if !fact.needs.is_empty() {
                let explain = format!("explain/{key}@1");
                let answer = json!({
                    "schema_version":1,
                    "model":fact.address,
                    "unit":fact.unit,
                    "plan_hash":fact.plan_hash,
                    "parts":fact.needs.iter().map(|need| s(&need["address"])).collect::<Vec<_>>(),
                    "chains":fact.chains,
                });
                let id = register(&tx, "explain", &key, 1, &answer, Some(&fact.unit), false)?;
                if id != explain {
                    bail!("the registered explanation unit id changed: {id}");
                }
                dependency(&tx, &id, &fact.unit, "success")?;
                event(
                    &tx,
                    &id,
                    None,
                    None,
                    "registered",
                    &json!({
                        "producer":fact.unit,
                        "input_hash":canon::hash_value(&answer),
                        "recovered_from":"adopted Model verification",
                    }),
                )?;
                events += 1;
                emitted.push(json!({
                    "id":id,
                    "input_hash":canon::hash_value(&answer),
                    "dependencies":[{"unit":id,"needs":fact.unit,"condition":"success"}],
                }));
                targeted.push(json!({
                    "unit":explain,
                    "reason":format!("{} Derivation part(s) still lack 思路 or 解读", fact.needs.len()),
                }));
            }
            let result = adopted_model_result(fact, &fact.unit, &input_hash, emitted, plan, revision);
            tx.execute(
                "UPDATE units SET state='done',result_json=?2,completed_at=?3 WHERE id=?1",
                params![fact.unit, canon::canonical_json(&result), now],
            )?;
            output(
                &tx,
                &fact.unit,
                "adoption",
                "file",
                field(plan_record, "path")?,
                &plan_hash,
                &json!({"kind":"adoption_plan"}),
            )?;
            for (address, fingerprint) in &fact.fingerprints {
                output(
                    &tx,
                    &fact.unit,
                    &format!("card/{address}"),
                    "card",
                    address,
                    fingerprint,
                    &json!({"address":address}),
                )?;
            }
            event(
                &tx,
                &fact.unit,
                None,
                None,
                "adopted",
                &json!({
                    "evidence":{
                        "status":fact.status,
                        "parts":fact.parts,
                        "plan_identity":fact.plan_identity,
                        "source_review":fact.review_retained,
                    },
                    "verification":result["verification"],
                    "plan":plan_record,
                }),
            )?;
            events += 1;
            adopted.push(json!({"unit":fact.unit,"kind":"model","parts":fact.parts}));
        } else {
            event(
                &tx,
                &fact.unit,
                None,
                None,
                "registered",
                &json!({
                    "reason":fact.reason,
                    "placeholders":fact.placeholders,
                    "faults":fact.faults,
                    "recovered_from":"legacy model evidence",
                }),
            )?;
            events += 1;
            targeted.push(json!({"unit":fact.unit,"reason":fact.reason}));
        }
    }
    for note in notes {
        let id = s(&note["id"]);
        let unit = format!("notes/{id}@1");
        let inputs = json!({"id":id,"card":note["card"],"note":note["note"]});
        register(&tx, "notes", &id, 1, &inputs, None, false)?;
        dependency(&tx, &unit, "prepare/book@1", "success")?;
        event(
            &tx,
            &unit,
            None,
            None,
            "registered",
            &json!({
                "card":note["card"],
                "input_hash":canon::hash_value(&inputs),
                "recovered_from":"retained model plan",
            }),
        )?;
        events += 1;
        targeted.push(json!({"unit":unit,"reason":"recovered note suggestion"}));
    }
    let record = json!({
        "schema_version":1,
        "kind":"adoption_applied",
        "origin":"legacy_staging",
        "variant":plan["variant"],
        "applied_at":now,
        "tool":plan["tool"],
        "plan":{"path":plan_record["path"],"sha256":plan_hash,"proposal_hash":plan["proposal_hash"]},
        "archive_identity":plan["identity"]["archive_identity"],
        "citation_key":plan["citation_key"],
        "adopted":adopted,
        "targeted":targeted,
        "derived":plan["derived"],
        "missing":plan["missing"],
        "unknown":plan["classification"]["unknown"],
    });
    put_meta(&tx, "adoption", &record)?;
    tx.execute(
        "INSERT INTO events(unit_id,invocation,utc,event,facts_json) VALUES(NULL,1,?1,?2,?3)",
        params!["adopted_legacy_staging", canon::utc_now_o(), canon::canonical_json(&record)],
    )?;
    events += 1;
    event(
        &tx,
        "prepare/book@1",
        None,
        None,
        "adopted",
        &json!({
            "plan":plan_record,
            "derived":plan["derived"],
            "reason":"the prepared manifest of this run was written by legacy adoption",
        }),
    )?;
    events += 1;
    reconcile(&tx, now_seconds())?;
    tx.commit()?;
    Ok(json!({
        "schema_version":1,
        "kind":"adoption_applied",
        "status":"applied",
        "run_id":run_id,
        "invocation":1,
        "resume_count":0,
        "citation_key":plan["citation_key"],
        "variant":plan["variant"],
        "archive_identity":plan["identity"]["archive_identity"],
        "adopted":adopted,
        "targeted":targeted,
        "derived":plan["derived"],
        "events":events,
    }))
}

/// The Terms work the legacy folder really proves: a retained term table, and the book's
/// Terms Card in the agent Workspace.
fn prepare_terms(p: &Prepared) -> Option<(PathBuf, String)> {
    let path = p.out.join("terms.md");
    if !path.is_file() {
        return None;
    }
    let path = canonical(&path).ok()?;
    let sha = hash_file(&path).ok()?;
    Some((path, sha))
}

// --- command -----------------------------------------------------------------------------------------

fn plan_path(out: &Path) -> PathBuf {
    out.join("adoption").join("plan.json")
}

/// Writes the plan and returns its record, so a reader always gets a separately identifiable
/// artifact beside the applied ledger (R416).
fn write_plan(out: &Path, plan: &Value) -> Result<Value> {
    let path = plan_path(out);
    std::fs::create_dir_all(path.parent().context("adoption plan has no directory")?)?;
    let text = ps::json_pretty(plan);
    util::write_atomic(&path, &text)?;
    let record = json!({
        "path":canonical(&path)?.to_string_lossy(),
        "sha256":canon::sha256_hex(text.as_bytes()),
    });
    check_file(&record, "adoption plan")?;
    Ok(record)
}

/// The plan a read-only run left behind, if any: its exact value and its verified record.
fn reviewed_plan(out: &Path) -> Result<Option<(Value, Value)>> {
    let path = plan_path(out);
    if !path.is_file() {
        return Ok(None);
    }
    let text = util::read_text(&path)?;
    let record = json!({
        "path":canonical(&path)?.to_string_lossy(),
        "sha256":canon::sha256_hex(text.as_bytes()),
    });
    check_file(&record, "adoption plan")?;
    Ok(Some((canon::parse_json(&text)?, record)))
}

/// The header of an applied adoption the ledger already records, if any.
fn recorded_adoption(ledger: &Path) -> Result<Option<Value>> {
    if !ledger.is_file() {
        return Ok(None);
    }
    let conn = connect(ledger, false)?;
    let raw: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key='adoption'", [], |row| row.get(0))
        .optional()?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    Ok(Some(canon::parse_json(&raw)?))
}

/// R417: an old folder lacking enough source/group evidence keeps its artifacts and names what
/// is missing. The report is written read-only and no ledger is created.
fn missing_report(directory: &Path, variant: &str, apply: bool, error: &anyhow::Error) -> Result<i32> {
    let report = match error.downcast_ref::<MissingScope>() {
        Some(scope) => json!({
            "schema_version":1,
            "kind":"adoption_plan",
            "status":"insufficient_evidence",
            "out":directory.to_string_lossy(),
            "variant":variant,
            "created_at":canon::utc_now_o(),
            "tool":format!("zt-digest {}", env!("CARGO_PKG_VERSION")),
            "missing":[{"what":scope.what,"reason":scope.reason}],
            "derived":[],
            "notes":[],
            "targeted":[],
            "proposal":{"units":[],"adopted":0,"unresolved":0,"by_layer":{}},
            "classification":{"verified":[],"incomplete":[],"unknown":[]},
            "apply":{
                "allowed":false,
                "reason":"the plan names missing scope: adoption writes no ledger until that scope is prepared explicitly",
            },
        }),
        None => return Err(anyhow::anyhow!("{error:#}")),
    };
    let record = write_plan(directory, &report)?;
    let mut printed = report;
    printed["plan"] = record;
    util::print_line(&serde_json::to_string(&printed)?)?;
    let _ = apply;
    Ok(3)
}

pub(crate) fn cli(argv: &[String]) -> Result<i32> {
    let parsed = Args::parse(&argv[1..], &["Out", "Variant"], &["Apply"]);
    let a = match parsed {
        Ok(a) => a,
        Err(error) => {
            eprintln!("zt-digest ledger adopt: {error}");
            return Ok(2);
        }
    };
    let (out, variant, apply) = match (a.required("Out"), a.choice("Variant", &["econ", "math"])) {
        (Ok(out), Ok(variant)) if !out.trim().is_empty() => (out, variant, a.flag("Apply")),
        (Ok(_), Ok(_)) => {
            eprintln!("zt-digest ledger adopt: -Out is required");
            return Ok(2);
        }
        (Err(error), _) | (_, Err(error)) => {
            eprintln!("zt-digest ledger adopt: {error}");
            return Ok(2);
        }
    };
    if !a.positional.is_empty() {
        eprintln!("zt-digest ledger adopt: unexpected positional arguments");
        return Ok(2);
    }
    let directory = match canonical(&out) {
        Ok(directory) => directory,
        Err(_) => {
            eprintln!("zt-digest ledger adopt: legacy staging folder does not exist: {out}");
            return Ok(2);
        }
    };
    // R410: a prepared schema-1 folder is resumed, never adopted. The one exception is the
    // folder's own applied adoption, which is reported instead of being adopted again.
    let ledger = directory.join("ledger.sqlite3");
    if directory.join("manifest.json").is_file() {
        let manifest = util::read_json(directory.join("manifest.json"))?;
        if manifest["run_contract"]["schema_version"] == json!(1) {
            if let Some(recorded) = recorded_adoption(&ledger)? {
                if recorded["variant"] != json!(variant) {
                    bail!(
                        "adoption_variant_changed: this folder was adopted as `{}`, not `{variant}`; a different contract needs a new output directory",
                        s(&recorded["variant"])
                    );
                }
                let mut report = json!({
                    "schema_version":1,
                    "kind":if apply {"adoption_applied"} else {"adoption_plan"},
                    "status":"already_applied",
                    "out":directory.to_string_lossy(),
                    "variant":recorded["variant"],
                    "adoption":recorded,
                    "ledger":{"path":canonical(&ledger)?.to_string_lossy()},
                });
                if !apply {
                    report["apply"] = json!({"allowed":false,"reason":"this folder's ledger already holds the applied adoption; a changed reconciliation needs a new output directory"});
                }
                util::print_line(&serde_json::to_string(&report)?)?;
                return Ok(0);
            }
            bail!(
                "already_prepared: {} is a prepared run with a schema-1 contract; use `ledger status` or `ledger resume`, never adoption",
                directory.display()
            );
        }
    }
    let legacy = match resolve(&out, &variant) {
        Ok(legacy) => legacy,
        Err(error) => return missing_report(&directory, &variant, apply, &error),
    };
    let mut zt = crate::zt::start()?;
    let (plan, snapshot, group_facts, model_facts, notes) = match analyse(&legacy, &mut zt) {
        Ok(facts) => facts,
        Err(error) => return missing_report(&directory, &variant, apply, &error),
    };
    let mut plan = plan;
    plan["proposal_hash"] = json!(proposal_hash(&plan));
    // R416: the applied ledger names the plan the operator reviewed. A read-only run writes a
    // fresh report; an apply reuses that exact artifact, and refuses when the evidence moved.
    let plan_record = if apply {
        match reviewed_plan(&directory)? {
            Some((reviewed, record)) => {
                if reviewed["proposal_hash"] != plan["proposal_hash"] || reviewed["status"] != plan["status"] {
                    bail!(
                        "plan_changed: the prepared adoption plan no longer matches the evidence; read the new plan with `ledger adopt` before applying it"
                    );
                }
                plan = reviewed;
                record
            }
            None => write_plan(&directory, &plan)?,
        }
    } else {
        write_plan(&directory, &plan)?
    };
    plan["plan"] = plan_record.clone();
    if !apply {
        util::print_line(&serde_json::to_string(&plan)?)?;
        return Ok(if plan["status"] == json!("ready") { 0 } else { 3 });
    }
    if plan["status"] != json!("ready") {
        // The plan line stays last, as every tool's compact result does.
        eprintln!(
            "zt-digest ledger adopt: the plan names missing scope; no ledger was written. Prepare that scope explicitly, then adopt again."
        );
        util::print_line(&serde_json::to_string(&plan)?)?;
        return Ok(3);
    }
    // An applied adoption of this same proposal is idempotent; a different one is history a
    // second adoption must never overwrite.
    let ledger = directory.join("ledger.sqlite3");
    if let Some(recorded) = recorded_adoption(&ledger)? {
        if recorded["plan"]["proposal_hash"] != plan["proposal_hash"] {
            bail!(
                "adoption_changed: the ledger already records an adoption of a different proposal; a changed reconciliation needs a new output directory"
            );
        }
        let mut applied = recorded;
        applied["kind"] = json!("adoption_applied");
        applied["status"] = json!("already_applied");
        applied["plan"] = plan_record;
        applied["ledger"] = json!({"path":canonical(&ledger)?.to_string_lossy()});
        util::print_line(&serde_json::to_string(&applied)?)?;
        return Ok(0);
    }
    if ledger.is_file() {
        bail!(
            "ledger_exists: {} already holds a run ledger that records no adoption; adopt into a new output directory",
            ledger.display()
        );
    }
    derive_artifacts(&legacy)?;
    let manifest = prepared_manifest(&legacy, &snapshot)?;
    let contract = prepare::run_contract(
        manifest.as_object().context("the adopted manifest is not an object")?,
        &legacy.variant,
        &legacy.source_sha256,
        manifest["options"].clone(),
        snapshot.archive["archive_identity"].clone(),
    )?;
    let mut contract = contract;
    contract["settings"] = settings_of(&contract)?;
    let mut manifest = manifest;
    manifest["run_contract"] = contract;
    let manifest_text = ps::json_pretty(&manifest);
    let manifest_path = directory.join("manifest.json");
    util::write_atomic(&manifest_path, &manifest_text)?;
    // The folder is now a prepared run of its own: every later command verifies it the same
    // way, and the adoption is the only thing that differs.
    let p = Prepared::read_mode(&out, &mut zt, true)?;
    initialize(&p, true)?;
    let applied = apply_plan(&p, &plan, &group_facts, &model_facts, &notes, &plan_record)?;
    let record = json!({
        "schema_version":1,
        "kind":"adoption_record",
        "status":"applied",
        "plan":plan_record,
        "manifest":{
            "path":canonical(&manifest_path)?.to_string_lossy(),
            "sha256":canon::sha256_hex(manifest_text.as_bytes()),
        },
        "ledger":{"path":canonical(&ledger)?.to_string_lossy()},
        "applied_at":canon::utc_now_o(),
    });
    util::write_atomic(
        &directory.join("adoption").join("applied.json"),
        &ps::json_pretty(&record),
    )?;
    let mut printed = applied;
    printed["plan"] = record["plan"].clone();
    printed["manifest"] = record["manifest"].clone();
    printed["ledger"] = record["ledger"].clone();
    util::print_line(&serde_json::to_string(&printed)?)?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_legacy_index_names_the_key_the_source_and_each_groups_sections() {
        // R412: the retained index is the folder's preparation record.
        let text = "# 全书索引：《A Book》\n\n每个小节文件位于 `D:\\o\\sections\\`；全书合并文本为 `D:\\o\\book.zh.md`；双语原文（只读）为 `C:\\files\\107\\book.md`。\n\n| 文件 | 字数 | 组 | 锚点卡片 | 章 | 小节 |\n|---|---|---|---|---|---|\n| `s01.md` | 3164 | 1 | `Key2014/1` | 前言 | 前言 |\n| `s02.md` | 2203 | 1 | `Key2014/1` | 前言 | 致谢 |\n| `s03.md` | 100 | 2 | `Key2014/2` | 导论 | 导论 |\n";
        let (title, source, rows) = parse_index(text).unwrap();
        assert_eq!(title, "A Book");
        assert_eq!(source.as_deref(), Some("D:\\o\\book.zh.md"));
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].group, 1);
        assert_eq!(rows[0].file, "s01.md");
        assert_eq!(rows[2].anchor, "Key2014/2");
        assert_eq!(citation_key_of(&rows[2].anchor), "Key2014");
        assert!(parse_index("# nothing here").is_err());
    }

    #[test]
    fn a_skeleton_anchor_and_a_待写_part_are_not_written_work() {
        assert!(is_skeleton(&format!("本卡对应原书前言中的：前言。{SKELETON_ANCHOR}这一部分的核心论点与小节索引。")));
        assert!(!is_skeleton("§前言 - 首场讲座用学习的洞见重审自由贸易。"));
        assert!(is_placeholder("第一节。\n待写\n"));
        assert!(!is_placeholder("第一节。\n待写部分已经补齐的程度说明。\n"));
    }

    #[test]
    fn an_adopted_model_gets_a_readable_fixed_key() {
        assert_eq!(model_key("Book/1|b"), "adopted-Book-1-b");
        assert_eq!(model_key("Book/12|a|a|2|b"), "adopted-Book-12-a-a-2-b");
        assert!(model_key(&"x".repeat(400)).len() <= 150);
    }

    #[test]
    fn only_a_note_that_names_its_link_becomes_a_suggestion() {
        let fact = |address: &str, notes: &[&str]| ModelFact {
            address: address.to_string(),
            unit: format!("models/{}@1", model_key(address)),
            state: "verified",
            reason: String::new(),
            status: "built".into(),
            level: "concept".into(),
            parts: 1,
            placeholders: Vec::new(),
            faults: BTreeMap::new(),
            order: Vec::new(),
            derivations: Vec::new(),
            needs: Vec::new(),
            chains: json!([]),
            plan_hash: "h".into(),
            plan_identity: "retained_plan",
            notes: notes.iter().map(|note| note.to_string()).collect(),
            review_retained: false,
            fingerprints: Vec::new(),
        };
        let facts = vec![fact(
            "Book/1|b",
            &[
                "not merged: Book/1|b|a has successors",
                "建议补充 承接 [[Topic/3]]：本条说明该机制的先决条件。",
            ],
        )];
        let notes = note_suggestions(&facts);
        assert_eq!(notes.len(), 1, "{}", util::compact(&Value::Array(notes.clone())));
        assert_eq!(notes[0]["id"], json!("m-Book-1-b-n02"));
        assert_eq!(notes[0]["card"], json!("Book/1|b"));
    }
}
