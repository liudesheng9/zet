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
use std::sync::OnceLock;

use crate::link::{LinkBody, LinkLifecycle};
use crate::model::{Emphasis, Facet, Level, Part};
pub(crate) use crate::place::Target;

/// How long a change waits for another client's short write to finish.
const EDIT_PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);
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
    let _lock = crate::acquire_edit_lock_within(root, EDIT_PATIENCE)?;
    let mut conn = crate::open_db(root)?;
    crate::create_topic(&mut conn, workspace, text)
}

pub(crate) fn create_successor(
    root: &Path,
    access: Access,
    kind: Successor,
    at: &str,
    card_type: CardType,
    part: Option<Part>,
    text: &str,
) -> Result<String> {
    let _lock = crate::acquire_edit_lock_within(root, EDIT_PATIENCE)?;
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
        part.as_ref(),
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
    let _lock = crate::acquire_edit_lock_within(root, EDIT_PATIENCE)?;
    let mut conn = crate::open_db(root)?;
    let metadata = crate::literature::parse_metadata(bibtex)?;
    let parsed = crate::parse_literature_edit_text(text)?;
    let text = crate::compose_card_text(&metadata.title, &parsed.body, "");
    crate::insert_literature_card(&mut conn, &metadata.citation_key, workspace, bibtex, &text)?;
    Ok(metadata.citation_key)
}

/// A change to a Card's type, Model facet, emphasis, and elaboration level.
/// `facet`, `emphasis`, and `level` are `None` to leave them alone and
/// `Some(None)` to clear them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct TypeChange {
    pub(crate) card_type: Option<CardType>,
    pub(crate) facet: Option<Option<Facet>>,
    pub(crate) emphasis: Option<Option<Emphasis>>,
    pub(crate) level: Option<Option<Level>>,
    pub(crate) hub_level: Option<Option<crate::hub::HubLevel>>,
    pub(crate) form: Option<Option<crate::math::MathForm>>,
}

impl TypeChange {
    pub(crate) fn is_empty(&self) -> bool {
        self.card_type.is_none()
            && self.facet.is_none()
            && self.emphasis.is_none()
            && self.level.is_none()
            && self.hub_level.is_none()
            && self.form.is_none()
    }

    /// The Math form the Card has after the change: only a Model Card (no
    /// facet) has one, and it keeps its form unless the change names one.
    pub(crate) fn resolve_form(&self, card: &Card) -> Result<Option<crate::math::MathForm>> {
        let (card_type, facet, _) = self.resolve(card);
        let head = card_type == CardType::Model && facet.is_none();
        match self.form {
            Some(Some(_)) if !head => bail!(crate::math::FORM_NEEDS_MODEL_CARD),
            Some(form) => Ok(form),
            None => Ok(card.form.filter(|_| head)),
        }
    }

    /// The hub level the Card has after the change: only a Hub Card has one,
    /// and it keeps its level unless the change names one.
    pub(crate) fn resolve_hub_level(&self, card: &Card) -> Result<Option<crate::hub::HubLevel>> {
        let (card_type, _, _) = self.resolve(card);
        let hub = card_type == CardType::Hub;
        match self.hub_level {
            Some(Some(_)) if !hub => bail!(crate::hub::LEVEL_NEEDS_HUB_CARD),
            Some(level) => Ok(level),
            None => Ok(card.hub_level.filter(|_| hub)),
        }
    }

    /// The elaboration level the Card has after the change: only a Model Card
    /// (no facet) has one, and it keeps its level unless the change names one.
    pub(crate) fn resolve_level(&self, card: &Card) -> Result<Option<Level>> {
        let (card_type, facet, _) = self.resolve(card);
        let head = card_type == CardType::Model && facet.is_none();
        match self.level {
            Some(Some(_)) if !head => bail!(crate::model::LEVEL_NEEDS_MODEL_CARD),
            Some(level) => Ok(level),
            None => Ok(card.level.filter(|_| head)),
        }
    }

    /// The type, facet, and emphasis the Card has after the change. A facet
    /// makes the Card a Model Card; any other type clears the facet; naming
    /// the Model type alone keeps a part's facet. A Card that is no longer a
    /// part loses its emphasis; a part keeps it unless the change names one.
    pub(crate) fn resolve(&self, card: &Card) -> (CardType, Option<Facet>, Option<Emphasis>) {
        let (card_type, facet) = match (self.card_type, &self.facet) {
            (Some(card_type), _) if card_type != CardType::Model => (card_type, None),
            (_, Some(Some(facet))) => (CardType::Model, Some(facet.clone())),
            (Some(card_type), Some(None)) => (card_type, None),
            (None, Some(None)) => (card.card_type, None),
            (Some(card_type), None) if card.card_type == card_type => {
                (card_type, card.facet.clone())
            }
            (Some(card_type), None) => (card_type, None),
            (None, None) => (card.card_type, card.facet.clone()),
        };
        let emphasis = match self.emphasis {
            Some(emphasis) => emphasis,
            None => card.emphasis.filter(|_| facet.is_some()),
        };
        (card_type, facet, emphasis)
    }
}

