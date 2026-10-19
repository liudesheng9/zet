//! Hub Cards: ways into one subject.
//!
//! A Hub Card (type `hub`) lists, in named groups, Links to the Cards and the
//! lower Hub Cards that carry its subject, one list item each with a reason.
//! Its Hub level says how wide the subject is: `line` (one line of thought),
//! `topic` (one Topic tree), `field` (several Topic trees), or `register`
//! (the Workspace). ZT computes each hub's members, range, upper and lower
//! hubs, and faults from the stored Cards; it reports faults and never
//! enforces them. See `docs/hub-cards-design.md`.

use anyhow::{Result, bail};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::Card;
use crate::card_type::CardType;
use crate::link::LinkBody;
use crate::ops::Snapshot;
use crate::workspace::Workspace;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum HubLevel {
    /// One line of thought inside one tree.
    Line,
    /// One Topic tree.
    Topic,
    /// Several Topic trees that share one subject.
    Field,
    /// The whole Workspace: key terms, each with one to three ways in.
    Register,
}

impl HubLevel {
    pub(crate) fn id(self) -> &'static str {
        match self {
            HubLevel::Line => "line",
            HubLevel::Topic => "topic",
            HubLevel::Field => "field",
            HubLevel::Register => "register",
        }
    }

    /// 1 for a line hub up to 4 for the register.
    pub(crate) fn number(self) -> u8 {
        match self {
            HubLevel::Line => 1,
            HubLevel::Topic => 2,
            HubLevel::Field => 3,
            HubLevel::Register => 4,
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "line" | "线索" | "线索枢纽" => Ok(HubLevel::Line),
            "topic" | "主题" | "主题枢纽" => Ok(HubLevel::Topic),
            "field" | "领域" | "领域枢纽" => Ok(HubLevel::Field),
            "register" | "索引" | "登记簿" => Ok(HubLevel::Register),
            other => bail!(
                "unknown hub level `{other}`; expected line, topic, field, register, or empty for none"
            ),
        }
    }

    pub(crate) fn from_stored(value: Option<&str>) -> Option<Self> {
        value.and_then(|value| HubLevel::parse(value).ok())
    }
}

pub(crate) const LEVEL_NEEDS_HUB_CARD: &str =
    "only a Hub Card has a hub level (line, topic, field, or register)";

/// True when `value` names a hub level rather than an elaboration level.
pub(crate) fn names_hub_level(value: &str) -> bool {
    HubLevel::parse(value).is_ok()
}

/// Store a Hub Card's level; `None` clears it.
pub(crate) fn set_level(conn: &Connection, card: &Card, level: Option<HubLevel>) -> Result<()> {
    if level.is_some() && card.card_type != CardType::Hub {
        bail!(LEVEL_NEEDS_HUB_CARD);
    }
    conn.execute(
        "UPDATE cards SET hub_level = ?1 WHERE row_id = ?2",
        params![level.map(HubLevel::id), card.row_id],
    )?;
    Ok(())
}

/// The numbers behind the fixed policy. The book digest's hub manager chooses
/// them for each run; these are the defaults of the design.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Limits {
    /// A hub holds at most this many groups (H5).
    pub(crate) max_groups: usize,
    /// A hub holds at most this many member lines (H5).
    pub(crate) max_lines: usize,
    /// A member line's reason has at least this many characters (H8).
    pub(crate) min_reason: usize,
    /// A Topic tree with at least this many Cards requires a topic hub (H1).
    pub(crate) topic_min: usize,
    /// A branch with at least this many Cards requires a line hub (H2).
    pub(crate) line_min: usize,
    /// Two trees with at least this many Links between them are neighbours (H10).
    pub(crate) neighbour_links: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_groups: 7,
            max_lines: 25,
            min_reason: 8,
            topic_min: 7,
            line_min: 12,
            neighbour_links: 3,
        }
    }
}

impl Limits {
    /// Read the optional limits of a request; each one left out keeps its default.
    pub(crate) fn from_args(args: &Value) -> Result<Self> {
        let mut limits = Self::default();
        for (name, slot) in [
            ("max_groups", &mut limits.max_groups),
            ("max_lines", &mut limits.max_lines),
            ("min_reason", &mut limits.min_reason),
            ("topic_min", &mut limits.topic_min),
            ("line_min", &mut limits.line_min),
            ("neighbour_links", &mut limits.neighbour_links),
        ] {
            match args.get(name) {
                None | Some(Value::Null) => {}
                Some(value) => match value.as_u64() {
                    Some(number) if number >= 1 => *slot = number as usize,
                    _ => bail!("argument `{name}` must be a whole number of 1 or more"),
                },
            }
        }
        Ok(limits)
    }

    fn json(&self) -> Value {
        json!({
            "max_groups": self.max_groups,
            "max_lines": self.max_lines,
            "min_reason": self.min_reason,
            "topic_min": self.topic_min,
            "line_min": self.line_min,
            "neighbour_links": self.neighbour_links,
        })
    }
}

/// One list item of a hub body that holds a Link.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Line {
    pub(crate) group: Option<String>,
    /// The first Link of the item: the member.
    pub(crate) target: String,
    /// The text after the member's Link, without its marks; for the register,
    /// the term written before the first Link.
    pub(crate) reason: String,
    /// The Relation kind the item's words give its first Link.
    pub(crate) kind: &'static str,
    /// Every Link target of the item, in order.
    pub(crate) targets: Vec<String>,
    /// The 1-based body line the item sits on, so a report can name it.
    pub(crate) line: usize,
    /// The authored item without its list mark: the words a member match quotes (D202).
    pub(crate) text: String,
}

