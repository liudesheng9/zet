//! `finish-batch`: the checked finishes of a layer's claimed units in one process (P1/P4 of
//! docs/digest-claude-token-proposal.md). Each unit keeps its own verifier (`cards-finish`,
//! `events-finish`, `findings-finish`, `model-finish`, `explain-finish`), its own transaction and its own
//! claim token; one unit's failure is recorded against that unit alone (`fail`, the same classes the
//! workflow's `finishOrFault` gave it), and the batch goes on with the next unit. The full results go to a
//! file under `<out>/finish/`; the printed line carries counts and one short entry per unit, so the relay
//! that reads it never retypes kilobytes of completion evidence.
//!
//! With `-Layer (-Take <n> | -All) -Owner` the command also claims the units it finishes (the Cards
//! layer, whose work is the finish alone), draining until the queue empties or the command-entry
//! admission budget expires.
//!
//! `model-deps -Units|-UnitsFile` is the batch form of `model-deps`: every planned model's
//! references in one process with a single `Zt` client.
use super::*;
use std::time::Instant;

/// Default bounded `-Take` when `-All` is not specified.
const DEFAULT_TAKE: usize = 8;
/// Internal claim chunk size for `-Layer <l> -All` so claim-and-finish does not hold huge
/// unstarted claims if the admission budget fires mid-drain.
const DRAIN_CHUNK: i64 = 16;
/// A finish takes about a second (ebrill2001modern trial 1: 1.0-1.9 s each); no unit starts after this
/// many seconds from command entry, so the command ends well inside the workflow's 600-second command
/// timeout. Any unstarted unit stays claimed, is reported `not_run`, and is persisted in `remaining`.
pub(super) const BUDGET_SECONDS: u64 = 420;
const LAYERS: &[&str] = &["cards", "events", "findings", "models", "explain"];

/// Effective admission budget in seconds. Isolated scratch tests under the temp directory may
/// shorten the budget via `ZT_FINISH_BATCH_BUDGET_SECONDS` to exercise mid-chunk and pre-claim
/// budget expiration deterministically.
pub(super) fn budget_seconds(p: &Prepared) -> u64 {
    if let Ok(raw) = std::env::var("ZT_FINISH_BATCH_BUDGET_SECONDS")
        && let Ok(secs) = raw.trim().parse::<u64>()
        && let Ok(tmp) = std::fs::canonicalize(std::env::temp_dir())
        && p.out.starts_with(&tmp)
    {
        return secs;
    }
    BUDGET_SECONDS
}

fn short(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// Validates `<unit>:<token>` pairs for `finish-batch`: nonempty, positive tokens, supported
/// batch layers only, strict canonical group IDs for `cards`/`events`/`findings`, and each unit
/// named once. No arbitrary eight-unit count cap.
pub(super) fn validate_batch_targets(targets: Vec<(String, i64)>) -> Result<Vec<(String, i64)>> {
    let targets = batch::validate_unique_targets(targets)?;
    let mut seen_groups: BTreeSet<(String, u64)> = BTreeSet::new();
    for (unit, _) in &targets {
        let layer = unit.split('/').next().unwrap_or_default();
        if !LAYERS.contains(&layer) {
            bail!(
                "-Units names {unit}; finish-batch finishes {} units only",
                LAYERS.join(", ")
            );
        }
        if matches!(layer, "cards" | "events" | "findings") {
            let n = group_of(unit)?;
            let canonical = format!("{layer}/g{n:03}@1");
            if unit != &canonical {
                bail!("{unit} is not canonical; expected {canonical}");
            }
            if !seen_groups.insert((layer.to_string(), n)) {
                bail!("target list names {canonical} twice");
            }
        }
    }
    Ok(targets)
}

/// `<unit>:<token>` pairs, comma separated, each a supported layer and named once.
pub(super) fn batch_targets(text: &str) -> Result<Vec<(String, i64)>> {
    validate_batch_targets(hold_targets(text)?)
}

/// The group number of a `<layer>/gNNN@1` unit (strict canonical form).
fn group_of(unit: &str) -> Result<u64> {
    let (layer, rest) = unit
        .split_once("/g")
        .with_context(|| format!("{unit} names no group"))?;
    let digits = rest
        .strip_suffix("@1")
        .with_context(|| format!("{unit} names no group"))?;
    let n = digits
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .with_context(|| format!("{unit} names no group"))?;
    let canonical = format!("{layer}/g{n:03}@1");
    if unit != canonical {
        bail!("{unit} is not canonical; expected {canonical}");
    }
    Ok(n)
}

/// The unit's own checked finish, the same call its single command makes.
fn finish_one(p: &Prepared, zt: &mut Zt, unit: &str, token: i64) -> Result<Value> {
    match unit.split('/').next().unwrap_or_default() {
        "cards" => cards_finish(p, zt, group_of(unit)?, token, false),
        "events" => empirical_finish(p, zt, &EVENTS, group_of(unit)?, token, false),
        "findings" => empirical_finish(p, zt, &FINDINGS, group_of(unit)?, token, false),
        "models" => model_finish(p, zt, unit, token, false),
        "explain" => explain_finish(p, zt, unit, token, false),
        other => bail!("no checked finish for layer {other}"),
    }
}

/// What the ledger holds for one unit now: state, token, claim input hash and disposition.
pub(super) fn held_state(p: &Prepared, unit: &str) -> Result<Option<(String, i64, String, Value)>> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    conn.query_row(
        "SELECT state,claim_token,input_hash,result_json FROM units WHERE id=?1",
        [unit],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        },
    )
    .optional()?
    .map(|(state, token, hash, result)| {
        let disposition = result
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .map(|v| v["disposition"].clone())
            .unwrap_or(Value::Null);
        Ok((state, token, hash, disposition))
    })
    .transpose()
}

