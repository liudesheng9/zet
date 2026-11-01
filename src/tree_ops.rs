//! Tree operations inside the caller's transaction: the one executor behind the structural items of
//! `apply_batch` (S3, #231) and the span, outline, reorder and relink tools (S4 #232, S5 #233). Each operation plans one Location mapping (and, for a delete, one delete
//! set) under the same `Access`, Link-rewrite and Lock rules as the single tools, applies it, and
//! gathers what it touched into a shared `link_index::Changes`, which the caller refreshes once.

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::Access;
use crate::link_index::Changes;
use crate::place::Target;

/// The tools a batch treats as tree operations: they rename, delete or re-point Cards.
pub(crate) const TOOLS: [&str; 9] = [
    "move_card",
    "insert_card",
    "delete_card",
    "move_span",
    "delete_span",
    "indent",
    "outdent",
    "reorder",
    "relink",
];

/// The confirmation word a single tree tool's plan names; `relink` deletes and renames nothing, so
/// it applies at once.
pub(crate) fn confirmation(tool: &str) -> Option<&'static str> {
    match tool {
        "delete_card" | "delete_span" => Some("delete"),
        "insert_card" => Some("insert"),
        "relink" => None,
        _ => Some("move"),
    }
}

pub(crate) fn is_structural(tool: &str) -> bool {
    TOOLS.contains(&tool)
}

/// What one tree operation did, in the addresses current when it ran.
pub(crate) struct Applied {
    /// The Card the operation names afterwards: the moved Card, the new Card, or the deleted Card's
    /// old address.
    pub(crate) address: String,
    /// Every renamed Card, old to new.
    pub(crate) mapping: BTreeMap<String, String>,
    pub(crate) deleted: BTreeSet<String>,
    /// The Card an insert made.
    pub(crate) created: Option<String>,
    pub(crate) rewritten: usize,
    /// Links from a staying Card to a deleted one that no `into` re-points: `(source, target)`.
    pub(crate) breaks: Vec<(String, String)>,
    pub(crate) lines: Vec<String>,
    pub(crate) accepted: crate::gate::Accepted,
}

fn string(args: &Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("missing string argument `{name}`"))
}

fn optional(args: &Value, name: &str) -> Option<String> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// The place a moving Card goes, named by a Card: `before: X` takes X's place (X moves down);
/// `after: X` becomes X's Direct successor (X's old Direct successor and its chain move down);
/// `side_of: X` becomes X's next Side successor.
pub(crate) fn destination(conn: &Connection, args: &Value) -> Result<Target> {
    if let Some(before) = optional(args, "before") {
        return Ok(Target::Before(before));
    }
    if let Some(after) = optional(args, "after") {
        crate::load_card(conn, &after)?;
        let next = crate::direct_successor(&after)?;
        return Ok(if crate::target_exists(conn, &next)? {
            Target::Before(next)
        } else {
            Target::To(next)
        });
    }
    if let Some(parent) = optional(args, "side_of") {
        crate::load_card(conn, &parent)?;
        return Ok(Target::To(crate::next_side_successor(
            conn,
            &parent,
            &BTreeSet::new(),
        )?));
    }
    bail!("name the destination with `after`, `side_of` or `before`")
}

/// Plan and apply one tree operation inside the caller's transaction.
pub(crate) fn apply_tx(
    root: &Path,
    conn: &Connection,
    access: Access,
    tool: &str,
    args: &Value,
    changes: &mut Changes,
) -> Result<Applied> {
    match tool {
        "move_card" => move_tx(conn, access, args, changes),
        "insert_card" => insert_tx(root, conn, access, args, changes),
        "delete_card" => delete_tx(conn, access, args, changes),
        "move_span" => {
            let from = string(args, "from")?;
            let through = optional(args, "through").unwrap_or_else(|| from.clone());
            let target = destination(conn, args)?;
            span_tx(conn, access, &from, &through, &target, changes)
        }
        "indent" | "outdent" => {
            let at = string(args, "at")?;
            let through = optional(args, "through").unwrap_or_else(|| at.clone());
            let locations = crate::addresses(conn)?;
            let target = if tool == "indent" {
                crate::place::indent_target(&locations, &at)?
            } else {
                crate::place::outdent_target(&locations, &at)?
            };
            span_tx(conn, access, &at, &through, &target, changes)
        }
        "delete_span" => delete_span_tx(conn, access, args, changes),
        "reorder" => reorder_tx(conn, access, args, changes),
        "relink" => relink_tx(conn, access, args, changes),
        other => bail!("`{other}` is not a tree operation"),
    }
}

