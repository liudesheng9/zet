//! Hub Cards: ways into one subject.
//!
//! A Hub Card (type `hub`) lists, in named groups, Links to the Cards and the
//! lower Hub Cards that carry its subject, one list item each with a reason.
//! Its Hub level says how wide the subject is: `line` (one line of thought),
//! `topic` (one Topic tree), `field` (several Topic trees), or `register`
//! (the Workspace). ZT computes each hub's members, range, upper and lower
//! hubs, and faults from the stored Cards; it reports faults and never
//! enforces them. See `docs/hub-cards-design.md`.

use anyhow::{Context, Result, bail};
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

/// True for a body group that lists ways into other Cards rather than stating anything itself:
/// the neighbour group, a Topic body's entry section, and a register's term and part groups.
/// A term occurrence derivation reads prose, so it skips these sections (§9.1, D196).
pub(crate) fn is_navigation_group(group: &str) -> bool {
    let group = group.trim();
    is_neighbour_group(Some(group))
        || is_entry_group(Some(group))
        || REGISTER_TERM_GROUPS.contains(&group)
        || group == REGISTER_PART_GROUP
}

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

/// One declaration of a register's `词条分册` section: the accepted label of the part's initial
/// range and the Link of the part Card (§8.4, D177, D181). A part's identity is that Link plus its
/// place as an immediate Side successor of the register (D182); the label carries the accepted
/// pinyin grouping the compiler planned and is never recomputed at read time (D184).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegisterPartRow {
    /// The 1-based line in the register body.
    pub(crate) line: usize,
    /// The words of the row that are not its Link: `A–F`, `S–Z、其他`, `其他`, `S–Z(2)`.
    pub(crate) label: String,
    /// The part Card the row names; `None` when the row names no Card at all.
    pub(crate) address: Option<String>,
}

/// The declared parts of a register, in declaration order (§8.4). The same row is read whether the
/// label comes before the Link (`- A–F：[[3/1]]`) or after it (`- [[3/1]] A–F`), so a hand-written
/// declaration and the compiler's own rows read the same.
pub(crate) fn register_part_rows(body: &str) -> Vec<RegisterPartRow> {
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
        if group.as_deref() != Some(REGISTER_PART_GROUP) {
            continue;
        }
        let Some(item) = list_item(trimmed) else {
            continue;
        };
        let links = LinkBody::for_rendering(item);
        let occurrences = links.occurrences();
        rows.push(RegisterPartRow {
            line: index + 1,
            label: part_label(item),
            address: occurrences.first().map(|first| first.target.to_string()),
        });
    }
    rows
}

/// The label words of one part declaration: the row without its Links, trimmed of the separators a
/// row may put around them. The accepted range labels keep their own en dash and their `、`.
fn part_label(item: &str) -> String {
    let mut words = String::new();
    let mut rest = item;
    while let Some(start) = rest.find("[[") {
        words.push_str(&rest[..start]);
        match rest[start..].find("]]") {
            Some(end) => rest = &rest[start + end + 2..],
            None => {
                rest = "";
            }
        }
    }
    words.push_str(rest);
    words
        .trim_matches(|ch: char| {
            ch.is_whitespace() || matches!(ch, ':' | '：' | '-' | '—' | '·' | '，' | ',' | '、')
        })
        .to_string()
}

/// The Cards that can be a register's part by place alone: its immediate Side successors in the
/// same Workspace, in address order (D182). A part is one of these that the root also declares and
/// that holds `词条` rows; the read adapter loads their bodies so the projection can name an
/// undeclared one instead of treating it as a new register.
pub(crate) fn register_side_successors(snapshot: &Snapshot, register: &str) -> Vec<String> {
    let Some(own) = snapshot.by_address.get(register) else {
        return Vec::new();
    };
    let mut found = snapshot
        .cards
        .iter()
        .filter(|card| card.workspace == own.workspace)
        .filter(|card| {
            crate::immediate_side_label(&card.address, register)
                .is_some_and(|label| card.address == format!("{register}|{label}"))
        })
        .map(|card| card.address.clone())
        .collect::<Vec<_>>();
    found.sort();
    found
}

/// The targets of a register's `词条分册` declaration, in declaration order (§8.4): every Link of
/// every row, once. A part's identity is these Links plus its place as an immediate Side successor
/// of the register.
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
            let mut listed = Vec::new();
            if card.hub_level == Some(HubLevel::Register) {
                for row in register_rows(&parsed.body) {
                    for (index, target) in row.targets.iter().enumerate() {
                        if target == address && !listed.contains(&(row.term.clone(), index)) {
                            listed.push((row.term.clone(), index));
                        }
                    }
                }
                // A term written in a declared part names the part's Cards, so its targets are
                // members of the register too (D187, §8.4). A part's body is read when the
                // caller's read includes it; a truncated read reports only what it holds.
                for part in register_part_rows(&parsed.body) {
                    let Some(part) = part.address else { continue };
                    let Some(part) = snapshot.parsed.get(&part) else {
                        continue;
                    };
                    for row in register_rows(&part.body) {
                        for (index, target) in row.targets.iter().enumerate() {
                            if target == address && !listed.contains(&(row.term.clone(), index)) {
                                listed.push((row.term.clone(), index));
                            }
                        }
                    }
                }
            } else if parsed.body.contains(&link)
                && parse_body(&parsed.body)
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

/// The source evidence of one Card (design §4.3, D068-D072): the Citation keys its `来源` lines
/// name, else the Literature tree it is filed in. A Model part falls back to the evidence of its
/// model's head when it has none of its own, and the second element names that head. An empty set
/// is unknown; a set of several is mixed, never forced to one book.
///
/// This is the one derivation: `Graph` reads it through the bodies one hub report loaded, and the
/// distant selection (`src/entry.rs`) reads it through the Snapshot (S19, #108).
pub(crate) fn source_evidence(snapshot: &Snapshot, address: &str) -> (Vec<String>, Option<String>) {
    let body = |at: &str| snapshot.parsed.get(at).map(|parsed| parsed.body.clone());
    source_evidence_with(snapshot, address, &body)
}

/// [`source_evidence`] over a supplied body reader: the indexed hub read holds only the bodies its
/// report needs, so it does not read every ordinary body from the Snapshot again.
fn source_evidence_with(
    snapshot: &Snapshot,
    address: &str,
    body: &dyn Fn(&str) -> Option<String>,
) -> (Vec<String>, Option<String>) {
    let mut sources: Vec<String> = Vec::new();
    let push = |sources: &mut Vec<String>, source: String| {
        if !source.is_empty() && !sources.iter().any(|seen| seen.eq_ignore_ascii_case(&source)) {
            sources.push(source);
        }
    };
    if let Some(body) = body(address) {
        for line in body.lines() {
            let line = line.trim();
            if !is_source_line(line) {
                continue;
            }
            for occurrence in LinkBody::for_rendering(line).occurrences() {
                if let Some(source) = literature_tree_of(snapshot, occurrence.target) {
                    push(&mut sources, source);
                }
            }
        }
    }
    if sources.is_empty()
        && let Some(source) = literature_tree_of(snapshot, address)
    {
        push(&mut sources, source);
    }
    if sources.is_empty()
        && let Some(card) = snapshot.by_address.get(address)
        && card.facet.is_some()
        && let Some(head) = snapshot.model_head(card)
        && head != address
    {
        let (derived, _) = source_evidence_with(snapshot, &head, body);
        return (derived, Some(head));
    }
    (sources, None)
}

/// The Literature tree a Link target names: its Citation key, when the target is a Literature Card
/// or a Card of one.
fn literature_tree_of(snapshot: &Snapshot, target: &str) -> Option<String> {
    let tree = crate::tree_id(target);
    let root = crate::tree_root_address(tree);
    let root_card = snapshot.by_address.get(&root)?;
    root_card.is_lit.then(|| tree.to_string())
}

