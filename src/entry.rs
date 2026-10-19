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
//! - [`search`] is the one entry-first search: an exact address, a matching register term and its
//!   entry Cards, a matching hub member line and the Card it opens, a matching model head, a
//!   matching Card title, then a matching authored body (§9.2, D199-D206). The MCP server's
//!   `search_cards` `mode: entry` and the GUI's `/api/search` `mode=entry` both return its rows,
//!   counts, and evidence unchanged; neither adds a second ranking (D194, D198, D278).
//!
//! The view is derived and disposable: it stores nothing, and its addresses are ordinary Card
//! addresses, never concept ids. Normal Link rewriting keeps a moved entry current. The
//! module opens no file, starts no model, and saves no Card; the storage read lives in
//! `src/read.rs::register` and `src/read.rs::hubs`. The ranking is a read projection over the
//! supplied [`Snapshot`]: one coherent read, no model run, no Link walk, and bounded work
//! (`SEARCH_*` constants, reported in `bounds` and `scanned`).
//!
//! Later slices build on this view and must keep using it: restatement folding (S14) groups the
//! rows [`search`] returns, and the bounded gather and the one distant Card (S19, design §9.1
//! `gather`/`distant`) select from the same entries. They add pure functions over a supplied
//! `View`; they do not parse the register again.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::hub::{
    RegisterRow, register_part_rows, register_part_targets, register_row_faults, register_rows,
};
use crate::ops::Snapshot;
use crate::{CardType, Workspace};

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

/// Whether two rows name the same Cards: the one row an interrupted publication wrote twice
/// (§8.5, D189). The order of a term's destinations is display order, so the same Cards in another
/// order are the same entry, exactly as `mark_ambiguous_terms` reads a label.
fn same_targets(a: &Entry, b: &Entry) -> bool {
    let addresses = |entry: &Entry| {
        let mut addresses = entry
            .targets
            .iter()
            .map(|target| target.address.clone())
            .collect::<Vec<_>>();
        addresses.sort();
        addresses
    };
    addresses(a) == addresses(b)
}

/// The two search orders (D198, D275): the existing plain lexical order and the entry-first order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    Plain,
    Entry,
}

impl Mode {
    /// One `mode` value: `plain` or `entry`.
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "plain" => Ok(Self::Plain),
            "entry" => Ok(Self::Entry),
            other => bail!("unknown search mode `{other}`; expected plain or entry"),
        }
    }

    /// The mode a tool call asks for; an omitted mode is `plain`, the existing MCP behaviour, so
    /// an existing caller keeps its order and shape (D198, US024).
    pub(crate) fn from_args(args: &Value) -> Result<Self> {
        match args.get("mode") {
            None | Some(Value::Null) => Ok(Self::Plain),
            Some(Value::String(value)) => Self::parse(value),
            Some(_) => bail!("argument `mode` must be a string"),
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Entry => "entry",
        }
    }
}

/// The most rows one entry search returns: the same clamp the MCP `search_cards` already applies
/// (§12.2, D275).
pub(crate) const SEARCH_LIMIT_MAX: usize = 200;
/// The most Hub Cards one entry search reads for member lines, in natural address order.
pub(crate) const SEARCH_HUB_MAX: usize = 2000;
/// The most register term rows one entry search ranks, across the archive's registers.
pub(crate) const SEARCH_TERM_MAX: usize = 1000;
/// The most register faults one entry search reports, so one broken register cannot flood a reply.
pub(crate) const SEARCH_FAULT_MAX: usize = 100;
/// The most characters of a matched hub member line the evidence quotes.
pub(crate) const SEARCH_LINE_MAX_CHARS: usize = 200;

