//! Per-request Card projections. Bodies and domain calculations remain authoritative.
use crate::{Access, Card, ops::Snapshot};
use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug)]
pub(crate) struct Limit {
    pub(crate) kind: &'static str,
    pub(crate) limit: usize,
    pub(crate) actual: usize,
}
impl std::fmt::Display for Limit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "read limit exceeded for {}: {} > {}; reduce the requested chunk",
            self.kind, self.actual, self.limit
        )
    }
}
impl std::error::Error for Limit {}
impl Limit {
    pub(crate) fn value(&self) -> Value {
        json!({"error":self.to_string(),"code":"read_limit","limit_kind":self.kind,"limit":self.limit,"actual":self.actual,"complete":false})
    }
}

/// The optional label projection of a public read (R0273–R0275, S12 / #154; design 10.4). `Raw` is
/// the default and keeps the authored Term-link macros byte for byte; `Label` returns the visible
/// prose a reader sees through the one shared projection. Every projected reply carries the marker,
/// so a consumer can never mistake projected text for an edit base or a fingerprint input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TermLinks {
    Raw,
    Label,
}

impl TermLinks {
    /// The response marker a projected read carries; the only value the read mode accepts besides
    /// `raw`.
    pub(crate) const MARKER: &'static str = "label";

    fn project(self, text: &str) -> String {
        match self {
            Self::Raw => text.to_string(),
            Self::Label => zt_check::text::strip_term_links(text),
        }
    }

    fn mark(self, value: &mut Value) {
        if self == Self::Label {
            value["term_links"] = json!(Self::MARKER);
        }
    }
}

pub(crate) fn text_value(
    card: &Card,
    lock: &crate::tree_lock::LockView,
    term_links: TermLinks,
) -> Result<Value> {
    let parsed = crate::parse_card_text(&card.text)?;
    let mut value = json!({
        "address":card.address,"tree":crate::tree_id(&card.address),
        "workspace":card.workspace.id(),
        "writable":lock.writable_for(card.row_id, Access::Agent, card.workspace),
        "kind":crate::ops::kind(card),"type":card.card_type.id(),
        // The exact Term name is an authored fact of a Term Card (S07, #149); later slices add the
        // computed role, list, and count facts beside it.
        "term":card.term_name,
        "facet":card.facet.as_ref().map(crate::model::Facet::id),
        "emphasis":card.emphasis.map(crate::model::Emphasis::id),
        "level":card.level_id(),"form":card.form.map(crate::math::MathForm::id),
        "title":parsed.title,"body":term_links.project(&parsed.body),"text":term_links.project(&card.text),
        "bibtex":card.bibtex,"citation_key":card.citation_key,
        // The derived navigation role of a restatement entry (D086) and the exact statement
        // fingerprint folding compares (§4.5, D087). Both are read from the raw authored text in
        // either mode, so a projected body never feeds a fingerprint (S12-AC04).
        "restatement":crate::entry::is_restatement_entry(&parsed.body),
        "statement_fingerprint":crate::card::statement_fingerprint(&parsed.body)
    });
    term_links.mark(&mut value);
    crate::tree_lock::decorate_card(&mut value, card.row_id, lock);
    Ok(value)
}

pub(crate) fn card_value(snapshot: &Snapshot, at: &str, term_links: TermLinks) -> Result<Value> {
    let card = snapshot
        .by_address
        .get(at)
        .with_context(|| format!("card `{at}` does not exist"))?;
    let mut value = text_value(card, snapshot.lock(), term_links)?;
    value["model"] = json!(snapshot.model(at));
    value["hubs"] = json!(crate::hub::hubs_of(snapshot, at));
    value["parent"] = json!(snapshot.nav(crate::parent_location(at)));
    value["direct"] = json!(snapshot.nav(crate::direct_successor(at).ok()));
    value["sides"] = json!(snapshot.sides(card));
    value["outbound"] = json!(snapshot.outbound(at));
    value["inbound"] = json!(snapshot.inbound_refs(at));
    Ok(value)
}

fn existing(conn: &Connection, at: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cards WHERE location=?1 OR citation_key=?1)",
        [at],
        |r| r.get(0),
    )?)
}

/// Cheap authored-body/index facts for scoped clients, without model or hub
/// projections and without fetching a whole tree to recover its summaries. The
/// caller passes the Lock view of the same read, so a lean read and a full read
/// answer the same Lock facts at one revision.
pub(crate) fn text_facts(
    conn: &Connection,
    card: &Card,
    lock: &crate::tree_lock::LockView,
    term_links: TermLinks,
) -> Result<Value> {
    let mut value = text_value(card, lock, term_links)?;
    // The excerpt is derived from the body this read returns, so a projected read previews the same
    // visible prose it names (S12-AC03).
    value["excerpt"] = json!(crate::ops::excerpt(
        value["body"].as_str().unwrap_or_default()
    ));
    let mut query=conn.prepare_cached("SELECT l.target,l.kind,t.row_id IS NOT NULL FROM links l LEFT JOIN cards t ON t.location=l.target OR t.citation_key=l.target WHERE l.source=?1 ORDER BY l.ord")?;
    let rows = query.query_map([&card.address], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, bool>(2)?))
    })?;
    let mut links = Vec::new();
    let mut seen = BTreeSet::new();
    let mut kinds = serde_json::Map::new();
    // The Term uses of this Card, apart from its reasoning kinds (S16, #158; design 10.2): a Term
    // use is named here so a graph reader can hide it, and never satisfies a reasoning count.
    let mut term_use_targets = Vec::new();
    let mut seen_term = BTreeSet::new();
    for row in rows {
        let (target, kind, exists) = row?;
        let first=seen.insert(target.clone());
        if !exists {continue;}
        if first {
            links.push(target.clone());
        }
        if kind.as_deref() == Some("term") {
            if seen_term.insert(target.clone()) {
                term_use_targets.push(target);
            }
        } else if let Some(kind) = kind {
            kinds.entry(target).or_insert(json!(kind));
        }
    }
    value["links"] = json!(links);
    value["outbound_count"] = json!(seen.len());
    value["kinds"] = json!(kinds);
    value["term_use_targets"] = json!(term_use_targets);
    value["edge"] = json!(crate::ops::edge(card));
    let parent = match crate::parent_location(&card.address) {
        Some(at) if existing(conn, &at)? => Some(at),
        _ => None,
    };
    value["parent_address"] = json!(parent);
    Ok(value)
}

