//! Shared classification and orphan facts over an already complete graph view.
//! Callers own reads and revision consistency; generated/navigation edges are
//! never inputs. Inbound sources may be in either Workspace.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

// The orphan rule vocabulary and the Card classification are the core's: the
// write gate and the audit path share one definition.
pub use crate::core::{
    EXCLUDED, SUBSTANTIVE, category, category_with_parts, is_orphan, is_restatement_entry,
};

/// Complete explicit agent target scope. Missing inbound facts are an error,
/// never a fabricated zero. Strong kinded inbound remains a separate layer rule.
pub fn orphan_report(cards: &[Value], revision: u64, changed: &[String]) -> Result<Value> {
    let mut targets = BTreeSet::new();
    let mut eligible = BTreeSet::new();
    let mut excluded: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut list = BTreeSet::new();
    let mut kinded = BTreeMap::new();
    for card in cards {
        let address = card["address"]
            .as_str()
            .context("incomplete orphan scope: missing address")?;
        let category = card["category"]
            .as_str()
            .context("incomplete orphan scope: missing category")?;
        if card["workspace"] != "agent" || !targets.insert(address.to_string()) {
            bail!("invalid orphan target scope at `{address}`");
        }
        if SUBSTANTIVE.contains(&category) {
            eligible.insert(address.to_string());
            kinded.insert(
                address,
                card["inbound_kinded"]
                    .as_u64()
                    .context("incomplete Idea-kinded inbound statistic")?,
            );
            let inbound = card["inbound"]
                .as_array()
                .with_context(|| format!("incomplete orphan inputs at `{address}`"))?
                .iter()
                .map(|from| {
                    from.as_str()
                        .map(str::to_string)
                        .context("incomplete orphan source address")
                })
                .collect::<Result<Vec<_>>>()?;
            if is_orphan(address, category, &inbound) {
                list.insert(address.to_string());
            }
        } else if EXCLUDED.contains(&category) {
            excluded
                .entry(category.to_string())
                .or_default()
                .push(address.to_string());
        } else {
            bail!("unknown orphan target category `{category}` at `{address}`");
        }
    }
    for addresses in excluded.values_mut() {
        addresses.sort();
    }
    let changed: BTreeSet<_> = changed.iter().collect();
    Ok(
        json!({"complete":true,"archive_revision":revision,"definition":"substantive_regular_without_other_authored_inbound","source_workspaces":["human","agent"],"target_workspace":"agent","eligible_categories":SUBSTANTIVE,"excluded_categories":EXCLUDED,"targets":targets,"eligible":eligible,"excluded":excluded,"list":list,"count":list.len(),"inbound_from_idea_with_kind":kinded,"changed_cards":changed,"changed_cards_scope":"authored_targets_only"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::{HashMap, HashSet};

    /// The graph classifier derives the same navigation role from the authored section, from the
    /// flag the archive derives from it, and from nothing else.
    #[test]
    fn the_entry_role_is_derived_from_the_section_or_the_archive_flag() {
        let literature = HashSet::new();
        let anchors = HashMap::new();
        let by_body = json!({
            "address": "1/1", "type": "idea", "title": "入口",
            "body": "陈述。\n\n## 同述入口\n\n- [[1/2]] 定义。\n",
        });
        assert_eq!(
            crate::graph::category(&by_body, &literature, &anchors),
            "restatement-entry"
        );
        let by_flag = json!({
            "address": "1/1", "type": "idea", "title": "入口", "restatement": true,
        });
        assert_eq!(
            crate::graph::category(&by_flag, &literature, &anchors),
            "restatement-entry"
        );
        let prose = json!({
            "address": "1/1", "type": "idea", "title": "入口",
            "body": "这张卡是 [[1/2]] 的同述入口。\n",
        });
        assert_eq!(crate::graph::category(&prose, &literature, &anchors), "idea");
        assert!(crate::graph::EXCLUDED.contains(&"restatement-entry"));
        assert!(!crate::graph::SUBSTANTIVE.contains(&"restatement-entry"));
        assert!(crate::graph::is_restatement_entry(
            "## 同述入口\n\n- [[1/2]] 定义。\n"
        ));
        assert!(!crate::graph::is_restatement_entry(
            "```\n## 同述入口\n- [[1/2]]\n```\n"
        ));
    }
}