/// One entry-first search request: the query, the filters that apply to target Cards, and the page
/// size. The MCP server and the GUI both build it and call [`search`] (D194, D198, D278).
#[derive(Clone, Debug)]
pub(crate) struct SearchArgs {
    pub(crate) query: String,
    pub(crate) workspace: Option<Workspace>,
    pub(crate) card_type: Option<CardType>,
    pub(crate) form: Option<crate::math::MathForm>,
    pub(crate) limit: usize,
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
    /// Row, part, restatement, and ambiguous-label faults of the register as a whole.
    pub(crate) faults: Vec<Value>,
    /// The declared `词条分册` parts, in declaration order.
    pub(crate) parts: Vec<Value>,
    /// The terms each Card carries: the register rows that name it, in register order, then the
    /// accepted terms its substantive title or prose states literally, in register order (D196).
    pub(crate) terms_by_card: BTreeMap<String, Vec<String>>,
    /// Confirmed restatement entries (`## 同述入口`): entry Card → the substantive Card.
    pub(crate) restatements: Vec<Value>,
    /// Rows an earlier equivalent row already serves: the register or part they were written on,
    /// so a reader can see that an interrupted publication left a row behind (D189, AC08).
    pub(crate) duplicates: Vec<Value>,
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
            duplicates: Vec::new(),
        };
        view.add_rows(snapshot, register, None, &body, bodies);
        view.read_parts(snapshot, register, &body, bodies);
        view.read_unlisted_parts(snapshot, register, &body, bodies);
        view.read_restatements(snapshot, restatement_sections);
        view.deduplicate();
        view.mark_ambiguous_terms();
        view
    }

    /// An interrupted publication may leave the same row on the root and in a part (D189). A row
    /// the view already serves under the same term with the same Cards is served once and named in
    /// `duplicates`, so nothing is counted twice; the same term with other Cards stays two entries
    /// and is one `ambiguous` fault, so a contradictory target is never silently dropped (AC08).
    fn deduplicate(&mut self) {
        let mut served: Vec<Entry> = Vec::new();
        for entry in std::mem::take(&mut self.entries) {
            match served
                .iter()
                .find(|seen| seen.term == entry.term)
                .filter(|seen| same_targets(seen, &entry))
            {
                Some(seen) => self.duplicates.push(json!({
                    "term": entry.term,
                    "card": entry.card,
                    "part": entry.part,
                    "line": entry.line,
                    "served_by": seen.card,
                    "served_line": seen.line,
                })),
                None => served.push(entry),
            }
        }
        self.entries = served;
    }

    /// Identical term labels with conflicting destinations are one ambiguous term (§12.3, D299):
    /// every row stays an entry and every destination stays visible, and one fault names the
    /// conflict. The reader never merges the labels and never chooses between the senses; the
    /// register author gives each sense its own scoped label (§8.4).
    fn mark_ambiguous_terms(&mut self) {
        let mut by_term: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for (index, entry) in self.entries.iter().enumerate() {
            by_term.entry(entry.term.as_str()).or_default().push(index);
        }
        let mut faults = Vec::new();
        for (term, indices) in by_term {
            let mut distinct = BTreeSet::new();
            for index in &indices {
                let mut targets = self.entries[*index]
                    .targets
                    .iter()
                    .map(|target| target.address.clone())
                    .collect::<Vec<_>>();
                targets.sort();
                distinct.insert(targets);
            }
            if distinct.len() < 2 {
                continue;
            }
            faults.push(json!({
                "fault": "ambiguous",
                "term": term,
                "detail": format!(
                    "the term `{term}` is written {} times with different destinations; each sense needs its own scoped label",
                    indices.len()
                ),
                "rows": indices
                    .iter()
                    .map(|index| {
                        let entry = &self.entries[*index];
                        json!({
                            "card": entry.card,
                            "line": entry.line,
                            "part": entry.part,
                            "targets": entry
                                .targets
                                .iter()
                                .map(|target| target.address.clone())
                                .collect::<Vec<_>>(),
                        })
                    })
                    .collect::<Vec<_>>(),
            }));
        }
        self.faults.extend(faults);
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

    /// A Side successor of the register that holds `词条` rows but is no longer declared under
    /// `词条分册` is reported, never silently accepted as another register and never dropped: its
    /// rows are missing from the register, so the view says so (D182, AC05).
    fn read_unlisted_parts(
        &mut self,
        snapshot: &Snapshot,
        register: &str,
        body: &str,
        bodies: &BTreeMap<String, String>,
    ) {
        let declared = register_part_rows(body)
            .into_iter()
            .filter_map(|row| row.address)
            .collect::<BTreeSet<_>>();
        for address in crate::hub::register_side_successors(snapshot, register) {
            if declared.contains(&address) {
                continue;
            }
            let part_body = bodies
                .get(&address)
                .cloned()
                .or_else(|| snapshot.parsed.get(&address).map(|parsed| parsed.body.clone()))
                .unwrap_or_default();
            if crate::hub::register_rows(&part_body).is_empty() {
                continue;
            }
            self.faults.push(json!({
                "fault": "part-unlisted",
                "detail": format!(
                    "[[{address}]] is a Side successor of [[{register}]] and holds 词条 rows, but the register does not declare it under 词条分册; declare it or move its rows"
                ),
                "card": register,
                "part": address,
            }));
            self.parts.push(json!({
                "label": "",
                "address": address,
                "title": snapshot.title(&address),
                "exists": true,
                "place": "unlisted",
                "entries": 0,
            }));
        }
    }

    /// The declared `词条分册` parts: their identity is the root's Links plus their place as
    /// immediate Side successors of the register (§8.4). A missing, misplaced, nested, or Hub
    /// part is reported and its rows are not accepted. The accepted label a part was planned with
    /// travels with it; a declaration without one is reported, because the accepted grouping is
    /// then no longer visible in the register (D177, D184).
    fn read_parts(
        &mut self,
        snapshot: &Snapshot,
        register: &str,
        body: &str,
        bodies: &BTreeMap<String, String>,
    ) {
        let mut declared = BTreeSet::new();
        for row in register_part_rows(body) {
            let Some(address) = row.address else {
                self.faults.push(json!({
                    "fault": "part-label",
                    "detail": format!(
                        "the 词条分册 row on line {} names no Card; a part is declared as `- [[address]] A–F`",
                        row.line
                    ),
                    "card": register,
                    "line": row.line,
                }));
                continue;
            };
            if row.label.is_empty() {
                self.faults.push(json!({
                    "fault": "part-label",
                    "detail": format!(
                        "[[{address}]] is declared under 词条分册 without its accepted initial range; write the range the part groups, for example `- [[{address}]] A–F`"
                    ),
                    "card": register,
                    "line": row.line,
                    "part": address,
                }));
            }
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
                self.parts.push(json!({"label": row.label, "address": address, "title": null, "exists": false, "place": "missing", "entries": 0}));
                continue;
            };
            if card.card_type == crate::CardType::Hub {
                self.faults.push(json!({
                    "fault": "part-hub",
                    "detail": format!("[[{address}]] is a Hub Card; a register part is an ordinary Idea Card"),
                    "card": register,
                }));
                self.parts.push(json!({"label": row.label, "address": address, "title": snapshot.title(&address), "exists": true, "place": "hub", "entries": 0}));
                continue;
            }
            if crate::parent_location(&address).as_deref() != Some(register) {
                self.faults.push(json!({
                    "fault": "part-place",
                    "detail": format!("[[{address}]] is declared under 词条分册 but is not an immediate Side successor of [[{register}]]"),
                    "card": register,
                }));
                self.parts.push(json!({"label": row.label, "address": address, "title": snapshot.title(&address), "exists": true, "place": "moved", "entries": 0}));
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
                "label": row.label,
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

    /// The accepted terms one Card carries (§9.1, D196): the register rows that name it, in
    /// register order, then the accepted terms its substantive title and prose state literally, in
    /// register order. `body` is the Card's authored body, which the caller read.
    ///
    /// A term row is not the only place a Card carries a term: a Card that states an accepted term
    /// shares it even when it is not one of that term's selected entry targets, which is exactly
    /// what a distant selection asks about a held Card. The register, its parts (declared or left
    /// behind), Hub Cards and tree roots are navigation, and a source locator line or a navigation
    /// section is not prose, so none of them supplies a term. A label is matched whole, so a scoped
    /// term is never reduced to its bare word, and an incomplete row is no evidence (D297).
    pub(crate) fn terms_at(
        &self,
        snapshot: &Snapshot,
        address: &str,
        body: &str,
    ) -> Vec<String> {
        let mut terms = self
            .terms_by_card
            .get(address)
            .cloned()
            .unwrap_or_default();
        let Some(card) = snapshot.by_address.get(address) else {
            return terms;
        };
        if card.card_type == CardType::Hub
            || crate::is_tree_root(card)
            || self.navigation(snapshot).contains(address)
        {
            return terms;
        }
        let mut accepted: Vec<&str> = Vec::new();
        for entry in &self.entries {
            if !entry.complete() {
                continue;
            }
            let label = entry.term.trim();
            if !label.is_empty() && !accepted.contains(&label) {
                accepted.push(label);
            }
        }
        if accepted.is_empty() {
            return terms;
        }
        let index = TermIndex::new(&accepted);
        let found = index.occurrences(&snapshot.title(address), body);
        for position in found {
            let label = accepted[position];
            if !terms.iter().any(|known| known == label) {
                terms.push(label.to_string());
            }
        }
        terms
    }

    /// The Cards of this register's own navigation: the register, its declared parts, and every
    /// immediate Side successor that holds term rows (a part an interrupted publication left
    /// undeclared). They are ways in, never substance.
    fn navigation(&self, snapshot: &Snapshot) -> BTreeSet<String> {
        let mut navigation = BTreeSet::new();
        navigation.insert(self.register.clone());
        for part in &self.parts {
            if let Some(address) = part["address"].as_str() {
                navigation.insert(address.to_string());
            }
        }
        navigation.extend(crate::hub::register_side_successors(
            snapshot,
            &self.register,
        ));
        navigation
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
            "duplicates": self.duplicates,
            "bounds": bounds(),
            "archive_revision": revision,
            "archive_identity": identity,
        })
    }
}

