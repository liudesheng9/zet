//! Shared classification and orphan facts over an already complete graph view.
//! Callers own reads and revision consistency; generated/navigation edges are
//! never inputs. Inbound sources may be in either Workspace.

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

static ANCHOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^/]+/\d+$").unwrap());
static TERMS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^术语表").unwrap());
static NOTATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(符号表|notation)").unwrap());
static STRUCTURE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^本卡是《.*》的结构卡").unwrap());

pub const SUBSTANTIVE: &[&str] = &["idea", "event", "finding", "model", "part"];
pub const EXCLUDED: &[&str] = &[
    "root",
    "anchor",
    "structure",
    "terms",
    "notation",
    "hub",
    "register-part",
    "restatement-entry",
];

/// Whether a Card body holds the `## 同述入口` section: the derived navigation role of an accepted
/// restatement entry (§4.5, D086). The role is derived from the written text on every read, never
/// stored as Card metadata; a heading inside a code fence is not a section.
pub fn is_restatement_entry(body: &str) -> bool {
    let mut fenced = false;
    for raw in body.lines() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(heading) = trimmed
            .strip_prefix("## ")
            .or_else(|| trimmed.strip_prefix("### "))
            && heading.trim() == "同述入口"
        {
            return true;
        }
    }
    false
}

fn field<'a>(card: &'a Value, name: &str) -> &'a str {
    card[name].as_str().unwrap_or_default()
}

/// Classify the same metadata for audit, compiler and standalone checks. A scope that does not
/// read the register's declaration classifies no Card as a register part; the register read does.
pub fn category(
    card: &Value,
    literature: &HashSet<String>,
    anchors: &HashMap<String, usize>,
) -> String {
    category_with_parts(card, literature, anchors, &HashSet::new())
}

/// The same classification, with the addresses a register's root declares under `词条分册` (§8.5,
/// D188): a register part is a navigation Card holding term rows, not a substantive Idea, so it
/// triggers no claim-title or Idea-size rule and needs no topic hub of its own.
pub fn category_with_parts(
    card: &Value,
    literature: &HashSet<String>,
    anchors: &HashMap<String, usize>,
    register_parts: &HashSet<String>,
) -> String {
    if register_parts.contains(field(card, "address")) {
        return "register-part".into();
    }
    if matches!(field(card, "kind"), "topic" | "literature") {
        return "root".into();
    }
    match field(card, "type") {
        "finding" => return "finding".into(),
        "event" => return "event".into(),
        "hub" => return "hub".into(),
        "model" => {
            return if field(card, "facet").is_empty() {
                "model"
            } else {
                "part"
            }
            .into();
        }
        _ => {}
    }
    let tree = field(card, "tree");
    let in_book = literature.contains(tree);
    if in_book && ANCHOR.is_match(field(card, "address")) {
        return "anchor".into();
    }
    if TERMS.is_match(field(card, "title")) {
        return "terms".into();
    }
    if NOTATION.is_match(field(card, "title")) {
        return "notation".into();
    }
    let prose = card["excerpt"]
        .as_str()
        .unwrap_or_else(|| field(card, "body"));
    if in_book && STRUCTURE.is_match(prose) {
        return "structure".into();
    }
    // An accepted restatement entry names the substantive statement at another address: it is
    // navigation, not a claim of its own (§4.5, D086). The role comes from the authored section
    // (or the flag the archive derives from it), never from a stored Card field.
    if card["restatement"] == true
        || card["body"]
            .as_str()
            .is_some_and(|body| is_restatement_entry(body))
    {
        return "restatement-entry".into();
    }
    let parent = card["parent"]
        .as_str()
        .unwrap_or_else(|| field(card, "parent_address"));
    if in_book
        && parent == format!("{tree}/1")
        && field(card, "edge") == "side"
        && field(card, "type") == "idea"
    {
        let own = Regex::new(&format!(r"^{}/\d+$", regex::escape(tree))).unwrap();
        let count = card["links"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|target| own.is_match(target))
            .collect::<HashSet<_>>()
            .len();
        if count >= 3 && anchors.get(tree).is_none_or(|total| count * 2 >= *total) {
            return "structure".into();
        }
    }
    "idea".into()
}

/// Filing, outbound citations and Reverse text are not inbound authored Links.
/// An actual authored Link from a parent is evidence; a self-Link is not.
pub fn is_orphan(address: &str, category: &str, inbound: &[String]) -> bool {
    SUBSTANTIVE.contains(&category)
        && !inbound
            .iter()
            .any(|from| !from.is_empty() && from != address)
}

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
