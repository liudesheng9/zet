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

use crate::{Card, CardType, MovePlan, model};

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

/// Apply an insert plan in one transaction: move the Cards down, then write the
/// new Card. Links in `text` name today's addresses; they follow the move too,
/// so a Link to the Card that moves down still reaches it.
pub(crate) fn apply_insert_plan(
    conn: &mut Connection,
    plan: &InsertPlan,
    card_type: CardType,
    part: Option<&model::Part>,
    text: &str,
) -> Result<()> {
    let workspace = crate::workspace::tree_workspace(conn, &plan.location)?;
    let facet = part.map(|part| &part.facet);
    let emphasis = part.and_then(|part| part.emphasis);
    model::ensure_allowed(conn, None, card_type, facet, emphasis)?;
    crate::validate_card_text(conn, &plan.location, text, None)?;
    let text = crate::replace_link_locations(text, &plan.mapping)?;
    let tx = conn.transaction()?;
    crate::apply_location_mapping_tx(&tx, &plan.mapping)?;
    tx.execute(
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis)
         VALUES(?1, NULL, 0, 0, NULL, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            plan.location,
            text,
            workspace.id(),
            card_type.id(),
            facet.map(model::Facet::id),
            emphasis.map(model::Emphasis::id)
        ],
    )?;
    crate::regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(())
}

fn all_locations(conn: &Connection) -> Result<BTreeSet<String>> {
    Ok(crate::load_cards(conn)?
        .into_iter()
        .map(|card: Card| card.address)
        .collect())
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
fn layout(
    locations: &BTreeSet<String>,
    at: &str,
    target: &Target,
    alone: bool,
) -> Result<BTreeMap<String, String>> {
    // 1. Detach.
    let moving = if alone {
        let sides = locations
            .iter()
            .filter(|location| crate::immediate_side_label(location, at).is_some())
            .count();
        if sides > 0 {
            bail!("`{at}` has Side successors: move them first, or move it with its subtree");
        }
        BTreeSet::from([at.to_string()])
    } else {
        subtree(locations, at)
    };
    let rest = locations
        .difference(&moving)
        .cloned()
        .collect::<BTreeSet<_>>();
    let close = match crate::direct_successor(at) {
        Ok(next) if alone && rest.contains(&next) => shift(&rest, &next, at)?,
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
}
