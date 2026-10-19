//! The accepted register entries: one in-memory projection for every reader.
//!
//! A concept is represented for navigation by a register term, its one to three entry
//! addresses, and the explanatory prose of those Cards (`docs/luhmann-comparison-implementation-
//! design.md` §4.1, D058-D062). This module reads the accepted register and builds that
//! projection, so the MCP server and the GUI show one contract instead of two:
//!
//! - [`View::read`] parses the register's `词条` rows (and the rows of its declared parts) with
//!   `hub::register_rows`, validates every target of every row (not only the first, D051), and
//!   projects the bounded authored body of each existing target (§8.5, D049).
//! - [`View::json`] returns the flattened, paged entry view: `entries`, `entry_total`,
//!   `entry_next_offset`, `parts`, `faults`, `terms_by_card`, `restatements`, `complete`, `bounds`,
//!   and the archive identity/revision the view was read at.
//!
//! The view is derived and disposable: it stores nothing, and its addresses are ordinary Card
//! addresses, never concept ids. Normal Link rewriting keeps a moved entry current. The
//! module opens no file, starts no model, and saves no Card; the storage read lives in
//! `src/read.rs::register` and `src/read.rs::hubs`.
//!
//! Later slices build on this view and must keep using it: entry-first ranking (S13) ranks
//! [`View::entries`] and skips an entry whose `complete` is false, and the bounded gather and the
//! one distant Card (S19, design §9.1 `gather`/`distant`) select from the same entries. They add
//! pure functions over a supplied `View`; they do not parse the register again.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::hub::{RegisterRow, register_part_targets, register_row_faults, register_rows};
use crate::ops::Snapshot;

/// The default page of flattened entries (D276, §8.5).
pub(crate) const ENTRY_LIMIT_DEFAULT: usize = 200;
/// The largest page a reader may ask for.
pub(crate) const ENTRY_LIMIT_MAX: usize = 1000;
/// The most characters of one target's authored body the projection carries.
pub(crate) const BODY_MAX_CHARS: usize = 1200;
/// The most lines of one target's authored body the projection carries.
pub(crate) const BODY_MAX_LINES: usize = 24;

/// The requested page of flattened entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EntryArgs {
    pub(crate) offset: usize,
    pub(crate) limit: usize,
}

impl Default for EntryArgs {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: ENTRY_LIMIT_DEFAULT,
        }
    }
}

impl EntryArgs {
    /// `entry_offset` and `entry_limit`; a limit above the maximum is clamped, and the report
    /// echoes the page it served (`entry_offset`, `entry_limit`).
    pub(crate) fn from_args(args: &Value) -> Result<Self> {
        let mut entries = Self::default();
        if let Some(value) = args.get("entry_offset")
            && !value.is_null()
        {
            let offset = value
                .as_u64()
                .context("argument `entry_offset` must be a whole number of 0 or more")?;
            entries.offset = usize::try_from(offset)
                .map_err(|_| anyhow::anyhow!("argument `entry_offset` is too large"))?;
        }
        if let Some(value) = args.get("entry_limit")
            && !value.is_null()
        {
            let limit = value
                .as_u64()
                .context("argument `entry_limit` must be a whole number of 1 or more")?;
            entries.limit = (limit as usize).clamp(1, ENTRY_LIMIT_MAX);
        }
        Ok(entries)
    }
}

/// The fixed bounds a view reports, so a reader can tell a cut body from a whole one.
pub(crate) fn bounds() -> Value {
    json!({
        "body_max_chars": BODY_MAX_CHARS,
        "body_max_lines": BODY_MAX_LINES,
        "entry_limit_default": ENTRY_LIMIT_DEFAULT,
        "entry_limit_max": ENTRY_LIMIT_MAX,
    })
}

