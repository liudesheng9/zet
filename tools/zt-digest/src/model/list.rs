//! `zt-digest list-models` (list-models.ps1): the Model Cards a digest wrote.
//! Reads only.
//!
//!   zt-digest list-models -Staging <folder> -Groups 1,2,3
//!   zt-digest list-models -Staging <folder> -All [-CitationKey <key>]
//!
//! With -Groups, a Model Card counts when one of those groups wrote it (its
//! address is in owned-gNN.txt). With -All, every Model Card in the agent
//! Literature tree of -CitationKey counts, plus every Model Card any group
//! wrote. Model parts are never listed. Prints one compact JSON line:
//! {"models":[{"address","title","group","level","form","parts","issues","faults"}]}.

use super::ps::{self, CiMap, pipe, s, truthy};
use super::read_calls;
use crate::args::Args;
use crate::util;
use crate::zt::Zt;
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::{Value, json};

pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(argv, &["Staging", "Groups", "CitationKey"], &["All"])?;
    let staging = a.required("Staging")?;
    if staging.is_empty() {
        bail!("Cannot bind argument to parameter 'Staging' because it is an empty string.");
    }
    // The parameter sets of the script: Groups (mandatory -Groups) or All (mandatory -All, -CitationKey).
    let all = a.flag("All");
    if all && a.has("Groups") || !all && a.has("CitationKey") {
        bail!("Parameter set cannot be resolved using the specified named parameters.");
    }
    if !all && !a.has("Groups") {
        bail!("-Groups is required (or -All)");
    }
    let groups = a.str("Groups");
    if !all && groups.is_empty() {
        bail!("Cannot bind argument to parameter 'Groups' because it is an empty string.");
    }
    let citation_key = a.str("CitationKey");
    let numbers: Vec<i64> = Regex::new(r"[,\s]+")
        .unwrap()
        .split(&groups)
        .filter(|g| !g.is_empty())
        .map(|g| ps::int(&Value::String(g.to_string())))
        .collect::<Result<_>>()?;

    // Which group wrote each address.
    let mut owner: CiMap<i64> = CiMap::new();
    let prefix = Regex::new("(?i)^owned-g").unwrap();
    for name in ps::child_files(&staging, "owned-g", ".txt") {
        let n = ps::int(&Value::String(prefix.replace(&name[..name.len() - 4], "").into_owned()))?;
        if !all && !numbers.contains(&n) {
            continue;
        }
        for address in ps::read_lines(&ps::join(&staging, &name))? {
            if !address.trim().is_empty() {
                owner.insert(address.trim(), n);
            }
        }
    }

    let mut zt = Zt::start()?;
    // Read whole trees, not search_cards: a search stops at 200 results. The trees are the
    // book's and those the groups wrote into, in Sort-Object -Unique order.
    let mut trees: Vec<String> = owner.keys().iter().map(|a| a.split('/').next().unwrap_or_default().to_string()).collect();
    trees.push(citation_key.clone());
    trees.retain(|t| !t.is_empty());
    trees.sort_by(|a, b| ps::culture_cmp(a, b));
    trees.dedup_by(|b, a| ps::culture_cmp(a, b).is_eq());
    // A Topic the groups wrote into may since have been merged away; get_tree on it would fail.
    let listed = read_calls(&mut zt, &[("list_trees", json!({ "workspace": "agent" }))])?;
    let existing: Vec<String> = listed.first().map(|l| pipe(&l["trees"]).iter().map(|t| s(&t["tree"])).collect()).unwrap_or_default();
    trees.retain(|t| ps::is_in(t, &existing));
    let calls: Vec<(&str, Value)> = trees.iter().map(|t| ("get_tree", json!({ "tree": t }))).collect();
    let read = if calls.is_empty() { Vec::new() } else { read_calls(&mut zt, &calls)? };
    let found: Vec<Value> = read
        .iter()
        .flat_map(|t| pipe(&t["cards"]).into_iter().cloned().collect::<Vec<_>>())
        .filter(|c| ps::eq(&s(&c["workspace"]), "agent") && ps::eq(&s(&c["type"]), "model"))
        .collect();
    let pattern = format!("{citation_key}/*");
    let heads: Vec<&Value> = found
        .iter()
        .filter(|c| {
            !truthy(&c["facet"])
                && (owner.contains(&s(&c["address"])) || (all && !citation_key.is_empty() && ps::like(&s(&c["address"]), &pattern)))
        })
        .collect();
    let mut models = Vec::new();
    if !heads.is_empty() {
        let calls: Vec<(&str, Value)> = heads.iter().map(|h| ("get_model", json!({ "at": h["address"] }))).collect();
        let structures = read_calls(&mut zt, &calls)?;
        for (i, head) in heads.iter().enumerate() {
            let st = structures.get(i).cloned().unwrap_or(Value::Null);
            let build = &st["build"];
            let count = |v: &Value| pipe(v).len();
            models.push(json!({
                "address": head["address"],
                "title": head["title"],
                "group": owner.get(&s(&head["address"])),
                "level": st["level"],
                "form": st["form"],
                "parts": count(&st["parts"]),
                "issues": count(&st["issues"]),
                "faults": count(&build["closure"]) + count(&build["forward"]) + count(&build["unanchored"]) + count(&build["beyond_level"]),
            }));
        }
    }
    util::print_line(&ps::json_compact(&json!({ "models": models })))?;
    Ok(0)
}
