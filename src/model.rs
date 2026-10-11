//! Model structure: one Model Card and the Model parts filed under it.
//!
//! A whole model is presented by its Model Card (the head: question, overview,
//! structure chart) and by Model parts: Model Cards that each carry a Model
//! facet and are filed below the head, as its successors or as successors of
//! other parts. A facet is a short label the author of the model chooses
//! (`setting`, `动态`, `a worked example`); the common facets are only a
//! suggested vocabulary. A part may also carry an Emphasis: `core` for the
//! parts a reader must see, `aside` for detail a reader may skip.
//!
//! Inside a model the structure is a graph, not the tree its Cards are filed
//! in: two parts are related when one links the other, and a part that links
//! no other part and is linked by none is loose. Facets and emphasis are
//! stored on the Card row like the Card type; the structure is computed from
//! Locations, facets, and Links, never stored, so it follows the Cards when
//! they move.

use anyhow::{Result, bail};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::link::LinkBody;
use crate::{Card, CardType};

/// Which part of its model a Model part presents: a short, one-line label.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct Facet(String);

/// The facets most models need, in the order a model is usually read, with
/// what a part of each holds. A suggestion for authors, not a rule.
pub(crate) const COMMON_FACETS: [(&str, &str); 8] = [
    ("setting", "agents, assets, actions, and payoffs"),
    (
        "assumptions",
        "what the model assumes, and what each assumption buys",
    ),
    ("timing", "the order of moves and who knows what when"),
    (
        "dynamics",
        "how the state evolves: laws of motion, best responses, feedback",
    ),
    (
        "solution",
        "the equilibrium or optimum and how it is derived",
    ),
    ("results", "propositions and comparative statics"),
    ("intuition", "why the results hold, in words"),
    ("limits", "where the model stops applying, and its critics"),
];

impl Facet {
    /// The longest facet, in characters.
    pub(crate) const MAX_CHARS: usize = 32;

    /// The stored and typed label.
    pub(crate) fn id(&self) -> &str {
        &self.0
    }

    /// A label: trimmed, its inner whitespace collapsed, one line of at most
    /// [`Facet::MAX_CHARS`] characters. A common facet in any case is stored in
    /// lower case.
    pub(crate) fn parse(value: &str) -> Result<Self> {
        let label = value.split_whitespace().collect::<Vec<_>>().join(" ");
        if label.is_empty() {
            bail!("a Model facet cannot be empty");
        }
        if label.chars().count() > Self::MAX_CHARS {
            bail!(
                "Model facet `{label}` is too long; a facet is a short label of at most {} characters",
                Self::MAX_CHARS
            );
        }
        if label.chars().any(char::is_control) || label.contains("[[") || label.contains("]]") {
            bail!("Model facet `{label}` must be plain text on one line, without Links");
        }
        let common = COMMON_FACETS
            .iter()
            .find(|(id, _)| id.eq_ignore_ascii_case(&label))
            .map(|(id, _)| (*id).to_string());
        Ok(Facet(common.unwrap_or(label)))
    }

    /// `setting, assumptions, …, or limits`.
    pub(crate) fn common_names() -> String {
        let ids = COMMON_FACETS.map(|(id, _)| id);
        format!(
            "{}, or {}",
            ids[..ids.len() - 1].join(", "),
            ids[ids.len() - 1]
        )
    }

    /// Read a stored value; an empty or unreadable one is no facet.
    pub(crate) fn from_stored(value: Option<&str>) -> Option<Self> {
        value.and_then(|value| Facet::parse(value).ok())
    }
}

impl std::fmt::Display for Facet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// How much of a reader's attention a Model part asks for. A part without an
/// Emphasis is a regular part.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum Emphasis {
    /// The heart of the model: what a reader must see first.
    Core,
    /// Detail a reader may skip.
    Aside,
}

impl Emphasis {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Emphasis::Core => "core",
            Emphasis::Aside => "aside",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "core" => Ok(Emphasis::Core),
            "aside" => Ok(Emphasis::Aside),
            other => bail!("unknown emphasis `{other}`; expected core, aside, or empty for none"),
        }
    }

    pub(crate) fn from_stored(value: Option<&str>) -> Option<Self> {
        value.and_then(|value| Emphasis::parse(value).ok())
    }
}

impl std::fmt::Display for Emphasis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// What makes a new Card a Model part: its facet and, if any, its emphasis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Part {
    pub(crate) facet: Facet,
    pub(crate) emphasis: Option<Emphasis>,
}

