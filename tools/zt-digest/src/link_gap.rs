//! The semantic gap set of one Link unit (§5.5, D118–D124; issue #110).
//!
//! A group owns exactly the Cards its own write receipts name. From those, the gap set selects
//! only the Cards that still need a Link job:
//!
//! - an Idea Card with no valid kinded semantic Link to another digest group or relevant earlier
//!   material (D119);
//! - a substantive orphan according to the existing authored-Link evidence (D120);
//! - the anchor whose one coordinator-owned `在全书中的位置` paragraph is missing, duplicated, or
//!   names no other part (D121).
//!
//! Source locators (the book Card and its anchors), register membership, generated Reverse links,
//! and a parent-only Link never close a semantic gap (D122, AC04). The classifier is the one
//! authority behind both `prepare-link`'s public scope and `link-finish`'s frozen enumeration, so
//! the two cannot disagree about which Cards a Link job may touch.

use crate::ps::{self, eq, s};
use crate::card::text;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

/// An Idea Card with no valid kinded semantic Link outside its own group.
pub(crate) const NO_SEMANTIC_LINK: &str = "no_semantic_link";
/// A substantive Card that no other Card's authored text links.
pub(crate) const ORPHAN: &str = "orphan";
/// The anchor holds no `在全书中的位置` paragraph.
pub(crate) const ANCHOR_MISSING: &str = "anchor_position_missing";
/// The anchor holds more than one `在全书中的位置` paragraph.
pub(crate) const ANCHOR_DUPLICATE: &str = "anchor_position_duplicate";
/// The anchor's one `在全书中的位置` paragraph names no other part of the book.
pub(crate) const ANCHOR_UNLINKED: &str = "anchor_position_unlinked";

/// The paragraph prefix the round's coordinator owns on each anchor.
pub(crate) const POSITION_PREFIX: &str = "在全书中的位置";

/// A Literature Card (`<citation-key>`) or an anchor (`<citation-key>/<n>`): a source place,
/// never a semantic Link target. The same shape the checker's L1 rule uses.
static BOOK_PLACE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^\d/|][^/|]*(?:/\d+)?$").unwrap());

/// Whether an address is a source place: the book Card or one of its anchors.
pub(crate) fn is_book_place(target: &str) -> bool {
    BOOK_PLACE.is_match(target)
}

/// The archive-wide facts the classification needs: every authored Link, both registers, the
/// Literature trees, and the anchor count per tree.
pub(crate) struct Archive {
    pub links: Vec<(String, String)>,
    pub register: HashSet<String>,
    pub registers: HashSet<String>,
    pub literature: HashSet<String>,
    pub anchors: HashMap<String, usize>,
}

/// One digest group's authority: its anchor and the addresses its own write receipts name.
#[derive(Clone, Debug)]
pub(crate) struct Group {
    pub n: i64,
    pub id: String,
    pub anchor: String,
    pub owned: Vec<String>,
}

/// One complete archive read the classification uses. `cards` holds the text facts of every owned
/// Card; it is not the whole archive.
pub(crate) struct Facts {
    pub book: String,
    pub cards: BTreeMap<String, Value>,
    pub links: Vec<(String, String)>,
    pub register: HashSet<String>,
    pub registers: HashSet<String>,
    pub literature: HashSet<String>,
    pub anchors: HashMap<String, usize>,
    /// The archive revision the facts were read at.
    pub revision: u64,
}

impl Facts {
    pub(crate) fn new(
        book: &str,
        cards: BTreeMap<String, Value>,
        archive: Archive,
        revision: u64,
    ) -> Facts {
        Facts {
            book: book.to_string(),
            cards,
            links: archive.links,
            register: archive.register,
            registers: archive.registers,
            literature: archive.literature,
            anchors: archive.anchors,
            revision,
        }
    }
}

impl Archive {
    pub(crate) fn new(
        links: Vec<(String, String)>,
        register: HashSet<String>,
        registers: HashSet<String>,
        literature: HashSet<String>,
        anchors: HashMap<String, usize>,
    ) -> Archive {
        Archive {
            links,
            register,
            registers,
            literature,
            anchors,
        }
    }

