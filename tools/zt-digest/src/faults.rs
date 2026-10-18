//! `zt-digest source-faults`: the retained source-fault record and its deterministic report
//! (implementation design 11.6, R312/R313).
//!
//! Architects return source faults as structured entries: the source's hash and file id, the
//! original line span, the chapter/equation/note label, the observed text, a proposed
//! interpretation, a confidence, and the Cards the fault affects. This module validates each
//! entry against the prepared source, keeps one entry per source location (deduplicated across
//! attempts), and renders `<out>\source-faults.md` from the retained record only. The preparation
//! also records its own faults here: an ambiguous notes mapping, a subsection that names no
//! chapter, and the notes a chapter cites but the book never defines.
//!
//! Nothing in this module writes to the Scholar Archive, and nothing edits a Card. An observation
//! whose text the current source does not print at its span is kept as an unsupported observation
//! in its own section: it is never rendered as a fault about the source, so a stale or invented
//! historical example cannot appear as evidence. A proposed correction is kept apart from a
//! verified one, and a verified correction needs the evidence that verified it.

use crate::args::Args;
use crate::ps::{self, eq, s};
use crate::util;
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};

/// The kinds of fault the report knows. The notes kinds are the preparation's own.
const NOTES_KINDS: [&str; 5] = ["notes_ambiguous", "notes_unmapped", "notes_inside_range", "notes_missing", "notes_split"];
/// The confidences an architect may state.
const CONFIDENCE: [&str; 3] = ["high", "medium", "low"];

/// The prepared source this report belongs to.
pub(crate) struct Identity {
    pub path: String,
    pub file_id: i64,
    pub sha256: String,
    /// The source's lines, BOM dropped and CRLF folded, so a span means the same thing here.
    pub lines: Vec<String>,
}

impl Identity {
    fn short(&self) -> String {
        self.sha256.chars().take(12).collect()
    }
}

/// `zt-digest source-faults -Staging <out> [-Json <file>] [-Origin <label>]`
pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(argv, &["Staging", "Json", "Origin"], &[])?;
    let staging = a.required("Staging")?;
    if staging.is_empty() {
        bail!("Cannot bind argument to parameter 'Staging' because it is an empty string.");
    }
    let identity = identity(&staging)?;
    let summary = if a.has("Json") {
        let path = a.required("Json")?;
        let text = std::fs::read_to_string(&path).with_context(|| format!("Cannot find path '{path}' because it does not exist."))?;
        let list = list_of(&serde_json::from_str::<Value>(&text).with_context(|| format!("{path} is not JSON"))?);
        let origin = if a.has("Origin") { a.str("Origin") } else { path.clone() };
        let normalized = entries(&identity, &list, &origin)?;
        record(&staging, &identity, normalized)?
    } else {
        render(&staging, &identity)?
    };
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

/// The entries of a JSON document: an array, or an object's `faults` array.
fn list_of(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(list) => list.clone(),
        Value::Object(map) => map.get("faults").and_then(Value::as_array).cloned().unwrap_or_else(|| vec![value.clone()]),
        _ => vec![value.clone()],
    }
}