impl Part {
    /// A part from optional arguments; an emphasis without a facet is refused.
    pub(crate) fn from_args(
        facet: Option<Facet>,
        emphasis: Option<Emphasis>,
    ) -> Result<Option<Self>> {
        match (facet, emphasis) {
            (Some(facet), emphasis) => Ok(Some(Part { facet, emphasis })),
            (None, Some(_)) => bail!(EMPHASIS_NEEDS_FACET),
            (None, None) => Ok(None),
        }
    }
}

const EMPHASIS_NEEDS_FACET: &str =
    "only Model parts can have an emphasis; give the Card a Model facet first";

fn clears(value: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value.eq_ignore_ascii_case("none")
}

/// Parse an optional facet argument: an empty string or `none` clears the facet.
pub(crate) fn parse_optional(value: &str) -> Result<Option<Facet>> {
    if clears(value) {
        Ok(None)
    } else {
        Facet::parse(value).map(Some)
    }
}

/// Parse an optional emphasis argument: empty, `none`, or `regular` clears it.
pub(crate) fn parse_optional_emphasis(value: &str) -> Result<Option<Emphasis>> {
    if clears(value) || value.trim().eq_ignore_ascii_case("regular") {
        Ok(None)
    } else {
        Emphasis::parse(value).map(Some)
    }
}

/// The Model Card a Card filed at `start` belongs to: walk up through Model
/// parts until a Model Card without a facet. `lookup` tells whether the Card at
/// an address is a Model Card and whether it has a facet.
pub(crate) fn find_head(
    start: Option<String>,
    lookup: impl Fn(&str) -> Option<(CardType, bool)>,
) -> Option<String> {
    let mut at = start;
    while let Some(address) = at {
        match lookup(&address)? {
            (CardType::Model, false) => return Some(address),
            (CardType::Model, true) => at = crate::parent_location(&address),
            _ => return None,
        }
    }
    None
}

fn db_lookup(conn: &Connection) -> impl Fn(&str) -> Option<(CardType, bool)> + '_ {
    |address| {
        crate::load_card(conn, address)
            .ok()
            .map(|card| (card.card_type, card.facet.is_some()))
    }
}