/// Request-local input collector. Only requested models/theory units expand
/// into parts. Boundary models supply ownership, citation and title context.
struct Scope<'a> {
    conn: &'a Connection,
    cards: BTreeMap<String, Card>,
    inbound: BTreeMap<String, BTreeSet<String>>,
    owners_done: BTreeSet<String>,
    parts: BTreeMap<String, BTreeSet<String>>,
}
impl<'a> Scope<'a> {
    fn new(conn: &'a Connection) -> Self {
        Self {
            conn,
            cards: BTreeMap::new(),
            inbound: BTreeMap::new(),
            owners_done: BTreeSet::new(),
            parts: BTreeMap::new(),
        }
    }
    fn add(&mut self, at: &str) -> Result<bool> {
        if self.cards.contains_key(at) {
            return Ok(true);
        }
        if !existing(self.conn, at)? {
            return Ok(false);
        }
        self.cards
            .insert(at.to_string(), crate::load_card(self.conn, at)?);
        Ok(true)
    }
    fn outgoing(&mut self, at: &str) -> Result<Vec<String>> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT DISTINCT target FROM links WHERE source=?1")?;
        let targets = statement
            .query_map([at], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for target in &targets {
            self.add(target)?;
        }
        Ok(targets)
    }
    fn incoming(&mut self, at: &str) -> Result<Vec<String>> {
        let mut statement = self
            .conn
            .prepare_cached("SELECT DISTINCT source, kind FROM links WHERE target=?1")?;
        let rows = statement
            .query_map([at], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let sources = rows.iter().map(|(source, _)| source.clone()).collect::<Vec<_>>();
        // Only an ordinary Link is a reasoning edge: a Term use never makes a Model Card take a
        // part in, so it is not recorded in the membership view (S16, #158; design 10.2).
        self.inbound.entry(at.to_string()).or_default().extend(
            rows.iter()
                .filter(|(_, kind)| kind.as_deref() != Some("term"))
                .map(|(source, _)| source.clone()),
        );
        for source in &sources {
            self.add(source)?;
        }
        Ok(sources)
    }
    fn boundary(&mut self, at: &str) -> Result<()> {
        self.outgoing(at)?;
        self.incoming(at)?;
        Ok(())
    }
    fn children(&mut self, at: &str) -> Result<Vec<String>> {
        let mut children = crate::direct_successor(at)
            .ok()
            .into_iter()
            .collect::<Vec<_>>();
        if self.cards.get(at).is_some_and(|c| !crate::is_tree_root(c)) {
            let lower = format!("{at}|");
            let upper = format!("{at}}}");
            let mut statement = self
                .conn
                .prepare_cached("SELECT location FROM cards WHERE location>=?1 AND location<?2")?;
            for side in statement.query_map([lower, upper], |r| r.get::<_, String>(0))? {
                let side = side?;
                if crate::immediate_side_label(&side, at)
                    .is_some_and(|label| side == format!("{at}|{label}"))
                {
                    children.push(side);
                }
            }
        }
        for child in &children {
            self.add(child)?;
        }
        Ok(children)
    }
    fn owners(&mut self, at: &str) -> Result<()> {
        if !self.add(at)? || !self.owners_done.insert(at.to_string()) {
            return Ok(());
        }
        if self.cards[at].card_type != crate::CardType::Model || self.cards[at].facet.is_none() {
            return Ok(());
        }
        let mut parent = crate::parent_location(at);
        while let Some(above) = parent {
            if !self.add(&above)? || self.cards[&above].card_type != crate::CardType::Model {
                break;
            }
            if self.cards[&above].facet.is_none() {
                break;
            }
            parent = crate::parent_location(&above);
        }
        let mut statement=self.conn.prepare_cached("SELECT DISTINCT l.source FROM links l JOIN cards c ON c.location=l.source OR c.citation_key=l.source WHERE l.target=?1 AND (l.kind IS NULL OR l.kind<>'term') AND c.card_type='model' AND c.model_facet IS NULL")?;
        let heads = statement
            .query_map([at], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        self.inbound
            .entry(at.to_string())
            .or_default()
            .extend(heads.iter().cloned());
        for head in heads {
            self.add(&head)?;
        }
        Ok(())
    }
    fn heads(&mut self, at: &str) -> Result<Vec<String>> {
        self.owners(at)?;
        Ok(crate::model::heads(at, &self.cards, &self.inbound))
    }
    fn members(&mut self, head: &str) -> Result<BTreeSet<String>> {
        if let Some(parts) = self.parts.get(head) {
            return Ok(parts.clone());
        }
        let mut members = BTreeSet::from([head.to_string()]);
        let mut pending = vec![head.to_string()];
        while let Some(at) = pending.pop() {
            for child in self.children(&at)? {
                if self
                    .cards
                    .get(&child)
                    .is_some_and(|c| c.card_type == crate::CardType::Model && c.facet.is_some())
                    && members.insert(child.clone())
                {
                    pending.push(child);
                }
            }
        }
        for target in self.outgoing(head)? {
            if self
                .cards
                .get(&target)
                .is_some_and(|c| c.card_type == crate::CardType::Model && c.facet.is_some())
            {
                members.insert(target);
            }
        }
        self.parts.insert(head.to_string(), members.clone());
        Ok(members)
    }
    fn theories(&mut self, at: &str) -> Result<()> {
        let mut statement=self.conn.prepare_cached("SELECT DISTINCT l.source FROM links l JOIN cards c ON c.location=l.source OR c.citation_key=l.source WHERE l.target=?1 AND (l.kind IS NULL OR l.kind<>'term') AND c.math_form='theory'")?;
        let sources = statement
            .query_map([at], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        self.inbound
            .entry(at.to_string())
            .or_default()
            .extend(sources.iter().cloned());
        for source in sources {
            self.add(&source)?;
        }
        Ok(())
    }
}

fn context(conn: &Connection, addresses: &[String], models: bool) -> Result<Snapshot> {
    let mut scope = Scope::new(conn);
    let mut report_heads = BTreeSet::new();
    let mut report_members = BTreeSet::new();
    for at in addresses {
        if !scope.add(at)? {
            continue;
        }
        scope.boundary(at)?;
        scope.children(at)?;
        if let Some(parent) = crate::parent_location(at) {
            scope.add(&parent)?;
        }
        // A register part holds a term row for this Card, and the register that declares the part
        // is the hub the Card is a member of (D187, §8.4): load it so `hubs_of` can report it.
        for source in scope.inbound.get(at).cloned().unwrap_or_default() {
            if let Some(parent) = crate::parent_location(&source) {
                scope.add(&parent)?;
            }
        }
        if models {
            if let Some(head) = scope.heads(at)?.first().cloned() {
                report_heads.insert(head.clone());
                report_members.extend(scope.members(&head)?);
                // Only a requested Theory report needs all of its units' parts.
                if scope.cards[&head].form == Some(crate::math::MathForm::Theory) {
                    for unit in scope.outgoing(&head)? {
                        if scope
                            .cards
                            .get(&unit)
                            .is_some_and(crate::math::is_unit_head)
                        {
                            report_heads.insert(unit.clone());
                            report_members.extend(scope.members(&unit)?);
                        }
                    }
                }
            }
        }
    }
    for member in &report_members {
        scope.boundary(member)?;
        scope.owners(member)?;
    }
    let known = scope.cards.keys().cloned().collect::<Vec<_>>();
    for at in known {
        scope.owners(&at)?;
    }
    // Unit load, used_by and study need incoming users and the incoming users
    // of each citer, including plain Cards. They do not need those users' parts.
    for head in &report_heads {
        if !crate::math::is_unit_head(&scope.cards[head]) {
            continue;
        }
        let mut citers = BTreeSet::new();
        for source in scope.incoming(head)? {
            if !scope.cards.contains_key(&source) {
                continue;
            }
            let owners = scope.heads(&source)?;
            citers.insert(owners.first().cloned().unwrap_or(source));
        }
        for citer in citers {
            for source in scope.incoming(&citer)? {
                scope.owners(&source)?;
            }
        }
    }
    let units = scope
        .cards
        .values()
        .filter(|c| crate::math::is_unit_head(c))
        .map(|c| c.address.clone())
        .collect::<Vec<_>>();
    for unit in units {
        scope.theories(&unit)?;
    }
    // BTreeMap order preserves the existing navigation/Reverse presentation;
    // the tree endpoint sorts its selected output in ZET address order itself.
    // The selected rows' Lock facts and the archive revision come from the same read, so a scoped
    // view answers the full view's annotations at one revision (R207, S03-AC07).
    let cards = scope.cards.into_values().collect::<Vec<_>>();
    let rows = cards.iter().map(|card| card.row_id).collect::<Vec<_>>();
    let lock = crate::tree_lock::LockView::read(conn, Some(&rows))?;
    let revision = crate::link_index::revision(conn)?;
    Snapshot::from_cards(cards, lock, revision)
}

fn log_large_context(
    root: &Path,
    name: &str,
    snapshot: &Snapshot,
    started: std::time::Instant,
    revision: u64,
) {
    if snapshot.cards.len() >= 200 {
        let _ = crate::log_event(
            root,
            &format!(
                "read {name}: context_cards={} revision={revision} elapsed_ms={}",
                snapshot.cards.len(),
                started.elapsed().as_millis()
            ),
        );
    }
}

pub(crate) fn model(root: &Path, at: &str) -> Result<Value> {
    let started = std::time::Instant::now();
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let snapshot = context(&tx, &[at.to_string()], true)?;
    if !snapshot.by_address.contains_key(at) {
        anyhow::bail!("card `{at}` does not exist");
    }
    let mut model = snapshot.model(at).with_context(|| {
        format!("`{at}` is not a Model Card or Model part; set type `model` first")
    })?;
    let body = |address: &str| {
        snapshot
            .parsed
            .get(address)
            .map(|p| p.body.clone())
            .unwrap_or_default()
    };
    if let Some(head) = model["head"]["address"].as_str().map(str::to_string) {
        model["head"]["body"] = json!(body(&head));
    }
    if let Some(parts) = model["parts"].as_array_mut() {
        for part in parts {
            if let Some(at) = part["address"].as_str().map(str::to_string) {
                part["body"] = json!(body(&at));
            }
        }
    }
    log_large_context(
        root,
        "get_model",
        &snapshot,
        started,
        crate::link_index::revision(&tx)?,
    );
    tx.commit()?;
    Ok(model)
}

pub(crate) fn card(root: &Path, at: &str, term_links: TermLinks) -> Result<Value> {
    let started = std::time::Instant::now();
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let snapshot = context(&tx, &[at.to_string()], true)?;
    let mut result = card_value(&snapshot, at, term_links)?;
    if let Some(card) = snapshot.by_address.get(at) {
        attach_term_facts(&tx, &mut result, card)?;
    }
    log_large_context(
        root,
        "get_card",
        &snapshot,
        started,
        crate::link_index::revision(&tx)?,
    );
    tx.commit()?;
    Ok(result)
}

/// The computed Term facts of one Card (S08, #150): its `term_role`, the `term_list` it belongs
/// to and how many stored Term uses name it. A Card of another type carries none, so every Card
/// read keeps its existing shape.
fn attach_term_facts(conn: &Connection, value: &mut Value, card: &Card) -> Result<()> {
    if let Some(facts) = crate::term::facts(conn, card)?
        && let Some(fields) = facts.as_object()
    {
        for (key, field) in fields {
            value[key] = field.clone();
        }
    }
    Ok(())
}

/// The computed structure of one Term list (S08, #150): its table rows, heads, parts, unfiled
/// Cards, Term uses, coverage and faults, read in one transaction together with the revision and
/// archive identity they describe.
pub(crate) fn terms(root: &Path, at: &str) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let mut value = crate::term::structure(&tx, at)?;
    let identity = archive_identity(&tx, root)?;
    value["archive_revision"] = json!(crate::link_index::revision(&tx)?);
    value["archive_identity"] = identity;
    tx.commit()?;
    Ok(value)
}

pub(crate) fn cards(
    root: &Path,
    addresses: &[String],
    text: bool,
    term_links: TermLinks,
) -> Result<Value> {
    let started = std::time::Instant::now();
    if addresses.len() > 200 {
        return Err(Limit {
            kind: "addresses",
            limit: 200,
            actual: addresses.len(),
        }
        .into());
    }
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let mut missing = Vec::new();
    let mut cards = Vec::new();
    for at in addresses {
        if existing(&tx, at)? {
            cards.push(crate::load_card(&tx, at)?);
        } else {
            missing.push(at.clone());
        }
    }
    let snapshot = (!text).then(|| context(&tx, addresses, true)).transpose()?;
    // One Lock view for the whole lean read: every requested row's facts from one pair of queries,
    // so a lean `get_cards` and a full one answer the same annotations at one revision.
    let lean = match &snapshot {
        Some(_) => crate::tree_lock::LockView::empty(),
        None => {
            let rows = cards.iter().map(|card| card.row_id).collect::<Vec<_>>();
            crate::tree_lock::LockView::read(&tx, Some(&rows))?
        }
    };
    let mut values = Vec::new();
    for card in &cards {
        values.push(match &snapshot {
            Some(snapshot) => {
                let mut value = card_value(snapshot, &card.address, term_links)?;
                attach_term_facts(&tx, &mut value, card)?;
                value
            }
            None => {
                let mut value = text_facts(&tx, card, &lean, term_links)?;
                // The lean read answers the same computed Term facts as the full one (S14, #161).
                attach_term_facts(&tx, &mut value, card)?;
                value
            }
        });
    }
    let identity = archive_identity(&tx, root)?;
    // The check contract (S06, #148): the running service publishes the rules,
    // effective policy, limits, archive, and syntax capabilities that would decide
    // a write, in the one verified read a self-check already makes. A client that
    // cannot read this block refuses to present its answer as the gate's.
    let contract = contract_value(&tx, &identity)?;
    let mut result = json!({"cards":values,"complete":missing.is_empty(),"missing":missing,"archive_revision":crate::link_index::revision(&tx)?,"archive_identity":identity,"check_contract":contract});
    term_links.mark(&mut result);
    if let Some(snapshot) = &snapshot {
        log_large_context(
            root,
            "get_cards",
            snapshot,
            started,
            crate::link_index::revision(&tx)?,
        );
    }
    tx.commit()?;
    Ok(result)
}

fn tree_addresses(conn: &Connection, tree: &str) -> Result<Vec<String>> {
    let root = crate::tree_root_address(tree);
    if !existing(conn, &root)? {
        anyhow::bail!("tree `{tree}` does not exist");
    }
    let lower = format!("{tree}/");
    let upper = format!("{tree}0");
    let mut statement =
        conn.prepare_cached("SELECT location FROM cards WHERE location>=?1 AND location<?2")?;
    let mut addresses = statement
        .query_map([lower, upper], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !addresses.contains(&root) {
        addresses.push(root);
    }
    addresses.retain(|at| crate::is_tree_member(at, tree));
    addresses.sort_by(|a, b| crate::model::address_order(a, b));
    Ok(addresses)
}

pub(crate) fn tree(root: &Path, tree: &str, bodies: bool, term_links: TermLinks) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let tree = crate::tree_id(tree);
    let addresses = tree_addresses(&tx, tree)?;
    let snapshot = context(&tx, &addresses, false)?;
    let root_address = crate::tree_root_address(tree);
    let root_card = &snapshot.by_address[&root_address];
    let mut cards = Vec::new();
    for at in &addresses {
        let card = &snapshot.by_address[at];
        let mut value = snapshot.summary(card);
        value["excerpt"] = json!(term_links.project(value["excerpt"].as_str().unwrap_or_default()));
        if bodies {
            value["body"] = json!(term_links.project(&snapshot.parsed[at].body));
            value["text"] = json!(term_links.project(&card.text));
            value["bibtex"] = json!(card.bibtex);
        }
        term_links.mark(&mut value);
        attach_term_facts(&tx, &mut value, card)?;
        cards.push(value);
    }
    let mut value = json!({"tree":tree,"root":root_address,"workspace":root_card.workspace.id(),"zone":crate::ops::zone(root_card),"title":snapshot.title(&root_address),"cards":cards,"complete":true,"archive_revision":crate::link_index::revision(&tx)?});
    term_links.mark(&mut value);
    tx.commit()?;
    Ok(value)
}

pub(crate) fn titles(
    root: &Path,
    workspace: Option<crate::Workspace>,
    zone: Option<&str>,
) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let mut statement=tx.prepare("SELECT COALESCE(location,citation_key),text,workspace,card_type,model_facet,row_id FROM cards WHERE (?1 IS NULL OR workspace=?1) AND (?2 IS NULL OR CASE WHEN is_lit=1 OR citation_key IS NOT NULL OR location GLOB '[A-Za-z]*' THEN 'literature' ELSE 'topic' END=?2)")?;
    let rows = statement.query_map(
        rusqlite::params![workspace.map(crate::Workspace::id), zone],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
            ))
        },
    )?;
    let mut titles = Vec::new();
    let mut title_rows = Vec::new();
    for row in rows {
        let (at, text, workspace, card_type, facet, row_id) = row?;
        let parsed = crate::parse_card_text(&text).with_context(|| format!("Card `{at}`"))?;
        titles.push(json!({"address":at,"tree":crate::tree_id(&at),"workspace":workspace,"type":card_type,"facet":facet,"title":parsed.title,"restatement":crate::entry::is_restatement_entry(&parsed.body)}));
        title_rows.push(row_id);
    }
    drop(statement);
    // A destination plan reads these titles, so it learns there which destinations a Lock freezes
    // (R208). The facts come from the same read as the rows.
    let lock = crate::tree_lock::LockView::read(&tx, Some(&title_rows))?;
    for (value, row_id) in titles.iter_mut().zip(&title_rows) {
        crate::tree_lock::decorate_card(value, *row_id, &lock);
    }
    titles.sort_by(|a, b| {
        crate::model::address_order(
            a["address"].as_str().unwrap(),
            b["address"].as_str().unwrap(),
        )
    });
    let value = json!({"count":titles.len(),"titles":titles,"complete":true,"archive_revision":crate::link_index::revision(&tx)?});
    tx.commit()?;
    Ok(value)
}