    /// The archive-wide facts of one read: every authored Link, both registers, and the title
    /// catalogue that classifies anchors and Literature trees.
    pub(crate) fn from_views(
        trees: &[Value],
        links_view: &Value,
        register_views: &[Value],
    ) -> Result<Archive> {
        let mut links = Vec::new();
        for row in ps::ps_items(&links_view["links"]) {
            let (source, target) = (s(&row["source"]), s(&row["target"]));
            if !source.is_empty() && !target.is_empty() {
                links.push((source, target));
            }
        }
        let mut register = HashSet::new();
        let mut registers = HashSet::new();
        for view in register_views {
            if view["register_missing"] == true {
                continue;
            }
            if let Some(address) = view["register"].as_str()
                && !address.trim().is_empty()
            {
                registers.insert(ps::key(address.trim()));
            }
            for part in ps::ps_items(&view["parts"]) {
                if part["place"] != "ok" {
                    continue;
                }
                if let Some(address) = part["address"].as_str()
                    && !address.trim().is_empty()
                {
                    register.insert(ps::key(address.trim()));
                }
            }
        }
        let mut literature = HashSet::new();
        let mut anchors: HashMap<String, usize> = HashMap::new();
        for tree in trees {
            if eq(&s(&tree["kind"]), "literature") {
                literature.insert(s(&tree["tree"]));
            }
            for card in ps::ps_items(&tree["cards"]) {
                let address = s(&card["address"]);
                if is_book_place(&address) && address.contains('/') {
                    *anchors.entry(s(&tree["tree"])).or_default() += 1;
                }
            }
        }
        Ok(Archive::new(links, register, registers, literature, anchors))
    }
}

/// One gap Card and the reasons it is in the gap set (in a stable order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Gap {
    pub at: String,
    pub reasons: Vec<String>,
}

impl Gap {
    /// One gap Card as the frozen scope and the public scope evidence show it.
    pub fn json(&self) -> Value {
        json!({"at": self.at, "reasons": self.reasons})
    }
}

/// One group's frozen gap scope.
#[derive(Clone, Debug)]
pub(crate) struct GroupGaps {
    pub n: i64,
    pub id: String,
    pub anchor: String,
    pub owned: Vec<String>,
    pub gaps: Vec<Gap>,
    pub skipped: Vec<String>,
    /// The anchor's position state: `linked`, `missing`, `duplicate`, or `unlinked`.
    pub position: String,
}

impl GroupGaps {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.gaps.is_empty()
    }

    /// The public scope evidence: the exact owned enumeration, every gap with its reasons, and
    /// every owned Card the job must not touch.
    pub fn json(&self) -> Value {
        let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
        for gap in &self.gaps {
            for reason in &gap.reasons {
                *reasons.entry(reason.clone()).or_default() += 1;
            }
        }
        json!({
            "n": self.n,
            "id": self.id,
            "anchor": self.anchor,
            "position": self.position,
            "owned": self.owned,
            "gaps": self.gaps.iter().map(|gap| json!({"at": gap.at, "reasons": gap.reasons})).collect::<Vec<_>>(),
            "skipped": self.skipped,
            "counts": {
                "owned": self.owned.len(),
                "gaps": self.gaps.len(),
                "skipped": self.skipped.len(),
                "reasons": reasons,
            },
        })
    }
}