/// One unit's outcome: `(entry for the line, full record for the file, refusal if any, transport dead, archive mismatch)`.
fn settle_one(
    p: &Prepared,
    zt: &mut Zt,
    unit: &str,
    token: i64,
) -> (Value, Value, Option<Value>, bool, bool) {
    let finished = finish_one(p, zt, unit, token);
    let transport_dead = finished.as_ref().err().is_some_and(batch::is_transport_fault);
    let archive_mismatch = finished.as_ref().err().is_some_and(batch::is_archive_identity_fault);
    let cause = match &finished {
        Ok(v) if v["state"] == json!("done") || v["complete"] == json!(true) => {
            let status = if v["status"] == json!("partial") {
                "partial"
            } else {
                "done"
            };
            return (
                json!({"unit": unit, "token": token, "status": status, "disposition": v["disposition"]}),
                json!({"unit": unit, "token": token, "status": status, "result": v}),
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
    // R326/R337: a typed planning stop (`card_locked` or `scope_refused`) is a content fault of
    // this unit, and the batch stops with its exact native code.
    if let Err(error) = &finished
        && let Some(refusal) = crate::lockcheck::stopped_refusal(error)
    {
        let code = crate::lockcheck::refusal_code(&refusal);
        let fault = json!({"schema_version": 1, "unit": unit, "input_hash": "", "code": code, "cause": short(&format!("{cause} {}", util::compact(&refusal)), 800)});
        let recorded = fail(p, unit, token, "content", &fault).is_ok();
        return (
            json!({"unit": unit, "token": token, "status": "failed", "code": code, "cause": short(&cause, 200), "recorded": recorded}),
            json!({"unit": unit, "token": token, "status": "failed", "fault": fault, "recorded": recorded}),
            Some(refusal),
            false,
            false,
        );
    }
    if archive_mismatch {
        return (
            json!({"unit": unit, "token": token, "status": "not_run", "code": "archive_identity_mismatch", "cause": short(&cause, 200), "recorded": false}),
            json!({"unit": unit, "token": token, "status": "not_run", "code": "archive_identity_mismatch", "cause": cause, "recorded": false}),
            None,
            transport_dead,
            true,
        );
    }
    // The ledger decides every finish that is not plainly done before any fault: only stale-token
    // replays or post-commit hook failures treat a done row as clean; integrity/verification errors
    // or transport death during re-verification surface instead of masquerading as done.
    match held_state(p, unit) {
        Ok(Some((state, _, _, disposition))) if state == "done" => {
            let stale_or_post_commit = !transport_dead
                && (cause.contains("stale_claim")
                    || cause.contains("injected completion fault: after_commit"));
            if stale_or_post_commit {
                return (
                    json!({"unit": unit, "token": token, "status": "done", "disposition": disposition, "from": "ledger"}),
                    json!({"unit": unit, "token": token, "status": "done", "from": "ledger", "result": result, "cause": cause}),
                    None,
                    false,
                    false,
                );
            }
            let code = super::cause_code(&cause);
            return (
                json!({"unit": unit, "token": token, "status": "failed", "ledger_state": "done", "verified": false, "disposition": disposition, "code": code, "cause": short(&cause, 200), "recorded": false}),
                json!({"unit": unit, "token": token, "status": "failed", "ledger_state": "done", "verified": false, "disposition": disposition, "code": code, "result": result, "cause": cause, "recorded": false}),
                None,
                transport_dead,
                false,
            );
        }
        // A unit this claim no longer holds was settled or reclaimed elsewhere: its fault, if any, is
        // already recorded, so nothing is recorded twice.
        Ok(Some((state, now, _, _))) if state != "claimed" || now != token => {
            let status = if state == "failed" { "failed" } else { "retry" };
            return (
                json!({"unit": unit, "token": token, "status": status, "cause": short(&format!("the ledger holds {unit} {state}"), 200), "recorded": false}),
                json!({"unit": unit, "token": token, "status": status, "ledger_state": state, "result": result, "cause": cause}),
                None,
                transport_dead,
                false,
            );
        }
        _ => {}
    }
    // S5: the ledger's one reading of the cause (ledger.rs cause_code, fault_class).
    let code = super::cause_code(&cause);
    let class = super::fault_class(code).unwrap_or("content");
    let fault = json!({"schema_version": 1, "unit": unit, "input_hash": "", "code": code, "cause": cause});
    // Use the ledger's actual post-`fail` state (respecting attempt budget exhaustion).
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
        json!({"unit": unit, "token": token, "status": status, "code": code, "cause": short(&cause, 200), "recorded": recorded}),
        json!({"unit": unit, "token": token, "status": status, "fault": fault, "recorded": recorded, "refused": refused, "result": result}),
        None,
        transport_dead,
        false,
    )
}

/// One named ready unit claimed for a checked finish (§3.7: the Explain unit a Model completion just
/// registered). `None` when the unit is claimed elsewhere, done, or still waiting on a dependency.
fn claim_named(p: &Prepared, unit: &str, owner: &str) -> Result<Option<i64>> {
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let now = now_seconds();
    reconcile(&tx, now)?;
    let row: Option<(String, i64, String)> = tx
        .query_row(
            "SELECT state,claim_token,input_hash FROM units WHERE id=?1",
            [unit],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((state, old_token, input_hash)) = row else {
        bail!("unknown unit {unit}");
    };
    if state != "ready" {
        tx.commit()?;
        return Ok(None);
    }
    let token = old_token.checked_add(1).context("claim token exhausted")?;
    let lease_until = now + claim_lease(lease_seconds(), 0);
    tx.execute(
        "UPDATE units SET state='claimed',owner=?2,claim_token=?3,lease_until=?4,retry_after=NULL,attempts=attempts+1 WHERE id=?1",
        params![unit, owner, token, lease_until],
    )?;
    event(
        &tx,
        unit,
        Some(owner),
        Some(token),
        "claimed",
        &json!({"input_hash":input_hash,"lease_until":lease_until,"follow":"model-finish","process":{"pid":std::process::id()}}),
    )?;
    tx.commit()?;
    Ok(Some(token))
}

/// The unit's state after a dependency reconciliation, without claiming it.
fn ready_state(p: &Prepared, unit: &str) -> Result<Option<String>> {
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    reconcile(&tx, now_seconds())?;
    let state: Option<String> = tx
        .query_row("SELECT state FROM units WHERE id=?1", [unit], |r| r.get(0))
        .optional()?;
    tx.commit()?;
    Ok(state)
}

/// A claim released for the Explain stage: the unit's own work is still open, and this is not its fault.
fn release_named(p: &Prepared, unit: &str, token: i64) -> Result<()> {
    let mut conn = writer(p)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    verify_binding(&tx, p)?;
    let changed = tx.execute(
        "UPDATE units SET state='ready',owner=NULL,claim_token=?2,lease_until=NULL,retry_after=NULL WHERE id=?1 AND state='claimed' AND claim_token=?3",
        params![
            unit,
            token.checked_add(1).context("claim token exhausted")?,
            token
        ],
    )?;
    if changed > 0 {
        event(
            &tx,
            unit,
            None,
            None,
            "released",
            &json!({"reason":"the Model's explanations are not complete","claim_token":token}),
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// §3.7: `finish-batch` records the Explain completion of a Model whose explanations the batch wrote,
/// right after the Model's own completion, so the Explain stage starts no job for it. A Model whose
/// explanations are still open leaves its unit ready for that stage; a typed Lock/scope refusal from
/// the nested Explain check IS propagated so the batch stops immediately with exit 2.
fn finish_explained(
    p: &Prepared,
    zt: &mut Zt,
    completion: &Value,
) -> Result<(Option<Value>, Option<Value>, bool, bool)> {
    let unit = completion["explain_unit"].as_str().unwrap_or_default();
    if unit.is_empty() {
        return Ok((None, None, false, false));
    }
    let needs = completion["explanation"]["needs"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    if needs > 0 {
        let state = ready_state(p, unit)?.unwrap_or_else(|| "unknown".into());
        return Ok((Some(json!({"unit": unit, "status": state, "open": needs})), None, false, false));
    }
    let Some(token) = claim_named(p, unit, "finish-batch")? else {
        let state = ready_state(p, unit)?.unwrap_or_else(|| "unknown".into());
        return Ok((Some(json!({"unit": unit, "status": state})), None, false, false));
    };
    let finished = explain_finish(p, zt, unit, token, false);
    if let Err(error) = &finished
        && let Some(refusal) = crate::lockcheck::stopped_refusal(error)
    {
        let code = crate::lockcheck::refusal_code(&refusal);
        let cause = short(&format!("{error:#}"), 800);
        let fault = json!({
            "schema_version": 1,
            "unit": unit,
            "input_hash": "",
            "code": code,
            "cause": short(&format!("{cause} {}", util::compact(&refusal)), 800),
        });
        let recorded = fail(p, unit, token, "content", &fault).is_ok();
        return Ok((
            Some(json!({
                "unit": unit, "token": token, "status": "failed",
                "code": code, "cause": short(&cause, 200), "recorded": recorded,
            })),
            Some(refusal),
            false,
            false,
        ));
    }
    let transport_dead = finished.as_ref().err().is_some_and(batch::is_transport_fault);
    let archive_mismatch = finished.as_ref().err().is_some_and(batch::is_archive_identity_fault);
    let (status, cause) = match &finished {
        Ok(v) if v["state"] == json!("done") || v["complete"] == json!(true) => ("done", None),
        Ok(v) => (
            "ready",
            Some(
                v["fault"]
                    .as_str()
                    .or_else(|| v["error"].as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        "the checked explanation completion did not complete".into()
                    }),
            ),
        ),
        Err(error) => ("ready", Some(format!("{error:#}"))),
    };
    if status != "done" {
        release_named(p, unit, token)?;
    }
    let mut entry = json!({"unit": unit, "token": token, "status": status});
    if let Some(cause) = cause {
        entry["cause"] = json!(short(&cause, 200));
    }
    Ok((Some(entry), None, transport_dead, archive_mismatch))
}

/// Finishes the listed claims in turn using the caller's single `Zt` client; returns the printed
/// line and the exit code.
pub(super) fn finish_batch(
    p: &Prepared,
    zt: &mut Zt,
    targets: &[(String, i64)],
    claim_meta: Option<&Value>,
    started: Instant,
) -> Result<(Value, i32)> {
    let budget = budget_seconds(p);
    let mut entries = Vec::with_capacity(targets.len());
    let mut records = Vec::with_capacity(targets.len());
    let mut explains = Vec::new();
    let mut remaining = Vec::new();
    let mut lock: Option<Value> = None;
    let mut stop_reason: Option<&'static str> = None;

    for (idx, (unit, token)) in targets.iter().enumerate() {
        if lock.is_some() || stop_reason.is_some() || started.elapsed().as_secs() >= budget {
            if stop_reason.is_none() && lock.is_none() {
                stop_reason = Some("budget");
            }
            entries.push(json!({"unit": unit, "token": token, "status": "not_run"}));
            records.push(json!({"unit": unit, "token": token, "status": "not_run"}));
            remaining.push(json!({"id": unit, "token": token}));
            continue;
        }
        if idx > 0
            && let Err(error) = verify_live_archive_identity(p, zt)
        {
            let msg = format!("{error:#}");
            stop_reason = Some(if batch::is_transport_fault(&error) {
                "transport"
            } else {
                "archive_identity"
            });
            entries.push(json!({"unit": unit, "token": token, "status": "not_run", "cause": short(&msg, 200)}));
            records.push(json!({"unit": unit, "token": token, "status": "not_run", "cause": short(&msg, 800)}));
            remaining.push(json!({"id": unit, "token": token}));
            continue;
        }
        let (entry, record, refusal, transport_dead, archive_mismatch) =
            settle_one(p, zt, unit, *token);
        let mut explain_lock: Option<Value> = None;
        let mut explain_transport_dead = false;
        let mut explain_archive_mismatch = false;
        if refusal.is_none()
            && !transport_dead
            && !archive_mismatch
            && unit.starts_with("models/")
            && matches!(entry["status"].as_str(), Some("done" | "partial"))
        {
            match finish_explained(p, zt, &record["result"]) {
                Ok((Some(explain), ex_refusal, ex_dead, ex_arch)) => {
                    explains.push(explain);
                    explain_lock = ex_refusal;
                    explain_transport_dead = ex_dead;
                    explain_archive_mismatch = ex_arch;
                }
                Ok((None, ex_refusal, ex_dead, ex_arch)) => {
                    explain_lock = ex_refusal;
                    explain_transport_dead = ex_dead;
                    explain_archive_mismatch = ex_arch;
                }
                Err(error) => {
                    if batch::is_transport_fault(&error) {
                        explain_transport_dead = true;
                    } else if batch::is_archive_identity_fault(&error) {
                        explain_archive_mismatch = true;
                    }
                    explains.push(json!({
                        "unit": Value::Null,
                        "status": "error",
                        "cause": short(&format!("{error:#}"), 200),
                    }));
                }
            }
        }
        if archive_mismatch {
            remaining.push(json!({"id": unit, "token": token}));
        }
        entries.push(entry);
        records.push(record);
        if let Some(r) = refusal.or(explain_lock) {
            let code = crate::lockcheck::refusal_code(&r);
            lock = Some(r);
            stop_reason = Some(code);
        } else if archive_mismatch || explain_archive_mismatch {
            stop_reason = Some("archive_identity");
        } else if transport_dead || explain_transport_dead {
            stop_reason = Some("transport");
        }
    }

    finalize_batch_reply(
        p,
        targets,
        entries,
        records,
        explains,
        remaining,
        claim_meta,
        lock,
        stop_reason,
    )
}

#[allow(clippy::too_many_arguments)]
fn finalize_batch_reply(
    p: &Prepared,
    targets: &[(String, i64)],
    entries: Vec<Value>,
    records: Vec<Value>,
    explains: Vec<Value>,
    remaining: Vec<Value>,
    claim_meta: Option<&Value>,
    lock: Option<Value>,
    stop_reason: Option<&'static str>,
) -> Result<(Value, i32)> {
    let count = |status: &str| entries.iter().filter(|e| e["status"] == json!(status)).count();
    let layer = targets
        .first()
        .map(|(unit, _)| unit.split('/').next().unwrap_or_default().to_string())
        .or_else(|| {
            claim_meta
                .and_then(|c| c["layer"].as_str())
                .map(str::to_string)
        })
        .unwrap_or_default();
    let names: Vec<String> = targets
        .iter()
        .map(|(unit, token)| format!("{unit}:{token}"))
        .collect();
    let folder = p.out.join("finish");
    let file = folder.join(format!(
        "{layer}-{}.json",
        &canon::sha256_hex(names.join(",").as_bytes())[..12]
    ));
    let persist_res = std::fs::create_dir_all(&folder).and_then(|_| {
        let payload = serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "kind": "finish_batch",
            "stop_reason": stop_reason,
            "remaining": remaining,
            "units": records,
            "explain": explains,
            "claim": claim_meta,
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
    let explain_count = |status: &str| explains.iter().filter(|e| e["status"] == json!(status)).count();
    let mut line = json!({
        "schema_version": 1,
        "kind": "finish_batch",
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
    // §3.7: the Explain units this batch recorded or left for the Explain stage, beside the units it
    // was asked to finish (they are not among `results`, so a Model block's counts stay Model counts).
    if !explains.is_empty() {
        line["explain"] = json!(explains);
        line["explain_done"] = json!(explain_count("done"));
        line["explain_ready"] = json!(explain_count("ready"));
    }
    if claim_meta.is_some() {
        line["claimed"] = json!(targets.len());
    }
    let mut exit = 0;
    if let Some(refusal) = lock {
        let code = crate::lockcheck::refusal_code(&refusal);
        // The batch line itself carries the typed refusal, so the workflow stops the stage on it.
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

/// Drains a claim-and-finish layer (`-Layer <l> -All -Owner ...`) in bounded internal chunks until
/// no ready units remain, the command-entry admission budget expires, a typed Lock/scope refuses,
/// archive identity changes, or the transport fails. Each unit is admitted at most once per drain.
fn drain_layer(
    p: &Prepared,
    zt: &mut Zt,
    layer: &str,
    owner: &str,
    wait: i64,
    lease: i64,
    started: Instant,
) -> Result<(Value, i32)> {
    let budget = budget_seconds(p);
    let mut all_targets: Vec<(String, i64)> = Vec::new();
    let mut admitted: BTreeSet<String> = BTreeSet::new();
    let mut entries = Vec::new();
    let mut records = Vec::new();
    let mut explains = Vec::new();
    let mut remaining = Vec::new();
    let mut lock: Option<Value> = None;
    let mut stop_reason: Option<&'static str> = None;
    let mut first_round = true;

    loop {
        let elapsed = started.elapsed().as_secs();
        if lock.is_some() || stop_reason.is_some() || elapsed >= budget {
            if stop_reason.is_none() && lock.is_none() {
                stop_reason = Some("budget");
            }
            break;
        }
        let rem_budget = (budget - elapsed) as i64;
        let round_wait = if first_round { wait.min(rem_budget) } else { 0 };
        first_round = false;
        let exclude = (!admitted.is_empty()).then_some(&admitted);
        let claim = next_filtered(
            p,
            layer,
            Some(DRAIN_CHUNK),
            owner,
            round_wait,
            lease,
            None,
            None,
            exclude,
        )?;
        let targets: Vec<(String, i64)> = claim["units"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|u| Some((u["id"].as_str()?.to_string(), u["token"].as_i64()?)))
            .collect();
        if targets.is_empty() {
            break;
        }
        for (unit, token) in targets {
            let idx = all_targets.len();
            admitted.insert(unit.clone());
            all_targets.push((unit.clone(), token));
            if lock.is_some()
                || stop_reason.is_some()
                || started.elapsed().as_secs() >= budget
            {
                if stop_reason.is_none() && lock.is_none() {
                    stop_reason = Some("budget");
                }
                entries.push(json!({"unit": unit, "token": token, "status": "not_run"}));
                records.push(json!({"unit": unit, "token": token, "status": "not_run"}));
                remaining.push(json!({"id": unit, "token": token}));
                continue;
            }
            if idx > 0
                && let Err(error) = verify_live_archive_identity(p, zt)
            {
                let msg = format!("{error:#}");
                stop_reason = Some(if batch::is_transport_fault(&error) {
                    "transport"
                } else {
                    "archive_identity"
                });
                entries.push(json!({"unit": unit, "token": token, "status": "not_run", "cause": short(&msg, 200)}));
                records.push(json!({"unit": unit, "token": token, "status": "not_run", "cause": short(&msg, 800)}));
                remaining.push(json!({"id": unit, "token": token}));
                continue;
            }
            let (entry, record, refusal, transport_dead, archive_mismatch) =
                settle_one(p, zt, &unit, token);
            let mut explain_lock: Option<Value> = None;
            let mut explain_transport_dead = false;
            let mut explain_archive_mismatch = false;
            if refusal.is_none()
                && !transport_dead
                && !archive_mismatch
                && unit.starts_with("models/")
                && matches!(entry["status"].as_str(), Some("done" | "partial"))
            {
                match finish_explained(p, zt, &record["result"]) {
                    Ok((Some(explain), ex_refusal, ex_dead, ex_arch)) => {
                        explains.push(explain);
                        explain_lock = ex_refusal;
                        explain_transport_dead = ex_dead;
                        explain_archive_mismatch = ex_arch;
                    }
                    Ok((None, ex_refusal, ex_dead, ex_arch)) => {
                        explain_lock = ex_refusal;
                        explain_transport_dead = ex_dead;
                        explain_archive_mismatch = ex_arch;
                    }
                    Err(error) => {
                        if batch::is_transport_fault(&error) {
                            explain_transport_dead = true;
                        } else if batch::is_archive_identity_fault(&error) {
                            explain_archive_mismatch = true;
                        }
                        explains.push(json!({
                            "unit": Value::Null,
                            "status": "error",
                            "cause": short(&format!("{error:#}"), 200),
                        }));
                    }
                }
            }
            if archive_mismatch {
                remaining.push(json!({"id": unit, "token": token}));
            }
            entries.push(entry);
            records.push(record);
            if let Some(r) = refusal.or(explain_lock) {
                let code = crate::lockcheck::refusal_code(&r);
                lock = Some(r);
                stop_reason = Some(code);
            } else if archive_mismatch || explain_archive_mismatch {
                stop_reason = Some("archive_identity");
            } else if transport_dead || explain_transport_dead {
                stop_reason = Some("transport");
            }
        }
    }

    let post_status = status(&p.out, layer, None)?;
    if all_targets.is_empty() {
        let mut line = json!({
            "schema_version": 1,
            "kind": "finish_batch",
            "units": 0,
            "claimed": 0,
            "done": 0,
            "partial": 0,
            "retry": 0,
            "failed": 0,
            "not_run": 0,
            "results": [],
        });
        if let Some(reason) = stop_reason {
            line["stop_reason"] = json!(reason);
        }
        for key in ["layer", "settled", "complete", "counts"] {
            line[key] = post_status[key].clone();
        }
        return Ok((line, 0));
    }

    let brief = json!({
        "layer": layer,
        "owner": owner,
        "all": true,
        "units": all_targets.iter().map(|(u, t)| json!({"id": u, "token": t})).collect::<Vec<_>>(),
    });
    let (mut line, exit) = finalize_batch_reply(
        p,
        &all_targets,
        entries,
        records,
        explains,
        remaining,
        Some(&brief),
        lock,
        stop_reason,
    )?;
    for key in ["layer", "settled", "complete", "counts"] {
        line[key] = post_status[key].clone();
    }
    Ok((line, exit))
}

/// `finish-batch -Out <out> (-Units "<unit>:<token>,…" | -UnitsFile <path> | -Layer <l> (-Take <n> | -All) -Owner <label> [-Wait s] [-Lease s])`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let started = Instant::now();
    enum Mode {
        Units(Vec<(String, i64)>),
        Claim {
            layer: String,
            take: Option<i64>,
            owner: String,
            wait: i64,
            lease: i64,
        },
    }
    let parsed = (|| -> Result<(String, Mode)> {
        let a = Args::parse(
            argv,
            &["Out", "Units", "UnitsFile", "Layer", "Take", "Owner", "Wait", "Lease"],
            &["All"],
        )?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        if a.has("Units") && a.has("UnitsFile") {
            bail!("-Units and -UnitsFile are mutually exclusive");
        }
        if a.has("Units") || a.has("UnitsFile") {
            if a.has("Layer")
                || a.has("Take")
                || a.flag("All")
                || a.has("Owner")
                || a.has("Wait")
                || a.has("Lease")
            {
                bail!(
                    "-Units/-UnitsFile finishes the named claims; it takes no -Layer, -Take, -All, -Owner, -Wait or -Lease"
                );
            }
            let targets = if a.has("Units") {
                batch_targets(&a.str("Units"))?
            } else {
                validate_batch_targets(batch::parse_units_file(&a.required("UnitsFile")?)?)?
            };
            return Ok((a.str("Out"), Mode::Units(targets)));
        }
        let layer = a.required("Layer")?;
        if !LAYERS.contains(&layer.as_str()) {
            bail!("-Layer must be one of {}", LAYERS.join(", "));
        }
        if a.flag("All") && a.has("Take") {
            bail!("-All and -Take are mutually exclusive");
        }
        let number = |name: &str, default: i64, low: i64, high: i64| -> Result<i64> {
            if !a.has(name) {
                return Ok(default);
            }
            a.str(name)
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|n| (low..=high).contains(n))
                .with_context(|| {
                    format!("-{name} must be a whole number from {low} through {high}")
                })
        };
        let take = if a.flag("All") {
            None
        } else {
            Some(number("Take", DEFAULT_TAKE as i64, 1, 64)?)
        };
        let wait = number("Wait", 0, 0, 600)?;
        let lease = number("Lease", 0, 0, LEASE_MAX)?;
        let owner = a.required("Owner")?;
        if owner.trim().is_empty() || owner.len() > 256 {
            bail!("-Owner must be nonempty and at most 256 bytes");
        }
        Ok((
            a.str("Out"),
            Mode::Claim {
                layer,
                take,
                owner,
                wait,
                lease,
            },
        ))
    })();
    let (out, mode) = match parsed {
        Ok(value) => value,
        Err(error) => {
            crate::util::emit_err(&format!("zt-digest finish-batch: {error}"));
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    let (line, exit) = match mode {
        Mode::Units(targets) => finish_batch(&p, &mut zt, &targets, None, started)?,
        Mode::Claim {
            layer,
            take: None,
            owner,
            wait,
            lease,
        } => drain_layer(&p, &mut zt, &layer, &owner, wait, lease, started)?,
        Mode::Claim {
            layer,
            take: Some(take),
            owner,
            wait,
            lease,
        } => {
            let claim = next(&p, &layer, take, &owner, wait, lease)?;
            let targets: Vec<(String, i64)> = claim["units"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|u| Some((u["id"].as_str()?.to_string(), u["token"].as_i64()?)))
                .collect();
            if targets.is_empty() {
                let mut line = json!({"schema_version": 1, "kind": "finish_batch", "units": 0, "claimed": 0, "done": 0, "partial": 0, "retry": 0, "failed": 0, "not_run": 0, "results": []});
                for key in ["layer", "settled", "complete", "counts"] {
                    line[key] = claim[key].clone();
                }
                (line, 0)
            } else {
                let brief = json!({"layer": claim["layer"], "owner": owner, "units": targets.iter().map(|(u, t)| json!({"id": u, "token": t})).collect::<Vec<_>>()});
                let (mut line, exit) =
                    finish_batch(&p, &mut zt, &targets, Some(&brief), started)?;
                let post_status = status(&p.out, &layer, None)?;
                for key in ["layer", "settled", "complete", "counts"] {
                    line[key] = post_status[key].clone();
                }
                (line, exit)
            }
        }
    };
    util::print_line(&serde_json::to_string(&line)?)?;
    Ok(exit)
}

/// `model-deps -Out <out> (-Units "<unit>:<token>,…" | -UnitsFile <path>)`: every listed Model
/// unit's references in one process using the caller's single `Zt` client, one short entry per
/// unit (ok, waiting, state, or the error).
pub fn model_deps_batch(
    p: &Prepared,
    zt: &mut Zt,
    targets: &[(String, i64)],
) -> Result<Value> {
    let mut results = Vec::with_capacity(targets.len());
    for (unit, token) in targets {
        results.push(match model_deps(p, zt, unit, *token) {
            Ok(v) => json!({
                "unit": unit,
                "ok": v["ok"],
                "waiting": v["waiting"],
                "state": v["state"],
                "in_batch": v["in_batch"].as_array().map(Vec::len),
            }),
            Err(error) => json!({"unit": unit, "error": short(&format!("{error:#}"), 300)}),
        });
    }
    Ok(json!({
        "schema_version": 1,
        "kind": "model_deps_batch",
        "units": results.len(),
        "results": results,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_names_supported_units_once_without_eight_unit_cap() {
        let ok = batch_targets("cards/g001@1:2, events/g002@1:1,models/g003-m0001@1:4").unwrap();
        assert_eq!(
            ok,
            vec![
                ("cards/g001@1".into(), 2),
                ("events/g002@1".into(), 1),
                ("models/g003-m0001@1".into(), 4)
            ]
        );
        assert!(batch_targets("cards/g001@1:1,cards/g001@1:2")
            .unwrap_err()
            .to_string()
            .contains("twice"));
        assert!(batch_targets("digest/g001@1:1")
            .unwrap_err()
            .to_string()
            .contains("only"));
        assert!(batch_targets("cards/g001@1:0").is_err(), "a token is positive");
        let nine: Vec<String> = (1..=9).map(|n| format!("cards/g{n:03}@1:1")).collect();
        assert_eq!(batch_targets(&nine.join(",")).unwrap().len(), 9);
    }

    #[test]
    fn a_unit_names_its_group() {
        assert_eq!(group_of("cards/g012@1").unwrap(), 12);
        assert!(group_of("models/g001-m0002@1").is_err());
    }

    #[test]
    // S5: the causes the trial logs recorded, read by the ledger's one classifier, give the outcome the four
    // copies gave (the JavaScript regexes, this file's list); cleanup_transient is retryable everywhere now.
    fn the_trial_causes_keep_their_outcome() {
        let table = [
            ("stale_claim: the claim moved on", "retryable"),
            ("read_view_changed: group completion evidence changed before ledger commit", "retryable"),
            ("provider_cap: run cap reached", "retryable"),
            ("cleanup_transient: delete refused, still open", "retryable"),
            ("lock_busy", "retryable"),
            ("sharing_violation on ledger.sqlite3", "retryable"),
            ("sqlite_busy", "retryable"),
            ("transport_closed: zt mcp ended", "retryable"),
            ("command_timeout after 600 s", "retryable"),
            ("service_unavailable", "retryable"),
            ("missing_result: no answer file", "retryable"),
            ("pending_jobs: 2", "retryable"),
            ("check_failed: R3 a semicolon", "content"),
            ("plan faults: id A is used twice", "content"),
            ("the checked completion did not complete", "content"),
            ("model 5|a has no part DA yet", "content"),
        ];
        for (cause, class) in table {
            assert_eq!(
                super::super::fault_class(super::super::cause_code(cause)).unwrap(),
                class,
                "{cause}"
            );
        }
    }
}