/// Check the type, facet, and emphasis a Card at `location` would have. A
/// facet needs a Model Card filed below another Model Card; only a part has an
/// emphasis; a Model Card with parts under it cannot stop being their head.
pub(crate) fn ensure_allowed(
    conn: &Connection,
    location: &str,
    current: Option<&Card>,
    card_type: CardType,
    facet: Option<&Facet>,
    emphasis: Option<Emphasis>,
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
    } else if emphasis.is_some() {
        bail!(EMPHASIS_NEEDS_FACET);
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
            .map(|card| (card.card_type, card.facet.is_some()))
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

/// Store a Card's type, facet, and emphasis.
pub(crate) fn set(
    conn: &Connection,
    card: &Card,
    card_type: CardType,
    facet: Option<&Facet>,
    emphasis: Option<Emphasis>,
) -> Result<()> {
    crate::card_type::ensure_allowed(card, card_type)?;
    ensure_allowed(conn, &card.address, Some(card), card_type, facet, emphasis)?;
    conn.execute(
        "UPDATE cards SET card_type = ?1, model_facet = ?2, model_emphasis = ?3 WHERE row_id = ?4",
        params![
            card_type.id(),
            facet.map(Facet::id),
            emphasis.map(Emphasis::id),
            card.row_id
        ],
    )?;
    Ok(())
}

/// Folgezettel order: numbers by value, side letters by length then
/// alphabet, so `3/2|b` comes before `3/2|aa` and `3/10` after `3/9`.
pub(crate) fn address_order(a: &str, b: &str) -> Ordering {
    fn segments(address: &str) -> Vec<(u8, u64, &str)> {
        address
            .split(['/', '|'])
            .map(|segment| match segment.parse::<u64>() {
                Ok(number) => (0, number, ""),
                Err(_) => (1, segment.len() as u64, segment),
            })
            .collect()
    }
    segments(a).cmp(&segments(b)).then_with(|| a.cmp(b))
}

/// The sentence of `body` around the byte range `at`, with every Link in it
/// replaced by `name(target)`, clipped for a tooltip.
fn sentence_around(
    body: &str,
    at: std::ops::Range<usize>,
    name: impl Fn(&str) -> String,
) -> String {
    // A sentence ends at CJK or ASCII end punctuation, a line break, or a
    // period followed by a space.
    let ends_at = |index: usize, ch: char| {
        ['。', '！', '？', '!', '?', '；', ';', '\n'].contains(&ch)
            || (ch == '.' && body[index + 1..].starts_with(char::is_whitespace))
    };
    let start = body[..at.start]
        .char_indices()
        .rev()
        .find(|&(index, ch)| ends_at(index, ch))
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let end = body[at.end..]
        .char_indices()
        .find(|&(index, ch)| ends_at(at.end + index, ch))
        .map_or(body.len(), |(index, ch)| {
            at.end + index + if ch == '\n' { 0 } else { ch.len_utf8() }
        });
    let sentence = &body[start..end];
    let mut text = String::new();
    let mut rest = sentence;
    while let Some(open) = rest.find("[[") {
        let Some(close) = rest[open..].find("]]") else {
            break;
        };
        text.push_str(&rest[..open]);
        text.push('「');
        text.push_str(&name(&rest[open + 2..open + close]));
        text.push('」');
        rest = &rest[open + close + 2..];
    }
    text.push_str(rest);
    let text = text
        .trim()
        .trim_start_matches(['-', '*', '>', '#', ' '])
        .replace("**", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = text.chars();
    let clipped: String = chars.by_ref().take(140).collect();
    if chars.next().is_some() {
        format!("{clipped}…")
    } else {
        clipped
    }
}

/// The structure of the model a Card belongs to, computed over a set of
/// Cards. `None` when the Card is neither a Model Card nor a Model part.
///
/// Parts are listed in Folgezettel order. `relations` lists every Link from
/// one part to another, once per pair, with the sentence that makes it;
/// `loose` lists the parts in no relation.
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
    let lookup = |at: &str| {
        by_address
            .get(at)
            .map(|card| (card.card_type, card.facet.is_some()))
    };
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
    let body_of = |at: &str| {
        parsed
            .get(at)
            .map(|parsed| parsed.body.as_str())
            .unwrap_or_default()
    };
    let links_of = |at: &str| -> BTreeSet<String> {
        LinkBody::parse(body_of(at))
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
            "facet": card.facet.as_ref().map(Facet::id),
            "emphasis": card.emphasis.map(Emphasis::id),
            "parts": [],
            "relations": [],
            "loose": [],
            "issues": [format!(
                "`{}` is a Model part ({}) but is not filed under a Model Card",
                card.address,
                card.facet.as_ref().map(Facet::id).unwrap_or_default()
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
    parts.sort_by(|a, b| address_order(&a.address, &b.address));
    let facet_of = parts
        .iter()
        .map(|part| {
            (
                part.address.as_str(),
                part.facet.as_ref().map(Facet::id).unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let members = std::iter::once(head.clone())
        .chain(parts.iter().map(|part| part.address.clone()))
        .collect::<BTreeSet<_>>();
    let head_links = links_of(&head);
    let mut issues = Vec::new();
    let mut relations = Vec::new();
    let mut related = BTreeSet::new();
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
            let body = body_of(&part.address);
            let mut seen = BTreeSet::new();
            if let Ok(parsed_body) = LinkBody::parse(body) {
                for occurrence in parsed_body.occurrences() {
                    let target = occurrence.target;
                    if target == part.address
                        || !facet_of.contains_key(target)
                        || !seen.insert(target)
                    {
                        continue;
                    }
                    related.insert(part.address.clone());
                    related.insert(target.to_string());
                    let name = |at: &str| {
                        facet_of.get(at).map_or_else(
                            || {
                                if at == head {
                                    "模型".to_string()
                                } else {
                                    at.to_string()
                                }
                            },
                            |facet| (*facet).to_string(),
                        )
                    };
                    relations.push(json!({
                        "from": part.address,
                        "to": target,
                        "note": sentence_around(body, occurrence.macro_range.clone(), name),
                    }));
                }
            }
            json!({
                "address": part.address,
                "title": title(&part.address),
                "facet": part.facet.as_ref().map(Facet::id),
                "emphasis": part.emphasis.map(Emphasis::id),
                "links": links
                    .iter()
                    .filter(|target| members.contains(*target) && **target != part.address)
                    .collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let loose = parts
        .iter()
        .filter(|part| !related.contains(&part.address))
        .map(|part| part.address.clone())
        .collect::<Vec<_>>();
    // Cards outside the model that link any of its Cards: its discussion.
    let discussion = members
        .iter()
        .flat_map(|member| inbound.get(member).into_iter().flatten())
        .filter(|source| !members.contains(*source))
        .collect::<BTreeSet<_>>();
    Some(json!({
        "head": { "address": head, "title": title(&head) },
        "facet": card.facet.as_ref().map(Facet::id),
        "emphasis": card.emphasis.map(Emphasis::id),
        "head_links": head_links
            .iter()
            .filter(|target| members.contains(*target) && **target != head)
            .collect::<Vec<_>>(),
        "parts": part_values,
        "relations": relations,
        "loose": loose,
        "discussion": discussion.len(),
        "issues": issues,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facets_are_short_free_labels_and_common_facets_are_normalized() {
        assert_eq!(Facet::parse(" Dynamics ").unwrap().id(), "dynamics");
        assert_eq!(Facet::parse("动态").unwrap().id(), "动态");
        assert_eq!(
            Facet::parse("  a   worked\texample ").unwrap().id(),
            "a worked example"
        );
        assert_eq!(
            Facet::parse("  ").unwrap_err().to_string(),
            "a Model facet cannot be empty"
        );
        assert!(
            Facet::parse(&"长".repeat(33))
                .unwrap_err()
                .to_string()
                .contains("at most 32 characters")
        );
        assert!(Facet::parse(&"长".repeat(32)).is_ok());
        assert!(
            Facet::parse("see [[3/1]]")
                .unwrap_err()
                .to_string()
                .contains("without Links")
        );
        assert_eq!(
            Facet::common_names(),
            "setting, assumptions, timing, dynamics, solution, results, intuition, or limits"
        );
        assert_eq!(Facet::from_stored(Some("")), None);
        assert_eq!(Facet::from_stored(None), None);
        assert_eq!(parse_optional("").unwrap(), None);
        assert_eq!(parse_optional("none").unwrap(), None);
        assert_eq!(parse_optional("均衡").unwrap(), Some(Facet("均衡".into())));
    }

    #[test]
    fn emphasis_is_core_aside_or_none() {
        assert_eq!(Emphasis::parse(" Core").unwrap(), Emphasis::Core);
        assert_eq!(
            parse_optional_emphasis("aside").unwrap(),
            Some(Emphasis::Aside)
        );
        assert_eq!(parse_optional_emphasis("").unwrap(), None);
        assert_eq!(parse_optional_emphasis("regular").unwrap(), None);
        assert_eq!(
            Emphasis::parse("loud").unwrap_err().to_string(),
            "unknown emphasis `loud`; expected core, aside, or empty for none"
        );
    }

    #[test]
    fn the_head_is_the_nearest_model_card_above_a_chain_of_parts() {
        let cards: BTreeMap<&str, (CardType, bool)> = [
            ("3/1", (CardType::Idea, false)),
            ("3/2", (CardType::Model, false)),
            ("3/2|a", (CardType::Model, true)),
            ("3/2|a|1", (CardType::Model, true)),
            ("3/3", (CardType::Model, true)),
            ("3/1|a", (CardType::Model, true)),
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

    #[test]
    fn addresses_sort_in_folgezettel_order() {
        let mut addresses = vec!["3/10", "3/2|aa", "3/2|b", "3/9", "3/2|a|1", "3/2"];
        addresses.sort_by(|a, b| address_order(a, b));
        assert_eq!(
            addresses,
            ["3/2", "3/2|a|1", "3/2|b", "3/2|aa", "3/9", "3/10"]
        );
    }

    #[test]
    fn a_relation_note_is_the_sentence_that_makes_the_link() {
        let body = "本卡是模型 [[3/2]] 的动态。依赖时序 [[3/2|a|1]]：投资发生在**议价**之前，所以投资不足。另一句。";
        let at = body.find("[[3/2|a|1]]").unwrap();
        let note = sentence_around(body, at..at + "[[3/2|a|1]]".len(), |target| {
            if target == "3/2|a|1" {
                "时序".into()
            } else {
                target.into()
            }
        });
        assert_eq!(
            note,
            "依赖时序 「时序」：投资发生在议价之前，所以投资不足。"
        );
        let body = "- uses [[3/2|a]] for the payoffs\n- other";
        let at = body.find("[[").unwrap();
        assert_eq!(
            sentence_around(body, at..at + 11, |_| "setting".into()),
            "uses 「setting」 for the payoffs"
        );
    }
}