/// One target of a term: its address, whether the Card exists, and its bounded authored body.
#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub(crate) address: String,
    pub(crate) exists: bool,
    pub(crate) title: Option<String>,
    pub(crate) card_type: Option<&'static str>,
    pub(crate) facet: Option<String>,    pub(crate) workspace: Option<&'static str>,
    pub(crate) tree: String,
    pub(crate) body: Option<String>,
    pub(crate) body_chars: usize,
    pub(crate) body_lines: usize,
    pub(crate) truncated: bool,
}

impl Target {
    fn json(&self) -> Value {
        json!({
            "address": self.address,
            "exists": self.exists,
            "title": self.title,
            "type": self.card_type,
            "facet": self.facet,
            "workspace": self.workspace,
            "tree": self.tree,
            "body": self.body,
            "body_chars": self.body_chars,
            "body_lines": self.body_lines,
            "body_truncated": self.truncated,
        })
    }
}

/// One accepted term: its label, where it was written, and all of its targets.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) term: String,
    pub(crate) line: usize,
    /// The Card that holds the row: the register itself or one of its declared parts.
    pub(crate) card: String,
    /// The declared part that holds the row, if any.
    pub(crate) part: Option<String>,
    pub(crate) targets: Vec<Target>,
    pub(crate) faults: Vec<Value>,
}

impl Entry {
    fn complete(&self) -> bool {
        self.faults.is_empty()
    }

    fn json(&self) -> Value {
        json!({
            "term": self.term,
            "line": self.line,
            "origin": {"card": self.card, "line": self.line, "part": self.part},
            "targets": self.targets.iter().map(Target::json).collect::<Vec<_>>(),
            "complete": self.complete(),
            "faults": self.faults,
        })
    }
}

/// The disposable, revision-keyed view of one register (§4.1, D060-D061).
#[derive(Clone, Debug)]
pub(crate) struct View {
    pub(crate) register: String,
    pub(crate) entries: Vec<Entry>,
    /// Row, part, and restatement faults of the register as a whole.
    pub(crate) faults: Vec<Value>,
    /// The declared `词条分册` parts, in declaration order.
    pub(crate) parts: Vec<Value>,
    /// The terms each entry Card carries, in register order, derived from the explicit targets.
    pub(crate) terms_by_card: BTreeMap<String, Vec<String>>,
    /// Confirmed restatement entries (`## 同述入口`): entry Card → the substantive Card.
    pub(crate) restatements: Vec<Value>,
}

impl View {
    /// Build the view from one consistent read: the Card snapshot, the register's own address,
    /// the authored bodies of its term targets and declared parts, and the `同述入口` Links of
    /// each Card that has that section.
    pub(crate) fn read(
        snapshot: &Snapshot,
        register: &str,
        bodies: &BTreeMap<String, String>,
        restatement_sections: &BTreeMap<String, Vec<String>>,
    ) -> Self {
        let body = snapshot
            .parsed
            .get(register)
            .map(|parsed| parsed.body.clone())
            .unwrap_or_default();
        let mut view = Self {
            register: register.to_string(),
            entries: Vec::new(),
            faults: Vec::new(),
            parts: Vec::new(),
            terms_by_card: BTreeMap::new(),
            restatements: Vec::new(),
        };
        view.add_rows(snapshot, register, None, &body, bodies);
        view.read_parts(snapshot, register, &body, bodies);
        view.read_restatements(snapshot, restatement_sections);
        view
    }

    /// The term rows of one Card (the register or a declared part).
    fn add_rows(
        &mut self,
        snapshot: &Snapshot,
        card: &str,
        part: Option<&str>,
        body: &str,
        bodies: &BTreeMap<String, String>,
    ) {
        let exists = |address: &str| snapshot.by_address.contains_key(address);
        for row in register_rows(body) {
            let mut faults = register_row_faults(&row, &exists);
            for fault in &mut faults {
                fault["card"] = json!(card);
            }
            self.faults.extend(faults.clone());
            // A row that names no term or no address, or more than three, is reported and is
            // not an entry: retrieval must not invent an entry (D297, D289).
            if row.term.is_empty() || row.targets.is_empty() || row.targets.len() > 3 {
                continue;
            }
            let entry = self.entry(snapshot, &row, card, part, bodies, faults);
            for target in &entry.targets {
                if target.exists {
                    let terms = self.terms_by_card.entry(target.address.clone()).or_default();
                    if !terms.contains(&entry.term) {
                        terms.push(entry.term.clone());
                    }
                }
            }
            self.entries.push(entry);
        }
    }

