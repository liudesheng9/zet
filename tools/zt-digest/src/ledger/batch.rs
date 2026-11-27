//! Batch ledger operations for P0-1 (`group-start`, `link-start`, `group-finish`, `link-finish`)
//! and P0-2 (`ledger done -Units|-UnitsFile`, `-UnitsFile`, `-IdsFile`).
//!
//! Each batch command validates its full target list before opening the archive, holds ONE `Zt`
//! client and ONE `Prepared` view for the book, reuses the exact single-unit verifier/transaction
//! per unit under `unit_guard` + scoped `HeartbeatGuard`, and preserves typed lock refusals
//! (exit 2, immediate stop, remaining units marked `not_run`).
use super::*;
use std::collections::HashMap;
use std::time::Instant;

const START_RETRIES: usize = 3;

/// Command-local Literature title export cache shared across fresh `group-start` units whose
/// anchor reads agree on `(archive_identity, archive_revision)`.
pub(super) struct TreeCache {
    pub(super) revision: u64,
    pub(super) trees: Vec<Value>,
    pub(super) own_snapshot: String,
}

fn short(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

pub(crate) fn is_transport_fault(error: &anyhow::Error) -> bool {
    if error.chain().any(|c| c.is::<zt_check::mcp::TransportFault>()) {
        return true;
    }
    let text = format!("{error:#}");
    text.contains("transport_closed")
        || text.contains("transport_timeout")
        || text.contains("service_unavailable")
        || text.contains("zt mcp is closed")
}

pub(super) fn is_archive_identity_fault(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("archive_identity_mismatch")
        || text.contains("archive_identity_changed")
        || text.contains("archive identity mismatch")
}

fn is_read_view_changed(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("read_view_changed")
        || text.contains("changing view")
        || text.contains("changing read view")
}

/// Parses a `-UnitsFile <path>` JSON file containing `[{"id": "...", "token": N}, ...]`
/// (the `units.json` shape) or `{"units": [...]}` or `["unit:token", ...]`.
pub(super) fn parse_units_file(path: &str) -> Result<Vec<(String, i64)>> {
    let doc = util::read_json(path).with_context(|| format!("invalid -UnitsFile {path}"))?;
    let items = if let Some(arr) = doc.as_array() {
        arr
    } else if let Some(arr) = doc.get("units").and_then(Value::as_array) {
        arr
    } else {
        bail!("-UnitsFile must contain a JSON array of {{id, token}} objects or {{\"units\": [...]}}");
    };
    let mut out = Vec::with_capacity(items.len());
    for (idx, item) in items.iter().enumerate() {
        if let Some(text) = item.as_str() {
            let parsed = hold_targets(text)?;
            out.extend(parsed);
            continue;
        }
        let obj = item
            .as_object()
            .with_context(|| format!("-UnitsFile entry {idx} must be an object or \"unit:token\" string"))?;
        let unit = obj
            .get("id")
            .or_else(|| obj.get("unit"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .with_context(|| format!("-UnitsFile entry {idx} is missing a nonempty id/unit"))?;
        let token = obj
            .get("token")
            .and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse::<i64>().ok())))
            .filter(|t| *t > 0)
            .with_context(|| format!("-UnitsFile entry {idx} ({unit}) needs a positive integer token"))?;
        out.push((unit.to_string(), token));
    }
    if out.is_empty() {
        bail!("-UnitsFile names no unit");
    }
    Ok(out)
}

/// Parses `-IdsFile <path>` containing `["unit1", ...]` or `[{"id": "unit1"}, ...]` or `{"ids": [...]}`.
pub(super) fn parse_ids_file(path: &str) -> Result<BTreeSet<String>> {
    let doc = util::read_json(path).with_context(|| format!("invalid -IdsFile {path}"))?;
    let items = if let Some(arr) = doc.as_array() {
        arr
    } else if let Some(arr) = doc
        .get("ids")
        .or_else(|| doc.get("units"))
        .and_then(Value::as_array)
    {
        arr
    } else {
        bail!("-IdsFile must contain a JSON array of unit ids or {{\"ids\": [...]}}");
    };
    let mut ids = BTreeSet::new();
    for (idx, item) in items.iter().enumerate() {
        let id = if let Some(s) = item.as_str() {
            s.trim()
        } else if let Some(obj) = item.as_object() {
            obj.get("id")
                .or_else(|| obj.get("unit"))
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
        } else {
            ""
        };
        if id.is_empty() {
            bail!("-IdsFile entry {idx} is missing a nonempty unit id");
        }
        ids.insert(id.to_string());
    }
    if ids.is_empty() {
        bail!("-IdsFile names no unit id");
    }
    Ok(ids)
}

/// Validates that a target list is nonempty, has positive tokens, and names each unit ID once
/// (rejecting duplicates even with different tokens).
pub(super) fn validate_unique_targets(targets: Vec<(String, i64)>) -> Result<Vec<(String, i64)>> {
    if targets.is_empty() {
        bail!("at least one <unit>:<token> target is required");
    }
    let mut seen = BTreeSet::new();
    for (unit, token) in &targets {
        if unit.trim().is_empty() {
            bail!("target unit id must not be empty");
        }
        if *token <= 0 {
            bail!("target {unit} requires a positive token");
        }
        if !seen.insert(unit.clone()) {
            bail!("target list names {unit} twice");
        }
    }
    Ok(targets)
}

/// Validates targets for `group-start`/`group-finish` (`layer == "digest"`) or
/// `link-start`/`link-finish` (`layer == "link"`), rejecting noncanonical unit IDs that could
/// alias another unit's group number.
pub(super) fn parse_layer_group_targets(
    layer: &str,
    raw: Vec<(String, i64)>,
) -> Result<Vec<(String, u64, i64)>> {
    let targets = validate_unique_targets(raw)?;
    let prefix = format!("{layer}/g");
    let mut out = Vec::with_capacity(targets.len());
    let mut seen_groups = BTreeSet::new();
    for (unit, token) in targets {
        let number = unit
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_suffix("@1"))
            .and_then(|digits| digits.parse::<u64>().ok())
            .filter(|n| *n > 0)
            .with_context(|| format!("{unit} is not a canonical {layer}/gNNN@1 unit"))?;
        let canonical = format!("{layer}/g{number:03}@1");
        if unit != canonical {
            bail!("{unit} is not canonical; expected {canonical}");
        }
        if !seen_groups.insert(number) {
            bail!("target list names group {number} twice");
        }
        out.push((unit, number, token));
    }
    Ok(out)
}

