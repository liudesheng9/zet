//! `cleanup-run`: the run's DeepSeek cleanup with no model in the loop. The ledger already holds every id
//! the run created and every workspace it used, so the stage needs exact provider facts, not agents to relay
//! them (ebrill2001modern trial 1 spent a Haiku agent per 40 conversations, per 8 workspaces and per cleanup
//! unit). Two steps around one provider call:
//!
//! 1. `-Step plan` records the evidence of every conversation whose answer file exists, then writes
//!    `<out>/cleanup/settle-plan.json`: a delete op for every recorded id not yet deleted and a
//!    settle_workspace op for every recorded workspace not yet removed.
//! 2. The DeepSeek bridge runs that plan (`deepseek_settle`, which closes the conversations its server
//!    holds open first) and writes `<out>/cleanup/settle-results.json`.
//! 3. `-Step apply` checks that the results answer that plan op for op, records each fact (terminal state,
//!    deletion, workspace listing and removal), then settles every ready cleanup unit (`cleanup_ready`).
//!
//! Nothing here deletes by title, and the bridge deletes only the ids the plan names: a workspace that still
//! lists a conversation is kept, and each member the run did not record is reported as the user's.
use super::*;

const PLAN: &str = "settle-plan.json";
const RESULTS: &str = "settle-results.json";

fn files(p: &Prepared) -> Result<(PathBuf, PathBuf)> {
    let folder = p.out.join("cleanup");
    std::fs::create_dir_all(&folder)?;
    Ok((folder.join(PLAN), folder.join(RESULTS)))
}

fn deleted(conversation: &Value) -> bool {
    matches!(conversation["delete"].as_str(), Some("confirmed" | "already_absent"))
}

/// The ops for the run's recorded resources: every recorded id not yet deleted, then every recorded
/// workspace not yet removed (a folder once, whatever its case).
pub(super) fn settle_ops(resources: &[Value]) -> Vec<Value> {
    let mut ops = Vec::new();
    let mut ids: BTreeSet<String> = BTreeSet::new();
    for resource in resources {
        for conversation in resource["conversations"].as_array().into_iter().flatten() {
            let Some(id) = conversation["id"].as_str() else { continue };
            if conversation["lost"] == json!(true) || deleted(conversation) || !ids.insert(id.to_string()) {
                continue;
            }
            ops.push(json!({"op":"delete","session_id":id}));
        }
    }
    let mut folders: BTreeSet<String> = BTreeSet::new();
    for resource in resources {
        let Some(path) = resource["workspace"]["path"].as_str() else { continue };
        if resource["workspace_removed"] == json!(true) || !folders.insert(path.to_lowercase()) {
            continue;
        }
        ops.push(json!({"op":"settle_workspace","cwd":path}));
    }
    ops
}

/// `-Step plan`: the evidence sweep and the plan file. A results file left from an earlier plan is removed,
/// so apply can never read an answer to another plan.
pub(super) fn plan(p: &Prepared) -> Result<Value> {
    let answers = p.out.join("answers");
    let swept = if answers.is_dir() { resource_sweep_evidence(p, &answers.to_string_lossy())? } else { json!({"recorded":0,"missing":0}) };
    let resources = recorded_resources(&connect(&p.out.join("ledger.sqlite3"), false)?, None)?;
    let ops = settle_ops(&resources);
    let (plan, results) = files(p)?;
    if results.exists() {
        std::fs::remove_file(&results)?;
    }
    std::fs::write(&plan, serde_json::to_string_pretty(&ops)?)?;
    let count = |name: &str| ops.iter().filter(|op| op["op"] == json!(name)).count();
    Ok(json!({
        "schema_version":1,"kind":"cleanup_plan",
        "plan_file":plan.to_string_lossy(),"results_file":results.to_string_lossy(),
        "deletes":count("delete"),"workspaces":count("settle_workspace"),
        "evidence_recorded":swept["recorded"],"evidence_missing":swept["missing"]
    }))
}

/// The result answers its op: the same op on the same id or folder, in plan order.
fn answers(op: &Value, result: &Value) -> bool {
    op["op"] == result["op"]
        && match op["op"].as_str() {
            Some("delete") => op["session_id"] == result["session_id"],
            Some("settle_workspace") => op["cwd"].as_str().map(str::to_lowercase) == result["cwd"].as_str().map(str::to_lowercase),
            _ => false,
        }
}