/// A `level` argument: a hub level (line, topic, field, register) for a Hub
/// Card, or an elaboration level (concept, formal) for a Model Card. Absent
/// leaves both alone; null, an empty string, or `none` clears both.
#[allow(clippy::type_complexity)]
pub(crate) fn parse_level_arg(
    value: Option<&Value>,
) -> Result<(Option<Option<Level>>, Option<Option<crate::hub::HubLevel>>)> {
    let text = match value {
        None => return Ok((None, None)),
        Some(Value::Null) => return Ok((Some(None), Some(None))),
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(_) => bail!(
            "argument `level` must be concept, formal, line, topic, field, register, or empty"
        ),
    };
    if crate::hub::names_hub_level(&text) {
        return Ok((None, Some(Some(crate::hub::HubLevel::parse(&text)?))));
    }
    match crate::model::parse_optional_level(&text)? {
        Some(level) => Ok((Some(Some(level)), None)),
        None => Ok((Some(None), Some(None))),
    }
}

/// Replace a Card's text, change its Card type, Model facet, emphasis, or
/// elaboration level, or both. With
/// `base`, refuse if the stored text changed since the caller read it.
pub(crate) fn edit(
    root: &Path,
    access: Access,
    at: &str,
    text: Option<&str>,
    change: TypeChange,
    base: Option<&str>,
) -> Result<String> {
    let _lock = crate::acquire_edit_lock_within(root, EDIT_PATIENCE)?;
    let mut conn = crate::open_db(root)?;
    let card = unchanged_card(&conn, at, base)?;
    access.ensure_writable(card.workspace)?;
    let (card_type, facet, emphasis) = change.resolve(&card);
    let level = change.resolve_level(&card)?;
    let hub_level = change.resolve_hub_level(&card)?;
    let form = change.resolve_form(&card)?;
    let retyped = (card_type, &facet, emphasis) != (card.card_type, &card.facet, card.emphasis);
    if retyped {
        crate::card_type::ensure_allowed(&card, card_type)?;
        crate::model::ensure_allowed(&conn, Some(&card), card_type, facet.as_ref(), emphasis)?;
    }
    let tx = conn.transaction()?;
    let mut changed =
        retyped || level != card.level || hub_level != card.hub_level || form != card.form;
    if let Some(text) = text {
        let old = crate::parse_card_text(&card.text)?;
        let mut parsed = if card.is_lit {
            crate::parse_literature_edit_text(text)?
        } else {
            crate::parse_card_text(text)?
        };
        if card.is_lit {
            parsed.title = old.title.clone();
        }
        if parsed.title != old.title || parsed.body != old.body {
            let text = crate::compose_card_text(&parsed.title, &parsed.body, &old.reverse);
            crate::validate_card_text(&tx, &card.address, &text, Some(&card.text))?;
            tx.execute(
                "UPDATE cards SET text=?1 WHERE row_id=?2",
                rusqlite::params![text, card.row_id],
            )?;
            crate::link_index::sync_source(&tx, &card.address)?;
            changed = true;
        }
    }
    if retyped {
        crate::model::set(&tx, &card, card_type, facet.as_ref(), emphasis)?;
    }
    if level != card.level {
        let card = crate::load_card(&tx, &card.address)?;
        crate::model::set_level(&tx, &card, level)?;
    }
    if hub_level != card.hub_level {
        let card = crate::load_card(&tx, &card.address)?;
        crate::hub::set_level(&tx, &card, hub_level)?;
    }
    if form != card.form {
        let card = crate::load_card(&tx, &card.address)?;
        crate::math::set_form(&tx, &card, form)?;
    }
    if changed && crate::link_index::ready(&tx) {
        crate::link_index::bump_revision(&tx)?;
    }
    tx.commit()?;
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
    let _lock = crate::acquire_edit_lock_within(root, EDIT_PATIENCE)?;
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
    into: Option<&str>,
    confirm: Option<&str>,
) -> Result<Outcome> {
    let _lock = crate::acquire_edit_lock_within(root, EDIT_PATIENCE)?;
    let mut conn = crate::open_db(root)?;
    let card = crate::load_card(&conn, at)?;
    access.ensure_writable(card.workspace)?;
    let mut plan = crate::delete_plan(&conn, at)?;
    if let Some(into) = into {
        plan = plan.into(&conn, into)?;
        access.ensure_link_rewrites_allowed(&conn, &plan.repoint_mapping())?;
    }
    access.ensure_link_rewrites_allowed(&conn, &plan.compaction())?;
    if confirm != Some(plan.confirmation.as_str()) {
        return Ok(Outcome::Confirm {
            confirm: plan.confirmation.clone(),
            lines: crate::delete_verification_lines(&plan),
        });
    }
    let result = crate::apply_delete_plan(&mut conn, plan)?;
    let next = crate::parent_location(at)
        .filter(|parent| crate::target_exists(&conn, parent).unwrap_or(false));
    Ok(Outcome::Done(json!({
        "deleted": result.deleted_count,
        "moved": result.moved,
        "address": next,
    })))
}