/// A hub body read as groups (`##` or `###` headings) and member lines (list
/// items with a Link). Text outside list items, such as the `范围` line, holds
/// no members.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Body {
    pub(crate) groups: Vec<String>,
    pub(crate) lines: Vec<Line>,
}

pub(crate) fn parse_body(body: &str) -> Body {
    let mut parsed = Body::default();
    let mut group: Option<String> = None;
    let mut fenced = false;
    for (index, raw) in body.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(heading) = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
        {
            let heading = heading.trim().to_string();
            parsed.groups.push(heading.clone());
            group = Some(heading);
            continue;
        }
        let Some(item) = list_item(trimmed) else {
            continue;
        };
        // Tolerant, so a member whose Link is malformed stays a member and its check reports it.
        let links = LinkBody::for_rendering(item);
        let Some(first) = links.occurrences().first() else {
            continue;
        };
        let targets = links
            .occurrences()
            .iter()
            .map(|occurrence| occurrence.target.to_string())
            .collect::<Vec<_>>();
        let after = &item[first.macro_range.end..];
        let before = &item[..first.macro_range.start];
        let kind = crate::model::read_links(item, str::to_string)
            .into_iter()
            .next()
            .map_or("refers", |read| read.kind.id());
        parsed.lines.push(Line {
            group: group.clone(),
            target: first.target.to_string(),
            reason: clean_reason(after),
            kind,
            targets,
            line: index + 1,
            text: item.trim().to_string(),
        });
        // The register's reason is its term, written before the first Link.
        if let Some(line) = parsed.lines.last_mut()
            && line.reason.is_empty()
        {
            line.reason = clean_reason(before);
        }
    }
    parsed
}

fn list_item(line: &str) -> Option<&str> {
    for mark in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(mark) {
            return Some(rest);
        }
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    if digits > 0 {
        let rest = &line[digits..];
        if let Some(rest) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            return Some(rest);
        }
    }
    None
}

/// The group headings a register's term rows live under. `词条` is the grammar of
/// `docs/luhmann-comparison-implementation-design.md` §8.4; `关键词` is the heading the earlier
/// hub compiler wrote, so an existing register stays readable. A row written before any heading is
/// read too: an existing register may have none.
pub(crate) const REGISTER_TERM_GROUPS: [&str; 2] = ["词条", "关键词"];

/// The group heading that declares a register's parts.
pub(crate) const REGISTER_PART_GROUP: &str = "词条分册";

/// One term row of a register: one term before a colon and one to three Links to the Cards that
/// carry the term's substance (§8.4). `line` is the 1-based line in the body, so a fault can name
/// the exact row (D297).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegisterRow {
    pub(crate) line: usize,
    pub(crate) term: String,
    /// Every Link target of the row, in order, repeats kept (the checks name duplicates).
    pub(crate) targets: Vec<String>,
    /// A description of how the row breaks the term grammar; `None` when it follows it.
    pub(crate) shape: Option<String>,
}

/// The term rows of a register body. The parser is deliberately tolerant: a row that breaks the
/// grammar stays a row, so a check can name it instead of letting it vanish.
pub(crate) fn register_rows(body: &str) -> Vec<RegisterRow> {
    let mut rows = Vec::new();
    let mut group: Option<String> = None;
    let mut fenced = false;
    for (index, raw) in body.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(heading) = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
        {
            group = Some(heading.trim().to_string());
            continue;
        }
        let in_terms = match group.as_deref() {
            None => true,
            Some(name) => REGISTER_TERM_GROUPS.contains(&name),
        };
        if !in_terms {
            continue;
        }
        let Some(item) = list_item(trimmed) else {
            continue;
        };
        let links = LinkBody::for_rendering(item);
        let occurrences = links.occurrences();
        let targets = occurrences
            .iter()
            .map(|occurrence| occurrence.target.to_string())
            .collect::<Vec<_>>();
        let first_link = occurrences.first().map_or(item.len(), |o| o.macro_range.start);
        let mut term = clean_reason(&item[..first_link]);
        let mut shape = None;
        match item[..first_link].find(['：', ':']) {
            Some(colon) => {
                term = item[..colon].trim().to_string();
                if term.is_empty() {
                    shape = Some("a term row is one term before `：`; this row has no term".into());
                }
            }
            None => {
                shape = Some(format!(
                    "a term row is one term before `：`; `{term}` has no `：`"
                ));
            }
        }
        if let Some(last) = occurrences.last() {
            let after = clean_reason(&item[last.macro_range.end..]);
            if !after.is_empty() && shape.is_none() {
                shape = Some(format!(
                    "the words after a term's Links (`{after}`) belong in the target Cards; the row is a term and its Links"
                ));
            }
        }
        rows.push(RegisterRow {
            line: index + 1,
            term,
            targets,
            shape,
        });
    }
    rows
}

