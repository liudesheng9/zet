//! The checked source-review artifact of one accepted model plan (design 9.2 R247,
//! 11.3 R297).
//!
//! The architect's brief is the one owner of a Model draft's source review: it reads the
//! frozen draft against its own source span and the chapter's notes, and it writes the
//! plan. `validate` and `apply` record that review as one immutable artifact per model,
//! bound to the accepted plan and to the context pack the architect read. Every written
//! part has at least one source span in the form the plan already uses (`sections/s05.md:120-160`),
//! and it has an outcome: the source supports it, it was narrowed to the source, it is an
//! independently supplied step (`（补：书中未写）` / `（补：书中略去）`), or the source does
//! not state it. A part with no span is `unverified`, so the artifact is incomplete and the
//! model cannot claim completion.
//!
//! The whole-book Card cycle consumes this artifact for its coverage report instead of
//! reviewing the same draft a second time (R247). A model excluded from elaboration has no
//! plan, so the Card cycle itself reviews and corrects it.

use crate::ps;
use crate::util;
use anyhow::Result;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

/// The established markers of an independently supplied step (skill `zettelkasten`,
/// `zt-digest-protocol`): the only two forms a supplied claim may carry.
pub const MARKERS: [&str; 2] = ["（补：书中未写）", "（补：书中略去）"];

/// The review outcomes a part may declare.
pub const OUTCOMES: [&str; 4] = ["supported", "narrowed", "supplied", "absent"];

/// The outcome of a written part the plan did not review.
pub const UNVERIFIED: &str = "unverified";

static SPAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(.+?):(\d+)(?:-(\d+))?$").unwrap());
static OPEN_MARKER: LazyLock<Regex> = LazyLock::new(|| Regex::new("（补").unwrap());

/// A source span as the plan writes it: `file:from-to` (or `file:line`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub file: String,
    pub from: i64,
    pub to: i64,
}

impl Span {
    pub fn value(&self) -> Value {
        json!({"text": self.text, "file": self.file, "from": self.from, "to": self.to})
    }
}

/// A span in `file:from-to` form, with `to` defaulting to `from`.
pub fn span(text: &str) -> Option<Span> {
    let text = text.trim();
    let matched = SPAN.captures(text)?;
    let from: i64 = matched[2].parse().ok()?;
    let to = match matched.get(3) {
        Some(to) if !to.as_str().is_empty() => to.as_str().parse().ok()?,
        _ => from,
    };
    Some(Span { text: text.to_string(), file: matched[1].to_string(), from, to })
}

/// A span that names an existing file of the prepared source. The line range is the
/// architect's report of what it read; the file must exist, so an invented locator is caught.
pub fn resolve(staging: &str, span: &Span) -> Option<String> {
    if span.from < 1 || span.to < span.from {
        return None;
    }
    let path = if ps::rooted(&span.file) { span.file.clone() } else { ps::join(staging, &span.file) };
    ps::exists(&path).then_some(path)
}

/// Every `（补` in the text is one of the two established markers.
pub fn markers_ok(text: &str) -> bool {
    let mut start = 0;
    while let Some(found) = OPEN_MARKER.find_at(text, start) {
        let rest = &text[found.start()..];
        if !MARKERS.iter().any(|marker| rest.starts_with(marker)) {
            return false;
        }
        start = found.start() + "（补".len();
    }
    true
}

/// The text carries at least one established supplied-step marker.
pub fn supplied_marked(text: &str) -> bool {
    MARKERS.iter().any(|marker| text.contains(marker))
}

/// A part this build authors: not a taken-in, moved, or kept existing part.
fn written(part: &Value) -> bool {
    let truthy = |key: &str| ps::truthy(&part[key]);
    if truthy("take") || truthy("move") {
        return false;
    }
    !truthy("existing") || truthy("refill")
}

/// The facts one accepted plan's review is bound to: where the source and the plan live, and
/// the context pack the architect read.
pub struct Review<'a> {
    pub staging: &'a str,
    pub dir: &'a str,
    pub model: &'a str,
    pub plan_path: &'a str,
    pub plan_hash: &'a str,
    pub context: &'a str,
    pub context_hash: &'a str,
}

