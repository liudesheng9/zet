//! R331–R342: one prepared run's report, from ledger facts and verified reads.
//!
//! The report is a *rendering*, never a second authority: every fact comes from the run
//! ledger (units, dependencies, events, recorded resources) or from a verified read of
//! the archive that names its revision. Re-rendering it writes no Card: it only rewrites
//! the two report files when their bytes change, so the same facts give the same report
//! and a second render changes nothing (R331).
//!
//! Two rules decide what may appear here:
//!
//! * A fact that could not be read is `unknown`, never zero (R336). A failed placeholder
//!   read keeps its count `null`, and the run is not quality-complete.
//! * Per-run usage is deduplicated by provider run id (and request id), never derived by
//!   subtracting two rolling account totals (R343/R344). A run whose usage the provider
//!   reports only account-wide stays *missing coverage*, and the report states the known
//!   subtotal beside the missing-run count (R345).

use crate::ps;
use crate::zt::Zt;
use anyhow::{Context, Result};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const REPORT_JSON: &str = "run-report.json";
pub const REPORT_MARKDOWN: &str = "run-report.md";

/// The phases a stop can name: the initial work, a retry of one unit, a repair pass, or
/// the single supported resume. Anything else is derived from the attempt and invocation.
const PHASES: [&str; 4] = ["initial", "retry", "fix", "resume"];

// --- time -----------------------------------------------------------------------------------------

/// Epoch milliseconds of an RFC 3339 UTC stamp with up to seven fractional digits (the
/// form `canon::utc_now_o` writes). `None` for anything else, so a malformed stamp never
/// becomes zero elapsed time.
pub fn epoch_millis(utc: &str) -> Option<i64> {
    let text = utc.trim();
    let bytes = text.as_bytes();
    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' || bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    let number = |from: usize, to: usize| text.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute, second) = (number(11, 13)?, number(14, 16)?, number(17, 19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let tail = text.get(19..)?;
    let millis = if tail == "Z" || tail == "z" {
        0
    } else {
        let fraction = tail.strip_suffix('Z').or_else(|| tail.strip_suffix('z'))?;
        let digits = fraction.strip_prefix('.')?;
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let mut value = String::from("000");
        for (at, c) in digits.chars().take(3).enumerate() {
            value.replace_range(at..at + 1, &c.to_string());
        }
        value.parse::<i64>().ok()?
    };
    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second;
    Some(seconds * 1_000 + millis)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = year - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Seconds between two stamps, never negative, and never a fabricated zero for a
/// malformed stamp.
pub fn span_seconds(from: &str, to: &str) -> Option<f64> {
    let (from, to) = (epoch_millis(from)?, epoch_millis(to)?);
    Some(((to - from).max(0) as f64) / 1_000.0)
}

/// A numeric fact with PowerShell's spelling: an integral value stays an integer, and a
/// fraction keeps three decimals at most. Rounding is applied once, so two renderings of
/// the same measured value are equal.
pub fn number_json(value: f64) -> Value {
    let rounded = (value * 1_000.0).round() / 1_000.0;
    if rounded.fract() == 0.0 && rounded.abs() < 9.0e15 {
        json!(rounded as i64)
    } else {
        json!(rounded)
    }
}

// --- usage ----------------------------------------------------------------------------------------

fn number_of(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64().filter(|n| n.is_finite() && *n >= 0.0),
        _ => None,
    }
}

fn numeric_facts(value: &Value) -> Option<BTreeMap<String, f64>> {
    let object = value.as_object()?;
    let mut facts = BTreeMap::new();
    for (name, item) in object {
        facts.insert(name.clone(), number_of(item)?);
    }
    Some(facts)
}

fn add_into(total: &mut BTreeMap<String, f64>, facts: &BTreeMap<String, f64>) {
    for (name, value) in facts {
        *total.entry(name.clone()).or_insert(0.0) += *value;
    }
}

fn facts_json(facts: &BTreeMap<String, f64>) -> Value {
    let mut object = Map::new();
    for (name, value) in facts {
        object.insert(name.clone(), number_json(*value));
    }
    Value::Object(object)
}

/// One conversation's recorded usage: its run id, the request-scoped or run-scoped
/// numbers, and the phase the work belonged to.
fn usage_record(resource: &Value, conversation: &Value) -> Option<Value> {
    let usage = conversation["usage"].as_object()?;
    let run = usage
        .get("run")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| conversation["run"].as_str().map(str::to_string))
        .or_else(|| conversation["id"].as_str().map(str::to_string))?;
    Some(json!({
        "run": run,
        "unit": resource["owner"],
        "token": resource["token"],
        "invocation": usage.get("invocation").cloned().unwrap_or(Value::Null),
        "phase": usage.get("phase").cloned().unwrap_or(Value::Null),
        "state": usage.get("state").cloned().unwrap_or(Value::Null),
        "currency": usage.get("currency").cloned().unwrap_or(Value::Null),
        "requests": usage.get("requests").cloned().unwrap_or(Value::Null),
        "total": usage.get("total").cloned().unwrap_or(Value::Null),
        "estimate": usage.get("estimate").cloned().unwrap_or(Value::Null),
        "resource": resource["id"],
        "conversation": conversation["id"]
    }))
}