/// Move the span `from` through `through` with its Side successors to `target` (S4, #232).
fn span_tx(
    conn: &Connection,
    access: Access,
    from: &str,
    through: &str,
    target: &Target,
    changes: &mut Changes,
) -> Result<Applied> {
    let card = crate::load_card(conn, from)?;
    access.ensure_writable(card.workspace)?;
    let plan = crate::place::span_plan(conn, &card.address, through, target)?;
    access.ensure_link_rewrites_allowed(conn, &plan.mapping)?;
    crate::apply_location_mapping_tx(conn, &plan.mapping, changes)?;
    Ok(Applied {
        address: plan.mapping.get(&card.address).cloned().unwrap_or(card.address),
        lines: crate::move_verification_lines(&plan),
        rewritten: plan.rewritten_links,
        mapping: plan.mapping,
        deleted: BTreeSet::new(),
        created: None,
        breaks: Vec::new(),
        accepted: crate::gate::Accepted::none(),
    })
}

/// Delete the span `from` through `through` with its Side successors; the rest of its chain moves
/// up into its place (S4, #232).
fn delete_span_tx(conn: &Connection, access: Access, args: &Value, changes: &mut Changes) -> Result<Applied> {
    let from = string(args, "from")?;
    let through = optional(args, "through").unwrap_or_else(|| from.clone());
    let card = crate::load_card(conn, &from)?;
    access.ensure_writable(card.workspace)?;
    let (mut plan, closes_chain) = crate::span_delete_plan(conn, &card.address, &through)?;
    if let Some(into) = optional(args, "into") {
        plan = plan.into(conn, &into)?;
        access.ensure_link_rewrites_allowed(conn, &plan.repoint_mapping())?;
    }
    if closes_chain && plan.compaction_kept_out() {
        bail!(
            "{} Link(s) reach the span, and the Cards after it would take its place; pass `into` to re-point them to a Card that stays",
            plan.inbound.len()
        );
    }
    access.ensure_link_rewrites_allowed(conn, &plan.compaction())?;
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Delete(&plan))?;
    let lines = crate::delete_verification_lines(&plan);
    let breaks = if plan.into.is_some() {
        Vec::new()
    } else {
        plan.inbound.clone()
    };
    crate::apply_delete_plan_into(conn, &plan, changes)?;
    Ok(Applied {
        address: card.address,
        mapping: plan.compaction(),
        deleted: plan.delete_set.clone(),
        created: None,
        rewritten: if plan.into.is_some() { plan.inbound.len() } else { 0 },
        breaks,
        lines,
        accepted: crate::gate::Accepted::none(),
    })
}

fn strings(value: &Value, name: &str) -> Result<Vec<String>> {
    value
        .as_array()
        .with_context(|| format!("`{name}` must be an array of addresses"))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .with_context(|| format!("every item of `{name}` must be an address"))
        })
        .collect()
}

