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
    /// substantive Card of the same Workspace, not itself (§4.5, D084-D086). An entry names its
    /// terminal substantive Card: a section that names another entry is a `chain` fault that names
    /// the terminal to write, and a loop of such sections is a `cycle` fault. The view records no
    /// confirmed row for either, so a faulty entry can never become folding evidence.
    fn read_restatements(
        &mut self,
        snapshot: &Snapshot,
        sections: &BTreeMap<String, Vec<String>>,
    ) {
        let parts: BTreeSet<&str> = self
            .parts
            .iter()
            .filter_map(|part| part["address"].as_str())
            .collect();
        let (confirmed, faults) = restatement_rows(snapshot, sections, &parts);
        self.faults.extend(faults);
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

/// The confirmed `## 同述入口` entries of the supplied sections, and the faults that keep one
/// from being confirmed (§4.5, D084-D086). `parts` are the register parts of the read: they are
/// navigation, so an entry that names one is never confirmed. A register view calls this for its
/// own Workspace, and the entry search calls it once for the whole archive, so a fold needs no
/// register.
fn restatement_rows(
    snapshot: &Snapshot,
    sections: &BTreeMap<String, Vec<String>>,
    parts: &BTreeSet<&str>,
) -> (Vec<Value>, Vec<Value>) {
    let mut faults = Vec::new();
    if sections.is_empty() {
        return (Vec::new(), faults);
    }
    let entries: BTreeSet<&str> = sections.keys().map(String::as_str).collect();
    // The one Link of each entry that may be followed: a section with a different shape is
    // reported where it is read and never becomes a step of a chain.
    let mut step: BTreeMap<&str, &str> = BTreeMap::new();
    let mut direct: BTreeMap<&str, &str> = BTreeMap::new();
    let mut confirmed = Vec::new();
    let mut reported: BTreeSet<&str> = BTreeSet::new();
    for (card, links) in sections {
        let card = card.as_str();
        let Some(own) = snapshot.by_address.get(card) else {
            continue;
        };
        if links.len() != 1 {
            faults.push(json!({
                "fault": "restatement",
                "kind": "malformed",
                "detail": format!(
                    "the 同述入口 section of [[{card}]] names {} Cards; it names exactly one",
                    links.len()
                ),
                "card": card,
            }));
            reported.insert(card);
            continue;
        }
        let target = links[0].as_str();
        if target == card {
            faults.push(json!({
                "fault": "restatement",
                "kind": "self",
                "detail": format!("the 同述入口 section of [[{card}]] points at itself"),
                "card": card,
            }));
            reported.insert(card);
            continue;
        }
        let Some(substantive) = snapshot.by_address.get(target) else {
            faults.push(json!({
                "fault": "restatement",
                "kind": "missing",
                "detail": format!("the 同述入口 section of [[{card}]] names [[{target}]], which does not exist"),
                "card": card,
            }));
            reported.insert(card);
            continue;
        };
        if substantive.workspace != own.workspace {
            faults.push(json!({
                "fault": "restatement",
                "kind": "workspace",
                "detail": format!(
                    "the 同述入口 section of [[{card}]] names [[{target}]] in the {} Workspace; it must stay in the {} Workspace",
                    substantive.workspace.id(),
                    own.workspace.id()
                ),
                "card": card,
            }));
            reported.insert(card);
            continue;
        }
        // A root, a Hub Card and a declared register part are navigation, not a statement: the
        // section names a substantive Card or it names nothing this view can confirm.
        if crate::is_tree_root(substantive) || substantive.card_type == CardType::Hub || parts.contains(target) {
            faults.push(json!({
                "fault": "restatement",
                "kind": "not-substantive",
                "detail": format!(
                    "the 同述入口 section of [[{card}]] names [[{target}]], which is navigation, not a substantive Card"
                ),
                "card": card,
            }));
            reported.insert(card);
            continue;
        }
        if entries.contains(target) {
            // A chain: the direct target is itself an entry. It is reported, and the entry
            // names the terminal instead (D085).
            step.insert(card, target);
            continue;
        }
        confirmed.push(json!({
            "entry": card,
            "substantive": target,
            "terminal": target,
            "title": snapshot.title(target),
        }));
        direct.insert(card, target);
    }
    // Resolve every chain to its terminal, and report each loop once per entry on it. The
    // compiler writes a direct terminal reference, so a chain is never accepted as written.
    let starts: Vec<&str> = step
        .keys()
        .copied()
        .filter(|card| !reported.contains(card))
        .collect();
    for start in starts {
        if reported.contains(start) {
            continue;
        }
        let mut path: Vec<&str> = vec![start];
        let mut loop_nodes: Vec<&str> = Vec::new();
        let terminal: Option<&str> = loop {
            let at = *path.last().expect("a chain has a start");
            if let Some(terminal) = direct.get(at) {
                break Some(*terminal);
            }
            let Some(next) = step.get(at) else {
                break None;
            };
            if let Some(seen) = path.iter().position(|known| *known == *next) {
                loop_nodes = path[seen..].to_vec();
                break None;
            }
            path.push(next);
        };
        let shown = path
            .iter()
            .map(|card| format!("[[{card}]]"))
            .collect::<Vec<_>>()
            .join(" → ");
        match terminal {
            Some(terminal) => {
                reported.insert(start);
                faults.push(json!({
                    "fault": "restatement",
                    "kind": "chain",
                    "detail": format!(
                        "the 同述入口 section of [[{start}]] names another entry ({shown}); name the terminal substantive Card [[{terminal}]] directly"
                    ),
                    "card": start,
                    "terminal": terminal,
                    "chain": path,
                    "cycle": Value::Null,
                }));
            }
            None => {
                let cycle_shown = loop_nodes
                    .iter()
                    .map(|card| format!("[[{card}]]"))
                    .collect::<Vec<_>>()
                    .join(" → ");
                for card in &loop_nodes {
                    if reported.insert(card) {
                        faults.push(json!({
                            "fault": "restatement",
                            "kind": "cycle",
                            "detail": format!(
                                "the 同述入口 sections of {cycle_shown} form a cycle; the statement must be held by a substantive Card"
                            ),
                            "card": card,
                            "terminal": Value::Null,
                            "chain": loop_nodes,
                            "cycle": loop_nodes,
                        }));
                    }
                }
                // A chain that leads into a loop names no terminal either: it is reported as a
                // chain whose `cycle` names the loop it cannot reach past.
                if !loop_nodes.contains(&start) && reported.insert(start) {
                    faults.push(json!({
                        "fault": "restatement",
                        "kind": "chain",
                        "detail": format!(
                            "the 同述入口 section of [[{start}]] reaches the cycle {cycle_shown}; no substantive Card holds that statement"
                        ),
                        "card": start,
                        "terminal": Value::Null,
                        "chain": path,
                        "cycle": loop_nodes,
                    }));
                }
            }
        }
    }
    // Only the entries that name a substantive Card are confirmed, so a chain or a cycle is
    // never served as a valid restatement and never supports folding (§9.2, D087).
    confirmed.retain(|row| {
        row["entry"]
            .as_str()
            .is_some_and(|entry| !reported.contains(entry))
    });
    confirmed.sort_by(|a, b| {
        a["entry"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["entry"].as_str().unwrap_or_default())
    });
    (confirmed, faults)
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

/// The most members one folded row exposes: a bigger confirmed group reports its size and the flag,
/// so one group cannot make a reply unbounded.
pub(crate) const SEARCH_MEMBER_MAX: usize = 200;

/// One confirmed restatement group (§4.5): the Cards that state one claim.
struct Group {
    /// The canonical substantive Card: the terminal an accepted entry names, else the group's
    /// substantive member in natural address order.
    canonical: String,
    /// Every member: the canonical Card first, then natural address order.
    members: Vec<String>,
}

/// What one search read may fold: the confirmed groups, the group of every Card that belongs to
/// one, and how many Cards' exact statement fingerprints the fold compared.
struct Folds {
    groups: Vec<Group>,
    by_member: BTreeMap<String, usize>,
    statements: usize,
}

/// A small disjoint-set over Card addresses, so a restatement group is one component whatever order
/// its evidence was read in. A union keeps the lower address as the root, so a component's identity
/// never depends on that order.
#[derive(Default)]
struct Disjoint {
    parent: BTreeMap<String, String>,
}

impl Disjoint {
    fn add(&mut self, at: &str) {
        self.parent
            .entry(at.to_string())
            .or_insert_with(|| at.to_string());
    }

    fn find(&mut self, at: &str) -> String {
        let mut root = at.to_string();
        while let Some(next) = self.parent.get(&root) {
            if *next == root {
                break;
            }
            root = next.clone();
        }
        let mut cursor = at.to_string();
        while let Some(next) = self.parent.get(&cursor).cloned() {
            if next == root {
                break;
            }
            self.parent.insert(cursor, root.clone());
            cursor = next;
        }
        root
    }

    fn union(&mut self, left: &str, right: &str) {
        let (left_root, right_root) = (self.find(left), self.find(right));
        if left_root == right_root {
            return;
        }
        if crate::model::address_order(&left_root, &right_root) == std::cmp::Ordering::Greater {
            self.parent.insert(left_root, right_root);
        } else {
            self.parent.insert(right_root, left_root);
        }
    }
}

/// Whether a Card states a claim a restatement can hold: a tree root, a Hub Card, a register part
/// and a restatement entry are all navigation, never a statement (§4.5, D086).
fn substantive(snapshot: &Snapshot, address: &str, parts: &BTreeSet<&str>) -> bool {
    let Some(card) = snapshot.by_address.get(address) else {
        return false;
    };
    if crate::is_tree_root(card) || card.card_type == CardType::Hub || parts.contains(address) {
        return false;
    }
    snapshot
        .parsed
        .get(address)
        .is_some_and(|parsed| !is_restatement_entry(&parsed.body))
}

/// Every declared register part in the archive: a part holds term rows, so it is navigation and
/// never a statement (§8.4). Read from the registers themselves, so a Workspace with two registers
/// (a reported fault) still keeps its parts out of a fold.
fn register_parts(snapshot: &Snapshot) -> BTreeSet<String> {
    let mut parts = BTreeSet::new();
    for card in &snapshot.cards {
        if card.hub_level != Some(crate::hub::HubLevel::Register) {
            continue;
        }
        if let Some(parsed) = snapshot.parsed.get(&card.address) {
            parts.extend(crate::hub::register_part_targets(&parsed.body));
        }
    }
    parts
}

/// The `## 同述入口` sections of the whole archive, entry Card → its Links in natural address order,
/// from one pass over the Cards the caller already read. The section is rare, so the cheap substring
/// test is the filter, as it is in the storage read.
pub(crate) fn restatement_sections(snapshot: &Snapshot) -> BTreeMap<String, Vec<String>> {
    let mut sections = BTreeMap::new();
    for (address, parsed) in &snapshot.parsed {
        if !parsed.body.contains(RESTATEMENT_GROUP) {
            continue;
        }
        let links = restatement_links(&parsed.body);
        if !links.is_empty() {
            sections.insert(address.clone(), links);
        }
    }
    sections
}

/// The confirmed restatement groups of one search read (§4.5, §9.2 D209): the connected components
/// of an accepted `## 同述入口` entry and the substantive Card it names, and of two Cards whose exact
/// statement fingerprints are equal.
///
/// Only the Cards the query matched, the confirmed entry partners of those Cards, and the exact
/// statements those Cards state take part: the fold needs no whole-archive hash pass, and a Card the
/// query never matched never takes a matched Card's place. A shared formula, a shared term, a
/// proposed or uncertain merge, or a changed assumption, sign, value, variable, negation or scope
/// never groups two Cards, and no similarity is closed transitively: only a confirmed entry Link or
/// an equal exact fingerprint joins two Cards.
fn fold_groups(
    snapshot: &Snapshot,
    sections: &BTreeMap<String, Vec<String>>,
    candidates: &[Candidate],
) -> Folds {
    let mut folds = Folds {
        groups: Vec::new(),
        by_member: BTreeMap::new(),
        statements: 0,
    };
    if sections.is_empty() && candidates.len() < 2 {
        return folds;
    }
    let parts = register_parts(snapshot);
    let part_refs: BTreeSet<&str> = parts.iter().map(String::as_str).collect();
    let (confirmed, _) = restatement_rows(snapshot, sections, &part_refs);
    let mut nodes: BTreeSet<String> = candidates
        .iter()
        .map(|candidate| candidate.address.clone())
        .collect();
    // A confirmed partner of a matched Card takes part too, so the group exposes every member.
    loop {
        let mut added = false;
        for row in &confirmed {
            let (Some(entry), Some(terminal)) = (row["entry"].as_str(), row["terminal"].as_str())
            else {
                continue;
            };
            if nodes.contains(entry) || nodes.contains(terminal) {
                added |= nodes.insert(entry.to_string());
                added |= nodes.insert(terminal.to_string());
            }
        }
        if !added {
            break;
        }
    }
    if nodes.len() < 2 {
        return folds;
    }
    let mut sets = Disjoint::default();
    for at in &nodes {
        sets.add(at);
    }
    // (a) An accepted entry and the substantive Card it names: authored evidence, never a guess.
    for row in &confirmed {
        let (Some(entry), Some(terminal)) = (row["entry"].as_str(), row["terminal"].as_str()) else {
            continue;
        };
        if nodes.contains(entry) && nodes.contains(terminal) {
            sets.union(entry, terminal);
        }
    }
    // (b) The exact statement fingerprint of a substantive Card. Only equality joins two Cards, so
    // no changed formula, value, sign, variable, negation, assumption or scope is ever folded.
    let mut same: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for at in &nodes {
        if !substantive(snapshot, at, &part_refs) {
            continue;
        }
        let Some(parsed) = snapshot.parsed.get(at) else {
            continue;
        };
        if crate::card::statement_text(&parsed.body).is_empty() {
            continue;
        }
        folds.statements += 1;
        same.entry(crate::card::statement_fingerprint(&parsed.body))
            .or_default()
            .push(at.clone());
    }
    for addresses in same.values() {
        for other in &addresses[1..] {
            sets.union(&addresses[0], other);
        }
    }
    // The terminals: the substantive Cards an accepted entry names. They are the canonical
    // statements, so a reader sees the statement before an entry that only points at it.
    let mut terminals: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in &confirmed {
        let (Some(_), Some(terminal)) = (row["entry"].as_str(), row["terminal"].as_str()) else {
            continue;
        };
        if !nodes.contains(terminal) {
            continue;
        }
        let root = sets.find(terminal);
        terminals.entry(root).or_default().insert(terminal.to_string());
    }
    let mut components: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for at in &nodes {
        let root = sets.find(at);
        components.entry(root).or_default().push(at.clone());
    }
    for (root, members) in components {
        if members.len() < 2 {
            continue;
        }
        let canonical = terminals
            .get(&root)
            .and_then(|terminals| {
                terminals
                    .iter()
                    .min_by(|left, right| crate::model::address_order(left, right))
                    .cloned()
            })
            .or_else(|| {
                members
                    .iter()
                    .find(|at| substantive(snapshot, at, &part_refs))
                    .cloned()
            })
            .unwrap_or_else(|| members[0].clone());
        let mut ordered = vec![canonical.clone()];
        ordered.extend(members.iter().filter(|at| **at != canonical).cloned());
        let index = folds.groups.len();
        folds.groups.push(Group {
            canonical,
            members: ordered,
        });
        let exposed = folds.groups[index].members.clone();
        for at in exposed {
            folds.by_member.insert(at, index);
        }
    }
    folds
}

/// The other matching rows one row replaces, and every member a reader can open (D209, S14-AC03):
/// the row names the canonical substantive Card when it survived the filters, else the matching
/// member the query ranked first.
struct Fold {
    address: String,
    canonical: String,
    folded: usize,
    members: Vec<Value>,
    member_total: usize,
    members_truncated: bool,
}

/// One row of the ranked page, before its JSON is built: a matching Card, or one confirmed
/// restatement group of matching Cards.
struct Planned<'a> {
    carrier: &'a Candidate,
    fold: Option<Fold>,
}

/// One row's view of its confirmed group: the Card it names, the members with their roles, and the
/// matching rows it replaces.
fn fold_of(
    snapshot: &Snapshot,
    args: &SearchArgs,
    group: &Group,
    carrier: &Candidate,
    roles: &BTreeMap<String, &'static str>,
    folded: usize,
) -> Fold {
    let members = group
        .members
        .iter()
        .take(SEARCH_MEMBER_MAX)
        .map(|at| {
            let statement = if snapshot
                .parsed
                .get(at)
                .is_some_and(|parsed| is_restatement_entry(&parsed.body))
            {
                "entry"
            } else {
                "substantive"
            };
            json!({
                "address": at,
                "title": snapshot.title(at),
                "role": roles.get(at),
                "statement": statement,
                "matched": roles.contains_key(at),
                "kept": keeps(snapshot, args, at),
                "canonical": *at == group.canonical,
            })
        })
        .collect::<Vec<_>>();
    Fold {
        address: if keeps(snapshot, args, &group.canonical) {
            group.canonical.clone()
        } else {
            carrier.address.clone()
        },
        canonical: group.canonical.clone(),
        folded,
        members,
        member_total: group.members.len(),
        members_truncated: group.members.len() > SEARCH_MEMBER_MAX,
    }
}

/// The explanation of a folded row: the matching member's own reason, or the member the query
/// matched when the row shows the group's canonical statement in its place.
fn fold_reason(fold: &Fold, carrier: &Candidate) -> String {
    let mut reason = if fold.address == carrier.address {
        carrier.reason.clone()
    } else {
        format!(
            "the query matched [[{}]], which states the same statement as this Card",
            carrier.address
        )
    };
    if fold.folded > 0 {
        reason.push_str(&format!(
            "; {} more matching {} folded here: the same statement",
            fold.folded,
            if fold.folded == 1 {
                "Card is"
            } else {
                "Cards are"
            }
        ));
    }
    reason
}

/// The one entry-first search projection: rows, order, counts, and evidence are the same for the
/// MCP server and the GUI (D193, D194, D198, D278). It is a read-only projection over one Card
/// read: no model run, no Card write, no Link walk, and bounded work reported in `bounds` and
/// `scanned`. An empty query has no ranking; callers keep the plain listing path for it.
pub(crate) fn search(snapshot: &Snapshot, args: &SearchArgs) -> Value {
    let query = Query::new(&args.query);
    let limit = args.limit.clamp(1, SEARCH_LIMIT_MAX);
    let sections = if query.is_empty() {
        BTreeMap::new()
    } else {
        restatement_sections(snapshot)
    };
    let registers = registers(snapshot, &sections);
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
        &sections,
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
    sections: &BTreeMap<String, Vec<String>>,
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
    }

    // The full scoped match set: every matching Card the filters keep, in ranked order and before
    // the page limit. Folding happens here, after the filters and before the limit, so a member a
    // fold hides is neither lost nor counted twice (D209, S14-AC04).
    let survivors = candidates
        .iter()
        .filter(|candidate| keeps(snapshot, args, &candidate.address))
        .collect::<Vec<_>>();
    let total = survivors.len();
    let folds = fold_groups(snapshot, sections, &candidates);
    let roles = candidates
        .iter()
        .map(|candidate| (candidate.address.clone(), candidate.role))
        .collect::<BTreeMap<_, _>>();
    let mut survivors_in: BTreeMap<&str, usize> = BTreeMap::new();
    for candidate in &survivors {
        if let Some(index) = folds.by_member.get(&candidate.address) {
            *survivors_in
                .entry(folds.groups[*index].canonical.as_str())
                .or_insert(0) += 1;
        }
    }
    // One row per surviving Card, except that one confirmed group is one row, placed where its
    // best-ranked member was and naming the canonical statement when that Card survived.
    let mut planned: Vec<Planned> = Vec::new();
    let mut emitted: BTreeSet<&str> = BTreeSet::new();
    for candidate in &survivors {
        let Some(index) = folds.by_member.get(&candidate.address) else {
            planned.push(Planned {
                carrier: candidate,
                fold: None,
            });
            continue;
        };
        let group = &folds.groups[*index];
        if !emitted.insert(group.canonical.as_str()) {
            continue;
        }
        let folded = survivors_in[group.canonical.as_str()] - 1;
        planned.push(Planned {
            carrier: candidate,
            fold: Some(fold_of(snapshot, args, group, candidate, &roles, folded)),
        });
    }
    let row_total = planned.len();
    let results = planned
        .iter()
        .take(limit)
        .filter_map(|plan| row(snapshot, args, query, plan))
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
        "row_total": row_total,
        "folded_total": total.saturating_sub(row_total),
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
            "members_max": SEARCH_MEMBER_MAX,
        },
        "scanned": {
            "registers": registers.views.len(),
            "terms": terms.count,
            "terms_truncated": terms.truncated,
            "hubs": hubs.count,
            "hubs_truncated": hubs.truncated,
            "restatements": sections.len(),
            "statements": folds.statements,
        },
        "archive_fingerprint": snapshot.fingerprint(),
    })
}

