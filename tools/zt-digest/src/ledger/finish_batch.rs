//! `finish-batch`: the checked finishes of one block's claimed units in one process (P1/P4 of
//! docs/digest-claude-token-proposal.md). Each unit keeps its own verifier (`cards-finish`,
//! `events-finish`, `findings-finish`, `model-finish`, `explain-finish`), its own transaction and its own
//! claim token; one unit's failure is recorded against that unit alone (`fail`, the same classes the
//! workflow's `finishOrFault` gave it), and the batch goes on with the next unit. The full results go to a
//! file under `<out>/finish/`; the printed line carries counts and one short entry per unit, so the relay
//! that reads it never retypes kilobytes of completion evidence.
//!
//! With `-Layer -Take -Owner` the command also claims the units it finishes (the Cards layer, whose work is
//! the finish alone), so one command replaces a claim relay and a finish relay.
//!
//! `model-deps -Units` is the batch form of `model-deps`: every planned model's references in one process.
use super::*;

/// A block holds at most eight units (the DeepSeek Subagent Managed Protocol's eight per manager).
const BATCH_MAX: usize = 8;
/// A finish takes about a second (ebrill2001modern trial 1: 1.0-1.9 s each); no unit starts after this
/// many seconds, so the command ends well inside the workflow's 600-second command timeout. A unit that
/// did not start stays claimed and is reported `not_run`; the workflow finishes it on its own.
const BUDGET_SECONDS: u64 = 420;
/// The causes the workflow's `finishOrFault` treats as transient (a retry within the attempt budget).
const RETRYABLE: &[&str] = &[
    "stale_claim",
    "read_view_changed",
    "lock_busy",
    "sharing_violation",
    "sqlite_busy",
    "transport_closed",
    "command_timeout",
    "service_unavailable",
    "provider_",
    "missing_result",
    "pending_jobs",
    "cleanup_transient",
];
const LAYERS: &[&str] = &["cards", "events", "findings", "models", "explain"];