    fn entry(
        &self,
        snapshot: &Snapshot,
        row: &RegisterRow,
        card: &str,
        part: Option<&str>,
        bodies: &BTreeMap<String, String>,
        faults: Vec<Value>,
    ) -> Entry {
        let mut seen = BTreeSet::new();
        let targets = row
            .targets
            .iter()
            .filter(|address| seen.insert((*address).clone()))
            .map(|address| target(snapshot, address, bodies))
            .collect();
        Entry {
            term: row.term.clone(),
            line: row.line,
            card: card.to_string(),
            part: part.map(str::to_string),
            targets,
            faults,
        }
    }

    /// The declared `词条分册` parts: their identity is the root's Links plus their place as
    /// immediate Side successors of the register (§8.4). A missing, misplaced, nested, or Hub
    /// part is reported and its rows are not accepted.
    fn read_parts(
        &mut self,
        snapshot: &Snapshot,
        register: &str,
        body: &str,
        bodies: &BTreeMap<String, String>,
    ) {
        let mut declared = BTreeSet::new();
        for address in register_part_targets(body) {
            if !declared.insert(address.clone()) {
                self.faults.push(json!({
                    "fault": "part-duplicate",
                    "detail": format!("[[{address}]] is declared twice under 词条分册"),
                    "card": register,
                }));
                continue;
            }
            let Some(card) = snapshot.by_address.get(&address) else {
                self.faults.push(json!({
                    "fault": "part-missing",
                    "detail": format!("[[{address}]] is declared under 词条分册 but does not exist"),
                    "card": register,
                }));
                self.parts.push(json!({"address": address, "title": null, "exists": false, "place": "missing", "entries": 0}));
                continue;
            };
            if card.card_type == crate::CardType::Hub {
                self.faults.push(json!({
                    "fault": "part-hub",
                    "detail": format!("[[{address}]] is a Hub Card; a register part is an ordinary Idea Card"),
                    "card": register,
                }));
                self.parts.push(json!({"address": address, "title": snapshot.title(&address), "exists": true, "place": "hub", "entries": 0}));
                continue;
            }
            if crate::parent_location(&address).as_deref() != Some(register) {
                self.faults.push(json!({
                    "fault": "part-place",
                    "detail": format!("[[{address}]] is declared under 词条分册 but is not an immediate Side successor of [[{register}]]"),
                    "card": register,
                }));
                self.parts.push(json!({"address": address, "title": snapshot.title(&address), "exists": true, "place": "moved", "entries": 0}));
                continue;
            }
            let part_body = bodies.get(&address).cloned().unwrap_or_default();
            if !register_part_targets(&part_body).is_empty() {
                self.faults.push(json!({
                    "fault": "part-nested",
                    "detail": format!("[[{address}]] declares parts of its own; parts cannot contain parts"),
                    "card": address,
                }));
            }
            let before = self.entries.len();
            self.add_rows(snapshot, &address, Some(&address), &part_body, bodies);
            self.parts.push(json!({
                "address": address,
                "title": snapshot.title(&address),
                "exists": true,
                "place": "ok",
                "entries": self.entries.len() - before,
            }));
        }
    }

