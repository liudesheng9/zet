//! Placing Cards between Cards: insert a new Card before an existing one, and
//! move a Card (with or without its successors) into another Card's place.
//!
//! A Location says where a Card stands in its chain, so putting a Card between
//! two Cards renumbers the Cards after it. Every plan here is one mapping from
//! old Locations to new ones, computed over the set of Locations in three steps:
//!
//! 1. **Detach.** The moving Card leaves its place: with its subtree, or alone,
//!    when its Direct successor chain moves up one place to close the gap.
//! 2. **Make room.** With `before`, the Card at that place and its subtree move
//!    down by the length of the incoming chain, so the last incoming Card's
//!    Direct successor is the Card that stood there.
//! 3. **Place.** The moving Cards take the freed place, each keeping its path
//!    from the moving Card.
//!
//! Each Card keeps its path from the root of the subtree that moves, so Side
//! successors move with the Card they branch from. Links, Reverse links, and
//! Spreads follow the mapping as in any move.

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet};

use crate::{CardType, MovePlan, model};

/// Where a moving Card goes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    /// A free Location: the next Direct or Side successor slot of a Card.
    To(String),
    /// The place of an existing Regular Card, which moves down to make room.
    Before(String),
}

/// A new Card at the place of `before`, and the Cards that move down for it.
pub(crate) struct InsertPlan {
    pub(crate) location: String,
    pub(crate) mapping: BTreeMap<String, String>,
    pub(crate) rewritten_links: usize,
}

/// Plan a move of the Card at `at` to `target`. Without `alone` the Card moves
/// with its successors; with it, only the Card moves and its Direct successor
/// chain closes the gap. Every address is today's address. A plain move to a
/// free Location keeps the rules of [`crate::move_plan`].
pub(crate) fn move_plan(
    conn: &Connection,
    at: &str,
    target: &Target,
    alone: bool,
) -> Result<MovePlan> {
    if let (Target::To(to), false) = (target, alone) {
        return crate::move_plan(conn, at, to);
    }
    let card = crate::load_card(conn, at)?;
    if card.is_lit {
        bail!("Literature Card cannot be moved; edit metadata to change its Citation key");
    }
    if card.is_topic {
        bail!("Topic Cards cannot be moved");
    }
    let destination = match target {
        Target::To(to) => to,
        Target::Before(before) => before,
    };
    if !crate::is_valid_regular_location(destination) {
        bail!("invalid move location `{destination}`");
    }
    crate::ensure_same_tree_kind_boundary(&card.address, destination)?;
    let locations = all_locations(conn)?;
    if let Target::Before(before) = target
        && !locations.contains(before)
    {
        bail!("card `{before}` does not exist");
    }
    crate::workspace::ensure_same_workspace(conn, &card.address, destination)?;
    let mapping = layout(&locations, &card.address, target, alone)?;
    if mapping.is_empty() {
        bail!("`{at}` is already there; nothing moves");
    }
    let rewritten_links = crate::count_rewritten_links(conn, &mapping)?;
    Ok(MovePlan {
        mapping,
        rewritten_links,
    })
}

/// Plan a new Card at the place of the Regular Card `before`. That Card and
/// its whole subtree move down one place, and it becomes the new Card's Direct
/// successor.
pub(crate) fn insert_plan(conn: &Connection, before: &str) -> Result<InsertPlan> {
    let card = crate::load_card(conn, before)?;
    if crate::is_tree_root(&card) {
        bail!("cannot insert before a tree root; insert before its Direct successor instead");
    }
    let locations = all_locations(conn)?;
    let mapping = make_room(&locations, &card.address, 1)?;
    let rewritten_links = crate::count_rewritten_links(conn, &mapping)?;
    Ok(InsertPlan {
        location: card.address,
        mapping,
        rewritten_links,
    })
}

/// Plan a new Card as an immediate Side successor of `before`'s parent, in `before`'s place: the
/// new Card takes that Side label, and that Card and every later Side child of the same parent move
/// one label on, each with its whole subtree. Unlike [`insert_plan`], nothing moves down a Direct
/// successor line, so a Term list keeps its rows as Side children in table order.
pub(crate) fn insert_side_plan(conn: &Connection, before: &str) -> Result<InsertPlan> {
    let card = crate::load_card(conn, before)?;
    let parent = crate::parent_location(&card.address).with_context(|| {
        format!("`{before}` is a Tree root; a Side successor needs a parent Card")
    })?;
    let label = crate::immediate_side_label(&card.address, &parent).with_context(|| {
        format!("`{before}` is not a Side successor of `{parent}`; name the Side child the new Card takes the place of")
    })?;
    let locations = all_locations(conn)?;
    let mapping = shift_later_siblings(&locations, &parent, crate::side_label_to_number(&label))?;
    let rewritten_links = crate::count_rewritten_links(conn, &mapping)?;
    Ok(InsertPlan {
        location: card.address,
        mapping,
        rewritten_links,
    })
}