fn short(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

/// `<unit>:<token>` pairs, comma separated, at most eight, each a supported layer and named once.
pub(super) fn batch_targets(text: &str) -> Result<Vec<(String, i64)>> {
    let targets = hold_targets(text)?;
    if targets.len() > BATCH_MAX {
        bail!("-Units names {} units; a batch holds at most {BATCH_MAX}", targets.len());
    }
    let mut seen = BTreeSet::new();
    for (unit, _) in &targets {
        let layer = unit.split('/').next().unwrap_or_default();
        if !LAYERS.contains(&layer) {
            bail!("-Units names {unit}; finish-batch finishes {} units only", LAYERS.join(", "));
        }
        if !seen.insert(unit.clone()) {
            bail!("-Units names {unit} twice");
        }
    }
    Ok(targets)
}

/// The group number of a `<layer>/gNNN@1` unit.
fn group_of(unit: &str) -> Result<u64> {
    unit.split_once("/g")
        .and_then(|(_, rest)| rest.strip_suffix("@1"))
        .and_then(|digits| digits.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .with_context(|| format!("{unit} names no group"))
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
fn held(p: &Prepared, unit: &str) -> Result<Option<(String, i64, String, Value)>> {
    let conn = connect(&p.out.join("ledger.sqlite3"), false)?;
    conn.query_row(
        "SELECT state,claim_token,input_hash,result_json FROM units WHERE id=?1",
        [unit],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?)),
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

/// One unit's outcome: `(entry for the line, full record for the file, lock refusal if any)`.
fn settle_one(p: &Prepared, zt: &mut Zt, unit: &str, token: i64) -> (Value, Value, Option<Value>) {
    let finished = finish_one(p, zt, unit, token);
    let cause = match &finished {
        Ok(v) if v["state"] == json!("done") || v["complete"] == json!(true) => {
            let status = if v["status"] == json!("partial") { "partial" } else { "done" };
            return (
                json!({"unit": unit, "token": token, "status": status, "disposition": v["disposition"]}),
                json!({"unit": unit, "token": token, "status": status, "result": v}),
                None,
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
    // R326/R337: the owner's Lock is a content fault of this unit, and the batch stops with it.
    if let Err(error) = &finished
        && let Some(refusal) = crate::lockcheck::stopped_refusal(error)
    {
        let fault = json!({"schema_version": 1, "unit": unit, "input_hash": "", "code": "card_locked", "cause": short(&format!("{cause} {}", util::compact(&refusal)), 800)});
        let recorded = fail(p, unit, token, "content", &fault).is_ok();
        return (
            json!({"unit": unit, "token": token, "status": "failed", "code": "card_locked", "cause": short(&cause, 200), "recorded": recorded}),
            json!({"unit": unit, "token": token, "status": "failed", "fault": fault, "recorded": recorded}),
            Some(refusal),
        );
    }
    // The ledger decides every finish that is not plainly done before any fault: a unit it holds done was
    // finished (a repeated batch, or a completion whose reply failed after its commit).
    match held(p, unit) {
        Ok(Some((state, _, _, disposition))) if state == "done" => {
            return (
                json!({"unit": unit, "token": token, "status": "done", "disposition": disposition, "from": "ledger"}),
                json!({"unit": unit, "token": token, "status": "done", "from": "ledger", "result": result, "cause": cause}),
                None,
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
            );
        }
        _ => {}
    }
    let retryable = RETRYABLE.iter().any(|word| cause.contains(word));
    let (class, code, status) = if retryable { ("retryable", "provider_transient", "retry") } else { ("content", "check_failed", "failed") };
    let fault = json!({"schema_version": 1, "unit": unit, "input_hash": "", "code": code, "cause": cause});
    let (recorded, refused) = match fail(p, unit, token, class, &fault) {
        Ok(_) => (true, Value::Null),
        Err(error) => (false, json!(short(&format!("{error:#}"), 300))),
    };
    (
        json!({"unit": unit, "token": token, "status": status, "code": code, "cause": short(&cause, 200), "recorded": recorded}),
        json!({"unit": unit, "token": token, "status": status, "fault": fault, "recorded": recorded, "refused": refused, "result": result}),
        None,
    )
}

/// Finishes the listed claims in turn; returns the printed line and the exit code.
pub(super) fn finish_batch(p: &Prepared, targets: &[(String, i64)], claim: Option<&Value>) -> Result<(Value, i32)> {
    let started = std::time::Instant::now();
    let mut zt = crate::zt::start()?;
    let mut entries = Vec::new();
    let mut records = Vec::new();
    let mut lock: Option<Value> = None;
    for (unit, token) in targets {
        if lock.is_some() || started.elapsed().as_secs() > BUDGET_SECONDS {
            entries.push(json!({"unit": unit, "token": token, "status": "not_run"}));
            records.push(json!({"unit": unit, "token": token, "status": "not_run"}));
            continue;
        }
        let (entry, record, refusal) = settle_one(p, &mut zt, unit, *token);
        entries.push(entry);
        records.push(record);
        if refusal.is_some() {
            lock = refusal;
        }
    }
    let count = |status: &str| entries.iter().filter(|e| e["status"] == json!(status)).count();
    let layer = targets.first().map(|(unit, _)| unit.split('/').next().unwrap_or_default().to_string()).unwrap_or_default();
    let names: Vec<String> = targets.iter().map(|(unit, token)| format!("{unit}:{token}")).collect();
    let folder = p.out.join("finish");
    std::fs::create_dir_all(&folder)?;
    let file = folder.join(format!("{layer}-{}.json", &canon::sha256_hex(names.join(",").as_bytes())[..12]));
    util::write_atomic(&file, &serde_json::to_string_pretty(&json!({"schema_version": 1, "kind": "finish_batch", "units": records, "claim": claim}))?)?;
    let mut line = json!({
        "schema_version": 1, "kind": "finish_batch", "units": entries.len(),
        "done": count("done"), "partial": count("partial"), "retry": count("retry"), "failed": count("failed"), "not_run": count("not_run"),
        "results": entries, "file": file.display().to_string(),
    });
    if let Some(claim) = claim {
        for key in ["layer", "settled", "complete", "counts"] {
            line[key] = claim[key].clone();
        }
        line["claimed"] = json!(targets.len());
    }
    let mut exit = 0;
    if let Some(refusal) = lock {
        // The batch line itself carries the typed refusal, so the workflow stops the stage on it.
        line["code"] = json!("card_locked");
        line["error"] = json!("card_locked");
        line["message"] = refusal["message"].as_str().or_else(|| refusal["error"].as_str()).map(|t| json!(t)).unwrap_or(Value::Null);
        line["refusal"] = refusal;
        exit = crate::lockcheck::LOCKED_EXIT;
    }
    Ok((line, exit))
}

/// `finish-batch -Out <out> (-Units "<unit>:<token>,…" | -Layer <l> -Take <n> -Owner <label> [-Wait s] [-Lease s])`.
pub fn cli(argv: &[String]) -> Result<i32> {
    enum Mode {
        Units(Vec<(String, i64)>),
        Claim { layer: String, take: i64, owner: String, wait: i64, lease: i64 },
    }
    let parsed = (|| -> Result<(String, Mode)> {
        let a = Args::parse(argv, &["Out", "Units", "Layer", "Take", "Owner", "Wait", "Lease"], &[])?;
        if !a.positional.is_empty() || a.required("Out")?.trim().is_empty() {
            bail!("explicit -Out is required");
        }
        if a.has("Units") {
            if a.has("Layer") || a.has("Take") || a.has("Owner") || a.has("Wait") || a.has("Lease") {
                bail!("-Units finishes the named claims; it takes no -Layer, -Take, -Owner, -Wait or -Lease");
            }
            return Ok((a.str("Out"), Mode::Units(batch_targets(&a.str("Units"))?)));
        }
        let layer = a.required("Layer")?;
        if !LAYERS.contains(&layer.as_str()) {
            bail!("-Layer must be one of {}", LAYERS.join(", "));
        }
        let number = |name: &str, default: i64, low: i64, high: i64| -> Result<i64> {
            if !a.has(name) {
                return Ok(default);
            }
            a.str(name).trim().parse::<i64>().ok().filter(|n| (low..=high).contains(n)).with_context(|| format!("-{name} must be a whole number from {low} through {high}"))
        };
        let take = number("Take", BATCH_MAX as i64, 1, BATCH_MAX as i64)?;
        let wait = number("Wait", 0, 0, 600)?;
        let lease = number("Lease", 0, 0, LEASE_MAX)?;
        let owner = a.required("Owner")?;
        if owner.trim().is_empty() || owner.len() > 256 {
            bail!("-Owner must be nonempty and at most 256 bytes");
        }
        Ok((a.str("Out"), Mode::Claim { layer, take, owner, wait, lease }))
    })();
    let (out, mode) = match parsed {
        Ok(value) => value,
        Err(error) => {
            eprintln!("zt-digest finish-batch: {error}");
            return Ok(2);
        }
    };
    let mut zt = crate::zt::start()?;
    let p = Prepared::read(&out, &mut zt)?;
    drop(zt);
    let (line, exit) = match mode {
        Mode::Units(targets) => finish_batch(&p, &targets, None)?,
        Mode::Claim { layer, take, owner, wait, lease } => {
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
                let (mut line, exit) = finish_batch(&p, &targets, Some(&brief))?;
                for key in ["layer", "settled", "complete", "counts"] {
                    line[key] = claim[key].clone();
                }
                (line, exit)
            }
        }
    };
    util::print_line(&serde_json::to_string(&line)?)?;
    Ok(exit)
}

/// `model-deps -Out <out> -Units "<unit>:<token>,…"`: every listed Model unit's references in one
/// process, one short entry per unit (ok, waiting, state, or the error). A unit's error never stops the
/// others, as one relay per model never stopped the next.
pub fn model_deps_batch(p: &Prepared, targets: &[(String, i64)]) -> Result<Value> {
    let mut zt = crate::zt::start()?;
    let mut results = Vec::new();
    for (unit, token) in targets {
        results.push(match model_deps(p, &mut zt, unit, *token) {
            Ok(v) => json!({"unit": unit, "ok": v["ok"], "waiting": v["waiting"], "state": v["state"], "in_batch": v["in_batch"].as_array().map(Vec::len)}),
            Err(error) => json!({"unit": unit, "error": short(&format!("{error:#}"), 300)}),
        });
    }
    Ok(json!({"schema_version": 1, "kind": "model_deps_batch", "units": results.len(), "results": results}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_names_at_most_eight_supported_units_once() {
        let ok = batch_targets("cards/g001@1:2, events/g002@1:1,models/g003-m0001@1:4").unwrap();
        assert_eq!(ok, vec![("cards/g001@1".into(), 2), ("events/g002@1".into(), 1), ("models/g003-m0001@1".into(), 4)]);
        assert!(batch_targets("cards/g001@1:1,cards/g001@1:2").unwrap_err().to_string().contains("twice"));
        assert!(batch_targets("digest/g001@1:1").unwrap_err().to_string().contains("only"));
        assert!(batch_targets("cards/g001@1:0").is_err(), "a token is positive");
        let nine: Vec<String> = (1..=9).map(|n| format!("cards/g{n:03}@1:1")).collect();
        assert!(batch_targets(&nine.join(",")).unwrap_err().to_string().contains("at most 8"));
    }

    #[test]
    fn a_unit_names_its_group() {
        assert_eq!(group_of("cards/g012@1").unwrap(), 12);
        assert!(group_of("models/g001-m0002@1").is_err());
    }

    #[test]
    fn the_transient_causes_are_the_workflows_own() {
        for cause in ["stale_claim: x", "read_view_changed", "provider_cap", "cleanup_transient"] {
            assert!(RETRYABLE.iter().any(|w| cause.contains(w)), "{cause}");
        }
        assert!(!RETRYABLE.iter().any(|w| "check_failed: a hard fault".contains(w)));
    }
}