/// The targets of a register's `词条分册` declaration, in declaration order (§8.4). A part's
/// identity is these Links plus its place as an immediate Side successor of the register.
pub(crate) fn register_part_targets(body: &str) -> Vec<String> {
    let mut group: Option<String> = None;
    let mut fenced = false;
    let mut targets = Vec::new();
    for raw in body.lines() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(heading) = trimmed
            .strip_prefix("### ")
            .or_else(|| trimmed.strip_prefix("## "))
        {
            group = Some(heading.trim().to_string());
            continue;
        }
        if group.as_deref() != Some(REGISTER_PART_GROUP) {
            continue;
        }
        let Some(item) = list_item(trimmed) else {
            continue;
        };
        for occurrence in LinkBody::for_rendering(item).occurrences() {
            if !targets.iter().any(|known| known == occurrence.target) {
                targets.push(occurrence.target.to_string());
            }
        }
    }
    targets
}

/// The policy faults of one register row (D289, D297, D051): its shape, one to three targets,
/// no repeated target, and every named Card existing. A row that names four Cards or no Card is
/// reported here and is not an entry.
pub(crate) fn register_row_faults(row: &RegisterRow, exists: &dyn Fn(&str) -> bool) -> Vec<Value> {
    let mut faults = Vec::new();
    if let Some(detail) = &row.shape {
        faults.push(json!({"fault": "shape", "detail": detail, "line": row.line, "term": row.term}));
    }
    if row.targets.is_empty() {
        faults.push(json!({
            "fault": "empty",
            "detail": format!("the term `{}` names no Card; name one to three", row.term),
            "line": row.line,
            "term": row.term,
        }));
    }
    if row.targets.len() > 3 {
        faults.push(json!({
            "fault": "oversize",
            "detail": format!("the term `{}` names {} Cards; name at most 3", row.term, row.targets.len()),
            "line": row.line,
            "term": row.term,
        }));
    }
    let mut seen = BTreeSet::new();
    for target in &row.targets {
        if !seen.insert(target) {
            faults.push(json!({
                "fault": "duplicate-target",
                "detail": format!("the term `{}` names [[{target}]] twice; name each Card once", row.term),
                "line": row.line,
                "term": row.term,
            }));
        } else if !exists(target) {
            faults.push(json!({
                "fault": "broken",
                "detail": format!("[[{target}]] of the term `{}` does not exist", row.term),
                "line": row.line,
                "term": row.term,
            }));
        }
    }
    faults
}

/// The words of a reason: Link macros, the `（枢纽）` mark, and leading
/// punctuation removed.
fn clean_reason(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        match rest[start..].find("]]") {
            Some(end) => rest = &rest[start + end + 2..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    let out = out.replace("（枢纽）", "").replace("(hub)", "");
    out.trim_matches(|ch: char| {
        ch.is_whitespace() || matches!(ch, ':' | '：' | '-' | '—' | '–' | '·' | '，' | ',' | '、')
    })
    .to_string()
}

/// Characters of a reason, not counting spaces and punctuation.
fn reason_chars(reason: &str) -> usize {
    reason
        .chars()
        .filter(|ch| ch.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(ch))
        .count()
}

/// The hubs that list a Card directly, highest level last. Only hubs whose body holds a Link to
/// the Card are parsed. A register lists all one to three targets of a term, so a Card named as a
/// term's second or third target is a member too (D051); that item also carries `term` and the
/// 0-based `target_index`.
pub(crate) fn hubs_of(snapshot: &Snapshot, address: &str) -> Vec<Value> {
    let link = format!("[[{address}]]");
    let mut hubs = snapshot
        .cards
        .iter()
        .filter(|card| card.card_type == CardType::Hub && card.address != address)
        .filter_map(|card| {
            let parsed = snapshot.parsed.get(&card.address)?;
            if !parsed.body.contains(&link) {
                return None;
            }
            let mut listed = Vec::new();
            if card.hub_level == Some(HubLevel::Register) {
                for row in register_rows(&parsed.body) {
                    for (index, target) in row.targets.iter().enumerate() {
                        if target == address {
                            listed.push((row.term.clone(), index));
                        }
                    }
                }
            } else if parse_body(&parsed.body)
                .lines
                .iter()
                .any(|line| line.target == address)
            {
                listed.push((String::new(), 0));
            }
            if listed.is_empty() {
                return None;
            }
            Some((card.hub_level.map_or(0, HubLevel::number), card, listed))
        })
        .collect::<Vec<_>>();
    hubs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.address.cmp(&b.1.address)));
    let mut values = Vec::new();
    for (_, hub, listed) in hubs {
        for (term, index) in listed {
            let mut value = json!({
                "address": hub.address,
                "title": snapshot.title(&hub.address),
                "level": hub.hub_level.map(HubLevel::id),
            });
            if hub.hub_level == Some(HubLevel::Register) {
                value["term"] = json!(term);
                value["target_index"] = json!(index);
            }
            values.push(value);
        }
    }
    values
}

/// Every hub of the archive with its parsed body, read once per request.
pub(crate) struct Graph<'a> {
    snapshot: &'a Snapshot,
    hubs: BTreeMap<String, (Option<HubLevel>, Body)>,
    /// Hub → the hubs that list it.
    upper: BTreeMap<String, BTreeSet<String>>,
    /// Per-request indexed occurrences for policy projections. The full
    /// Snapshot path remains the independent GUI/test oracle.
    indexed_links: Option<&'a BTreeMap<String, Vec<String>>>,
}