/// Records the plan's checked source review as `<dir>/source-review.json`, and returns the
/// artifact's own record plus one fault per reason the review is not complete. The file is
/// written either way so a refused review leaves its evidence; the same plan and context pack
/// always give the same bytes.
pub fn record(review: &Review, plan: &Value) -> (Value, Vec<String>) {
    let mut faults: Vec<String> = Vec::new();
    let parts: Vec<Value> = plan["parts"].as_array().cloned().unwrap_or_default();
    let ids: Vec<String> = parts.iter().map(|part| ps::s(&part["id"])).filter(|id| !id.is_empty()).collect();
    let mut declared: Map<String, Value> = Map::new();
    for (index, entry) in plan["source_review"].as_array().cloned().unwrap_or_default().iter().enumerate() {
        let part = ps::s(&entry["part"]);
        if part.is_empty() || (part != "card" && !ids.iter().any(|id| id == &part)) {
            faults.push(format!("source_review: entry {} names no planned part", index + 1));
            continue;
        }
        let outcome = ps::s(&entry["outcome"]);
        if !OUTCOMES.contains(&outcome.as_str()) {
            faults.push(format!(
                "source_review: part `{part}` has outcome `{outcome}`, not one of {}",
                OUTCOMES.join(", ")
            ));
            continue;
        }
        if declared.contains_key(&part) {
            faults.push(format!("source_review: part `{part}` is reviewed twice"));
            continue;
        }
        if outcome == "supplied" && ps::s(&entry["note"]).trim().is_empty() {
            faults.push(format!("source_review: supplied step of `{part}` needs one note saying what it supplies"));
            continue;
        }
        declared.insert(part, entry.clone());
    }
    let mut reviewed: Vec<Value> = Vec::new();
    let mut counts: Map<String, Value> = Map::new();
    for outcome in OUTCOMES.iter().chain([UNVERIFIED].iter()) {
        counts.insert((*outcome).to_string(), json!(0));
    }
    for part in parts.iter().filter(|part| written(part)) {
        let id = ps::s(&part["id"]);
        let mut spans: Vec<Span> = Vec::new();
        for raw in ps::list(&part["src"]).into_iter().map(ps::s) {
            let Some(found) = span(&raw) else {
                faults.push(format!("source_review: part `{id}` has the malformed source span `{raw}`"));
                continue;
            };
            if !review.staging.is_empty() && resolve(review.staging, &found).is_none() {
                faults.push(format!("source_review: part `{id}` names the source span `{raw}`, which the prepared source does not hold"));
                continue;
            }
            spans.push(found);
        }
        let entry = declared.get(&id).cloned();
        if let Some(entry) = &entry
            && let Some(raw) = entry["span"].as_str().filter(|raw| !raw.trim().is_empty())
        {
            match span(raw) {
                Some(found) if review.staging.is_empty() || resolve(review.staging, &found).is_some() => {
                    if !spans.iter().any(|known| known.text == found.text) {
                        spans.push(found);
                    }
                }
                Some(_) => faults.push(format!(
                    "source_review: part `{id}` reviews `{raw}`, which the prepared source does not hold"
                )),
                None => faults.push(format!("source_review: part `{id}` has the malformed source span `{raw}`")),
            }
        }
        let outcome = match &entry {
            Some(entry) => ps::s(&entry["outcome"]),
            None if !spans.is_empty() => "supported".to_string(),
            None => UNVERIFIED.to_string(),
        };
        if outcome == UNVERIFIED {
            faults.push(format!("source_review: part `{id}` has no reviewed source span"));
        }
        if let Some(at) = counts.get_mut(&outcome)
            && let Some(n) = at.as_i64()
        {
            *at = json!(n + 1);
        }
        let mut value = json!({
            "part": id,
            "outcome": outcome,
            "spans": spans.iter().map(Span::value).collect::<Vec<_>>(),
        });
        if let Some(note) = entry.as_ref().and_then(|entry| entry["note"].as_str()) {
            value["note"] = json!(note.trim());
        }
        reviewed.push(value);
    }
    let complete = faults.is_empty();
    let record = json!({
        "schema_version": 1,
        "kind": "model_source_review",
        "model": review.model,
        "plan": {"path": review.plan_path, "sha256": review.plan_hash},
        "context": {"path": review.context, "sha256": review.context_hash},
        "staging": review.staging,
        "source": source_identity(review.staging),
        "complete": complete,
        "outcomes": counts,
        "parts": reviewed,
    });
    let path = ps::join(review.dir, "source-review.json");
    let written: Result<Vec<u8>> = util::write_text(&path, &ps::json_pretty(&record))
        .and_then(|_| std::fs::read(&path).map_err(Into::into));
    match written {
        Ok(bytes) => (
            json!({
                "path": path,
                "sha256": crate::canon::sha256_hex(&bytes),
                "complete": complete,
                "parts": reviewed.len(),
                "outcomes": record["outcomes"],
            }),
            faults,
        ),
        Err(error) => {
            faults.push(format!("source_review: cannot record {path}: {error:#}"));
            (Value::Null, faults)
        }
    }
}