/// Runs `group-start` for every named `(unit, group, token)` in one process with one `Zt` and
/// one `Prepared`, sharing the Literature title export cache across fresh groups at a stable
/// archive revision while keeping per-group retries on `read_view_changed`.
pub(super) fn group_start_batch(
    p: &Prepared,
    zt: &mut Zt,
    targets: &[(String, u64, i64)],
) -> Result<(Value, i32)> {
    let mut cache: Option<TreeCache> = None;
    let mut entries = Vec::with_capacity(targets.len());
    let mut records = Vec::with_capacity(targets.len());
    let mut lock: Option<Value> = None;
    let mut stop_reason: Option<&'static str> = None;
    for (idx, (unit, number, token)) in targets.iter().enumerate() {
        if lock.is_some() || stop_reason.is_some() {
            entries.push(json!({"unit": unit, "token": token, "group": number, "status": "not_run"}));
            records.push(json!({"unit": unit, "token": token, "group": number, "status": "not_run"}));
            continue;
        }
        if idx > 0
            && let Err(error) = verify_live_archive_identity(p, zt)
        {
            let msg = format!("{error:#}");
            if is_transport_fault(&error) {
                stop_reason = Some("transport");
            } else {
                stop_reason = Some("archive_identity");
            }
            entries.push(json!({
                "unit": unit, "token": token, "group": number, "status": "error", "error": short(&msg, 300),
            }));
            records.push(json!({
                "unit": unit, "token": token, "group": number, "status": "error", "error": short(&msg, 800),
            }));
            continue;
        }
        let mut last_err: Option<anyhow::Error> = None;
        let mut started_val: Option<Value> = None;
        for attempt in 1..=START_RETRIES {
            match group_start_with_cache(p, zt, *number, *token, &mut cache) {
                Ok(val) => {
                    started_val = Some(val);
                    break;
                }
                Err(error) => {
                    let retryable_view = is_read_view_changed(&error);
                    if retryable_view {
                        cache = None;
                    }
                    if retryable_view && attempt < START_RETRIES {
                        continue;
                    }
                    last_err = Some(error);
                    break;
                }
            }
        }
        if let Some(val) = started_val {
            entries.push(json!({
                "unit": unit,
                "token": token,
                "group": number,
                "status": "started",
                "job": val["job"],
                "draft": val["draft"],
                "input_hash": val["input_hash"],
                "expected_jobs": val["expected_jobs"],
                "recovered": val["recovered"],
            }));
            records.push(json!({"unit": unit, "token": token, "group": number, "status": "started", "result": val}));
        } else if let Some(error) = last_err {
            let msg = format!("{error:#}");
            if let Some(refusal) = crate::lockcheck::stopped_refusal(&error) {
                let code = crate::lockcheck::refusal_code(&refusal);
                lock = Some(refusal);
                stop_reason = Some(code);
                entries.push(json!({
                    "unit": unit, "token": token, "group": number, "status": "error",
                    "code": code, "error": short(&msg, 300),
                }));
                records.push(json!({
                    "unit": unit, "token": token, "group": number, "status": "error",
                    "code": code, "error": short(&msg, 800),
                }));
                continue;
            }
            if is_transport_fault(&error) {
                stop_reason = Some("transport");
            } else if is_archive_identity_fault(&error) {
                stop_reason = Some("archive_identity");
            }
            entries.push(json!({
                "unit": unit,
                "token": token,
                "group": number,
                "status": "error",
                "error": short(&msg, 300),
            }));
            records.push(json!({
                "unit": unit,
                "token": token,
                "group": number,
                "status": "error",
                "error": short(&msg, 800),
            }));
        }
    }
    let count = |status: &str| entries.iter().filter(|e| e["status"] == json!(status)).count();
    let names: Vec<String> = targets.iter().map(|(u, _, t)| format!("{u}:{t}")).collect();
    let folder = p.out.join("start");
    let file = folder.join(format!(
        "group-start-{}.json",
        &canon::sha256_hex(names.join(",").as_bytes())[..12]
    ));
    let persist_res = std::fs::create_dir_all(&folder).and_then(|_| {
        let payload = serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "kind": "group_start_batch",
            "stop_reason": stop_reason,
            "units": records,
        }))
        .map_err(std::io::Error::other)?;
        util::write_atomic(&file, &payload).map_err(std::io::Error::other)
    });
    let mut detail_error: Option<String> = None;
    if let Err(err) = persist_res {
        if lock.is_none() {
            return Err(err.into());
        }
        detail_error = Some(short(&format!("{err:#}"), 200));
    }
    let failed = count("error");
    let not_run = count("not_run");
    let mut line = json!({
        "schema_version": 1,
        "kind": "group_start_batch",
        "ok": failed == 0 && not_run == 0 && lock.is_none(),
        "units": entries.len(),
        "started": count("started"),
        "failed": failed,
        "not_run": not_run,
        "results": entries,
        "file": file.display().to_string(),
    });
    if let Some(reason) = stop_reason {
        line["stop_reason"] = json!(reason);
    }
    if let Some(err) = detail_error {
        line["detail_error"] = json!(err);
    }
    let mut exit = 0;
    if let Some(refusal) = lock {
        let code = crate::lockcheck::refusal_code(&refusal);
        line["code"] = json!(code);
        line["error"] = json!(code);
        line["message"] = refusal["message"]
            .as_str()
            .or_else(|| refusal["error"].as_str())
            .map(|t| json!(t))
            .unwrap_or(Value::Null);
        line["refusal"] = refusal;
        exit = crate::lockcheck::LOCKED_EXIT;
    }
    Ok((line, exit))
}