/// The mapping that frees the Side label `label` of `parent`: every immediate Side child of
/// `parent` at that label or later moves one label on, each with its whole subtree.
fn shift_later_siblings(
    locations: &BTreeSet<String>,
    parent: &str,
    label: u64,
) -> Result<BTreeMap<String, String>> {
    let mut later: Vec<(u64, String)> = locations
        .iter()
        .filter(|location| crate::parent_location(location).as_deref() == Some(parent))
        .filter_map(|location| {
            crate::immediate_side_label(location, parent)
                .map(|label| (crate::side_label_to_number(&label), location.clone()))
        })
        .filter(|(number, _)| *number >= label)
        .collect();
    later.sort();
    let mut mapping = BTreeMap::new();
    for (number, location) in later {
        let to = format!("{parent}|{}", crate::number_to_side_label(number + 1));
        mapping.extend(shift(locations, &location, &to)?);
    }
    Ok(mapping)
}

pub(crate) fn insert_verification_lines(plan: &InsertPlan) -> Vec<String> {
    let mut lines = vec![
        "insert verification:".to_string(),
        format!("new card -> {}", plan.location),
    ];
    for (old, new) in &plan.mapping {
        lines.push(format!("{old} -> {new}"));
    }
    lines.push(format!("moved cards: {}", plan.mapping.len()));
    lines.push(format!("link macros rewritten: {}", plan.rewritten_links));
    lines
}

/// Apply an insert plan in one transaction: move the Cards, then write the new Card. Links in
/// `text` name today's addresses; they follow the move too, so a Link to a Card that moves still
/// reaches it.
pub(crate) fn apply_insert_plan(
    conn: &mut Connection,
    plan: &InsertPlan,
    card_type: CardType,
    part: Option<&model::Part>,
    term_name: Option<&str>,
    text: &str,
) -> Result<crate::gate::Accepted> {
    let tx = conn.transaction()?;
    let accepted = apply_insert_plan_tx(&tx, plan, card_type, part, term_name, text)?;
    tx.commit()?;
    Ok(accepted)
}

/// [`apply_insert_plan`] inside the caller's transaction, for an identified batch item: the mapping,
/// the new Card and the refreshed Link index commit together or not at all.
pub(crate) fn apply_insert_plan_tx(
    conn: &Connection,
    plan: &InsertPlan,
    card_type: CardType,
    part: Option<&model::Part>,
    term_name: Option<&str>,
    text: &str,
) -> Result<crate::gate::Accepted> {
    let mut changes = crate::link_index::Changes::default();
    let accepted = apply_insert_plan_into(conn, plan, card_type, part, term_name, text, &mut changes)?;
    changes.finish(conn)?;
    Ok(accepted)
}

/// The insert itself, gathering what it touches into `changes`, which the caller refreshes once: a
/// structural batch refreshes all its items together (S3, #231).
pub(crate) fn apply_insert_plan_into(
    conn: &Connection,
    plan: &InsertPlan,
    card_type: CardType,
    part: Option<&model::Part>,
    term_name: Option<&str>,
    text: &str,
    changes: &mut crate::link_index::Changes,
) -> Result<crate::gate::Accepted> {
    let workspace = crate::workspace::tree_workspace(conn, &plan.location)?;
    let facet = part.map(|part| &part.facet);
    let emphasis = part.and_then(|part| part.emphasis);
    crate::term::ensure_on_type(term_name, card_type)?;
    model::ensure_allowed(conn, None, card_type, facet, emphasis)?;
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Mapping(&plan.mapping))?;
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::CreateAt(&plan.location))?;
    // The Card that will be stored: Links to Cards that move are rewritten first, so the check reads
    // the exact committed text.
    let text = crate::replace_link_locations(text, &plan.mapping)?;
    let accepted = crate::gate::check_proposed(
        conn,
        &crate::gate::Candidate::create(
            &plan.location,
            crate::gate::Kind::Regular,
            workspace,
            &text,
            card_type,
            part,
        )
        .with_term(term_name),
    )?;
    crate::apply_location_mapping_tx(conn, &plan.mapping, changes)?;
    conn.execute(
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis, term_name)
         VALUES(?1, NULL, 0, 0, NULL, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            plan.location,
            text,
            workspace.id(),
            card_type.id(),
            facet.map(model::Facet::id),
            emphasis.map(model::Emphasis::id),
            term_name
        ],
    )?;
    changes.resync(conn, &plan.location)?;
    Ok(accepted)
}

fn all_locations(conn: &Connection) -> Result<BTreeSet<String>> {
    crate::addresses(conn)
}

/// `root` and every Location below it by parent Location, inside `locations`.
fn subtree(locations: &BTreeSet<String>, root: &str) -> BTreeSet<String> {
    locations
        .iter()
        .filter(|location| {
            let mut at = Some((*location).clone());
            while let Some(here) = at {
                if here == root {
                    return true;
                }
                at = crate::parent_location(&here);
            }
            false
        })
        .cloned()
        .collect()
}