/// The classification of every group of one read. `owners` maps a Card to the one group whose
/// receipts name it; a second claim on the same Card is a preparation fault.
pub(crate) fn classify(facts: &Facts, groups: &[Group]) -> Result<Vec<GroupGaps>> {
    let mut owner: HashMap<String, i64> = HashMap::new();
    for group in groups {
        let mut addresses = group.owned.clone();
        addresses.push(group.anchor.clone());
        for at in addresses {
            let key = ps::key(at.trim());
            if key.is_empty() {
                continue;
            }
            if let Some(other) = owner.get(&key)
                && *other != group.n
            {
                bail!(
                    "ownership fault: `{at}` is named by both group {other} and group {}; one Card has one receipt owner",
                    group.n
                );
            }
            owner.insert(key, group.n);
        }
    }
    let mut inbound: HashMap<String, BTreeSet<String>> = HashMap::new();
    for (source, target) in &facts.links {
        let (source, target) = (source.trim(), target.trim());
        if source.is_empty() || target.is_empty() || eq(source, target) {
            continue;
        }
        inbound
            .entry(ps::key(target))
            .or_default()
            .insert(source.to_string());
    }
    let register_membership = |target: &str| {
        let key = ps::key(target);
        facts.register.contains(&key) || facts.registers.contains(&key)
    };
    let mut out = Vec::new();
    for group in groups {
        let mut owned: Vec<String> = group.owned.clone();
        if !owned.iter().any(|at| eq(at, &group.anchor)) {
            owned.push(group.anchor.clone());
        }
        ps::ps_sort(&mut owned, |a, b| ps::culture_cmp(a, b));
        let mut gaps = Vec::new();
        let mut skipped = Vec::new();
        let mut position = "absent".to_string();
        for at in &owned {
            let card = facts.cards.get(&ps::key(at)).with_context(|| {
                format!("incomplete gap input: the owned Card `{at}` is missing from the archive read")
            })?;
            let category = zt_check::graph::category_with_parts(
                card,
                &facts.literature,
                &facts.anchors,
                &facts.register,
            );
            let mut reasons: Vec<String> = Vec::new();
            // D119: an Idea Card needs one kinded semantic Link outside its own group. The rule is
            // the checker's I4, read with AC04's exclusions.
            if eq(&category, "idea")
                && !has_semantic_link(card, group.n, &owner, &register_membership)
            {
                reasons.push(NO_SEMANTIC_LINK.to_string());
            }
            // D120: the existing authored-Link evidence, never a second orphan definition.
            if zt_check::graph::SUBSTANTIVE.contains(&category.as_str()) {
                let from: Vec<String> = inbound
                    .get(&ps::key(at))
                    .map(|sources| sources.iter().cloned().collect())
                    .unwrap_or_default();
                if zt_check::graph::is_orphan(at, &category, &from) {
                    reasons.push(ORPHAN.to_string());
                }
            }
            // D121: the anchor keeps one coordinator-owned `在全书中的位置` paragraph.
            if eq(at, &group.anchor) {
                let body = s(&card["body"]);
                let paragraphs = position_paragraphs(&body);
                let state = match paragraphs.len() {
                    0 => ANCHOR_MISSING,
                    1 if paragraph_links_another_part(&paragraphs[0], group.n, &owner, facts, &register_membership) => "linked",
                    1 => ANCHOR_UNLINKED,
                    _ => ANCHOR_DUPLICATE,
                };
                position = if state == ANCHOR_MISSING || state == ANCHOR_UNLINKED || state == ANCHOR_DUPLICATE {
                    state.to_string()
                } else {
                    "linked".to_string()
                };
                if state != "linked" {
                    reasons.push(state.to_string());
                }
            }
            if reasons.is_empty() {
                skipped.push(at.clone());
            } else {
                gaps.push(Gap {
                    at: at.clone(),
                    reasons,
                });
            }
        }
        out.push(GroupGaps {
            n: group.n,
            id: group.id.clone(),
            anchor: group.anchor.clone(),
            owned,
            gaps,
            skipped,
            position,
        });
    }
    Ok(out)
}

/// D119 + AC04: a Link closes a gap only when it is kinded, names a Card other than the parent,
/// and names neither a source place (the book Card or an anchor), a register Card, nor a Card of
/// the Card's own group. A Link the index reads as `refers`, a generated Reverse link (absent from
/// the authored body), and a parent-only Link never count.
fn has_semantic_link(
    card: &Value,
    group: i64,
    owner: &HashMap<String, i64>,
    register: &dyn Fn(&str) -> bool,
) -> bool {
    let Some(kinds) = card["kinds"].as_object() else {
        return false;
    };
    let parent = s(&card["parent_address"]);
    let mut targets: Vec<&String> = kinds.keys().collect();
    targets.sort();
    for target in targets {
        let target = target.trim();
        if target.is_empty()
            || eq(&s(&kinds[target]), "refers")
            || (!parent.is_empty() && eq(target, &parent))
            || BOOK_PLACE.is_match(target)
            || register(target)
        {
            continue;
        }
        if owner.get(&ps::key(target)) == Some(&group) {
            continue;
        }
        return true;
    }
    false
}

/// The anchor's position paragraph names another part of the book: an anchor or a Card outside
/// this group. The book Card itself and the register stay navigation.
fn paragraph_links_another_part(
    paragraph: &str,
    group: i64,
    owner: &HashMap<String, i64>,
    facts: &Facts,
    register: &dyn Fn(&str) -> bool,
) -> bool {
    for target in text::link_targets(paragraph) {
        let target: &str = target.trim();
        if target.is_empty()
            || eq(target, &facts.book)
            || register(target)
            || owner.get(&ps::key(target)) == Some(&group)
        {
            continue;
        }
        return true;
    }
    false
}

/// The `在全书中的位置` paragraphs of one authored body, outside code fences.
pub(crate) fn position_paragraphs(body: &str) -> Vec<String> {
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current = String::new();
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
        if trimmed.is_empty() {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
            continue;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(trimmed);
    }
    if !current.is_empty() {
        paragraphs.push(current);
    }
    paragraphs
        .into_iter()
        .filter(|paragraph| paragraph.starts_with(POSITION_PREFIX))
        .collect()
}