/// One row of the entry-first order: the existing Card summary fields, plus the role, the rank
/// tier, the term or line evidence, the explanation, and the navigation Card it came from. A Card in
/// a confirmed restatement group also reports the members its row folds and the canonical Card that
/// holds the statement (§9.2, D207, D209).
fn row(snapshot: &Snapshot, args: &SearchArgs, query: &Query, plan: &Planned) -> Option<Value> {
    let candidate = plan.carrier;
    let address = plan
        .fold
        .as_ref()
        .map_or(candidate.address.as_str(), |fold| fold.address.as_str());
    let card = snapshot.by_address.get(address)?;
    let tree = crate::tree_id(address);
    let body = snapshot
        .parsed
        .get(address)
        .map_or("", |parsed| parsed.body.as_str());
    let body_term = if address == candidate.address {
        candidate.body_term.clone()
    } else {
        None
    }
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
    let reason = match &plan.fold {
        Some(fold) => fold_reason(fold, candidate),
        None => candidate.reason.clone(),
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
        "snippet": snapshot.snippet(address, body_term.as_deref()),
        "role": candidate.role,
        "rank_tier": candidate.tier,
        "matched_term": candidate.matched_term,
        "matched_line": candidate.matched_line,
        "reason": reason,
        "origin": candidate.origin,
        "entry_targets": entry_targets,
        "folded": plan.fold.as_ref().map_or(0, |fold| fold.folded),
        "member_total": plan.fold.as_ref().map_or(1, |fold| fold.member_total),
        "representative": plan.fold.as_ref().map(|fold| fold.canonical.clone()),
        "members": plan.fold.as_ref().map(|fold| fold.members.clone()),
        "members_truncated": plan.fold.as_ref().is_some_and(|fold| fold.members_truncated),
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
    // Restatement entries are search's own evidence (`fold_groups`), not part of this read.
    let sections = BTreeMap::new();
    match register_addresses(snapshot, workspace).as_slice() {
        [at] => Some(read_view(snapshot, at, &sections)),
        _ => None,
    }
}

/// The addresses of one Workspace's registers, in natural address order: none, one, or several (a
/// Workspace has one, and several are reported rather than chosen between).
fn register_addresses(snapshot: &Snapshot, workspace: Workspace) -> Vec<String> {
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
    found
}

/// The most Cards of a Table context one distant selection reads: a Spread holds at most 400.
pub(crate) const DISTANT_TABLE_MAX: usize = 400;
/// The most model heads one distant selection reads, in natural address order.
pub(crate) const DISTANT_MODEL_MAX: usize = 2000;

/// One distant selection request (§10.2, design D249-D256; `/api/distant`, D280): the Card the
/// reader has open or holds, the Cards already on the Table, and — when the reader has selected
/// one — a still-current target of an immutable Bridge report.
#[derive(Clone, Debug, Default)]
pub(crate) struct DistantArgs {
    pub(crate) at: String,
    pub(crate) table: Vec<String>,
    /// The validated output of the Bridge report projection (S20, #112). This pure function takes
    /// that input and opens no report itself; `None` is the ordinary term and model-name evidence.
    pub(crate) report: Option<ReportTarget>,
}

impl DistantArgs {
    pub(crate) fn new(at: &str) -> Self {
        Self {
            at: at.to_string(),
            table: Vec::new(),
            report: None,
        }
    }
}

/// One still-current target of a user-selected immutable Bridge report (§10.3, D251): its report,
/// its exact proposal id, the Card it names, and the Relation kind the proposal states. The Bridge
/// compiler validates the report and its endpoint fingerprints; this selection only orders the
/// target first and never re-judges it (#112 fills this input).
#[derive(Clone, Debug, Default)]
pub(crate) struct ReportTarget {
    pub(crate) report: String,
    pub(crate) proposal: String,
    pub(crate) target: String,
    pub(crate) relation: Option<String>,
}

impl ReportTarget {
    fn json(&self) -> Value {
        json!({
            "report": self.report,
            "proposal": self.proposal,
            "target": self.target,
            "relation": self.relation,
        })
    }
}

/// The candidate sources, in the order the design sets (§10.2, D250): a selected report's target,
/// then a shared accepted register term, then a shared model name.
const RANK_REPORT: u8 = 1;
const RANK_TERM: u8 = 2;
const RANK_MODEL: u8 = 3;

/// One candidate before the cross-source check orders it. `seq` is the stable order inside a rank:
/// the register row's place and the row's Link order, or the model head's place in address order.
struct Offer {
    rank: u8,
    seq: usize,
    address: String,
    term: Option<String>,
    origin: Option<Value>,
    report: Option<ReportTarget>,
    /// The Card's own source evidence (design §4.3, D068-D072).
    sources: Vec<String>,
    /// Whether the offer needs supported different-source evidence. A report target carries the
    /// report's own judged evidence; a shared term or model name must show a source the focus lacks.
    cross_source: bool,
}

impl Offer {
    fn source(&self) -> &'static str {
        match self.rank {
            RANK_REPORT => "bridge_report",
            RANK_TERM => "register_term",
            _ => "model_name",
        }
    }
}