/// The precheck of a Hub Card draft that is not saved yet (S17, #118; design §9.6, D235-D237):
/// the body rules `Graph::check` applies to a saved hub, plus the placement rules of filing. It
/// reads the Snapshot and answers one value; it writes nothing, so the composer runs it on every
/// edit of the desk.
///
/// The request names the whole Card `text`, the `level` the draft is filed at (`line`, `topic`, or
/// `field`), how it is filed — `how: "side"` makes it a side successor of `at`, `how: "edit"`
/// rewrites the hub already at `at` — and `at`, the Card it is filed under. The answer is
/// `{ok, level, title, at, anchor, tree, workspace, lines, faults}`.
pub(crate) fn precheck(snapshot: &Snapshot, args: &Value) -> Result<Value> {
    let text = args
        .get("text")
        .and_then(Value::as_str)
        .context("`text` must be the draft's Card text")?;
    let level_name = args
        .get("level")
        .and_then(Value::as_str)
        .context("`level` must name the hub level the draft is filed at: line, topic, or field")?;
    let at = args
        .get("at")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|at| !at.is_empty())
        .context("`at` must name the Card the draft is filed under")?;
    let edit = match args.get("how").and_then(Value::as_str) {
        Some("edit") => true,
        Some("side") | None => false,
        Some(other) => bail!("unknown `how` `{other}`; expected side or edit"),
    };
    let mut faults = Vec::new();
    let fault = |faults: &mut Vec<Value>, id: &str, detail: String| {
        faults.push(json!({ "fault": id, "detail": detail }));
    };
    let level = HubLevel::parse(level_name)?;
    if level == HubLevel::Register {
        fault(
            &mut faults,
            "level",
            "a draft is filed at line, topic, or field level; the register is the Workspace's one register".into(),
        );
    }
    // The Card text itself: a draft that is not a Card is judged on nothing else.
    let parsed = match crate::parse_card_text(text) {
        Ok(parsed) => Some(parsed),
        Err(error) => {
            fault(&mut faults, "text", error.to_string());
            None
        }
    };
    let destination = snapshot.by_address.get(at);
    let workspace = destination.map(|card| card.workspace);
    // The placement: the draft is filed in the agent Workspace, at a destination its level allows.
    match destination {
        None => fault(
            &mut faults,
            "destination",
            format!("`{at}` does not exist; choose where the hub is filed"),
        ),
        Some(card) if edit => {
            if card.card_type != CardType::Hub {
                fault(
                    &mut faults,
                    "destination",
                    format!("`{at}` is not a Hub Card; reuse rewrites the hub already filed there"),
                );
            }
            if card.workspace != Workspace::Agent {
                fault(
                    &mut faults,
                    "workspace",
                    format!(
                        "`{at}` is in the {} Workspace; a hub draft is filed in the agent Workspace",
                        card.workspace.id()
                    ),
                );
            }
            if card.hub_level != Some(level) {
                fault(
                    &mut faults,
                    "placement",
                    format!(
                        "`{at}` is a `{}` hub; reuse keeps its level",
                        card.hub_level.map_or("level-less", HubLevel::id)
                    ),
                );
            }
        }
        Some(card) => {
            if card.workspace != Workspace::Agent {
                fault(
                    &mut faults,
                    "workspace",
                    format!(
                        "`{at}` is in the {} Workspace; a hub draft is filed in the agent Workspace",
                        card.workspace.id()
                    ),
                );
            }
            if crate::is_tree_root(card) {
                fault(
                    &mut faults,
                    "placement",
                    format!("side successors cannot start from a tree root; choose a Card inside the tree, not `{at}`"),
                );
            }
            let tree = crate::tree_id(&card.address);
            let topic_tree = snapshot
                .by_address
                .get(&crate::tree_root_address(tree))
                .is_some_and(|root| root.is_topic);
            match level {
                HubLevel::Field => match crate::entry::register_addresses(snapshot, Workspace::Agent).as_slice() {
                    [] => fault(
                        &mut faults,
                        "placement",
                        "the agent Workspace has no register, so a field hub has no place to hang; file a topic or line hub, or publish the register first".into(),
                    ),
                    [register] if register != at => fault(
                        &mut faults,
                        "placement",
                        format!("a field hub is filed under the agent register (`{register}`), not under `{at}`"),
                    ),
                    [_] => {}
                    _ => fault(
                        &mut faults,
                        "placement",
                        "the agent Workspace has two registers; none is used, so a field hub has no place to hang".into(),
                    ),
                },
                HubLevel::Topic => {
                    if !topic_tree {
                        fault(
                            &mut faults,
                            "placement",
                            format!("a topic hub is filed inside a Topic tree; `{at}` is in the Literature tree `{tree}`"),
                        );
                    }
                    for other in topic_hubs_of(snapshot, tree) {
                        if other != at {
                            fault(
                                &mut faults,
                                "duplicate",
                                format!("[[{other}]] is another topic hub of tree {tree}; reuse it instead of writing a second one"),
                            );
                        }
                    }
                }
                HubLevel::Line => {
                    if !topic_tree {
                        fault(
                            &mut faults,
                            "placement",
                            format!("a line hub is filed where its line starts, inside a Topic tree; `{at}` is in the Literature tree `{tree}`"),
                        );
                    }
                }
                HubLevel::Register => {}
            }
        }
    }
    // The body rules a saved hub is judged by. A side successor is filed under `at`, so its
    // address is the one the write would give it; a reuse keeps the hub's own address.
    let mut lines = 0;
    let draft_address = if edit {
        at.to_string()
    } else {
        side_address(snapshot, at)
    };
    if let Some(parsed) = &parsed {
        let graph = Graph::new(snapshot);
        let body = parse_body(&parsed.body);
        lines = body.lines.len();
        let anchors = graph.anchors(&draft_address, Some(level));
        let lower = graph.lower_in(&body, &draft_address);
        faults.extend(graph.body_faults(
            &draft_address,
            Some(level),
            &parsed.title,
            &parsed.body,
            &body,
            &anchors,
            &lower,
            &Limits::default(),
        ));
    }
    let title = parsed
        .as_ref()
        .map(|parsed| parsed.title.clone())
        .unwrap_or_default();
    Ok(json!({
        "ok": faults.is_empty(),
        "level": if level == HubLevel::Register { Value::Null } else { json!(level.id()) },
        "title": title,
        "at": at,
        "how": if edit { "edit" } else { "side" },
        "address": draft_address,
        "anchor": crate::parent_location(&draft_address),
        "tree": crate::tree_id(at),
        "workspace": workspace.map(Workspace::id),
        "lines": lines,
        "faults": faults,
    }))
}

/// The address a draft would take as a side successor of `at`: the next free side label, as the
/// write allocates it (`crate::next_side_successor`).
fn side_address(snapshot: &Snapshot, at: &str) -> String {
    let mut highest = 0;
    for card in &snapshot.cards {
        if let Some(label) = crate::immediate_side_label(&card.address, at) {
            highest = highest.max(crate::side_label_to_number(&label));
        }
    }
    format!("{at}|{}", crate::number_to_side_label(highest + 1))
}

/// The topic hubs of one tree, in natural address order.
fn topic_hubs_of(snapshot: &Snapshot, tree: &str) -> Vec<String> {
    let mut hubs = snapshot
        .cards
        .iter()
        .filter(|card| {
            card.card_type == CardType::Hub
                && card.hub_level == Some(HubLevel::Topic)
                && crate::tree_id(&card.address) == tree
        })
        .map(|card| card.address.clone())
        .collect::<Vec<_>>();
    hubs.sort_by(|left, right| crate::model::address_order(left, right));
    hubs
}

