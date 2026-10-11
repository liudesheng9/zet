//! Model structure: one Model Card and the Model parts filed under it.
//!
//! A whole model is presented by its Model Card (the head: question, overview,
//! structure chart) and by Model parts: Model Cards that each carry a Model
//! facet (setting, assumptions, timing, dynamics, solution, results,
//! intuition, limits) and are filed below the head, as its successors or as
//! successors of other parts. The facet is stored on the Card row like the
//! Card type; the structure is computed from Locations and facets, never
//! stored, so it follows the Cards when they move.

use anyhow::{Result, bail};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::link::LinkBody;
use crate::{Card, CardType};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum Facet {
    Setting,
    Assumptions,
    Timing,
    Dynamics,
    Solution,
    Results,
    Intuition,
    Limits,
}

impl Facet {
    /// Every facet, in the order a model is read.
    pub(crate) const ALL: [Facet; 8] = [
        Facet::Setting,
        Facet::Assumptions,
        Facet::Timing,
        Facet::Dynamics,
        Facet::Solution,
        Facet::Results,
        Facet::Intuition,
        Facet::Limits,
    ];

    /// The stored and typed name.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Facet::Setting => "setting",
            Facet::Assumptions => "assumptions",
            Facet::Timing => "timing",
            Facet::Dynamics => "dynamics",
            Facet::Solution => "solution",
            Facet::Results => "results",
            Facet::Intuition => "intuition",
            Facet::Limits => "limits",
        }
    }

    /// What a part with this facet holds.
    pub(crate) fn note(self) -> &'static str {
        match self {
            Facet::Setting => "agents, assets, actions, and payoffs",
            Facet::Assumptions => "what the model assumes, and what each assumption buys",
            Facet::Timing => "the order of moves and who knows what when",
            Facet::Dynamics => "how the state evolves: laws of motion, best responses, feedback",
            Facet::Solution => "the equilibrium or optimum and how it is derived",
            Facet::Results => "propositions and comparative statics",
            Facet::Intuition => "why the results hold, in words",
            Facet::Limits => "where the model stops applying, and its critics",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        let value = value.trim().to_ascii_lowercase();
        Facet::ALL
            .into_iter()
            .find(|facet| facet.id() == value)
            .ok_or_else(|| {
                anyhow::anyhow!("unknown model facet `{value}`; expected {}", Self::names())
            })
    }

    /// `setting, assumptions, …, or limits`.
    pub(crate) fn names() -> String {
        let ids = Facet::ALL.map(Facet::id);
        format!(
            "{}, or {}",
            ids[..ids.len() - 1].join(", "),
            ids[ids.len() - 1]
        )
    }

    /// Read a stored value; anything unexpected is no facet.
    pub(crate) fn from_stored(value: Option<&str>) -> Option<Self> {
        value.and_then(|value| Facet::parse(value).ok())
    }

    fn rank(self) -> usize {
        Facet::ALL
            .iter()
            .position(|facet| *facet == self)
            .unwrap_or(0)
    }
}

impl std::fmt::Display for Facet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Parse an optional facet argument: an empty string clears the facet.
pub(crate) fn parse_optional(value: &str) -> Result<Option<Facet>> {
    if value.trim().is_empty() || value.trim().eq_ignore_ascii_case("none") {
        Ok(None)
    } else {
        Facet::parse(value).map(Some)
    }
}

/// The Model Card a Card filed at `start` belongs to: walk up through Model
/// parts until a Model Card without a facet. `lookup` gives the type and facet
/// of the Card at an address.
pub(crate) fn find_head(
    start: Option<String>,
    lookup: impl Fn(&str) -> Option<(CardType, Option<Facet>)>,
) -> Option<String> {
    let mut at = start;
    while let Some(address) = at {
        match lookup(&address)? {
            (CardType::Model, None) => return Some(address),
            (CardType::Model, Some(_)) => at = crate::parent_location(&address),
            _ => return None,
        }
    }
    None
}

fn db_lookup(conn: &Connection) -> impl Fn(&str) -> Option<(CardType, Option<Facet>)> + '_ {
    |address| {
        crate::load_card(conn, address)
            .ok()
            .map(|card| (card.card_type, card.facet))
    }
}