/// The run's phase: an explicit label the manager recorded, else the attempt and the
/// invocation the observation was made in.
fn phase_of(record: &Value) -> &'static str {
    if let Some(label) = record["phase"].as_str() {
        if let Some(found) = PHASES.iter().find(|phase| **phase == label) {
            return found;
        }
    }
    if record["invocation"].as_i64().unwrap_or(1) > 1 {
        return "resume";
    }
    if record["token"].as_i64().unwrap_or(1) > 1 {
        return "retry";
    }
    "initial"
}

/// R343–R346: exact per-run usage.
///
/// A run's numbers come from its own recorded result, never from a rolling account total
/// or the difference of two totals. The same run read twice counts once; a continuation
/// that reports new requests adds only those requests (or replaces the run's cumulative
/// total, which is not a second count). A run the provider reports only account-wide is
/// kept as missing coverage, so the report states a known subtotal and the missing count
/// rather than a wrong complete total. Negative, non-numeric or currency-inconsistent
/// numbers are data faults, not usage.
pub fn usage_facts(resources: &[Value]) -> Value {
    let mut data_faults: Vec<Value> = Vec::new();
    let mut records: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut missing: Vec<Value> = Vec::new();
    let mut aggregates: Vec<Value> = Vec::new();
    for resource in resources {
        if let Some(usage) = resource["usage"].as_object() {
            // A resource-wide aggregate is a measured fact about that resource, but it is not
            // attributable to one provider run; it is reported separately, never as run usage.
            if let Some(facts) = numeric_facts(&Value::Object(usage.clone())) {
                let mut total = BTreeMap::new();
                add_into(&mut total, &facts);
                aggregates.push(json!({
                    "resource": resource["id"], "unit": resource["owner"],
                    "attribution": "resource", "usage": facts_json(&total)
                }));
            } else {
                data_faults.push(json!({
                    "code": "usage_invalid", "resource": resource["id"],
                    "cause": "a resource usage record holds a value that is not a non-negative number"
                }));
            }
        }
        for conversation in resource["conversations"].as_array().cloned().unwrap_or_default() {
            let usage = conversation["usage"].as_object();
            if let Some(usage) = usage {
                if let Some(reason) = usage.get("unavailable").and_then(Value::as_str) {
                    missing.push(json!({
                        "run": usage.get("run").cloned().unwrap_or(conversation["run"].clone()),
                        "conversation": conversation["id"], "resource": resource["id"],
                        "unit": resource["owner"], "reason": reason
                    }));
                    continue;
                }
            }
            match usage_record(resource, &conversation) {
                Some(record) => {
                    let run = record["run"].as_str().unwrap_or_default().to_string();
                    records.entry(run).or_default().push(record);
                }
                None => missing.push(json!({
                    "run": conversation["run"], "conversation": conversation["id"],
                    "resource": resource["id"], "unit": resource["owner"],
                    "reason": "no per-run usage was recorded for this conversation"
                })),
            }
        }
    }
    let mut known: BTreeMap<String, f64> = BTreeMap::new();
    let mut estimated: BTreeMap<String, f64> = BTreeMap::new();
    let mut currencies: BTreeSet<String> = BTreeSet::new();
    let mut runs: Vec<Value> = Vec::new();
    for (run, group) in records {
        let mut first = group[0].clone();
        // Deduplication by provider run id: retrieving the same result again, or reading it
        // through a second conversation, never adds its cost twice.
        for other in &group[1..] {
            let same = other["total"] == first["total"] && other["requests"] == first["requests"];
            if !same {
                data_faults.push(json!({
                    "code": "conflicting_run_usage", "run": run,
                    "cause": "this provider run was recorded with two different usage values"
                }));
            }
        }
        let mut total = BTreeMap::new();
        let mut request_count = 0usize;
        if let Some(requests) = first["requests"].as_object() {
            for (_, request) in requests {
                match numeric_facts(request) {
                    Some(facts) => {
                        request_count += 1;
                        add_into(&mut total, &facts);
                    }
                    None => data_faults.push(json!({
                        "code": "usage_invalid", "run": run,
                        "cause": "a request usage value is not a non-negative number"
                    })),
                }
            }
        } else if !first["total"].is_null() {
            match numeric_facts(&first["total"]) {
                Some(facts) => add_into(&mut total, &facts),
                None => data_faults.push(json!({
                    "code": "usage_invalid", "run": run,
                    "cause": "a run usage value is not a non-negative number"
                })),
            }
        }
        // A recorded estimate is its own labelled fact, never part of the measured charge.
        let mut estimate: BTreeMap<String, f64> = BTreeMap::new();
        if !first["estimate"].is_null() {
            match numeric_facts(&first["estimate"]) {
                Some(facts) => add_into(&mut estimate, &facts),
                None => data_faults.push(json!({
                    "code": "usage_invalid", "run": run,
                    "cause": "an estimate holds a value that is not a non-negative number"
                })),
            }
        }
        if let Some(currency) = first["currency"].as_str().filter(|text| !text.trim().is_empty()) {
            currencies.insert(currency.trim().to_ascii_lowercase());
        }
        add_into(&mut known, &total);
        add_into(&mut estimated, &estimate);
        first["phase"] = json!(phase_of(&first));
        first["requests"] = json!(request_count);
        first["usage"] = facts_json(&total);
        first["estimate"] = facts_json(&estimate);
        runs.push(json!({
            "run": first["run"], "unit": first["unit"], "token": first["token"],
            "phase": first["phase"], "state": first["state"], "usage": first["usage"],
            "estimate": first["estimate"], "requests": first["requests"],
            "resource": first["resource"], "conversation": first["conversation"],
            "invocation": first["invocation"]
        }));
    }
    if currencies.len() > 1 {
        data_faults.push(json!({
            "code": "currency_inconsistent",
            "cause": format!("the recorded runs use {} different currencies: {}", currencies.len(), currencies.iter().cloned().collect::<Vec<_>>().join(", "))
        }));
    }
    let mut phases: BTreeMap<String, Value> = BTreeMap::new();
    for phase in PHASES {
        let of: Vec<&Value> = runs.iter().filter(|run| run["phase"] == json!(phase)).collect();
        let mut total: BTreeMap<String, f64> = BTreeMap::new();
        for run in &of {
            if let Some(facts) = numeric_facts(&run["usage"]) {
                add_into(&mut total, &facts);
            }
        }
        phases.insert(
            phase.to_string(),
            json!({"runs": of.len(), "usage": facts_json(&total)}),
        );
    }
    let status = if data_faults.is_empty() && missing.is_empty() {
        "complete"
    } else if runs.is_empty() {
        "unknown"
    } else {
        "partial"
    };
    json!({
        "status": status,
        "source": "per-run",
        "runs": runs,
        "known_runs": runs.len(),
        "missing_runs": missing.len(),
        "missing": missing,
        "totals": facts_json(&known),
        "estimates": if estimated.is_empty() { Value::Null } else { facts_json(&estimated) },
        "by_phase": phases,
        "currencies": currencies.iter().cloned().collect::<Vec<_>>(),
        "data_faults": data_faults,
        "resource_aggregates": aggregates,
        "account_wide": Value::Null
    })
}