/// The plan and the draft of the agent Hub Card that files a Spread (S17, #118; design §9.6,
/// D235-D237). It reads the supplied Cards, the `question` the Spread was kept from and the
/// `reasons` the answer gave each Card, and answers where the hub would be filed, at what level,
/// the draft the desk opens with, every legal destination, and the precheck of that draft. Nothing
/// is written, so a cancelled draft leaves no Card behind.
///
/// The default placement is the design's: a set spanning several Trees is a field hub under the
/// agent register; a set inside one agent Topic is a topic hub under that tree's first Card, or a
/// rewrite of the tree's topic hub when it already has one. Everything else asks for a
/// destination instead of filing into a tree the agent does not own.
pub(crate) fn draft(snapshot: &Snapshot, args: &Value) -> Result<Value> {
    let cards = args
        .get("cards")
        .and_then(Value::as_array)
        .context("`cards` must be the list of the Spread's addresses")?;
    let question = args
        .get("question")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|question| !question.is_empty())
        .unwrap_or_default();
    let reasons = args.get("reasons").and_then(Value::as_object);
    let mut members: Vec<&Card> = Vec::new();
    let mut seen = BTreeSet::new();
    for value in cards {
        let at = value
            .as_str()
            .context("every `cards` item must be a Card address")?
            .trim();
        let card = snapshot
            .by_address
            .get(at)
            .with_context(|| format!("card `{at}` does not exist"))?;
        if seen.insert(card.address.clone()) {
            members.push(card);
        }
    }
    if members.is_empty() {
        bail!("a hub draft needs at least one Card; the Spread is empty");
    }
    // The Trees the set spans, in natural address order: the hub's groups are these Trees.
    let mut trees: Vec<(String, Vec<&Card>)> = Vec::new();
    for member in &members {
        let tree = crate::tree_id(&member.address).to_string();
        match trees.iter_mut().find(|(seen, _)| *seen == tree) {
            Some((_, held)) => held.push(member),
            None => trees.push((tree, vec![member])),
        }
    }
    trees.sort_by(|(left, _), (right, _)| crate::model::address_order(left, right));
    let registers = crate::entry::register_addresses(snapshot, Workspace::Agent);
    // The default placement.
    let mut level = HubLevel::Field;
    let mut planned: Option<(String, bool)> = None;
    let mut missing: Option<&'static str> = None;
    let mut note = String::new();
    if let [(tree, _)] = trees.as_slice()
        && snapshot
            .by_address
            .get(&crate::tree_root_address(tree))
            .is_some_and(|root| root.is_topic && root.workspace == Workspace::Agent)
    {
        let topic = tree_title(snapshot, tree);
        let hubs = topic_hubs_of(snapshot, tree);
        if let Some(hub) = hubs.first() {
            level = HubLevel::Topic;
            planned = Some((hub.clone(), true));
            note = format!(
                "one agent Topic “{topic}”; its topic hub `{hub}` already exists, so the draft adds these Cards to it"
            );
        } else if let Some(first) = snapshot.by_address.get(&format!("{tree}/1")) {
            level = HubLevel::Topic;
            planned = Some((first.address.clone(), false));
            note = format!(
                "one agent Topic “{topic}”; a topic hub is filed under its first Card `{}`",
                first.address
            );
        }
    }
    if planned.is_none() {
        level = HubLevel::Field;
        match registers.as_slice() {
            [register] => {
                planned = Some((register.clone(), false));
                note = if trees.len() > 1 {
                    format!(
                        "the set spans {} Trees, so it is a field hub filed under the agent register `{register}`",
                        trees.len()
                    )
                } else {
                    format!(
                        "the set lies outside the agent Topics, so it is a field hub filed under the agent register `{register}`"
                    )
                };
            }
            [] => {
                missing = Some("register");
                note = "the agent Workspace has no register, so a field hub has nowhere to hang; choose a destination".into();
            }
            _ => {
                missing = Some("register");
                note = "the agent Workspace has two registers; none is used, so a field hub has nowhere to hang; choose a destination".into();
            }
        }
    }
    // Every legal destination, so the placement controls can offer them.
    let choices = destination_choices(snapshot, &registers, &trees, &members);
    // The draft's title: a rewrite keeps the hub's own title, a new one takes the Spread's name,
    // its question, or a plain name.
    let title = match &planned {
        Some((at, true)) => snapshot.title(at),
        _ => args
            .get("title")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or(if question.is_empty() { "Hub draft" } else { question })
            .to_string(),
    };
    let lines = members
        .iter()
        .map(|member| hub_line(member, reasons))
        .collect::<Vec<_>>();
    let scope = match question.is_empty() {
        true => format!(
            "范围：{} 张卡，来自 {} 棵树。",
            members.len(),
            trees.len()
        ),
        false => format!(
            "范围：{} 张卡，来自 {} 棵树，回答“{question}”。",
            members.len(),
            trees.len()
        ),
    };
    let body = match &planned {
        Some((at, true)) => {
            // A rewrite keeps the authored body byte for byte and adds one group of the new Cards.
            let held = snapshot
                .parsed
                .get(at)
                .map(|parsed| parsed.body.trim().to_string())
                .unwrap_or_default();
            let heading = if question.is_empty() {
                "新增成员".to_string()
            } else {
                question.chars().take(80).collect::<String>()
            };
            format!("{held}\n\n## {heading}\n{}", lines.join("\n"))
        }
        // A new draft groups its members by the Trees they lie in, which is how a field hub reads
        // and how a smaller hub starts: the reader sorts the groups and edits the reasons.
        _ => {
            let mut groups = Vec::new();
            for (tree, held) in &trees {
                let title = tree_title(snapshot, tree);
                let held = held
                    .iter()
                    .map(|member| hub_line(member, reasons))
                    .collect::<Vec<_>>()
                    .join("\n");
                groups.push(format!("## {title}\n{held}"));
            }
            format!("{scope}\n\n{}", groups.join("\n\n"))
        }
    };
    let text = crate::compose_card_text(&title, &body, "");
    let destination = match &planned {
        Some((at, edit)) => {
            let card = snapshot.by_address.get(at).expect("a planned destination");
            let mut value = json!({
                "at": at,
                "title": snapshot.title(at),
                "level": level.id(),
                "how": if *edit { "edit" } else { "side" },
                "reuse": *edit,
            });
            if *edit {
                // The rewrite is guarded by the hub's own text: a body that changed elsewhere is a
                // conflict to show, never to overwrite.
                value["base"] = json!(card.text);
            }
            Some(value)
        }
        None => None,
    };
    let check = precheck(
        snapshot,
        &json!({
            "text": text,
            "level": level.id(),
            "how": destination.as_ref().map_or("side", |destination| destination["how"].as_str().unwrap_or("side")),
            "at": destination.as_ref().map_or(Value::Null, |destination| destination["at"].clone()),
        }),
    )
    .unwrap_or_else(|error| json!({ "ok": false, "faults": [{ "fault": "destination", "detail": error.to_string() }] }));
    Ok(json!({
        "workspace": Workspace::Agent.id(),
        "question": question,
        "cards": members.iter().map(|member| json!({
            "at": member.address,
            "title": snapshot.title(&member.address),
            "type": member.card_type.id(),
            "tree": crate::tree_id(&member.address),
            "tree_title": tree_title(snapshot, crate::tree_id(&member.address)),
            "workspace": member.workspace.id(),
        })).collect::<Vec<_>>(),
        "trees": trees.iter().map(|(tree, held)| json!({
            "tree": tree,
            "title": tree_title(snapshot, tree),
            "kind": if snapshot.by_address.get(&crate::tree_root_address(tree)).is_some_and(|root| root.is_topic) { "topic" } else { "literature" },
            "workspace": snapshot.by_address.get(&crate::tree_root_address(tree)).map(|root| root.workspace.id()),
            "cards": held.iter().map(|member| member.address.clone()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "level": level.id(),
        "destination": destination,
        "choices": choices,
        "missing": missing,
        "note": note,
        "draft": { "title": title, "text": text },
        "check": check,
    }))
}

/// The title of one Tree: its root Card's title, or a plain fallback.
fn tree_title(snapshot: &Snapshot, tree: &str) -> String {
    let title = snapshot.title(&crate::tree_root_address(tree));
    if title.trim().is_empty() {
        format!("Tree {tree}")
    } else {
        title
    }
}

/// One member line of a hub body: the Card's Link and, when the answer justified it, its reason.
fn hub_line(member: &Card, reasons: Option<&serde_json::Map<String, Value>>) -> String {
    let reason = reasons
        .and_then(|reasons| reasons.get(&member.address))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default();
    match reason.is_empty() {
        true => format!("- [[{}]]", member.address),
        false => format!("- [[{}]]：{reason}", member.address),
    }
}

/// Every destination a Spread's hub may be filed at, in the order the placement controls offer
/// them: the agent register (a field hub), each agent Topic's topic hub or first Card (a topic
/// hub), and the Cards the Spread's own members hang under (a line hub).
fn destination_choices(
    snapshot: &Snapshot,
    registers: &[String],
    trees: &[(String, Vec<&Card>)],
    members: &[&Card],
) -> Vec<Value> {
    let mut choices: Vec<Value> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut push = |at: &str, level: HubLevel, edit: bool, note: &str| {
        if !seen.insert((at.to_string(), level.id(), edit)) {
            return;
        }
        choices.push(json!({
            "at": at,
            "title": snapshot.title(at),
            "level": level.id(),
            "how": if edit { "edit" } else { "side" },
            "reuse": edit,
            "note": note,
        }));
    };
    if let [register] = registers {
        push(register, HubLevel::Field, false, "filed under the agent register");
    }
    // The agent's Topic trees, natural order, bounded by the fixed 40 choices of one desk.
    let mut topics = snapshot
        .cards
        .iter()
        .filter(|card| card.is_topic && card.workspace == Workspace::Agent)
        .map(|card| crate::tree_id(&card.address).to_string())
        .collect::<Vec<_>>();
    topics.sort_by(|left, right| crate::model::address_order(left, right));
    for tree in topics {
        let hubs = topic_hubs_of(snapshot, &tree);
        for hub in &hubs {
            push(hub, HubLevel::Topic, true, "the Tree's topic hub, rewritten in place");
        }
        // A second topic hub of one tree is a `duplicate`: a Tree that has one offers only its
        // rewrite, never a fresh hub beside it.
        if hubs.is_empty()
            && let Some(first) = snapshot.by_address.get(&format!("{tree}/1"))
        {
            push(
                &first.address,
                HubLevel::Topic,
                false,
                "a topic hub filed under the Tree's first Card",
            );
        }
    }
    // A line hub is filed where its line starts: the Card one of the Spread's own members hangs
    // under, inside an agent Topic tree.
    for member in members {
        let Some(parent) = crate::parent_location(&member.address) else {
            continue;
        };
        let Some(card) = snapshot.by_address.get(&parent) else {
            continue;
        };
        let tree = crate::tree_id(&card.address);
        let topic = snapshot
            .by_address
            .get(&crate::tree_root_address(tree))
            .is_some_and(|root| root.is_topic && root.workspace == Workspace::Agent);
        if topic && !crate::is_tree_root(card) && card.card_type != CardType::Hub {
            push(
                &parent,
                HubLevel::Line,
                false,
                "a line hub filed where its line starts",
            );
        }
    }
    let _ = trees;
    choices
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
    /// Authored bodies an indexed read loaded beyond the Hub and Topic Cards it keeps, for the
    /// source evidence of `one-book` advice (§8.2, D166). A missing body leaves the evidence
    /// unknown, never guessed.
    bodies: BTreeMap<String, String>,
    /// True when the Snapshot came from a full read, so every authored body is present.
    full: bool,
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
            bodies: BTreeMap::new(),
            full: true,
        }
    }

    pub(crate) fn with_links(
        snapshot: &'a Snapshot,
        links: &'a BTreeMap<String, Vec<String>>,
    ) -> Self {
        let mut graph = Self::new(snapshot);
        graph.indexed_links = Some(links);
        graph.full = false;
        graph
    }

    /// Supplies the authored bodies an indexed read did not keep (the members of the hub the
    /// request reports, and the register's targets): the evidence `one-book` advice needs.
    pub(crate) fn with_bodies(mut self, bodies: BTreeMap<String, String>) -> Self {
        self.bodies = bodies;
        self
    }

    /// One Card's authored body: the supplied evidence body, else the Snapshot's own read when the
    /// read kept whole bodies. `None` means this read did not load it, so its source evidence is
    /// unknown rather than empty.
    fn authored(&self, address: &str) -> Option<&str> {
        if let Some(body) = self.bodies.get(address) {
            return Some(body.as_str());
        }
        let whole = self.full
            || self
                .snapshot
                .by_address
                .get(address)
                .is_some_and(|card| card.card_type == CardType::Hub || card.is_topic);
        if whole {
            return self
                .snapshot
                .parsed
                .get(address)
                .map(|parsed| parsed.body.as_str());
        }
        None
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
        self.body(address)
            .map(|body| self.lower_in(body, address))
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
        // A field hub stands for the Topic trees it crosses; the substantive Literature-tree Cards
        // it names explain the crossing but are members, not whole trees of its range (§8.1, D162).
        let field_trees = body
            .map(|body| {
                body.lines
                    .iter()
                    .map(|line| crate::tree_id(&line.target).to_string())
                    .filter(|tree| crate::valid_topic_id(tree))
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
            "one_book": advice.one_book,
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

    /// The Cards whose title a hub of this level may not copy: the Card it hangs under and, for a
    /// topic hub, its tree's Topic Card (D130, D169).
    fn anchors(&self, address: &str, level: Option<HubLevel>) -> Vec<String> {
        if !matches!(level, Some(HubLevel::Line) | Some(HubLevel::Topic)) {
            return Vec::new();
        }
        let mut anchors: Vec<String> = crate::parent_location(address).into_iter().collect();
        if level == Some(HubLevel::Topic) {
            let topic = crate::tree_root_address(crate::tree_id(address));
            if !anchors.contains(&topic) {
                anchors.push(topic);
            }
        }
        anchors
    }

    /// The hubs one body lists, in body order: every Link target that is a Hub Card other than the
    /// body's own Card.
    fn lower_in(&self, body: &Body, address: &str) -> Vec<String> {
        let mut seen = BTreeSet::new();
        body.lines
            .iter()
            .flat_map(|line| line.targets.iter())
            .filter(|target| self.hubs.contains_key(*target) && *target != address)
            .filter(|target| seen.insert((*target).clone()))
            .cloned()
            .collect()
    }

    /// The rules one hub body answers on its own: `level`, `anchor-title` (D130, D169),
    /// `oversize`/`flat` (H5), `bare` (H8), `broken` members and `upward` (H6). `Graph::check`
    /// judges a saved Hub Card by it and [`precheck`] a draft that is not saved yet, so the
    /// composer's draft runs the report's own rules rather than a second copy of them.
    ///
    /// `title` is the hub's own title, `raw` its authored body text, `anchors` the Cards whose
    /// titles it may not copy, and `lower` the hubs the body lists in body order.
    fn body_faults(
        &self,
        address: &str,
        level: Option<HubLevel>,
        title: &str,
        raw: &str,
        body: &Body,
        anchors: &[String],
        lower: &[String],
        limits: &Limits,
    ) -> Vec<Value> {
        let mut faults = Vec::new();
        let fault = |faults: &mut Vec<Value>, id: &str, detail: String| {
            faults.push(json!({ "fault": id, "detail": detail }));
        };
        if level.is_none() {
            fault(
                &mut faults,
                "level",
                "a Hub Card needs a hub level: line, topic, field, or register".into(),
            );
        }
        // `anchor-title` (D130, D169): a line or topic hub whose title copies the title of
        // the Card it hangs under tells the reader nothing about its navigation role. Case
        // and whitespace are normalized; broad semantic similarity is not a refusal. A topic
        // hub also names the tree's Topic Card: that is the Card the compiler's precheck
        // anchors a new draft to, so a draft it refuses is a fault this report names.
        if matches!(level, Some(HubLevel::Line) | Some(HubLevel::Topic)) {
            let own = normalized_title(title);
            let copied = if own.is_empty() {
                None
            } else {
                anchors
                    .iter()
                    .find(|anchor| normalized_title(&self.snapshot.title(anchor)) == own)
            };
            if let Some(anchor) = copied {
                fault(
                    &mut faults,
                    "anchor-title",
                    format!(
                        "the title repeats the title of `{anchor}`; name what the hub navigates, for example 消费需求的分解入口"
                    ),
                );
            }
        }
        // H5: size. The register's size is its number of terms; each term names 1 to 3 ways in,
        // and its rows are checked by their own grammar (D050, D289, D297).
        if level == Some(HubLevel::Register) {
            for row in register_rows(raw) {
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
        for lower in lower {
            let lower_level = self.level(lower).map_or(0, HubLevel::number);
            let nested = level == Some(HubLevel::Line)
                && self.level(lower) == Some(HubLevel::Line)
                && nested_line(address, lower);
            if level.is_some() && lower_level >= own && !nested {
                fault(
                    &mut faults,
                    "upward",
                    format!(
                        "lists [[{lower}]], a hub of level {} or higher",
                        self.level(lower).map_or("none", HubLevel::id)
                    ),
                );
            }
        }
        faults
    }

    fn check(&self, hub: &Card, limits: &Limits) -> (Vec<Value>, Advice) {
        let address = hub.address.as_str();
        let body = self.body(address).cloned().unwrap_or_default();
        let level = hub.hub_level;
        let raw = self
            .snapshot
            .parsed
            .get(address)
            .map(|parsed| parsed.body.as_str())
            .unwrap_or_default();
        let title = self.snapshot.title(address);
        let anchors = self.anchors(address, level);
        let lower = self.lower(address);
        let mut faults =
            self.body_faults(address, level, &title, raw, &body, &anchors, &lower, limits);
        let fault = |faults: &mut Vec<Value>, id: &str, detail: String| {
            faults.push(json!({ "fault": id, "detail": detail }));
        };
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
            one_book: self.one_book(hub, body, covered),
        }
    }

    /// `one-book` advice (§8.2, D166): a topic hub whose members carry one book's source
    /// evidence, while an accepted register term links a member to a treatment of the same
    /// subject with source evidence from another book that the hub lists nowhere. The advice
    /// names that treatment and the source it adds; a treatment whose evidence this read cannot
    /// see is reported as unknown, never asserted, and no candidate is invented when there is
    /// none. It is a report only: nothing writes a Link or a group.
    fn one_book(&self, hub: &Card, body: &Body, covered: &BTreeSet<String>) -> Value {
        if hub.hub_level != Some(HubLevel::Topic) {
            return Value::Null;
        }
        // An indexed read supplies the evidence bodies; a report without them says nothing
        // rather than guessing from bodies it never kept.
        if !self.full && self.bodies.is_empty() {
            return Value::Null;
        }
        // The hub's own evidence: what its members (outside 邻近) are sourced from.
        let mut sources: Vec<String> = Vec::new();
        for line in body
            .lines
            .iter()
            .filter(|line| !is_neighbour_group(line.group.as_deref()))
        {
            for source in self.source_evidence(&line.target).0 {
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
        }
        // An empty set is unknown, not "no book": claiming another book then would be a guess.
        let view = crate::entry::register_of(self.snapshot, hub.workspace);
        let register = view
            .as_ref()
            .map_or(Value::Null, |view| json!(view.register));
        let Some(view) = view else {
            return json!({"sources": sources, "candidates": [], "unknown": [], "register": register});
        };
        if sources.is_empty() {
            return json!({"sources": [], "candidates": [], "unknown": [], "register": register});
        }
        let members: BTreeSet<String> = body
            .lines
            .iter()
            .filter(|line| !is_neighbour_group(line.group.as_deref()))
            .map(|line| line.target.clone())
            .collect();
        let mut candidates: Vec<Value> = Vec::new();
        let mut unknown: Vec<Value> = Vec::new();
        for entry in &view.entries {
            if !entry.faults.is_empty() {
                continue;
            }
            for target in &entry.targets {
                if !target.exists || covered.contains(&target.address) || members.contains(&target.address) {
                    continue;
                }
                // A treatment is only relevant when an accepted term already links it to a member.
                let Some(member) = entry
                    .targets
                    .iter()
                    .find(|other| members.contains(&other.address))
                    .map(|other| other.address.clone())
                else {
                    continue;
                };
                let (evidence, _) = self.source_evidence(&target.address);
                let added = evidence
                    .iter()
                    .find(|source| !sources.contains(source))
                    .cloned();
                match added {
                    Some(source) => candidates.push(json!({
                        "address": target.address,
                        "title": target.title,
                        "term": entry.term,
                        "member": member,
                        "source": source,
                        "sources": evidence,
                    })),
                    None if evidence.is_empty() => unknown.push(json!({
                        "address": target.address,
                        "title": target.title,
                        "term": entry.term,
                        "member": member,
                    })),
                    // The same book, no different source evidence: no advice for it.
                    None => {}
                }
            }
        }
        // One row per Card: the first accepted term that names it, in register order.
        let mut seen = BTreeSet::new();
        candidates.retain(|row| seen.insert(row["address"].as_str().unwrap_or_default().to_string()));
        unknown.retain(|row| seen.insert(row["address"].as_str().unwrap_or_default().to_string()));
        candidates.truncate(10);
        unknown.truncate(10);
        json!({
            "sources": sources,
            "candidates": candidates,
            "unknown": unknown,
            "register": view.register,
        })
    }

    /// The source evidence of one Card, from the bodies this read holds (see the free
    /// [`source_evidence`]: the indexed hub read loads only the bodies its report needs).
    fn source_evidence(&self, address: &str) -> (Vec<String>, Option<String>) {
        let body = |at: &str| self.authored(at).map(str::to_string);
        source_evidence_with(self.snapshot, address, &body)
    }

    /// Every Topic body's `## 入口` section (§8.2, D167, D168): the one documented exception to
    /// the size-based topic-hub requirement. A tree whose section names one to three existing
    /// Cards of the tree, each saying what it adds, needs no topic hub; a missing, oversized,
    /// outside, repeated, broken, or unexplained entry is reported so the requirement returns.
    fn entry_sections(&self) -> Vec<EntrySection> {
        let mut sections = Vec::new();
        for card in self.snapshot.cards.iter().filter(|card| card.is_topic) {
            let tree = crate::tree_id(&card.address).to_string();
            let body = self
                .snapshot
                .parsed
                .get(&card.address)
                .map(|parsed| parsed.body.clone())
                .unwrap_or_default();
            let parsed = parse_body(&body);
            if !parsed.groups.iter().any(|group| is_entry_group(Some(group.as_str()))) {
                continue;
            }
            let lines = parsed
                .lines
                .iter()
                .filter(|line| is_entry_group(line.group.as_deref()))
                .collect::<Vec<_>>();
            let mut entries: Vec<String> = Vec::new();
            let mut fault = None;
            let mut detail = String::new();
            if lines.is_empty() || lines.len() > ENTRY_MAX {
                fault = Some("entry-shape");
                detail = if lines.is_empty() {
                    format!("the {ENTRY_GROUP} group names no Card; write one to three list items")
                } else {
                    format!("the {ENTRY_GROUP} group names {} Cards; write one to three", lines.len())
                };
            }
            for line in &lines {
                if !entries.contains(&line.target) {
                    entries.push(line.target.clone());
                }
            }
            if fault.is_none() {
                fault = Some("entry-target");
                for line in &lines {
                    let target = line.target.as_str();
                    let why = if !self.snapshot.by_address.contains_key(target) {
                        format!("[[{target}]] does not exist")
                    } else if !crate::is_tree_member(target, &tree) || target == crate::tree_root_address(&tree) {
                        format!("[[{target}]] is not a Card of tree {tree}")
                    } else if self
                        .snapshot
                        .by_address
                        .get(target)
                        .is_some_and(|card| card.card_type == CardType::Hub)
                    {
                        format!("[[{target}]] is a Hub Card; name the Card a reader enters, not a hub")
                    } else if lines.iter().filter(|other| other.target == target).count() > 1 {
                        format!("[[{target}]] is named twice")
                    } else {
                        continue;
                    };
                    detail = why;
                    break;
                }
                if detail.is_empty() {
                    fault = None;
                }
            }
            if fault.is_none()
                && let Some(line) = lines.iter().find(|line| reason_chars(&line.reason) == 0)
            {
                fault = Some("entry-bare");
                detail = format!("[[{}]] has no explanation of what it adds", line.target);
            }
            sections.push(EntrySection {
                tree,
                card: card.address.clone(),
                entries,
                fault,
                detail,
            });
        }
        sections
    }

    /// Every hub of a Workspace with its level, upper and lower hubs, and
    /// faults, the hubs the policy requires but the Workspace lacks, and the
    /// Topic-body entry sections that satisfy the topic-hub requirement.
    pub(crate) fn list(&self, workspace: Option<Workspace>, limits: &Limits) -> Value {
        let in_workspace =
            |card: &Card| workspace.is_none_or(|workspace| card.workspace == workspace);
        let sections = self.entry_sections();
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
        let entries: Vec<Value> = sections
            .iter()
            .filter(|section| {
                self.snapshot
                    .by_address
                    .get(&section.card)
                    .is_some_and(|card| in_workspace(card))
            })
            .map(|section| {
                json!({
                    "tree": section.tree,
                    "card": section.card,
                    "entries": section.entries,
                    "valid": section.fault.is_none(),
                    "fault": section.fault,
                    "detail": section.detail,
                })
            })
            .collect();
        json!({
            "workspace": workspace.map(Workspace::id),
            "limits": limits.json(),
            "counts": counts,
            "hubs": hubs,
            "entries": entries,
            "missing": self.missing(workspace, limits, &sections),
        })
    }

    /// H1, H2, and H4: the required hubs that do not exist yet.
    fn missing(&self, workspace: Option<Workspace>, limits: &Limits, sections: &[EntrySection]) -> Vec<Value> {
        // A register's declared parts are navigation Cards of the 索引 tree, not content of it
        // (§8.5, D188): they neither make the tree require a topic hub of its own nor form a branch.
        let register_parts: BTreeSet<String> = self
            .hubs
            .values()
            .flat_map(|(_, body)| body.lines.iter())
            .filter(|line| line.group.as_deref() == Some(REGISTER_PART_GROUP))
            .flat_map(|line| line.targets.iter().cloned())
            .collect();
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
                .filter(|card| card.card_type != CardType::Hub && !register_parts.contains(&card.address))
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
                // H1's one documented exception (§8.2, D168): an accepted, valid entry section in
                // the Topic body is a way in without a redundant topic hub. A broken or removed
                // section is no exception, and its fault stays in the row so nothing is skipped
                // silently.
                let section = sections
                    .iter()
                    .find(|section| section.tree == tree && section.card == root.address);
                if section.is_none_or(|section| section.fault.is_some()) {
                    let mut row = json!({
                        "level": "topic",
                        "tree": tree,
                        "anchor": format!("{tree}/1"),
                        "title": self.snapshot.title(&root.address),
                        "cards": content,
                    });
                    if let Some(section) = section {
                        row["entries"] = json!({
                            "valid": false,
                            "fault": section.fault,
                            "detail": section.detail,
                        });
                    }
                    missing.push(row);
                }
            }
            // A branch: a Side successor that is not part of a model.
            for branch in members.iter().filter(|card| {
                card.card_type == CardType::Idea
                    && !register_parts.contains(&card.address)
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
    /// `one-book` advice of a topic hub, or null for a hub that has no subject tree.
    one_book: Value,
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

/// The group heading of a Topic body's entry section: one to three Links a reader enters the
/// subject through, in place of a topic hub whose only useful lines would repeat the tree map
/// (§8.2, D167, D168). `entries` and `entrance` are the English spellings, like `邻近`.
pub(crate) const ENTRY_GROUP: &str = "入口";

/// The most Cards one Topic-body entry section may name (§8.2: "one to three entry Links").
pub(crate) const ENTRY_MAX: usize = 3;

/// True for the group that names the Topic body's entry Cards (`入口`, `entries`, `entrance`).
fn is_entry_group(group: Option<&str>) -> bool {
    group.is_some_and(|group| {
        let group = group.trim();
        group.starts_with(ENTRY_GROUP) || group.eq_ignore_ascii_case("entries") || group.eq_ignore_ascii_case("entrance")
    })
}

/// True for a body line that labels its Links as the Card's source (`来源：[[book]] §3`): the one
/// authored place source evidence is written, beside the Literature tree a Card is filed in.
pub(crate) fn is_source_line(line: &str) -> bool {
    let Some((label, _)) = line.split_once(['：', ':']) else {
        return false;
    };
    let label = label.trim();
    label.eq_ignore_ascii_case("source") || label.eq_ignore_ascii_case("sources") || label == "来源" || label == "出处"
}

/// One Topic body's entry section: the Cards a reader enters the subject through, and why the
/// section does or does not satisfy the topic-hub requirement (§8.2, D167, D168).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EntrySection {
    /// The tree the section belongs to.
    pub(crate) tree: String,
    /// The Topic Card the section is written on.
    pub(crate) card: String,
    /// The entry Cards, in body order, without repeats.
    pub(crate) entries: Vec<String>,
    /// `None` when the section is accepted; else the fault that keeps the requirement standing.
    pub(crate) fault: Option<&'static str>,
    pub(crate) detail: String,
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

    #[test]
    fn declared_parts_are_read_with_their_accepted_labels_in_both_row_forms() {
        let body = "范围：代理工作区的入口。\n\n## 词条\n- 学习：[[5/1]]\n\n## 词条分册\n- [[196/1|a]] A–F\n- [[196/1|b]] G–L\n- S–Z、其他：[[196/1|c]]\n\n## 领域\n- 信息经济学：[[196/1|d]]\n";
        let rows = register_part_rows(body);
        assert_eq!(rows.len(), 3, "only the 词条分册 rows declare parts: {rows:?}");
        assert_eq!(rows[0].label, "A–F");
        assert_eq!(rows[0].address.as_deref(), Some("196/1|a"));
        assert_eq!(rows[0].line, 7, "the row names the line it was written on");
        assert_eq!(rows[1].address.as_deref(), Some("196/1|b"));
        assert_eq!(
            rows[2].label, "S–Z、其他",
            "a label before the Link keeps its own words and its accepted range"
        );
        assert_eq!(rows[2].address.as_deref(), Some("196/1|c"));
        // The same declarations read as the target list the read adapter loads bodies by.
        assert_eq!(
            register_part_targets(body),
            vec!["196/1|a", "196/1|b", "196/1|c"]
        );
        // A row with no label is still a declaration: the label is reported empty, not invented.
        let bare = register_part_rows("## 词条分册\n- [[196/1|a]]\n");
        assert_eq!(bare[0].label, "");
        assert_eq!(bare[0].address.as_deref(), Some("196/1|a"));
        // A fenced row is not a declaration.
        let fenced = register_part_rows("## 词条分册\n```\n- [[196/1|a]] A–F\n```\n");
        assert!(fenced.is_empty(), "{fenced:?}");
    }

    /// A scratch archive for the draft precheck: the agent register (a Hub Card of level
    /// `register`, the first Card of the Topic `索引`), one agent Topic with two Cards, and one
    /// human Topic with a Card — the tree the cabinet usually shows.
    struct DraftArchive {
        _temp: tempfile::TempDir,
        snapshot: Snapshot,
        register: String,

        topic_card: String,
        human_card: String,
    }

    impl DraftArchive {
        fn new() -> Self {
            use crate::Access;
            use crate::ops::{self, Successor, TypeChange};
            let temp = tempfile::tempdir().expect("temp dir");
            let root = temp.path();
            crate::initialize_database(root).expect("initialize database");
            // Two books, so a draft can name Cards of two trees.
            for (key, title) in [("bookA", "Book A"), ("bookB", "Book B")] {
                crate::ops::create_literature(
                    root,
                    Access::Owner,
                    Workspace::Agent,
                    &format!("@book{{{key}, title={{{title}}}}}"),
                    &format!("# {title}\n\n该书。\n"),
                )
                .expect(title);
            }
            let index = ops::create_topic(root, Access::Owner, Workspace::Agent, "# 索引\n\n入口。\n")
                .expect("agent Topic");
            let register = ops::create_successor(
                root,
                Access::Owner,
                Successor::Direct,
                &index,
                CardType::Hub,
                None,
                "# 索引：代理工作区的入口\n\n## 词条\n",
            )
            .expect("register");
            ops::edit(
                root,
                Access::Owner,
                &register,
                None,
                TypeChange {
                    hub_level: Some(Some(HubLevel::Register)),
                    ..TypeChange::default()
                },
                None,
            )
            .expect("register level");
            let topic = ops::create_topic(root, Access::Owner, Workspace::Agent, "# 学习\n\n学。\n")
                .expect("agent Topic");
            let first = ops::create_successor(
                root,
                Access::Owner,
                Successor::Direct,
                &topic,
                CardType::Idea,
                None,
                "# 学习的第一张卡\n\n文本。\n",
            )
            .expect("topic Card");
            for (title, body) in [
                ("学习的个体行为", "学习改变信念。\n\n来源：[[bookA]]"),
                ("学习的产业生产率", "学习提高产出。\n\n来源：[[bookB]]"),
            ] {
                ops::create_successor(
                    root,
                    Access::Owner,
                    Successor::Side,
                    &first,
                    CardType::Idea,
                    None,
                    &format!("# {title}\n\n{body}\n"),
                )
                .expect("topic Card");
            }
            // The register accepts the term its entry Card states.
            ops::edit(
                root,
                Access::Owner,
                &register,
                Some(&format!(
                    "# 索引：代理工作区的入口\n\n## 词条\n- 学习：[[{first}]]\n"
                )),
                TypeChange::default(),
                None,
            )
            .expect("register body");
            let human = ops::create_topic(root, Access::Owner, Workspace::Human, "# 人的主题\n\n人。\n")
                .expect("human Topic");
            let human_card = ops::create_successor(
                root,
                Access::Owner,
                Successor::Direct,
                &human,
                CardType::Idea,
                None,
                "# 人的卡\n\n文本。\n",
            )
            .expect("human Card");
            let snapshot = Snapshot::read(root).expect("snapshot");
            Self {
                _temp: temp,
                snapshot,
                register,

                topic_card: first,
                human_card,
            }
        }

        fn draft(&self, at: &str, level: &str, body: &str) -> Value {
            precheck(
                &self.snapshot,
                &json!({
                    "text": format!("# 跨书的多重均衡\n\n{body}\n<!-- zt:reverse-links -->\n"),
                    "level": level,
                    "how": "side",
                    "at": at,
                }),
            )
            .expect("precheck")
        }

        fn faults(&self, at: &str, level: &str, body: &str) -> Vec<String> {
            self.draft(at, level, body)["faults"]
                .as_array()
                .expect("faults")
                .iter()
                .map(|fault| fault["fault"].as_str().expect("fault id").to_string())
                .collect()
        }

        /// The title of the Card the single-Topic drafts hang under.
        fn topic_card_title(&self) -> String {
            self.snapshot.title(&self.topic_card)
        }

        /// The tree's topic hub: a Hub Card of level `topic`, a Side successor of its first Card —
        /// the placement the digest's hub stage uses.
        fn topic_hub(&self) -> String {
            use crate::Access;
            use crate::ops::{self, Successor, TypeChange};
            let root = self._temp.path();
            let hub = ops::create_successor(
                root,
                Access::Owner,
                Successor::Side,
                &self.topic_card,
                CardType::Hub,
                None,
                "# 学习的入口\n\n## 个体学习\n- [[bookA]]：学习会改变个体的信念。\n",
            )
            .expect("topic hub");
            ops::edit(
                root,
                Access::Owner,
                &hub,
                None,
                TypeChange {
                    hub_level: Some(Some(HubLevel::Topic)),
                    ..TypeChange::default()
                },
                None,
            )
            .expect("topic hub level");
            hub
        }
    }

    /// S17 (#118, D235-D237, S17-AC02, S17-AC03): a field-hub draft is filed under the agent
    /// Workspace's register; the same draft offered at a Topic Card or in the human tree is
    /// refused, so a Spread is never saved into the tree the cabinet happens to show.
    #[test]
    fn a_field_hub_draft_is_filed_under_the_agent_register() {
        let archive = DraftArchive::new();
        let body = "## Book A\n- [[bookA]]：多重均衡来自策略互补。\n\n## Book B\n- [[bookB]]：策略互补解释多重均衡。";
        let filed = archive.draft(&archive.register, "field", body);
        assert_eq!(filed["ok"], json!(true), "{filed}");
        assert_eq!(filed["faults"], json!([]), "{filed}");
        assert_eq!(filed["level"], json!("field"));
        assert_eq!(filed["workspace"], json!("agent"));
        assert_eq!(filed["at"], json!(archive.register));
        assert_eq!(filed["anchor"], json!(archive.register));
        assert_eq!(filed["title"], json!("跨书的多重均衡"));
        assert_eq!(filed["lines"], json!(2), "the precheck counts the members it read: {filed}");
        // A Topic Card is no place for a field hub.
        let wrong = archive.draft(&archive.topic_card, "field", body);
        assert_eq!(wrong["ok"], json!(false), "{wrong}");
        assert!(
            wrong["faults"]
                .as_array()
                .expect("faults")
                .iter()
                .any(|fault| fault["fault"] == json!("placement")),
            "{wrong}"
        );
        // The human tree is never a destination for an agent draft.
        let human = archive.draft(&archive.human_card, "field", body);
        assert_eq!(human["ok"], json!(false), "{human}");
        assert_eq!(human["workspace"], json!("human"));
        assert!(
            human["faults"]
                .as_array()
                .expect("faults")
                .iter()
                .any(|fault| fault["fault"] == json!("workspace")),
            "{human}"
        );
    }

    /// S17 (#118, S17-AC02, S17-AC04): a topic-hub draft is filed under the tree's first Card; a
    /// tree that already has a topic hub is a `duplicate` (reuse it), a title that copies the
    /// anchor's or the Topic's title is `anchor-title`, and reuse rewrites the hub already filed.
    #[test]
    fn a_topic_hub_draft_is_filed_under_the_trees_first_card_and_reuses_an_existing_hub() {
        let archive = DraftArchive::new();
        let body = "## 个体学习\n- [[bookA]]：学习会改变个体的信念。\n\n## 产业学习\n- [[bookB]]：学习会提高产业的产出。";
        let filed = archive.draft(&archive.topic_card, "topic", body);
        assert_eq!(filed["ok"], json!(true), "{filed}");
        assert_eq!(filed["level"], json!("topic"));
        assert_eq!(filed["anchor"], json!(archive.topic_card));
        let address = filed["address"].as_str().expect("address");
        assert!(
            address.starts_with(&format!("{}|", archive.topic_card)),
            "the draft is placed where the write would put it: {filed}"
        );
        assert!(
            !archive.snapshot.by_address.contains_key(address),
            "the draft's place is free: {filed}"
        );
        assert_eq!(filed["tree"], json!(crate::tree_id(&archive.topic_card)));
        // A title that copies the Card it hangs under tells the reader nothing.
        let copied = precheck(
            &archive.snapshot,
            &json!({
                "text": format!("# {}\n\n{body}", archive.topic_card_title()),
                "level": "topic",
                "at": archive.topic_card,
            }),
        )
        .expect("precheck");
        assert_eq!(copied["ok"], json!(false), "{copied}");
        assert!(
            copied["faults"]
                .as_array()
                .expect("faults")
                .iter()
                .any(|fault| fault["fault"] == json!("anchor-title")),
            "{copied}"
        );
        // Reuse rewrites the hub already filed there, never a Card that is not a hub.
        let not_a_hub = precheck(
            &archive.snapshot,
            &json!({ "text": format!("# 学习的两条线\n\n{body}"), "level": "topic", "how": "edit", "at": archive.topic_card }),
        )
        .expect("precheck");
        assert_eq!(not_a_hub["how"], json!("edit"));
        assert!(
            not_a_hub["faults"]
                .as_array()
                .expect("faults")
                .iter()
                .any(|fault| fault["fault"] == json!("destination")),
            "a Card that is not a hub has nothing to reuse: {not_a_hub}"
        );
        let hub = archive.topic_hub();
        let snapshot = Snapshot::read(archive._temp.path()).expect("snapshot");
        let second = precheck(
            &snapshot,
            &json!({ "text": format!("# 另一条入口\n\n{body}"), "level": "topic", "at": archive.topic_card }),
        )
        .expect("precheck");
        assert!(
            second["faults"]
                .as_array()
                .expect("faults")
                .iter()
                .any(|fault| fault["fault"] == json!("duplicate")),
            "a second topic hub of one tree is named: {second}"
        );
        let reuse = precheck(
            &snapshot,
            &json!({
                "text": "# 学习的入口\n\n## 个体学习\n- [[bookA]]：学习会改变个体的信念。\n\n## 产业学习\n- [[bookB]]：学习会提高产业的产出。",
                "level": "topic",
                "how": "edit",
                "at": hub,
            }),
        )
        .expect("precheck");
        assert_eq!(reuse["ok"], json!(true), "the existing hub is rewritten in place: {reuse}");
        assert_eq!(reuse["address"], json!(hub));
        let wrong_level = precheck(
            &snapshot,
            &json!({ "text": "# 学习的入口\n\n## 个体学习\n- [[bookA]]：学习会改变个体的信念。", "level": "line", "how": "edit", "at": hub }),
        )
        .expect("precheck");
        assert!(
            wrong_level["faults"]
                .as_array()
                .expect("faults")
                .iter()
                .any(|fault| fault["fault"] == json!("placement")),
            "reuse keeps the hub's level: {wrong_level}"
        );
        // The hub exists and its body is unchanged by any precheck.
        assert_eq!(
            snapshot.parsed[&hub].body,
            "## 个体学习\n- [[bookA]]：学习会改变个体的信念。",
            "a precheck writes nothing"
        );
    }

    /// S17 (#118, S17-AC03, S17-AC04): the draft's level is required and legal, and the body is
    /// judged by the report's own rules: a missing reason, a missing member, a hub of the same or a
    /// higher level listed, and the size rules.
    #[test]
    fn a_hub_draft_runs_the_report_rules_before_it_is_saved() {
        let archive = DraftArchive::new();
        // No level, an unknown level, a missing destination.
        assert!(precheck(&archive.snapshot, &json!({ "text": "# 草稿\n", "at": archive.register })).is_err());
        assert!(
            precheck(&archive.snapshot, &json!({ "text": "# 草稿\n", "level": "concept", "at": archive.register }))
                .expect_err("an unknown level")
                .to_string()
                .contains("unknown hub level")
        );
        assert!(
            precheck(&archive.snapshot, &json!({ "text": "# 草稿\n", "level": "field" }))
                .expect_err("no destination")
                .to_string()
                .contains("`at`")
        );
        // The register is the Workspace's one register, never a draft's level.
        let register = precheck(
            &archive.snapshot,
            &json!({ "text": "# 草稿\n", "level": "register", "at": archive.register }),
        )
        .expect("precheck");
        assert_eq!(register["ok"], json!(false));
        assert_eq!(register["level"], Value::Null, "{register}");
        // The body's own rules: a reason too short, a member that does not exist.
        let faults = archive.faults(
            &archive.register,
            "field",
            "## Book A\n- [[bookA]]：短\n\n## Book B\n- [[noSuchCard]]：策略互补解释多重均衡。",
        );
        assert!(faults.contains(&"bare".to_string()), "{faults:?}");
        assert!(faults.contains(&"broken".to_string()), "{faults:?}");
        // A draft that lists the register itself lists a hub of a higher level.
        let upward = archive.faults(
            &archive.register,
            "field",
            &format!("## Book A\n- [[bookA]]：多重均衡来自策略互补。\n\n## Book B\n- [[{}]]：另一条入口。", archive.register),
        );
        assert!(upward.contains(&"upward".to_string()), "{upward:?}");
        // Nine lines in one group are `flat`: the draft must be sorted into named groups.
        let lines = (0..9)
            .map(|index| format!("- [[bookA]]：第 {index} 条理由写在这里。"))
            .collect::<Vec<_>>()
            .join("\n");
        let flat = archive.faults(&archive.register, "field", &format!("## Book A\n{lines}"));
        assert!(flat.contains(&"flat".to_string()), "{flat:?}");
    }

    /// S17 (#118, D235-D237, S17-AC01, S17-AC02): a Spread that spans Trees becomes a field-hub
    /// draft under the agent register — its question, its selected Links and their reasons — and
    /// the draft passes the precheck.
    #[test]
    fn a_cross_tree_spread_becomes_a_field_hub_draft_under_the_agent_register() {
        let archive = DraftArchive::new();
        let plan = draft(
            &archive.snapshot,
            &json!({
                "cards": ["1/1|a", "bookA", "bookB"],
                "question": "为什么学习可能造成多重均衡？",
                "reasons": {
                    "1/1|a": "States the accepted term 学习（个体行为）.",
                    "bookA": "Reached in 1 semantic step(s) from the entry Card.",
                    "bookB": "Adds the other book's treatment of the same term.",
                },
            }),
        )
        .expect("draft");
        assert_eq!(plan["level"], json!("field"), "{plan}");
        assert_eq!(plan["workspace"], json!("agent"));
        assert_eq!(plan["destination"]["at"], json!(archive.register), "{plan}");
        assert_eq!(plan["destination"]["how"], json!("side"));
        assert_eq!(plan["destination"]["reuse"], json!(false));
        assert_eq!(plan["missing"], Value::Null);
        assert_eq!(plan["question"], json!("为什么学习可能造成多重均衡？"));
        assert_eq!(
            plan["trees"].as_array().expect("trees").len(),
            3,
            "each Tree the set spans is named: {plan}"
        );
        let text = plan["draft"]["text"].as_str().expect("draft text");
        assert_eq!(plan["draft"]["title"], json!("为什么学习可能造成多重均衡？"));
        assert!(
            text.starts_with("# 为什么学习可能造成多重均衡？\n"),
            "the desk opens with the question: {text}"
        );
        assert!(
            text.contains("范围：") && text.contains("为什么学习可能造成多重均衡？"),
            "the drafted body states its scope and carries the whole question: {text}"
        );
        for (group, member, reason) in [
            ("学习", "[[1/1|a]]", "States the accepted term"),
            ("Book A", "[[bookA]]", "Reached in 1 semantic step(s)"),
            ("Book B", "[[bookB]]", "Adds the other book's treatment"),
        ] {
            assert!(text.contains(&format!("## {group}")), "{group}: {text}");
            assert!(
                text.lines()
                    .any(|line| line.contains(member) && line.contains(reason)),
                "the drafted line names {member} with its reason: {text}"
            );
        }
        assert!(
            text.trim_end().ends_with("<!-- zt:reverse-links -->"),
            "{text}"
        );
        assert_eq!(plan["check"]["ok"], json!(true), "{plan}");
        assert_eq!(plan["check"]["faults"], json!([]), "{plan}");
    }

    /// S17 (#118, S17-AC02): a Spread inside one agent Topic is a topic-hub draft under that
    /// Tree's first Card; when the Topic already has a topic hub the draft rewrites it, keeping
    /// the body it had, instead of writing a second one.
    #[test]
    fn a_single_topic_spread_becomes_a_topic_hub_draft_or_rewrites_the_tree_hub() {
        let archive = DraftArchive::new();
        let question = "为什么学习可能造成多重均衡？";
        let plan = draft(
            &archive.snapshot,
            &json!({
                "cards": [archive.topic_card, "1/1|a"],
                "question": question,
                "reasons": { "1/1": "The first Card of the Topic states the subject.", "1/1|a": "States the accepted term 学习（个体行为）." },
            }),
        )
        .expect("draft");
        assert_eq!(plan["level"], json!("topic"), "{plan}");
        assert_eq!(plan["destination"]["at"], json!(archive.topic_card));
        assert_eq!(plan["destination"]["how"], json!("side"));
        assert_eq!(plan["destination"]["reuse"], json!(false));
        assert_eq!(plan["trees"].as_array().expect("trees").len(), 1);
        let text = plan["draft"]["text"].as_str().expect("text");
        assert!(text.contains("## 学习\n"), "one group, named by the Tree: {text}");
        assert!(text.contains("- [[1/1|a]]：States the accepted term"), "{text}");
        assert!(text.contains("- [[1/1]]：The first Card of the Topic"), "{text}");
        assert_eq!(plan["check"]["ok"], json!(true), "{plan}");

        // The Tree already has a topic hub: the draft adds this question's Cards to it.
        let hub = archive.topic_hub();
        let snapshot = Snapshot::read(archive._temp.path()).expect("snapshot");
        let reuse = draft(
            &snapshot,
            &json!({
                "cards": [archive.topic_card, "1/1|a"],
                "question": question,
                "reasons": { "1/1": "The first Card of the Topic states the subject.", "1/1|a": "States the accepted term 学习（个体行为）." },
            }),
        )
        .expect("draft");
        assert_eq!(reuse["level"], json!("topic"), "{reuse}");
        assert_eq!(reuse["destination"]["at"], json!(hub), "{reuse}");
        assert_eq!(reuse["destination"]["how"], json!("edit"));
        assert_eq!(reuse["destination"]["reuse"], json!(true));
        assert!(
            reuse["destination"]["base"].as_str().is_some_and(|base| !base.is_empty()),
            "a rewrite is guarded by the hub's own text: {reuse}"
        );
        let merged = reuse["draft"]["text"].as_str().expect("text");
        assert!(
            merged.starts_with("# 学习的入口\n\n## 个体学习\n- [[bookA]]：学习会改变个体的信念。\n"),
            "the hub's authored body survives byte for byte: {merged}"
        );
        assert!(
            merged.contains(&format!("## {question}\n")),
            "the addition is one named group: {merged}"
        );
        assert!(
            merged.contains("- [[1/1|a]]：States the accepted term 学习（个体行为）."),
            "the new members carry their reasons: {merged}"
        );
        assert!(merged.trim_end().ends_with("<!-- zt:reverse-links -->"), "{merged}");
        assert_eq!(reuse["check"]["ok"], json!(true), "{reuse}");
        // A Tree that has a topic hub offers its rewrite, never a second hub beside it.
        let choices = reuse["choices"].as_array().expect("choices");
        assert!(
            choices.iter().any(|choice| choice["at"] == json!(hub) && choice["how"] == json!("edit")),
            "{reuse}"
        );
        assert!(
            !choices.iter().any(|choice| {
                choice["at"] == json!(archive.topic_card) && choice["level"] == json!("topic")
            }),
            "a duplicate topic hub is never offered: {reuse}"
        );
    }

    /// S17 (#118, S17-AC03): a Spread the agent cannot file inside its own tree asks for a
    /// destination instead — a cross-Tree set with no register, and a set that lies in the human
    /// Workspace.
    #[test]
    fn a_spread_without_an_agent_home_asks_for_a_destination() {
        use crate::Access;
        use crate::ops;
        // A fresh agent Workspace: a Topic with Cards, but no register yet.
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path();
        crate::initialize_database(root).expect("initialize database");
        crate::ops::create_literature(
            root,
            Access::Owner,
            Workspace::Agent,
            "@book{bookA, title={Book A}}",
            "# Book A\n\n该书。\n",
        )
        .expect("Book A");
        let topic = ops::create_topic(root, Access::Owner, Workspace::Agent, "# 学习\n\n学。\n")
            .expect("Topic");
        let card = ops::create_successor(
            root,
            Access::Owner,
            ops::Successor::Direct,
            &topic,
            CardType::Idea,
            None,
            "# 学习的个体行为\n\n学习会改变个体的信念。\n",
        )
        .expect("Card");
        let snapshot = Snapshot::read(root).expect("snapshot");
        let plan = draft(
            &snapshot,
            &json!({
                "cards": [card, "bookA"],
                "question": "学习怎样影响产出？",
                "reasons": { "bookA": "Adds the book's treatment of learning." },
            }),
        )
        .expect("draft");
        assert_eq!(plan["missing"], json!("register"), "{plan}");
        assert_eq!(plan["destination"], Value::Null, "{plan}");
        assert!(
            plan["note"].as_str().is_some_and(|note| note.contains("no register")),
            "the desk asks for a destination: {plan}"
        );
        let choices = plan["choices"].as_array().expect("choices");
        assert!(
            choices.iter().any(|choice| choice["at"] == json!(format!("{}/1", crate::tree_id(&topic)))
                && choice["level"] == json!("topic")),
            "an agent Topic is still a legal destination: {plan}"
        );
        assert!(
            choices.iter().all(|choice| choice["level"] != json!("field")),
            "no register means no field destination: {plan}"
        );
        // The draft is still written; only its placement is missing.
        assert!(
            plan["draft"]["text"]
                .as_str()
                .is_some_and(|text| text.contains("## Book A") && text.contains("## 学习")),
            "{plan}"
        );
        assert_eq!(plan["check"]["ok"], json!(false), "{plan}");
    }
}