/// Check the type and facet a Card at `location` would have. A facet needs a
/// Model Card filed below another Model Card; a Model Card with parts under
/// it cannot stop being their head.
pub(crate) fn ensure_allowed(
    conn: &Connection,
    location: &str,
    current: Option<&Card>,
    card_type: CardType,
    facet: Option<Facet>,
) -> Result<()> {
    if let Some(facet) = facet {
        if card_type != CardType::Model {
            bail!("only Model Cards can have a Model facet");
        }
        if find_head(crate::parent_location(location), db_lookup(conn)).is_none() {
            bail!(
                "a Model part (`{facet}`) must be filed under its Model Card: create it as a successor of the Model Card or of one of its parts"
            );
        }
    }
    if let Some(card) = current
        && card.card_type == CardType::Model
        && card.facet.is_none()
        && (card_type != CardType::Model || facet.is_some())
    {
        let parts = parts_in_db(conn, &card.address)?;
        if !parts.is_empty() {
            bail!(
                "`{}` is the Model Card of {} Model part(s) ({}); change or move them first",
                card.address,
                parts.len(),
                parts.join(", ")
            );
        }
    }
    Ok(())
}

fn parts_in_db(conn: &Connection, head: &str) -> Result<Vec<String>> {
    let cards = crate::load_cards(conn)?;
    let by_address: BTreeMap<&str, &Card> = cards
        .iter()
        .map(|card| (card.address.as_str(), card))
        .collect();
    let lookup = |address: &str| {
        by_address
            .get(address)
            .map(|card| (card.card_type, card.facet))
    };
    Ok(cards
        .iter()
        .filter(|card| card.facet.is_some())
        .filter(|card| {
            find_head(crate::parent_location(&card.address), lookup).as_deref() == Some(head)
        })
        .map(|card| card.address.clone())
        .collect())
}

/// Store a Card's type and facet.
pub(crate) fn set(
    conn: &Connection,
    card: &Card,
    card_type: CardType,
    facet: Option<Facet>,
) -> Result<()> {
    crate::card_type::ensure_allowed(card, card_type)?;
    ensure_allowed(conn, &card.address, Some(card), card_type, facet)?;
    conn.execute(
        "UPDATE cards SET card_type = ?1, model_facet = ?2 WHERE row_id = ?3",
        params![card_type.id(), facet.map(Facet::id), card.row_id],
    )?;
    Ok(())
}