/// The prepared source's own identity, when the staging folder holds its manifest.
fn source_identity(staging: &str) -> Value {
    if staging.is_empty() {
        return Value::Null;
    }
    let manifest = ps::join(staging, "manifest.json");
    if !ps::exists(&manifest) {
        return Value::Null;
    }
    match util::read_json(&manifest) {
        Ok(manifest) => json!({
            "path": manifest["source"]["path"],
            "sha256": manifest["source"]["sha256"],
            "file_id": manifest["source"]["file_id"],
        }),
        Err(_) => Value::Null,
    }
}

/// Reads one retained artifact and checks it describes `plan_hash` and is complete. The
/// checks that read an existing artifact share this: a later plan change makes the
/// recorded review describe a different plan, never the accepted one.
pub fn read(path: &str, model: &str, plan_hash: &str) -> Result<Value> {
    let record = util::read_json(path)?;
    if record["schema_version"] != 1 || record["kind"] != "model_source_review" {
        anyhow::bail!("{path} is not a model source-review artifact");
    }
    if record["model"] != json!(model) {
        anyhow::bail!("the source-review artifact in {path} reviews `{}`, not `{model}`", ps::s(&record["model"]));
    }
    if record["plan"]["sha256"] != json!(plan_hash) {
        anyhow::bail!("the source-review artifact in {path} describes another plan");
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_takes_its_optional_end() {
        let one = span("sections/s05.md:120").unwrap();
        assert_eq!((one.file.as_str(), one.from, one.to), ("sections/s05.md", 120, 120));
        let range = span("sections/s05.md:120-160").unwrap();
        assert_eq!((range.from, range.to), (120, 160));
        assert!(span("sections/s05.md").is_none());
        assert!(span("s05:0-3").is_some());
    }

    #[test]
    fn only_the_established_markers_pass() {
        assert!(markers_ok("代入 (5.10)（补：书中略去）得到结论。"));
        assert!(markers_ok("这一步（补：书中未写）由读者自行验证。"));
        assert!(markers_ok("书中直接给出结果。"));
        assert!(!markers_ok("这一步（补充：由我补上）得到结论。"));
        assert!(!markers_ok("（补：书中略去）（补）"));
        assert!(supplied_marked("（补：书中未写）"));
        assert!(!supplied_marked("（补充说明）"));
    }

    #[test]
    fn a_written_part_needs_a_span() {
        assert!(written(&json!({"id":"A","line":"main"})));
        assert!(!written(&json!({"id":"G","take":"Key/3|b|a"})));
        assert!(!written(&json!({"id":"M","move":"Key/9"})));
        assert!(!written(&json!({"id":"S","existing":true})));
        assert!(written(&json!({"id":"S","existing":true,"refill":true})));
    }
}