/// Runs `link-start` for every named `(unit, group, token)` in one process with one `Zt` and
/// one `Prepared`, retrying up to `START_RETRIES` times on `read_view_changed`.
pub(super) fn link_start_batch(
    p: &Prepared,
    zt: &mut Zt,
    targets: &[(String, u64, i64)],
) -> Result<(Value, i32)> {
    let mut entries = Vec::with_capacity(targets.len());
    let mut records = Vec::with_capacity(targets.len());
    let mut lock: Option<Value> = None;
    let mut stop_reason: Option<&'static str> = None;
    for (idx, (unit, number, token)) in targets.iter().enumerate() {
        if lock.is_some() || stop_reason.is_some() {
            entries.push(json!({"unit": unit, "token": token, "group": number, "status": "not_run"}));
            records.push(json!({"unit": unit, "token": token, "group": number, "status": "not_run"}));
            continue;
        }
        if idx > 0
            && let Err(error) = verify_live_archive_identity(p, zt)
        {
            let msg = format!("{error:#}");
            if is_transport_fault(&error) {
                stop_reason = Some("transport");
            } else {
                stop_reason = Some("archive_identity");
            }
            entries.push(json!({
                "unit": unit, "token": token, "group": number, "status": "error", "error": short(&msg, 300),
            }));
            records.push(json!({
                "unit": unit, "token": token, "group": number, "status": "error", "error": short(&msg, 800),
            }));
            continue;
        }
        let mut last_err: Option<anyhow::Error> = None;
        let mut started_val: Option<Value> = None;
        for attempt in 1..=START_RETRIES {
            match link_start(p, zt, *number, *token) {
                Ok(val) => {
                    started_val = Some(val);
                    break;
                }
                Err(error) => {
                    if is_read_view_changed(&error) && attempt < START_RETRIES {
                        continue;
                    }
                    last_err = Some(error);
                    break;
                }
            }
        }
        if let Some(val) = started_val {
            entries.push(json!({
                "unit": unit,
                "token": token,
                "group": number,
                "status": "started",
                "job": val["job"],
                "draft": val["draft"],
                "input_hash": val["input_hash"],
                "expected_jobs": val["expected_jobs"],
                "counts": val["counts"],
            }));
            records.push(json!({"unit": unit, "token": token, "group": number, "status": "started", "result": val}));
        } else if let Some(error) = last_err {
            let msg = format!("{error:#}");
            if let Some(refusal) = crate::lockcheck::stopped_refusal(&error) {
                let code = crate::lockcheck::refusal_code(&refusal);
                lock = Some(refusal);
                stop_reason = Some(code);
                entries.push(json!({
                    "unit": unit, "token": token, "group": number, "status": "error",
                    "code": code, "error": short(&msg, 300),
                }));
                records.push(json!({
                    "unit": unit, "token": token, "group": number, "status": "error",
                    "code": code, "error": short(&msg, 800),
                }));
                continue;
            }
            if is_transport_fault(&error) {
                stop_reason = Some("transport");
            } else if is_archive_identity_fault(&error) {
                stop_reason = Some("archive_identity");
            }
            entries.push(json!({
                "unit": unit,
                "token": token,
                "group": number,
                "status": "error",
                "error": short(&msg, 300),
            }));
            records.push(json!({
                "unit": unit,
                "token": token,
                "group": number,
                "status": "error",
                "error": short(&msg, 800),
            }));
        }
    }
    let count = |status: &str| entries.iter().filter(|e| e["status"] == json!(status)).count();
    let names: Vec<String> = targets.iter().map(|(u, _, t)| format!("{u}:{t}")).collect();
    let folder = p.out.join("start");
    let file = folder.join(format!(
        "link-start-{}.json",
        &canon::sha256_hex(names.join(",").as_bytes())[..12]
    ));
    let persist_res = std::fs::create_dir_all(&folder).and_then(|_| {
        let payload = serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "kind": "link_start_batch",
            "stop_reason": stop_reason,
            "units": records,
        }))
        .map_err(std::io::Error::other)?;
        util::write_atomic(&file, &payload).map_err(std::io::Error::other)
    });
    let mut detail_error: Option<String> = None;
    if let Err(err) = persist_res {
        if lock.is_none() {
            return Err(err.into());
        }
        detail_error = Some(short(&format!("{err:#}"), 200));
    }
    let failed = count("error");
    let not_run = count("not_run");
    let mut line = json!({
        "schema_version": 1,
        "kind": "link_start_batch",
        "ok": failed == 0 && not_run == 0 && lock.is_none(),
        "units": entries.len(),
        "started": count("started"),
        "failed": failed,
        "not_run": not_run,
        "results": entries,
        "file": file.display().to_string(),
    });
    if let Some(reason) = stop_reason {
        line["stop_reason"] = json!(reason);
    }
    if let Some(err) = detail_error {
        line["detail_error"] = json!(err);
    }
    let mut exit = 0;
    if let Some(refusal) = lock {
        let code = crate::lockcheck::refusal_code(&refusal);
        line["code"] = json!(code);
        line["error"] = json!(code);
        line["message"] = refusal["message"]
            .as_str()
            .or_else(|| refusal["error"].as_str())
            .map(|t| json!(t))
            .unwrap_or(Value::Null);
        line["refusal"] = refusal;
        exit = crate::lockcheck::LOCKED_EXIT;
    }
    Ok((line, exit))
}