/// The structure of the model a Card belongs to, computed over a set of
/// Cards. `None` when the Card is neither a Model Card nor a Model part.
pub(crate) fn structure(
    address: &str,
    by_address: &BTreeMap<String, Card>,
    parsed: &BTreeMap<String, crate::ParsedCard>,
    inbound: &BTreeMap<String, BTreeSet<String>>,
) -> Option<Value> {
    let card = by_address.get(address)?;
    if card.card_type != CardType::Model {
        return None;
    }
    let lookup = |at: &str| by_address.get(at).map(|card| (card.card_type, card.facet));
    let head = match card.facet {
        None => Some(card.address.clone()),
        Some(_) => find_head(crate::parent_location(&card.address), lookup),
    };
    let title = |at: &str| {
        parsed
            .get(at)
            .map(|parsed| parsed.title.clone())
            .unwrap_or_default()
    };
    let links_of = |at: &str| -> BTreeSet<String> {
        parsed
            .get(at)
            .and_then(|parsed| LinkBody::parse(&parsed.body).ok())
            .map(|body| {
                body.occurrences()
                    .iter()
                    .map(|occurrence| occurrence.target.to_string())
                    .collect()
            })
            .unwrap_or_default()
    };
    let Some(head) = head else {
        return Some(json!({
            "head": null,
            "facet": card.facet.map(Facet::id),
            "parts": [],
            "missing": [],
            "issues": [format!(
                "`{}` is a Model part ({}) but is not filed under a Model Card",
                card.address,
                card.facet.map(Facet::id).unwrap_or_default()
            )],
        }));
    };
    let tree = crate::tree_id(&head);
    let mut parts = by_address
        .values()
        .filter(|other| other.facet.is_some() && crate::is_tree_member(&other.address, tree))
        .filter(|other| {
            find_head(crate::parent_location(&other.address), lookup).as_deref()
                == Some(head.as_str())
        })
        .collect::<Vec<_>>();
    parts.sort_by(|a, b| {
        (a.facet.map(Facet::rank), &a.address).cmp(&(b.facet.map(Facet::rank), &b.address))
    });
    let members = std::iter::once(head.clone())
        .chain(parts.iter().map(|part| part.address.clone()))
        .collect::<BTreeSet<_>>();
    let head_links = links_of(&head);
    let mut issues = Vec::new();
    let part_values = parts
        .iter()
        .map(|part| {
            let links = links_of(&part.address);
            if !head_links.contains(&part.address) {
                issues.push(format!(
                    "the Model Card `{head}` does not link its part `{}`",
                    part.address
                ));
            }
            if !links.contains(&head) {
                issues.push(format!(
                    "the Model part `{}` does not link its Model Card `{head}`",
                    part.address
                ));
            }
            json!({
                "address": part.address,
                "title": title(&part.address),
                "facet": part.facet.map(Facet::id),
                "links": links
                    .iter()
                    .filter(|target| members.contains(*target) && **target != part.address)
                    .collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let present = parts
        .iter()
        .filter_map(|part| part.facet)
        .collect::<BTreeSet<_>>();
    let missing = Facet::ALL
        .into_iter()
        .filter(|facet| !present.contains(facet))
        .map(Facet::id)
        .collect::<Vec<_>>();
    // Cards outside the model that link any of its Cards: its discussion.
    let discussion = members
        .iter()
        .flat_map(|member| inbound.get(member).into_iter().flatten())
        .filter(|source| !members.contains(*source))
        .collect::<BTreeSet<_>>();
    Some(json!({
        "head": { "address": head, "title": title(&head) },
        "facet": card.facet.map(Facet::id),
        "head_links": head_links
            .iter()
            .filter(|target| members.contains(*target) && **target != head)
            .collect::<Vec<_>>(),
        "parts": part_values,
        "missing": missing,
        "discussion": discussion.len(),
        "issues": issues,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facet_names_parse_and_list_in_reading_order() {
        for facet in Facet::ALL {
            assert_eq!(Facet::parse(facet.id()).unwrap(), facet);
        }
        assert_eq!(Facet::parse(" Dynamics ").unwrap(), Facet::Dynamics);
        assert_eq!(
            Facet::parse("proof").unwrap_err().to_string(),
            "unknown model facet `proof`; expected setting, assumptions, timing, dynamics, solution, results, intuition, or limits"
        );
        assert_eq!(Facet::from_stored(Some("bogus")), None);
        assert_eq!(Facet::from_stored(None), None);
        assert_eq!(parse_optional("").unwrap(), None);
        assert_eq!(parse_optional("none").unwrap(), None);
        assert_eq!(parse_optional("timing").unwrap(), Some(Facet::Timing));
    }

    #[test]
    fn the_head_is_the_nearest_model_card_above_a_chain_of_parts() {
        let cards: BTreeMap<&str, (CardType, Option<Facet>)> = [
            ("3/1", (CardType::Idea, None)),
            ("3/2", (CardType::Model, None)),
            ("3/2|a", (CardType::Model, Some(Facet::Setting))),
            ("3/2|a|1", (CardType::Model, Some(Facet::Assumptions))),
            ("3/3", (CardType::Model, Some(Facet::Results))),
            ("3/1|a", (CardType::Model, Some(Facet::Limits))),
        ]
        .into_iter()
        .collect();
        let lookup = |at: &str| cards.get(at).copied();
        let head = |at: &str| find_head(crate::parent_location(at), lookup);
        assert_eq!(head("3/2|a").as_deref(), Some("3/2"));
        assert_eq!(head("3/2|a|1").as_deref(), Some("3/2"));
        assert_eq!(head("3/3").as_deref(), Some("3/2"));
        assert_eq!(head("3/1|a"), None);
        assert_eq!(head("3/2"), None);
    }
}