impl<'a> Graph<'a> {
    pub(crate) fn new(snapshot: &'a Snapshot) -> Self {
        let mut hubs = BTreeMap::new();
        for card in &snapshot.cards {
            if card.card_type == CardType::Hub {
                let body = snapshot
                    .parsed
                    .get(&card.address)
                    .map(|parsed| parse_body(&parsed.body))
                    .unwrap_or_default();
                hubs.insert(card.address.clone(), (card.hub_level, body));
            }
        }
        let mut upper: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (address, (_, body)) in &hubs {
            for target in body.lines.iter().flat_map(|line| line.targets.iter()) {
                if hubs.contains_key(target) && target != address {
                    upper
                        .entry(target.clone())
                        .or_default()
                        .insert(address.clone());
                }
            }
        }
        Self {
            snapshot,
            hubs,
            upper,
            indexed_links: None,
        }
    }

    pub(crate) fn with_links(
        snapshot: &'a Snapshot,
        links: &'a BTreeMap<String, Vec<String>>,
    ) -> Self {
        let mut graph = Self::new(snapshot);
        graph.indexed_links = Some(links);
        graph
    }

    fn outbound(&self, at: &str) -> Vec<String> {
        if let Some(links) = self.indexed_links {
            return links.get(at).cloned().unwrap_or_default();
        }
        self.snapshot
            .outbound(at)
            .into_iter()
            .filter_map(|link| link["address"].as_str().map(str::to_string))
            .collect()
    }

    fn level(&self, address: &str) -> Option<HubLevel> {
        self.hubs.get(address).and_then(|(level, _)| *level)
    }

    fn body(&self, address: &str) -> Option<&Body> {
        self.hubs.get(address).map(|(_, body)| body)
    }