pub(crate) fn workspaces(root: &Path) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let mut rows = Vec::new();
    let mut total = 0u64;
    for ws in crate::Workspace::ALL {
        let (cards,topics,literature):(u64,u64,u64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(is_topic),0),COALESCE(SUM(is_lit),0) FROM cards WHERE workspace=?1",[ws.id()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        total += cards;
        rows.push(json!({"id":ws.id(),"label":ws.label(),"access":if Access::Agent.can_write(ws){"read-write"}else{"read-only"},"zones":{"topic":{"trees":topics},"literature":{"trees":literature}},"cards":cards}));
    }
    let broken:u64=tx.query_row("SELECT COUNT(*) FROM links l LEFT JOIN cards c ON c.location=l.target OR c.citation_key=l.target WHERE c.row_id IS NULL",[],|r|r.get(0))?;
    tx.commit()?;
    Ok(json!({"workspaces":rows,"cards":total,"broken_links":broken}))
}

pub(crate) fn trees(
    root: &Path,
    workspace: Option<crate::Workspace>,
    zone: Option<&str>,
) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let mut query=tx.prepare("SELECT COALESCE(location,citation_key) FROM cards WHERE (is_topic=1 OR is_lit=1) AND (?1 IS NULL OR workspace=?1) AND (?2 IS NULL OR CASE WHEN is_lit=1 THEN 'literature' ELSE 'topic' END=?2)")?;
    let mut roots = query
        .query_map(
            rusqlite::params![workspace.map(crate::Workspace::id), zone],
            |r| r.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(query);
    roots.sort_by(|a, b| crate::model::address_order(a, b));
    // The rows' Lock facts, from the same read as the tree rows they annotate (R204, S03-AC01).
    let rows = roots
        .iter()
        .filter_map(|at| crate::load_card(&tx, at).ok().map(|card| card.row_id))
        .collect::<Vec<_>>();
    let lock = crate::tree_lock::LockView::read(&tx, Some(&rows))?;
    let mut trees = Vec::new();
    for at in roots {
        let card = crate::load_card(&tx, &at)?;
        let parsed = crate::parse_card_text(&card.text)?;
        let tree = crate::tree_id(&at);
        let lower = format!("{tree}/");
        let upper = format!("{tree}0");
        let (count,events,findings,models,terms):(u64,u64,u64,u64,u64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(card_type='event' AND model_facet IS NULL),0),COALESCE(SUM(card_type='finding' AND model_facet IS NULL),0),COALESCE(SUM(card_type='model' AND model_facet IS NULL),0),COALESCE(SUM(card_type='term'),0) FROM cards WHERE (location>=?1 AND location<?2) OR citation_key=?3",rusqlite::params![lower,upper,tree],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        trees.push(json!({"tree":tree,"root":at,"workspace":card.workspace.id(),"kind":crate::ops::zone(&card),"title":parsed.title,"excerpt":crate::ops::excerpt(&parsed.body),"count":count,"events":events,"findings":findings,"models":models,"terms":terms,"locked":lock.is_locked(card.row_id)}));
    }
    tx.commit()?;
    Ok(json!({"trees":trees}))
}