/// Runs checked finishes for `digest` (`is_link == false`) or `link` (`is_link == true`) units
/// in one process with one `Zt`, one `Prepared`, token-bound draft resolution under `unit_guard`,
/// command-entry deadline enforcement, immediate typed planning-stop (`card_locked`/`scope_refused`,
/// exit 2), archive-identity mismatch stop, and transport-death stop.
pub(super) fn group_or_link_finish_batch(
    p: &Prepared,
    zt: &mut Zt,
    targets: &[(String, u64, i64)],
    is_link: bool,
    started: Instant,
) -> Result<(Value, i32)> {
    let layer = if is_link { "link" } else { "digest" };
    let kind = if is_link {
        "link_finish_batch"
    } else {
        "digest_finish_batch"
    };
    let budget = finish_batch::budget_seconds(p);
    let mut entries = Vec::with_capacity(targets.len());
    let mut records = Vec::with_capacity(targets.len());
    let mut remaining = Vec::new();
    let mut lock: Option<Value> = None;
    let mut stop_reason: Option<&'static str> = None;

    for (idx, (unit, number, token)) in targets.iter().enumerate() {
        if lock.is_some() || stop_reason.is_some() || started.elapsed().as_secs() >= budget {
            if stop_reason.is_none() && lock.is_none() {
                stop_reason = Some("budget");
            }
            entries.push(json!({"unit": unit, "token": token, "group": number, "status": "not_run"}));
            records.push(json!({"unit": unit, "token": token, "group": number, "status": "not_run"}));
            remaining.push(json!({"id": unit, "token": token}));
            continue;
        }
        if idx > 0
            && let Err(error) = verify_live_archive_identity(p, zt)
        {
            let msg = format!("{error:#}");
            stop_reason = Some(if is_transport_fault(&error) {
                "transport"
            } else {
                "archive_identity"
            });
            entries.push(json!({
                "unit": unit, "token": token, "group": number,
                "status": "not_run", "cause": short(&msg, 200),
            }));
            records.push(json!({
                "unit": unit, "token": token, "group": number,
                "status": "not_run", "cause": short(&msg, 800),
            }));
            remaining.push(json!({"id": unit, "token": token}));
            continue;
        }
        let outcome = if is_link {
            link_finish(p, zt, *number, *token, None, false)
        } else {
            group_finish(p, zt, *number, *token, None, false)
                .and_then(|opt| opt.context("group completion returned no result"))
        };
        let (entry, record, refusal, transport_dead, archive_mismatch) =
            settle_group_or_link(p, unit, *number, *token, is_link, outcome);
        entries.push(entry);
        records.push(record);
        if let Some(r) = refusal {
            let code = crate::lockcheck::refusal_code(&r);
            lock = Some(r);
            stop_reason = Some(code);
        } else if archive_mismatch {
            stop_reason = Some("archive_identity");
        } else if transport_dead {
            stop_reason = Some("transport");
        }
    }

    let count = |status: &str| entries.iter().filter(|e| e["status"] == json!(status)).count();
    let names: Vec<String> = targets.iter().map(|(u, _, t)| format!("{u}:{t}")).collect();
    let folder = p.out.join("finish");
    let file = folder.join(format!(
        "{layer}-{}.json",
        &canon::sha256_hex(names.join(",").as_bytes())[..12]
    ));
    let persist_res = std::fs::create_dir_all(&folder).and_then(|_| {
        let payload = serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "kind": kind,
            "stop_reason": stop_reason,
            "remaining": remaining,
            "units": records,
        }))
        .map_err(std::io::Error::other)?;
        util::write_atomic(&file, &payload).map_err(std::io::Error::other)
    });
    let mut detail_error: Option<String> = None;
    if let Err(err) = persist_res {
        if lock.is_none() {
            return Err(err.into());
        }
        detail_error = Some(short(&format!("{err:#}"), 200));
    }

    let mut line = json!({
        "schema_version": 1,
        "kind": kind,
        "ok": lock.is_none() && !matches!(stop_reason, Some("transport" | "archive_identity")),
        "units": entries.len(),
        "done": count("done"),
        "partial": count("partial"),
        "retry": count("retry"),
        "failed": count("failed"),
        "not_run": count("not_run"),
        "results": entries,
        "file": file.display().to_string(),
    });
    if let Some(reason) = stop_reason {
        line["stop_reason"] = json!(reason);
    }
    if let Some(err) = detail_error {
        line["detail_error"] = json!(err);
    }
    let mut exit = 0;
    if let Some(refusal) = lock {
        let code = crate::lockcheck::refusal_code(&refusal);
        line["code"] = json!(code);
        line["error"] = json!(code);
        line["message"] = refusal["message"]
            .as_str()
            .or_else(|| refusal["error"].as_str())
            .map(|t| json!(t))
            .unwrap_or(Value::Null);
        line["refusal"] = refusal;
        exit = crate::lockcheck::LOCKED_EXIT;
    }
    Ok((line, exit))
}

