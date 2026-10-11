//! Card operations shared by the GUI and the MCP server.
//!
//! Each change takes an [`Access`] and refuses to touch a Workspace that the
//! client may not write. Destructive changes are two-step: called without the
//! expected confirmation they only describe what would happen.

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::link::{LinkBody, LinkLifecycle};
use crate::{Access, Card, CardType, Workspace};

/// The result of a change that may need a typed confirmation first.
pub(crate) enum Outcome {
    /// Nothing changed; repeat the call with `confirm` set to this text.
    Confirm {
        confirm: String,
        lines: Vec<String>,
    },
    Done(Value),
}

impl Outcome {
    pub(crate) fn into_value(self) -> Value {
        match self {
            Outcome::Confirm { confirm, lines } => json!({ "confirm": confirm, "lines": lines }),
            Outcome::Done(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Successor {
    Direct,
    Side,
}

pub(crate) fn create_topic(
    root: &Path,
    access: Access,
    workspace: Workspace,
    text: &str,
) -> Result<String> {
    access.ensure_writable(workspace)?;
    let _lock = crate::acquire_edit_lock(root)?;
    let mut conn = crate::open_db(root)?;
    crate::create_topic(&mut conn, workspace, text)
}

pub(crate) fn create_successor(
    root: &Path,
    access: Access,
    kind: Successor,
    at: &str,
    card_type: CardType,
    text: &str,
) -> Result<String> {
    let _lock = crate::acquire_edit_lock(root)?;
    let mut conn = crate::open_db(root)?;
    let parent = crate::load_card(&conn, at)?;
    access.ensure_writable(parent.workspace)?;
    let location = match kind {
        Successor::Direct => {
            let location = crate::direct_successor(&parent.address)?;
            if crate::location_exists(&conn, &location)? {
                bail!("direct successor already exists: {location}");
            }
            location
        }
        Successor::Side => {
            crate::ensure_side_parent(&parent)?;
            crate::next_side_successor(&conn, &parent.address, &BTreeSet::new())?
        }
    };
    crate::insert_card(
        &mut conn,
        &location,
        false,
        parent.workspace,
        card_type,
        text,
    )?;
    Ok(location)
}

/// `text` is the Card text after the generated title: only its body is kept,
/// and the title always comes from the BibTeX `title` field.
pub(crate) fn create_literature(
    root: &Path,
    access: Access,
    workspace: Workspace,
    bibtex: &str,
    text: &str,
) -> Result<String> {
    access.ensure_writable(workspace)?;
    let _lock = crate::acquire_edit_lock(root)?;
    let mut conn = crate::open_db(root)?;
    let metadata = crate::literature::parse_metadata(bibtex)?;
    let parsed = crate::parse_literature_edit_text(text)?;
    let text = crate::compose_card_text(&metadata.title, &parsed.body, "");
    crate::insert_literature_card(&mut conn, &metadata.citation_key, workspace, bibtex, &text)?;
    Ok(metadata.citation_key)
}

/// Replace a Card's text, change its Card type, or both. With `base`, refuse
/// if the stored text changed since the caller read it.
pub(crate) fn edit(
    root: &Path,
    access: Access,
    at: &str,
    text: Option<&str>,
    card_type: Option<CardType>,
    base: Option<&str>,
) -> Result<String> {
    let _lock = crate::acquire_edit_lock(root)?;
    let mut conn = crate::open_db(root)?;
    let card = unchanged_card(&conn, at, base)?;
    access.ensure_writable(card.workspace)?;
    if let Some(card_type) = card_type {
        crate::card_type::ensure_allowed(&card, card_type)?;
    }
    match text {
        Some(text) if card.is_lit => crate::update_literature_text(&mut conn, &card, text)?,
        Some(text) => crate::update_card_text(&mut conn, &card, text)?,
        None => {}
    }
    if let Some(card_type) = card_type
        && card_type != card.card_type
    {
        crate::card_type::set(&conn, &card, card_type)?;
    }
    Ok(card.address)
}

/// Replace a Literature Card's BibTeX. A new Citation key renames the whole
/// tree and rewrites every Link to it, so it must be confirmed with `move`.
pub(crate) fn edit_metadata(
    root: &Path,
    access: Access,
    at: &str,
    bibtex: &str,
    base: Option<&str>,
    confirm: Option<&str>,
) -> Result<Outcome> {
    let _lock = crate::acquire_edit_lock(root)?;
    let mut conn = crate::open_db(root)?;
    let card = unchanged_card(&conn, at, base)?;
    access.ensure_writable(card.workspace)?;
    if !card.is_lit {
        bail!("only Literature Cards have BibTeX metadata");
    }
    let old_key = card
        .citation_key
        .clone()
        .context("Literature Card is missing its Citation key")?;
    let metadata = crate::literature::parse_metadata(bibtex)?;
    if metadata.citation_key != old_key {
        if metadata.citation_key.eq_ignore_ascii_case(&old_key)
            || crate::citation_key_conflicts(&conn, &metadata.citation_key, card.row_id)?
        {
            bail!(
                "Citation key `{}` conflicts with an existing Literature Card",
                metadata.citation_key
            );
        }
        let mapping = crate::citation_rename_mapping(&conn, &old_key, &metadata.citation_key)?;
        access.ensure_link_rewrites_allowed(&conn, &mapping)?;
        if confirm != Some("move") {
            let lines = crate::citation_rename_lines(&conn, &old_key, &metadata.citation_key)?;
            return Ok(Outcome::Confirm {
                confirm: "move".to_string(),
                lines,
            });
        }
    }
    crate::update_literature_metadata(
        &mut conn,
        &card,
        &metadata.citation_key,
        bibtex,
        &metadata.title,
    )?;
    Ok(Outcome::Done(json!({ "address": metadata.citation_key })))
}

/// Delete a Card and its successors (a Tree root deletes its whole tree).
pub(crate) fn delete(
    root: &Path,
    access: Access,
    at: &str,
    confirm: Option<&str>,
) -> Result<Outcome> {
    let _lock = crate::acquire_edit_lock(root)?;
    let mut conn = crate::open_db(root)?;
    let card = crate::load_card(&conn, at)?;
    access.ensure_writable(card.workspace)?;
    let plan = crate::delete_plan(&conn, at)?;
    if !plan.is_tree_root {
        let compaction = crate::side_compaction_mapping(&conn, at, &plan.delete_set)?;
        access.ensure_link_rewrites_allowed(&conn, &compaction)?;
    }
    if confirm != Some(plan.confirmation.as_str()) {
        return Ok(Outcome::Confirm {
            confirm: plan.confirmation.clone(),
            lines: crate::delete_verification_lines(&plan),
        });
    }
    let result = crate::apply_delete_plan(&mut conn, at, plan)?;
    let next = crate::parent_location(at)
        .filter(|parent| crate::target_exists(&conn, parent).unwrap_or(false));
    Ok(Outcome::Done(json!({
        "deleted": result.deleted_count,
        "moved": result.moved,
        "address": next,
    })))
}

/// Move a Regular Card and its successors inside its Workspace.
pub(crate) fn move_card(
    root: &Path,
    access: Access,
    at: &str,
    to: &str,
    confirm: Option<&str>,
) -> Result<Outcome> {
    let _lock = crate::acquire_edit_lock(root)?;
    let mut conn = crate::open_db(root)?;
    let card = crate::load_card(&conn, at)?;
    access.ensure_writable(card.workspace)?;
    let plan = crate::move_plan(&conn, at, to)?;
    access.ensure_link_rewrites_allowed(&conn, &plan.mapping)?;
    if confirm != Some("move") {
        return Ok(Outcome::Confirm {
            confirm: "move".to_string(),
            lines: crate::move_verification_lines(&plan),
        });
    }
    crate::apply_move_plan(&mut conn, &plan)?;
    Ok(Outcome::Done(
        json!({ "address": to, "moved": plan.mapping }),
    ))
}

fn unchanged_card(conn: &Connection, at: &str, base: Option<&str>) -> Result<Card> {
    let card = crate::load_card(conn, at)?;
    if let Some(base) = base
        && base != card.text
    {
        bail!("card `{at}` changed since it was opened; reload it and try again");
    }
    Ok(card)
}

/// Every Card with its parsed text and Link graph, read once per request.
pub(crate) struct Snapshot {
    pub(crate) cards: Vec<Card>,
    pub(crate) by_address: BTreeMap<String, Card>,
    pub(crate) parsed: BTreeMap<String, crate::ParsedCard>,
    pub(crate) lifecycle: LinkLifecycle,
    pub(crate) inbound: BTreeMap<String, BTreeSet<String>>,
}

impl Snapshot {
    pub(crate) fn read(root: &Path) -> Result<Self> {
        crate::ensure_markdown_cards(root)?;
        let conn = crate::open_db(root)?;
        Self::load(&conn)
    }

    pub(crate) fn load(conn: &Connection) -> Result<Self> {
        let cards = crate::load_cards(conn)?;
        let lifecycle = crate::link_lifecycle_from_cards(&cards)?;
        let mut parsed = BTreeMap::new();
        for card in &cards {
            parsed.insert(card.address.clone(), crate::parse_card_text(&card.text)?);
        }
        let by_address: BTreeMap<String, Card> = cards
            .iter()
            .map(|card| (card.address.clone(), card.clone()))
            .collect();
        let mut inbound: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for card in &cards {
            for occurrence in LinkBody::parse(&parsed[&card.address].body)?.occurrences() {
                if by_address.contains_key(occurrence.target) {
                    inbound
                        .entry(occurrence.target.to_string())
                        .or_default()
                        .insert(card.address.clone());
                }
            }
        }
        Ok(Self {
            cards,
            by_address,
            parsed,
            lifecycle,
            inbound,
        })
    }

    /// Changes whenever any Card changes, so open windows can follow edits
    /// made from the shell or a Session.
    pub(crate) fn fingerprint(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for card in &self.cards {
            hasher.update(card.address.as_bytes());
            hasher.update([0]);
            hasher.update(card.text.as_bytes());
            hasher.update([0]);
            hasher.update(card.bibtex.as_deref().unwrap_or_default().as_bytes());
            hasher.update([0]);
            hasher.update(card.workspace.id().as_bytes());
            hasher.update([0]);
            hasher.update(card.card_type.id().as_bytes());
            hasher.update([0]);
        }
        hasher
            .finalize()
            .iter()
            .take(12)
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub(crate) fn title(&self, address: &str) -> String {
        self.parsed
            .get(address)
            .map(|parsed| parsed.title.clone())
            .unwrap_or_default()
    }

    pub(crate) fn workspace(&self, address: &str) -> Option<Workspace> {
        self.by_address.get(address).map(|card| card.workspace)
    }

    /// A short reference to another Card: its address, title, and Workspace.
    pub(crate) fn reference(&self, address: &str) -> Value {
        json!({
            "address": address,
            "title": self.title(address),
            "workspace": self.workspace(address).map(Workspace::id),
        })
    }

    pub(crate) fn outbound(&self, address: &str) -> Vec<Value> {
        let Some(parsed) = self.parsed.get(address) else {
            return Vec::new();
        };
        let mut seen = BTreeSet::new();
        LinkBody::parse(&parsed.body)
            .map(|body| {
                body.occurrences()
                    .iter()
                    .filter(|occurrence| seen.insert(occurrence.target.to_string()))
                    .map(|occurrence| {
                        let exists = self.by_address.contains_key(occurrence.target);
                        json!({
                            "address": occurrence.target,
                            "title": self.title(occurrence.target),
                            "workspace": self.workspace(occurrence.target).map(Workspace::id),
                            "broken": !exists,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn summary(&self, card: &Card) -> Value {
        let parent = crate::parent_location(&card.address);
        let edge = if crate::is_tree_root(card) {
            "root"
        } else if crate::valid_side_label(crate::last_segment(&card.address)) {
            "side"
        } else {
            "direct"
        };
        let links = self
            .outbound(&card.address)
            .into_iter()
            .filter(|link| link["broken"] == false)
            .filter_map(|link| link["address"].as_str().map(str::to_string))
            .collect::<Vec<_>>();
        json!({
            "address": card.address,
            "tree": crate::tree_id(&card.address),
            "workspace": card.workspace.id(),
            "title": self.title(&card.address),
            "kind": kind(card),
            "type": card.card_type.id(),
            "edge": edge,
            "parent": parent,
            "excerpt": excerpt(&self.parsed[&card.address].body),
            "links": links,
            "inbound": self.inbound.get(&card.address).map_or(0, BTreeSet::len),
        })
    }

    /// Every tree root, with its Workspace, Zone, size, and how many of its
    /// Cards are Event and Model Cards.
    pub(crate) fn trees(&self) -> Vec<Value> {
        self.cards
            .iter()
            .filter(|card| crate::is_tree_root(card))
            .map(|card| {
                let tree = crate::tree_id(&card.address);
                let members = self
                    .cards
                    .iter()
                    .filter(|member| crate::is_tree_member(&member.address, tree))
                    .collect::<Vec<_>>();
                let typed = |card_type: CardType| {
                    members
                        .iter()
                        .filter(|member| member.card_type == card_type)
                        .count()
                };
                json!({
                    "tree": tree,
                    "root": card.address,
                    "workspace": card.workspace.id(),
                    "kind": zone(card),
                    "title": self.title(&card.address),
                    "excerpt": excerpt(&self.parsed[&card.address].body),
                    "count": members.len(),
                    "events": typed(CardType::Event),
                    "models": typed(CardType::Model),
                })
            })
            .collect()
    }

    /// The direct child Side successors of a Regular Card.
    pub(crate) fn sides(&self, card: &Card) -> Vec<Value> {
        if crate::is_tree_root(card) {
            return Vec::new();
        }
        self.cards
            .iter()
            .filter(|other| {
                crate::immediate_side_label(&other.address, &card.address)
                    .is_some_and(|label| other.address == format!("{}|{label}", card.address))
            })
            .map(|other| self.reference(&other.address))
            .collect()
    }

    pub(crate) fn inbound_refs(&self, address: &str) -> Vec<Value> {
        self.inbound
            .get(address)
            .into_iter()
            .flatten()
            .map(|source| self.reference(source))
            .collect()
    }

    /// Navigation to an address, if a Card is there.
    pub(crate) fn nav(&self, address: Option<String>) -> Option<Value> {
        address
            .filter(|address| self.by_address.contains_key(address))
            .map(|address| self.reference(&address))
    }

    /// Case-insensitive full-text search over address, title, and body. With
    /// a Card type, only Cards of that type match, and an empty query lists
    /// them all.
    pub(crate) fn search(
        &self,
        query: &str,
        workspace: Option<Workspace>,
        card_type: Option<CardType>,
        limit: usize,
    ) -> Vec<Value> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() && card_type.is_none() {
            return Vec::new();
        }
        self.cards
            .iter()
            .filter(|card| workspace.is_none_or(|workspace| card.workspace == workspace))
            .filter(|card| card_type.is_none_or(|card_type| card.card_type == card_type))
            .filter_map(|card| {
                let parsed = &self.parsed[&card.address];
                let snippet = if needle.is_empty() {
                    excerpt(&parsed.body)
                } else {
                    let haystack = format!("{}\n{}\n{}", card.address, parsed.title, parsed.body);
                    let lower = haystack.to_lowercase();
                    let at = lower.find(&needle)?;
                    let line = lower[..at].matches('\n').count();
                    clip(haystack.lines().nth(line).unwrap_or_default().trim(), 160)
                };
                Some(json!({
                    "address": card.address,
                    "title": parsed.title,
                    "kind": kind(card),
                    "type": card.card_type.id(),
                    "workspace": card.workspace.id(),
                    "snippet": snippet,
                }))
            })
            .take(limit)
            .collect()
    }

    pub(crate) fn broken(&self, workspace: Option<Workspace>) -> Result<Vec<Value>> {
        Ok(self
            .lifecycle
            .broken_links()?
            .into_iter()
            .filter(|broken| {
                workspace.is_none_or(|workspace| {
                    self.workspace(&broken.source_address) == Some(workspace)
                })
            })
            .map(|broken| {
                json!({
                    "source": broken.source_address,
                    "title": broken.source_title,
                    "workspace": self.workspace(&broken.source_address).map(Workspace::id),
                    "target": broken.target,
                    "line": broken.source_line,
                })
            })
            .collect())
    }
}

pub(crate) fn kind(card: &Card) -> &'static str {
    if card.is_lit {
        "literature"
    } else if card.is_topic {
        "topic"
    } else {
        "regular"
    }
}

/// The Zone a tree root opens: Topic trees or Literature trees.
pub(crate) fn zone(card: &Card) -> &'static str {
    if crate::is_literature_tree_id(crate::tree_id(&card.address)) {
        "literature"
    } else {
        "topic"
    }
}

pub(crate) fn excerpt(body: &str) -> String {
    let fenced = crate::link::fenced_code_ranges(body);
    let mut start = 0;
    for raw in body.split_inclusive('\n') {
        let line_start = start;
        start += raw.len();
        if fenced.iter().any(|range| range.contains(&line_start)) {
            continue;
        }
        let line = raw
            .trim()
            .trim_start_matches(['#', '>', '-', '*', '+', ' '])
            .trim();
        if !line.is_empty() {
            let plain = ["**", "__", "~~", "`", "[[", "]]"]
                .iter()
                .fold(line.to_string(), |text, mark| text.replace(mark, ""));
            return clip(&plain, 140);
        }
    }
    String::new()
}

pub(crate) fn clip(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let mut clipped = text.chars().take(max_chars - 1).collect::<String>();
        clipped.push('…');
        clipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpts_skip_headings_markers_and_code() {
        assert_eq!(
            excerpt("```\ncode\n```\n\n> ## Quote line\nrest"),
            "Quote line"
        );
        assert_eq!(
            excerpt("A **bold** `code` see [[0/1]]"),
            "A bold code see 0/1"
        );
        assert_eq!(excerpt(""), "");
    }

    const ROOT_REFUSAL: &str = "only Regular Cards can be Event or Model Cards";

    fn archive() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        temp
    }

    fn type_of(root: &Path, at: &str) -> CardType {
        let conn = crate::open_db(root).expect("open database");
        crate::load_card(&conn, at).expect("card").card_type
    }

    fn text_of(root: &Path, at: &str) -> String {
        let conn = crate::open_db(root).expect("open database");
        crate::load_card(&conn, at).expect("card").text
    }

    #[test]
    fn successors_take_a_card_type_and_edits_change_it_within_access() {
        let temp = archive();
        let root = temp.path();
        let owner = Access::Owner;
        assert_eq!(
            create_topic(root, owner, Workspace::Human, "# Firms\n").unwrap(),
            "0/0"
        );
        assert_eq!(
            create_topic(root, owner, Workspace::Agent, "# Agent\n").unwrap(),
            "1/0"
        );
        let event = "# 1926 GM buys Fisher Body\n\nwhat happened\n";
        assert_eq!(
            create_successor(
                root,
                owner,
                Successor::Direct,
                "0/0",
                CardType::Event,
                event
            )
            .unwrap(),
            "0/1"
        );
        assert_eq!(type_of(root, "0/1"), CardType::Event);
        assert_eq!(type_of(root, "0/0"), CardType::Idea);

        // A type alone changes only the type.
        let before = text_of(root, "0/1");
        edit(root, owner, "0/1", None, Some(CardType::Model), None).unwrap();
        assert_eq!(type_of(root, "0/1"), CardType::Model);
        assert_eq!(text_of(root, "0/1"), before);
        edit(
            root,
            owner,
            "0/1",
            Some("# Renamed\n\nnew body\n"),
            Some(CardType::Idea),
            None,
        )
        .unwrap();
        assert_eq!(type_of(root, "0/1"), CardType::Idea);
        assert!(text_of(root, "0/1").starts_with("# Renamed\n\nnew body\n"));
        edit(root, owner, "0/1", Some("# Renamed\n\nagain\n"), None, None).unwrap();
        assert_eq!(
            type_of(root, "0/1"),
            CardType::Idea,
            "text edits keep the type"
        );

        // Tree roots stay Idea Cards.
        assert_eq!(
            edit(root, owner, "0/0", None, Some(CardType::Event), None)
                .unwrap_err()
                .to_string(),
            ROOT_REFUSAL
        );
        let before = text_of(root, "0/0");
        assert_eq!(
            edit(
                root,
                owner,
                "0/0",
                Some("# Changed\n"),
                Some(CardType::Model),
                None
            )
            .unwrap_err()
            .to_string(),
            ROOT_REFUSAL,
            "a refused type leaves the text alone too"
        );
        assert_eq!(text_of(root, "0/0"), before);
        edit(root, owner, "0/0", None, Some(CardType::Idea), None).unwrap();
        let mut conn = crate::open_db(root).unwrap();
        assert_eq!(
            crate::insert_card(
                &mut conn,
                "9/0",
                true,
                Workspace::Human,
                CardType::Event,
                "# Topic\n"
            )
            .unwrap_err()
            .to_string(),
            ROOT_REFUSAL
        );
        drop(conn);
        create_literature(
            root,
            owner,
            Workspace::Human,
            "@book{Lit2020, title={Lit}}",
            "",
        )
        .unwrap();
        assert_eq!(
            edit(root, owner, "Lit2020", None, Some(CardType::Event), None)
                .unwrap_err()
                .to_string(),
            ROOT_REFUSAL
        );

        // Agent Access changes types only in the agent Workspace.
        let read_only = "the human workspace is read-only for AI agents";
        assert!(
            create_successor(
                root,
                Access::Agent,
                Successor::Side,
                "0/1",
                CardType::Event,
                event
            )
            .unwrap_err()
            .to_string()
            .contains(read_only)
        );
        assert!(
            edit(
                root,
                Access::Agent,
                "0/1",
                None,
                Some(CardType::Event),
                None
            )
            .unwrap_err()
            .to_string()
            .contains(read_only)
        );
        assert_eq!(type_of(root, "0/1"), CardType::Idea);
        assert_eq!(
            create_successor(
                root,
                Access::Agent,
                Successor::Direct,
                "1/0",
                CardType::Model,
                "# GHM property-rights model\n"
            )
            .unwrap(),
            "1/1"
        );
        edit(
            root,
            Access::Agent,
            "1/1",
            None,
            Some(CardType::Event),
            None,
        )
        .unwrap();
        assert_eq!(type_of(root, "1/1"), CardType::Event);
    }

    #[test]
    fn snapshots_report_types_and_search_lists_a_type_without_a_query() {
        let temp = archive();
        let root = temp.path();
        let owner = Access::Owner;
        create_topic(root, owner, Workspace::Human, "# Firms\n").unwrap();
        create_topic(root, owner, Workspace::Agent, "# Agent\n").unwrap();
        for (at, card_type, text) in [
            (
                "0/0",
                CardType::Idea,
                "# Integration idea\n\nsee the event\n",
            ),
            (
                "0/1",
                CardType::Event,
                "# Fisher Body event\n\nGM bought it\n",
            ),
            (
                "0/2",
                CardType::Model,
                "# GHM model\n\nownership and investment\n",
            ),
            ("1/0", CardType::Event, "# Agent event\n\nan agent event\n"),
        ] {
            create_successor(root, owner, Successor::Direct, at, card_type, text).unwrap();
        }
        let snapshot = Snapshot::read(root).unwrap();
        let types = |results: Vec<Value>| {
            results
                .iter()
                .map(|hit| {
                    (
                        hit["address"].as_str().unwrap().to_string(),
                        hit["type"].as_str().unwrap().to_string(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let pairs = |items: &[(&str, &str)]| {
            items
                .iter()
                .map(|(address, card_type)| (address.to_string(), card_type.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            types(snapshot.search("", None, Some(CardType::Event), 30)),
            pairs(&[("0/2", "event"), ("1/1", "event")])
        );
        assert_eq!(
            types(snapshot.search("  ", Some(Workspace::Agent), Some(CardType::Event), 30)),
            pairs(&[("1/1", "event")])
        );
        assert_eq!(
            types(snapshot.search("", None, Some(CardType::Event), 1)),
            pairs(&[("0/2", "event")])
        );
        assert_eq!(
            types(snapshot.search("event", None, Some(CardType::Idea), 30)),
            pairs(&[("0/1", "idea")])
        );
        assert_eq!(
            types(snapshot.search("model", None, None, 30)),
            pairs(&[("0/3", "model")])
        );
        assert!(snapshot.search("", None, None, 30).is_empty());
        assert_eq!(
            snapshot.search("", None, Some(CardType::Model), 30)[0]["snippet"],
            "ownership and investment"
        );

        let summary = snapshot.summary(&snapshot.by_address["0/3"]);
        assert_eq!(summary["type"], "model");
        assert_eq!(
            snapshot.summary(&snapshot.by_address["0/0"])["type"],
            "idea"
        );
        let trees = snapshot.trees();
        assert_eq!(
            (&trees[0]["root"], &trees[0]["events"], &trees[0]["models"]),
            (&json!("0/0"), &json!(1), &json!(1))
        );
        assert_eq!(
            (&trees[1]["root"], &trees[1]["events"], &trees[1]["models"]),
            (&json!("1/0"), &json!(1), &json!(0))
        );
        let before = snapshot.fingerprint();
        edit(root, owner, "0/1", None, Some(CardType::Event), None).unwrap();
        assert_ne!(
            Snapshot::read(root).unwrap().fingerprint(),
            before,
            "a type change alone refreshes open windows"
        );
    }

    #[test]
    fn types_stay_with_cards_through_moves_side_compaction_and_citation_renames() {
        let temp = archive();
        let root = temp.path();
        let owner = Access::Owner;
        create_topic(root, owner, Workspace::Human, "# Firms\n").unwrap();
        create_successor(
            root,
            owner,
            Successor::Direct,
            "0/0",
            CardType::Idea,
            "# Base\n",
        )
        .unwrap();
        for (card_type, title) in [
            (CardType::Event, "# Side event\n"),
            (CardType::Model, "# Side model\n"),
            (CardType::Event, "# Third side\n"),
        ] {
            create_successor(root, owner, Successor::Side, "0/1", card_type, title).unwrap();
        }
        assert_eq!(
            [
                type_of(root, "0/1|a"),
                type_of(root, "0/1|b"),
                type_of(root, "0/1|c")
            ],
            [CardType::Event, CardType::Model, CardType::Event]
        );

        // Deleting 0/1|a compacts 0/1|b and 0/1|c one label down.
        delete(root, owner, "0/1|a", Some("delete")).unwrap();
        assert!(text_of(root, "0/1|a").starts_with("# Side model\n"));
        assert_eq!(type_of(root, "0/1|a"), CardType::Model);
        assert_eq!(type_of(root, "0/1|b"), CardType::Event);

        move_card(root, owner, "0/1|a", "0/2", Some("move")).unwrap();
        assert!(text_of(root, "0/2").starts_with("# Side model\n"));
        assert_eq!(type_of(root, "0/2"), CardType::Model);
        assert_eq!(type_of(root, "0/1|b"), CardType::Event);

        create_literature(
            root,
            owner,
            Workspace::Human,
            "@book{Lit2020, title={Lit}}",
            "",
        )
        .unwrap();
        create_successor(
            root,
            owner,
            Successor::Direct,
            "Lit2020",
            CardType::Event,
            "# Reported event\n",
        )
        .unwrap();
        edit_metadata(
            root,
            owner,
            "Lit2020",
            "@book{Lit2021, title={Lit}}",
            None,
            Some("move"),
        )
        .unwrap();
        assert_eq!(type_of(root, "Lit2021/1"), CardType::Event);
        assert_eq!(type_of(root, "Lit2021"), CardType::Idea);
    }
}