pub(crate) fn links(
    root: &Path,
    workspace: Option<crate::Workspace>,
    cross_only: bool,
) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    // The first occurrence of each distinct source-target pair decides its form, so a Term use is
    // reported with the authored label an editor completes from (S14, #161; design 15.1). SQLite's
    // bare-column rule on the min() aggregate keeps `kind` on that same first row.
    let mut query=tx.prepare("SELECT l.source,s.workspace,l.target,t.workspace,l.kind,MIN(l.ord) FROM links l JOIN cards s ON s.location=l.source OR s.citation_key=l.source LEFT JOIN cards t ON t.location=l.target OR t.citation_key=l.target WHERE (?1 IS NULL OR s.workspace=?1 OR t.workspace=?1) AND (?2=0 OR s.workspace<>t.workspace) GROUP BY l.source,l.target ORDER BY l.source,MIN(l.ord)")?;
    let rows=query.query_map(rusqlite::params![workspace.map(crate::Workspace::id),cross_only],|r|{
        let source:String=r.get(0)?;let source_workspace:String=r.get(1)?;
        let target:String=r.get(2)?;let target_workspace:Option<String>=r.get(3)?;
        let kind:Option<String>=r.get(4)?;
        Ok((source,source_workspace,target,target_workspace,kind))
    })?.collect::<rusqlite::Result<Vec<_>>>()?;
    drop(query);
    // A Term row names no label in the index; the authored body holds it. Only sources with at
    // least one Term use are parsed, and each source once.
    let mut labels: BTreeMap<(String, String), Option<String>> = BTreeMap::new();
    for (source, _, target, _, kind) in &rows {
        if kind.as_deref() != Some("term") || labels.contains_key(&(source.clone(), target.clone())) {
            continue;
        }
        let parsed = crate::parse_card_text(&crate::load_card(&tx, source)?.text)?;
        let label = crate::link::LinkBody::parse(&parsed.body)?
            .occurrences()
            .iter()
            .find(|occurrence| {
                occurrence.target == target && occurrence.form == crate::link::LinkForm::Term
            })
            .and_then(|occurrence| occurrence.label)
            .map(str::to_string);
        labels.insert((source.clone(), target.clone()), label);
    }
    let values=rows.into_iter().map(|(source,source_workspace,target,target_workspace,kind)|{
        let mut value = json!({"source":source,"source_workspace":source_workspace,"target":target,"target_workspace":target_workspace,"broken":target_workspace.is_none()});
        if kind.as_deref() == Some("term") {
            value["kind"] = json!("term");
            value["label"] = json!(labels.get(&(source, target)).cloned().flatten());
        }
        value
    }).collect::<Vec<_>>();
    tx.commit()?;
    Ok(json!({"links":values}))
}