    /// The hubs a hub lists, in body order.
    fn lower(&self, address: &str) -> Vec<String> {
        let mut seen = BTreeSet::new();
        self.body(address)
            .map(|body| {
                body.lines
                    .iter()
                    .flat_map(|line| line.targets.iter())
                    .filter(|target| self.hubs.contains_key(*target) && *target != address)
                    .filter(|target| seen.insert((*target).clone()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every Card a hub reaches through its own lines and its lower hubs.
    fn covered(&self, address: &str) -> BTreeSet<String> {
        let mut covered = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut stack = vec![address.to_string()];
        while let Some(hub) = stack.pop() {
            if !visited.insert(hub.clone()) {
                continue;
            }
            if let Some(body) = self.body(&hub) {
                for target in body.lines.iter().flat_map(|line| line.targets.iter()) {
                    covered.insert(target.clone());
                    if self.hubs.contains_key(target) {
                        stack.push(target.clone());
                    }
                }
            }
        }
        covered
    }

    fn in_cycle(&self, address: &str) -> bool {
        let mut visited = BTreeSet::new();
        let mut stack = self.lower(address);
        while let Some(hub) = stack.pop() {
            if hub == address {
                return true;
            }
            if visited.insert(hub.clone()) {
                stack.extend(self.lower(&hub));
            }
        }
        false
    }

    /// The Cards a hub is responsible for (its Hub range), without tree roots
    /// and hubs.
    fn range(&self, hub: &Card) -> Vec<&'a Card> {
        let level = hub.hub_level;
        let body = self.body(&hub.address);
        let field_trees = body
            .map(|body| {
                body.lines
                    .iter()
                    .map(|line| crate::tree_id(&line.target).to_string())
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        let anchor = crate::parent_location(&hub.address);
        let tree = crate::tree_id(&hub.address).to_string();
        self.snapshot
            .cards
            .iter()
            .filter(|card| card.card_type != CardType::Hub && !crate::is_tree_root(card))
            .filter(|card| match level {
                Some(HubLevel::Line) => anchor.as_deref().is_some_and(|anchor| {
                    card.address == anchor || card.address.starts_with(&format!("{anchor}|"))
                }),
                Some(HubLevel::Topic) | None => crate::tree_id(&card.address) == tree,
                Some(HubLevel::Field) => field_trees.contains(crate::tree_id(&card.address)),
                Some(HubLevel::Register) => card.workspace == hub.workspace,
            })
            .collect()
    }

    /// Links between the hub's tree and every other Topic tree, either way. A
    /// Literature tree is the source a Topic draws on, not a neighbour subject.
    fn tree_links(&self, tree: &str) -> BTreeMap<String, usize> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for card in &self.snapshot.cards {
            let source_tree = crate::tree_id(&card.address);
            for target in self.outbound(&card.address) {
                if !self.snapshot.by_address.contains_key(&target) {
                    continue;
                }
                let target_tree = crate::tree_id(&target);
                let other = if source_tree == tree && target_tree != tree {
                    target_tree
                } else if target_tree == tree && source_tree != tree {
                    source_tree
                } else {
                    continue;
                };
                if !crate::valid_topic_id(other) {
                    continue;
                }
                *counts.entry(other.to_string()).or_default() += 1;
            }
        }
        counts
    }

    /// The report of one hub: its lines, groups, range, upper and lower
    /// hubs, faults (H1 to H11), and the advice lists.
    pub(crate) fn report(&self, at: &str, limits: &Limits) -> Result<Value> {
        let Some(hub) = self.snapshot.by_address.get(at) else {
            bail!("card `{at}` does not exist");
        };
        if hub.card_type != CardType::Hub {
            bail!("`{at}` is not a Hub Card; set type `hub` and a level first");
        }
        let (faults, advice) = self.check(hub, limits);
        let body = self.body(at).cloned().unwrap_or_default();
        let lines = body
            .lines
            .iter()
            .map(|line| {
                let card = self.snapshot.by_address.get(&line.target);
                json!({
                    "group": line.group,
                    "address": line.target,
                    "title": self.snapshot.title(&line.target),
                    "type": card.map(|card| card.card_type.id()),
                    "level": card.and_then(|card| card.level_id()),
                    "tree": crate::tree_id(&line.target),
                    "kind": line.kind,
                    "reason": line.reason,
                    "exists": card.is_some(),
                })
            })
            .collect::<Vec<_>>();
        let range = self.range(hub);
        Ok(json!({
            "address": hub.address,
            "title": self.snapshot.title(&hub.address),
            "workspace": hub.workspace.id(),
            "level": hub.hub_level.map(HubLevel::id),
            "anchor": crate::parent_location(&hub.address),
            "groups": body.groups,
            "lines": lines,
            "lower": self.lower(at).iter().map(|address| self.reference(address)).collect::<Vec<_>>(),
            "upper": self.upper.get(at).into_iter().flatten().map(|address| self.reference(address)).collect::<Vec<_>>(),
            "range": range.len(),
            "faults": faults,
            "mirror": advice.mirror,
            "candidates": advice.candidates,
            "unreached": advice.unreached,
            "unreached_count": advice.unreached_count,
            "limits": limits.json(),
        }))
    }

    fn reference(&self, address: &str) -> Value {
        json!({
            "address": address,
            "title": self.snapshot.title(address),
            "level": self.level(address).map(HubLevel::id),
        })
    }

    fn check(&self, hub: &Card, limits: &Limits) -> (Vec<Value>, Advice) {
        let mut faults = Vec::new();
        let fault = |faults: &mut Vec<Value>, id: &str, detail: String| {
            faults.push(json!({ "fault": id, "detail": detail }));
        };
        let address = hub.address.as_str();
        let body = self.body(address).cloned().unwrap_or_default();
        let level = hub.hub_level;
        if level.is_none() {
            fault(
                &mut faults,
                "level",
                "a Hub Card needs a hub level: line, topic, field, or register".into(),
            );
        }
        // `anchor-title` (D130, D169): a line or topic hub whose title copies the title of
        // the Card it hangs under tells the reader nothing about its navigation role. Case
        // and whitespace are normalized; broad semantic similarity is not a refusal.
        if matches!(level, Some(HubLevel::Line) | Some(HubLevel::Topic))
            && let Some(anchor) = crate::parent_location(address)
        {
            let own = normalized_title(&self.snapshot.title(address));
            let upper = normalized_title(&self.snapshot.title(&anchor));
            if !own.is_empty() && own == upper {
                fault(
                    &mut faults,
                    "anchor-title",
                    format!(
                        "the title repeats the anchor `{anchor}` title; name what the hub navigates, for example 消费需求的分解入口"
                    ),
                );
            }
        }
        // H5: size. The register's size is its number of terms; each term names 1 to 3 ways in,
        // and its rows are checked by their own grammar (D050, D289, D297).
        if level == Some(HubLevel::Register) {
            let raw = self
                .snapshot
                .parsed
                .get(address)
                .map(|parsed| parsed.body.clone())
                .unwrap_or_default();
            for row in register_rows(&raw) {
                let exists = |at: &str| self.snapshot.by_address.contains_key(at);
                faults.extend(register_row_faults(&row, &exists));
            }
        } else {
            // The 邻近 group points out of the subject; it is not part of the hub's size.
            let lines = body
                .lines
                .iter()
                .filter(|line| !is_neighbour_group(line.group.as_deref()))
                .count();
            let groups = body
                .groups
                .iter()
                .filter(|group| !is_neighbour_group(Some(group.as_str())))
                .count();
            if lines > limits.max_lines {
                fault(
                    &mut faults,
                    "oversize",
                    format!(
                        "{lines} lines outside 邻近; at most {} (split a group into a lower hub)",
                        limits.max_lines
                    ),
                );
            }
            if groups > limits.max_groups {
                fault(
                    &mut faults,
                    "oversize",
                    format!(
                        "{groups} groups outside 邻近; at most {}",
                        limits.max_groups
                    ),
                );
            }
            if groups < 2 && lines > 8 {
                fault(
                    &mut faults,
                    "flat",
                    format!(
                        "{lines} lines in {groups} group; sort them into 2 or more named groups"
                    ),
                );
            }
        }
        // H8: every line says what its Card adds; a register line names its term.
        let min_reason = if level == Some(HubLevel::Register) {
            1
        } else {
            limits.min_reason
        };
        for line in &body.lines {
            if reason_chars(&line.reason) < min_reason {
                fault(
                    &mut faults,
                    "bare",
                    format!(
                        "[[{}]] has no reason of {} or more characters",
                        line.target, min_reason
                    ),
                );
            }
        }
        // A register row's own target checks run above; every other line's member is its first
        // Link (a register names one to three members, D051).
        if level != Some(HubLevel::Register) {
            for line in &body.lines {
                if !self.snapshot.by_address.contains_key(&line.target) {
                    fault(
                        &mut faults,
                        "broken",
                        format!("[[{}]] does not exist", line.target),
                    );
                }
            }
        }
        // H6: a hub lists only hubs of a lower level, and no cycle. A line hub may also list the
        // line hub of a branch inside its own branch.
        let own = level.map_or(0, HubLevel::number);
        for lower in self.lower(address) {
            let lower_level = self.level(&lower).map_or(0, HubLevel::number);
            let nested = level == Some(HubLevel::Line)
                && self.level(&lower) == Some(HubLevel::Line)
                && nested_line(address, &lower);
            if level.is_some() && lower_level >= own && !nested {
                fault(
                    &mut faults,
                    "upward",
                    format!(
                        "lists [[{lower}]], a hub of level {} or higher",
                        self.level(&lower).map_or("none", HubLevel::id)
                    ),
                );
            }
        }
        if self.in_cycle(address) {
            fault(
                &mut faults,
                "cycle",
                "a chain of lower hubs leads back to this hub".into(),
            );
        }
        // H7 (and H3, H4): every hub but the register is listed by a higher hub.
        if let Some(level) = level
            && level != HubLevel::Register
        {
            let anchored = self.upper.get(address).is_some_and(|upper| {
                upper.iter().any(|upper| {
                    self.level(upper)
                        .is_some_and(|upper| upper.number() > level.number())
                })
            });
            if !anchored {
                let wanted = match level {
                    HubLevel::Line => "a topic hub",
                    HubLevel::Topic => "a field hub or the register",
                    _ => "the register",
                };
                fault(
                    &mut faults,
                    "unanchored",
                    format!("no hub of a higher level lists it; {wanted} should"),
                );
            }
            if level == HubLevel::Topic {
                let root = format!("{}/0", crate::tree_id(address));
                let linked = self.outbound(&root).iter().any(|target| target == address);
                if self.snapshot.by_address.contains_key(&root) && !linked {
                    fault(
                        &mut faults,
                        "unanchored",
                        format!("the Topic body [[{root}]] does not link its topic hub"),
                    );
                }
                // H1: one topic hub per tree.
                let tree = crate::tree_id(address);
                for (other, (other_level, _)) in &self.hubs {
                    if other != address
                        && *other_level == Some(HubLevel::Topic)
                        && crate::tree_id(other) == tree
                    {
                        fault(
                            &mut faults,
                            "duplicate",
                            format!("[[{other}]] is another topic hub of tree {tree}"),
                        );
                    }
                }
            }
        }
        let range = self.range(hub);
        let covered = self.covered(address);
        // H9: every Model Card and every line hub of the range is reached.
        if matches!(level, Some(HubLevel::Line) | Some(HubLevel::Topic)) {
            for card in &range {
                if card.card_type == CardType::Model
                    && card.facet.is_none()
                    && !covered.contains(&card.address)
                {
                    fault(
                        &mut faults,
                        "unlisted",
                        format!(
                            "the Model Card [[{}]] is in no line of this hub",
                            card.address
                        ),
                    );
                }
            }
            for (other, (other_level, _)) in &self.hubs {
                if other != address
                    && *other_level == Some(HubLevel::Line)
                    && range_contains_anchor(&range, other)
                    && !covered.contains(other)
                {
                    fault(
                        &mut faults,
                        "unlisted",
                        format!("the line hub [[{other}]] is in no line of this hub"),
                    );
                }
            }
        }
        // H10: a topic hub whose tree has neighbours names at least one of them.
        if level == Some(HubLevel::Topic) {
            let tree = crate::tree_id(address);
            let mut neighbours = self
                .tree_links(tree)
                .into_iter()
                .filter(|(_, count)| *count >= limits.neighbour_links)
                .collect::<Vec<_>>();
            neighbours.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            let outside = body
                .lines
                .iter()
                .any(|line| crate::tree_id(&line.target) != tree);
            if !neighbours.is_empty() && !outside {
                let named = neighbours
                    .iter()
                    .take(5)
                    .map(|(tree, count)| format!("{tree} ({count} Links)"))
                    .collect::<Vec<_>>()
                    .join(", ");
                fault(
                    &mut faults,
                    "isolated",
                    format!("lists no Card outside tree {tree}; its neighbours: {named}"),
                );
            }
        }
        (faults, self.advice(hub, &body, &range, &covered))
    }

    fn advice(
        &self,
        hub: &Card,
        body: &Body,
        range: &[&Card],
        covered: &BTreeSet<String>,
    ) -> Advice {
        // H11: a hub that lists only its own tree in address order repeats the Folgezettel map.
        let tree = crate::tree_id(&hub.address);
        let own = body
            .lines
            .iter()
            .all(|line| crate::tree_id(&line.target) == tree);
        let ordered = body.lines.windows(2).all(|pair| {
            crate::model::address_order(&pair[0].target, &pair[1].target)
                != std::cmp::Ordering::Greater
        });
        let mirror = matches!(hub.hub_level, Some(HubLevel::Line) | Some(HubLevel::Topic))
            && body.lines.len() >= 4
            && own
            && ordered;
        // H12: Cards of the range that link two or more members but are in no line.
        let mut candidates = Vec::new();
        for card in range {
            if covered.contains(&card.address) {
                continue;
            }
            let links = self
                .outbound(&card.address)
                .into_iter()
                .filter(|target| covered.contains(target))
                .count();
            if links >= 2 {
                candidates.push(json!({
                    "address": card.address,
                    "title": self.snapshot.title(&card.address),
                    "links": links,
                }));
            }
        }
        candidates.truncate(30);
        // H13: a Card is reached when any hub lists it or a Card above it in its tree.
        let listed = self
            .hubs
            .values()
            .flat_map(|(_, body)| body.lines.iter().flat_map(|line| line.targets.iter()))
            .cloned()
            .collect::<BTreeSet<_>>();
        let unreached = range
            .iter()
            .filter(|card| !reached(&card.address, &listed))
            .map(|card| card.address.clone())
            .collect::<Vec<_>>();
        Advice {
            mirror,
            candidates,
            unreached_count: unreached.len(),
            unreached: unreached.into_iter().take(40).collect(),
        }
    }

    /// Every hub of a Workspace with its level, upper and lower hubs, and
    /// faults, and the hubs the policy requires but the Workspace lacks.
    pub(crate) fn list(&self, workspace: Option<Workspace>, limits: &Limits) -> Value {
        let in_workspace =
            |card: &Card| workspace.is_none_or(|workspace| card.workspace == workspace);
        let mut hubs = Vec::new();
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for card in self
            .snapshot
            .cards
            .iter()
            .filter(|card| card.card_type == CardType::Hub && in_workspace(card))
        {
            let (faults, advice) = self.check(card, limits);
            *counts
                .entry(card.hub_level.map_or("none", HubLevel::id))
                .or_default() += 1;
            hubs.push(json!({
                "address": card.address,
                "title": self.snapshot.title(&card.address),
                "workspace": card.workspace.id(),
                "level": card.hub_level.map(HubLevel::id),
                "tree": crate::tree_id(&card.address),
                "lines": self.body(&card.address).map_or(0, |body| body.lines.len()),
                "upper": self.upper.get(&card.address).into_iter().flatten().cloned().collect::<Vec<_>>(),
                "lower": self.lower(&card.address),
                "faults": faults.iter().filter_map(|fault| fault["fault"].as_str().map(str::to_string)).collect::<Vec<_>>(),
                "mirror": advice.mirror,
                "unreached": advice.unreached_count,
            }));
        }
        json!({
            "workspace": workspace.map(Workspace::id),
            "limits": limits.json(),
            "counts": counts,
            "hubs": hubs,
            "missing": self.missing(workspace, limits),
        })
    }

    /// H1, H2, and H4: the required hubs that do not exist yet.
    fn missing(&self, workspace: Option<Workspace>, limits: &Limits) -> Vec<Value> {
        let mut missing = Vec::new();
        let roots = self
            .snapshot
            .cards
            .iter()
            .filter(|card| card.is_topic)
            .filter(|card| workspace.is_none_or(|workspace| card.workspace == workspace));
        for root in roots {
            let tree = crate::tree_id(&root.address);
            let members = self
                .snapshot
                .cards
                .iter()
                .filter(|card| !card.is_topic && crate::tree_id(&card.address) == tree)
                .collect::<Vec<_>>();
            let hubs_here = members
                .iter()
                .filter(|card| card.card_type == CardType::Hub)
                .collect::<Vec<_>>();
            let content = members
                .iter()
                .filter(|card| card.card_type != CardType::Hub)
                .count();
            // The register tree holds hubs only; it requires none of its own.
            if content == 0 && !hubs_here.is_empty() {
                continue;
            }
            if content >= limits.topic_min
                && !hubs_here
                    .iter()
                    .any(|hub| hub.hub_level == Some(HubLevel::Topic))
            {
                missing.push(json!({
                    "level": "topic",
                    "tree": tree,
                    "anchor": format!("{tree}/1"),
                    "title": self.snapshot.title(&root.address),
                    "cards": content,
                }));
            }
            // A branch: a Side successor that is not part of a model.
            for branch in members.iter().filter(|card| {
                card.card_type == CardType::Idea
                    && crate::valid_side_label(crate::last_segment(&card.address))
            }) {
                let prefix = format!("{}|", branch.address);
                let inside = members
                    .iter()
                    .filter(|card| card.card_type != CardType::Hub)
                    .filter(|card| {
                        card.address == branch.address || card.address.starts_with(&prefix)
                    })
                    .collect::<Vec<_>>();
                let models = inside
                    .iter()
                    .filter(|card| card.card_type == CardType::Model && card.facet.is_none())
                    .count();
                if inside.len() < limits.line_min && models < 2 {
                    continue;
                }
                let has_hub = hubs_here.iter().any(|hub| {
                    hub.hub_level == Some(HubLevel::Line)
                        && crate::parent_location(&hub.address).as_deref()
                            == Some(branch.address.as_str())
                });
                if !has_hub {
                    missing.push(json!({
                        "level": "line",
                        "tree": tree,
                        "anchor": branch.address,
                        "title": self.snapshot.title(&branch.address),
                        "cards": inside.len(),
                        "models": models,
                    }));
                }
            }
        }
        for workspace in [Workspace::Human, Workspace::Agent]
            .into_iter()
            .filter(|candidate| workspace.is_none_or(|workspace| workspace == *candidate))
        {
            let hubs = self
                .snapshot
                .cards
                .iter()
                .filter(|card| card.card_type == CardType::Hub && card.workspace == workspace)
                .collect::<Vec<_>>();
            let registers = hubs
                .iter()
                .filter(|card| card.hub_level == Some(HubLevel::Register))
                .count();
            if !hubs.is_empty() && registers == 0 {
                missing.push(json!({ "level": "register", "workspace": workspace.id() }));
            }
            if registers > 1 {
                missing.push(json!({
                    "level": "register",
                    "workspace": workspace.id(),
                    "fault": "duplicate",
                    "registers": registers,
                }));
            }
        }
        missing
    }
}

struct Advice {
    mirror: bool,
    candidates: Vec<Value>,
    unreached: Vec<String>,
    unreached_count: usize,
}

/// True when `address` or a Card above it in its tree is listed.
fn reached(address: &str, listed: &BTreeSet<String>) -> bool {
    if listed.contains(address) {
        return true;
    }
    let mut current = address;
    while let Some((prefix, _)) = current.rsplit_once('|') {
        if listed.contains(prefix) {
            return true;
        }
        current = prefix;
    }
    false
}

/// True for the group that points out of the subject (`邻近`, `neighbours`).
fn is_neighbour_group(group: Option<&str>) -> bool {
    group.is_some_and(|group| {
        let group = group.trim();
        group.starts_with("邻近") || group.eq_ignore_ascii_case("neighbours")
    })
}

/// A title for exact comparison: whitespace collapsed, case ignored. `anchor-title` uses
/// only this much normalization — a differently worded navigation title is no fault.
fn normalized_title(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// True when `inner`'s anchor lies strictly inside `outer`'s branch: the line hub of a
/// sub-branch of the outer hub's line.
fn nested_line(outer: &str, inner: &str) -> bool {
    match (crate::parent_location(outer), crate::parent_location(inner)) {
        (Some(outer), Some(inner)) => inner.starts_with(&format!("{outer}|")),
        _ => false,
    }
}

/// True when a line hub's anchor lies in the range.
fn range_contains_anchor(range: &[&Card], hub: &str) -> bool {
    crate::parent_location(hub)
        .is_some_and(|anchor| range.iter().any(|card| card.address == anchor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_levels_parse_in_both_languages() {
        for level in [
            HubLevel::Line,
            HubLevel::Topic,
            HubLevel::Field,
            HubLevel::Register,
        ] {
            assert_eq!(HubLevel::parse(level.id()).unwrap(), level);
        }
        assert_eq!(HubLevel::parse("主题").unwrap(), HubLevel::Topic);
        assert_eq!(HubLevel::parse("索引").unwrap(), HubLevel::Register);
        assert!(HubLevel::parse("concept").is_err());
        assert!(names_hub_level("field"));
        assert!(!names_hub_level("formal"));
    }

    #[test]
    fn hub_bodies_read_groups_members_reasons_and_kinds() {
        let body = "范围：主题 130 的全部卡。\n\n## 逆向选择\n- [[130/2]]：知情者按私人信息决定是否交易。\n- [[130/2|f|c]]（枢纽）：多重均衡一支。\n\n## 邻近\n- 对照 [[122/1]]：信号传递是知情一方的应对。\n\n```\n- [[9/9]] 代码里不算\n```\n";
        let parsed = parse_body(body);
        assert_eq!(parsed.groups, vec!["逆向选择", "邻近"]);
        assert_eq!(parsed.lines.len(), 3);
        assert_eq!(parsed.lines[0].target, "130/2");
        assert_eq!(parsed.lines[0].reason, "知情者按私人信息决定是否交易。");
        assert_eq!(parsed.lines[1].reason, "多重均衡一支。");
        assert_eq!(parsed.lines[2].group.as_deref(), Some("邻近"));
        assert_eq!(parsed.lines[2].kind, "contrasts");
    }

    #[test]
    fn a_member_line_with_a_malformed_link_is_kept_for_the_checks() {
        let parsed = parse_body("## 起点\n- [[0/1]]：第一张。\n- [[0/01]]：地址写错了。\n");
        assert_eq!(parsed.lines.len(), 2);
        assert_eq!(parsed.lines[1].target, "0/01");
        assert_eq!(parsed.lines[1].reason, "地址写错了。");
    }

    #[test]
    fn register_terms_are_the_reason_of_their_line() {
        let parsed = parse_body("- 逆向选择：[[130/1|b]]、[[900/1|a]]\n");
        assert_eq!(parsed.lines[0].reason, "逆向选择");
        assert_eq!(parsed.lines[0].targets.len(), 2);
    }

    #[test]
    fn a_card_is_reached_through_a_listed_card_above_it() {
        let listed = BTreeSet::from(["130/2|f".to_string()]);
        assert!(reached("130/2|f|1|a", &listed));
        assert!(reached("130/2|f", &listed));
        assert!(!reached("130/2|g", &listed));
        assert!(!reached("130/3", &listed));
    }

    #[test]
    fn a_sub_branch_line_hub_nests_in_its_outer_line_hub() {
        assert!(nested_line("130/2|f|b", "130/2|f|a|a"));
        assert!(!nested_line("130/2|f|a|a", "130/2|f|b"));
        assert!(!nested_line("130/2|f|b", "130/2|g|b"));
        assert!(is_neighbour_group(Some("邻近")));
        assert!(!is_neighbour_group(Some("均衡")));
        assert!(!is_neighbour_group(None));
    }

    #[test]
    fn reasons_count_words_not_marks() {
        assert_eq!(reason_chars("：多重均衡。"), 4);
        assert_eq!(reason_chars(" - ok"), 2);
    }
}