/// Why a Card the terms or model names offered never reached the slot, and the Cards the exclusions
/// removed before the offers were built (AC03, D255).
#[derive(Default)]
struct Exclusions {
    why: BTreeMap<String, &'static str>,
    counts: BTreeMap<&'static str, usize>,
}

impl Exclusions {
    fn add(&mut self, address: Option<String>, why: &'static str) {
        let Some(address) = address.filter(|address| !address.is_empty()) else {
            return;
        };
        if self.why.contains_key(&address) {
            return;
        }
        self.why.insert(address, why);
        self.skip(why);
    }

    fn skip(&mut self, why: &'static str) {
        *self.counts.entry(why).or_insert(0) += 1;
    }

    fn has(&self, address: &str) -> bool {
        self.why.contains_key(address)
    }

    fn json(&self) -> Value {
        Value::Object(
            self.counts
                .iter()
                .map(|(why, count)| ((*why).to_string(), json!(count)))
                .collect(),
        )
    }
}

/// The one distant Card of §10.2: zero or one Card of another book that the open or held Card
/// shares an accepted register term or a model name with, explained by one line of evidence. It is
/// a read projection over the supplied Snapshot: no model runs, no Link is walked, nothing is
/// written — opening, laying, or replying writes no Link, and no viewing telemetry is recorded
/// (S19-AC04, S19-AC06). The same read always gives the same Card; nothing rotates.
pub(crate) fn distant(snapshot: &Snapshot, args: &DistantArgs) -> Result<Value> {
    let focus = snapshot
        .by_address
        .get(args.at.trim())
        .with_context(|| format!("card `{}` does not exist", args.at.trim()))?;
    let address = focus.address.clone();
    let body = snapshot
        .parsed
        .get(&address)
        .map(|parsed| parsed.body.clone())
        .unwrap_or_default();
    let workspace = focus.workspace;
    let registers = register_addresses(snapshot, workspace);
    // One Workspace, one register: two are reported and neither is evidence (D299). Restatement
    // entries are search's own evidence (`fold_groups`), not part of this read.
    let sections = BTreeMap::new();
    let view = match registers.as_slice() {
        [at] => Some(read_view(snapshot, at, &sections)),
        _ => None,
    };
    let mut excluded = Exclusions::default();
    // A bare ambiguous term is no evidence for a confident cross-book suggestion (§12.3, D299):
    // two rows write one label with different destinations, so the label names no agreed sense.
    let ambiguous = view
        .as_ref()
        .map(ambiguous_terms)
        .unwrap_or_default();
    let terms = match &view {
        Some(view) => view.terms_at(snapshot, &address, &body),
        None => Vec::new(),
    }
    .into_iter()
    .filter(|term| {
        if ambiguous.contains(term) {
            excluded.skip("ambiguous_term");
            false
        } else {
            true
        }
    })
    .collect::<Vec<_>>();
    let focus_sources = crate::hub::source_evidence(snapshot, &address).0;

    // The exclusion set of D255: the focus; the Cards already on the Table; its immediate filing
    // neighbours; its existing direct semantic neighbours (either direction); Broken links; the
    // register and its parts; and the Card that holds the same confirmed statement.
    excluded.add(Some(address.clone()), "focus");
    let mut table_truncated = false;
    let mut table = Vec::new();
    for at in &args.table {
        let at = at.trim();
        if at.is_empty() || at.chars().count() > 512 {
            continue;
        }
        if table.len() >= DISTANT_TABLE_MAX {
            table_truncated = true;
            break;
        }
        if !table.iter().any(|seen| seen == at) {
            table.push(at.to_string());
            excluded.add(Some(at.to_string()), "table");
        }
    }
    excluded.add(crate::parent_location(&address), "neighbour");
    excluded.add(
        crate::direct_successor(&address)
            .ok()
            .filter(|at| snapshot.by_address.contains_key(at)),
        "neighbour",
    );
    for side in snapshot.sides(focus) {
        excluded.add(side["address"].as_str().map(str::to_string), "neighbour");
    }
    for link in snapshot.outbound(&address) {
        let why = if link["broken"] == json!(true) {
            "broken"
        } else {
            "linked"
        };
        excluded.add(link["address"].as_str().map(str::to_string), why);
    }
    for source in snapshot.inbound.get(&address).into_iter().flatten() {
        excluded.add(Some(source.clone()), "linked");
    }
    if let Some(view) = &view {
        for at in view.navigation(snapshot) {
            excluded.add(Some(at), "navigation");
        }
    }
    let statement = statement_noun(snapshot, &address);
    if statement != address {
        excluded.add(Some(statement.clone()), "restatement");
    }

    let mut offers = Vec::new();
    // The selected Bridge report's target comes first (§10.2, D251): "still valid" here means the
    // Card exists and passes the same exclusions. The report itself was validated by the Bridge
    // compiler (#112), which is why a report target needs no source check of this read.
    if let Some(report) = &args.report {
        let target = report.target.trim().to_string();
        if !snapshot.by_address.contains_key(&target) {
            excluded.skip("stale_report");
        } else if excluded.has(&target) || statement_noun(snapshot, &target) == statement {
            excluded.skip("report_excluded");
        } else {
            offers.push(Offer {
                rank: RANK_REPORT,
                seq: 0,
                sources: crate::hub::source_evidence(snapshot, &target).0,
                address: target,
                term: None,
                origin: None,
                report: Some(report.clone()),
                cross_source: false,
            });
        }
    }
    let mut entries = 0usize;
    if let Some(view) = &view {
        let mut seq = 0usize;
        for entry in &view.entries {
            entries += 1;
            if !entry.complete() || !terms.iter().any(|term| term == &entry.term) {
                continue;
            }
            for target in &entry.targets {
                seq += 1;
                if !target.exists {
                    excluded.add(Some(target.address.clone()), "missing");
                    continue;
                }
                if excluded.has(&target.address) {
                    continue;
                }
                if statement_noun(snapshot, &target.address) == statement {
                    excluded.skip("restatement");
                    continue;
                }
                offers.push(Offer {
                    rank: RANK_TERM,
                    seq,
                    address: target.address.clone(),
                    term: Some(entry.term.clone()),
                    origin: Some(json!({
                        "card": entry.card,
                        "line": entry.line,
                        "part": entry.part,
                    })),
                    report: None,
                    sources: crate::hub::source_evidence(snapshot, &target.address).0,
                    cross_source: true,
                });
            }
        }
    }
    // Another book's model head with the same scoped model name (§10.2, D253): an accepted label
    // the focus carries, matched whole against the head's own name.
    let mut models = 0usize;
    if !terms.is_empty() {
        let labels = terms.iter().map(String::as_str).collect::<Vec<_>>();
        let index = TermIndex::new(&labels);
        // The register row that declared each accepted label: the slot names it as the term's
        // origin, exactly as a term offer does.
        let origins = match &view {
            Some(view) => view
                .entries
                .iter()
                .filter(|entry| entry.complete())
                .fold(BTreeMap::new(), |mut origins, entry| {
                    origins.entry(entry.term.clone()).or_insert_with(|| {
                        json!({"card": entry.card, "line": entry.line, "part": entry.part})
                    });
                    origins
                }),
            None => BTreeMap::new(),
        };
        let mut heads = snapshot
            .cards
            .iter()
            .filter(|card| card.card_type == CardType::Model && card.facet.is_none())
            .collect::<Vec<_>>();
        heads.sort_by(|left, right| crate::model::address_order(&left.address, &right.address));
        for head in heads {
            if models >= DISTANT_MODEL_MAX {
                break;
            }
            models += 1;
            if excluded.has(&head.address) {
                continue;
            }
            let found = index.occurrences(&snapshot.title(&head.address), "");
            let Some(position) = found.first() else {
                continue;
            };
            if statement_noun(snapshot, &head.address) == statement {
                excluded.skip("restatement");
                continue;
            }
            let term = &terms[*position];
            offers.push(Offer {
                rank: RANK_MODEL,
                seq: models,
                sources: crate::hub::source_evidence(snapshot, &head.address).0,
                address: head.address.clone(),
                term: Some(term.clone()),
                origin: origins.get(term).cloned(),
                report: None,
                cross_source: true,
            });
        }
    }
    let scanned_offers = offers.len();

    let chosen = choose(offers, &focus_sources, &mut excluded);
    let candidate = match &chosen {
        Some(offer) => Some(candidate_json(snapshot, offer, &focus_sources)?),
        None => None,
    };
    let register = match (&view, registers.as_slice()) {
        (Some(view), [at]) => json!({
            "workspace": workspace.id(),
            "card": at,
            "title": snapshot.title(at),
            "missing": false,
            "duplicate": 0,
            "complete": view.faults.is_empty(),
            "entry_total": view.entries.len(),
        }),
        (_, []) => json!({
            "workspace": workspace.id(),
            "card": Value::Null,
            "title": Value::Null,
            "missing": true,
            "duplicate": 0,
            "complete": false,
            "entry_total": 0,
        }),
        (_, many) => json!({
            "workspace": workspace.id(),
            "card": Value::Null,
            "title": Value::Null,
            "missing": false,
            "duplicate": many.len(),
            "complete": false,
            "entry_total": 0,
        }),
    };
    Ok(json!({
        "at": address,
        "title": snapshot.title(&address),
        "workspace": workspace.id(),
        "focus": {"terms": terms, "sources": focus_sources},
        "register": register,
        "candidate": candidate,
        "source": chosen.as_ref().map(Offer::source),
        "excluded": excluded.json(),
        "bounds": {"table_max": DISTANT_TABLE_MAX, "models_max": DISTANT_MODEL_MAX},
        "scanned": {
            "table": table.len(),
            "table_truncated": table_truncated,
            "terms": terms.len(),
            "entries": entries,
            "offers": scanned_offers,
            "models": models,
        },
        "archive_fingerprint": snapshot.fingerprint(),
    }))
}