/// Put a Card's Side successors, or the spans of the Direct chain after it, in a new order (S5, #233).
fn reorder_tx(conn: &Connection, access: Access, args: &Value, changes: &mut Changes) -> Result<Applied> {
    let parent = string(args, "parent")?;
    let card = crate::load_card(conn, &parent)?;
    access.ensure_writable(card.workspace)?;
    let locations = crate::addresses(conn)?;
    let mapping = match (args.get("side"), args.get("direct")) {
        (Some(side), None) => crate::place::reorder_side(&locations, &card.address, &strings(side, "side")?)?,
        (None, Some(direct)) => {
            let spans = direct
                .as_array()
                .context("`direct` must be an array of [from, through] pairs")?
                .iter()
                .map(|pair| {
                    let pair = strings(pair, "direct")?;
                    match pair.as_slice() {
                        [from, through] => Ok((from.clone(), through.clone())),
                        [one] => Ok((one.clone(), one.clone())),
                        _ => bail!("each item of `direct` is [from, through] or [card]"),
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            crate::place::reorder_direct(&locations, &card.address, &spans)?
        }
        _ => bail!("reorder takes exactly one of `side` (Side successors) or `direct` (spans of the Direct chain)"),
    };
    if mapping.is_empty() {
        bail!("the Cards are already in this order; nothing moves");
    }
    access.ensure_link_rewrites_allowed(conn, &mapping)?;
    let plan = crate::MovePlan {
        rewritten_links: crate::count_rewritten_links(conn, &mapping)?,
        mapping,
    };
    crate::apply_location_mapping_tx(conn, &plan.mapping, changes)?;
    Ok(Applied {
        address: card.address,
        lines: crate::move_verification_lines(&plan),
        rewritten: plan.rewritten_links,
        mapping: plan.mapping,
        deleted: BTreeSet::new(),
        created: None,
        breaks: Vec::new(),
        accepted: crate::gate::Accepted::none(),
    })
}

/// Re-point authored Links from `from` to `to` without deleting anything (S5, #233): every source
/// that links `from`, or only the listed `sources`. A Term use keeps its label.
fn relink_tx(conn: &Connection, access: Access, args: &Value, changes: &mut Changes) -> Result<Applied> {
    let from = crate::load_card(conn, &string(args, "from")?)?.address;
    let to = crate::load_card(conn, &string(args, "to")?)?.address;
    if from == to {
        bail!("relink needs two different Cards");
    }
    if !crate::link_index::ready(conn) {
        bail!("relink reads the Link index; restart the service (`zt up`) to build it");
    }
    let linking = crate::link_index::sources_linking(conn, [&from])?;
    let sources = match args.get("sources") {
        None | Some(Value::Null) => linking.into_iter().collect::<Vec<_>>(),
        Some(listed) => {
            let listed = strings(listed, "sources")?;
            if let Some(stray) = listed.iter().find(|source| !linking.contains(*source)) {
                bail!("`{stray}` has no Link to `{from}`");
            }
            listed
        }
    };
    if sources.is_empty() {
        bail!("no Card links `{from}`; nothing to relink");
    }
    let mapping = BTreeMap::from([(from.clone(), to.clone())]);
    let mut lines = vec!["relink verification:".to_string()];
    let mut rewritten = 0;
    for source in &sources {
        let card = crate::load_card(conn, source)?;
        access.ensure_writable(card.workspace)?;
        crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Edit(card.row_id))?;
        let old_body = crate::parse_card_text(&card.text)?.body;
        let rewrite = crate::link::LinkBody::parse(&old_body)?.rewrite_targets(&mapping);
        crate::link_index::validate(conn, &rewrite.body, Some(&old_body))?;
        let text = crate::replace_link_locations(&card.text, &mapping)?;
        conn.execute(
            "UPDATE cards SET text = ?1 WHERE row_id = ?2",
            rusqlite::params![text, card.row_id],
        )?;
        changes.resync(conn, &card.address)?;
        rewritten += rewrite.count;
        lines.push(format!("{}: [[{from}]] -> [[{to}]] ({} macro(s))", card.address, rewrite.count));
    }
    lines.push(format!("link macros rewritten: {rewritten}"));
    Ok(Applied {
        address: to,
        mapping: BTreeMap::new(),
        deleted: BTreeSet::new(),
        created: None,
        rewritten,
        breaks: Vec::new(),
        lines,
        accepted: crate::gate::Accepted::none(),
    })
}

fn move_tx(conn: &Connection, access: Access, args: &Value, changes: &mut Changes) -> Result<Applied> {
    let at = string(args, "at")?;
    let card = crate::load_card(conn, &at)?;
    access.ensure_writable(card.workspace)?;
    let target = destination(conn, args)?;
    let alone = args.get("alone").and_then(Value::as_bool).unwrap_or(false);
    let plan = crate::place::move_plan(conn, &card.address, &target, alone)?;
    access.ensure_link_rewrites_allowed(conn, &plan.mapping)?;
    crate::apply_location_mapping_tx(conn, &plan.mapping, changes)?;
    Ok(Applied {
        address: plan.mapping.get(&card.address).cloned().unwrap_or(card.address),
        lines: crate::move_verification_lines(&plan),
        rewritten: plan.rewritten_links,
        mapping: plan.mapping,
        deleted: BTreeSet::new(),
        created: None,
        breaks: Vec::new(),
        accepted: crate::gate::Accepted::none(),
    })
}

fn insert_tx(
    root: &Path,
    conn: &Connection,
    access: Access,
    args: &Value,
    changes: &mut Changes,
) -> Result<Applied> {
    let before = string(args, "before")?;
    let card = crate::load_card(conn, &before)?;
    access.ensure_writable(card.workspace)?;
    let (card_type, part, term, text) = crate::mcp::insert_request(root, args)?;
    crate::term::ensure_on_type(term.as_deref(), card_type)?;
    let plan = crate::place::insert_plan(conn, &card.address)?;
    access.ensure_link_rewrites_allowed(conn, &plan.mapping)?;
    let accepted = crate::place::apply_insert_plan_into(
        conn,
        &plan,
        card_type,
        part.as_ref(),
        term.as_deref(),
        &text,
        changes,
    )?;
    Ok(Applied {
        address: plan.location.clone(),
        lines: crate::place::insert_verification_lines(&plan),
        rewritten: plan.rewritten_links,
        created: Some(plan.location),
        mapping: plan.mapping,
        deleted: BTreeSet::new(),
        breaks: Vec::new(),
        accepted,
    })
}

fn delete_tx(conn: &Connection, access: Access, args: &Value, changes: &mut Changes) -> Result<Applied> {
    let at = string(args, "at")?;
    let card = crate::load_card(conn, &at)?;
    access.ensure_writable(card.workspace)?;
    let mut plan = crate::delete_plan(conn, &card.address)?;
    if let Some(into) = optional(args, "into") {
        plan = plan.into(conn, &into)?;
        access.ensure_link_rewrites_allowed(conn, &plan.repoint_mapping())?;
    }
    access.ensure_link_rewrites_allowed(conn, &plan.compaction())?;
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Delete(&plan))?;
    let lines = crate::delete_verification_lines(&plan);
    let breaks = if plan.into.is_some() {
        Vec::new()
    } else {
        plan.inbound.clone()
    };
    crate::apply_delete_plan_into(conn, &plan, changes)?;
    Ok(Applied {
        address: card.address,
        mapping: plan.compaction(),
        deleted: plan.delete_set.clone(),
        created: None,
        rewritten: if plan.into.is_some() { plan.inbound.len() } else { 0 },
        breaks,
        lines,
        accepted: crate::gate::Accepted::none(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{self, Outcome, Successor};
    use crate::{CardType, Workspace};
    use serde_json::json;

    /// Agent Topic `0/0` with `0/1`..`0/3`, a locked agent Topic `1/0` with `1/1`, and a human Topic
    /// `2/0` whose Card `2/1` links `0/2`.
    fn archive() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("tempdir");
        crate::initialize_database(temp.path()).expect("initialize");
        let root = temp.path();
        let owner = Access::Owner;
        ops::create_topic(root, owner, Workspace::Agent, "# 甲书\n").unwrap();
        ops::create_topic(root, owner, Workspace::Agent, "# 乙书\n").unwrap();
        ops::create_topic(root, owner, Workspace::Human, "# 人书\n").unwrap();
        for (at, text) in [("0/0", "# 一\n\n一。\n"), ("0/1", "# 二\n\n二。\n"), ("0/2", "# 三\n\n三。\n"), ("1/0", "# 壹\n\n壹。\n"), ("2/0", "# 人\n\n见 [[0/2]]。\n")] {
            ops::create_successor(root, owner, Successor::Direct, at, CardType::Idea, None, text).unwrap();
        }
        let mut conn = crate::open_db(root).unwrap();
        let plan = crate::tree_lock::plan(&conn, "1/0", None).unwrap();
        let tx = conn.transaction().unwrap();
        crate::tree_lock::lock_tx(&tx, &plan).unwrap();
        tx.commit().unwrap();
        temp
    }

    /// S4 (#232): a span move into a Locked tree answers the typed Lock refusal and moves nothing.
    #[test]
    fn a_span_move_into_a_locked_tree_is_refused() {
        let temp = archive();
        let root = temp.path();
        let error = ops::tree_tool(root, Access::Agent, "move_span", &json!({"from": "0/3", "side_of": "1/1"}), Some("move"))
            .err()
            .expect("refused");
        assert!(error.downcast_ref::<crate::tree_lock::Refusal>().is_some(), "{error:#}");
        let conn = crate::open_db(root).unwrap();
        assert!(crate::load_card(&conn, "0/3").unwrap().text.contains("# 三"));
    }

    /// S5 (#233): an agent relink never rewrites a human Card.
    #[test]
    fn an_agent_relink_refuses_a_human_source() {
        let temp = archive();
        let root = temp.path();
        let error = ops::tree_tool(root, Access::Agent, "relink", &json!({"from": "0/2", "to": "0/1"}), None)
            .err()
            .expect("refused");
        assert!(format!("{error:#}").contains("read-only"), "{error:#}");
        let owner = ops::tree_tool(root, Access::Owner, "relink", &json!({"from": "0/2", "to": "0/1"}), None).unwrap();
        let Outcome::Done(value) = owner else { panic!("relink applies at once") };
        assert_eq!(value["link_macros_rewritten"], 1, "{value}");
    }
}