/// The one archive read: the owned Cards' text facts, every authored Link, both registers, and the
/// The one archive read: the owned Cards' text facts, every authored Link, both registers, and the
/// title catalogue that classifies anchors and Literature trees. A changing view is refused.
pub(crate) fn read(zt: &mut Zt, book: &str, addresses: &[String]) -> Result<Facts> {
    let trees = crate::snapshot::read_title_trees(zt)?;
    let revision = trees
        .first()
        .and_then(|tree| tree["archive_revision"].as_u64())
        .context("incomplete gap read: the title export names no archive revision")?;
    let read = zt.cards(addresses, "text")?;
    if read.archive_revision != revision {
        bail!("incomplete gap read: changing view between titles and Card bodies");
    }
    let mut cards = BTreeMap::new();
    for card in &read.cards {
        cards.insert(ps::key(&s(&card["address"])), card.clone());
    }
    let links_view = zt.call("list_links", json!({}))?;
    let mut register_views = Vec::new();
    for workspace in ["agent", "human"] {
        register_views.push(zt.call(
            "get_register",
            json!({"workspace": workspace, "entry_offset": 0, "entry_limit": 1}),
        )?);
    }
    let archive = Archive::from_views(&trees, &links_view, &register_views)?;
    if zt.cards(&[], "text")?.archive_revision != revision {
        bail!("incomplete gap read: changing view after the gap inputs");
    }
    Ok(Facts::new(book, cards, archive, revision))
}
#[cfg(test)]
mod tests {
    use super::*;

    fn idea(address: &str, parent: &str, body: &str, kinds: Value) -> Value {
        json!({
            "address": address, "tree": "K", "workspace": "agent", "kind": "regular",
            "type": "idea", "facet": null, "title": "一张观点卡", "body": body,
            "parent_address": parent, "kinds": kinds,
        })
    }

    fn facts(cards: Vec<Value>, links: Vec<(&str, &str)>) -> Facts {
        let mut by = BTreeMap::new();
        for card in cards {
            by.insert(ps::key(&s(&card["address"])), card);
        }
        Facts {
            book: "K".into(),
            cards: by,
            links: links
                .into_iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            register: HashSet::new(),
            registers: HashSet::new(),
            literature: HashSet::from(["K".to_string()]),
            anchors: HashMap::from([("K".to_string(), 1)]),
            revision: 0,
        }
    }

    fn group(owned: &[&str], anchor: &str) -> Group {
        Group {
            n: 1,
            id: "g001".into(),
            anchor: anchor.into(),
            owned: owned.iter().map(|at| at.to_string()).collect(),
        }
    }

    /// The anchor `K/1` with a linked position paragraph, so a test can vary one Card only.
    fn anchor(paragraph: &str) -> Value {
        json!({
            "address": "K/1", "tree": "K", "workspace": "agent", "kind": "regular",
            "type": "idea", "facet": null, "title": "第一章的论点", "body": paragraph,
            "parent_address": "K", "kinds": {},
        })
    }

    #[test]
    fn a_kinded_link_to_another_group_closes_an_idea_gap() {
        let unlinked = idea("K/1|a", "K/1", "企业替代市场。", json!({}));
        let linked = idea("K/1|b", "K/1", "依赖 [[0/1]] 的成本论证。", json!({"12/1": "rests-on"}));
        let read = facts(
            vec![anchor("在全书中的位置：承接 [[0/1]]。"), unlinked, linked],
            vec![("0/1", "K/1|b")],
        );
        let gaps = classify(&read, &[group(&["K/1", "K/1|a", "K/1|b"], "K/1")]).unwrap();
        let scope = &gaps[0];
        assert_eq!(scope.gaps.len(), 1);
        assert_eq!(scope.gaps[0].at, "K/1|a");
        assert_eq!(
            scope.gaps[0].reasons,
            vec![NO_SEMANTIC_LINK.to_string(), ORPHAN.to_string()]
        );
        assert!(scope.skipped.contains(&"K/1|b".to_string()));
        assert_eq!(scope.position, "linked");
        assert_eq!(scope.json()["counts"]["gaps"], json!(1));
    }