/// Move a Regular Card inside its Workspace: with its successors, or `alone`
/// (its Direct successor chain closes the gap), to a free Location or into
/// the place of another Card, which moves down.
pub(crate) fn move_card(
    root: &Path,
    access: Access,
    at: &str,
    target: &Target,
    alone: bool,
    confirm: Option<&str>,
) -> Result<Outcome> {
    let _lock = crate::acquire_edit_lock_within(root, EDIT_PATIENCE)?;
    let mut conn = crate::open_db(root)?;
    let card = crate::load_card(&conn, at)?;
    access.ensure_writable(card.workspace)?;
    let plan = crate::place::move_plan(&conn, at, target, alone)?;
    access.ensure_link_rewrites_allowed(&conn, &plan.mapping)?;
    if confirm != Some("move") {
        return Ok(Outcome::Confirm {
            confirm: "move".to_string(),
            lines: crate::move_verification_lines(&plan),
        });
    }
    crate::apply_move_plan(&mut conn, &plan)?;
    let address = plan
        .mapping
        .get(at)
        .cloned()
        .unwrap_or_else(|| at.to_string());
    Ok(Outcome::Done(
        json!({ "address": address, "moved": plan.mapping }),
    ))
}

/// Insert a new Card at the place of the Regular Card `before`, which moves
/// down with its subtree and becomes the new Card's Direct successor. Links in
/// `text` name today's addresses.
pub(crate) fn insert_card(
    root: &Path,
    access: Access,
    before: &str,
    card_type: CardType,
    part: Option<Part>,
    text: &str,
    confirm: Option<&str>,
) -> Result<Outcome> {
    let _lock = crate::acquire_edit_lock_within(root, EDIT_PATIENCE)?;
    let mut conn = crate::open_db(root)?;
    let card = crate::load_card(&conn, before)?;
    access.ensure_writable(card.workspace)?;
    let plan = crate::place::insert_plan(&conn, before)?;
    access.ensure_link_rewrites_allowed(&conn, &plan.mapping)?;
    crate::model::ensure_allowed(
        &conn,
        None,
        card_type,
        part.as_ref().map(|part| &part.facet),
        part.as_ref().and_then(|part| part.emphasis),
    )?;
    crate::validate_card_text(&conn, &plan.location, text, None)?;
    if confirm != Some("insert") {
        return Ok(Outcome::Confirm {
            confirm: "insert".to_string(),
            lines: crate::place::insert_verification_lines(&plan),
        });
    }
    crate::place::apply_insert_plan(&mut conn, &plan, card_type, part.as_ref(), text)?;
    Ok(Outcome::Done(
        json!({ "address": plan.location, "moved": plan.mapping }),
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
    /// Each Card's address, title, and body in lower case, in `cards` order, made on the
    /// first search; a Snapshot kept across requests then folds case only once.
    folded: OnceLock<Vec<Folded>>,
}

struct Folded {
    address: String,
    title: String,
    body: String,
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
            folded: OnceLock::new(),
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
            hasher.update(card.facet.as_ref().map_or("", Facet::id).as_bytes());
            hasher.update([0]);
            hasher.update(card.emphasis.map_or("", Emphasis::id).as_bytes());
            hasher.update([0]);
            hasher.update(card.level_id().unwrap_or_default().as_bytes());
            hasher.update([0]);
            hasher.update(card.form.map_or("", crate::math::MathForm::id).as_bytes());
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
        // What each Link says, where its words announce a kind; a plain reference is left out.
        let mut kinds = serde_json::Map::new();
        for read in crate::model::read_links(&self.parsed[&card.address].body, str::to_string) {
            if read.kind != crate::model::RelationKind::Refers && links.contains(&read.target) {
                kinds
                    .entry(read.target)
                    .or_insert_with(|| Value::String(read.kind.id().to_string()));
            }
        }
        json!({
            "address": card.address,
            "tree": crate::tree_id(&card.address),
            "workspace": card.workspace.id(),
            "title": self.title(&card.address),
            "kind": kind(card),
            "type": card.card_type.id(),
            "facet": card.facet.as_ref().map(Facet::id),
            "emphasis": card.emphasis.map(Emphasis::id),
            "level": card.level_id(),
            "form": card.form.map(crate::math::MathForm::id),
            "head": self.model_head(card),
            "edge": edge,
            "parent": parent,
            "excerpt": excerpt(&self.parsed[&card.address].body),
            "links": links,
            "kinds": kinds,
            "inbound": self.inbound.get(&card.address).map_or(0, BTreeSet::len),
        })
    }

    /// Every tree root, with its Workspace, Zone, size, and how many Event
    /// Cards, Finding Cards, and models (Model Cards, not counting their
    /// parts) it holds.
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
                // A model counts once: its Model Card, not its parts.
                let typed = |card_type: CardType| {
                    members
                        .iter()
                        .filter(|member| member.card_type == card_type && member.facet.is_none())
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
                    "findings": typed(CardType::Finding),
                    "models": typed(CardType::Model),
                })
            })
            .collect()
    }

    /// The Model Card a Model part belongs to first (the one it is filed
    /// under, else the first that links it); `None` for any other Card.
    pub(crate) fn model_head(&self, card: &Card) -> Option<String> {
        card.facet.as_ref()?;
        crate::model::heads(&card.address, &self.by_address, &self.inbound)
            .into_iter()
            .next()
    }

    /// The structure of the model a Card belongs to: its Model Card, its
    /// parts, the kinded relations between them, its loose parts, and its
    /// Links with Cards outside it.
    pub(crate) fn model(&self, address: &str) -> Option<Value> {
        crate::model::structure(address, &self.by_address, &self.parsed, &self.inbound)
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

    fn folded(&self) -> &[Folded] {
        self.folded.get_or_init(|| {
            self.cards
                .iter()
                .map(|card| {
                    let parsed = &self.parsed[&card.address];
                    Folded {
                        address: card.address.to_lowercase(),
                        title: parsed.title.to_lowercase(),
                        body: parsed.body.to_lowercase(),
                    }
                })
                .collect()
        })
    }

    /// The GUI's ranked search. Every word of the query must occur in the
    /// address, title, or body; a hit in the title or address outranks a hit
    /// in the body, and tree roots and Hub Cards (ways in) rank a little
    /// higher. Each hit names its tree and where it matched, and its snippet
    /// shows the body around the first word the body holds. Besides the hits,
    /// it counts the matches by Workspace (under the type filter) and by
    /// Card type (under the Workspace filter), so a filter shows what it
    /// would give before it is chosen.
    pub(crate) fn find(
        &self,
        query: &str,
        workspace: Option<Workspace>,
        card_type: Option<CardType>,
        limit: usize,
    ) -> Value {
        let needle = query.trim().to_lowercase();
        let terms = needle.split_whitespace().collect::<Vec<_>>();
        let mut matches = Vec::new();
        if !terms.is_empty() {
            for (order, (card, folded)) in self.cards.iter().zip(self.folded()).enumerate() {
                let parsed = &self.parsed[&card.address];
                let Folded {
                    address,
                    title,
                    body,
                } = folded;
                let mut score = 0_i64;
                let mut field = "body";
                let mut body_term = None;
                let mut all = true;
                for term in &terms {
                    let in_title = title.contains(term);
                    let in_address = address.contains(term);
                    let in_body = body.matches(term).take(5).count() as i64;
                    if !in_title && !in_address && in_body == 0 {
                        all = false;
                        break;
                    }
                    if in_title {
                        score += if title.starts_with(term) { 300 } else { 200 };
                        field = "title";
                    }
                    if in_address {
                        score += if address == *term {
                            800
                        } else if address.starts_with(term) {
                            120
                        } else {
                            40
                        };
                        if field == "body" {
                            field = "address";
                        }
                    }
                    if in_body > 0 {
                        score += 20 + 2 * in_body;
                        body_term.get_or_insert(*term);
                    }
                }
                if !all {
                    continue;
                }
                // The whole title, written with or without the spaces between the words.
                if *title == needle
                    || title.split_whitespace().collect::<String>() == terms.concat()
                {
                    score += 1000;
                }
                if crate::is_tree_root(card) || card.card_type == CardType::Hub {
                    score += 40;
                }
                // A short title that holds the words says more about them than a long one.
                score -= parsed.title.chars().count() as i64 / 8;
                matches.push((score, order, card, field, body_term));
            }
        }
        matches.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut workspaces = serde_json::Map::new();
        let mut types = serde_json::Map::new();
        for (_, _, card, _, _) in &matches {
            if card_type.is_none_or(|card_type| card.card_type == card_type) {
                let count = workspaces.entry(card.workspace.id()).or_insert(json!(0));
                *count = json!(count.as_u64().unwrap_or(0) + 1);
            }
            if workspace.is_none_or(|workspace| card.workspace == workspace) {
                let count = types.entry(card.card_type.id()).or_insert(json!(0));
                *count = json!(count.as_u64().unwrap_or(0) + 1);
            }
        }
        let kept = matches
            .into_iter()
            .filter(|(_, _, card, _, _)| {
                workspace.is_none_or(|workspace| card.workspace == workspace)
                    && card_type.is_none_or(|card_type| card.card_type == card_type)
            })
            .collect::<Vec<_>>();
        let total = kept.len();
        let results = kept
            .into_iter()
            .take(limit)
            .map(|(score, _, card, field, body_term)| {
                let body = &self.parsed[&card.address].body;
                let tree = crate::tree_id(&card.address);
                json!({
                    "address": card.address,
                    "title": self.title(&card.address),
                    "kind": kind(card),
                    "type": card.card_type.id(),
                    "facet": card.facet.as_ref().map(Facet::id),
                    "emphasis": card.emphasis.map(Emphasis::id),
                    "level": card.level_id(),
                    "form": card.form.map(crate::math::MathForm::id),
                    "workspace": card.workspace.id(),
                    "tree": tree,
                    "tree_title": self.title(&crate::tree_root_address(tree)),
                    "field": field,
                    "score": score,
                    "snippet": body_term
                        .and_then(|term| context_snippet(body, term))
                        .unwrap_or_else(|| excerpt(body)),
                })
            })
            .collect::<Vec<_>>();
        json!({ "results": results, "total": total, "workspaces": workspaces, "types": types })
    }

    /// Case-insensitive full-text search over address, title, and body. With
    /// a Card type, only Cards of that type match, and an empty query lists
    /// them all.
    #[cfg(test)]
    pub(crate) fn search(
        &self,
        query: &str,
        workspace: Option<Workspace>,
        card_type: Option<CardType>,
        limit: usize,
    ) -> Vec<Value> {
        self.search_in(query, workspace, card_type, None, limit)
    }

    /// [`Snapshot::search`] that may keep only the Model Cards of one Math form;
    /// with a form, an empty query lists them all.
    pub(crate) fn search_in(
        &self,
        query: &str,
        workspace: Option<Workspace>,
        card_type: Option<CardType>,
        form: Option<crate::math::MathForm>,
        limit: usize,
    ) -> Vec<Value> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() && card_type.is_none() && form.is_none() {
            return Vec::new();
        }
        self.cards
            .iter()
            .filter(|card| workspace.is_none_or(|workspace| card.workspace == workspace))
            .filter(|card| card_type.is_none_or(|card_type| card.card_type == card_type))
            .filter(|card| form.is_none_or(|form| card.form == Some(form)))
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
                    "facet": card.facet.as_ref().map(Facet::id),
                    "emphasis": card.emphasis.map(Emphasis::id),
                    "level": card.level_id(),
                    "form": card.form.map(crate::math::MathForm::id),
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
        // Code, charts, and raw HTML are not prose to preview.
        if fenced.iter().any(|range| range.contains(&line_start))
            || raw.trim_start().starts_with('<')
            || raw.trim_start().starts_with("![")
        {
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

/// The body line that holds `term` (already lower case), cleaned of Markdown
/// marks and cut to a window of about 150 characters around the term.
fn context_snippet(body: &str, term: &str) -> Option<String> {
    let lower = body.to_lowercase();
    let at = lower.find(term)?;
    let line = body.lines().nth(lower[..at].matches('\n').count())?;
    let line = line
        .trim()
        .trim_start_matches(['#', '>', '-', '*', '+', '|', ' '])
        .trim();
    let plain = ["**", "__", "~~", "`", "[[", "]]"]
        .iter()
        .fold(line.to_string(), |text, mark| text.replace(mark, ""));
    let chars = plain.chars().collect::<Vec<_>>();
    let found = plain
        .to_lowercase()
        .find(term)
        .map_or(0, |at| plain.to_lowercase()[..at].chars().count());
    let start = found.min(chars.len()).saturating_sub(40);
    let end = (start + 150).min(chars.len());
    let mut window = chars[start..end].iter().collect::<String>();
    if start > 0 {
        window.insert(0, '…');
    }
    if end < chars.len() {
        window.push('…');
    }
    Some(window)
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

    fn ty(card_type: CardType) -> TypeChange {
        TypeChange {
            card_type: Some(card_type),
            ..TypeChange::default()
        }
    }

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
        assert_eq!(
            excerpt("![Figure 1](artifact:abc)\n\nThe figure shows"),
            "The figure shows"
        );
        assert_eq!(
            excerpt(
                "<div class=\"chart\">
<svg></svg>
</div>

The model"
            ),
            "The model"
        );
    }

    const ROOT_REFUSAL: &str = "only Regular Cards can be Event, Finding, Model, or Hub Cards";

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

    fn facet_of(root: &Path, at: &str) -> Option<String> {
        let conn = crate::open_db(root).expect("open database");
        crate::load_card(&conn, at)
            .expect("card")
            .facet
            .map(|facet| facet.id().to_string())
    }

    fn emphasis_of(root: &Path, at: &str) -> Option<Emphasis> {
        let conn = crate::open_db(root).expect("open database");
        crate::load_card(&conn, at).expect("card").emphasis
    }

    fn label(value: &str) -> Option<Facet> {
        Some(Facet::parse(value).unwrap())
    }

    fn part(facet: &str) -> TypeChange {
        TypeChange {
            facet: Some(label(facet)),
            ..TypeChange::default()
        }
    }

    fn stress(emphasis: Option<Emphasis>) -> TypeChange {
        TypeChange {
            emphasis: Some(emphasis),
            ..TypeChange::default()
        }
    }

    #[test]
    fn model_parts_are_filed_under_their_model_card_and_form_its_structure() {
        let temp = archive();
        let root = temp.path();
        let owner = Access::Owner;
        let make = |kind, at: &str, card_type, facet: Option<Facet>, text: &str| {
            create_successor(
                root,
                owner,
                kind,
                at,
                card_type,
                facet.map(|facet| Part {
                    facet,
                    emphasis: None,
                }),
                text,
            )
        };
        create_topic(root, owner, Workspace::Agent, "# Firms\n").unwrap();
        make(
            Successor::Direct,
            "0/0",
            CardType::Model,
            None,
            "# GHM\n\nthe model\n",
        )
        .unwrap();
        assert_eq!(
            make(
                Successor::Side,
                "0/1",
                CardType::Model,
                label("Setting"),
                "# Setting\n\nof [[0/1]]\n"
            )
            .unwrap(),
            "0/1|a"
        );
        // The author names a part's facet; it need not be a common facet.
        assert_eq!(
            make(
                Successor::Direct,
                "0/1|a",
                CardType::Model,
                label("讨价还价"),
                "# Bargaining\n\nof [[0/1]]. 它依赖设定 [[0/1|a]]：双方各有一项资产。\n"
            )
            .unwrap(),
            "0/1|a|1"
        );
        assert_eq!(facet_of(root, "0/1|a").as_deref(), Some("setting"));
        assert_eq!(facet_of(root, "0/1|a|1").as_deref(), Some("讨价还价"));
        // A part may carry an emphasis from the start.
        assert_eq!(
            create_successor(
                root,
                owner,
                Successor::Side,
                "0/1",
                CardType::Model,
                Some(Part {
                    facet: Facet::parse("a worked example").unwrap(),
                    emphasis: Some(Emphasis::Aside),
                }),
                "# Example\n\nof [[0/1]]\n",
            )
            .unwrap(),
            "0/1|b"
        );
        assert_eq!(emphasis_of(root, "0/1|b"), Some(Emphasis::Aside));

        // A part may be filed anywhere; until it is filed under a Model Card or
        // linked from one, it belongs to no model. Only Model Cards have
        // facets, and only parts have an emphasis.
        make(
            Successor::Direct,
            "0/1",
            CardType::Idea,
            None,
            "# An idea\n",
        )
        .unwrap();
        assert_eq!(
            make(
                Successor::Direct,
                "0/2",
                CardType::Model,
                label("results"),
                "# Stray\n"
            )
            .unwrap(),
            "0/3"
        );
        let stray = Snapshot::read(root).unwrap().model("0/3").unwrap();
        assert_eq!(stray["head"], Value::Null);
        assert_eq!(
            stray["issues"],
            json!([
                "`0/3` is a Model part (results) but belongs to no model: file it under a Model Card, or link it from one"
            ])
        );
        assert_eq!(
            make(
                Successor::Side,
                "0/1",
                CardType::Event,
                label("results"),
                "# Event\n"
            )
            .unwrap_err()
            .to_string(),
            "only Model Cards can have a Model facet"
        );
        assert_eq!(
            edit(root, owner, "0/1", None, stress(Some(Emphasis::Core)), None)
                .unwrap_err()
                .to_string(),
            "only Model parts can have an emphasis; give the Card a Model facet first"
        );

        let snapshot = Snapshot::read(root).unwrap();
        let model = snapshot.model("0/1|a|1").expect("a Model part");
        assert_eq!(model["head"]["address"], "0/1");
        assert_eq!(model["facet"], "讨价还价");
        let parts = model["parts"].as_array().unwrap();
        assert_eq!(
            parts
                .iter()
                .map(|part| (
                    part["address"].as_str().unwrap(),
                    part["facet"].as_str().unwrap(),
                    part["emphasis"].as_str()
                ))
                .collect::<Vec<_>>(),
            [
                ("0/1|a", "setting", None),
                ("0/1|a|1", "讨价还价", None),
                ("0/1|b", "a worked example", Some("aside"))
            ]
        );
        assert_eq!(parts[1]["links"], json!(["0/1", "0/1|a"]));
        // Parts relate by their Links, each with its kind and the sentence that
        // makes it; a part in no relation is loose. Neither the Model Card nor
        // its parts need to link each other.
        assert_eq!(
            model["relations"],
            json!([{ "from": "0/1|a|1", "to": "0/1|a", "kind": "rests-on", "note": "它依赖设定 「setting」：双方各有一项资产。" }])
        );
        assert_eq!(model["loose"], json!(["0/1|b"]));
        assert_eq!(model["issues"], json!([]));
        assert_eq!(
            model["models"],
            json!([{ "address": "0/1", "title": "GHM" }])
        );
        assert_eq!(
            snapshot.summary(snapshot.by_address.get("0/1|a").unwrap())["head"],
            "0/1"
        );
        assert!(snapshot.model("0/2").is_none());
        assert_eq!(snapshot.trees()[0]["models"], 1);

        // The Model Card names its parts in its own order, and a Link from it
        // takes in the stray part filed elsewhere.
        edit(
            root,
            owner,
            "0/1",
            Some("# GHM\n\nparts: [[0/1|b]] [[0/1|a]] [[0/3]]\n"),
            TypeChange::default(),
            None,
        )
        .unwrap();
        let snapshot = Snapshot::read(root).unwrap();
        let model = snapshot.model("0/1").unwrap();
        assert_eq!(
            model["parts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|part| (
                    part["address"].as_str().unwrap(),
                    part["filed"].as_bool().unwrap()
                ))
                .collect::<Vec<_>>(),
            [
                ("0/1|b", true),
                ("0/1|a", true),
                ("0/3", false),
                ("0/1|a|1", true)
            ]
        );
        assert_eq!(model["head_links"], json!(["0/1|a", "0/1|b", "0/3"]));
        assert_eq!(snapshot.model("0/3").unwrap()["head"]["address"], "0/1");
        edit(
            root,
            owner,
            "0/1",
            Some("# GHM\n\nparts: [[0/1|a]] [[0/1|a|1]] [[0/1|b]]\n"),
            TypeChange::default(),
            None,
        )
        .unwrap();
        edit(root, owner, "0/3", None, ty(CardType::Idea), None).unwrap();
        let snapshot = Snapshot::read(root).unwrap();

        // An emphasis changes alone and refreshes open windows; it survives a
        // new facet.
        let before = snapshot.fingerprint();
        edit(
            root,
            owner,
            "0/1|a|1",
            None,
            stress(Some(Emphasis::Core)),
            None,
        )
        .unwrap();
        assert_eq!(emphasis_of(root, "0/1|a|1"), Some(Emphasis::Core));
        assert_ne!(Snapshot::read(root).unwrap().fingerprint(), before);
        edit(root, owner, "0/1|a|1", None, part("dynamics"), None).unwrap();
        assert_eq!(
            (
                facet_of(root, "0/1|a|1").as_deref(),
                emphasis_of(root, "0/1|a|1")
            ),
            (Some("dynamics"), Some(Emphasis::Core))
        );

        // A Model Card with parts cannot stop heading them.
        assert_eq!(
            edit(root, owner, "0/1", None, ty(CardType::Idea), None)
                .unwrap_err()
                .to_string(),
            "`0/1` is the Model Card of 3 Model part(s) (0/1|a, 0/1|a|1, 0/1|b); change or move them first"
        );
        // A facet alone makes a Card a Model part; an empty facet makes it a
        // Model Card again and drops its emphasis.
        make(
            Successor::Side,
            "0/1",
            CardType::Idea,
            None,
            "# Results\n\nof [[0/1]]\n",
        )
        .unwrap();
        edit(root, owner, "0/1|c", None, part("results"), None).unwrap();
        assert_eq!(
            (type_of(root, "0/1|c"), facet_of(root, "0/1|c").as_deref()),
            (CardType::Model, Some("results"))
        );
        let cleared = TypeChange {
            facet: Some(None),
            ..TypeChange::default()
        };
        edit(root, owner, "0/1|b", None, cleared, None).unwrap();
        assert_eq!(
            (
                type_of(root, "0/1|b"),
                facet_of(root, "0/1|b"),
                emphasis_of(root, "0/1|b")
            ),
            (CardType::Model, None, None)
        );
        // Naming the Model type alone keeps a part's facet; another type clears it.
        edit(root, owner, "0/1|a|1", None, ty(CardType::Model), None).unwrap();
        assert_eq!(facet_of(root, "0/1|a|1").as_deref(), Some("dynamics"));
        edit(root, owner, "0/1|a|1", None, ty(CardType::Idea), None).unwrap();
        assert_eq!(
            (
                type_of(root, "0/1|a|1"),
                facet_of(root, "0/1|a|1"),
                emphasis_of(root, "0/1|a|1")
            ),
            (CardType::Idea, None, None)
        );
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
                None,
                event
            )
            .unwrap(),
            "0/1"
        );
        assert_eq!(type_of(root, "0/1"), CardType::Event);
        assert_eq!(type_of(root, "0/0"), CardType::Idea);

        // A type alone changes only the type.
        let before = text_of(root, "0/1");
        edit(root, owner, "0/1", None, ty(CardType::Model), None).unwrap();
        assert_eq!(type_of(root, "0/1"), CardType::Model);
        assert_eq!(text_of(root, "0/1"), before);
        edit(
            root,
            owner,
            "0/1",
            Some("# Renamed\n\nnew body\n"),
            ty(CardType::Idea),
            None,
        )
        .unwrap();
        assert_eq!(type_of(root, "0/1"), CardType::Idea);
        assert!(text_of(root, "0/1").starts_with("# Renamed\n\nnew body\n"));
        edit(
            root,
            owner,
            "0/1",
            Some("# Renamed\n\nagain\n"),
            TypeChange::default(),
            None,
        )
        .unwrap();
        assert_eq!(
            type_of(root, "0/1"),
            CardType::Idea,
            "text edits keep the type"
        );

        // Tree roots stay Idea Cards.
        assert_eq!(
            edit(root, owner, "0/0", None, ty(CardType::Event), None)
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
                ty(CardType::Model),
                None
            )
            .unwrap_err()
            .to_string(),
            ROOT_REFUSAL,
            "a refused type leaves the text alone too"
        );
        assert_eq!(text_of(root, "0/0"), before);
        edit(root, owner, "0/0", None, ty(CardType::Idea), None).unwrap();
        let mut conn = crate::open_db(root).unwrap();
        assert_eq!(
            crate::insert_card(
                &mut conn,
                "9/0",
                true,
                Workspace::Human,
                CardType::Event,
                None,
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
            edit(root, owner, "Lit2020", None, ty(CardType::Event), None)
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
                None,
                event
            )
            .unwrap_err()
            .to_string()
            .contains(read_only)
        );
        assert!(
            edit(root, Access::Agent, "0/1", None, ty(CardType::Event), None)
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
                None,
                "# GHM property-rights model\n"
            )
            .unwrap(),
            "1/1"
        );
        edit(root, Access::Agent, "1/1", None, ty(CardType::Event), None).unwrap();
        assert_eq!(type_of(root, "1/1"), CardType::Event);
    }

    #[test]
    fn find_ranks_titles_first_needs_every_word_and_counts_by_filter() {
        let temp = archive();
        let root = temp.path();
        let owner = Access::Owner;
        create_topic(root, owner, Workspace::Human, "# Market failure\n").unwrap();
        create_topic(root, owner, Workspace::Agent, "# Agent\n").unwrap();
        let filler = "words before the point ".repeat(4);
        for (at, card_type, text) in [
            (
                "0/0",
                CardType::Idea,
                format!("# Prices\n\n{filler}a market failure appears with an externality\n"),
            ),
            (
                "0/1",
                CardType::Model,
                "# A market failure model\n\nthe setting\n".to_string(),
            ),
            (
                "1/0",
                CardType::Event,
                "# Failure event\n\nthe market crashed\n".to_string(),
            ),
            (
                "1/1",
                CardType::Idea,
                "# Market only\n\nno other word\n".to_string(),
            ),
        ] {
            create_successor(root, owner, Successor::Direct, at, card_type, None, &text).unwrap();
        }
        let snapshot = Snapshot::read(root).unwrap();
        let addresses = |found: &Value| {
            found["results"]
                .as_array()
                .unwrap()
                .iter()
                .map(|hit| hit["address"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        };

        // The exact title first, then a title holding both words, then body hits.
        let found = snapshot.find("Market   FAILURE", None, None, 10);
        assert_eq!(addresses(&found), ["0/0", "0/2", "1/1", "0/1"]);
        assert_eq!(found["total"], 4);
        assert_eq!(found["workspaces"], json!({ "human": 3, "agent": 1 }));
        assert_eq!(found["types"], json!({ "idea": 2, "model": 1, "event": 1 }));
        let body_hit = &found["results"][3];
        assert_eq!(body_hit["field"], "body");
        assert_eq!(body_hit["tree"], "0");
        assert_eq!(body_hit["tree_title"], "Market failure");
        let snippet = body_hit["snippet"].as_str().unwrap();
        assert!(snippet.starts_with('…') && snippet.contains("a market failure appears"));

        // A filter keeps the other filter's counts honest.
        let agent = snapshot.find("market failure", Some(Workspace::Agent), None, 10);
        assert_eq!(addresses(&agent), ["1/1"]);
        assert_eq!(agent["total"], 1);
        assert_eq!(agent["workspaces"], json!({ "human": 3, "agent": 1 }));
        assert_eq!(agent["types"], json!({ "event": 1 }));
        assert_eq!(
            addresses(&snapshot.find("market failure", None, Some(CardType::Model), 10)),
            ["0/2"]
        );
        assert_eq!(snapshot.find("  ", None, None, 10)["total"], 0);
        assert_eq!(snapshot.find("market failure", None, None, 2)["total"], 4);
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
            create_successor(root, owner, Successor::Direct, at, card_type, None, text).unwrap();
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
        edit(root, owner, "0/1", None, ty(CardType::Event), None).unwrap();
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
            None,
            "# Base\n",
        )
        .unwrap();
        for (card_type, title) in [
            (CardType::Event, "# Side event\n"),
            (CardType::Model, "# Side model\n"),
            (CardType::Event, "# Third side\n"),
        ] {
            create_successor(root, owner, Successor::Side, "0/1", card_type, None, title).unwrap();
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
        delete(root, owner, "0/1|a", None, Some("delete")).unwrap();
        assert!(text_of(root, "0/1|a").starts_with("# Side model\n"));
        assert_eq!(type_of(root, "0/1|a"), CardType::Model);
        assert_eq!(type_of(root, "0/1|b"), CardType::Event);

        move_card(
            root,
            owner,
            "0/1|a",
            &Target::To("0/2".into()),
            false,
            Some("move"),
        )
        .unwrap();
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
            None,
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

    #[test]
    fn a_change_waits_for_another_short_write_to_finish() {
        let temp = archive();
        let root = temp.path().to_path_buf();
        let lock = crate::acquire_edit_lock(&root).unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            drop(lock);
        });
        assert_eq!(
            create_topic(&root, Access::Owner, Workspace::Agent, "# Waits\n").unwrap(),
            "0/0"
        );
        release.join().unwrap();
    }

    #[test]
    fn spreads_follow_their_cards_through_moves_deletes_and_citation_renames() {
        let temp = archive();
        let root = temp.path();
        let owner = Access::Owner;
        create_topic(root, owner, Workspace::Human, "# Firms\n").unwrap();
        for (at, title) in [("0/0", "# Base\n"), ("0/1", "# Next\n")] {
            create_successor(
                root,
                owner,
                Successor::Direct,
                at,
                CardType::Idea,
                None,
                title,
            )
            .unwrap();
        }
        for title in ["# Side a\n", "# Side b\n"] {
            create_successor(
                root,
                owner,
                Successor::Side,
                "0/1",
                CardType::Idea,
                None,
                title,
            )
            .unwrap();
        }
        create_literature(
            root,
            owner,
            Workspace::Human,
            "@book{Lit2020, title={Lit}}",
            "",
        )
        .unwrap();
        let spread = |cards: &[&str]| {
            json!({ "current": "s", "spreads": [{ "id": "s", "name": "Set", "cards": cards
                .iter()
                .map(|at| json!({ "at": at, "x": 0, "y": 0 }))
                .collect::<Vec<_>>() }] })
        };
        let lying = || -> Vec<String> {
            let conn = crate::open_db(root).unwrap();
            crate::spread::load(&conn).unwrap()["spreads"][0]["cards"]
                .as_array()
                .unwrap()
                .iter()
                .map(|card| card["at"].as_str().unwrap().to_string())
                .collect()
        };
        crate::spread::save(
            &crate::open_db(root).unwrap(),
            &spread(&["0/2", "0/1|a", "0/1|b", "Lit2020"]),
        )
        .unwrap();

        // Deleting 0/1|a takes it off the Table and compacts 0/1|b to 0/1|a.
        delete(root, owner, "0/1|a", None, Some("delete")).unwrap();
        assert_eq!(lying(), ["0/2", "0/1|a", "Lit2020"]);
        move_card(
            root,
            owner,
            "0/1|a",
            &Target::To("0/3".into()),
            false,
            Some("move"),
        )
        .unwrap();
        assert_eq!(lying(), ["0/2", "0/3", "Lit2020"]);
        edit_metadata(
            root,
            owner,
            "Lit2020",
            "@book{Lit2021, title={Lit}}",
            None,
            Some("move"),
        )
        .unwrap();
        assert_eq!(lying(), ["0/2", "0/3", "Lit2021"]);
    }
}