/// The first offer that passes, in the design's source order and then by its stable order. A term
/// or model-name offer needs supported different-source evidence: an unknown attribution on either
/// side is never a confident cross-book suggestion, and a candidate of the focus's own book is
/// skipped in favour of the next offer (D255, S19-AC02).
fn choose(
    mut offers: Vec<Offer>,
    focus_sources: &[String],
    excluded: &mut Exclusions,
) -> Option<Offer> {
    offers.sort_by(|left, right| {
        left.rank
            .cmp(&right.rank)
            .then(left.seq.cmp(&right.seq))
            .then_with(|| crate::model::address_order(&left.address, &right.address))
    });
    for offer in offers {
        if !offer.cross_source {
            return Some(offer);
        }
        if focus_sources.is_empty() || offer.sources.is_empty() {
            excluded.skip("unknown_source");
            continue;
        }
        if !added_sources(&offer.sources, focus_sources).is_empty() {
            return Some(offer);
        }
        excluded.skip("same_source");
    }
    None
}

/// The sources one Card's evidence adds to another's, in the candidate's order.
fn added_sources(candidate: &[String], focus: &[String]) -> Vec<String> {
    candidate
        .iter()
        .filter(|source| !focus.iter().any(|own| own.eq_ignore_ascii_case(source)))
        .cloned()
        .collect()
}

