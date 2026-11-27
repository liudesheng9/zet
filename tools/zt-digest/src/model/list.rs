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
//! {"models":[{"address","title","group","level","form","parts","todo","placeholders",
//! "issues","faults"}]}.
//!
//! `todo` and `placeholders` are the checked unwritten state of the model: a
//! Model Card or part whose body still holds `待写`. Selection by "parts
//! greater than zero" would call such a model built (R235): a partly written
//! model has many parts and no fault yet.

use crate::ps::{self, CiMap, pipe, s, truthy};
use super::read_calls;
use crate::args::Args;
use crate::util;
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

    let mut zt = crate::zt::start()?;
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
            // The unwritten state, not the part count, says whether a model still needs work.
            let placeholders = placeholders_of(&s(&head["address"]), &st);
            models.push(json!({
                "address": head["address"],
                "title": head["title"],
                "group": owner.get(&s(&head["address"])),
                "level": st["level"],
                "form": st["form"],
                "parts": own_parts(&s(&head["address"]), &pipe(&st["parts"])),
                "todo": placeholders.len(),
                "placeholders": placeholders,
                "issues": count(&st["issues"]),
                "faults": count(&build["closure"]) + count(&build["forward"]) + count(&build["unanchored"]) + count(&build["beyond_level"])
                    + count(&build["import_target"]) + count(&build["import_cycle"])
                    + count(&build["empirical_shape"]) + count(&build["estimate_unbound"]) + count(&build["spec_unbound"]) + count(&build["test_unbound"]) + count(&build["variable_uncovered"])
                    + count(&build["formal_shape"]) + count(&build["setting_dependency"]) + count(&build["criterion_unbound"]) + count(&build["result_unbound"]) + count(&build["proof_unbound"]) + count(&build["experiment_unbound"]) + count(&build["symbol_uncovered"]),
            }));
        }
    }
    util::print_line(&ps::json_compact(&json!({ "models": models })))?;
    Ok(0)
}

/// The parts filed under the Model Card itself (`key/26|a|…`). `get_model` also names parts the model reads from
/// elsewhere; becker2025division-appendix (2026-11-10): `26|a` had no part of its own, the listing counted 2, the
/// planner skipped it as built while its unit stayed claimed, and the finish refused it with "no accepted plan".
pub(crate) fn own_parts(head: &str, parts: &[&Value]) -> usize {
    let prefix = format!("{}|", head.to_lowercase());
    parts.iter().filter(|p| s(&p["address"]).to_lowercase().starts_with(&prefix)).count()
}

/// The unwritten Cards of one model read: its head and every part that still holds a placeholder,
/// by the run report's own test (a bare 待写 line, a skeleton section or title), so the compiler and
/// the report agree on when a head is written.
fn placeholders_of(head: &str, st: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if crate::report::is_placeholder(&s(&st["head"]["title"]), &s(&st["head"]["body"])) {
        out.push(head.to_string());
    }
    for part in pipe(&st["parts"]) {
        if crate::report::is_placeholder(&s(&part["title"]), &s(&part["body"])) {
            out.push(s(&part["address"]));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skeleton_head_with_boundary_parts_is_still_unwritten() {
        // Astra round 5: a definition whose head kept `**陈述**：待写。` but already had 例子/非例 parts was
        // listed with todo 0 and skipped as built.
        let st = json!({"head":{"title":"微分 dx：待写","body":"**陈述**：待写。

来源：[[k]]"},
            "parts":[{"address":"k/2|a|1","title":"例子","body":"$x=3$ 时 $dx=0.1$。"},{"address":"k/2|a|2","title":"非例","body":"- 是什么：待写"}]});
        assert_eq!(placeholders_of("k/2|a", &st), vec!["k/2|a".to_string(), "k/2|a|2".to_string()]);
        let written = json!({"head":{"title":"微分 dx","body":"**定义**：(C1) ……"},"parts":[{"address":"k/2|a|1","title":"例子","body":"……"}]});
        assert!(placeholders_of("k/2|a", &written).is_empty());
    }

    #[test]
    fn only_the_parts_filed_under_the_model_card_count() {
        let parts = [json!({"address":"K/26|a|a"}), json!({"address":"K/24|a|b"}), json!({"address":"K/26|ab"})];
        let refs: Vec<&Value> = parts.iter().collect();
        assert_eq!(own_parts("K/26|a", &refs), 1);
        assert_eq!(own_parts("K/26|b", &refs), 0, "a head with no part of its own is not built");
    }
}