/// Move the subtree at `from` (inside `locations`) so its root lands at `to`,
/// each Card keeping its path from the root.
fn shift(locations: &BTreeSet<String>, from: &str, to: &str) -> Result<BTreeMap<String, String>> {
    let mut mapping = BTreeMap::new();
    for location in subtree(locations, from) {
        let mut target = to.to_string();
        for edge in crate::path_edges(from, &location, locations)? {
            target = match edge {
                crate::Edge::Direct => crate::direct_successor(&target)?,
                crate::Edge::Side(label) => format!("{target}|{label}"),
            };
        }
        mapping.insert(location, target);
    }
    Ok(mapping)
}

/// Move the subtree at `at` down `steps` places along its Direct successor line.
fn make_room(
    locations: &BTreeSet<String>,
    at: &str,
    steps: usize,
) -> Result<BTreeMap<String, String>> {
    let mut to = at.to_string();
    for _ in 0..steps {
        to = crate::direct_successor(&to)?;
    }
    shift(locations, at, &to)
}

fn through<'a>(mapping: &'a BTreeMap<String, String>, location: &'a str) -> &'a str {
    mapping.get(location).map_or(location, String::as_str)
}

/// The whole mapping of a move between Cards, over the set of Locations. Only
/// Locations that change are listed.
/// Which Cards leave with the moving Card.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Detach<'a> {
    /// The Card and its whole subtree (every later Direct successor included).
    Subtree,
    /// The Card alone; it may have no Side successors, and its Direct successor chain closes the gap.
    Alone,
    /// The span from the Card through this later Card of its Direct chain, each with its Side
    /// successors; the Cards after the span close the gap (S4, #232).
    Span(&'a str),
}

fn layout(
    locations: &BTreeSet<String>,
    at: &str,
    target: &Target,
    alone: bool,
) -> Result<BTreeMap<String, String>> {
    layout_with(locations, at, target, if alone { Detach::Alone } else { Detach::Subtree })
}

/// The Cards of the span `from` through `through` (each with its Side successors), and the Card
/// after the span on its Direct chain, if one exists.
pub(crate) fn span_cards(
    locations: &BTreeSet<String>,
    from: &str,
    through: &str,
) -> Result<(BTreeSet<String>, Option<String>)> {
    if !crate::is_valid_regular_location(from) || !locations.contains(from) {
        bail!("a span starts at an existing Regular Card, not `{from}`");
    }
    let mut end = from.to_string();
    while end != through {
        end = crate::direct_successor(&end)?;
        if !locations.contains(&end) {
            bail!("span is not one Direct chain: `{through}` does not follow `{from}` on its Direct successor line");
        }
    }
    let next = crate::direct_successor(through)
        .ok()
        .filter(|next| locations.contains(next));
    let mut cards = subtree(locations, from);
    if let Some(next) = &next {
        for later in subtree(locations, next) {
            cards.remove(&later);
        }
    }
    Ok((cards, next))
}

/// Where `indent` puts a span starting at `at`: the next Side successor of the Card before it on
/// its Direct chain (S4, #232).
pub(crate) fn indent_target(locations: &BTreeSet<String>, at: &str) -> Result<Target> {
    let before = crate::parent_location(at)
        .filter(|_| crate::valid_positive_number(crate::last_segment(at)))
        .filter(|parent| crate::is_valid_regular_location(parent))
        .with_context(|| format!("nothing before `{at}` to indent under: it opens its chain"))?;
    Ok(Target::To(format!("{before}|{}", next_side_label(locations, &before))))
}

/// Where `outdent` puts a span that starts the Side branch `at`: right after the branch's parent,
/// whose old Direct successor moves down (S4, #232).
pub(crate) fn outdent_target(locations: &BTreeSet<String>, at: &str) -> Result<Target> {
    if !crate::valid_side_label(crate::last_segment(at)) {
        bail!("`{at}` does not start a Side branch, so it has no parent to outdent beside");
    }
    let parent = crate::parent_location(at).context("a Side branch has a parent")?;
    let next = crate::direct_successor(&parent)?;
    Ok(if locations.contains(&next) {
        Target::Before(next)
    } else {
        Target::To(next)
    })
}

/// The Cards a span delete removes, and the mapping that moves the rest of its chain up into the
/// span's place (S4, #232).
pub(crate) fn span_delete(
    locations: &BTreeSet<String>,
    from: &str,
    through: &str,
) -> Result<(BTreeSet<String>, BTreeMap<String, String>)> {
    let (cards, next) = span_cards(locations, from, through)?;
    let rest = locations.difference(&cards).cloned().collect::<BTreeSet<_>>();
    let close = match next {
        Some(next) => shift(&rest, &next, from)?,
        None => BTreeMap::new(),
    };
    Ok((cards, close))
}