/// The term labels the register wrote twice with different destinations: one `ambiguous` fault per
/// label, so a distant selection skips the label instead of choosing a sense (D299).
fn ambiguous_terms(view: &View) -> BTreeSet<String> {
    view.faults
        .iter()
        .filter(|fault| fault["fault"] == json!("ambiguous"))
        .filter_map(|fault| fault["term"].as_str().map(str::to_string))
        .collect()
}

/// The substantive Card that holds a Card's statement: its confirmed `## 同述入口` target, else the
/// Card itself (§4.5, D084-D086). One section deep: the register's view resolves a chain, and a
/// distant selection never follows one to justify a suggestion.
fn statement_noun(snapshot: &Snapshot, address: &str) -> String {
    let Some(card) = snapshot.by_address.get(address) else {
        return address.to_string();
    };
    let body = snapshot
        .parsed
        .get(address)
        .map_or("", |parsed| parsed.body.as_str());
    let links = restatement_links(body);
    if links.len() != 1 {
        return address.to_string();
    }
    let target = links[0].as_str();
    let Some(named) = snapshot.by_address.get(target) else {
        return address.to_string();
    };
    if named.workspace != card.workspace
        || crate::is_tree_root(named)
        || named.card_type == CardType::Hub
    {
        return address.to_string();
    }
    target.to_string()
}

/// One candidate as the slot shows it: the Card, the evidence, and one line of explanation. It
/// never claims a relation: a shared term is a way in, not a proven Link (D254, S19-AC01).
fn candidate_json(snapshot: &Snapshot, offer: &Offer, focus_sources: &[String]) -> Result<Value> {
    let card = snapshot
        .by_address
        .get(&offer.address)
        .with_context(|| format!("card `{}` is gone", offer.address))?;
    let tree = crate::tree_id(&offer.address);
    Ok(json!({
        "address": card.address,
        "title": snapshot.title(&card.address),
        "workspace": card.workspace.id(),
        "tree": tree,
        "tree_title": snapshot.title(&crate::tree_root_address(tree)),
        "kind": crate::ops::kind(card),
        "type": card.card_type.id(),
        "level": card.level_id(),
        "form": card.form.map(crate::math::MathForm::id),
        "authored_fingerprint": crate::card::authored_fingerprint(card)?,
        "source": offer.source(),
        "term": offer.term,
        "origin": offer.origin,
        "report": offer.report.as_ref().map(ReportTarget::json),
        "reason": explanation(offer, focus_sources),
        "evidence": {
            "sources": offer.sources,
            "added": added_sources(&offer.sources, focus_sources),
            "focus_sources": focus_sources,
            "mixed": offer.sources.len() > 1 || focus_sources.len() > 1,
        },
    }))
}

/// The one line of evidence the slot shows (D254): the report and proposal of a judged Bridge
/// proposal, or the shared term or model name with the source it adds to the focus. Where either
/// side's evidence is mixed, the line says so: the claim is the added source, not a relation.
fn explanation(offer: &Offer, focus_sources: &[String]) -> String {
    if let Some(report) = &offer.report {
        return match report.relation.as_deref().filter(|relation| !relation.is_empty()) {
            Some(relation) => format!(
                "Bridge report {}, proposal {}: {relation}",
                report.report, report.proposal
            ),
            None => format!("Bridge report {}, proposal {}", report.report, report.proposal),
        };
    }
    let lead = if offer.rank == RANK_TERM {
        "Shares register term"
    } else {
        "Shares the model name"
    };
    let term = offer.term.as_deref().unwrap_or_default();
    let added = added_sources(&offer.sources, focus_sources);
    let first = added.first().cloned().unwrap_or_default();
    let own = if focus_sources.is_empty() {
        "none".to_string()
    } else {
        focus_sources.join(", ")
    };
    if offer.sources.len() > 1 {
        format!(
            "{lead} {term}; its evidence is mixed ({}), and source {first} is absent from this Card's sources ({own})",
            offer.sources.join(", ")
        )
    } else if focus_sources.len() > 1 {
        format!(
            "{lead} {term}; source {first} is absent from this Card's mixed sources ({own})"
        )
    } else {
        format!("{lead} {term}; source {first} is not in this Card's sources ({own})")
    }
}