pub(crate) fn broken(root: &Path, workspace: Option<crate::Workspace>) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    // A missing target is Broken for either form; a Term row whose target exists is Broken only
    // when that row is no longer a Term head (S11, #153, design 10.3).
    let mut query=tx.prepare("SELECT l.source,l.ord,l.target,l.kind FROM links l JOIN cards s ON s.location=l.source OR s.citation_key=l.source LEFT JOIN cards t ON t.location=l.target OR t.citation_key=l.target WHERE (t.row_id IS NULL OR l.kind = 'term') AND (?1 IS NULL OR s.workspace=?1) ORDER BY l.source,l.ord")?;
    let rows = query
        .query_map([workspace.map(crate::Workspace::id)], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, usize>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(query);
    let mut bodies = BTreeMap::new();
    let mut heads: BTreeMap<String, bool> = BTreeMap::new();
    let mut broken = Vec::new();
    for (source, ord, target, kind) in rows {
        if !bodies.contains_key(&source) {
            let card = crate::load_card(&tx, &source)?;
            let parsed = crate::parse_card_text(&card.text)?;
            bodies.insert(source.clone(), (card, parsed));
        }
        let (card, parsed) = &bodies[&source];
        let body = crate::link::LinkBody::parse(&parsed.body)?;
        let occurrence = body
            .occurrences()
            .get(ord)
            .filter(|o| o.target == target)
            .context("Link index drift in broken-link source; run `zt status --check-links`")?;
        let term_row = kind.as_deref() == Some("term");
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM cards WHERE location=?1 OR citation_key=?1)",
            [&target],
            |r| r.get(0),
        )?;
        let reason = if !exists {
            crate::link::BrokenReason::Missing
        } else if term_row {
            let is_head = match heads.get(&target) {
                Some(is_head) => *is_head,
                None => {
                    let is_head = crate::term::is_head(&tx, &target)?;
                    heads.insert(target.clone(), is_head);
                    is_head
                }
            };
            if is_head {
                continue;
            }
            crate::link::BrokenReason::NotHead
        } else {
            // An ordinary Link to an existing row is valid, whatever its Card type.
            continue;
        };
        broken.push(json!({"source":source,"title":parsed.title,"workspace":card.workspace.id(),"target":target,"label":occurrence.label,"reason":reason.id(),"line":crate::link::line_around(&parsed.body,occurrence.macro_range.start)}));
    }
    tx.commit()?;
    Ok(json!({"broken":broken}))
}