fn settle_group_or_link(
    p: &Prepared,
    unit: &str,
    number: u64,
    token: i64,
    is_link: bool,
    finished: Result<Value>,
) -> (Value, Value, Option<Value>, bool, bool) {
    let transport_dead = finished.as_ref().err().is_some_and(is_transport_fault);
    let archive_mismatch = finished.as_ref().err().is_some_and(is_archive_identity_fault);
    let cause = match &finished {
        Ok(v) if v["state"] == json!("done") || v["complete"] == json!(true) => {
            let status = if v["status"] == json!("partial") {
                "partial"
            } else {
                "done"
            };
            let mut entry = json!({
                "unit": unit,
                "token": token,
                "group": number,
                "status": status,
                "disposition": v["disposition"],
            });
            if !is_link {
                if let Some(owned) = v["owned"].as_array() {
                    entry["owned"] = json!(owned.len());
                }
                if v["recovered"] == true {
                    entry["recovered"] = json!(true);
                }
            } else {
                entry["counts"] = v["counts"].clone();
                entry["quality"] = v["quality"].clone();
                entry["changed"] = v["changed"].clone();
            }
            return (
                entry,
                json!({"unit": unit, "token": token, "group": number, "status": status, "result": v}),
                None,
                false,
                false,
            );
        }
        Ok(v) => v["fault"]
            .as_str()
            .or_else(|| v["error"].as_str())
            .map(str::to_string)
            .or_else(|| (!v["fault"].is_null()).then(|| util::compact(&v["fault"])))
            .unwrap_or_else(|| "the checked completion did not complete".into()),
        Err(error) => format!("{error:#}"),
    };
    let cause = short(&cause, 800);
    let result = finished.as_ref().ok().cloned().unwrap_or(Value::Null);
    if let Err(error) = &finished
        && let Some(refusal) = crate::lockcheck::stopped_refusal(error)
    {
        let code = crate::lockcheck::refusal_code(&refusal);
        let fault = json!({
            "schema_version": 1,
            "unit": unit,
            "input_hash": "",
            "code": code,
            "cause": short(&format!("{cause} {}", util::compact(&refusal)), 800),
        });
        let recorded = fail(p, unit, token, "content", &fault).is_ok();
        return (
            json!({
                "unit": unit, "token": token, "group": number, "status": "failed",
                "code": code, "cause": short(&cause, 200), "recorded": recorded,
            }),
            json!({
                "unit": unit, "token": token, "group": number, "status": "failed",
                "fault": fault, "recorded": recorded,
            }),
            Some(refusal),
            false,
            false,
        );
    }
    if archive_mismatch {
        return (
            json!({
                "unit": unit, "token": token, "group": number, "status": "not_run",
                "code": "archive_identity_mismatch", "cause": short(&cause, 200), "recorded": false,
            }),
            json!({
                "unit": unit, "token": token, "group": number, "status": "not_run",
                "code": "archive_identity_mismatch", "cause": cause, "recorded": false,
            }),
            None,
            transport_dead,
            true,
        );
    }
    match finish_batch::held_state(p, unit) {
        Ok(Some((state, _, _, disposition))) if state == "done" => {
            // Only stale-token replays or post-commit hook failures count as already-done; an
            // actual integrity/verification error or transport death on a done unit must surface
            // instead of masquerading as a clean verified pass.
            let stale_or_post_commit = !transport_dead
                && (cause.contains("stale_claim")
                    || cause.contains("injected completion fault: after_commit"));
            if stale_or_post_commit {
                return (
                    json!({
                        "unit": unit, "token": token, "group": number,
                        "status": "done", "disposition": disposition, "from": "ledger",
                    }),
                    json!({
                        "unit": unit, "token": token, "group": number,
                        "status": "done", "from": "ledger", "result": result, "cause": cause,
                    }),
                    None,
                    false,
                    false,
                );
            }
            let code = cause_code(&cause);
            return (
                json!({
                    "unit": unit, "token": token, "group": number,
                    "status": "failed", "ledger_state": "done", "verified": false,
                    "disposition": disposition, "code": code,
                    "cause": short(&cause, 200), "recorded": false,
                }),
                json!({
                    "unit": unit, "token": token, "group": number,
                    "status": "failed", "ledger_state": "done", "verified": false,
                    "disposition": disposition, "code": code,
                    "result": result, "cause": cause, "recorded": false,
                }),
                None,
                transport_dead,
                false,
            );
        }
        Ok(Some((state, now, _, _))) if state != "claimed" || now != token => {
            let status = if state == "failed" { "failed" } else { "retry" };
            return (
                json!({
                    "unit": unit, "token": token, "group": number, "status": status,
                    "cause": short(&format!("the ledger holds {unit} {state}"), 200),
                    "recorded": false,
                }),
                json!({
                    "unit": unit, "token": token, "group": number, "status": status,
                    "ledger_state": state, "result": result, "cause": cause,
                }),
                None,
                transport_dead,
                false,
            );
        }
        _ => {}
    }
    let code = cause_code(&cause);
    let class = fault_class(code).unwrap_or("content");
    let fault = json!({"schema_version": 1, "unit": unit, "input_hash": "", "code": code, "cause": cause});
    let (status, recorded, refused) = match fail(p, unit, token, class, &fault) {
        Ok(res) => {
            let actual = if res["state"].as_str() == Some("failed") {
                "failed"
            } else {
                "retry"
            };
            (actual, true, Value::Null)
        }
        Err(error) => (
            if class == "retryable" { "retry" } else { "failed" },
            false,
            json!(short(&format!("{error:#}"), 300)),
        ),
    };
    (
        json!({
            "unit": unit, "token": token, "group": number,
            "status": status, "code": code, "cause": short(&cause, 200), "recorded": recorded,
        }),
        json!({
            "unit": unit, "token": token, "group": number,
            "status": status, "fault": fault, "recorded": recorded,
            "refused": refused, "result": result,
        }),
        None,
        transport_dead,
        false,
    )
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum DoneBatchFamily {
    TermCardsJob,
    CitedCardsJob,
}

/// Validates that all targets in `ledger done -Units|-UnitsFile` belong to one shared-readiness
/// job family (`termcards/t*` OR `citedcards/t*`). Book publication units keep single-unit `done`.
pub(super) fn validate_done_batch_targets(
    raw: Vec<(String, i64)>,
) -> Result<(DoneBatchFamily, Vec<(String, i64)>)> {
    let targets = validate_unique_targets(raw)?;
    let mut family: Option<DoneBatchFamily> = None;
    for (unit, _) in &targets {
        let f = if let Some(rest) = unit.strip_prefix("termcards/") {
            if rest.starts_with("book@") {
                bail!("ledger done batch supports job units only; book unit {unit} uses single-unit done");
            }
            DoneBatchFamily::TermCardsJob
        } else if let Some(rest) = unit.strip_prefix("citedcards/") {
            if rest.starts_with("book@") {
                bail!("ledger done batch supports job units only; book unit {unit} uses single-unit done");
            }
            DoneBatchFamily::CitedCardsJob
        } else {
            bail!("ledger done batch supports termcards or citedcards job units only, not {unit}");
        };
        match family {
            Some(prev) if prev != f => {
                bail!("ledger done batch cannot mix termcards and citedcards units in one call");
            }
            None => family = Some(f),
            _ => {}
        }
    }
    Ok((family.unwrap(), targets))
}

/// Completes a batch of Term-card or Cited-card job units against ONE parsed, hashed, indexed
/// `readiness.json` snapshot. Does not fault or release claims on failure: callers inspect
/// per-unit `status` (`done` | `stale_claim` | `error`) to drive stale recovery, Term `-Retry`,
/// or final content failure.
pub(super) fn done_batch(
    p: &Prepared,
    family: DoneBatchFamily,
    targets: &[(String, i64)],
    outputs_path: &Path,
) -> Result<Value> {
    let (expected_path, expected_kind, array_key) = match family {
        DoneBatchFamily::TermCardsJob => (
            p.out.join("term-cards").join("check").join("readiness.json"),
            "term_cards_readiness",
            "terms",
        ),
        DoneBatchFamily::CitedCardsJob => (
            p.out.join("cited-cards").join("check").join("readiness.json"),
            "cited_cards_readiness",
            "rows",
        ),
    };
    if !expected_path.is_file() {
        bail!(
            "the readiness artifact `{}` is missing; rerun -Step check",
            path_to_string(&expected_path)
        );
    }
    let canon_given = std::fs::canonicalize(outputs_path).context("missing completion evidence")?;
    let canon_expected = std::fs::canonicalize(&expected_path)?;
    if canon_given != canon_expected {
        bail!("completion evidence locator mismatch");
    }
    let raw_bytes = std::fs::read(&canon_given).context("failed to read readiness evidence")?;
    let snapshot_sha = canon::sha256_hex(&raw_bytes);
    let raw_text = std::str::from_utf8(&raw_bytes)
        .context("readiness evidence is not valid UTF-8")?;
    let artifact = canon::parse_json(raw_text).context("invalid readiness JSON evidence")?;
    if artifact["schema_version"] != 1 || artifact["kind"] != expected_kind {
        bail!("not a {expected_kind} artifact");
    }

    // Build an O(1) lookup table keyed by (job_id, row_number) once for the entire batch,
    // refusing duplicate keys before any unit commits so batch verification can never diverge from
    // first-match single-unit verification.
    let mut index: HashMap<(String, u64), Value> = HashMap::new();
    if let Some(items) = artifact[array_key].as_array() {
        for item in items {
            if let (Some(job_id), Some(row_num)) = (
                item["job"].as_str(),
                item["row"].as_u64(),
            ) {
                let key = (job_id.to_string(), row_num);
                if index.insert(key, item.clone()).is_some() {
                    bail!("duplicate readiness entry for job `{job_id}` row {row_num}");
                }
            }
        }
    }

    // Retain an immutable content-addressed copy of the readiness generation used by committed rows.
    let snapshot_copy = expected_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("readiness-{}.json", &snapshot_sha[..12]));
    if !snapshot_copy.is_file() {
        util::write_atomic(&snapshot_copy, raw_text)?;
    }

    let mut results = Vec::with_capacity(targets.len());
    let mut done_n = 0usize;
    let mut stale_n = 0usize;
    let mut error_n = 0usize;
    let mut not_run_n = 0usize;
    let mut stop_reason: Option<&'static str> = None;

    for (unit, token) in targets {
        if stop_reason.is_some() {
            not_run_n += 1;
            results.push(json!({
                "unit": unit,
                "token": token,
                "status": "not_run",
            }));
            continue;
        }
        let outcome = match family {
            DoneBatchFamily::TermCardsJob => termcards_job_finish_indexed(
                p,
                unit,
                *token,
                outputs_path,
                &canon_given,
                &snapshot_copy,
                &artifact,
                &snapshot_sha,
                &index,
            ),
            DoneBatchFamily::CitedCardsJob => citedcards_job_finish_indexed(
                p,
                unit,
                *token,
                outputs_path,
                &canon_given,
                &snapshot_copy,
                &artifact,
                &snapshot_sha,
                &index,
            ),
        };
        match outcome {
            Ok(v) => {
                done_n += 1;
                let mut entry = json!({
                    "unit": unit,
                    "token": token,
                    "status": "done",
                });
                if v["reused"] == true {
                    entry["reused"] = json!(true);
                }
                results.push(entry);
            }
            Err(error) => {
                let text = format!("{error:#}");
                if text.contains("readiness evidence changed during batch completion") {
                    stop_reason = Some("readiness_changed");
                    not_run_n += 1;
                    results.push(json!({
                        "unit": unit,
                        "token": token,
                        "status": "not_run",
                        "code": "readiness_changed",
                        "cause": short(&text, 300),
                    }));
                    continue;
                }
                let held = finish_batch::held_state(p, unit).ok().flatten();
                let ledger_state = held.as_ref().map(|(s, _, _, _)| s.as_str());
                let committed = held
                    .as_ref()
                    .is_some_and(|(s, tok, _, _)| s == "done" && *tok == *token);
                if text.contains("stale_claim") {
                    stale_n += 1;
                    let mut entry = json!({
                        "unit": unit,
                        "token": token,
                        "status": "stale_claim",
                        "cause": short(&text, 300),
                    });
                    if let Some(st) = ledger_state {
                        entry["ledger_state"] = json!(st);
                    }
                    results.push(entry);
                } else {
                    error_n += 1;
                    let mut entry = json!({
                        "unit": unit,
                        "token": token,
                        "status": "error",
                        "code": cause_code(&text),
                        "cause": short(&text, 300),
                    });
                    if let Some(st) = ledger_state {
                        entry["ledger_state"] = json!(st);
                    }
                    if committed {
                        entry["committed"] = json!(true);
                    }
                    results.push(entry);
                }
            }
        }
    }

    let mut reply = json!({
        "schema_version": 1,
        "kind": "ledger_done_batch",
        "ok": stale_n == 0 && error_n == 0 && not_run_n == 0 && stop_reason.is_none(),
        "units": results.len(),
        "done": done_n,
        "stale": stale_n,
        "errors": error_n,
        "not_run": not_run_n,
        "snapshot": path_to_string(&snapshot_copy),
        "results": results,
    });
    if let Some(reason) = stop_reason {
        reply["stop_reason"] = json!(reason);
    }
    Ok(reply)
}