    /// The confirmed restatement entries: a `## 同述入口` section that names exactly one existing
    /// substantive Card of the same Workspace, not itself (§4.5). Cycles and chains are a later
    /// slice's finding; this view records the direct reference and reports a malformed section.
    fn read_restatements(
        &mut self,
        snapshot: &Snapshot,
        sections: &BTreeMap<String, Vec<String>>,
    ) {
        let mut confirmed = Vec::new();
        for (card, links) in sections {
            let Some(own) = snapshot.by_address.get(card) else {
                continue;
            };
            if links.len() != 1 {
                self.faults.push(json!({
                    "fault": "restatement",
                    "detail": format!(
                        "the 同述入口 section of [[{card}]] names {} Cards; it names exactly one",
                        links.len()
                    ),
                    "card": card,
                }));
                continue;
            }
            let target = &links[0];
            if target == card {
                self.faults.push(json!({
                    "fault": "restatement",
                    "detail": format!("the 同述入口 section of [[{card}]] points at itself"),
                    "card": card,
                }));
                continue;
            }
            let Some(substantive) = snapshot.by_address.get(target) else {
                self.faults.push(json!({
                    "fault": "restatement",
                    "detail": format!("the 同述入口 section of [[{card}]] names [[{target}]], which does not exist"),
                    "card": card,
                }));
                continue;
            };
            if substantive.workspace != own.workspace {
                self.faults.push(json!({
                    "fault": "restatement",
                    "detail": format!(
                        "the 同述入口 section of [[{card}]] names [[{target}]] in the {} Workspace; it must stay in the {} Workspace",
                        substantive.workspace.id(),
                        own.workspace.id()
                    ),
                    "card": card,
                }));
                continue;
            }
            confirmed.push(json!({
                "entry": card,
                "substantive": target,
                "title": snapshot.title(target),
            }));
        }
        confirmed.sort_by(|a, b| {
            a["entry"]
                .as_str()
                .unwrap_or_default()
                .cmp(b["entry"].as_str().unwrap_or_default())
        });
        self.restatements = confirmed;
    }

    /// The flattened, paged entry view. `entry_offset` and `entry_limit` page the entries;
    /// `entry_next_offset` is `null` on the last page. `complete` is false when any row, part, or
    /// restatement fault was found: the view then reports less than the register holds.
    pub(crate) fn json(&self, args: EntryArgs, revision: u64, identity: &Value) -> Value {
        let total = self.entries.len();
        let start = args.offset.min(total);
        let end = start.saturating_add(args.limit).min(total);
        let next = (end < total).then_some(end);
        json!({
            "register": self.register,
            "entries": self.entries[start..end].iter().map(Entry::json).collect::<Vec<_>>(),
            "entry_offset": args.offset,
            "entry_limit": args.limit,
            "entry_total": total,
            "entry_next_offset": next,
            "parts": self.parts,
            "faults": self.faults,
            "complete": self.faults.is_empty(),
            "terms_by_card": self.terms_by_card,
            "restatements": self.restatements,
            "bounds": bounds(),
            "archive_revision": revision,
            "archive_identity": identity,
        })
    }
}

fn target(snapshot: &Snapshot, address: &str, bodies: &BTreeMap<String, String>) -> Target {
    let tree = crate::tree_id(address).to_string();
    let Some(card) = snapshot.by_address.get(address) else {
        return Target {
            address: address.to_string(),
            exists: false,
            title: None,
            card_type: None,
            facet: None,
            workspace: None,
            tree,
            body: None,
            body_chars: 0,
            body_lines: 0,
            truncated: false,
        };
    };
    let body = bodies.get(address).cloned().unwrap_or_default();
    let (kept, truncated) = bounded_body(&body);
    Target {
        address: address.to_string(),
        exists: true,
        title: Some(snapshot.title(address)),
        card_type: Some(card.card_type.id()),
        facet: card.facet.as_ref().map(|facet| crate::model::Facet::id(facet).to_string()),
        workspace: Some(card.workspace.id()),
        tree,
        body: Some(kept),
        body_chars: body.chars().count(),
        body_lines: body.lines().count(),
        truncated,
    }
}