/// The persisted random archive identity and its resolved path: the evidence every bulk read and
/// every entry view names, so a client can bind to one archive rather than a path alone.
fn archive_identity(conn: &Connection, root: &Path) -> Result<Value> {
    let archive_id: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key='archive_id'",
            [],
            |row| row.get(0),
        )
        .context("missing archive identity; archive migration must complete before ledger binding")?;
    if archive_id.len() != 32 || !archive_id.bytes().all(|c| c.is_ascii_hexdigit()) {
        bail!("invalid archive identity; refusing an unbound read");
    }
    let archive_path = std::fs::canonicalize(root).context("resolve archive identity path")?;
    Ok(json!({"id":archive_id,"path":archive_path.to_string_lossy()}))
}

/// The check contract this service publishes beside an archive read (design
/// 8.3, S06 / #148): the rule and diagnostic versions, the effective policy and
/// its hash, the limits, the archive identity and revision, the write-gate
/// stage, and the syntax capabilities. A read that cannot name them refuses to
/// answer, never silently defaulting a fact that decides a check.
fn contract_value(conn: &Connection, identity: &Value) -> Result<Value> {
    let policy = crate::gate::load(conn)?;
    let stage = crate::gate::current_stage(conn)?;
    let archive = zt_check::contract::ArchiveFacts {
        id: identity["id"].as_str().unwrap_or_default().to_string(),
        path: identity["path"].as_str().unwrap_or_default().to_string(),
        revision: crate::link_index::revision(conn)?,
        stage: stage.id().to_string(),
    };
    Ok(zt_check::contract::Contract::served(
        &policy,
        &zt_check::core::Limits::default(),
        archive,
    )
    .publish())
}

/// One coherent hub read: every Card's metadata and heading line, the complete body of every Hub
/// Card, and the source→target index, inside one transaction.
struct HubRead {
    snapshot: Snapshot,
    links: BTreeMap<String, Vec<String>>,
    revision: u64,
}