#[allow(clippy::too_many_arguments)]
fn termcards_job_finish_indexed(
    p: &Prepared,
    unit: &str,
    token: i64,
    path: &Path,
    canon_path: &Path,
    snapshot_copy: &Path,
    artifact: &Value,
    snapshot_sha: &str,
    index: &HashMap<(String, u64), Value>,
) -> Result<Value> {
    let rest = unit
        .strip_prefix("termcards/")
        .context("not a Term-card unit")?;
    let (key, revision) = rest
        .split_once('@')
        .context("invalid Term-card unit")?;
    let revision: i64 = revision.parse().context("invalid Term-card revision")?;
    let inputs = unit_inputs(p, unit)?;
    let kind = field(&inputs, "kind")?;
    if inputs["schema_version"] != 1 || kind != "term_card_job" || key == "book" {
        bail!("the Term-card unit `{unit}` does not carry a job plan input");
    }
    if inputs["plan_revision"].as_i64() != Some(revision) {
        bail!("the Term-card unit `{unit}` is bound to another plan revision");
    }
    finish_claim_checked(p, unit, token)?;
    let _guard = unit_guard(p, unit)?;
    // Verify claim/done state and row readiness BEFORE touching the claim's activity timestamp
    // with a heartbeat renewal, so unready rows stay releasable for `-Retry`.
    let (_claimed, _progress, _run, done) = finish_claim_checked(p, unit, token)?;
    if done {
        return Ok(json!({"schema_version":1,"unit":unit,"state":"done","reused":true}));
    }
    let bound = |value: &Value, expected: &Value| -> Result<()> {
        if value != expected {
            bail!("the Term-card readiness is bound to another plan; rerun -Step plan");
        }
        Ok(())
    };
    bound(&artifact["book"], &inputs["book"])?;
    bound(&artifact["plan_revision"], &inputs["plan_revision"])?;
    bound(&artifact["plan_hash"], &inputs["plan_hash"])?;
    bound(&artifact["list"]["body_sha256"], &inputs["list_sha256"])?;
    bound(&artifact["source"]["fingerprint"], &inputs["source_fingerprint"])?;

    let job = &inputs["job"];
    let id = field(job, "id")?;
    let rows = job["rows"]
        .as_array()
        .filter(|rows| !rows.is_empty())
        .context("the Term-card job names no row")?;
    let mut verified = Vec::with_capacity(rows.len());
    for row in rows {
        let number = row.as_u64().context("invalid Term-card row number")?;
        let term = index
            .get(&(id.to_string(), number))
            .with_context(|| format!("the readiness result holds no row {number} of job `{id}`"))?;
        if term["status"] != "ready" {
            bail!(
                "row {number} of the Term-card job `{id}` is `{}`, not ready",
                term["status"].as_str().unwrap_or("unknown")
            );
        }
        verified.push(json!({
            "row": number,
            "term": term["term"],
            "draft_sha256": term["draft_sha256"],
            "render_sha256": term["render_sha256"],
        }));
    }
    let result = json!({
        "schema_version": 1,
        "kind": "termcards_completion",
        "unit": unit,
        "token": token,
        "job": id,
        "rows": verified,
        "readiness": {"path": path_to_string(path), "snapshot": path_to_string(snapshot_copy), "sha256": snapshot_sha},
    });
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let claimed = claim(&tx, unit, token)?;
    let current_sha = canon::sha256_hex(&std::fs::read(canon_path)?);
    if current_sha != snapshot_sha {
        bail!("readiness evidence changed during batch completion; rerun -Step check");
    }
    output(
        &tx,
        unit,
        "completion",
        "file",
        &path_to_string(snapshot_copy),
        snapshot_sha,
        &result,
    )?;
    completion_fault(p, "after_outputs")?;
    tx.execute(
        "UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",
        params![unit, canon::canonical_json(&result), canon::utc_now_o()],
    )?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":"verified","kind":result["kind"]}),
    )?;
    completion_fault(p, "before_commit")?;
    tx.commit()?;
    completion_fault(p, "after_commit")?;
    let mut line = result;
    line["state"] = json!("done");
    Ok(line)
}