/// The documented projection of one authored body: at most [`BODY_MAX_LINES`] lines and
/// [`BODY_MAX_CHARS`] characters, cut at a character boundary.
pub(crate) fn bounded_body(body: &str) -> (String, bool) {
    let mut kept = String::new();
    let mut truncated = false;
    for (index, line) in body.lines().enumerate() {
        if index >= BODY_MAX_LINES {
            truncated = true;
            break;
        }
        if index > 0 {
            kept.push('\n');
        }
        kept.push_str(line);
    }
    if kept.chars().count() > BODY_MAX_CHARS {
        kept = kept.chars().take(BODY_MAX_CHARS).collect();
        truncated = true;
    }
    (kept, truncated)
}

/// The Links of a Card's `## 同述入口` section, in body order. A Card without that section has
/// no restatement entry, so nothing is recorded for it.
pub(crate) fn restatement_links(body: &str) -> Vec<String> {
    let mut inside = false;
    let mut fenced = false;
    let mut links = Vec::new();
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
            .strip_prefix("## ")
            .or_else(|| trimmed.strip_prefix("### "))
        {
            inside = heading.trim() == "同述入口";
            continue;
        }
        if inside && trimmed.starts_with("# ") {
            inside = false;
            continue;
        }
        if !inside {
            continue;
        }
        // The section names its Card with list items; a `来源：` line names a source, not the
        // substantive Card.
        if !is_list_item(trimmed) {
            continue;
        }
        for occurrence in crate::link::LinkBody::for_rendering(trimmed).occurrences() {
            if !links.iter().any(|known| known == occurrence.target) {
                links.push(occurrence.target.to_string());
            }
        }
    }
    links
}

fn is_list_item(line: &str) -> bool {
    if ["- ", "* ", "+ "].iter().any(|mark| line.starts_with(mark)) {
        return true;
    }
    let digits = line.chars().take_while(char::is_ascii_digit).count();
    digits > 0
        && line[digits..]
            .strip_prefix(". ")
            .or_else(|| line[digits..].strip_prefix(") "))
            .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_is_cut_at_the_documented_bounds() {
        let long = "字".repeat(BODY_MAX_CHARS + 10);
        let (kept, truncated) = bounded_body(&long);
        assert_eq!(kept.chars().count(), BODY_MAX_CHARS);
        assert!(truncated);
        let lines = (0..BODY_MAX_LINES + 5)
            .map(|index| format!("第 {index} 行"))
            .collect::<Vec<_>>()
            .join("\n");
        let (kept, truncated) = bounded_body(&lines);
        assert_eq!(kept.lines().count(), BODY_MAX_LINES);
        assert!(truncated);
        let (kept, truncated) = bounded_body("短");
        assert_eq!(kept, "短");
        assert!(!truncated);
    }

    #[test]
    fn a_restatement_section_names_its_one_substantive_card() {
        let body = "陈述。\n\n## 同述入口\n\n- [[88/2|a]] 给出完整定义。\n\n来源：[[K]]\n";
        assert_eq!(restatement_links(body), vec!["88/2|a"]);
        assert!(restatement_links("没有入口的卡 [[88/2]]").is_empty());
        assert_eq!(restatement_links("## 同述入口\n- [[88/2]] 一处\n- [[88/3]] 两处\n").len(), 2);
    }

    #[test]
    fn entry_arguments_default_page_and_clamp_the_maximum() {
        assert_eq!(EntryArgs::from_args(&json!({})).unwrap(), EntryArgs::default());
        assert_eq!(
            EntryArgs::from_args(&json!({"entry_offset": 2, "entry_limit": 1})).unwrap(),
            EntryArgs {
                offset: 2,
                limit: 1
            }
        );
        assert_eq!(
            EntryArgs::from_args(&json!({"entry_limit": 5000})).unwrap().limit,
            ENTRY_LIMIT_MAX
        );
        assert!(EntryArgs::from_args(&json!({"entry_offset": "0"})).is_err());
        assert!(EntryArgs::from_args(&json!({"entry_limit": -1})).is_err());
    }
}