fn hub_read(conn: &Connection) -> Result<HubRead> {
    // Hub bodies are parsed for the hub policy and a Topic's body carries its `## 入口` section;
    // every other Card contributes its heading line only (the indexed read's economy).
    let mut query=conn.prepare("SELECT row_id,COALESCE(location,citation_key),citation_key,is_topic,is_lit,bibtex,CASE WHEN card_type='hub' OR is_topic = 1 THEN text ELSE substr(ltrim(text,char(9)||char(10)||char(13)||' '),1,instr(ltrim(text,char(9)||char(10)||char(13)||' ')||char(10),char(10))-1) END,workspace,card_type,model_facet,model_emphasis,model_level,hub_level,math_form,term_name FROM cards ORDER BY COALESCE(location,citation_key)")?;
    let cards = query
        .query_map([], crate::card_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(query);
    let revision = crate::link_index::revision(conn)?;
    let lock = crate::tree_lock::LockView::read(conn, None)?;
    let snapshot = Snapshot::from_cards(cards, lock, revision)?;
    let mut query = conn.prepare(
        "SELECT source,target FROM links GROUP BY source,target ORDER BY source,MIN(ord)",
    )?;
    let rows = query.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut links: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for row in rows {
        let (source, target) = row?;
        links.entry(source).or_default().push(target);
    }
    drop(query);
    Ok(HubRead {
        snapshot,
        links,
        revision,
    })
}

/// The authored bodies `one-book` advice needs from an indexed read (§8.2, D166): the members of
/// the hub the request reports, and the accepted register's term targets. Nothing else is fetched,
/// so an ordinary body never enters the hub read; a Card beyond the bound stays unknown evidence.
fn hub_evidence(
    conn: &Connection,
    read: &HubRead,
    at: &str,
) -> Result<std::collections::BTreeMap<String, String>> {
    const EVIDENCE_MAX: usize = 800;
    let mut wanted: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    if let Some(parsed) = read.snapshot.parsed.get(at) {
        for line in crate::hub::parse_body(&parsed.body).lines {
            wanted.insert(line.target);
        }
    }
    if let Some(hub) = read.snapshot.by_address.get(at) {
        for card in read.snapshot.cards.iter().filter(|card| {
            card.card_type == crate::card_type::CardType::Hub
                && card.hub_level == Some(crate::hub::HubLevel::Register)
                && card.workspace == hub.workspace
        }) {
            let body = read
                .snapshot
                .parsed
                .get(&card.address)
                .map(|parsed| parsed.body.clone())
                .unwrap_or_default();
            for row in crate::hub::register_rows(&body) {
                wanted.extend(row.targets);
            }
            wanted.extend(crate::hub::register_part_targets(&body));
        }
    }
    wanted.remove(at);
    let mut bodies = std::collections::BTreeMap::new();
    let mut query = conn.prepare("SELECT text FROM cards WHERE COALESCE(location,citation_key) = ?1")?;
    for address in wanted.into_iter().take(EVIDENCE_MAX) {
        if let Some(text) = query.query_row([&address], |row| row.get::<_, String>(0)).ok() {
            if let Ok(parsed) = crate::parse_card_text(&text) {
                bodies.insert(address, parsed.body);
            }
        }
    }
    Ok(bodies)
}

/// Hub policy is global: stream metadata/title lines and the index once, and
/// parse full authored bodies only for Hub Cards. Domain policy stays in Graph.
///
/// `entries` pages the flattened entry projection of a register (§8.5, D276); it applies only to
/// `get_hub` on a register, and the existing hub report is not truncated by it. The report's own
/// `faults` (H5's register rows and H8's line reasons) stay in the list beside the entry view's
/// row, part, restatement and ambiguous-label faults; a row fault both check is one item.
pub(crate) fn hubs(
    root: &Path,
    at: Option<&str>,
    workspace: Option<crate::Workspace>,
    limits: &crate::hub::Limits,
    entries: &crate::entry::EntryArgs,
) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let read = hub_read(&tx)?;
    let graph = match at {
        Some(at) => {
            let bodies = hub_evidence(&tx, &read, at)?;
            crate::hub::Graph::with_links(&read.snapshot, &read.links).with_bodies(bodies)
        }
        None => crate::hub::Graph::with_links(&read.snapshot, &read.links),
    };
    let value = if let Some(at) = at {
        let mut value = graph.report(at, limits)?;
        if value["level"] == json!("register") {
            let identity = archive_identity(&tx, root)?;
            let hub_faults = value["faults"].as_array().cloned().unwrap_or_default();
            let projection = register_view(&tx, &read, at, entries, identity, None)?;
            if let Some(fields) = projection.as_object() {
                for (key, field) in fields {
                    value[key] = field.clone();
                }
            }
            let mut faults = value["faults"].as_array().cloned().unwrap_or_default();
            for fault in hub_faults {
                let reported = faults.iter().any(|known| {
                    known["fault"] == fault["fault"] && known["detail"] == fault["detail"]
                });
                if !reported {
                    faults.push(fault);
                }
            }
            value["faults"] = json!(faults);
        }
        value
    } else {
        graph.list(workspace, limits)
    };
    tx.commit()?;
    Ok(value)
}