/// The rank tiers of the entry-first order (§9.2, D199-D206), reported as `rank_tier`.
const TIER_ADDRESS: u8 = 1;
const TIER_TERM: u8 = 2;
const TIER_MEMBER: u8 = 3;
const TIER_MODEL: u8 = 4;
const TIER_TITLE: u8 = 5;
const TIER_BODY: u8 = 6;

/// What one query means to a label: every word must occur, and an exact label beats a prefix,
/// which beats a partial hit (§9.2). Exact-first orders the declarations.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Quality {
    Exact,
    Prefix,
    Partial,
}

impl Quality {
    fn word(self) -> &'static str {
        match self {
            Self::Exact => "exactly",
            Self::Prefix => "as a prefix",
            Self::Partial => "partly",
        }
    }
}

/// The folded words of a query: the whole needle, the same without whitespace for exact and prefix
/// comparisons, and the words a label or title must hold. Plain and entry search both fold case.
struct Query {
    needle: String,
    compact: String,
    terms: Vec<String>,
}

impl Query {
    fn new(query: &str) -> Self {
        let needle = fold(query);
        let compact = compact(&needle);
        let terms = needle.split_whitespace().map(str::to_string).collect();
        Self {
            needle,
            compact,
            terms,
        }
    }

    fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// The quality of a label match, or `None` when a query word is absent. Matching ignores
    /// spacing (and case), so `学习 曲线` still names the label `学习曲线` and `学 习` the label
    /// `学习`; an exact label beats a prefix, which beats a partial hit.
    fn quality(&self, label: &str) -> Option<Quality> {
        if self.is_empty() {
            return None;
        }
        let label = compact(&fold(label));
        if !self
            .terms
            .iter()
            .all(|term| label.contains(term.as_str()))
        {
            return None;
        }
        if label == self.compact {
            Some(Quality::Exact)
        } else if label.starts_with(&self.compact) {
            Some(Quality::Prefix)
        } else {
            Some(Quality::Partial)
        }
    }

    /// The first query word a text holds, for the snippet's centre.
    fn snippet_term(&self, text: &str) -> Option<String> {
        let folded = text.to_lowercase();
        self.terms
            .iter()
            .find(|term| folded.contains(term.as_str()))
            .cloned()
    }
}