/// `-Step apply`: record every provider fact, then settle the ready cleanup units.
pub(super) fn apply(p: &Prepared, owner: &str) -> Result<Value> {
    let (plan_file, results_file) = files(p)?;
    let ops: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&plan_file).context("no settle plan: run cleanup-run -Step plan first")?)?;
    let answered: Value = serde_json::from_str(
        &std::fs::read_to_string(&results_file).context("no settle results: the bridge's deepseek_settle has not run on this plan")?,
    )?;
    let results = answered["results"].as_array().cloned().unwrap_or_default();
    if answered["ok"] != json!(true) || results.len() != ops.len() || ops.iter().zip(&results).any(|(op, result)| !answers(op, result)) {
        bail!("settle results do not answer the plan op for op; run the plan again");
    }
    let resources = recorded_resources(&connect(&p.out.join("ledger.sqlite3"), false)?, None)?;
    let terminal: BTreeSet<String> = resources
        .iter()
        .flat_map(|r| r["conversations"].as_array().cloned().unwrap_or_default())
        .filter(|c| c["state"] == json!("terminal"))
        .filter_map(|c| c["id"].as_str().map(str::to_string))
        .collect();
    let recorded_at = |path: &str| -> BTreeSet<String> {
        resources
            .iter()
            .filter(|r| r["workspace"]["path"].as_str().is_some_and(|known| known.eq_ignore_ascii_case(path)))
            .flat_map(|r| r["conversations"].as_array().cloned().unwrap_or_default())
            .filter_map(|c| c["id"].as_str().map(str::to_string))
            .collect()
    };
    let mut unrecorded: Vec<Value> = Vec::new();
    let mut note = |what: &str, outcome: Result<Value>| {
        if let Err(error) = outcome {
            unrecorded.push(json!({"fact":what,"error":format!("{error:#}").chars().take(160).collect::<String>()}));
        }
    };
    let (mut gone, mut absent, mut removed, mut kept) = (0, 0, 0, 0);
    let mut failed: Vec<Value> = Vec::new();
    let mut strays: Vec<Value> = Vec::new();
    for result in &results {
        match result["op"].as_str() {
            Some("delete") => {
                let id = field(result, "session_id")?;
                let status = result["status"].as_str().unwrap_or("failed");
                let record = |id: &str| -> Result<Value> {
                    let key = resource_holding(p, id)?;
                    // A conversation already terminal keeps its recorded outcome; only its deletion is new.
                    if !terminal.contains(id) {
                        let outcome = if status == "absent" { "absent" } else { "finished" };
                        resource_observe_input(p, json!({"schema_version":1,"op":"terminal","resource":key,"conversation":id,"state":"finished","outcome":outcome}), "", None)?;
                    }
                    match status {
                        "deleted" | "absent" => resource_observe_input(
                            p,
                            json!({"schema_version":1,"op":"deleted","resource":key,"conversation":id,"result":if status == "deleted" { "confirmed" } else { "already_absent" }}),
                            "",
                            None,
                        ),
                        _ => resource_observe_input(
                            p,
                            json!({"schema_version":1,"op":"delete_failed","resource":key,"conversation":id,"code":"provider","message":result["error"].as_str().unwrap_or("the provider refused the deletion").chars().take(300).collect::<String>()}),
                            "",
                            None,
                        ),
                    }
                };
                note(id, record(id));
                match status {
                    "deleted" => gone += 1,
                    "absent" => absent += 1,
                    _ => failed.push(json!({"id":id,"error":result["error"]})),
                }
            }
            _ => {
                let cwd = field(result, "cwd")?;
                match result["status"].as_str() {
                    Some("removed" | "absent") => {
                        removed += 1;
                        note(cwd, resource_settle_workspace(p, cwd, true, "", "", "", None));
                    }
                    Some("kept") => {
                        kept += 1;
                        let ours = recorded_at(cwd);
                        let listed: Vec<(String, Value)> = result["listed"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|c| c["id"].as_str().map(|id| (id.to_string(), c["title"].clone())))
                            .collect();
                        let remaining: Vec<&str> = listed.iter().filter(|(id, _)| ours.contains(id)).map(|(id, _)| id.as_str()).collect();
                        let unknown: Vec<&str> = listed.iter().filter(|(id, _)| !ours.contains(id)).map(|(id, _)| id.as_str()).collect();
                        for (id, title) in listed.iter().filter(|(id, _)| !ours.contains(id)) {
                            strays.push(json!({"workspace":cwd,"id":id,"title":title}));
                        }
                        note(cwd, resource_settle_workspace(p, cwd, false, &remaining.join(","), &unknown.join(","), "", None));
                    }
                    _ => failed.push(json!({"workspace":cwd,"error":result["error"]})),
                }
            }
        }
    }
    let ready = cleanup_ready(p, owner)?;
    Ok(json!({
        "schema_version":1,"kind":"cleanup_run",
        "deleted":gone,"absent":absent,"workspaces_removed":removed,"workspaces_kept":kept,
        "failed_count":failed.len(),"failed":failed.into_iter().take(8).collect::<Vec<_>>(),
        "unknown_count":strays.len(),"unknown":strays.into_iter().take(8).collect::<Vec<_>>(),
        "unrecorded_count":unrecorded.len(),"unrecorded":unrecorded.into_iter().take(8).collect::<Vec<_>>(),
        "ready":ready
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The plan names each recorded id not yet deleted once, and each workspace not yet removed once.
    #[test]
    fn the_plan_names_only_what_the_run_still_owes() {
        let resources = vec![
            json!({"workspace":{"path":"D:\\run\\jobs"},"workspace_removed":null,"conversations":[
                {"id":"a","delete":null},{"id":"b","delete":"confirmed"},{"id":null,"lost":true},{"id":"c","delete":"failed"}]}),
            json!({"workspace":{"path":"d:\\RUN\\jobs"},"workspace_removed":null,"conversations":[{"id":"a","delete":null}]}),
            json!({"workspace":{"path":"D:\\run\\done"},"workspace_removed":true,"conversations":[{"id":"d","delete":"already_absent"}]}),
        ];
        assert_eq!(
            settle_ops(&resources),
            vec![
                json!({"op":"delete","session_id":"a"}),
                json!({"op":"delete","session_id":"c"}),
                json!({"op":"settle_workspace","cwd":"D:\\run\\jobs"}),
            ]
        );
    }

    #[test]
    fn a_result_answers_only_its_own_op() {
        let op = json!({"op":"settle_workspace","cwd":"D:\\Run"});
        assert!(answers(&op, &json!({"op":"settle_workspace","cwd":"d:\\run","status":"kept"})));
        assert!(!answers(&op, &json!({"op":"settle_workspace","cwd":"D:\\Other"})));
        assert!(!answers(&json!({"op":"delete","session_id":"a"}), &json!({"op":"delete","session_id":"b"})));
    }
}