#[allow(clippy::too_many_arguments)]
fn citedcards_job_finish_indexed(
    p: &Prepared,
    unit: &str,
    token: i64,
    path: &Path,
    canon_path: &Path,
    snapshot_copy: &Path,
    artifact: &Value,
    snapshot_sha: &str,
    index: &HashMap<(String, u64), Value>,
) -> Result<Value> {
    let inputs = unit_inputs(p, unit)?;
    let kind = field(&inputs, "kind")?;
    if kind != "cited_cards_job" {
        bail!("the Cited Card unit `{unit}` does not carry a job plan input");
    }
    finish_claim_checked(p, unit, token)?;
    let _guard = unit_guard(p, unit)?;
    let (_claimed, _progress, _run, done) = finish_claim_checked(p, unit, token)?;
    if done {
        return Ok(json!({"schema_version":1,"unit":unit,"state":"done","reused":true}));
    }
    if artifact["plan_hash"] != inputs["plan_hash"]
        || artifact["plan_revision"] != inputs["plan_revision"]
    {
        bail!("the Cited Card evidence is bound to another plan; rerun -Step plan");
    }
    let id = field(&inputs["job"], "id")?;
    let rows = inputs["job"]["rows"].as_array().cloned().unwrap_or_default();
    for n in &rows {
        let number = n.as_u64().context("invalid Cited Card row number")?;
        let status = index
            .get(&(id.to_string(), number))
            .and_then(|s| s["status"].as_str())
            .unwrap_or("missing");
        if status != "ready" {
            bail!("row {n} of the Cited Card job `{id}` is `{status}`, not ready");
        }
    }
    let result = json!({
        "schema_version": 1,
        "kind": "citedcards_completion",
        "unit": unit,
        "token": token,
        "rows": rows,
        "evidence": {"path": path_to_string(path), "snapshot": path_to_string(snapshot_copy), "sha256": snapshot_sha},
    });
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let claimed = claim(&tx, unit, token)?;
    let current_sha = canon::sha256_hex(&std::fs::read(canon_path)?);
    if current_sha != snapshot_sha {
        bail!("readiness evidence changed during batch completion; rerun -Step check");
    }
    output(
        &tx,
        unit,
        "completion",
        "file",
        &path_to_string(snapshot_copy),
        snapshot_sha,
        &result,
    )?;
    tx.execute(
        "UPDATE units SET state='done',result_json=?2,completed_at=?3,lease_until=NULL,retry_after=NULL,fault_class=NULL,fault_json=NULL WHERE id=?1",
        params![unit, canon::canonical_json(&result), canon::utc_now_o()],
    )?;
    event(
        &tx,
        unit,
        Some(&claimed.owner),
        Some(token),
        "completed",
        &json!({"disposition":"verified","kind":result["kind"]}),
    )?;
    tx.commit()?;
    let mut line = result;
    line["state"] = json!("done");
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_group_targets_require_canonical_unique_ids() {
        let ok = parse_layer_group_targets(
            "digest",
            vec![("digest/g001@1".into(), 2), ("digest/g012@1".into(), 3)],
        )
        .unwrap();
        assert_eq!(ok, vec![("digest/g001@1".into(), 1, 2), ("digest/g012@1".into(), 12, 3)]);
        assert!(parse_layer_group_targets("digest", vec![("digest/g1@1".into(), 1)]).is_err());
        assert!(parse_layer_group_targets("digest", vec![("digest/g0001@1".into(), 1)]).is_err());
        assert!(parse_layer_group_targets("digest", vec![("link/g001@1".into(), 1)]).is_err());
        assert!(
            parse_layer_group_targets(
                "digest",
                vec![("digest/g001@1".into(), 1), ("digest/g001@1".into(), 2)]
            )
            .unwrap_err()
            .to_string()
            .contains("twice")
        );
        assert!(parse_layer_group_targets("link", vec![("link/g002@1".into(), 0)]).is_err());
    }

    #[test]
    fn done_batch_targets_require_single_job_family() {
        let (fam, list) = validate_done_batch_targets(vec![
            ("termcards/t001@2".into(), 3),
            ("termcards/t013@2".into(), 4),
        ])
        .unwrap();
        assert_eq!(fam, DoneBatchFamily::TermCardsJob);
        assert_eq!(list.len(), 2);

        let (fam2, _) = validate_done_batch_targets(vec![("citedcards/t001@1".into(), 1)]).unwrap();
        assert_eq!(fam2, DoneBatchFamily::CitedCardsJob);

        assert!(validate_done_batch_targets(vec![("termcards/book@2".into(), 1)]).is_err());
        assert!(validate_done_batch_targets(vec![("citedcards/book@1".into(), 1)]).is_err());
        assert!(
            validate_done_batch_targets(vec![
                ("termcards/t001@2".into(), 1),
                ("citedcards/t001@1".into(), 1),
            ])
            .is_err()
        );
        assert!(validate_done_batch_targets(vec![("digest/g001@1".into(), 1)]).is_err());
    }

    #[test]
    fn units_and_ids_files_parse_standard_shapes() {
        let dir = tempfile::tempdir().unwrap();
        let ufile = dir.path().join("units.json");
        std::fs::write(
            &ufile,
            r#"[{"id":"termcards/t001@1","token":5,"extra":true},{"unit":"termcards/t013@1","token":6}]"#,
        )
        .unwrap();
        let parsed = parse_units_file(&ufile.to_string_lossy()).unwrap();
        assert_eq!(
            parsed,
            vec![("termcards/t001@1".into(), 5), ("termcards/t013@1".into(), 6)]
        );

        let ifile = dir.path().join("ids.json");
        std::fs::write(&ifile, r#"["termcards/t001@1", {"id":"termcards/t013@1"}]"#).unwrap();
        let ids = parse_ids_file(&ifile.to_string_lossy()).unwrap();
        assert!(ids.contains("termcards/t001@1"));
        assert!(ids.contains("termcards/t013@1"));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn transport_and_archive_identity_fault_classifiers_cover_closed_client() {
        assert!(is_transport_fault(&anyhow::anyhow!("zt mcp is closed")));
        assert!(is_transport_fault(&anyhow::anyhow!("transport_closed: pipe broke")));
        assert!(!is_transport_fault(&anyhow::anyhow!("stale_claim: unit changed")));
        assert!(is_archive_identity_fault(&anyhow::anyhow!(
            "archive_identity_mismatch: live archive differs"
        )));
        assert!(is_archive_identity_fault(&anyhow::anyhow!(
            "archive_identity_changed: group-start"
        )));
        assert!(!is_archive_identity_fault(&anyhow::anyhow!("read_view_changed")));
    }

    #[test]
    fn planning_stop_code_preserves_scope_refused_and_card_locked() {
        assert_eq!(
            crate::lockcheck::refusal_code(&json!({"error": "scope_refused"})),
            "scope_refused"
        );
        assert_eq!(
            crate::lockcheck::refusal_code(&json!({"code": "scope_refused"})),
            "scope_refused"
        );
        assert_eq!(
            crate::lockcheck::refusal_code(&json!({"error": "card_locked"})),
            "card_locked"
        );
    }

    #[test]
    fn finish_batch_rejects_noncanonical_group_aliases() {
        assert!(finish_batch::validate_batch_targets(vec![("cards/g1@1".into(), 1)]).is_err());
        assert!(finish_batch::validate_batch_targets(vec![("events/g01@1".into(), 1)]).is_err());
        assert!(finish_batch::validate_batch_targets(vec![("findings/g1@1".into(), 1)]).is_err());
        assert!(finish_batch::validate_batch_targets(vec![("cards/g001@1".into(), 1)]).is_ok());
        assert!(finish_batch::validate_batch_targets(vec![("events/g001@1".into(), 1)]).is_ok());
        assert!(finish_batch::validate_batch_targets(vec![("findings/g001@1".into(), 1)]).is_ok());
    }
}