fn registers(snapshot: &Snapshot, sections: &BTreeMap<String, Vec<String>>) -> Registers {
    let mut views = Vec::new();
    let mut summary = Vec::new();
    let mut faults = Vec::new();
    for workspace in Workspace::ALL {
        let found = register_addresses(snapshot, workspace);
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
                let view = read_view(snapshot, at, sections);
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

/// One register's view from the supplied Snapshot: its term rows, the declared parts, the authored
/// bodies the projection carries, and its Workspace's confirmed restatement entries. The sections
/// are supplied because search already scanned the archive for them (D209).
fn read_view(snapshot: &Snapshot, register: &str, sections: &BTreeMap<String, Vec<String>>) -> View {
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
    // A register reports the entries of its own Workspace: an entry that names a Card of the other
    // Workspace is that Workspace's own `workspace` fault, reported where it was written.
    let workspace = snapshot.workspace(register);
    let own = sections
        .iter()
        .filter(|(card, _)| snapshot.workspace(card) == workspace)
        .map(|(card, links)| (card.clone(), links.clone()))
        .collect::<BTreeMap<_, _>>();
    View::read(snapshot, register, &bodies, &own)
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

/// Whether one authored body holds a `## 同述入口` section (D086): the derived navigation role of a
/// restatement entry. It is derived from the written text on every read, never stored as Card
/// metadata; a heading inside a code fence or a chart is not a section.
pub(crate) fn is_restatement_entry(body: &str) -> bool {
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
        if let Some(heading) = trimmed
            .strip_prefix("## ")
            .or_else(|| trimmed.strip_prefix("### "))
            && heading.trim() == RESTATEMENT_GROUP
        {
            return true;
        }
    }
    false
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

    #[test]
    fn the_entry_role_is_the_section_and_not_the_word() {
        assert!(is_restatement_entry("陈述。\n\n## 同述入口\n\n- [[88/2]] 一处。\n"));
        assert!(is_restatement_entry("## 同述入口\n- [[88/2]] 一处。\n"));
        assert!(!is_restatement_entry("陈述。\n\n这张卡是 [[88/2]] 的同述入口。\n"));
        assert!(!is_restatement_entry("陈述。\n\n```\n## 同述入口\n- [[88/2]]\n```\n"));
        assert!(!is_restatement_entry("## 同述入口之外\n\n- [[88/2]]\n"));
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

    // ---- the distant selection (S19, #108, design §10.2) ----

    /// A scratch archive for the distant selection: `topics` first, then one Literature tree per
    /// `(citation key, first Card text)`, then every `(parent, kind, type, text)` Card, then one
    /// register whose authored body is `body`. `{register}` in a text or row is replaced by the
    /// register's own address (its tree's first Direct successor). Returns the temp dir and that
    /// address.
    fn archive(
        topics: &[&str],
        books: &[(&str, &str)],
        more: &[(&str, crate::ops::Successor, CardType, &str)],
        body: &str,
    ) -> (tempfile::TempDir, String) {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path();
        crate::initialize_database(root).expect("initialize database");
        for text in topics {
            crate::ops::create_topic(root, crate::Access::Owner, Workspace::Agent, text)
                .expect("topic");
        }
        for (key, card) in books {
            crate::ops::create_literature(
                root,
                crate::Access::Owner,
                Workspace::Agent,
                &format!("@book{{{key}, title={{{key} work}}}}"),
                "",
            )
            .expect("literature card");
            crate::ops::create_successor(
                root,
                crate::Access::Owner,
                crate::ops::Successor::Direct,
                key,
                CardType::Idea,
                None,
                card,
            )
            .expect("a card of the book");
        }
        for (parent, kind, card_type, text) in more {
            crate::ops::create_successor(
                root,
                crate::Access::Owner,
                *kind,
                parent,
                *card_type,
                None,
                text,
            )
            .expect("a card");
        }
        let topic =
            crate::ops::create_topic(root, crate::Access::Owner, Workspace::Agent, "# 索引\n")
                .expect("index topic");
        let register = format!("{}/1", crate::tree_id(&topic));
        let register = {
            let made = crate::ops::create_successor(
                root,
                crate::Access::Owner,
                crate::ops::Successor::Direct,
                &topic,
                CardType::Hub,
                None,
                &format!(
                    "# 索引：代理工作区的入口\n\n{}",
                    body.replace("{register}", &register)
                ),
            )
            .expect("register");
            assert_eq!(made, register, "the register sits at its tree's first place");
            made
        };
        crate::ops::edit(
            root,
            crate::Access::Owner,
            &register,
            None,
            crate::ops::TypeChange {
                hub_level: Some(Some(crate::hub::HubLevel::Register)),
                ..Default::default()
            },
            None,
        )
        .expect("register level");
        (temp, register)
    }

    /// Replace one Card's authored text, keeping its generated Reverse section.
    fn edit(root: &std::path::Path, at: &str, text: &str) {
        crate::ops::edit(
            root,
            crate::Access::Owner,
            at,
            Some(text),
            crate::ops::TypeChange::default(),
            None,
        )
        .expect("edit");
    }

    /// Delete one Card, taking the confirmation word the plan returned.
    fn delete(root: &std::path::Path, at: &str) {
        let confirm = match crate::ops::delete(root, crate::Access::Owner, at, None, None)
            .expect("delete plan")
        {
            crate::ops::Outcome::Confirm { confirm, .. } => confirm,
            crate::ops::Outcome::Done(_) => panic!("the delete needed no confirmation"),
        };
        crate::ops::delete(
            root,
            crate::Access::Owner,
            at,
            None,
            Some(confirm.as_str()),
        )
        .expect("delete");
    }

    fn distant_at(snapshot: &Snapshot, at: &str) -> Value {
        distant(snapshot, &DistantArgs::new(at)).expect("a distant read")
    }

    #[test]
    fn distant_offers_a_card_of_another_book_through_a_shared_register_term() {
        let (temp, register) = archive(
            &[],
            &[
                ("bookA", "# 多重均衡的陈述\n\n多重均衡来自策略互补。\n"),
                ("bookB", "# 多重均衡的另一种说法\n\n这里也从策略互补说起。\n"),
            ],
            &[],
            "## 词条\n- 多重均衡：[[bookB/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let value = distant_at(&snapshot, "bookA/1");
        let candidate = &value["candidate"];
        assert_eq!(candidate["address"], json!("bookB/1"), "{value}");
        assert_eq!(candidate["title"], json!("多重均衡的另一种说法"));
        assert_eq!(candidate["workspace"], json!("agent"));
        assert_eq!(candidate["tree"], json!("bookB"));
        assert_eq!(candidate["tree_title"], json!("bookB work"));
        assert_eq!(candidate["source"], json!("register_term"));
        assert_eq!(candidate["term"], json!("多重均衡"));
        assert_eq!(candidate["origin"]["card"], json!(register));
        assert_eq!(
            candidate["origin"]["line"],
            json!(2),
            "the term row is body line 2: {value}"
        );
        assert!(candidate["authored_fingerprint"].is_string());
        assert_eq!(candidate["evidence"]["sources"], json!(["bookB"]));
        assert_eq!(candidate["evidence"]["added"], json!(["bookB"]));
        assert_eq!(candidate["evidence"]["mixed"], json!(false));
        assert_eq!(value["focus"]["terms"], json!(["多重均衡"]));
        assert_eq!(value["focus"]["sources"], json!(["bookA"]));
        let reason = candidate["reason"].as_str().expect("a one-line explanation");
        assert!(
            reason.starts_with("Shares register term 多重均衡; source bookB"),
            "{reason}"
        );
        assert!(
            !reason.contains("proves"),
            "a shared term is not a proven relation: {reason}"
        );
        assert_eq!(value["register"]["card"], json!(register));
        assert_eq!(value["register"]["missing"], json!(false));
        assert_eq!(value["archive_fingerprint"], json!(snapshot.fingerprint()));
        // The suggestion is a read: it wrote no Card, Link, or metadata row.
        assert_eq!(
            Snapshot::read(temp.path()).expect("again").fingerprint(),
            snapshot.fingerprint(),
            "the distant selection changed the archive"
        );
    }

    /// S19-AC03, D255: the focus, the Cards already on the Table, and its immediate filing
    /// neighbours are never the distant Card, and the next valid offer is chosen instead.
    #[test]
    fn distant_excludes_the_table_and_the_immediate_filing_neighbours() {
        let (temp, _) = archive(
            &[],
            &[
                ("bookA", "# 多重均衡的陈述\n\n多重均衡来自策略互补。\n"),
                ("bookB", "# 多重均衡的另一种说法\n\n这里也从策略互补说起。\n"),
            ],
            &[(
                "bookA/1",
                crate::ops::Successor::Side,
                CardType::Idea,
                "# 多重均衡：局部说明\n\n同一机制的局部说明。\n\n来源：[[bookB]]\n",
            )],
            "## 词条\n- 多重均衡：[[bookA/1|a]] [[bookB/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        // The filing neighbour is offered first, but it is the focus's own Side successor.
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(value["candidate"]["address"], json!("bookB/1"), "{value}");
        assert_eq!(value["excluded"]["focus"], json!(1));
        assert_eq!(
            value["excluded"]["neighbour"],
            json!(2),
            "the parent Literature Card and the Side successor: {value}"
        );
        assert_eq!(value["excluded"]["navigation"], json!(1), "{value}");
        // A Card already on the Table is no distant Card either.
        let args = DistantArgs {
            at: "bookA/1".to_string(),
            table: vec!["bookB/1".to_string(), " bookB/1 ".to_string()],
            report: None,
        };
        let value = distant(&snapshot, &args).expect("a distant read");
        assert_eq!(value["candidate"], Value::Null, "{value}");
        assert_eq!(value["excluded"]["table"], json!(1), "one Card, listed twice");
        assert_eq!(value["scanned"]["table"], json!(1));
        assert_eq!(value["source"], Value::Null);
    }

    /// S19-AC03, D255: existing direct semantic neighbours (either direction) and Broken links are
    /// no distant Card, while a later offer of the same term still is.
    #[test]
    fn distant_excludes_the_cards_a_link_already_reaches() {
        let (temp, _) = archive(
            &[],
            &[
                ("bookC", "# 无关的书\n\n与均衡无关。\n"),
                (
                    "bookB",
                    "# 多重均衡的另一种说法\n\n这里也从策略互补说起。\n",
                ),
                (
                    "bookA",
                    "# 多重均衡的陈述\n\n多重均衡来自策略互补。\n\n来源：[[bookA]]\n",
                ),
            ],
            &[
                (
                    "bookB/1",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 多重均衡：反面的说法\n\n也从策略互补说起。\n",
                ),
                (
                    "bookB/1",
                    crate::ops::Successor::Side,
                    CardType::Idea,
                    "# 逆向选择\n\n[[bookA/1]] 讨论了多重均衡。\n",
                ),
            ],
            "## 词条\n- 多重均衡：[[bookB/1]] [[bookB/1|a]] [[bookB/2]]\n",
        );
        edit(
            temp.path(),
            "bookA/1",
            "# 多重均衡的陈述\n\n多重均衡来自策略互补。\n\n来源：[[bookA]]\n\n[[bookB/2]] 是同一机制的另一面，[[bookC/1]] 也已引用。\n",
        );
        delete(temp.path(), "bookC/1");
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(value["candidate"]["address"], json!("bookB/1"), "{value}");
        assert_eq!(
            value["excluded"]["linked"],
            json!(2),
            "the Card the focus links and the Card that links the focus: {value}"
        );
        assert_eq!(
            value["excluded"]["broken"],
            json!(1),
            "the Broken link of the focus: {value}"
        );
        let args = DistantArgs {
            at: "bookA/1".to_string(),
            table: vec!["bookB/1".to_string()],
            report: None,
        };
        let value = distant(&snapshot, &args).expect("a distant read");
        assert_eq!(value["candidate"], Value::Null, "{value}");
    }

    /// S19-AC03, D255: two Cards that name the same confirmed statement are the same statement,
    /// even when neither Links the other.
    #[test]
    fn distant_excludes_a_second_entry_of_the_same_confirmed_statement() {
        let (temp, register) = archive(
            &[],
            &[
                (
                    "bookB",
                    "# 多重均衡的完整陈述\n\n多重均衡来自策略互补。\n\n来源：[[bookB]]\n",
                ),
                (
                    "bookA",
                    "# 多重均衡：本书的入口\n\n多重均衡来自策略互补。\n\n来源：[[bookA]]\n\n## 同述入口\n\n- [[bookB/1]] 完整陈述。\n",
                ),
                (
                    "bookC",
                    "# 多重均衡：另一本书的入口\n\n多重均衡来自策略互补。\n\n来源：[[bookC]]\n\n## 同述入口\n\n- [[bookB/1]] 完整陈述。\n",
                ),
            ],
            &[(
                "bookC/1",
                crate::ops::Successor::Direct,
                CardType::Idea,
                "# 策略互补的收益\n\n多重均衡来自策略互补。\n\n来源：[[bookC]]\n",
            )],
            "## 词条\n- 多重均衡：[[bookC/1]] [[bookC/2]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(
            value["candidate"]["address"],
            json!("bookC/2"),
            "the second book's entry names the same statement: {value}"
        );
        assert_eq!(value["candidate"]["origin"]["card"], json!(register));
        assert_eq!(
            value["excluded"]["restatement"],
            json!(1),
            "bookC/1 is the same statement as the focus: {value}"
        );
        let args = DistantArgs {
            at: "bookA/1".to_string(),
            table: vec!["bookC/2".to_string()],
            report: None,
        };
        let value = distant(&snapshot, &args).expect("a distant read");
        assert_eq!(
            value["candidate"], Value::Null,
            "only the same statement is left: {value}"
        );
    }

    /// S19-AC03, D255: the register and its parts are navigation, never a distant Card.
    #[test]
    fn distant_excludes_the_register_and_its_parts() {
        let (temp, register) = archive(
            &[],
            &[
                (
                    "bookB",
                    "# 多重均衡的另一种说法\n\n这里也从策略互补说起。\n",
                ),
                (
                    "bookA",
                    "# 多重均衡的陈述\n\n多重均衡来自策略互补。\n",
                ),
            ],
            &[],
            "## 词条\n- 多重均衡：[[bookB/1]]\n",
        );
        // A declared register part holds rows of its own; the register declares it afterwards,
        // because a Link to a Card needs that Card to exist.
        let part = crate::ops::create_successor(
            temp.path(),
            crate::Access::Owner,
            crate::ops::Successor::Side,
            &register,
            CardType::Idea,
            None,
            "# 词条 A–F\n\n## 词条\n- 学习：[[bookB/1]]\n",
        )
        .expect("register part");
        assert_eq!(part, format!("{register}|a"));
        edit(
            temp.path(),
            &register,
            &format!(
                "# 索引：代理工作区的入口\n\n## 词条\n- 多重均衡：[[{part}]]\n\n## 词条分册\n- [[{part}]] A–F\n"
            ),
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(value["candidate"], Value::Null, "{value}");
        assert_eq!(
            value["excluded"]["navigation"],
            json!(2),
            "the register and its declared part: {value}"
        );
        assert_eq!(value["register"]["complete"], json!(true));
        assert_eq!(value["register"]["entry_total"], json!(2));
    }

    /// S19-AC02, D255: a shared term needs supported different-source evidence. Unknown
    /// attribution is never a cross-book suggestion, a candidate of the focus's own book is no
    /// suggestion, and mixed evidence names the source it adds.
    #[test]
    fn distant_requires_supported_different_source_evidence() {
        // The candidate's evidence is mixed and adds `bookB`, which the focus does not name.
        let (temp, _) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 多重均衡的陈述\n\n多重均衡来自策略互补。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookB",
                    "# 多重均衡的另一种说法\n\n这里也从策略互补说起。\n\n来源：[[bookA]] [[bookB]]\n",
                ),
            ],
            &[],
            "## 词条\n- 多重均衡：[[bookB/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(value["candidate"]["address"], json!("bookB/1"), "{value}");
        assert_eq!(
            value["candidate"]["evidence"]["sources"],
            json!(["bookA", "bookB"])
        );
        assert_eq!(value["candidate"]["evidence"]["added"], json!(["bookB"]));
        assert_eq!(value["candidate"]["evidence"]["mixed"], json!(true));
        let reason = value["candidate"]["reason"].as_str().expect("a reason");
        assert!(
            reason.contains("its evidence is mixed (bookA, bookB)"),
            "{reason}"
        );
        assert!(reason.contains("source bookB is absent"), "{reason}");

        // A focus whose own evidence is unknown is never a confident cross-book suggestion.
        let (temp, _) = archive(
            &["# 杂项\n"],
            &[(
                "bookB",
                "# 多重均衡的另一种说法\n\n这里也从策略互补说起。\n",
            )],
            &[(
                "0/0",
                crate::ops::Successor::Direct,
                CardType::Idea,
                "# 多重均衡的假设\n\n多重均衡来自策略互补。\n",
            )],
            "## 词条\n- 多重均衡：[[bookB/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let value = distant_at(&snapshot, "0/1");
        assert_eq!(value["candidate"], Value::Null, "{value}");
        assert_eq!(value["focus"]["terms"], json!(["多重均衡"]));
        assert_eq!(value["focus"]["sources"], json!([]));
        assert_eq!(value["excluded"]["unknown_source"], json!(1), "{value}");

        // A candidate of the focus's own book is no suggestion either.
        let (temp, _) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 多重均衡的陈述\n\n多重均衡来自策略互补。\n\n来源：[[bookA]]\n",
                ),
                ("bookB", "# 另一件事\n\n与均衡无关。\n"),
            ],
            &[
                (
                    "bookA/1",
                    crate::ops::Successor::Side,
                    CardType::Idea,
                    "# 多重均衡：同书另一支\n\n同一本书里的另一支。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/1|a",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 多重均衡：同书另一处\n\n同一本书里的另一处。\n\n来源：[[bookA]]\n",
                ),
            ],
            "## 词条\n- 多重均衡：[[bookA/1|a|1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(value["candidate"], Value::Null, "{value}");
        assert_eq!(value["excluded"]["same_source"], json!(1), "{value}");
    }

    /// S19-AC01, D250-D253: the selected report's target comes first, then a shared accepted
    /// register term, then a model head with the same scoped model name.
    #[test]
    fn distant_prefers_the_report_then_the_term_then_the_model_name() {
        let (temp, _) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 多重均衡的陈述\n\n多重均衡来自策略互补。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookB",
                    "# 多重均衡的另一种说法\n\n这里也从策略互补说起。\n",
                ),
            ],
            &[],
            "## 词条\n- 多重均衡：[[bookB/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        // A still-current target of the report the reader selected is the first source.
        let mut args = DistantArgs::new("bookA/1");
        args.report = Some(ReportTarget {
            report: "bridge-2026-10-19".to_string(),
            proposal: "b003".to_string(),
            target: "bookB/1".to_string(),
            relation: Some("contrasts".to_string()),
        });
        let value = distant(&snapshot, &args).expect("a distant read");
        assert_eq!(value["candidate"]["source"], json!("bridge_report"), "{value}");
        assert_eq!(value["candidate"]["address"], json!("bookB/1"));
        assert_eq!(value["candidate"]["report"]["proposal"], json!("b003"));
        assert_eq!(
            value["candidate"]["reason"],
            json!("Bridge report bridge-2026-10-19, proposal b003: contrasts")
        );
        // A stale target falls back to the term evidence and says why it was dropped.
        let mut args = DistantArgs::new("bookA/1");
        args.report = Some(ReportTarget {
            report: "bridge-2026-10-19".to_string(),
            proposal: "b004".to_string(),
            target: "bookB/9".to_string(),
            relation: Some("contrasts".to_string()),
        });
        let value = distant(&snapshot, &args).expect("a distant read");
        assert_eq!(value["candidate"]["source"], json!("register_term"), "{value}");
        assert_eq!(value["excluded"]["stale_report"], json!(1), "{value}");
        // A report target already on the Table or already reached by a Link is no suggestion.
        let mut args = DistantArgs::new("bookA/1");
        args.table = vec!["bookB/1".to_string()];
        args.report = Some(ReportTarget {
            report: "bridge-2026-10-19".to_string(),
            proposal: "b005".to_string(),
            target: "bookB/1".to_string(),
            relation: None,
        });
        let value = distant(&snapshot, &args).expect("a distant read");
        assert_eq!(value["candidate"], Value::Null, "{value}");

        // A model head of another book with the same scoped model name is the third source.
        let (temp, register) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 学习（个体行为）的陈述\n\n学习（个体行为）提高技能。\n\n来源：[[bookA]]\n",
                ),
                ("bookB", "# 另一件事\n\n与学习无关。\n"),
            ],
            &[
                (
                    "bookA/1",
                    crate::ops::Successor::Side,
                    CardType::Idea,
                    "# 学习（个体行为）：同书的另一处\n\n学习（个体行为）提高技能。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookB/1",
                    crate::ops::Successor::Side,
                    CardType::Model,
                    "# 学习（个体行为）模型\n\n技能存量决定产量。\n\n来源：[[bookB]]\n",
                ),
            ],
            "## 词条\n- 学习（个体行为）：[[bookA/1|a]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(value["candidate"]["address"], json!("bookB/1|a"), "{value}");
        assert_eq!(value["candidate"]["source"], json!("model_name"));
        assert_eq!(value["candidate"]["term"], json!("学习（个体行为）"));
        let reason = value["candidate"]["reason"].as_str().expect("a reason");
        assert!(
            reason.starts_with("Shares the model name 学习（个体行为）; source bookB"),
            "{reason}"
        );
        assert_eq!(value["candidate"]["origin"]["card"], json!(register));
    }

    /// S19-AC04, S19-AC07: zero or one, the same answer twice, and nothing when the evidence is
    /// gone — a missing Card is an error, an unshared term an empty slot.
    #[test]
    fn distant_returns_one_stable_card_or_none() {
        let (temp, _) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 多重均衡的陈述\n\n多重均衡来自策略互补。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookB",
                    "# 多重均衡的另一种说法\n\n这里也从策略互补说起。\n",
                ),
                ("bookC", "# 无关的书\n\n与均衡无关。\n"),
            ],
            &[(
                "bookB/1",
                crate::ops::Successor::Direct,
                CardType::Idea,
                "# 学习的另一面\n\n技能存量。\n\n来源：[[bookB]]\n",
            )],
            "## 词条\n- 多重均衡：[[bookB/1]]\n- 学习：[[bookB/2]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        assert_eq!(
            distant_at(&snapshot, "bookA/1"),
            distant_at(&snapshot, "bookA/1"),
            "the same read gives the same Card: it never rotates"
        );
        // The focus states only 多重均衡, so the 学习 entry is no evidence.
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(value["scanned"]["offers"], json!(1), "{value}");
        // A Card with no accepted term it states has no distant Card at all.
        let value = distant_at(&snapshot, "bookC/1");
        assert_eq!(value["candidate"], Value::Null, "{value}");
        assert_eq!(value["focus"]["terms"], json!([]));
        // Evidence that disappears leaves the slot empty, never a substitute.
        edit(
            temp.path(),
            "bookA/1",
            "# 无关的陈述\n\n这一张卡不谈那个术语。\n\n来源：[[bookA]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("a fresh read");
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(value["candidate"], Value::Null, "{value}");
        assert_eq!(value["focus"]["terms"], json!([]));
        // A Card that does not exist is an error, not an empty slot.
        let error = distant(&snapshot, &DistantArgs::new("bookZ/9")).expect_err("no such Card");
        assert_eq!(error.to_string(), "card `bookZ/9` does not exist");
    }

    /// S19-AC01, D196: a term a Card only cites on a source line or lists in a member group is not
    /// the Card's own statement, so it is no evidence for a distant Card.
    #[test]
    fn distant_reads_a_term_from_substance_not_from_a_source_line_or_a_member_list() {
        let (temp, _) = archive(
            &[],
            &[
                (
                    "bookB",
                    "# 多重均衡的另一种说法\n\n这里也从策略互补说起。\n",
                ),
                (
                    "bookA",
                    "# 一张提到别处的卡\n\n本卡讨论自己的问题。\n\n来源：《多重均衡手册》[[bookB]]\n\n## 邻近\n\n- [[bookB/1]] 讨论多重均衡。\n",
                ),
            ],
            &[],
            "## 词条\n- 多重均衡：[[bookB/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(value["candidate"], Value::Null, "{value}");
        assert_eq!(
            value["focus"]["terms"],
            json!([]),
            "a source line and a member group are no statement: {value}"
        );
        assert_eq!(value["scanned"]["offers"], json!(0));
        // A substantive sentence of the same Card is evidence again.
        edit(
            temp.path(),
            "bookA/1",
            "# 一张提到别处的卡\n\n多重均衡来自策略互补。\n\n来源：[[bookA]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("a fresh read");
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(value["focus"]["terms"], json!(["多重均衡"]), "{value}");
        assert_eq!(value["candidate"]["address"], json!("bookB/1"), "{value}");
    }

    /// S19-AC01, S19-AC02, D299: one label written with two different destinations is ambiguous,
    /// so it never becomes a confident cross-book suggestion.
    #[test]
    fn distant_refuses_a_bare_ambiguous_term() {
        let (temp, _) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 学习的陈述\n\n学习提高生产率。\n\n来源：[[bookA]]\n",
                ),
                ("bookB", "# 学习的另一面\n\n学习提高技能。\n"),
                ("bookC", "# 学习的第三面\n\n学习改变习惯。\n"),
            ],
            &[],
            "## 词条\n- 学习：[[bookB/1]]\n- 学习：[[bookC/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let value = distant_at(&snapshot, "bookA/1");
        assert_eq!(
            value["candidate"], Value::Null,
            "the label names two senses: {value}"
        );
        assert_eq!(value["focus"]["terms"], json!([]), "{value}");
        assert_eq!(value["excluded"]["ambiguous_term"], json!(1), "{value}");
        assert_eq!(value["register"]["complete"], json!(false));
    }

    /// One candidate row for an address, as the ranking hands it to the fold.
    fn candidate(address: &str) -> Candidate {
        Candidate {
            tier: TIER_BODY,
            quality: Quality::Partial,
            score: 0,
            address: address.to_string(),
            role: "hit",
            field: "body",
            matched_term: None,
            matched_line: None,
            reason: String::new(),
            origin: None,
            entry_targets: Vec::new(),
            body_term: None,
        }
    }

    /// The fold unions in any read order to one component, and an exact statement, a confirmed entry
    /// and an entry's partner are the only evidence that groups two Cards (§4.5, S14-AC02).
    #[test]
    fn a_fold_groups_an_entry_its_terminal_and_an_exact_duplicate_only() {
        let (temp, _register) = archive(
            &[
                "# 甲树\n\n补偿需求项的树。",
                "# 乙树\n\n补偿需求项的树。",
            ],
            &[],
            &[
                (
                    "1/0",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 甲的陈述\n\n补偿需求项决定就业。",
                ),
                (
                    "1/1",
                    crate::ops::Successor::Side,
                    CardType::Idea,
                    "# 乙\n\n补偿需求项决定就业。",
                ),
                (
                    "1/1",
                    crate::ops::Successor::Side,
                    CardType::Idea,
                    "# 甲的入口\n\n补偿需求项见甲。\n\n## 同述入口\n\n- [[1/1]] 完整陈述。",
                ),
                (
                    "1/1",
                    crate::ops::Successor::Side,
                    CardType::Idea,
                    "# 丙\n\n补偿需求项不决定就业。",
                ),
                (
                    "1/1",
                    crate::ops::Successor::Side,
                    CardType::Hub,
                    "# 枢纽一\n\n- [[1/1]]：补偿需求项。",
                ),
                (
                    "1/1",
                    crate::ops::Successor::Side,
                    CardType::Hub,
                    "# 枢纽二\n\n- [[1/1]]：补偿需求项。",
                ),
            ],
            "## 词条\n- 补偿需求项：[[1/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let sections = restatement_sections(&snapshot);
        let addresses = [
            "1/1", "1/1|a", "1/1|b", "1/1|c", "1/1|d", "1/1|e", "1/0", "2/0",
        ];
        let candidates = addresses.iter().map(|at| candidate(at)).collect::<Vec<_>>();
        let folds = fold_groups(&snapshot, &sections, &candidates);
        // The terminal, its exact duplicate and the entry that names it are one statement.
        let group = folds.by_member.get("1/1").expect("the statement's group");
        assert_eq!(folds.groups[*group].canonical, "1/1");
        assert_eq!(
            folds.groups[*group].members,
            vec!["1/1", "1/1|a", "1/1|b"]
        );
        assert_eq!(folds.by_member.get("1/1|a"), Some(group));
        assert_eq!(folds.by_member.get("1/1|b"), Some(group));
        // A different statement, two identical Hub bodies and two identical tree roots never fold.
        for at in ["1/1|c", "1/1|d", "1/1|e", "1/0", "2/0"] {
            assert!(
                !folds.by_member.contains_key(at),
                "{at} is not a statement: {:?}",
                folds.by_member.keys().collect::<Vec<_>>()
            );
        }
        assert_eq!(
            folds.statements, 3,
            "the statement, its duplicate and the different claim; hubs, roots and the entry hold none"
        );
    }

    /// The union order never changes the group, and the lowest address is its root.
    #[test]
    fn a_fold_set_unions_in_any_order_to_one_component() {
        let mut left = Disjoint::default();
        for at in ["1/2", "1/1", "logical"] {
            left.add(at);
        }
        left.union("1/2", "1/1");
        left.union("logical", "1/2");
        let mut right = Disjoint::default();
        for at in ["logical", "1/1", "1/2"] {
            right.add(at);
        }
        right.union("logical", "1/2");
        right.union("1/2", "1/1");
        assert_eq!(left.find("logical"), right.find("logical"));
        assert_eq!(left.find("1/1"), "1/1", "the lowest address is the root");
        assert_eq!(left.find("logical"), "1/1");
    }

    /// A group larger than the member bound reports its size and the flag, so one reply stays
    /// bounded whatever the archive holds.
    #[test]
    fn a_fold_row_exposes_at_most_the_member_bound() {
        let (temp, _) = archive(&["# 树\n\n问题。"], &[], &[], "");
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let mut members = vec!["1/1".to_string()];
        for index in 0..SEARCH_MEMBER_MAX + 4 {
            members.push(format!("9/{index}"));
        }
        let group = Group {
            canonical: "1/1".to_string(),
            members: members.clone(),
        };
        let args = SearchArgs {
            query: "问题".to_string(),
            workspace: None,
            card_type: None,
            form: None,
            limit: 10,
        };
        let fold = fold_of(
            &snapshot,
            &args,
            &group,
            &candidate("1/1"),
            &BTreeMap::new(),
            members.len() - 1,
        );
        assert_eq!(fold.members.len(), SEARCH_MEMBER_MAX);
        assert_eq!(fold.member_total, members.len());
        assert!(fold.members_truncated);
        assert_eq!(fold.folded, members.len() - 1);
        assert_eq!(fold.address, "1/1");
    }
}