    #[test]
    fn a_parent_only_link_or_a_source_place_does_not_close_a_gap() {
        let parent = idea("K/1|a", "K/1", "承接 [[K/1]] 的设定。", json!({"K/1": "rests-on"}));
        let anchor_place = idea("K/1|b", "K/1", "对应 [[K/1|a]]。", json!({"K/1|a": "corresponds"}));
        let book = idea("K/1|c", "K/1", "依赖 [[K]] 的框架。", json!({"K": "rests-on"}));
        let read = facts(
            vec![anchor("在全书中的位置：承接 [[0/1]]。"), parent, anchor_place, book],
            vec![],
        );
        let gaps = classify(&read, &[group(&["K/1", "K/1|a", "K/1|b", "K/1|c"], "K/1")]).unwrap();
        assert_eq!(
            gaps[0].gaps.iter().map(|gap| gap.at.as_str()).collect::<Vec<_>>(),
            vec!["K/1|a", "K/1|b", "K/1|c"]
        );
    }

    #[test]
    fn a_register_link_and_an_intra_group_link_do_not_close_a_gap() {
        let mut read = facts(
            vec![
                anchor("在全书中的位置：承接 [[0/1]]。"),
                idea("K/1|a", "K/1", "对照 [[索引/1]] 的词条。", json!({"索引/1": "contrasts"})),
                idea("K/1|b", "K/1", "依赖 [[K/1|a]] 的结论。", json!({"K/1|a": "rests-on"})),
            ],
            vec![],
        );
        read.registers.insert("索引/1".into());
        let gaps = classify(&read, &[group(&["K/1", "K/1|a", "K/1|b"], "K/1")]).unwrap();
        assert_eq!(
            gaps[0].gaps.iter().map(|gap| gap.at.as_str()).collect::<Vec<_>>(),
            vec!["K/1|a", "K/1|b"]
        );
    }

    #[test]
    fn a_substantive_orphan_stays_a_gap_even_with_a_cross_link() {
        let read = facts(
            vec![
                anchor("在全书中的位置：承接 [[0/1]]。"),
                idea("K/1|a", "K/1", "依赖 [[0/1]] 的成本论证。", json!({"12/1": "rests-on"})),
            ],
            vec![],
        );
        let gaps = classify(&read, &[group(&["K/1", "K/1|a"], "K/1")]).unwrap();
        assert_eq!(gaps[0].gaps.len(), 1);
        assert_eq!(gaps[0].gaps[0].at, "K/1|a");
        assert_eq!(gaps[0].gaps[0].reasons, vec![ORPHAN.to_string()]);
        // The same Card with an authored inbound Link from another Card is not an orphan.
        let read = facts(
            vec![
                anchor("在全书中的位置：承接 [[0/1]]。"),
                idea("K/1|a", "K/1", "依赖 [[0/1]] 的成本论证。", json!({"12/1": "rests-on"})),
            ],
            vec![("K/1", "K/1|a")],
        );
        let gaps = classify(&read, &[group(&["K/1", "K/1|a"], "K/1")]).unwrap();
        assert!(gaps[0].is_empty());
    }

    #[test]
    fn the_anchor_needs_one_linked_position_paragraph() {
        let cases = [
            ("", "anchor_position_missing", ANCHOR_MISSING),
            ("正文说明。\n\n在全书中的位置：本部分引出下一章。", "anchor_position_unlinked", ANCHOR_UNLINKED),
            ("在全书中的位置：承接 [[0/1]]。\n\n在全书中的位置：又一段。", "anchor_position_duplicate", ANCHOR_DUPLICATE),
        ];
        for (body, position, reason) in cases {
            let read = facts(vec![anchor(body)], vec![]);
            let gaps = classify(&read, &[group(&["K/1"], "K/1")]).unwrap();
            let scope = &gaps[0];
            assert_eq!(scope.position, position, "body: {body}");
            assert_eq!(scope.gaps[0].at, "K/1");
            assert!(scope.gaps[0].reasons.contains(&reason.to_string()), "{scope:?}");
        }
        let read = facts(vec![anchor("在全书中的位置：承接 [[0/1]]。")], vec![]);
        let gaps = classify(&read, &[group(&["K/1"], "K/1")]).unwrap();
        assert!(gaps[0].is_empty());
        assert_eq!(gaps[0].position, "linked");
    }

    #[test]
    fn conflicting_receipts_are_a_preparation_fault() {
        let read = facts(vec![anchor("在全书中的位置：承接 [[0/1]]。")], vec![]);
        let other = Group {
            n: 2,
            id: "g002".into(),
            anchor: "K/1|a".into(),
            owned: vec!["K/1".into()],
        };
        let error = classify(&read, &[group(&["K/1"], "K/1"), other]).unwrap_err();
        assert!(format!("{error:#}").contains("ownership fault"));
    }
}