/// The prepared source the staging folder belongs to: `<staging>\manifest.json` names it and its
/// hash; a manifest that predates the hash field is read from the source file itself.
pub(crate) fn identity(staging: &str) -> Result<Identity> {
    let manifest_file = ps::join(staging, "manifest.json");
    let manifest = if ps::exists(&manifest_file) { util::read_json(&manifest_file)? } else { Value::Null };
    let path = s(&manifest["source"]);
    if path.is_empty() {
        bail!(
            "no prepared source in {staging}: manifest.json names none, so a source fault cannot be bound to the source it describes (pass -Source to prepare first)"
        );
    }
    let bytes = std::fs::read(&path).with_context(|| format!("Cannot find the prepared source '{path}' because it does not exist."))?;
    let sha256 = crate::canon::sha256_hex(&bytes);
    let declared = s(&manifest["source_sha256"]);
    if !declared.is_empty() && !eq(&declared, &sha256) {
        bail!(
            "the prepared source {path} changed since preparation: the manifest records SHA-256 {declared}, the file now hashes {sha256}; a fault record must bind the source both were prepared from"
        );
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let text = text.strip_prefix('\u{FEFF}').map(str::to_string).unwrap_or(text).replace("\r\n", "\n");
    let file_id = manifest["file_id"].as_i64().unwrap_or(0);
    Ok(Identity { path, file_id, sha256, lines: text.split('\n').map(str::to_string).collect() })
}

/// Validate the entries and normalize them: one fault per source location is enforced at merge
/// time, and every value is checked against the prepared source before it is recorded.
pub(crate) fn entries(identity: &Identity, list: &[Value], origin: &str) -> Result<Vec<Value>> {
    let mut out = Vec::new();
    for (index, entry) in list.iter().enumerate() {
        let normalized = one(identity, entry).with_context(|| format!("source fault {} of {}", index + 1, list.len()))?;
        out.push(tagged(normalized, origin));
    }
    Ok(out)
}

/// One entry, validated and normalized.
fn one(identity: &Identity, entry: &Value) -> Result<Value> {
    if !entry.is_object() {
        bail!("a source fault must be a JSON object with the source lines, the observed text and a proposed interpretation");
    }
    let kind = s(&entry["kind"]);
    let kind = if kind.is_empty() { "source".to_string() } else { kind };
    if !eq(&kind, "source") && !NOTES_KINDS.iter().any(|k| eq(k, &kind)) {
        bail!("unknown fault kind `{kind}`; the report knows source, {}", NOTES_KINDS.join(", "));
    }
    // A fault names the source it was observed in; a different hash is never rebound silently.
    let declared = s(&entry["source_sha256"]);
    if !declared.is_empty() && !eq(&declared, &identity.sha256) {
        bail!(
            "the fault names source hash {declared} but the prepared source is {} ({})",
            identity.sha256,
            identity.path
        );
    }
    let declared_id = entry["file_id"].as_i64().unwrap_or(0);
    if declared_id != 0 && declared_id != identity.file_id {
        bail!("the fault names Scholar Archive file id {declared_id} but the prepared source is file id {}", identity.file_id);
    }
    let mut out = Map::new();
    out.insert("kind".into(), json!(kind));
    out.insert("source_sha256".into(), json!(identity.sha256));
    out.insert("file_id".into(), json!(identity.file_id));
    for key in ["chapter", "equation", "note", "heading", "chapter_id", "message"] {
        if !entry[key].is_null() {
            out.insert(key.into(), json!(s(&entry[key])));
        }
    }
    for key in ["note_ids", "candidates"] {
        if let Some(list) = entry[key].as_array() {
            out.insert(key.into(), json!(list.iter().map(s).collect::<Vec<_>>()));
        }
    }
    if let Some(parts) = entry["parts"].as_i64() {
        out.insert("parts".into(), json!(parts));
    }
    if eq(&kind, "source") {
        source_fields(identity, entry, &mut out)?;
    } else {
        notes_fields(identity, entry, &mut out)?;
    }
    out.insert("status".into(), json!(if out.contains_key("verified") { "verified" } else { "proposed" }));
    Ok(Value::Object(out))
}

/// The fields every architect observation must carry.
fn source_fields(identity: &Identity, entry: &Value, out: &mut Map<String, Value>) -> Result<()> {
    let start = entry["start_line"].as_i64().unwrap_or(0);
    let end = entry["end_line"].as_i64().unwrap_or(start);
    if start < 1 || end < start || end as usize > identity.lines.len() {
        bail!(
            "start_line {} and end_line {} are outside {} ({} lines)",
            entry["start_line"],
            entry["end_line"],
            identity.path,
            identity.lines.len()
        );
    }
    if ["chapter", "equation", "note"].iter().all(|key| out.get(*key).is_none_or(|v| s(v).trim().is_empty())) {
        bail!("name the fault's chapter, equation or note label so a reader can find it");
    }
    let observed = s(&entry["observed"]).trim().to_string();
    if observed.is_empty() {
        bail!("observed must hold the text the source prints at lines {start}–{end}");
    }
    let proposed = s(&entry["proposed"]).trim().to_string();
    if proposed.is_empty() {
        bail!("proposed must hold the proposed interpretation or correction");
    }
    let confidence = s(&entry["confidence"]).to_ascii_lowercase();
    if confidence.is_empty() {
        bail!("confidence is required: high, medium or low");
    }
    if !CONFIDENCE.contains(&confidence.as_str()) {
        bail!("confidence `{confidence}` is not high, medium or low");
    }
    let affected: Vec<String> = entry["affected"].as_array().map(|list| list.iter().map(s).collect()).unwrap_or_default();
    for address in &affected {
        if address.trim().is_empty() {
            bail!("an affected Card address is empty");
        }
    }
    // The current source must actually print the observation at that location.
    let span = identity.lines[start as usize - 1..end as usize].join("\n");
    let supported = squash(&span).contains(&squash(&observed));
    if let Some(verified) = entry["verified"].as_object() {
        let text = squash(&s(verified.get("text").unwrap_or(&Value::Null)));
        let evidence = squash(&s(verified.get("evidence").unwrap_or(&Value::Null)));
        if text.is_empty() || evidence.is_empty() {
            bail!("a verified correction needs its text and the evidence that verified it");
        }
        out.insert("verified".into(), json!({"text":text,"evidence":evidence}));
        out.insert("supported".into(), json!(true));
    } else {
        out.insert("supported".into(), json!(supported));
    }
    out.insert("start_line".into(), json!(start));
    out.insert("end_line".into(), json!(end));
    out.insert("observed".into(), json!(observed));
    out.insert("proposed".into(), json!(proposed));
    out.insert("confidence".into(), json!(confidence));
    out.insert("affected".into(), json!(affected));
    Ok(())
}

/// The preparation's own notes faults: they carry a location and a message, not a proposal.
fn notes_fields(identity: &Identity, entry: &Value, out: &mut Map<String, Value>) -> Result<()> {
    let start = entry["start_line"].as_i64().unwrap_or(0);
    let end = entry["end_line"].as_i64().unwrap_or(start);
    let span = entry["span"].clone();
    let (start, end) = if start == 0 && span["start_line"].as_i64().unwrap_or(0) != 0 {
        (span["start_line"].as_i64().unwrap_or(0), span["end_line"].as_i64().unwrap_or(0))
    } else {
        (start, end)
    };
    if start != 0 {
        if start < 1 || end < start || end as usize > identity.lines.len() {
            bail!(
                "lines {start}–{end} are outside {} ({} lines)",
                identity.path,
                identity.lines.len()
            );
        }
        out.insert("start_line".into(), json!(start));
        out.insert("end_line".into(), json!(end));
        out.insert("observed".into(), json!(s(&entry["observed"])));
    }
    if s(out.get("message").unwrap_or(&Value::Null)).trim().is_empty() {
        bail!("the preparation fault needs its message");
    }
    out.insert("supported".into(), json!(true));
    Ok(())
}

/// One entry with its origin and its first attempt.
fn tagged(mut entry: Value, origin: &str) -> Value {
    let attempt = json!({
        "origin":origin,
        "proposed":entry["proposed"].clone(),
        "confidence":entry["confidence"].clone(),
        "observed":entry["observed"].clone(),
    });
    entry["origin"] = json!(origin);
    entry["attempts"] = json!([attempt]);
    entry
}

/// The identity of one fault: its kind and the source location it belongs to.
fn fault_key(entry: &Value) -> String {
    match s(&entry["kind"]).as_str() {
        "source" => format!(
            "source|{}|{}|{}|{}",
            s(&entry["start_line"]),
            s(&entry["end_line"]),
            s(&entry["equation"]),
            s(&entry["note"])
        ),
        "notes_missing" => {
            let ids: Vec<String> = entry["note_ids"].as_array().map(|l| l.iter().map(s).collect()).unwrap_or_default();
            format!("notes_missing|{}|{}", s(&entry["chapter_id"]), ids.join(","))
        }
        "notes_split" => format!("notes_split|{}", s(&entry["chapter_id"])),
        kind => format!("{kind}|{}|{}|{}", s(&entry["start_line"]), s(&entry["end_line"]), s(&entry["heading"])),
    }
}

/// Merge normalized entries into the staging folder's retained record and render the report.
pub(crate) fn record(staging: &str, identity: &Identity, normalized: Vec<Value>) -> Result<Value> {
    let store_file = ps::join(staging, "source-faults.json");
    let mut store = read_store(staging, identity)?;
    let mut faults: Vec<Value> = store["faults"].as_array().cloned().unwrap_or_default();
    let mut seq = faults.iter().filter_map(|f| s(&f["id"]).strip_prefix('f').and_then(|n| n.parse::<i64>().ok())).max().unwrap_or(0);
    let recorded = normalized.len();
    for entry in normalized {
        let key = fault_key(&entry);
        match faults.iter_mut().find(|f| s(&f["key"]) == key) {
            Some(existing) => merge(existing, &entry),
            None => {
                seq += 1;
                let mut created = entry;
                created["id"] = json!(format!("f{seq:03}"));
                created["key"] = json!(key);
                faults.push(created);
            }
        }
    }
    // The record is written in a stable order: located faults by their source lines, then the
    // limitations that carry no location.
    ps::ps_sort(&mut faults, |a, b| {
        let key = |v: &Value| (v["start_line"].as_i64().unwrap_or(i64::MAX), v["end_line"].as_i64().unwrap_or(i64::MAX), s(&v["id"]));
        key(a).cmp(&key(b))
    });
    store["faults"] = Value::Array(faults);
    store["source"] = json!({"path":identity.path,"file_id":identity.file_id,"sha256":identity.sha256});
    util::write_atomic(&store_file, &ps::json_pretty(&store))?;
    let summary = render(staging, identity)?;
    let mut summary = summary;
    summary["recorded"] = json!(recorded);
    Ok(summary)
}

/// Merge a later observation of the same location into the retained entry: the original
/// observation stays, both attempts stay, and every affected Card is kept.
fn merge(existing: &mut Value, entry: &Value) {
    let attempt = json!({
        "origin":entry["origin"].clone(),
        "proposed":entry["proposed"].clone(),
        "confidence":entry["confidence"].clone(),
        "observed":entry["observed"].clone(),
    });
    let mut attempts: Vec<Value> = existing["attempts"].as_array().cloned().unwrap_or_default();
    let same = attempts.last().is_some_and(|last| {
        eq(&s(&last["origin"]), &s(&entry["origin"]))
            && eq(&s(&last["proposed"]), &s(&entry["proposed"]))
            && eq(&s(&last["confidence"]), &s(&entry["confidence"]))
    });
    if !same {
        attempts.push(attempt);
    }
    existing["attempts"] = Value::Array(attempts);
    if let Some(proposed) = entry["proposed"].as_str().filter(|p| !p.trim().is_empty()) {
        existing["proposed"] = json!(proposed);
        existing["confidence"] = entry["confidence"].clone();
    }
    let mut affected: Vec<String> = existing["affected"].as_array().map(|l| l.iter().map(s).collect()).unwrap_or_default();
    for address in entry["affected"].as_array().map(|l| l.iter().map(s).collect::<Vec<_>>()).unwrap_or_default() {
        if !affected.contains(&address) {
            affected.push(address);
        }
    }
    if !affected.is_empty() {
        existing["affected"] = json!(affected);
    }
    if entry["supported"] == true {
        existing["supported"] = json!(true);
    }
    if !existing["verified"].is_object() && entry["verified"].is_object() {
        existing["verified"] = entry["verified"].clone();
        existing["status"] = json!("verified");
    }
    if let Some(message) = entry["message"].as_str().filter(|m| !m.is_empty()) {
        existing["message"] = json!(message);
    }
}

/// The retained record of one staging folder, with the source binding checked.
fn read_store(staging: &str, identity: &Identity) -> Result<Value> {
    let store_file = ps::join(staging, "source-faults.json");
    if !ps::exists(&store_file) {
        return Ok(json!({"schema_version":1,"faults":[]}));
    }
    let mut store = util::read_json(&store_file)?;
    let sha = s(&store["source"]["sha256"]);
    if !sha.is_empty() && !eq(&sha, &identity.sha256) {
        bail!(
            "the retained source-fault record in {staging} belongs to source {sha}, but the prepared source is {}; keep one output directory per source",
            identity.sha256
        );
    }
    if store["faults"].is_null() {
        store["faults"] = json!([]);
    }
    Ok(store)
}

/// Render `<staging>\source-faults.md` from the retained record alone, and return its counts.
pub(crate) fn render(staging: &str, identity: &Identity) -> Result<Value> {
    let store = read_store(staging, identity)?;
    let faults: Vec<Value> = store["faults"].as_array().cloned().unwrap_or_default();
    let located: Vec<&Value> = faults.iter().filter(|f| eq(&s(&f["kind"]), "source") && f["supported"] == true).collect();
    let unsupported: Vec<&Value> = faults.iter().filter(|f| eq(&s(&f["kind"]), "source") && f["supported"] != true).collect();
    let preparation: Vec<&Value> = faults.iter().filter(|f| ["notes_ambiguous", "notes_unmapped", "notes_inside_range"].iter().any(|k| eq(k, &s(&f["kind"])))).collect();
    let limits: Vec<&Value> = faults.iter().filter(|f| ["notes_missing", "notes_split"].iter().any(|k| eq(k, &s(&f["kind"])))).collect();
    let verified = located.iter().filter(|f| f["status"] == "verified").count();

    let mut md = String::new();
    let manifest_file = ps::join(staging, "manifest.json");
    let title = if ps::exists(&manifest_file) {
        let manifest = util::read_json(&manifest_file)?;
        let title = s(&manifest["book_title"]);
        if title.is_empty() { s(&manifest["citation_key"]) } else { title }
    } else {
        String::new()
    };
    md.push_str(&if title.is_empty() { "# 源码缺陷报告\n\n".to_string() } else { format!("# 源码缺陷报告：《{title}》\n\n") });
    md.push_str(&format!("- 原书：`{}`\n", identity.path));
    if identity.file_id != 0 {
        md.push_str(&format!("- Scholar Archive 文件 id：{}\n", identity.file_id));
    }
    md.push_str(&format!("- 源码 SHA-256：`{}`（`{}`）\n", identity.sha256, identity.short()));
    md.push_str(&format!(
        "- 记录 {} 条：源码缺陷 {} 条（其中已核实 {} 条），未能核对 {} 条，准备阶段的映射缺陷 {} 条，源码缺失与已声明的限制 {} 条。\n",
        faults.len(),
        located.len(),
        verified,
        unsupported.len(),
        preparation.len(),
        limits.len()
    ));
    md.push_str("- 本报告只由保留的记录确定性渲染；它不修改 Scholar Archive 的副本，也不修改任何卡片。\n");

    md.push_str("\n## 源码缺陷\n\n");
    if located.is_empty() {
        md.push_str("（没有。只有当前原文支持、且架构师确实报告的缺陷才会出现在这里；不要据设计文档里的历史例子推断它们仍在。）\n");
    }
    for fault in &located {
        md.push_str(&source_entry(fault));
    }

    md.push_str("\n## 未能核对的观察\n\n");
    if unsupported.is_empty() {
        md.push_str("（没有。）\n");
    } else {
        md.push_str("这些观察记录的原文在当前原书的那些行里找不到，所以它们不计为源码缺陷。修好或更换原书后重新观察。\n\n");
        for fault in &unsupported {
            md.push_str(&source_entry(fault));
        }
    }

    md.push_str("\n## 准备阶段的映射缺陷\n\n");
    if preparation.is_empty() {
        md.push_str("（没有。）\n");
    }
    for fault in &preparation {
        md.push_str(&format!("### {} · {}\n\n{}\n\n", s(&fault["id"]), location(fault), s(&fault["message"])));
    }

    md.push_str("\n## 源码缺失与已声明的限制\n\n");
    if limits.is_empty() {
        md.push_str("（没有。）\n");
    }
    for fault in &limits {
        md.push_str(&format!("### {} · {}\n\n{}\n\n", s(&fault["id"]), location(fault), s(&fault["message"])));
    }

    let report = ps::join(staging, "source-faults.md");
    util::write_atomic(&report, &md)?;
    Ok(json!({
        "store":ps::join(staging, "source-faults.json"),
        "report":report,
        "recorded":0,
        "faults":located.len(),
        "source_faults":located.len(),
        "unsupported":unsupported.len(),
        "preparation":preparation.len(),
        "limitations":limits.len(),
        "verified":verified,
    }))
}

/// One located fault as Markdown.
fn source_entry(fault: &Value) -> String {
    let mut md = String::new();
    md.push_str(&format!("### {} · {}\n\n", s(&fault["id"]), location(fault)));
    if !s(&fault["chapter"]).is_empty() && !s(&fault["equation"]).is_empty() {
        md.push_str(&format!("- 章节：{}；式子：{}\n", s(&fault["chapter"]), s(&fault["equation"])));
    } else if !s(&fault["chapter"]).is_empty() {
        md.push_str(&format!("- 章节：{}\n", s(&fault["chapter"])));
    } else if !s(&fault["equation"]).is_empty() {
        md.push_str(&format!("- 式子：{}\n", s(&fault["equation"])));
    }
    if !s(&fault["note"]).is_empty() {
        md.push_str(&format!("- 注释：{}\n", s(&fault["note"])));
    }
    let observed = s(&fault["observed"]);
    if !observed.is_empty() {
        md.push_str("- 观察到的原文：\n\n");
        for line in observed.split('\n') {
            md.push_str(&format!("> {line}\n"));
        }
        md.push('\n');
    }
    if let Some(verified) = fault["verified"].as_object() {
        md.push_str(&format!(
            "- 已核实的修正（证据：{}）：{}\n",
            s(verified.get("evidence").unwrap_or(&Value::Null)),
            s(verified.get("text").unwrap_or(&Value::Null))
        ));
    }
    let proposed = s(&fault["proposed"]);
    if !proposed.is_empty() {
        let label = if fault["verified"].is_object() { "同时保留的建议" } else { "建议的解读或修正（未核实）" };
        md.push_str(&format!("- {label}：{proposed}\n"));
    }
    let affected: Vec<String> = fault["affected"].as_array().map(|l| l.iter().map(s).collect()).unwrap_or_default();
    md.push_str(&format!(
        "- 置信度：{}；影响的卡片：{}\n",
        s(&fault["confidence"]),
        if affected.is_empty() { "（没有）".to_string() } else { affected.join("、") }
    ));
    let attempts: Vec<Value> = fault["attempts"].as_array().cloned().unwrap_or_default();
    let origins: Vec<String> = attempts.iter().map(|a| s(&a["origin"])).collect::<Vec<_>>();
    md.push_str(&format!("- 观察次数：{}；来源：{}\n", attempts.len().max(1), ps::select_unique(origins).join("、")));
    if attempts.len() > 1 {
        md.push_str("- 每次观察：\n");
        for attempt in &attempts {
            md.push_str(&format!(
                "  - {}（{}）：{}\n",
                s(&attempt["origin"]),
                s(&attempt["confidence"]),
                s(&attempt["proposed"])
            ));
        }
    }
    md.push('\n');
    md
}

/// `第 12–14 行（第一章 契约，(A.66)）` for a located fault.
fn location(fault: &Value) -> String {
    let start = fault["start_line"].as_i64().unwrap_or(0);
    let end = fault["end_line"].as_i64().unwrap_or(start);
    if start == 0 {
        let chapter = s(&fault["chapter"]);
        if chapter.is_empty() { return "全书".to_string() } else { return chapter }
    }
    let mut labels: Vec<String> = Vec::new();
    for key in ["heading", "chapter"] {
        let text = s(&fault[key]);
        if !text.is_empty() && !labels.contains(&text) {
            labels.push(text);
        }
    }
    for key in ["equation", "note"] {
        let text = s(&fault[key]);
        if !text.is_empty() {
            labels.push(text);
        }
    }
    let ids: Vec<String> = fault["note_ids"].as_array().map(|l| l.iter().map(s).collect()).unwrap_or_default();
    if !ids.is_empty() {
        labels.push(format!("注释 {}", ids.join("、")));
    }
    if labels.is_empty() {
        format!("第 {start}–{end} 行")
    } else {
        format!("第 {start}–{end} 行（{}）", labels.join("，"))
    }
}

/// Whitespace-folded text, so a line break or an indent cannot hide a match or invent one.
fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What the preparation records about its own notes, under its own origin.
pub(crate) fn record_preparation(staging: &str, identity: &Identity, faults: &[Value]) -> Result<Value> {
    if faults.is_empty() {
        return render(staging, identity);
    }
    let normalized = entries(identity, faults, "prepare")?;
    record(staging, identity, normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fault(equation: &str, proposed: &str, confidence: &str, affected: Vec<&str>) -> Value {
        json!({
            "kind":"source","start_line":12,"end_line":14,"chapter":"第一章","equation":equation,
            "observed":"分母印刷为 $1+\\rho$","proposed":proposed,"confidence":confidence,
            "affected":affected,"attempts":[json!({"origin":"a","proposed":proposed,"confidence":confidence})],
            "id":"f001","key":fault_key(&json!({"kind":"source","start_line":12,"end_line":14,"equation":equation})),
        })
    }

    #[test]
    fn one_key_per_source_location_and_label() {
        let a = json!({"kind":"source","start_line":12,"end_line":14,"equation":"(A.66)","note":""});
        let same = json!({"kind":"source","start_line":12,"end_line":14,"equation":"(A.66)","note":""});
        let other = json!({"kind":"source","start_line":12,"end_line":14,"equation":"(A.67)","note":""});
        assert_eq!(fault_key(&a), fault_key(&same));
        assert_ne!(fault_key(&a), fault_key(&other));
        let split = json!({"kind":"notes_split","chapter_id":"c02"});
        let other_split = json!({"kind":"notes_split","chapter_id":"c03"});
        assert_ne!(fault_key(&split), fault_key(&other_split));
        let missing = json!({"kind":"notes_missing","chapter_id":"c02","note_ids":["1.4"]});
        let missing2 = json!({"kind":"notes_missing","chapter_id":"c02","note_ids":["1.5"]});
        assert_ne!(fault_key(&missing), fault_key(&missing2));
    }

    #[test]
    fn a_second_observation_keeps_the_original_observation_and_every_card() {
        let mut existing = fault("(A.66)", "分母应为 $1+\\rho+\\lambda$", "high", vec!["K/12|a"]);
        let later = json!({
            "kind":"source","start_line":12,"end_line":14,"chapter":"第一章","equation":"(A.66)",
            "observed":"另一段观察文字","proposed":"分母应为 $1+\\rho+\\lambda$（第二次）","confidence":"low",
            "affected":["K/12|b"],"origin":"b","supported":true,
        });
        merge(&mut existing, &later);
        assert_eq!(s(&existing["observed"]), "分母印刷为 $1+\\rho$");
        assert_eq!(s(&existing["proposed"]), "分母应为 $1+\\rho+\\lambda$（第二次）");
        assert_eq!(s(&existing["confidence"]), "low");
        assert_eq!(existing["affected"], json!(["K/12|a", "K/12|b"]));
        assert_eq!(existing["attempts"].as_array().map(Vec::len), Some(2));
        // Recording the same attempt twice is idempotent.
        merge(&mut existing, &later);
        assert_eq!(existing["attempts"].as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn whitespace_folding_cannot_hide_or_invent_an_observation() {
        assert!(squash("分母\n 印刷为 $1+\\rho$").contains(&squash("分母 印刷为 $1+\\rho$")));
        assert!(!squash("分母印刷为 $1+\\rho$").contains(&squash("分母印刷为 $1+\\rho+\\lambda$")));
    }
}