// --- verified placeholders ------------------------------------------------------------------------

static TODO_LINE: std::sync::LazyLock<regex::Regex> =
    std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)(^|\n)\s*待写\s*($|\n)").unwrap());

/// R336: the current placeholder addresses and parts, counted from verified reads.
///
/// A read that fails makes the count `null` with its reason; it never becomes zero. The
/// count is taken at the revision the read reported, so a later edit cannot be described
/// by an earlier read.
pub fn placeholders(zt: &mut Zt, citation_key: &str) -> Value {
    match verified_placeholders(zt, citation_key) {
        Ok(facts) => facts,
        Err(error) => json!({
            "status": "unknown",
            "count": Value::Null,
            "parts": Value::Null,
            "models": [],
            "revision": Value::Null,
            "reason": format!("{error:#}")
        }),
    }
}

fn verified_placeholders(zt: &mut Zt, citation_key: &str) -> Result<Value> {
    let trees = crate::model::read_calls(
        zt,
        &[("list_trees", json!({"workspace": "agent"}))],
    )?
    .into_iter()
    .next()
    .unwrap_or(Value::Null);
    let present = trees["trees"]
        .as_array()
        .map(|list| list.iter().any(|tree| tree["tree"] == json!(citation_key)))
        .unwrap_or(false);
    if !present {
        anyhow::bail!("the book's agent Literature tree `{citation_key}` was not found");
    }
    let tree = crate::model::read_calls(
        zt,
        &[("get_tree", json!({"tree": citation_key}))],
    )?
    .into_iter()
    .next()
    .unwrap_or(Value::Null);
    let revision = tree["archive_revision"].clone();
    let cards: Vec<Value> = tree["cards"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|card| card["type"] == json!("model"))
        .filter(|card| card["facet"].is_null() || card["facet"] == json!(""))
        .collect();
    let mut models = Vec::new();
    let mut count = 0usize;
    let mut parts = 0usize;
    for head in &cards {
        let address = head["address"].as_str().context("a Model Card without an address")?;
        let structure = crate::model::read_calls(zt, &[("get_model", json!({"at": address}))])?
            .into_iter()
            .next()
            .unwrap_or(Value::Null);
        let mut placeholders = Vec::new();
        if TODO_LINE.is_match(head["body"].as_str().unwrap_or_default())
            || TODO_LINE.is_match(structure["head"]["body"].as_str().unwrap_or_default())
        {
            placeholders.push(address.to_string());
        }
        let mut model_parts = 0usize;
        for part in structure["parts"].as_array().cloned().unwrap_or_default() {
            model_parts += 1;
            if TODO_LINE.is_match(part["body"].as_str().unwrap_or_default()) {
                if let Some(at) = part["address"].as_str() {
                    placeholders.push(at.to_string());
                }
            }
        }
        parts += model_parts;
        count += placeholders.len();
        let build = &structure["build"];
        let faults = ["closure", "forward", "unanchored", "beyond_level"]
            .iter()
            .map(|key| build[*key].as_array().map(Vec::len).unwrap_or(0))
            .sum::<usize>();
        models.push(json!({
            "model": address,
            "level": structure["level"],
            "parts": model_parts,
            "placeholders": placeholders,
            "issues": structure["issues"].as_array().map(Vec::len).unwrap_or(0),
            "build_faults": faults
        }));
    }
    Ok(json!({
        "status": "verified",
        "tree": citation_key,
        "revision": revision,
        "count": count,
        "models": models,
        "model_count": models.len(),
        "part_count": parts,
        "reason": Value::Null
    }))
}