/// Plan a span move: the Cards `from` through `through` with their Side successors go to
/// `target`, and the rest of their chain closes the gap (S4, #232).
pub(crate) fn span_plan(
    conn: &Connection,
    from: &str,
    through: &str,
    target: &Target,
) -> Result<MovePlan> {
    let card = crate::load_card(conn, from)?;
    if card.is_lit || card.is_topic {
        bail!("a span starts at a Regular Card, not the tree root `{from}`");
    }
    let destination = match target {
        Target::To(to) => to,
        Target::Before(before) => before,
    };
    if !crate::is_valid_regular_location(destination) {
        bail!("invalid move location `{destination}`");
    }
    crate::ensure_same_tree_kind_boundary(&card.address, destination)?;
    crate::workspace::ensure_same_workspace(conn, &card.address, destination)?;
    let locations = all_locations(conn)?;
    let mapping = layout_with(&locations, &card.address, target, Detach::Span(through))?;
    if mapping.is_empty() {
        bail!("`{from}` is already there; nothing moves");
    }
    let rewritten_links = crate::count_rewritten_links(conn, &mapping)?;
    Ok(MovePlan {
        mapping,
        rewritten_links,
    })
}

/// The refusal of a reorder whose list is not exactly the current children (S5, #233).
fn mismatch(listed: &BTreeSet<String>, current: &BTreeSet<String>) -> anyhow::Error {
    let missing = current.difference(listed).cloned().collect::<Vec<_>>();
    let extra = listed.difference(current).cloned().collect::<Vec<_>>();
    anyhow::anyhow!(
        "reorder_set_mismatch: list every current child exactly once; missing: [{}]; extra: [{}]",
        missing.join(", "),
        extra.join(", ")
    )
}

/// The mapping that gives one Location set's Cards their new places, checked for collisions.
fn finished(locations: &BTreeSet<String>, moves: BTreeMap<String, String>) -> Result<BTreeMap<String, String>> {
    let mut taken = BTreeSet::new();
    for location in locations {
        let new = moves.get(location).unwrap_or(location);
        if !taken.insert(new.clone()) {
            bail!("the reorder would put two Cards at `{new}`");
        }
    }
    Ok(moves.into_iter().filter(|(old, new)| old != new).collect())
}

/// Put the immediate Side successors of `parent` in the listed order, each with its whole branch
/// (S5, #233). The list names every current Side child exactly once.
pub(crate) fn reorder_side(
    locations: &BTreeSet<String>,
    parent: &str,
    order: &[String],
) -> Result<BTreeMap<String, String>> {
    let current = locations
        .iter()
        .filter(|location| crate::parent_location(location).as_deref() == Some(parent))
        .filter(|location| crate::immediate_side_label(location, parent).is_some())
        .cloned()
        .collect::<BTreeSet<_>>();
    let listed = order.iter().cloned().collect::<BTreeSet<_>>();
    if listed.len() != order.len() || listed != current {
        return Err(mismatch(&listed, &current));
    }
    let mut moves = BTreeMap::new();
    for (index, child) in order.iter().enumerate() {
        let to = format!("{parent}|{}", crate::number_to_side_label(index as u64 + 1));
        moves.extend(shift(locations, child, &to)?);
    }
    finished(locations, moves)
}

/// Put the spans of the Direct chain after `parent` in the listed order, each Card with its Side
/// successors (S5, #233). The spans cover the whole chain, each Card once.
pub(crate) fn reorder_direct(
    locations: &BTreeSet<String>,
    parent: &str,
    spans: &[(String, String)],
) -> Result<BTreeMap<String, String>> {
    let mut chain = Vec::new();
    let mut next = crate::direct_successor(parent)?;
    while locations.contains(&next) {
        chain.push(next.clone());
        next = crate::direct_successor(&next)?;
    }
    let current = chain.iter().cloned().collect::<BTreeSet<_>>();
    let mut listed = Vec::new();
    for (from, through) in spans {
        if !current.contains(from) || !current.contains(through) {
            listed.push(from.clone());
            listed.push(through.clone());
            continue;
        }
        let mut at = from.clone();
        listed.push(at.clone());
        while at != *through {
            at = crate::direct_successor(&at)?;
            if !current.contains(&at) {
                bail!("span is not one Direct chain: `{through}` does not follow `{from}`");
            }
            listed.push(at.clone());
        }
    }
    let listed_set = listed.iter().cloned().collect::<BTreeSet<_>>();
    if listed_set.len() != listed.len() || listed_set != current {
        return Err(mismatch(&listed_set, &current));
    }
    let mut moves = BTreeMap::new();
    let mut place = crate::direct_successor(parent)?;
    for card in &listed {
        // The Card with its Side successors, without the Cards after it on the chain.
        let mut branch = subtree(locations, card);
        if let Ok(after) = crate::direct_successor(card) {
            for later in subtree(locations, &after) {
                branch.remove(&later);
            }
        }
        moves.extend(shift(&branch, card, &place)?);
        place = crate::direct_successor(&place)?;
    }
    finished(locations, moves)
}