/// The register of one Workspace as a flattened entry view (§4.1, §8.5, D276, D296). An absent
/// register is an honest empty answer with `register_missing`; two registers are reported, never
/// chosen between. `terms_for` names one Card whose accepted terms the answer carries (`terms_at`
/// and `terms`): the register rows that name it and the accepted terms its own title and prose
/// state (§9.1, D196).
pub(crate) fn register(
    root: &Path,
    workspace: crate::Workspace,
    entries: &crate::entry::EntryArgs,
    terms_for: Option<&str>,
) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let read = hub_read(&tx)?;
    let identity = archive_identity(&tx, root)?;
    let registers = read
        .snapshot
        .cards
        .iter()
        .filter(|card| {
            card.card_type == crate::CardType::Hub
                && card.hub_level == Some(crate::hub::HubLevel::Register)
                && card.workspace == workspace
        })
        .map(|card| card.address.clone())
        .collect::<Vec<_>>();
    let mut value = json!({
        "workspace": workspace.id(),
        "register": Value::Null,
        "title": Value::Null,
        "register_missing": false,
        "duplicate_registers": [],
        "entries": [],
        "entry_offset": entries.offset,
        "entry_limit": entries.limit,
        "entry_total": 0,
        "entry_next_offset": Value::Null,
        "parts": [],
        "faults": [],
        "complete": false,
        "terms_by_card": {},
        "terms_at": Value::Null,
        "terms": [],
        "restatements": [],
        "bounds": crate::entry::bounds(),
        "archive_revision": read.revision,
        "archive_identity": identity,
    });
    match registers.as_slice() {
        [] => {
            value["register_missing"] = json!(true);
            value["faults"] = json!([{
                "fault": "register_missing",
                "detail": format!(
                    "the {} Workspace has no Hub Card of level register; there are no accepted entries to read",
                    workspace.id()
                ),
            }]);
        }
        [at] => {
            value["register"] = json!(at);
            value["title"] = json!(read.snapshot.title(at));
            let projection = register_view(
                &tx,
                &read,
                at,
                entries,
                value["archive_identity"].clone(),
                terms_for,
            )?;
            if let Some(fields) = projection.as_object() {
                for (key, field) in fields {
                    value[key] = field.clone();
                }
            }
            value["workspace"] = json!(workspace.id());
        }
        many => {
            value["duplicate_registers"] = json!(many);
            value["faults"] = json!([{
                "fault": "duplicate",
                "detail": format!(
                    "the {} Workspace has {} registers ({}); a Workspace has one",
                    workspace.id(),
                    many.len(),
                    many.join(", ")
                ),
            }]);
        }
    }
    tx.commit()?;
    Ok(value)
}

/// The flattened projection of one register: its rows and its declared parts with the bounded
/// authored bodies of their targets. Read in the same transaction as the hub graph (§8.5, D049).
/// `terms_for` names one Card whose accepted terms the projection carries (§9.1, D196); its own
/// authored body is read here, so no other body enters this read.
fn register_view(
    conn: &Connection,
    read: &HubRead,
    at: &str,
    entries: &crate::entry::EntryArgs,
    identity: Value,
    terms_for: Option<&str>,
) -> Result<Value> {
    let body = read
        .snapshot
        .parsed
        .get(at)
        .map(|parsed| parsed.body.clone())
        .unwrap_or_default();
    let mut wanted = BTreeSet::new();
    for row in crate::hub::register_rows(&body) {
        wanted.extend(row.targets);
    }
    for part in crate::hub::register_part_targets(&body) {
        wanted.insert(part);
    }
    // A Side successor of the register holding 词条 rows that the register no longer declares is
    // reported, not accepted and not dropped: its body is read to tell it from an ordinary Card
    // (D182, §8.4).
    for successor in crate::hub::register_side_successors(&read.snapshot, at) {
        wanted.insert(successor);
    }
    let mut bodies = BTreeMap::new();
    for address in &wanted {
        if !read.snapshot.by_address.contains_key(address) {
            continue;
        }
        let card = crate::load_card(conn, address)?;
        bodies.insert(address.clone(), crate::parse_card_text(&card.text)?.body);
    }
    // The confirmed restatement references (§4.5): Cards of the register's Workspace whose body
    // has a 同述入口 section. The section is rare, so one scan is cheaper than reading every body.
    let workspace = read
        .snapshot
        .by_address
        .get(at)
        .map_or("agent", |card| card.workspace.id());
    let mut restatement_sections = BTreeMap::new();
    let mut query = conn.prepare(
        "SELECT COALESCE(location,citation_key) FROM cards WHERE workspace=?1 AND text LIKE '%同述入口%' ORDER BY COALESCE(location,citation_key)",
    )?;
    let candidates = query
        .query_map([workspace], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(query);
    for address in candidates {
        let card = crate::load_card(conn, &address)?;
        let links = crate::entry::restatement_links(&crate::parse_card_text(&card.text)?.body);
        if !links.is_empty() {
            restatement_sections.insert(address, links);
        }
    }
    let view = crate::entry::View::read(&read.snapshot, at, &bodies, &restatement_sections);
    let mut value = view.json(*entries, read.revision, &identity);
    if let Some(address) = terms_for {
        // One Card's own authored body, read here and nowhere else: its accepted terms are the
        // register rows that name it plus the terms its substantive title and prose state (D196).
        let body = crate::load_card(conn, address)
            .ok()
            .and_then(|card| crate::parse_card_text(&card.text).ok())
            .map(|parsed| parsed.body)
            .unwrap_or_default();
        value["terms_at"] = json!(address);
        value["terms"] = json!(view.terms_at(&read.snapshot, address, &body));
    }
    Ok(value)
}

/// The unsaved Spread proposal of one question (§9.3-§9.5, D211-D231): one coherent read, the same
/// retrieval function the GUI and the MCP server share, and no write. Unlike the hub read, the walk
/// may need any authored body it reaches, so the whole Card set is read once inside the
/// transaction (as the entry search does); the bounds in `entry::gather` keep the work finite. The
/// archive identity and revision are read in the same transaction, so a caller can bind the
/// proposal to the exact archive state it describes.
pub(crate) fn ask(root: &Path, args: &crate::entry::AskArgs) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let snapshot = Snapshot::load(&tx)?;
    let identity = archive_identity(&tx, root)?;
    let value = crate::entry::gather(&snapshot, args, crate::link_index::revision(&tx)?, &identity);
    tx.commit()?;
    Ok(value)
}

