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

pub(crate) fn text_value(card: &Card) -> Result<Value> {
    let parsed = crate::parse_card_text(&card.text)?;
    Ok(json!({
        "address":card.address,"tree":crate::tree_id(&card.address),
        "workspace":card.workspace.id(),"writable":Access::Agent.can_write(card.workspace),
        "kind":crate::ops::kind(card),"type":card.card_type.id(),
        "facet":card.facet.as_ref().map(crate::model::Facet::id),
        "emphasis":card.emphasis.map(crate::model::Emphasis::id),
        "level":card.level_id(),"form":card.form.map(crate::math::MathForm::id),
        "title":parsed.title,"body":parsed.body,"text":card.text,
        "bibtex":card.bibtex,"citation_key":card.citation_key
    }))
}

pub(crate) fn card_value(snapshot: &Snapshot, at: &str) -> Result<Value> {
    let card = snapshot
        .by_address
        .get(at)
        .with_context(|| format!("card `{at}` does not exist"))?;
    let mut value = text_value(card)?;
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
/// projections and without fetching a whole tree to recover its summaries.
pub(crate) fn text_facts(conn: &Connection, card: &Card) -> Result<Value> {
    let mut value = text_value(card)?;
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
    for row in rows {
        let (target, kind, exists) = row?;
        let first=seen.insert(target.clone());
        if !exists {continue;}
        if first {
            links.push(target.clone());
        }
        if let Some(kind) = kind {
            kinds.entry(target).or_insert(json!(kind));
        }
    }
    value["links"] = json!(links);
    value["outbound_count"] = json!(seen.len());
    value["kinds"] = json!(kinds);
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
            .prepare_cached("SELECT DISTINCT source FROM links WHERE target=?1")?;
        let sources = statement
            .query_map([at], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        self.inbound
            .entry(at.to_string())
            .or_default()
            .extend(sources.iter().cloned());
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
        let mut statement=self.conn.prepare_cached("SELECT DISTINCT l.source FROM links l JOIN cards c ON c.location=l.source OR c.citation_key=l.source WHERE l.target=?1 AND c.card_type='model' AND c.model_facet IS NULL")?;
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
        let mut statement=self.conn.prepare_cached("SELECT DISTINCT l.source FROM links l JOIN cards c ON c.location=l.source OR c.citation_key=l.source WHERE l.target=?1 AND c.math_form='theory'")?;
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
    Snapshot::from_cards(scope.cards.into_values().collect())
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

pub(crate) fn card(root: &Path, at: &str) -> Result<Value> {
    let started = std::time::Instant::now();
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let snapshot = context(&tx, &[at.to_string()], true)?;
    let result = card_value(&snapshot, at)?;
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

pub(crate) fn cards(root: &Path, addresses: &[String], text: bool) -> Result<Value> {
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
    let snapshot = (!text).then(|| context(&tx, addresses, true)).transpose()?;
    let mut values = Vec::new();
    let mut missing = Vec::new();
    for at in addresses {
        if !existing(&tx, at)? {
            missing.push(at.clone());
            continue;
        }
        values.push(if let Some(snapshot) = &snapshot {
            card_value(snapshot, at)?
        } else {
            text_facts(&tx, &crate::load_card(&tx, at)?)?
        });
    }
    let identity = archive_identity(&tx, root)?;
    let result = json!({"cards":values,"complete":missing.is_empty(),"missing":missing,"archive_revision":crate::link_index::revision(&tx)?,"archive_identity":identity});
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

pub(crate) fn tree(root: &Path, tree: &str, bodies: bool) -> Result<Value> {
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
        if bodies {
            value["body"] = json!(snapshot.parsed[at].body);
            value["text"] = json!(card.text);
            value["bibtex"] = json!(card.bibtex);
        }
        cards.push(value);
    }
    let value = json!({"tree":tree,"root":root_address,"workspace":root_card.workspace.id(),"zone":crate::ops::zone(root_card),"title":snapshot.title(&root_address),"cards":cards,"complete":true,"archive_revision":crate::link_index::revision(&tx)?});
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
    let mut statement=tx.prepare("SELECT COALESCE(location,citation_key),text,workspace,card_type,model_facet FROM cards WHERE (?1 IS NULL OR workspace=?1) AND (?2 IS NULL OR CASE WHEN is_lit=1 OR citation_key IS NOT NULL OR location GLOB '[A-Za-z]*' THEN 'literature' ELSE 'topic' END=?2)")?;
    let rows = statement.query_map(
        rusqlite::params![workspace.map(crate::Workspace::id), zone],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        },
    )?;
    let mut titles = Vec::new();
    for row in rows {
        let (at, text, workspace, card_type, facet) = row?;
        let parsed = crate::parse_card_text(&text).with_context(|| format!("Card `{at}`"))?;
        titles.push(json!({"address":at,"tree":crate::tree_id(&at),"workspace":workspace,"type":card_type,"facet":facet,"title":parsed.title}));
    }
    drop(statement);
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
    let mut trees = Vec::new();
    for at in roots {
        let card = crate::load_card(&tx, &at)?;
        let parsed = crate::parse_card_text(&card.text)?;
        let tree = crate::tree_id(&at);
        let lower = format!("{tree}/");
        let upper = format!("{tree}0");
        let (count,events,findings,models):(u64,u64,u64,u64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(card_type='event' AND model_facet IS NULL),0),COALESCE(SUM(card_type='finding' AND model_facet IS NULL),0),COALESCE(SUM(card_type='model' AND model_facet IS NULL),0) FROM cards WHERE (location>=?1 AND location<?2) OR citation_key=?3",rusqlite::params![lower,upper,tree],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        trees.push(json!({"tree":tree,"root":at,"workspace":card.workspace.id(),"kind":crate::ops::zone(&card),"title":parsed.title,"excerpt":crate::ops::excerpt(&parsed.body),"count":count,"events":events,"findings":findings,"models":models}));
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
    let mut query=tx.prepare("SELECT l.source,s.workspace,l.target,t.workspace FROM links l JOIN cards s ON s.location=l.source OR s.citation_key=l.source LEFT JOIN cards t ON t.location=l.target OR t.citation_key=l.target WHERE (?1 IS NULL OR s.workspace=?1 OR t.workspace=?1) AND (?2=0 OR s.workspace<>t.workspace) GROUP BY l.source,l.target ORDER BY l.source,MIN(l.ord)")?;
    let values=query.query_map(rusqlite::params![workspace.map(crate::Workspace::id),cross_only],|r|{
        let source:String=r.get(0)?;let source_workspace:String=r.get(1)?;
        let target:String=r.get(2)?;let target_workspace:Option<String>=r.get(3)?;
        Ok(json!({"source":source,"source_workspace":source_workspace,"target":target,"target_workspace":target_workspace,"broken":target_workspace.is_none()}))
    })?.collect::<rusqlite::Result<Vec<_>>>()?;
    drop(query);
    tx.commit()?;
    Ok(json!({"links":values}))
}

pub(crate) fn broken(root: &Path, workspace: Option<crate::Workspace>) -> Result<Value> {
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let mut query=tx.prepare("SELECT l.source,l.ord,l.target FROM links l JOIN cards s ON s.location=l.source OR s.citation_key=l.source LEFT JOIN cards t ON t.location=l.target OR t.citation_key=l.target WHERE t.row_id IS NULL AND (?1 IS NULL OR s.workspace=?1) ORDER BY l.source,l.ord")?;
    let rows = query
        .query_map([workspace.map(crate::Workspace::id)], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, usize>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(query);
    let mut bodies = BTreeMap::new();
    let mut broken = Vec::new();
    for (source, ord, target) in rows {
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
        broken.push(json!({"source":source,"title":parsed.title,"workspace":card.workspace.id(),"target":target,"line":crate::link::line_around(&parsed.body,occurrence.macro_range.start)}));
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

/// One coherent hub read: every Card's metadata and heading line, the complete body of every Hub
/// Card, and the source→target index, inside one transaction.
struct HubRead {
    snapshot: Snapshot,
    links: BTreeMap<String, Vec<String>>,
    revision: u64,
}

fn hub_read(conn: &Connection) -> Result<HubRead> {
    let mut query=conn.prepare("SELECT row_id,COALESCE(location,citation_key),citation_key,is_topic,is_lit,bibtex,CASE WHEN card_type='hub' THEN text ELSE substr(ltrim(text,char(9)||char(10)||char(13)||' '),1,instr(ltrim(text,char(9)||char(10)||char(13)||' ')||char(10),char(10))-1) END,workspace,card_type,model_facet,model_emphasis,model_level,hub_level,math_form FROM cards ORDER BY COALESCE(location,citation_key)")?;
    let cards = query
        .query_map([], crate::card_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(query);
    let snapshot = Snapshot::from_cards(cards)?;
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
        revision: crate::link_index::revision(conn)?,
    })
}

/// Hub policy is global: stream metadata/title lines and the index once, and
/// parse full authored bodies only for Hub Cards. Domain policy stays in Graph.
///
/// `entries` pages the flattened entry projection of a register (§8.5, D276); it applies only to
/// `get_hub` on a register, and the existing hub report is not truncated by it.
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
    let graph = crate::hub::Graph::with_links(&read.snapshot, &read.links);
    let value = if let Some(at) = at {
        let mut value = graph.report(at, limits)?;
        if value["level"] == json!("register") {
            let identity = archive_identity(&tx, root)?;
            let projection = register_view(&tx, &read, at, entries, identity)?;
            if let Some(fields) = projection.as_object() {
                for (key, field) in fields {
                    value[key] = field.clone();
                }
            }
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
/// chosen between.
pub(crate) fn register(
    root: &Path,
    workspace: crate::Workspace,
    entries: &crate::entry::EntryArgs,
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
            let projection = register_view(&tx, &read, at, entries, value["archive_identity"].clone())?;
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
fn register_view(
    conn: &Connection,
    read: &HubRead,
    at: &str,
    entries: &crate::entry::EntryArgs,
    identity: Value,
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
    Ok(view.json(*entries, read.revision, &identity))
}