fn layout_with(
    locations: &BTreeSet<String>,
    at: &str,
    target: &Target,
    detach: Detach<'_>,
) -> Result<BTreeMap<String, String>> {
    // 1. Detach.
    let (moving, next) = match detach {
        Detach::Alone => {
            let sides = locations
                .iter()
                .filter(|location| crate::immediate_side_label(location, at).is_some())
                .count();
            if sides > 0 {
                bail!("`{at}` has Side successors: move them first, or move it with its subtree");
            }
            let next = crate::direct_successor(at).ok();
            (BTreeSet::from([at.to_string()]), next)
        }
        Detach::Subtree => (subtree(locations, at), None),
        Detach::Span(through) => span_cards(locations, at, through)?,
    };
    let alone = matches!(detach, Detach::Alone);
    let rest = locations
        .difference(&moving)
        .cloned()
        .collect::<BTreeSet<_>>();
    let close = match next {
        Some(next) if rest.contains(&next) => shift(&rest, &next, at)?,
        _ => BTreeMap::new(),
    };
    let detached = rest
        .iter()
        .map(|location| through(&close, location).to_string())
        .collect::<BTreeSet<_>>();

    // How many places the moving Cards take on their Direct successor line.
    let mut length = 1;
    let mut end = at.to_string();
    while let Ok(next) = crate::direct_successor(&end) {
        if !moving.contains(&next) {
            break;
        }
        length += 1;
        end = next;
    }

    // 2. Make room, and find the place.
    let (place, room) = match target {
        Target::Before(before) => {
            if moving.contains(before) {
                bail!("cannot move a card before one of its own successors");
            }
            if !locations.contains(before) {
                bail!("card `{before}` does not exist");
            }
            let place = through(&close, before).to_string();
            let room = make_room(&detached, &place, length)?;
            (place, room)
        }
        Target::To(to) => {
            let parent = crate::parent_location(to).context("move target has no parent")?;
            if moving.contains(&parent) {
                bail!("cannot move a card into its own successors");
            }
            if !locations.contains(&parent) {
                bail!("move target parent does not exist outside the moved subtree");
            }
            let last = crate::last_segment(to);
            let new_parent = through(&close, &parent).to_string();
            let place = if crate::valid_side_label(last) {
                let next = next_side_label(&rest, &parent);
                if format!("{parent}|{next}") != *to {
                    bail!(
                        "side successor move target must be the next available label: {parent}|{next}"
                    );
                }
                format!("{new_parent}|{next}")
            } else {
                if crate::direct_successor(&parent)? != *to {
                    bail!("move target does not preserve direct successor rules");
                }
                let place = crate::direct_successor(&new_parent)?;
                if detached.contains(&place) {
                    bail!(
                        "destination locations already exist: {place}; use `before` to move into an occupied place"
                    );
                }
                place
            };
            (place, BTreeMap::new())
        }
    };

    // 3. Place.
    let placed = if alone {
        BTreeMap::from([(at.to_string(), place)])
    } else {
        shift(&moving, at, &place)?
    };

    let mut mapping = BTreeMap::new();
    let mut taken = BTreeSet::new();
    for location in locations {
        let new = match placed.get(location) {
            Some(new) => new.clone(),
            None => {
                let detached = through(&close, location);
                through(&room, detached).to_string()
            }
        };
        if new != *location && !crate::is_valid_regular_location(&new) {
            bail!("the move would make an invalid location `{new}`");
        }
        if !taken.insert(new.clone()) {
            bail!("the move would put two Cards at `{new}`");
        }
        if new != *location {
            mapping.insert(location.clone(), new);
        }
    }
    Ok(mapping)
}