// --- rendering ------------------------------------------------------------------------------------

fn text(value: &Value) -> String {
    match value {
        Value::Null => "—".to_string(),
        Value::String(text) if text.is_empty() => "—".to_string(),
        Value::String(text) => plain_path(text),
        other => ps::json_compact(other),
    }
}

/// A canonical Windows path carries the extended-length prefix; a report a reader copies
/// into a command must not (R340 prints an invocation the user can run unchanged).
pub fn plain_path(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) => rest.to_string(),
        None => text.to_string(),
    }
}

fn cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn table(head: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut out = format!("| {} |\n|{}|\n", head.join(" | "), vec!["---"; head.len()].join("|"));
    for row in rows {
        out.push_str(&format!("| {} |\n", row.join(" | ")));
    }
    out
}

/// The human report. It states the same facts as the machine companion, in the order a
/// reader asks them: what ran, whether it completed, what is missing, what it cost, and
/// what a person must decide. Nothing here is a claim the ledger cannot support.
pub fn render(facts: &Value) -> String {
    let identity = &facts["identity"];
    let mut out = String::new();
    out.push_str(&format!("# Run report: {}\n\n", text(&identity["run_id"])));
    out.push_str(
        "Rendered from the run ledger and verified reads. Rendering writes no Card; the same facts give the same report.\n\n",
    );
    out.push_str(&format!(
        "- Book: {} (`{}`), variant `{}`\n",
        text(&identity["book_title"]),
        text(&identity["citation_key"]),
        text(&identity["variant"])
    ));
    out.push_str(&format!(
        "- Invocation {} (resume count {}), enabled layers: {}\n",
        text(&identity["invocation"]),
        text(&identity["resume_count"]),
        identity["enabled_layers"]
            .as_array()
            .map(|layers| layers.iter().map(text).collect::<Vec<_>>().join(", "))
            .unwrap_or_else(|| "—".to_string())
    ));
    out.push_str(&format!(
        "- Source: `{}` (sha256 {}), range hash {}\n",
        text(&identity["source"]["path"]),
        text(&identity["source"]["sha256"]),
        text(&identity["range"]["group_manifest_hash"])
    ));
    out.push_str(&format!(
        "- Archive: {} at `{}`, revision {} (verified: {})\n",
        text(&identity["archive"]["id"]),
        text(&identity["archive"]["path"]),
        text(&identity["archive"]["revision"]),
        text(&identity["archive"]["verified"])
    ));
    out.push_str(&format!(
        "- Workflow: base {} / variant {}; binaries: zt-digest {}, zt-check {}, zt {}\n\n",
        text(&identity["workflow"]["base_version"]),
        text(&identity["workflow"]["variant_version"]),
        text(&identity["binaries"]["zt_digest"]),
        text(&identity["binaries"]["zt_check"]),
        text(&identity["binaries"]["zt"])
    ));

    let completeness = &facts["completeness"];
    out.push_str("## Completeness\n\n");
    out.push_str(&table(
        &["Flag", "Value"],
        vec![
            vec!["`settled`".into(), text(&completeness["settled"])],
            vec!["`content_complete`".into(), text(&completeness["content_complete"])],
            vec!["`quality_complete`".into(), text(&completeness["quality_complete"])],
            vec!["`cleanup_complete`".into(), text(&completeness["cleanup_complete"])],
            vec!["`complete`".into(), text(&completeness["complete"])],
            vec!["quality reason".into(), text(&completeness["quality_reason"])],
            vec!["placeholders verified".into(), text(&completeness["placeholders_verified"])],
        ],
    ));
    out.push('\n');

    let counts = &facts["counts"];
    out.push_str("## Counts per layer\n\n");
    let mut rows = Vec::new();
    if let Some(layers) = counts["per_layer"].as_object() {
        for (layer, value) in layers {
            rows.push(vec![
                cell(layer),
                text(&value["declared"]),
                text(&value["done"]),
                text(&value["noop"]),
                text(&value["refused"]),
                text(&value["failed"]),
                text(&value["blocked"]),
                text(&value["registered"]),
                text(&value["ready"]),
                text(&value["claimed"]),
                text(&value["retry"]),
            ]);
        }
    }
    out.push_str(&table(
        &["Layer", "Declared", "Done", "No-op", "Refused", "Failed", "Blocked", "Registered", "Ready", "Claimed", "Retry"],
        rows,
    ));
    out.push_str(&format!(
        "\nTotal: declared {}, done {}, no-op {}, refused {}, failed {}, blocked {}, open {}.\n\n",
        text(&counts["declared"]),
        text(&counts["done"]),
        text(&counts["noop"]),
        text(&counts["refused"]),
        text(&counts["failed"]),
        text(&counts["blocked"]),
        text(&counts["open"])
    ));
    if let Some(history) = counts["invocations"].as_array().filter(|list| !list.is_empty()) {
        out.push_str("Invocations (initial work and resume history are kept apart):\n\n");
        let rows = history
            .iter()
            .map(|entry| {
                vec![
                    text(&entry["invocation"]),
                    text(&entry["claimed"]),
                    text(&entry["completed"]),
                    text(&entry["failed"]),
                    text(&entry["first_event"]),
                    text(&entry["last_event"]),
                ]
            })
            .collect();
        out.push_str(&table(&["Invocation", "Claimed", "Completed", "Failed", "First event", "Last event"], rows));
        out.push('\n');
    }

    out.push_str("## Gaps\n\n");
    let gaps = facts["gaps"].as_array().cloned().unwrap_or_default();
    if gaps.is_empty() {
        out.push_str("No gap is recorded.\n\n");
    } else {
        let rows = gaps
            .iter()
            .map(|gap| {
                vec![
                    cell(&text(&gap["kind"])),
                    cell(&text(&gap["unit"])),
                    cell(&text(&gap["address"])),
                    cell(&text(&gap["reason"])),
                ]
            })
            .collect();
        out.push_str(&table(&["Kind", "Unit", "Address or file", "Reason"], rows));
        out.push('\n');
    }

    out.push_str("## Placeholders\n\n");
    let placeholders = &facts["placeholders"];
    match placeholders["status"].as_str() {
        Some("verified") => {
            out.push_str(&format!(
                "{} placeholder address(es) in {} Model Card(s) at archive revision {}.\n\n",
                text(&placeholders["count"]),
                text(&placeholders["model_count"]),
                text(&placeholders["revision"])
            ));
            let rows: Vec<Vec<String>> = placeholders["models"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|model| model["placeholders"].as_array().is_some_and(|list| !list.is_empty()))
                .map(|model| {
                    vec![
                        cell(&text(&model["model"])),
                        cell(&text(&model["level"])),
                        text(&model["parts"]),
                        cell(
                            &model["placeholders"]
                                .as_array()
                                .map(|list| list.iter().map(text).collect::<Vec<_>>().join(", "))
                                .unwrap_or_default(),
                        ),
                    ]
                })
                .collect();
            if !rows.is_empty() {
                out.push_str(&table(&["Model", "Level", "Parts", "Placeholder addresses"], rows));
                out.push('\n');
            }
        }
        _ => {
            out.push_str(&format!(
                "Unknown: the placeholder count could not be verified (`{}`). It is not zero.\n\n",
                text(&placeholders["reason"])
            ));
        }
    }

    let timing = &facts["timing"];
    out.push_str("## Timing\n\n");
    out.push_str(&format!(
        "- Wall clock: {} s; queue wait {} s; tool time {} s; external runs {} s\n",
        text(&timing["wall_seconds"]),
        text(&timing["queue_wait_seconds"]),
        text(&timing["tool_seconds"]),
        text(&timing["external_run_seconds"])
    ));
    out.push_str(&format!(
        "- Critical path: {} s ({})\n",
        text(&timing["critical_path"]["seconds"]),
        timing["critical_path"]["units"]
            .as_array()
            .map(|list| list.iter().map(text).collect::<Vec<_>>().join(" → "))
            .unwrap_or_default()
    ));
    out.push_str("- Stage durations are wall-clock spans and are not added into a total: parallel stages overlap.\n\n");
    if let Some(stages) = timing["stages"].as_array().filter(|list| !list.is_empty()) {
        let rows = stages
            .iter()
            .map(|stage| {
                vec![
                    cell(&text(&stage["layer"])),
                    text(&stage["units"]),
                    text(&stage["elapsed_seconds"]),
                    text(&stage["queue_wait_seconds"]),
                    text(&stage["tool_seconds"]),
                    text(&stage["external_run_seconds"]),
                ]
            })
            .collect();
        out.push_str(&table(&["Layer", "Units", "Elapsed s", "Queue s", "Tool s", "External s"], rows));
        out.push('\n');
    }

    let usage = &facts["usage"];
    out.push_str("## DeepSeek usage\n\n");
    match usage["status"].as_str() {
        Some("complete") | Some("partial") => {
            out.push_str(&format!(
                "- Status: {}; known runs {}, missing runs {}\n",
                text(&usage["status"]),
                text(&usage["known_runs"]),
                text(&usage["missing_runs"])
            ));
            out.push_str(&format!("- Known subtotal: {}\n", text(&usage["totals"])));
            if !usage["estimates"].is_null() {
                out.push_str(&format!(
                    "- Labelled estimate (never a measured provider charge): {}\n",
                    text(&usage["estimates"])
                ));
            }
            if let Some(phase) = usage["by_phase"].as_object() {
                let rows = phase
                    .iter()
                    .map(|(name, value)| vec![cell(name), text(&value["runs"]), text(&value["usage"])])
                    .collect();
                out.push_str("\nUsage by phase (initial work, retries, fixes and resume shown apart):\n\n");
                out.push_str(&table(&["Phase", "Runs", "Usage"], rows));
            }
            if usage["missing_runs"].as_u64().unwrap_or(0) > 0 {
                out.push_str("\nMissing coverage (a known subtotal, never a complete total):\n\n");
                let rows = usage["missing"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .map(|entry| {
                        vec![
                            cell(&text(&entry["run"])),
                            cell(&text(&entry["conversation"])),
                            cell(&text(&entry["unit"])),
                            cell(&text(&entry["reason"])),
                        ]
                    })
                    .collect();
                out.push_str(&table(&["Run", "Conversation", "Unit", "Reason"], rows));
            }
            if let Some(faults) = usage["data_faults"].as_array().filter(|list| !list.is_empty()) {
                out.push_str("\nUsage data faults:\n\n");
                let rows = faults
                    .iter()
                    .map(|fault| vec![cell(&text(&fault["code"])), cell(&text(&fault["run"])), cell(&text(&fault["cause"]))])
                    .collect();
                out.push_str(&table(&["Fault", "Run", "Cause"], rows));
            }
            out.push('\n');
        }
        _ => {
            out.push_str("Unknown: no per-run usage was recorded. This is not a zero cost.\n\n");
        }
    }

    out.push_str("## Resume\n\n");
    let resume = &facts["resume"];
    if resume["available"] == true {
        out.push_str(&format!(
            "One supported resume remains. It reuses the retained preparation and runs no other pass:\n\n```javascript\n{}\n```\n\n",
            text(&resume["invocation"])
        ));
    } else if let Some(reason) = resume["reason"].as_str() {
        out.push_str(&format!("No automatic resume: {reason}.\n\n"));
        if let Some(manual) = resume["manual"].as_array().filter(|list| !list.is_empty()) {
            out.push_str("Manual decisions or corrections still required:\n\n");
            for entry in manual {
                out.push_str(&format!("- {}\n", cell(&text(entry))));
            }
            out.push('\n');
        }
    }

    if let Some(bridge) = facts["bridge"].as_object().filter(|value| !value.is_empty()) {
        out.push_str("## Bridge proposals\n\n");
        out.push_str(&format!(
            "- Report `{}`: {} open proposal(s) awaiting optional review, {} accepted, {} refused\n",
            text(&bridge["report_id"]),
            text(&bridge["open"]),
            text(&bridge["accepted"]),
            text(&bridge["refused"])
        ));
        out.push_str("- An open proposal is a review decision, never a missing digest write.\n\n");
    }

    out.push_str("## Latest required publications\n\n");
    let rows: Vec<Vec<String>> = facts["publications"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|publication| {
            vec![
                cell(&text(&publication["layer"])),
                cell(&text(&publication["unit"])),
                cell(&text(&publication["disposition"])),
                text(&publication["current"]),
                cell(&text(&publication["revision"])),
            ]
        })
        .collect();
    if rows.is_empty() {
        out.push_str("None declared.\n");
    } else {
        out.push_str(&table(&["Layer", "Unit", "Disposition", "Current", "Revision"], rows));
    }
    out
}

// --- publishing -----------------------------------------------------------------------------------

/// Write `<out>/run-report.json` and `<out>/run-report.md` from the same facts.
///
/// Idempotent: a file whose bytes did not change is not rewritten, so a reader's copy
/// stays valid and no timestamp moves. The JSON is the machine companion; the Markdown is
/// its rendering. Neither holds an agent's summary.
pub fn publish(out: &Path, facts: &Value) -> Result<Value> {
    let json_text = format!("{}\n", serde_json::to_string_pretty(facts)?);
    let markdown = render(facts);
    let mut written = Map::new();
    for (name, content) in [(REPORT_JSON, json_text), (REPORT_MARKDOWN, markdown)] {
        let path = out.join(name);
        let existing = std::fs::read_to_string(&path).ok();
        let changed = existing.as_deref() != Some(content.as_str());
        if changed {
            crate::util::write_atomic(&path, &content)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        written.insert(
            name.to_string(),
            json!({
                "path": path,
                "sha256": crate::canon::sha256_hex(content.as_bytes()),
                "changed": changed
            }),
        );
    }
    Ok(Value::Object(written))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resource(unit: &str, token: i64, conversations: Value) -> Value {
        json!({
            "id": format!("cleanup/{unit}-t{token}@1#ws"),
            "cleanup_unit": format!("cleanup/{unit}-t{token}@1"),
            "owner": unit, "token": token,
            "conversations": conversations,
            "usage": Value::Null
        })
    }

    fn usage(run: &str, total: Value) -> Value {
        json!({"run": run, "total": total, "invocation": 1})
    }

    #[test]
    fn a_run_read_twice_counts_once_and_a_continuation_adds_only_its_requests() {
        // The same run result read twice is one run, and the second read adds nothing.
        let twice = usage_facts(&[resource(
            "digest/g001@1",
            1,
            json!([
                {"id": "c1", "run": "r1", "usage": usage("r1", json!({"input_tokens": 100, "output_tokens": 40}))},
                {"id": "c1b", "run": "r1", "usage": usage("r1", json!({"input_tokens": 100, "output_tokens": 40}))}
            ]),
        )]);
        assert_eq!(twice["known_runs"], json!(1));
        assert_eq!(twice["totals"]["input_tokens"], json!(100));
        assert_eq!(twice["totals"]["output_tokens"], json!(40));
        assert_eq!(twice["missing_runs"], json!(0));
        assert_eq!(twice["status"], json!("complete"));
        // A continuation reports new request ids: only those requests are added.
        let continued = usage_facts(&[resource(
            "digest/g001@1",
            1,
            json!([{
                "id": "c1", "run": "r1",
                "usage": {"run": "r1", "requests": {
                    "req-1": {"input_tokens": 100, "output_tokens": 40},
                    "req-2": {"input_tokens": 30, "output_tokens": 10}
                }}
            }]),
        )]);
        assert_eq!(continued["totals"]["input_tokens"], json!(130));
        assert_eq!(continued["totals"]["output_tokens"], json!(50));
    }

    #[test]
    fn a_cancelled_run_counts_and_missing_usage_is_explicit() {
        let facts = usage_facts(&[resource(
            "models/m1@1",
            2,
            json!([
                {"id": "c1", "run": "r1", "state": "terminal", "usage": {"run": "r1", "total": {"cost": 0.5}, "state": "cancelled"}},
                {"id": "c2", "run": "r2", "state": "terminal"},
                {"id": "c3", "run": "r3", "state": "terminal", "usage": {"run": "r3", "unavailable": "account_wide_only"}}
            ]),
        )]);
        // The cancelled run still spent money, so it counts.
        assert_eq!(facts["totals"]["cost"], json!(0.5));
        assert_eq!(facts["known_runs"], json!(1));
        // Two runs have no exact usage; the total stays a subtotal, never a complete claim.
        assert_eq!(facts["missing_runs"], json!(2));
        assert_eq!(facts["status"], json!("partial"));
        let reasons: Vec<&str> = facts["missing"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["reason"].as_str().unwrap())
            .collect();
        assert!(reasons.contains(&"account_wide_only"));
    }

    #[test]
    fn a_shrinking_account_total_never_becomes_run_usage_or_a_negative_cost() {
        // A resource-wide aggregate is a measured fact about that resource, but attribute it to
        // no run; it can shrink without ever making the run total negative.
        let mut shrinking = resource("digest/g002@1", 1, json!([{"id": "c1", "run": "r1"}]));
        shrinking["usage"] = json!({"cost": 4.0});
        let before = usage_facts(&[shrinking.clone()]);
        shrinking["usage"] = json!({"cost": 1.0});
        let after = usage_facts(&[shrinking]);
        assert!(before["totals"]["cost"].is_null());
        assert!(after["totals"]["cost"].is_null());
        assert_eq!(before["resource_aggregates"][0]["attribution"], json!("resource"));
        assert_eq!(after["resource_aggregates"][0]["usage"]["cost"], json!(1));
        // The unrecorded run stays missing coverage instead of borrowing the aggregate.
        assert_eq!(after["missing_runs"], json!(1));
    }

    #[test]
    fn negative_and_inconsistent_usage_is_a_data_fault() {
        let negative = usage_facts(&[resource(
            "digest/g003@1",
            1,
            json!([{"id": "c1", "run": "r1", "usage": {"run": "r1", "total": {"cost": -1.5}}}]),
        )]);
        assert_eq!(negative["data_faults"][0]["code"], json!("usage_invalid"));
        let conflicting = usage_facts(&[resource(
            "digest/g003@1",
            1,
            json!([
                {"id": "c1", "run": "r1", "usage": usage("r1", json!({"cost": 1.0}))},
                {"id": "c2", "run": "r1", "usage": usage("r1", json!({"cost": 2.0}))}
            ]),
        )]);
        assert_eq!(conflicting["data_faults"][0]["code"], json!("conflicting_run_usage"));
        let currencies = usage_facts(&[resource(
            "digest/g003@1",
            1,
            json!([
                {"id": "c1", "run": "r1", "usage": {"run": "r1", "total": {"cost": 1.0}, "currency": "usd"}},
                {"id": "c2", "run": "r2", "usage": {"run": "r2", "total": {"cost": 2.0}, "currency": "cny"}}
            ]),
        )]);
        assert_eq!(currencies["data_faults"][0]["code"], json!("currency_inconsistent"));
        assert_eq!(currencies["status"], json!("partial"));
    }

    #[test]
    fn usage_is_reported_by_phase_with_the_retry_and_resume_apart() {
        let facts = usage_facts(&[resource(
            "digest/g004@1",
            2,
            json!([
                {"id": "c1", "run": "r1", "usage": {"run": "r1", "total": {"output_tokens": 10}, "invocation": 1, "phase": "initial"}},
                {"id": "c2", "run": "r2", "usage": {"run": "r2", "total": {"output_tokens": 5}, "invocation": 1}},
                {"id": "c3", "run": "r3", "usage": {"run": "r3", "total": {"output_tokens": 7}, "invocation": 2}}
            ]),
        )]);
        assert_eq!(facts["by_phase"]["initial"]["runs"], json!(1));
        // No explicit label on the second run of a retried unit: the attempt token says retry.
        assert_eq!(facts["by_phase"]["retry"]["runs"], json!(1));
        assert_eq!(facts["by_phase"]["resume"]["runs"], json!(1));
        assert_eq!(facts["by_phase"]["initial"]["usage"]["output_tokens"], json!(10));
        assert_eq!(facts["by_phase"]["resume"]["usage"]["output_tokens"], json!(7));
    }

    #[test]
    fn an_estimate_is_labelled_and_never_added_to_the_measured_charge() {
        let facts = usage_facts(&[resource(
            "digest/g005@1",
            1,
            json!([{"id": "c1", "run": "r1", "usage": {
                "run": "r1", "total": {"input_tokens": 10}, "estimate": {"cost": 0.25}, "currency": "usd"
            }}]),
        )]);
        assert_eq!(facts["totals"]["input_tokens"], json!(10));
        assert!(facts["totals"]["cost"].is_null());
        assert_eq!(facts["estimates"]["cost"], json!(0.25));
    }

    #[test]
    fn a_time_stamp_parses_into_epoch_millis_and_a_bad_one_is_not_zero() {
        assert_eq!(epoch_millis("1970-01-01T00:00:00.0000000Z"), Some(0));
        assert_eq!(epoch_millis("2026-10-18T03:25:23.5Z"), Some(1_792_293_923_500));
        assert_eq!(span_seconds("2026-10-18T03:00:00Z", "2026-10-18T03:00:10.5000000Z"), Some(10.5));
        assert_eq!(epoch_millis("not a time"), None);
        assert_eq!(span_seconds("not a time", "2026-10-18T03:00:10Z"), None);
    }
}