fn fold(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn compact(text: &str) -> String {
    text.chars().filter(|ch| !ch.is_whitespace()).collect()
}

/// One row before ranking: its tier, match quality, the current lexical score, and the evidence
/// that explains it. A Card named by more than one tier keeps its highest tier: a term is not a
/// second Card, and a register target that is also a title or body hit appears once (D201-D206).
struct Candidate {
    tier: u8,
    quality: Quality,
    score: i64,
    address: String,
    role: &'static str,
    field: &'static str,
    matched_term: Option<String>,
    matched_line: Option<String>,
    reason: String,
    origin: Option<Value>,
    /// Every existing target of the entry's term, in register order. The row reports those that
    /// survive the filters, so a term's one to three destinations stay visible (D207).
    entry_targets: Vec<String>,
    body_term: Option<String>,
}

/// How much of a bounded scan one search used, so the reply can report work and truncation.
#[derive(Default)]
struct Scan {
    count: usize,
    truncated: bool,
}

/// The archive's registers, one per Workspace at most, and the faults that keep a register out of
/// retrieval (D296, D297, D299). An absent register is reported, never substituted.
struct Registers {
    views: Vec<View>,
    /// One row per Workspace, in `Workspace::ALL` order: its register or `missing`.
    summary: Vec<Value>,
    faults: Vec<Value>,
}

/// The one entry-first search projection: rows, order, counts, and evidence are the same for the
/// MCP server and the GUI (D193, D194, D198, D278). It is a read-only projection over one Card
/// read: no model run, no Card write, no Link walk, and bounded work reported in `bounds` and
/// `scanned`. An empty query has no ranking; callers keep the plain listing path for it.
pub(crate) fn search(snapshot: &Snapshot, args: &SearchArgs) -> Value {
    let query = Query::new(&args.query);
    let limit = args.limit.clamp(1, SEARCH_LIMIT_MAX);
    let registers = registers(snapshot);
    let lexical = snapshot.lexical(&args.query);
    let mut lexical_at = BTreeMap::new();
    for (index, found) in lexical.iter().enumerate() {
        lexical_at.insert(snapshot.cards[found.order].address.as_str(), index);
    }
    let score = |address: &str| {
        lexical_at
            .get(address)
            .map_or(0, |index| lexical[*index].score)
    };
    let mut candidates = Vec::new();
    let mut terms = Scan::default();
    let mut hubs = Scan::default();
    if !query.is_empty() {
        address_candidate(snapshot, &query, &score, &mut candidates);
        terms = term_candidates(&registers, &query, &score, &mut candidates);
        hubs = member_candidates(snapshot, &query, &score, &mut candidates);
        model_candidates(snapshot, &query, &score, &mut candidates);
        lexical_candidates(&lexical, snapshot, &query, &mut candidates);
    }
    ranked(
        snapshot,
        args,
        &query,
        registers,
        candidates,
        terms,
        hubs,
        limit,
    )
}

#[allow(clippy::too_many_arguments)]
fn ranked(
    snapshot: &Snapshot,
    args: &SearchArgs,
    query: &Query,
    registers: Registers,
    mut candidates: Vec<Candidate>,
    terms: Scan,
    hubs: Scan,
    limit: usize,
) -> Value {
    // The lexicographic tier order, then exact/prefix/partial, then the current lexical score,
    // then natural address order as the final tie-break (§9.2, D206).
    candidates.sort_by(|left, right| {
        left.tier
            .cmp(&right.tier)
            .then(left.quality.cmp(&right.quality))
            .then(right.score.cmp(&left.score))
            .then_with(|| crate::model::address_order(&left.address, &right.address))
    });
    let mut seen = BTreeSet::new();
    candidates.retain(|candidate| seen.insert(candidate.address.clone()));

    // Counts filter one dimension at a time, as the plain search's chips do: by Workspace under
    // the type and form filters, by type under the Workspace and form filters. A count counts
    // matching Cards, never displayed rows (D208).
    let mut workspaces = serde_json::Map::new();
    let mut types = serde_json::Map::new();
    let mut total = 0usize;
    for candidate in &candidates {
        let Some(card) = snapshot.by_address.get(&candidate.address) else {
            continue;
        };
        if args
            .card_type
            .is_none_or(|card_type| card.card_type == card_type)
            && args.form.is_none_or(|form| card.form == Some(form))
        {
            let count = workspaces.entry(card.workspace.id()).or_insert(json!(0));
            *count = json!(count.as_u64().unwrap_or(0) + 1);
        }
        if args
            .workspace
            .is_none_or(|workspace| card.workspace == workspace)
            && args.form.is_none_or(|form| card.form == Some(form))
        {
            let count = types.entry(card.card_type.id()).or_insert(json!(0));
            *count = json!(count.as_u64().unwrap_or(0) + 1);
        }
        if keeps(snapshot, args, &candidate.address) {
            total += 1;
        }
    }
    let results = candidates
        .iter()
        .filter(|candidate| keeps(snapshot, args, &candidate.address))
        .take(limit)
        .filter_map(|candidate| row(snapshot, args, query, candidate))
        .collect::<Vec<_>>();
    let shown = results.len();
    let register_missing = registers
        .summary
        .iter()
        .all(|register| register["missing"] == json!(true));
    json!({
        "mode": Mode::Entry.id(),
        "query": args.query.trim(),
        "results": results,
        "shown": shown,
        "total": total,
        "row_total": total,
        "workspaces": workspaces,
        "types": types,
        "registers": registers.summary,
        "register_missing": register_missing,
        "register_faults": registers.faults.iter().take(SEARCH_FAULT_MAX).collect::<Vec<_>>(),
        "register_faults_truncated": registers.faults.len() > SEARCH_FAULT_MAX,
        "bounds": {
            "results_limit_max": SEARCH_LIMIT_MAX,
            "hub_scan_max": SEARCH_HUB_MAX,
            "term_max": SEARCH_TERM_MAX,
            "register_faults_max": SEARCH_FAULT_MAX,
            "matched_line_max_chars": SEARCH_LINE_MAX_CHARS,
        },
        "scanned": {
            "registers": registers.views.len(),
            "terms": terms.count,
            "terms_truncated": terms.truncated,
            "hubs": hubs.count,
            "hubs_truncated": hubs.truncated,
        },
        "archive_fingerprint": snapshot.fingerprint(),
    })
}

/// One row of the entry-first order: the existing Card summary fields, plus the role, the rank
/// tier, the term or line evidence, the explanation, and the navigation Card it came from.
fn row(snapshot: &Snapshot, args: &SearchArgs, query: &Query, candidate: &Candidate) -> Option<Value> {
    let card = snapshot.by_address.get(&candidate.address)?;
    let tree = crate::tree_id(&candidate.address);
    let body = snapshot
        .parsed
        .get(&candidate.address)
        .map_or("", |parsed| parsed.body.as_str());
    let body_term = candidate
        .body_term
        .clone()
        .or_else(|| query.snippet_term(body));
    let entry_targets = if candidate.role == "entry" {
        candidate
            .entry_targets
            .iter()
            .filter(|address| keeps(snapshot, args, address))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Some(json!({
        "address": card.address,
        "title": snapshot.title(&card.address),
        "kind": crate::ops::kind(card),
        "type": card.card_type.id(),
        "facet": card.facet.as_ref().map(crate::model::Facet::id),
        "emphasis": card.emphasis.map(crate::model::Emphasis::id),
        "level": card.level_id(),
        "form": card.form.map(crate::math::MathForm::id),
        "workspace": card.workspace.id(),
        "tree": tree,
        "tree_title": snapshot.title(&crate::tree_root_address(tree)),
        "field": candidate.field,
        "score": candidate.score,
        "snippet": snapshot.snippet(&candidate.address, body_term.as_deref()),
        "role": candidate.role,
        "rank_tier": candidate.tier,
        "matched_term": candidate.matched_term,
        "matched_line": candidate.matched_line,
        "reason": candidate.reason,
        "origin": candidate.origin,
        "entry_targets": entry_targets,
    }))
}

/// Whether a target Card survives the Workspace, type, and form filters. Filters apply to the
/// Cards a row opens, before rows and counts are built (D208).
fn keeps(snapshot: &Snapshot, args: &SearchArgs, address: &str) -> bool {
    let Some(card) = snapshot.by_address.get(address) else {
        return false;
    };
    args.workspace
        .is_none_or(|workspace| card.workspace == workspace)
        && args
            .card_type
            .is_none_or(|card_type| card.card_type == card_type)
        && args.form.is_none_or(|form| card.form == Some(form))
}

/// Tier 1 (D200): an exact address query opens that Card first, before any term match.
fn address_candidate(
    snapshot: &Snapshot,
    query: &Query,
    score: &dyn Fn(&str) -> i64,
    candidates: &mut Vec<Candidate>,
) {
    let Some(card) = snapshot
        .cards
        .iter()
        .filter(|card| card.address.to_lowercase() == query.needle)
        .min_by(|left, right| crate::model::address_order(&left.address, &right.address))
    else {
        return;
    };
    candidates.push(Candidate {
        tier: TIER_ADDRESS,
        quality: Quality::Exact,
        score: score(&card.address),
        address: card.address.clone(),
        role: "hit",
        field: "address",
        matched_term: None,
        matched_line: None,
        reason: "the query is this Card's address".to_string(),
        origin: None,
        entry_targets: Vec::new(),
        body_term: None,
    });
}

/// Tier 2 (D201): a matching register term and its entry Cards. A row with a fault is ignored for
/// retrieval and stays reported by the register faults (D297); a term whose targets all fail the
/// filters contributes no row (D208).
fn term_candidates(
    registers: &Registers,
    query: &Query,
    score: &dyn Fn(&str) -> i64,
    candidates: &mut Vec<Candidate>,
) -> Scan {
    let mut scan = Scan::default();
    'views: for view in &registers.views {
        for entry in &view.entries {
            if scan.count >= SEARCH_TERM_MAX {
                scan.truncated = true;
                break 'views;
            }
            scan.count += 1;
            if !entry.complete() {
                continue;
            }
            let Some(quality) = query.quality(&entry.term) else {
                continue;
            };
            let targets = entry
                .targets
                .iter()
                .filter(|target| target.exists)
                .collect::<Vec<_>>();
            if targets.is_empty() {
                continue;
            }
            let addresses = targets
                .iter()
                .map(|target| target.address.clone())
                .collect::<Vec<_>>();
            for target in targets {
                candidates.push(Candidate {
                    tier: TIER_TERM,
                    quality,
                    score: score(&target.address),
                    address: target.address.clone(),
                    role: "entry",
                    field: "term",
                    matched_term: Some(entry.term.clone()),
                    matched_line: None,
                    reason: format!(
                        "the accepted register term `{}` names this Card ({} line {})",
                        entry.term, entry.card, entry.line
                    ),
                    origin: Some(json!({
                        "card": entry.card,
                        "line": entry.line,
                        "part": entry.part,
                    })),
                    entry_targets: addresses.clone(),
                    body_term: None,
                });
            }
        }
    }
    scan
}

/// Tier 3 (D202): a matching hub member line and the Card it opens. The line's authored words and
/// the title of the Card it opens both name it, as the rendered line does.
fn member_candidates(
    snapshot: &Snapshot,
    query: &Query,
    score: &dyn Fn(&str) -> i64,
    candidates: &mut Vec<Candidate>,
) -> Scan {
    let mut hubs = snapshot
        .cards
        .iter()
        .filter(|card| card.card_type == CardType::Hub)
        .collect::<Vec<_>>();
    hubs.sort_by(|left, right| crate::model::address_order(&left.address, &right.address));
    let mut scan = Scan::default();
    for hub in hubs {
        if scan.count >= SEARCH_HUB_MAX {
            scan.truncated = true;
            break;
        }
        scan.count += 1;
        let Some(parsed) = snapshot.parsed.get(&hub.address) else {
            continue;
        };
        let register = hub.hub_level == Some(crate::hub::HubLevel::Register);
        for line in crate::hub::parse_body(&parsed.body).lines {
            // A register's term rows are tier 2. Only its other groups are ordinary hub lines.
            if register && register_navigation(line.group.as_deref()) {
                continue;
            }
            if !snapshot.by_address.contains_key(&line.target) {
                continue;
            }
            let label = format!("{} {}", line.text, snapshot.title(&line.target));
            let Some(quality) = query.quality(&label) else {
                continue;
            };
            candidates.push(Candidate {
                tier: TIER_MEMBER,
                quality,
                score: score(&line.target),
                address: line.target.clone(),
                role: "member",
                field: "member",
                matched_term: None,
                matched_line: Some(crate::ops::clip(&line.text, SEARCH_LINE_MAX_CHARS)),
                reason: format!(
                    "the hub line at {} line {} opens this Card",
                    hub.address, line.line
                ),
                origin: Some(json!({
                    "card": hub.address,
                    "line": line.line,
                    "part": Value::Null,
                })),
                entry_targets: Vec::new(),
                body_term: None,
            });
        }
    }
    scan
}

/// The register groups that hold term rows or part declarations, not ordinary navigation.
fn register_navigation(group: Option<&str>) -> bool {
    match group {
        None => true,
        Some(name) => {
            crate::hub::REGISTER_TERM_GROUPS.contains(&name)
                || name == crate::hub::REGISTER_PART_GROUP
        }
    }
}

/// Tier 4 (D203): a matching model head. A Model Card without a facet is the entry point of one
/// whole model, so its own name is a way in ahead of ordinary title hits.
fn model_candidates(
    snapshot: &Snapshot,
    query: &Query,
    score: &dyn Fn(&str) -> i64,
    candidates: &mut Vec<Candidate>,
) {
    for card in &snapshot.cards {
        if card.card_type != CardType::Model || card.facet.is_some() {
            continue;
        }
        let Some(quality) = query.quality(&snapshot.title(&card.address)) else {
            continue;
        };
        candidates.push(Candidate {
            tier: TIER_MODEL,
            quality,
            score: score(&card.address),
            address: card.address.clone(),
            role: "hit",
            field: "model",
            matched_term: None,
            matched_line: None,
            reason: format!(
                "this Model Card's name matches the query {}",
                quality.word()
            ),
            origin: None,
            entry_targets: Vec::new(),
            body_term: None,
        });
    }
}

/// Tiers 5 and 6 (D204, D205): matching Card titles and matching authored bodies, from the plain
/// lexical match set and its current score. A title that holds every query word is a title hit;
/// anything else the plain search found is an ordinary hit on its address or body.
fn lexical_candidates(
    lexical: &[crate::ops::Lexical],
    snapshot: &Snapshot,
    query: &Query,
    candidates: &mut Vec<Candidate>,
) {
    for found in lexical {
        let card = &snapshot.cards[found.order];
        let (tier, quality) = match query.quality(&snapshot.title(&card.address)) {
            Some(quality) => (TIER_TITLE, quality),
            None => (TIER_BODY, Quality::Partial),
        };
        let reason = match (tier, found.field) {
            (TIER_TITLE, _) => format!("the Card title matches the query {}", quality.word()),
            (_, "address") => "the Card address matches the query".to_string(),
            (_, "title") => "the Card title holds one query word; the rest match its text".to_string(),
            _ => "the Card text matches the query".to_string(),
        };
        candidates.push(Candidate {
            tier,
            quality,
            score: found.score,
            address: card.address.clone(),
            role: "hit",
            field: found.field,
            matched_term: None,
            matched_line: None,
            reason,
            origin: None,
            entry_targets: Vec::new(),
            body_term: found.body_term.clone(),
        });
    }
}

/// The archive's registers, in `Workspace::ALL` order, each built from the one Card read. Two
/// registers in one Workspace are reported and neither is used, so search never chooses between
/// them (D296, D299).
/// The one register of a Workspace as an entry view, or none: the same discovery `registers` uses.
/// A Workspace with two registers answers none, so the evidence never chooses between them
/// (D299). `hub` reads it for the accepted terms that justify `one-book` advice (§8.2, D166).
pub(crate) fn register_of(snapshot: &Snapshot, workspace: Workspace) -> Option<View> {
    let mut found = snapshot
        .cards
        .iter()
        .filter(|card| {
            card.card_type == CardType::Hub
                && card.hub_level == Some(crate::hub::HubLevel::Register)
                && card.workspace == workspace
        })
        .map(|card| card.address.clone())
        .collect::<Vec<_>>();
    found.sort_by(|left, right| crate::model::address_order(left, right));
    match found.as_slice() {
        [at] => Some(read_view(snapshot, at)),
        _ => None,
    }
}

fn registers(snapshot: &Snapshot) -> Registers {
    let mut views = Vec::new();
    let mut summary = Vec::new();
    let mut faults = Vec::new();
    for workspace in Workspace::ALL {
        let mut found = snapshot
            .cards
            .iter()
            .filter(|card| {
                card.card_type == CardType::Hub
                    && card.hub_level == Some(crate::hub::HubLevel::Register)
                    && card.workspace == workspace
            })
            .map(|card| card.address.clone())
            .collect::<Vec<_>>();
        found.sort_by(|left, right| crate::model::address_order(left, right));
        match found.as_slice() {
            [] => summary.push(json!({
                "workspace": workspace.id(),
                "card": Value::Null,
                "title": Value::Null,
                "missing": true,
                "complete": false,
                "entry_total": 0,
                "faults": 0,
            })),
            [at] => {
                let view = read_view(snapshot, at);
                summary.push(json!({
                    "workspace": workspace.id(),
                    "card": at,
                    "title": snapshot.title(at),
                    "missing": false,
                    "complete": view.faults.is_empty(),
                    "entry_total": view.entries.len(),
                    "faults": view.faults.len(),
                }));
                faults.extend(view.faults.iter().cloned());
                views.push(view);
            }
            many => {
                summary.push(json!({
                    "workspace": workspace.id(),
                    "card": Value::Null,
                    "title": Value::Null,
                    "missing": false,
                    "complete": false,
                    "entry_total": 0,
                    "faults": 1,
                    "duplicate_registers": many,
                }));
                faults.push(json!({
                    "fault": "duplicate",
                    "detail": format!(
                        "the {} Workspace has {} registers ({}); a Workspace has one, so none of them is used for search",
                        workspace.id(),
                        many.len(),
                        many.join(", ")
                    ),
                    "workspace": workspace.id(),
                }));
            }
        }
    }
    Registers {
        views,
        summary,
        faults,
    }
}

/// One register's view from the supplied Snapshot: its term rows, the declared parts, and the
/// authored bodies the projection carries. Restatements are folded by a later slice (S14), so
/// search does not scan every body for them.
fn read_view(snapshot: &Snapshot, register: &str) -> View {
    let body = snapshot
        .parsed
        .get(register)
        .map(|parsed| parsed.body.clone())
        .unwrap_or_default();
    let mut wanted = BTreeSet::new();
    for row in register_rows(&body) {
        wanted.extend(row.targets);
    }
    for part in register_part_targets(&body) {
        wanted.insert(part);
    }
    let mut bodies = BTreeMap::new();
    for address in wanted {
        if let Some(parsed) = snapshot.parsed.get(&address) {
            bodies.insert(address.clone(), parsed.body.clone());
        }
    }
    View::read(snapshot, register, &bodies, &BTreeMap::new())
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

/// The `## 同述入口` section: a Card's confirmed restatement entry (§4.5).
const RESTATEMENT_GROUP: &str = "同述入口";

/// One authored body's prose: its lines outside the navigation sections (`## 邻近`, `## 入口`,
/// `## 词条`, `## 词条分册`, `## 同述入口`), its source locator lines, and its fenced code. A term
/// written in a way-in list, a citation, or a chart is not the Card's own statement (§9.1, D196).
fn prose(body: &str) -> String {
    let mut kept = String::with_capacity(body.len());
    let mut navigation = false;
    let mut fenced = false;
    for line in body.lines() {
        let trimmed = line.trim();
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
            let heading = heading.trim();
            navigation =
                heading == RESTATEMENT_GROUP || crate::hub::is_navigation_group(heading);
            continue;
        }
        if navigation || crate::hub::is_source_line(trimmed) {
            continue;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    kept
}

/// The accepted term labels as one matcher: a Card's title or prose states a term when the whole
/// label occurs in it. Case is folded and the label's own spacing is kept, so `学习（个体行为）` is
/// never widened into the bare word `学习`. The labels are grouped by their first character, so one
/// Card's text is walked once instead of once per term.
struct TermIndex {
    folded: Vec<String>,
    by_first: std::collections::HashMap<char, Vec<usize>>,
}

impl TermIndex {
    fn new(labels: &[&str]) -> Self {
        let folded = labels
            .iter()
            .map(|label| label.to_lowercase())
            .collect::<Vec<_>>();
        let mut by_first: std::collections::HashMap<char, Vec<usize>> =
            std::collections::HashMap::new();
        for (index, label) in folded.iter().enumerate() {
            if let Some(first) = label.chars().next() {
                by_first.entry(first).or_default().push(index);
            }
        }
        Self { folded, by_first }
    }

    /// The positions of the accepted labels the title or prose states, in label order.
    fn occurrences(&self, title: &str, body: &str) -> Vec<usize> {
        let mut text = String::with_capacity(title.len() + body.len());
        text.push_str(&title.to_lowercase());
        text.push('\n');
        text.push_str(&prose(body).to_lowercase());
        let mut found = BTreeSet::new();
        for (at, ch) in text.char_indices() {
            let Some(candidates) = self.by_first.get(&ch) else {
                continue;
            };
            for index in candidates {
                if text[at..].starts_with(self.folded[*index].as_str()) {
                    found.insert(*index);
                }
            }
        }
        found.into_iter().collect()
    }
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
            inside = heading.trim() == RESTATEMENT_GROUP;
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

    fn target(address: &str) -> Target {
        Target {
            address: address.to_string(),
            exists: true,
            title: None,
            card_type: None,
            facet: None,
            workspace: Some("agent"),
            tree: "5".to_string(),
            body: None,
            body_chars: 0,
            body_lines: 0,
            truncated: false,
        }
    }

    fn entry(term: &str, card: &str, part: Option<&str>, line: usize, targets: &[&str]) -> Entry {
        Entry {
            term: term.to_string(),
            line,
            card: card.to_string(),
            part: part.map(str::to_string),
            targets: targets.iter().map(|at| target(at)).collect(),
            faults: Vec::new(),
        }
    }

    #[test]
    fn an_equivalent_row_is_served_once_and_a_contradiction_stays_two_entries() {
        let mut view = View {
            register: "2/1".to_string(),
            entries: vec![
                entry("均衡", "2/1", None, 3, &["5/1", "5/2"]),
                // The same row left in a part: served once, named as a duplicate.
                entry("均衡", "3/1", Some("3/1"), 2, &["5/1", "5/2"]),
                // The same term with other Cards: two entries, and mark_ambiguous_terms names it.
                entry("学习", "2/1", None, 4, &["6/1"]),
                entry("学习", "3/1", Some("3/1"), 3, &["7/1"]),
                // The same Cards in another order are the same entry: served once, not a fault.
                entry("投资", "2/1", None, 5, &["8/1", "8/2"]),
                entry("投资", "3/1", Some("3/1"), 4, &["8/2", "8/1"]),
            ],
            faults: Vec::new(),
            parts: Vec::new(),
            terms_by_card: BTreeMap::new(),
            restatements: Vec::new(),
            duplicates: Vec::new(),
        };
        view.deduplicate();
        view.mark_ambiguous_terms();
        assert_eq!(view.entries.len(), 4, "the two leftover rows are served once");
        assert_eq!(view.duplicates.len(), 2);
        assert_eq!(view.duplicates[0]["term"], json!("均衡"));
        assert_eq!(view.duplicates[0]["card"], json!("3/1"));
        assert_eq!(view.duplicates[0]["served_by"], json!("2/1"));
        assert_eq!(view.duplicates[0]["served_line"], json!(3));
        assert_eq!(
            view.duplicates[1]["term"],
            json!("投资"),
            "the same Cards in another order are one entry: {:?}",
            view.duplicates
        );
        let conflicting = view
            .faults
            .iter()
            .filter(|fault| fault["fault"] == json!("ambiguous"))
            .map(|fault| fault["term"].clone())
            .collect::<Vec<_>>();
        assert_eq!(
            conflicting,
            vec![json!("学习")],
            "the contradictory label is reported and the equivalent one is not: {:?}",
            view.faults
        );
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

    #[test]
    fn a_search_mode_defaults_to_plain_and_parses_entry() {
        assert_eq!(Mode::from_args(&json!({})).unwrap(), Mode::Plain);
        assert_eq!(Mode::from_args(&json!({"mode": "entry"})).unwrap(), Mode::Entry);
        assert_eq!(Mode::from_args(&json!({"mode": " PLAIN "})).unwrap(), Mode::Plain);
        assert!(Mode::from_args(&json!({"mode": "fuzzy"})).is_err());
        assert!(Mode::from_args(&json!({"mode": 1})).is_err());
    }

    #[test]
    fn a_label_match_prefers_exact_then_prefix_then_partial() {
        let query = Query::new("学习");
        assert_eq!(query.quality("学习"), Some(Quality::Exact));
        assert_eq!(query.quality("学习曲线"), Some(Quality::Prefix));
        assert_eq!(query.quality("干中学与学习"), Some(Quality::Partial));
        assert_eq!(query.quality("干中学"), None);
        assert_eq!(
            query.quality("学 习"),
            Some(Quality::Exact),
            "spacing does not change a label"
        );
        let words = Query::new("学习 曲线");
        assert_eq!(words.quality("学习曲线"), Some(Quality::Exact));
        assert_eq!(words.quality("曲线的学习"), Some(Quality::Partial));
        assert_eq!(words.quality("学习"), None);
        assert_eq!(Query::new("").quality("学习"), None);
    }

    #[test]
    fn prose_leaves_out_source_lines_navigation_sections_and_code() {
        let body = "作者自己的陈述。\n\n来源：《学习手册》第 3 章。\n\n## 邻近\n- [[5/1]] 讨论学习。\n\n## 主张\n学习提高生产率。\n\n```\n学习\n```\n";
        let kept = prose(body);
        assert!(kept.contains("作者自己的陈述。"), "{kept}");
        assert!(kept.contains("学习提高生产率。"), "{kept}");
        assert!(!kept.contains("手册"), "a 来源 line is not prose: {kept}");
        assert!(!kept.contains("讨论学习"), "a 邻近 list is not prose: {kept}");
        assert!(!kept.contains("```"), "fenced code is not prose: {kept}");
        assert_eq!(
            kept.matches("学习").count(),
            1,
            "only the ordinary prose line states the term: {kept}"
        );
        // A section after a navigation group is prose again.
        assert!(prose("## 入口\n- [[1/1]] 说明。\n\n## 历史\n学习。\n").contains("学习。"));
    }

    #[test]
    fn a_term_index_matches_a_whole_label_and_keeps_scoped_senses_apart() {
        let index = TermIndex::new(&["学习", "学习（个体行为）", "Learning by Doing"]);
        assert_eq!(
            index.occurrences("个体行为", "学习（个体行为）提高技能。"),
            vec![0, 1],
            "the scoped label states its own sense and the bare label occurs in it"
        );
        assert_eq!(
            index.occurrences("一般讨论", "学习是一件复杂的事。"),
            vec![0],
            "a bare word is no scoped sense"
        );
        assert_eq!(
            index.occurrences("干中学", "累计产量提高。"),
            Vec::<usize>::new()
        );
        assert_eq!(
            index.occurrences("learning", "Learning by Doing raises productivity."),
            vec![2],
            "case is folded for a Latin label"
        );
    }
}