/// The next free Side successor label of `parent` among `locations`.
fn next_side_label(locations: &BTreeSet<String>, parent: &str) -> String {
    let max = locations
        .iter()
        .filter_map(|location| crate::immediate_side_label(location, parent))
        .map(|label| crate::side_label_to_number(&label))
        .max()
        .unwrap_or(0);
    crate::number_to_side_label(max + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(locations: &[&str]) -> BTreeSet<String> {
        locations.iter().map(|at| at.to_string()).collect()
    }

    fn pairs(mapping: &BTreeMap<String, String>) -> Vec<(&str, &str)> {
        mapping
            .iter()
            .map(|(old, new)| (old.as_str(), new.as_str()))
            .collect()
    }

    /// Apply a mapping to a set of Locations.
    fn after(locations: &BTreeSet<String>, mapping: &BTreeMap<String, String>) -> Vec<String> {
        let mut out = locations
            .iter()
            .map(|at| through(mapping, at).to_string())
            .collect::<Vec<_>>();
        out.sort_by(|a, b| model::address_order(a, b));
        out
    }

    #[test]
    fn making_room_moves_a_chain_and_its_branches_down_one_place() {
        let locations = set(&["3/0", "3/1", "3/2", "3/2|a", "3/3", "3/3|a", "3/3|a|1"]);
        let room = make_room(&locations, "3/2", 1).unwrap();
        assert_eq!(
            pairs(&room),
            [
                ("3/2", "3/3"),
                ("3/2|a", "3/3|a"),
                ("3/3", "3/4"),
                ("3/3|a", "3/4|a"),
                ("3/3|a|1", "3/4|a|1")
            ]
        );
        // Before a Side successor, the Card becomes the first Card of the new Card's line.
        let locations = set(&["3/0", "3/1", "3/1|a", "3/1|a|1", "3/1|a|b", "3/1|a|1|a"]);
        let room = make_room(&locations, "3/1|a", 1).unwrap();
        assert_eq!(
            pairs(&room),
            [
                ("3/1|a", "3/1|a|1"),
                ("3/1|a|1", "3/1|a|2"),
                ("3/1|a|1|a", "3/1|a|2|a"),
                ("3/1|a|b", "3/1|a|1|b")
            ]
        );
    }

    #[test]
    fn a_side_placement_shifts_the_named_sibling_and_every_later_one_one_label() {
        let locations = set(&[
            "3/0",
            "3/1",
            "3/1|a",
            "3/1|a|a",
            "3/1|a|a|1",
            "3/1|a|a|1|a",
            "3/1|a|b",
            "3/1|a|b|1",
            "3/1|a|c",
            "3/1|a|c|a",
        ]);
        // Before `3/1|a|b`: the new Card takes |b, and |b and |c each move one label on.
        let mapping = shift_later_siblings(&locations, "3/1|a", 2).unwrap();
        assert_eq!(
            pairs(&mapping),
            [
                ("3/1|a|b", "3/1|a|c"),
                ("3/1|a|b|1", "3/1|a|c|1"),
                ("3/1|a|c", "3/1|a|d"),
                ("3/1|a|c|a", "3/1|a|d|a")
            ]
        );
        // No two Cards land on one Location, and |a stays exactly where it was.
        assert_eq!(
            after(&locations, &mapping),
            [
                "3/0",
                "3/1",
                "3/1|a",
                "3/1|a|a",
                "3/1|a|a|1",
                "3/1|a|a|1|a",
                "3/1|a|c",
                "3/1|a|c|1",
                "3/1|a|d",
                "3/1|a|d|a"
            ]
        );
        assert!(!mapping.contains_key("3/1|a|a") && !mapping.contains_key("3/1|a|a|1"));
        // Before the last sibling only that sibling and its subtree move.
        let mapping = shift_later_siblings(&locations, "3/1|a", 3).unwrap();
        assert_eq!(
            pairs(&mapping),
            [("3/1|a|c", "3/1|a|d"), ("3/1|a|c|a", "3/1|a|d|a")]
        );
        // A parent with no Side child at or after the label has nothing to move.
        assert!(
            shift_later_siblings(&locations, "3/1|a", 4)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_card_moves_alone_beside_the_step_it_illustrates() {
        // 11|b|a|1 … |5: take |4 off the line and file it as a Side successor of |2.
        let locations = set(&[
            "Hart1995/11", "Hart1995/11|b", "Hart1995/11|b|a", "Hart1995/11|b|a|1", "Hart1995/11|b|a|2", "Hart1995/11|b|a|3",
            "Hart1995/11|b|a|4", "Hart1995/11|b|a|5", "Hart1995/11|b|a|5|a",
        ]);
        let mapping = layout(
            &locations,
            "Hart1995/11|b|a|4",
            &Target::To("Hart1995/11|b|a|2|a".into()),
            true,
        )
        .unwrap();
        assert_eq!(
            pairs(&mapping),
            [
                ("Hart1995/11|b|a|4", "Hart1995/11|b|a|2|a"),
                ("Hart1995/11|b|a|5", "Hart1995/11|b|a|4"),
                ("Hart1995/11|b|a|5|a", "Hart1995/11|b|a|4|a")
            ]
        );
        // A Card that moves alone may name a place after it, in today's addresses.
        let mapping = layout(
            &locations,
            "Hart1995/11|b|a|2",
            &Target::To("Hart1995/11|b|a|4|a".into()),
            true,
        )
        .unwrap();
        assert_eq!(
            after(&locations, &mapping),
            [
                "Hart1995/11", "Hart1995/11|b", "Hart1995/11|b|a", "Hart1995/11|b|a|1", "Hart1995/11|b|a|2", "Hart1995/11|b|a|3",
                "Hart1995/11|b|a|3|a", "Hart1995/11|b|a|4", "Hart1995/11|b|a|4|a"
            ]
        );
        assert_eq!(mapping["Hart1995/11|b|a|2"], "Hart1995/11|b|a|3|a");
        assert_eq!(mapping["Hart1995/11|b|a|4"], "Hart1995/11|b|a|3");
    }

    #[test]
    fn before_reorders_one_step_of_a_chain() {
        let locations = set(&["3/0", "3/1", "3/2", "3/3", "3/4", "3/4|a"]);
        // Move 3/4 alone before 3/2: 3/4 -> 3/2, 3/2 -> 3/3, 3/3 -> 3/4, the branch follows 3/4.
        let mapping = layout(&locations, "3/4", &Target::Before("3/2".into()), false).unwrap();
        assert_eq!(
            pairs(&mapping),
            [("3/2", "3/3"), ("3/3", "3/4"), ("3/4", "3/2"), ("3/4|a", "3/2|a")]
        );
        // Move 3/2 alone before 3/4: the chain closes up, then opens at 3/4's new place.
        let mapping = layout(&locations, "3/2", &Target::Before("3/4".into()), true).unwrap();
        assert_eq!(pairs(&mapping), [("3/2", "3/3"), ("3/3", "3/2")]);
        // A Card cannot go before its own successor, except alone.
        assert_eq!(
            layout(&locations, "3/2", &Target::Before("3/4".into()), false)
                .unwrap_err()
                .to_string(),
            "cannot move a card before one of its own successors"
        );
        // Moving alone before its own Direct successor changes nothing.
        assert!(
            layout(&locations, "3/2", &Target::Before("3/3".into()), true)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_chain_moved_before_a_card_is_spliced_in_whole() {
        let locations = set(&["3/0", "3/1", "3/2", "3/1|a", "3/1|a|1", "3/1|a|1|a"]);
        let mapping = layout(&locations, "3/1|a", &Target::Before("3/2".into()), false).unwrap();
        assert_eq!(
            pairs(&mapping),
            [
                ("3/1|a", "3/2"),
                ("3/1|a|1", "3/3"),
                ("3/1|a|1|a", "3/3|a"),
                ("3/2", "3/4")
            ]
        );
    }

    #[test]
    fn alone_refuses_a_card_with_side_successors_and_to_keeps_the_slot_rules() {
        let locations = set(&["3/0", "3/1", "3/2", "3/2|a", "3/3", "3/1|a"]);
        assert_eq!(
            layout(&locations, "3/2", &Target::To("3/1|b".into()), true)
                .unwrap_err()
                .to_string(),
            "`3/2` has Side successors: move them first, or move it with its subtree"
        );
        assert_eq!(
            layout(&locations, "3/3", &Target::To("3/1|c".into()), true)
                .unwrap_err()
                .to_string(),
            "side successor move target must be the next available label: 3/1|b"
        );
        assert!(
            layout(&locations, "3/1|a", &Target::To("3/3".into()), true)
                .unwrap_err()
                .to_string()
                .starts_with("destination locations already exist: 3/3")
        );
    }
    /// S4 (#232): a span from the middle of a chain moves with its Cards' Side successors, and the
    /// Cards after it close the gap; the destination is named on today's Locations.
    #[test]
    fn a_span_moves_with_its_side_branches_and_the_chain_closes_up() {
        let locations = set(&["3/0", "3/1", "3/2", "3/2|a", "3/3", "3/4", "3/4|a", "3/5"]);
        let mapping = layout_with(&locations, "3/2", &Target::To("3/5|a".into()), Detach::Span("3/3")).unwrap();
        assert_eq!(
            pairs(&mapping),
            [
                ("3/2", "3/3|a"),
                ("3/2|a", "3/3|a|a"),
                ("3/3", "3/3|a|1"),
                ("3/4", "3/2"),
                ("3/4|a", "3/2|a"),
                ("3/5", "3/3"),
            ]
        );
    }

    /// S4 (#232): the span of one Card is the Card with its Side successors; `before` takes the place
    /// of a later Card in the same chain.
    #[test]
    fn the_span_of_one_card_keeps_its_branch_and_takes_a_later_place() {
        let locations = set(&["3/0", "3/1", "3/1|a", "3/2", "3/3"]);
        let mapping = layout_with(&locations, "3/1", &Target::Before("3/3".into()), Detach::Span("3/1")).unwrap();
        assert_eq!(pairs(&mapping), [("3/1", "3/2"), ("3/1|a", "3/2|a"), ("3/2", "3/1")]);
    }

    /// S4 (#232): `through` must be reached from `from` along one Direct chain.
    #[test]
    fn a_span_is_one_direct_chain() {
        let locations = set(&["3/0", "3/1", "3/1|a", "3/2", "3/3"]);
        let error = layout_with(&locations, "3/1", &Target::To("3/3|a".into()), Detach::Span("3/1|a"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("span is not one Direct chain"), "{error}");
        let error = layout_with(&locations, "3/2", &Target::To("3/3|a".into()), Detach::Span("3/1"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("span is not one Direct chain"), "{error}");
    }

    /// S4 (#232): indent makes a span the last Side branch of the Card before it; outdent moves a
    /// span that starts a Side branch after the branch's parent. Indent then outdent restores every
    /// Location.
    #[test]
    fn indent_then_outdent_restores_the_tree() {
        let locations = set(&["3/0", "3/1", "3/2", "3/2|a", "3/3"]);
        let indent = layout_with(&locations, "3/2", &indent_target(&locations, "3/2").unwrap(), Detach::Span("3/2")).unwrap();
        assert_eq!(pairs(&indent), [("3/2", "3/1|a"), ("3/2|a", "3/1|a|a"), ("3/3", "3/2")]);
        let indented = after(&locations, &indent).into_iter().collect::<BTreeSet<_>>();
        let outdent = layout_with(&indented, "3/1|a", &outdent_target(&indented, "3/1|a").unwrap(), Detach::Span("3/1|a")).unwrap();
        assert_eq!(pairs(&outdent), [("3/1|a", "3/2"), ("3/1|a|a", "3/2|a"), ("3/2", "3/3")]);
        assert_eq!(after(&indented, &outdent), after(&locations, &BTreeMap::new()));
        // A Card that opens its chain has nothing before it to indent under.
        let error = indent_target(&locations, "3/1").unwrap_err().to_string();
        assert!(error.contains("nothing before `3/1` to indent under"), "{error}");
        let error = outdent_target(&locations, "3/2").unwrap_err().to_string();
        assert!(error.contains("does not start a Side branch"), "{error}");
    }

    /// S4 (#232): outdent keeps the rest of the branch on the parent: the Card after the span takes
    /// the branch's place.
    #[test]
    fn outdent_leaves_the_rest_of_the_branch_in_place() {
        let locations = set(&["3/0", "3/1", "3/1|a", "3/1|a|1", "3/1|b", "3/2"]);
        let mapping = layout_with(&locations, "3/1|a", &outdent_target(&locations, "3/1|a").unwrap(), Detach::Span("3/1|a")).unwrap();
        assert_eq!(pairs(&mapping), [("3/1|a", "3/2"), ("3/1|a|1", "3/1|a"), ("3/2", "3/3")]);
    }

    /// S4 (#232): deleting a span removes its Cards with their Side successors and moves the rest of
    /// the chain up into its place.
    #[test]
    fn deleting_a_span_closes_the_chain() {
        let locations = set(&["3/0", "3/1", "3/2", "3/2|a", "3/3", "3/4", "3/4|a"]);
        let (gone, close) = span_delete(&locations, "3/2", "3/3").unwrap();
        assert_eq!(gone.into_iter().collect::<Vec<_>>(), ["3/2", "3/2|a", "3/3"]);
        assert_eq!(pairs(&close), [("3/4", "3/2"), ("3/4|a", "3/2|a")]);
    }
    /// S5 (#233): reordering the Side successors of a Card moves each with its whole branch.
    #[test]
    fn reordering_side_successors_moves_each_with_its_branch() {
        let locations = set(&["3/0", "3/1", "3/1|a", "3/1|a|1", "3/1|b", "3/1|c", "3/1|c|a", "3/1|d", "3/2"]);
        let order = ["3/1|d", "3/1|c", "3/1|b", "3/1|a"].map(String::from);
        let mapping = reorder_side(&locations, "3/1", &order).unwrap();
        assert_eq!(
            pairs(&mapping),
            [
                ("3/1|a", "3/1|d"),
                ("3/1|a|1", "3/1|d|1"),
                ("3/1|b", "3/1|c"),
                ("3/1|c", "3/1|b"),
                ("3/1|c|a", "3/1|b|a"),
                ("3/1|d", "3/1|a"),
            ]
        );
    }

    /// S5 (#233): a reorder list names every current child exactly once.
    #[test]
    fn a_reorder_list_must_name_every_child_once() {
        let locations = set(&["3/0", "3/1", "3/1|a", "3/1|b", "3/1|c"]);
        let missing = reorder_side(&locations, "3/1", &["3/1|b", "3/1|a"].map(String::from)).unwrap_err().to_string();
        assert!(missing.contains("reorder_set_mismatch") && missing.contains("missing: [3/1|c]"), "{missing}");
        let extra = reorder_side(&locations, "3/1", &["3/1|c", "3/1|b", "3/1|a", "3/2"].map(String::from)).unwrap_err().to_string();
        assert!(extra.contains("extra: [3/2]"), "{extra}");
        let twice = reorder_side(&locations, "3/1", &["3/1|c", "3/1|c", "3/1|b", "3/1|a"].map(String::from)).unwrap_err().to_string();
        assert!(twice.contains("reorder_set_mismatch"), "{twice}");
    }

    /// S5 (#233): reordering the spans of a Direct chain keeps each Card's Side successors with it.
    #[test]
    fn reordering_the_spans_of_a_chain_keeps_each_cards_branch() {
        let locations = set(&["3/0", "3/1", "3/2", "3/2|a", "3/3", "3/4", "3/4|a"]);
        let spans = [("3/3", "3/4"), ("3/1", "3/2")].map(|(a, b)| (a.to_string(), b.to_string()));
        let mapping = reorder_direct(&locations, "3/0", &spans).unwrap();
        assert_eq!(
            pairs(&mapping),
            [
                ("3/1", "3/3"),
                ("3/2", "3/4"),
                ("3/2|a", "3/4|a"),
                ("3/3", "3/1"),
                ("3/4", "3/2"),
                ("3/4|a", "3/2|a"),
            ]
        );
        let short = [("3/3", "3/4"), ("3/1", "3/1")].map(|(a, b)| (a.to_string(), b.to_string()));
        let error = reorder_direct(&locations, "3/0", &short).unwrap_err().to_string();
        assert!(error.contains("missing: [3/2]"), "{error}");
    }
}
