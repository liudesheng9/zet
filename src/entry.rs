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
    // (b) The exact statement fingerprint of an Idea Card. Only equality joins two Cards, so no
    // changed formula, value, sign, variable, negation, assumption or scope is ever folded.
    //
    // Only an Idea Card states one claim in its own words, which is the claim a fold may hide. A
    // Model part's meaning is also its model and facet, a Finding Card's is also its labelled lines
    // and source, and an Event Card's is also what happened, in what place; the fingerprint ignores
    // a separately labelled source line on purpose, so two such Cards with equal text are not a
    // demonstrated restatement. The live archive holds 143 Model parts whose body is a placeholder,
    // and folding them would hide 142 distinct facet titles from a search.
    let mut same: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for at in &nodes {
        if !substantive(snapshot, at, &part_refs) {
            continue;
        }
        let Some(card) = snapshot.by_address.get(at) else {
            continue;
        };
        if card.card_type != CardType::Idea {
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
    let mut value = json!({
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
    });
    // A search row is a Card projection, so it carries the same Lock facts as the Card it opens
    // (R208). Its selection and ranking rules are unchanged.
    snapshot.decorate(&mut value, card.row_id);
    Some(value)
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
/// Workspace has one, and several are reported rather than chosen between). The draft precheck of
/// a field hub (`src/hub.rs::precheck`) reads the same list, so both say a Workspace's register is
/// one Card or none.
pub(crate) fn register_addresses(snapshot: &Snapshot, workspace: Workspace) -> Vec<String> {
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

/// The most report proposals one distant selection reads: the Bridge projection's own bound
/// (§12.2, D293).
pub(crate) const DISTANT_REPORT_MAX: usize = 20;

/// One distant selection request (§10.2, design D249-D256; `/api/distant`, D280): the Card the
/// reader has open or holds, the Cards already on the Table, and — when the reader has selected
/// one — the still-current proposals of an immutable Bridge report.
#[derive(Clone, Debug, Default)]
pub(crate) struct DistantArgs {
    pub(crate) at: String,
    pub(crate) table: Vec<String>,
    /// The validated current proposals of the reader's selected Bridge report, in report order
    /// (S20, #112, design §10.3, D251). This pure function takes that projection as input and
    /// opens no report itself; an empty list is the ordinary term and model-name evidence.
    pub(crate) reports: Vec<ReportTarget>,
}

impl DistantArgs {
    pub(crate) fn new(at: &str) -> Self {
        Self {
            at: at.to_string(),
            table: Vec::new(),
            reports: Vec::new(),
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
    // The selected Bridge report's targets come first (§10.2, D251): "still valid" here means the
    // Card exists and passes the same exclusions. The report itself was validated by the Bridge
    // compiler (#112), which is why a report target needs no source check of this read. The
    // proposals are tried in report order, so the first one the exclusions leave answers.
    let mut reports = 0usize;
    for (n, report) in args.reports.iter().take(DISTANT_REPORT_MAX).enumerate() {
        reports += 1;
        let target = report.target.trim().to_string();
        if !snapshot.by_address.contains_key(&target) {
            excluded.skip("stale_report");
        } else if excluded.has(&target) || statement_noun(snapshot, &target) == statement {
            excluded.skip("report_excluded");
        } else {
            offers.push(Offer {
                rank: RANK_REPORT,
                seq: n,
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
        "bounds": {"table_max": DISTANT_TABLE_MAX, "models_max": DISTANT_MODEL_MAX, "reports_max": DISTANT_REPORT_MAX},
        "scanned": {
            "table": table.len(),
            "table_truncated": table_truncated,
            "terms": terms.len(),
            "entries": entries,
            "offers": scanned_offers,
            "models": models,
            "reports": reports,
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

// ---- the question path (S15, #114, design §9.3-§9.5) ----

/// The longest question the question path accepts, in Unicode characters (D228).
pub(crate) const ASK_QUESTION_MAX_CHARS: usize = 512;
/// The fewest Cards a reader may ask for (D228).
pub(crate) const ASK_LIMIT_MIN: usize = 5;
/// The most Cards a reader may ask for (D218, D228).
pub(crate) const ASK_LIMIT_MAX: usize = 9;
/// The selection size when the reader names none (D228).
pub(crate) const ASK_LIMIT_DEFAULT: usize = 7;

/// One question request (§9.5, D226-D228; MCP `ask_box`, GUI `POST /api/ask`): the question, the
/// Workspace filter (`all`, the default, is both Workspaces), and how many Cards the reader asked
/// for. The MCP server and the GUI build it from the same JSON, so the two transports accept and
/// refuse exactly the same requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AskArgs {
    pub(crate) question: String,
    pub(crate) workspace: Option<Workspace>,
    pub(crate) limit: usize,
}

impl AskArgs {
    /// `question` (required, nonblank, at most 512 characters), `workspace`
    /// (`human`/`agent`/`all`, default `all`), and `limit` (5 to 9, default 7; a size outside the
    /// range is clamped to it, so one reply stays bounded).
    pub(crate) fn from_args(args: &Value) -> Result<Self> {
        let question = args
            .get("question")
            .and_then(Value::as_str)
            .context("missing string argument `question`")?
            .trim()
            .to_string();
        if question.is_empty() {
            bail!("argument `question` must be a nonblank question");
        }
        if question.chars().count() > ASK_QUESTION_MAX_CHARS {
            bail!("argument `question` is longer than {ASK_QUESTION_MAX_CHARS} characters");
        }
        let workspace = match args.get("workspace") {
            None | Some(Value::Null) => None,
            Some(Value::String(value))
                if value.trim().is_empty() || value.trim().eq_ignore_ascii_case("all") =>
            {
                None
            }
            Some(Value::String(value)) => Some(Workspace::parse(value)?),
            Some(_) => bail!("argument `workspace` must be a string"),
        };
        let limit = match args.get("limit") {
            None | Some(Value::Null) => ASK_LIMIT_DEFAULT,
            Some(value) => {
                let asked = value
                    .as_u64()
                    .context("argument `limit` must be a whole number of 5 to 9")?;
                (asked as usize).clamp(ASK_LIMIT_MIN, ASK_LIMIT_MAX)
            }
        };
        Ok(Self {
            question,
            workspace,
            limit,
        })
    }
}

/// The most accepted terms one question takes (D213, D291).
pub(crate) const ASK_TERM_MAX: usize = 6;
/// The most distinct entry seeds one question takes (D213, D217).
pub(crate) const ASK_SEED_MAX: usize = 12;
/// The deepest semantic step count from a seed (D215, D217).
pub(crate) const ASK_DEPTH_MAX: usize = 2;
/// The most distinct substantive Cards one question visits (D217, D291).
pub(crate) const ASK_CARD_MAX: usize = 200;
/// The most authored semantic Links one question examines (D217, D291).
pub(crate) const ASK_LINK_MAX: usize = 1000;
/// The longest display name a proposed Spread carries: the Spread schema's own name limit, so the
/// full question stays in the response and in `spread.question` (D232).
const ASK_NAME_MAX: usize = 80;

/// The fixed bounds one question reports, so a reader can tell a bounded proposal from a whole
/// archive search (D217, D291, S15-AC05).
pub(crate) fn ask_bounds() -> Value {
    json!({
        "terms": ASK_TERM_MAX,
        "seeds": ASK_SEED_MAX,
        "depth": ASK_DEPTH_MAX,
        "cards": ASK_CARD_MAX,
        "links": ASK_LINK_MAX,
        "limit_min": ASK_LIMIT_MIN,
        "limit_max": ASK_LIMIT_MAX,
        "question_max_chars": ASK_QUESTION_MAX_CHARS,
        "name_max_chars": ASK_NAME_MAX,
    })
}

/// One accepted label the question states, with the register rows that write it.
struct QuestionTerm {
    term: String,
    /// The first occurrence in the normalized question, counted in characters.
    at: usize,
    /// The origin of each row that writes the label, in register order.
    origins: Vec<Value>,
    /// Every target of those rows, in register order, repeats kept (a seed is taken once).
    targets: Vec<String>,
}

/// One entry seed: a target of a matched term, or a fallback Card, with the evidence that reached
/// it.
struct Seed {
    address: String,
    /// The accepted term that reached the Card; `None` for a bigram fallback seed.
    term: Option<String>,
    origin: Value,
    /// How many character bigrams a fallback seed shares with the question.
    bigrams: usize,
    /// The matched term's place in evidence order.
    term_index: usize,
    /// The Hub Card whose member line supplied this entry, when one did (D216).
    via_hub: Option<String>,
}

/// The normalized text one question is matched in: case folded and whitespace collapsed, exactly
/// as D212 states, so an accepted label is found as a substring of the question and not only as a
/// whole word.
fn folded_question(question: &str) -> String {
    let mut folded = String::with_capacity(question.len());
    let mut space = false;
    for ch in question.chars() {
        if ch.is_whitespace() {
            space = true;
            continue;
        }
        if space && !folded.is_empty() {
            folded.push(' ');
        }
        space = false;
        folded.extend(ch.to_lowercase());
    }
    folded
}

/// One accepted label of the supplied registers and the rows that write it, in register order.
struct AcceptedLabel {
    label: String,
    folded: String,
    /// The label's place among the supplied rows, so a tie is broken by the register's own order.
    order: usize,
    origins: Vec<Value>,
    targets: Vec<String>,
}

/// The base of an explicitly scoped label such as `学习（产业生产率）`: the text before its
/// parenthesis (D212). A bare match of that base names every accepted sense rather than one.
fn scoped_base(label: &str) -> Option<&str> {
    let at = label.find(['（', '('])?;
    let base = label[..at].trim();
    (!base.is_empty()).then_some(base)
}

/// The accepted labels of the supplied registers, one per label, in register order: every complete
/// row's term with the Cards it names. An incomplete row is no evidence (D297).
fn accepted_labels(views: &[View]) -> Vec<AcceptedLabel> {
    let mut labels: Vec<AcceptedLabel> = Vec::new();
    let mut order = 0usize;
    for view in views {
        for entry in view.entries.iter().filter(|entry| entry.complete()) {
            let label = entry.term.trim();
            if label.is_empty() {
                continue;
            }
            order += 1;
            let origin = json!({"card": entry.card, "line": entry.line, "part": entry.part});
            let targets = entry
                .targets
                .iter()
                .filter(|target| target.exists)
                .map(|target| target.address.clone())
                .collect::<Vec<_>>();
            match labels.iter_mut().find(|known| known.label == label) {
                Some(known) => {
                    known.origins.push(origin);
                    for target in targets {
                        if !known.targets.contains(&target) {
                            known.targets.push(target);
                        }
                    }
                }
                None => labels.push(AcceptedLabel {
                    label: label.to_string(),
                    folded: label.to_lowercase(),
                    order,
                    origins: vec![origin],
                    targets,
                }),
            }
        }
    }
    labels
}

/// The accepted labels the question states, and the bare words whose senses it does not distinguish
/// (D212, D213). The rules, in the design's order:
///
/// - an accepted label matches when its folded text is a substring of the folded question;
/// - a matched label contained in another matched label is that longer concept's word, so only the
///   longer label matches; two labels that overlap without containing each other both match;
/// - a bare base a scoped label states (`学习` for `学习（个体行为）`) matches every accepted sense
///   when the question does not state any of those scoped labels in full, and names itself
///   ambiguous when more than one sense is accepted;
/// - the labels are ordered by first occurrence, then by the order the register writes them.
fn question_terms(views: &[View], folded: &str) -> (Vec<QuestionTerm>, Vec<String>) {
    let labels = accepted_labels(views);
    let position = |folded_label: &str| {
        folded
            .find(folded_label)
            .map(|byte| folded[..byte].chars().count())
    };
    // Direct occurrences, before the sense expansion: a label contained in a longer one the
    // question also states is not a second concept.
    let mut direct = labels
        .iter()
        .enumerate()
        .filter_map(|(index, label)| position(&label.folded).map(|at| (index, at)))
        .collect::<Vec<_>>();
    let all = direct.clone();
    direct.retain(|(index, _)| {
        !all.iter().any(|(other, _)| {
            other != index
                && labels[*other].folded != labels[*index].folded
                && labels[*other].folded.contains(&labels[*index].folded)
        })
    });
    let mut matched = direct.clone();
    let mut ambiguous = Vec::new();
    // The senses of every bare base the question states, when it states none of them in full.
    for label in &labels {
        let Some(base) = scoped_base(&label.label) else {
            continue;
        };
        let folded_base = base.to_lowercase();
        let Some(base_at) = position(&folded_base) else {
            continue;
        };
        let senses = labels
            .iter()
            .enumerate()
            .filter(|(_, sense)| scoped_base(&sense.label) == Some(base))
            .collect::<Vec<_>>();
        let stated = senses
            .iter()
            .any(|(_, sense)| position(&sense.folded).is_some());
        if stated {
            continue;
        }
        for (index, _) in &senses {
            if !matched.iter().any(|(known, _)| known == index) {
                matched.push((*index, base_at));
            }
        }
        if senses.len() > 1 && !ambiguous.iter().any(|term| term == base) {
            ambiguous.push(base.to_string());
        }
    }
    matched.sort_by_key(|(index, at)| (*at, labels[*index].order));
    let terms = matched
        .into_iter()
        .map(|(index, at)| {
            let label = &labels[index];
            QuestionTerm {
                term: label.label.clone(),
                at,
                origins: label.origins.clone(),
                targets: label.targets.clone(),
            }
        })
        .collect();
    (terms, ambiguous)
}

/// The fewest character bigrams a title must share with the question to be a fallback seed: one
/// shared pair is too little evidence to justify a Card (D212, S15-AC03).
const FALLBACK_BIGRAM_MIN: usize = 2;

/// The characters a bigram overlap compares: case-folded letters and digits. Whitespace and
/// punctuation carry no meaning of their own in a question, so they are left out of both sides.
fn overlap_text(text: &str) -> String {
    text.chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The distinct character bigrams of one text, in first-occurrence order.
fn bigrams(text: &str) -> Vec<(char, char)> {
    let folded = overlap_text(text);
    let chars = folded.chars().collect::<Vec<_>>();
    let mut pairs = Vec::new();
    for window in chars.windows(2) {
        let pair = (window[0], window[1]);
        if !pairs.contains(&pair) {
            pairs.push(pair);
        }
    }
    pairs
}

/// Whether a Card belongs to the question's Workspace filter (`all` keeps both Workspaces).
fn workspace_kept(snapshot: &Snapshot, workspace: Option<Workspace>, address: &str) -> bool {
    workspace.is_none_or(|asked| snapshot.workspace(address) == Some(asked))
}

/// The deterministic fallback seeds of a question that names no accepted term (§9.3, D212): the
/// substantive Cards whose title or model name shares at least [`FALLBACK_BIGRAM_MIN`] character
/// bigrams with the question, best overlap first, then in natural address order. Hubs, tree roots
/// and the register's own navigation are ways in, never entries, so they never seed a question; the
/// Workspace filter applies here as it does to an entry row's targets.
fn fallback_seeds(
    snapshot: &Snapshot,
    views: &[View],
    question: &str,
    workspace: Option<Workspace>,
) -> Vec<(String, usize)> {
    let asked = bigrams(question);
    if asked.len() < FALLBACK_BIGRAM_MIN {
        return Vec::new();
    }
    let navigation = views
        .iter()
        .flat_map(|view| view.navigation(snapshot))
        .collect::<BTreeSet<String>>();
    let mut seeds = snapshot
        .cards
        .iter()
        .filter(|card| !crate::is_tree_root(card))
        .filter(|card| card.card_type != CardType::Hub)
        .filter(|card| !navigation.contains(&card.address))
        .filter(|card| workspace_kept(snapshot, workspace, &card.address))
        .filter_map(|card| {
            let title = &snapshot.parsed.get(&card.address)?.title;
            let score = bigrams(title)
                .into_iter()
                .filter(|pair| asked.contains(pair))
                .count();
            (score >= FALLBACK_BIGRAM_MIN).then(|| (card.address.clone(), score))
        })
        .collect::<Vec<_>>();
    seeds.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| crate::model::address_order(&left.0, &right.0))
    });
    seeds
}

/// The archived display name of a question: the Spread schema's name limit is 80 characters, so
/// the name is cut (never the question itself).
fn display_name(question: &str) -> String {
    let one_line = question.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = one_line.chars().take(ASK_NAME_MAX).collect::<String>();
    if name.is_empty() {
        "Ask".to_string()
    } else {
        name
    }
}

/// What one Card of the archive is for a question (§9.4, D216): substance the answer may use, or
/// one of the ways in that the walk must not turn into a semantic step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    Substantive,
    /// The register, its declared parts, and an undeclared Side successor that holds term rows.
    Navigation,
    /// A Hub Card: its member lines are entries, never semantic Links.
    Hub,
    /// A Literature tree root: the source root that would join every Card of its book.
    Root,
    /// A Structure Card: it orders one book's argument, so its list is not a semantic step.
    Structure,
}

impl Role {
    /// Why this Card was not used, as the exclusion count names it.
    fn excluded(self) -> &'static str {
        match self {
            Role::Navigation => "navigation",
            Role::Hub => "hub",
            Role::Root => "source_root",
            Role::Structure => "structure",
            Role::Substantive => "substantive",
        }
    }
}

/// Whether an authored body is a Structure Card: the first non-blank line is the book's own marker,
/// which orders the argument rather than making a claim (`本卡是《…》的结构卡`, §9.4).
fn is_structure_card(body: &str) -> bool {
    let Some(first) = body.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return false;
    };
    first.starts_with("本卡是《") && first.contains("》的结构卡")
}

/// One Card's authored Links, split by what they are: the semantic Links a walk may follow, and the
/// occurrences that are a way in or a locator rather than a claim (§9.4, D216).
#[derive(Clone, Default)]
struct LinkScan {
    links: Vec<crate::model::LinkRead>,
    /// Links on a `来源`/`出处`/`Source` line: where the Card got its material.
    source: usize,
    /// Links in a navigation section (`邻近`, `入口`, `词条`, `词条分册`, `同述入口`).
    navigation: usize,
    /// Links in fenced code or a chart: not text the Card claims.
    fenced: usize,
}

impl LinkScan {
    /// The occurrences that are not semantic steps, each named with the count it contributes.
    fn skipped(&self) -> Vec<(&'static str, usize)> {
        [
            ("source_locator", self.source),
            ("navigation", self.navigation),
            ("fenced", self.fenced),
        ]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .collect()
    }
}

/// The authored Link occurrences of one line.
fn occurrences(line: &str) -> usize {
    crate::link::LinkBody::for_rendering(line).occurrences().len()
}

/// Read one authored body into its semantic Links and the Link occurrences that are not steps. The
/// generated Reverse section is not part of an authored body at all, so it is never read here. A
/// Hub Card, a Structure Card and a Literature source root have no semantic Link: a hub's list is a
/// way in, a structure Card's list is the book's order, and a source root would join a whole book.
/// A Topic Card is substance (a standalone Topic claim), so its prose Links are read like any other
/// Card's.
fn scan_links(snapshot: &Snapshot, address: &str) -> LinkScan {
    let mut scan = LinkScan::default();
    let Some(parsed) = snapshot.parsed.get(address) else {
        return scan;
    };
    if snapshot.by_address.get(address).is_none_or(|card| {
        card.card_type == CardType::Hub || (crate::is_tree_root(card) && card.is_lit)
    }) || is_structure_card(&parsed.body)
    {
        return scan;
    }
    let mut kept = String::with_capacity(parsed.body.len());
    let mut navigation = false;
    let mut fenced = false;
    for line in parsed.body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            scan.fenced += occurrences(line);
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
        // A source locator line is a source wherever it stands, inside a section or not.
        if crate::hub::is_source_line(trimmed) {
            scan.source += occurrences(line);
            continue;
        }
        if navigation {
            scan.navigation += occurrences(line);
            continue;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    // A Term use is a word reference, not a step of an argument (S16, #158; design 10.2): the
    // walk and the spread's threads read ordinary Links only.
    scan.links = crate::model::read_links(&kept, |target| snapshot.title(target))
        .into_iter()
        .filter(|read| !read.kind.is_term())
        .collect();
    scan
}

/// The role of one Card in a question's read: the navigation Cards of the registers in scope, the
/// Hub Cards, the source roots and the Structure Cards are ways in, never steps.
///
/// A **Literature** tree root is the source root of D216: it would join every Card of its book, so
/// the walk never uses it. A **Topic** Card is different: the register may name one as "a
/// standalone Topic claim" (`CONTEXT.md`, Register), so it is substance that a question may hold
/// and walk from.
fn role_of(snapshot: &Snapshot, navigation: &BTreeSet<String>, address: &str) -> Option<Role> {
    let card = snapshot.by_address.get(address)?;
    if navigation.contains(address) {
        return Some(Role::Navigation);
    }
    if card.card_type == CardType::Hub {
        return Some(Role::Hub);
    }
    if crate::is_tree_root(card) && card.is_lit {
        return Some(Role::Root);
    }
    let body = snapshot
        .parsed
        .get(address)
        .map_or("", |parsed| parsed.body.as_str());
    if is_structure_card(body) {
        return Some(Role::Structure);
    }
    Some(Role::Substantive)
}

/// One authored semantic Link of a Card: its target, Relation kind and sentence. A generated
/// Reverse link is not in the authored body; a source locator line, a navigation section, a fenced
/// code block and a Hub or Structure Card's member list are no semantic step (§9.4, D216).
fn semantic_links(snapshot: &Snapshot, address: &str) -> Vec<crate::model::LinkRead> {
    scan_links(snapshot, address).links
}

/// One step of a walk: the authored Link's two ends, the direction the walk took, its Relation kind,
/// and the sentence its author wrote.
fn step(from: &str, to: &str, direction: &str, link: &crate::model::LinkRead) -> Value {
    json!({
        "from": from,
        "to": to,
        "direction": direction,
        "kind": link.kind.id(),
        "sentence": link.note,
    })
}

/// One Card the walk reached, with the exact path that justifies it.
#[derive(Clone)]
struct Reached {
    address: String,
    /// The entry seed this Card was reached from (itself, at depth 0).
    seed: String,
    term: Option<String>,
    /// The register row that accepted the term, and the Hub Card whose member line supplied the
    /// entry when one did.
    origin: Value,
    via_hub: Option<String>,
    /// The matched term's place in evidence order, so a stronger term orders a frontier first.
    term_index: usize,
    /// The term evidence of the Card's own title and prose, or a fallback seed's bigram overlap:
    /// the lexical relevance that orders comparably relevant Cards.
    lexical: usize,
    /// How many character bigrams a fallback seed shares with the question.
    bigrams: usize,
    depth: usize,
    path: Vec<Value>,
}

/// What one bounded walk read, so the reply reports the actual work and where a bound stopped it.
#[derive(Default)]
struct Walk {
    reached: Vec<Reached>,
    excluded: BTreeMap<&'static str, usize>,
    links: usize,
    cards_truncated: bool,
    links_truncated: bool,
}

impl Walk {
    fn skip(&mut self, why: &'static str) {
        *self.excluded.entry(why).or_insert(0) += 1;
    }

    fn skip_count(&mut self, why: &'static str, count: usize) {
        *self.excluded.entry(why).or_insert(0) += count;
    }
}

/// How many accepted terms one Card's own title and prose state: the lexical evidence that orders a
/// frontier, so a Card that states the question's terms is walked before one that a Link merely
/// arrived at.
fn term_hits(snapshot: &Snapshot, labels: &[&str], address: &str) -> usize {
    if labels.is_empty() {
        return 0;
    }
    let Some(card) = snapshot.by_address.get(address) else {
        return 0;
    };
    let body = snapshot
        .parsed
        .get(address)
        .map_or("", |parsed| parsed.body.as_str());
    let text = format!("{}\n{}", snapshot.title(address), prose(body)).to_lowercase();
    labels
        .iter()
        .filter(|label| text.contains(label.to_lowercase().as_str()))
        .count()
        + usize::from(card.card_type == CardType::Model)
}

/// The bounded semantic walk from the entry seeds (§9.4, D215-D217): two steps in either direction,
/// at most [`ASK_CARD_MAX`] distinct substantive Cards and [`ASK_LINK_MAX`] examined authored
/// semantic Links, each frontier in a stable order. A Hub Card reached from a seed supplies its
/// members as entries; a Hub, tree root, navigation Card or Structure Card reached later is a way
/// in that the walk never passes through.
fn walk(
    snapshot: &Snapshot,
    views: &[View],
    seeds: &[Seed],
    workspace: Option<Workspace>,
) -> Walk {
    let navigation = views
        .iter()
        .flat_map(|view| view.navigation(snapshot))
        .collect::<BTreeSet<String>>();
    let labels = views
        .iter()
        .flat_map(|view| view.entries.iter())
        .filter(|entry| entry.complete())
        .map(|entry| entry.term.trim())
        .filter(|label| !label.is_empty())
        .collect::<Vec<_>>();
    let mut read: BTreeMap<String, LinkScan> = BTreeMap::new();
    let mut walk = Walk::default();
    let mut visited: BTreeSet<String> = BTreeSet::new();
    for seed in seeds {
        visited.insert(seed.address.clone());
        walk.reached.push(Reached {
            address: seed.address.clone(),
            seed: seed.address.clone(),
            term: seed.term.clone(),
            origin: seed.origin.clone(),
            via_hub: seed.via_hub.clone(),
            term_index: seed.term_index,
            lexical: term_hits(snapshot, &labels, &seed.address),
            bigrams: seed.bigrams,
            depth: 0,
            path: Vec::new(),
        });
    }
    let mut frontier = (0..walk.reached.len()).collect::<Vec<usize>>();
    for depth in 1..=ASK_DEPTH_MAX {
        let mut ordered = frontier.clone();
        ordered.sort_by(|left, right| {
            let (left, right) = (&walk.reached[*left], &walk.reached[*right]);
            left.term_index
                .cmp(&right.term_index)
                .then_with(|| right.lexical.cmp(&left.lexical))
                .then_with(|| crate::model::address_order(&left.address, &right.address))
        });
        let mut next = Vec::new();
        'frontier: for index in ordered {
            let node = walk.reached[index].clone();
            // The Card's own semantic Links, then the Cards that link to it. An inbound Link keeps
            // its own source as `from`, so its kind is never read as the node's own claim.
            let scan = scan_cached(snapshot, &node.address, &mut read);
            // The occurrences this Card's own body files as a way in or a locator: counted, never
            // followed (D216).
            for (why, count) in scan.skipped() {
                walk.skip_count(why, count);
            }
            let mut steps = Vec::new();
            for link in &scan.links {
                if link.target != node.address {
                    steps.push((link.target.clone(), "out", link.clone()));
                }
            }
            let sources = snapshot
                .inbound
                .get(&node.address)
                .into_iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>();
            for source in sources {
                if let Some(link) = scan_cached(snapshot, &source, &mut read)
                    .links
                    .into_iter()
                    .find(|link| link.target == node.address)
                {
                    steps.push((source, "in", link));
                }
            }
            for (neighbour, direction, link) in steps {
                if walk.links >= ASK_LINK_MAX {
                    walk.links_truncated = true;
                    break 'frontier;
                }
                walk.links += 1;
                // The Workspace filter selects which Cards the proposal may hold; a Link that
                // crosses it is still a way in, never a reason to include the other side.
                if !workspace_kept(snapshot, workspace, &neighbour) {
                    walk.skip("workspace");
                    continue;
                }
                let Some(role) = role_of(snapshot, &navigation, &neighbour) else {
                    walk.skip("broken");
                    continue;
                };
                if role != Role::Substantive {
                    walk.skip(role.excluded());
                    continue;
                }
                if visited.contains(&neighbour) {
                    continue;
                }
                if walk.reached.len() >= ASK_CARD_MAX {
                    walk.cards_truncated = true;
                    break 'frontier;
                }
                visited.insert(neighbour.clone());
                let (from, to) = if direction == "out" {
                    (node.address.as_str(), neighbour.as_str())
                } else {
                    (neighbour.as_str(), node.address.as_str())
                };
                let mut path = node.path.clone();
                path.push(step(from, to, direction, &link));
                let lexical = term_hits(snapshot, &labels, &neighbour);
                walk.reached.push(Reached {
                    address: neighbour,
                    seed: node.seed.clone(),
                    term: node.term.clone(),
                    origin: node.origin.clone(),
                    via_hub: node.via_hub.clone(),
                    term_index: node.term_index,
                    lexical,
                    bigrams: node.bigrams,
                    depth,
                    path,
                });
                next.push(walk.reached.len() - 1);
            }
        }
        if walk.links_truncated || walk.cards_truncated || next.is_empty() {
            break;
        }
        frontier = next;
    }
    walk
}

/// One Card's authored Link read, made once per walk.
fn scan_cached(
    snapshot: &Snapshot,
    address: &str,
    read: &mut BTreeMap<String, LinkScan>,
) -> LinkScan {
    read.entry(address.to_string())
        .or_insert_with(|| scan_links(snapshot, address))
        .clone()
}

/// The greedy selection of one question's Table (D218-D224, §9.4): the best entry Card is held,
/// and each further Card is the best remaining one by the design's ordered preference.
///
/// 1. a Card whose statement is not yet on the Table beats one that restates a selected statement
///    (D220); this is a preference, never a filter, so a duplicate is still chosen when nothing
///    else is left;
/// 2. a directly relevant Card beats a weaker step, and a shorter justified path beats a longer
///    one (D221);
/// 3. among comparably relevant Cards, one that adds a source, a tree or a Relation kind the Table
///    does not hold yet wins (D222), because diversity is a tie preference among useful Cards;
/// 4. the last tie is natural address order (D223).
fn select(snapshot: &Snapshot, reached: &[Reached], limit: usize) -> Vec<usize> {
    let mut selected = Vec::new();
    if reached.is_empty() || limit == 0 {
        return selected;
    }
    let mut sources: BTreeSet<String> = BTreeSet::new();
    let mut trees: BTreeSet<String> = BTreeSet::new();
    let mut kinds: BTreeSet<String> = BTreeSet::new();
    let mut statements: BTreeSet<String> = BTreeSet::new();
    // The best entry Card is the first seed the evidence order reached.
    selected.push(0);
    let absorb = |index: usize,
                  sources: &mut BTreeSet<String>,
                  trees: &mut BTreeSet<String>,
                  kinds: &mut BTreeSet<String>,
                  statements: &mut BTreeSet<String>| {
        let card = &reached[index];
        sources.extend(crate::hub::source_evidence(snapshot, &card.address).0);
        trees.insert(crate::tree_id(&card.address).to_string());
        if let Some(kind) = card.path.last().and_then(|step| step["kind"].as_str()) {
            kinds.insert(kind.to_string());
        }
        statements.insert(statement_noun(snapshot, &card.address));
    };
    absorb(0, &mut sources, &mut trees, &mut kinds, &mut statements);
    while selected.len() < limit {
        // The design's ordered preference is one key: a duplicate statement, then depth, then the
        // matched term's place, then the Card's own term evidence, then the novelty it adds, then
        // the address order.
        type Preference = (bool, usize, usize, std::cmp::Reverse<usize>, std::cmp::Reverse<usize>);
        let mut best: Option<(Preference, String)> = None;
        for (index, card) in reached.iter().enumerate() {
            if selected.contains(&index) {
                continue;
            }
            let statement = statement_noun(snapshot, &card.address);
            let duplicate = statements.contains(&statement);
            // What this Card would add to the Table: its sources, its tree, and the Relation kind
            // of the step that reached it.
            let mut novelty = 0;
            if crate::hub::source_evidence(snapshot, &card.address)
                .0
                .iter()
                .any(|source| !sources.contains(source))
            {
                novelty += 1;
            }
            if !trees.contains(crate::tree_id(&card.address)) {
                novelty += 1;
            }
            if card
                .path
                .last()
                .and_then(|step| step["kind"].as_str())
                .is_some_and(|kind| !kinds.contains(kind))
            {
                novelty += 1;
            }
            let key = (
                duplicate,
                card.depth,
                card.term_index,
                std::cmp::Reverse(card.lexical),
                std::cmp::Reverse(novelty),
            );
            let better = match &best {
                None => true,
                Some(known) => {
                    key.cmp(&known.0).then_with(|| {
                        crate::model::address_order(&card.address, &known.1)
                    }) == std::cmp::Ordering::Less
                }
            };
            if better {
                best = Some((key, card.address.clone()));
            }
        }
        let Some((_, address)) = best else {
            break;
        };
        let index = reached
            .iter()
            .position(|card| card.address == address)
            .expect("the chosen Card was reached");
        selected.push(index);
        absorb(index, &mut sources, &mut trees, &mut kinds, &mut statements);
    }
    selected
}

/// The unsaved Spread a question proposes (§9.3-§9.5, D211-D231): the question's accepted terms,
/// their bounded entry seeds, a semantic Link walk from those seeds, and the greedy selection of up
/// to `limit` explained Cards that holds the best entry Card. It is one pure function over the
/// supplied read: it starts no model, writes no Card, and returns a proposal the caller may keep or
/// discard. `revision` and `identity` are the read's own archive evidence, which the adapter read in
/// the same transaction as the Snapshot.
pub(crate) fn gather(snapshot: &Snapshot, args: &AskArgs, revision: u64, identity: &Value) -> Value {
    let sections = BTreeMap::new();
    let all = registers(snapshot, &sections);
    let views = all
        .views
        .into_iter()
        .filter(|view| {
            args.workspace
                .is_none_or(|workspace| snapshot.workspace(&view.register) == Some(workspace))
        })
        .collect::<Vec<_>>();
    let folded = folded_question(&args.question);
    let (terms, ambiguous) = question_terms(&views, &folded);
    let terms_total = terms.len();
    let terms = terms.into_iter().take(ASK_TERM_MAX).collect::<Vec<_>>();

    // The entries of every matched term, in evidence order. A term whose row names a Hub Card
    // supplies that hub's members as entries (D216); a tree root, the register's own navigation and
    // a Structure Card are ways in, so neither becomes an entry (D216, S15-AC04).
    let navigation = views
        .iter()
        .flat_map(|view| view.navigation(snapshot))
        .collect::<BTreeSet<String>>();
    let mut seeds: Vec<Seed> = Vec::new();
    let mut excluded: BTreeMap<&'static str, usize> = BTreeMap::new();
    let push = |seeds: &mut Vec<Seed>, address: String, term: Option<String>, origin: Value| {
        if seeds.iter().any(|seed| seed.address == address) {
            return;
        }
        seeds.push(Seed {
            address,
            term,
            origin,
            bigrams: 0,
            term_index: 0,
            via_hub: None,
        });
    };
    for (term_index, term) in terms.iter().enumerate() {
        for target in &term.targets {
            if !workspace_kept(snapshot, args.workspace, target) {
                *excluded.entry("workspace").or_insert(0) += 1;
                continue;
            }
            let Some(role) = role_of(snapshot, &navigation, target) else {
                *excluded.entry("missing").or_insert(0) += 1;
                continue;
            };
            match role {
                Role::Hub => {
                    let body = snapshot
                        .parsed
                        .get(target)
                        .map_or(String::new(), |parsed| parsed.body.clone());
                    for line in crate::hub::parse_body(&body).lines {
                        if role_of(snapshot, &navigation, &line.target) == Some(Role::Substantive) {
                            let address = line.target.clone();
                            push(&mut seeds, address, Some(term.term.clone()), term.origins.first().cloned().unwrap_or(Value::Null));
                            if let Some(seed) = seeds.last_mut() {
                                seed.via_hub = Some(target.clone());
                                seed.term_index = term_index;
                            }
                        } else {
                            *excluded.entry("hub_member").or_insert(0) += 1;
                        }
                    }
                }
                Role::Substantive => {
                    push(&mut seeds, target.clone(), Some(term.term.clone()), term.origins.first().cloned().unwrap_or(Value::Null));
                    if let Some(seed) = seeds.last_mut() {
                        seed.term_index = term_index;
                    }
                }
                other => *excluded.entry(other.excluded()).or_insert(0) += 1,
            }
        }
    }
    // No accepted label matches the question: a small deterministic bigram overlap against titles
    // and model names seeds it instead, with a low-confidence explanation (D212, S15-AC03).
    let mut fallback = Value::Null;
    if terms.is_empty() {
        let candidates = fallback_seeds(snapshot, &views, &args.question, args.workspace);
        fallback = json!({
            "kind": "bigram",
            "confidence": "low",
            "min_bigrams": FALLBACK_BIGRAM_MIN,
            "explanation": "No accepted register term matches the question. These Cards share character bigrams with its title only, so the match is low confidence; reformulate the question or name the term you mean.",
            "matched": candidates
                .iter()
                .take(ASK_SEED_MAX)
                .map(|(address, score)| json!({"at": address, "bigrams": score}))
                .collect::<Vec<_>>(),
        });
        for (address, score) in candidates {
            if seeds.iter().any(|seed| seed.address == address) {
                continue;
            }
            seeds.push(Seed {
                address,
                term: None,
                origin: Value::Null,
                bigrams: score,
                term_index: usize::MAX,
                via_hub: None,
            });
        }
    }
    let seeds_total = seeds.len();
    seeds.truncate(ASK_SEED_MAX);

    let mut walk = walk(snapshot, &views, &seeds, args.workspace);
    for (why, count) in excluded {
        *walk.excluded.entry(why).or_insert(0) += count;
    }
    let held = seeds.first().map(|seed| seed.address.clone());
    let reached = walk.reached.clone();
    let chosen = select(snapshot, &reached, args.limit);
    let mut selected: Vec<Value> = Vec::new();
    let mut reasons: Vec<Value> = Vec::new();
    for index in chosen {
        let reached = &reached[index];
        let at = &reached.address;
        let reason = match (&reached.term, reached.depth) {
            (Some(term), 0) => format!("States the accepted term {term}."),
            (Some(term), _) => format!(
                "Reached in {} semantic step(s) from the entry Card for {term}.",
                reached.depth
            ),
            (None, _) => format!(
                "Shares {} character bigrams with the question's title words; the match is low confidence.",
                reached.bigrams
            ),
        };
        selected.push(json!({
            "at": at,
            "held": held.as_deref() == Some(at.as_str()),
            "title": snapshot.title(at),
            "workspace": snapshot.workspace(at).map(Workspace::id),
            "tree": crate::tree_id(at),
            "type": snapshot.by_address.get(at).map(|card| card.card_type.id()),
            "term": reached.term,
            "origin": reached.origin,
            "via_hub": reached.via_hub,
            "depth": reached.depth,
            "seed": reached.seed,
            "path": reached.path,
            "statement": statement_noun(snapshot, at),
            "reason": reason,
            "authored_fingerprint": snapshot
                .by_address
                .get(at)
                .map(crate::card::authored_fingerprint)
                .transpose()
                .unwrap_or(None),
        }));
        reasons.push(json!({"at": at, "term": reached.term, "path": reached.path}));
    }
    for (index, card) in selected.iter_mut().enumerate() {
        let (x, y) = grid_position(index);
        card["position"] = json!({"x": x, "y": y});
    }
    let spread_cards = selected
        .iter()
        .map(|card| {
            json!({
                "at": card["at"],
                "x": card["position"]["x"],
                "y": card["position"]["y"],
            })
        })
        .collect::<Vec<_>>();
    let threads = threads(snapshot, &selected);
    let name = display_name(&args.question);
    let truncated =
        terms_total > terms.len() || seeds_total > seeds.len() || walk.cards_truncated || walk.links_truncated;
    // The registers this read used, and every fault that kept an entry out of it: a missing
    // register, a duplicate register, an incomplete row or part (§9.1, D296-D299, S15-AC09).
    let in_scope = |workspace: &str| args.workspace.is_none_or(|asked| asked.id() == workspace);
    let registers = all
        .summary
        .iter()
        .filter(|row| row["workspace"].as_str().is_some_and(in_scope))
        .cloned()
        .collect::<Vec<_>>();
    let faults = all
        .faults
        .iter()
        .filter(|fault| {
            fault["workspace"]
                .as_str()
                .is_none_or(|workspace| in_scope(workspace))
        })
        .cloned()
        .collect::<Vec<_>>();
    // A Workspace in scope with no register has no accepted term at all: the reply says so rather
    // than leaving the reader to infer it from an empty term list (S15-AC09).
    let mut faults = faults;
    for row in &registers {
        if row["missing"] != json!(true) {
            continue;
        }
        let workspace = row["workspace"].as_str().unwrap_or_default();
        faults.push(json!({
            "fault": "register_missing",
            "workspace": workspace,
            "detail": format!(
                "the {workspace} Workspace has no Hub Card of level register; there are no accepted terms to match"
            ),
        }));
    }
    json!({
        "question": args.question,
        "workspace": args.workspace.map_or("all", Workspace::id),
        "limit": args.limit,
        "matched_terms": terms.iter().map(|term| term.term.clone()).collect::<Vec<_>>(),
        "terms": terms
            .iter()
            .map(|term| json!({
                "term": term.term,
                "at": term.at,
                "origins": term.origins,
                "targets": term.targets,
            }))
            .collect::<Vec<_>>(),
        "ambiguous_terms": ambiguous,
        "fallback": fallback,
        "registers": registers,
        "faults": faults,
        "held": held,
        "spread": {
            "name": name,
            "question": args.question,
            "cards": spread_cards,
        },
        "cards": selected,
        "reasons": reasons,
        "threads": threads,
        "excluded": Value::Object(
            walk.excluded
                .iter()
                .map(|(why, count)| ((*why).to_string(), json!(count)))
                .collect(),
        ),
        "sparse": selected.len() < ASK_LIMIT_MIN,
        "truncated": truncated,
        "counts": {
            "terms": terms_total,
            "terms_dropped": terms_total - terms.len(),
            "seeds": seeds_total,
            "seeds_dropped": seeds_total - seeds.len(),
            "depth": ASK_DEPTH_MAX,
            "visited": walk.reached.len(),
            "links": walk.links,
            "candidates": walk.reached.len(),
            "selected": selected.len(),
            "cards_truncated": walk.cards_truncated,
            "links_truncated": walk.links_truncated,
        },
        "bounds": ask_bounds(),
        "archive_revision": revision,
        "archive_identity": identity,
    })
}

/// The authored Links between two selected Cards: each is one thread, named by its own Relation
/// kind and drawn from the Card that wrote it, so the Table never shows an inferred relation
/// (§9.4, D225, S15-AC08). A `refers` Link stays labelled `refers`.
fn threads(snapshot: &Snapshot, selected: &[Value]) -> Vec<Value> {
    let chosen = selected
        .iter()
        .filter_map(|card| card["at"].as_str())
        .collect::<BTreeSet<_>>();
    let mut found = Vec::new();
    for from in &chosen {
        for link in semantic_links(snapshot, from) {
            if link.target == *from || !chosen.contains(link.target.as_str()) {
                continue;
            }
            found.push(step(from, &link.target, "out", &link));
        }
    }
    found
}

/// The stable suggested place of the `index`th selected Card, in the GUI Table's own Card size and
/// gap: a grid in selection order, so the same question always lays out the same way.
fn grid_position(index: usize) -> (i64, i64) {
    const WIDTH: i64 = 236 + 30;
    const HEIGHT: i64 = 150 + 30;
    let per_row = 4i64;
    ((index as i64 % per_row) * WIDTH, (index as i64 / per_row) * HEIGHT)
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
                .expect("index topic")
                .0;
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
            .expect("register")
            .0;
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
            reports: Vec::new(),
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
            reports: Vec::new(),
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
            reports: Vec::new(),
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
        .expect("register part")
        .0;
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
        args.reports = vec![ReportTarget {
            report: "bridge-2026-10-19".to_string(),
            proposal: "b003".to_string(),
            target: "bookB/1".to_string(),
            relation: Some("contrasts".to_string()),
        }];
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
        args.reports = vec![ReportTarget {
            report: "bridge-2026-10-19".to_string(),
            proposal: "b004".to_string(),
            target: "bookB/9".to_string(),
            relation: Some("contrasts".to_string()),
        }];
        let value = distant(&snapshot, &args).expect("a distant read");
        assert_eq!(value["candidate"]["source"], json!("register_term"), "{value}");
        assert_eq!(value["excluded"]["stale_report"], json!(1), "{value}");
        // A report target already on the Table or already reached by a Link is no suggestion.
        let mut args = DistantArgs::new("bookA/1");
        args.table = vec!["bookB/1".to_string()];
        args.reports = vec![ReportTarget {
            report: "bridge-2026-10-19".to_string(),
            proposal: "b005".to_string(),
            target: "bookB/1".to_string(),
            relation: None,
        }];
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

    /// S20-AC05, D251: a selected report may project more than one current proposal for a Card.
    /// The ordered proposals are tried in report order; one the exclusions remove falls through to
    /// the next, and only when none survives does the term evidence answer.
    #[test]
    fn distant_takes_the_next_current_report_proposal() {
        let (temp, _) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 多重均衡的陈述\n\n多重均衡来自策略互补。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookB",
                    "# 多重均衡的另一种说法\n\n这里也从策略互补说起。\n\n来源：[[bookB]]\n",
                ),
                (
                    "bookC",
                    "# 多重均衡的第三种说法\n\n策略互补也在这里出现。\n\n来源：[[bookC]]\n",
                ),
            ],
            &[],
            "## 词条\n- 多重均衡：[[bookB/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let target = |proposal: &str, at: &str, relation: &str| ReportTarget {
            report: "r-one".to_string(),
            proposal: proposal.to_string(),
            target: at.to_string(),
            relation: Some(relation.to_string()),
        };
        let mut args = DistantArgs::new("bookA/1");
        args.reports = vec![
            target("b001", "bookB/1", "contrasts"),
            target("b002", "bookC/1", "illustrates"),
        ];
        // The first proposal names a Card already on the Table: the second current one answers.
        args.table = vec!["bookB/1".to_string()];
        let value = distant(&snapshot, &args).expect("a distant read");
        assert_eq!(value["candidate"]["address"], json!("bookC/1"), "{value}");
        assert_eq!(value["candidate"]["report"]["proposal"], json!("b002"));
        assert_eq!(
            value["candidate"]["reason"],
            json!("Bridge report r-one, proposal b002: illustrates")
        );
        assert_eq!(value["excluded"]["table"], json!(1));
        // A proposal whose target is gone is skipped as stale, and the next one answers.
        args.table.clear();
        args.reports[0].target = "bookZ/9".to_string();
        let value = distant(&snapshot, &args).expect("a distant read");
        assert_eq!(value["candidate"]["address"], json!("bookC/1"), "{value}");
        assert_eq!(value["excluded"]["stale_report"], json!(1));
        // No current proposal left: the term evidence answers, never an invented report target.
        args.reports[1].target = "bookZ/8".to_string();
        let value = distant(&snapshot, &args).expect("a distant read");
        assert_eq!(value["candidate"]["source"], json!("register_term"), "{value}");
        assert_eq!(value["excluded"]["stale_report"], json!(2));
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

    // ---- the question path (S15, #114, design §9.3-§9.5) ----

    /// S15-AC01, D228: a question is nonblank and at most 512 Unicode characters, the Workspace
    /// filter defaults to both Workspaces, and a requested selection is five to nine Cards with
    /// seven when the reader names none.
    #[test]
    fn ask_arguments_take_a_nonblank_bounded_question_and_a_five_to_nine_selection() {
        let plain = AskArgs::from_args(&json!({ "question": "为什么学习可能造成多重均衡？" }))
            .expect("a nonblank question");
        assert_eq!(plain.question, "为什么学习可能造成多重均衡？");
        assert_eq!(plain.workspace, None, "the default filter is both Workspaces");
        assert_eq!(plain.limit, ASK_LIMIT_DEFAULT);
        assert_eq!(plain.limit, 7);

        for (asked, kept) in [(5, 5), (9, 9), (1, ASK_LIMIT_MIN), (40, ASK_LIMIT_MAX)] {
            let args = AskArgs::from_args(&json!({ "question": "问题", "limit": asked }))
                .expect("a requested size");
            assert_eq!(args.limit, kept, "limit {asked}");
        }
        for (named, expected) in [
            ("human", Some(Workspace::Human)),
            ("agent", Some(Workspace::Agent)),
            ("all", None),
        ] {
            let args = AskArgs::from_args(&json!({ "question": "问题", "workspace": named }))
                .expect("a named Workspace");
            assert_eq!(args.workspace, expected, "workspace {named}");
        }

        // The bound counts characters, not bytes, and keeps the whole question.
        let longest = "学".repeat(ASK_QUESTION_MAX_CHARS);
        let args = AskArgs::from_args(&json!({ "question": longest })).expect("512 characters");
        assert_eq!(args.question.chars().count(), ASK_QUESTION_MAX_CHARS);
        let too_long = "学".repeat(ASK_QUESTION_MAX_CHARS + 1);
        assert!(AskArgs::from_args(&json!({ "question": too_long })).is_err());
        // A question of blanks is no question.
        for blank in ["", "   ", "\n\t"] {
            assert!(
                AskArgs::from_args(&json!({ "question": blank })).is_err(),
                "`{blank}` is blank"
            );
        }
        assert!(AskArgs::from_args(&json!({})).is_err(), "no question");
        assert!(AskArgs::from_args(&json!({ "question": 3 })).is_err());
        assert!(AskArgs::from_args(&json!({ "question": "问题", "workspace": "shared" })).is_err());
        assert!(AskArgs::from_args(&json!({ "question": "问题", "limit": "seven" })).is_err());
    }

    /// S15-AC01, D227, D228: one question over an accepted term returns an unsaved Spread
    /// proposal that holds the best entry card, names the whole question, and carries the archive
    /// identity and revision it was read at.
    #[test]
    fn a_question_gathers_a_spread_that_holds_its_best_entry_card() {
        let (temp, _register) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 学习的陈述\n\n学习提高生产率。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookB",
                    "# 学习的另一种说法\n\n这里也从学习说起。\n\n来源：[[bookB]]\n",
                ),
            ],
            &[],
            "## 词条\n- 学习：[[bookB/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "学习为什么提高生产率？" }))
            .expect("a question");
        let identity = json!({ "id": "0123456789abcdef0123456789abcdef", "path": "/tmp/archive" });
        let value = gather(&snapshot, &args, 7, &identity);

        assert_eq!(value["question"], json!("学习为什么提高生产率？"));
        assert_eq!(value["workspace"], json!("all"));
        assert_eq!(value["limit"], json!(ASK_LIMIT_DEFAULT));
        assert_eq!(value["matched_terms"], json!(["学习"]));
        assert_eq!(value["ambiguous_terms"], json!([]));
        assert_eq!(value["held"], json!("bookB/1"), "{value}");
        assert_eq!(
            value["spread"]["name"],
            json!("学习为什么提高生产率？"),
            "the display name is the question"
        );
        assert_eq!(
            value["spread"]["question"],
            json!("学习为什么提高生产率？")
        );
        assert_eq!(value["spread"]["cards"][0]["at"], json!("bookB/1"));
        assert_eq!(value["spread"]["cards"][0]["x"], json!(0));
        assert_eq!(value["spread"]["cards"][0]["y"], json!(0));
        assert_eq!(value["cards"][0]["at"], json!("bookB/1"));
        assert_eq!(value["cards"][0]["term"], json!("学习"));
        assert_eq!(value["cards"][0]["held"], json!(true));
        assert_eq!(value["reasons"][0]["at"], json!("bookB/1"));
        assert_eq!(value["reasons"][0]["term"], json!("学习"));
        assert_eq!(value["archive_revision"], json!(7));
        assert_eq!(value["archive_identity"], identity);
        assert_eq!(value["truncated"], json!(false));
        assert_eq!(
            value["sparse"], json!(true),
            "one justified Card is under the five a full Spread needs"
        );
    }

    /// S15-AC02, D212, D213: accepted labels are found as substrings of the question, a longer
    /// label is preferred over the shorter label it contains, two overlapping concepts that contain
    /// each other neither way both stay, and a bare word that several accepted scoped labels state
    /// exposes every sense with its own ambiguous entry instead of silently choosing one.
    #[test]
    fn a_question_exposes_every_sense_of_a_bare_scoped_word_and_prefers_the_longer_label() {
        let (temp, _register) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 个体学习\n\n个体学习提高技能。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookB",
                    "# 产业学习\n\n产业学习提高生产率。\n\n来源：[[bookB]]\n",
                ),
                (
                    "bookC",
                    "# 多重均衡\n\n多重均衡来自策略互补。\n\n来源：[[bookC]]\n",
                ),
                (
                    "bookD",
                    "# 产业政策\n\n产业政策改变投资。\n\n来源：[[bookD]]\n",
                ),
            ],
            &[],
            "## 词条\n- 学习（个体行为）：[[bookA/1]]\n- 学习（产业生产率）：[[bookB/1]]\n- 多重均衡：[[bookC/1]]\n- 均衡：[[bookC/1]]\n- 产业政策：[[bookD/1]]\n- 政策效应：[[bookD/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let ask = |question: &str| {
            let args = AskArgs::from_args(&json!({ "question": question })).expect("a question");
            gather(&snapshot, &args, 1, &json!({}))
        };

        // A bare word that two accepted scoped labels state: every sense is a matched term and the
        // bare word itself names the ambiguity. The scoped label `均衡` is contained in `多重均衡`,
        // which the question also states, so only the longer concept matches.
        let value = ask("为什么学习可能造成多重均衡？");
        assert_eq!(
            value["matched_terms"],
            json!(["学习（个体行为）", "学习（产业生产率）", "多重均衡"]),
            "{value}"
        );
        assert_eq!(value["ambiguous_terms"], json!(["学习"]), "{value}");
        assert_eq!(value["held"], json!("bookA/1"), "{value}");

        // The full scoped label is preferred over its bare word, so no sense is exposed.
        let value = ask("学习（产业生产率）为什么提高生产率？");
        assert_eq!(
            value["matched_terms"],
            json!(["学习（产业生产率）"]),
            "{value}"
        );
        assert_eq!(value["ambiguous_terms"], json!([]), "{value}");
        assert_eq!(value["held"], json!("bookB/1"), "{value}");

        // Two concepts that overlap without containing each other both match.
        let value = ask("产业政策效应如何？");
        assert_eq!(
            value["matched_terms"],
            json!(["产业政策", "政策效应"]),
            "{value}"
        );
        assert_eq!(value["held"], json!("bookD/1"), "{value}");
    }

    /// S15-AC02, D213, D291: at most six matched terms and twelve distinct entry seeds are taken,
    /// in evidence order, and the reply says how many it left out and which bounds are in force.
    #[test]
    fn a_question_takes_six_terms_and_twelve_seeds_and_reports_the_bounds_it_hit() {
        let (temp, _register) = archive(
            &[],
            &[("bookQ", "# 陈述一\n\n第一条陈述。\n\n来源：[[bookQ]]\n")],
            &[
                ("bookQ/1", crate::ops::Successor::Direct, CardType::Idea, "# 陈述二\n\n第二条陈述。"),
                ("bookQ/2", crate::ops::Successor::Direct, CardType::Idea, "# 陈述三\n\n第三条陈述。"),
                ("bookQ/3", crate::ops::Successor::Direct, CardType::Idea, "# 陈述四\n\n第四条陈述。"),
                ("bookQ/4", crate::ops::Successor::Direct, CardType::Idea, "# 陈述五\n\n第五条陈述。"),
                ("bookQ/5", crate::ops::Successor::Direct, CardType::Idea, "# 陈述六\n\n第六条陈述。"),
                ("bookQ/6", crate::ops::Successor::Direct, CardType::Idea, "# 陈述七\n\n第七条陈述。"),
                ("bookQ/7", crate::ops::Successor::Direct, CardType::Idea, "# 陈述八\n\n第八条陈述。"),
                ("bookQ/8", crate::ops::Successor::Direct, CardType::Idea, "# 陈述九\n\n第九条陈述。"),
                ("bookQ/9", crate::ops::Successor::Direct, CardType::Idea, "# 陈述十\n\n第十条陈述。"),
                ("bookQ/10", crate::ops::Successor::Direct, CardType::Idea, "# 陈述十一\n\n第十一条陈述。"),
                ("bookQ/11", crate::ops::Successor::Direct, CardType::Idea, "# 陈述十二\n\n第十二条陈述。"),
                ("bookQ/12", crate::ops::Successor::Direct, CardType::Idea, "# 陈述十三\n\n第十三条陈述。"),
            ],
            "## 词条\n- 甲：[[bookQ/1]] [[bookQ/2]]\n- 乙：[[bookQ/3]] [[bookQ/4]]\n- 丙：[[bookQ/5]] [[bookQ/6]]\n- 丁：[[bookQ/7]] [[bookQ/8]]\n- 戊：[[bookQ/9]] [[bookQ/10]]\n- 己：[[bookQ/11]] [[bookQ/12]] [[bookQ/13]]\n- 庚：[[bookQ/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "甲乙丙丁戊己庚分别是什么？" }))
            .expect("a question");
        let value = gather(&snapshot, &args, 1, &json!({}));

        assert_eq!(
            value["matched_terms"],
            json!(["甲", "乙", "丙", "丁", "戊", "己"]),
            "the seventh term is left out, in evidence order: {value}"
        );
        assert_eq!(value["counts"]["terms"], json!(7));
        assert_eq!(value["counts"]["terms_dropped"], json!(1));
        assert_eq!(
            value["counts"]["seeds"], json!(13),
            "six kept terms offer thirteen distinct Cards"
        );
        assert_eq!(value["counts"]["seeds_dropped"], json!(1));
        assert_eq!(value["counts"]["selected"], json!(ASK_LIMIT_DEFAULT));
        assert_eq!(value["truncated"], json!(true));
        assert_eq!(value["bounds"]["terms"], json!(ASK_TERM_MAX));
        assert_eq!(value["bounds"]["seeds"], json!(ASK_SEED_MAX));
        assert_eq!(value["bounds"]["depth"], json!(ASK_DEPTH_MAX));
        assert_eq!(value["bounds"]["cards"], json!(ASK_CARD_MAX));
        assert_eq!(value["bounds"]["links"], json!(ASK_LINK_MAX));
        assert_eq!(value["bounds"]["limit_min"], json!(ASK_LIMIT_MIN));
        assert_eq!(value["bounds"]["limit_max"], json!(ASK_LIMIT_MAX));
        assert_eq!(
            value["bounds"]["question_max_chars"],
            json!(ASK_QUESTION_MAX_CHARS)
        );
        // Every selected Card names the term that reached it and the register row that accepted it.
        for card in value["cards"].as_array().expect("cards") {
            assert!(card["term"].is_string(), "{card}");
            assert_eq!(card["origin"]["card"], json!("0/1"), "{card}");
            assert!(card["origin"]["line"].as_u64().is_some(), "{card}");
            assert!(card["path"].is_array(), "{card}");
            assert!(card["reason"].as_str().is_some_and(|reason| !reason.is_empty()));
        }
        assert_eq!(value["held"], json!("bookQ/1"));
    }

    /// S15-AC03, D212: when no accepted label matches, a small pure character-bigram overlap
    /// against titles and model names seeds the set, and the explanation says it is low confidence.
    /// The overlap is its own calculation: the digest's TF-IDF stack stays in its own crate.
    #[test]
    fn a_question_without_an_accepted_term_falls_back_to_a_low_confidence_title_overlap() {
        let (temp, _register) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 多重均衡的出现\n\n多重均衡来自策略互补。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookB",
                    "# 策略互补\n\n策略互补解释这个结果。\n\n来源：[[bookB]]\n",
                ),
            ],
            &[],
            "## 词条\n- 学习（个体行为）：[[bookA/1]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "多重均衡从哪里来？" }))
            .expect("a question");
        let value = gather(&snapshot, &args, 1, &json!({}));

        assert_eq!(value["matched_terms"], json!([]), "{value}");
        assert_eq!(value["ambiguous_terms"], json!([]), "{value}");
        assert_eq!(value["fallback"]["kind"], json!("bigram"), "{value}");
        assert_eq!(value["fallback"]["confidence"], json!("low"), "{value}");
        assert!(
            value["fallback"]["explanation"]
                .as_str()
                .is_some_and(|text| text.contains("low confidence")),
            "{value}"
        );
        assert_eq!(value["held"], json!("bookA/1"), "{value}");
        assert_eq!(value["cards"][0]["at"], json!("bookA/1"), "{value}");
        assert_eq!(value["cards"][0]["term"], Value::Null, "{value}");
        assert!(
            value["cards"][0]["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("bigram")),
            "{value}"
        );
        assert!(value["counts"]["visited"].as_u64().is_some(), "{value}");

        // The calculation is small and pure: no compiler dependency, no TF-IDF stack.
        let source = include_str!("entry.rs");
        let forbidden = ["tf".to_owned() + "idf", "zt_".to_owned() + "digest"];
        for name in forbidden {
            assert!(
                !source.contains(&name),
                "entry.rs must not reach into the {name} stack"
            );
        }
    }

    /// S15-AC04, D215, D216: two semantic steps in either direction from the entry seeds, each
    /// step keeping the Link's own from/to direction and Relation kind. An inbound `rests-on` Link
    /// stays the neighbour's `rests-on` Link to the seed, not the seed's relation to it.
    #[test]
    fn a_walk_takes_two_semantic_steps_in_both_directions_and_keeps_each_links_direction() {
        let (temp, _register) = archive(
            &[],
            &[("bookA", "# 甲的陈述\n\n甲的陈述内容。\n\n来源：[[bookA]]\n")],
            &[
                (
                    "bookA/1",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 乙\n\n乙的陈述。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/2",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 丙\n\n丙的陈述。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/3",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 丁\n\n丁的结果。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/4",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 戊\n\n更深一层。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/5",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 己\n\n乙的例子。\n\n来源：[[bookA]]\n",
                ),
            ],
            "## 词条\n- 甲：[[bookA/1]]\n",
        );
        // The semantic Links are edits, because a Link needs its target to exist when it is written.
        edit(
            temp.path(),
            "bookA/1",
            "# 甲的陈述\n\n甲的陈述内容。对应 [[bookA/3]] 的机制。\n\n来源：[[bookA]]\n",
        );
        edit(
            temp.path(),
            "bookA/2",
            "# 乙\n\n依赖 [[bookA/1]] 的设定。例示 [[bookA/6]]。\n\n来源：[[bookA]]\n",
        );
        edit(
            temp.path(),
            "bookA/3",
            "# 丙\n\n对照 [[bookA/4]] 的结果。\n\n来源：[[bookA]]\n",
        );
        edit(
            temp.path(),
            "bookA/4",
            "# 丁\n\n丁的结果。依赖 [[bookA/5]]。\n\n来源：[[bookA]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "甲讲了什么？" }))
            .expect("a question");
        let value = gather(&snapshot, &args, 1, &json!({}));
        let cards = value["cards"].as_array().expect("cards");
        let at = |address: &str| {
            cards
                .iter()
                .find(|card| card["at"] == json!(address))
                .unwrap_or_else(|| panic!("{address} was not gathered: {value}"))
                .clone()
        };

        assert_eq!(value["held"], json!("bookA/1"));
        assert_eq!(at("bookA/1")["depth"], json!(0));
        assert_eq!(at("bookA/1")["path"], json!([]));
        assert_eq!(at("bookA/2")["depth"], json!(1));
        assert_eq!(at("bookA/3")["depth"], json!(1));
        assert_eq!(at("bookA/4")["depth"], json!(2));
        assert_eq!(at("bookA/6")["depth"], json!(2));
        assert!(
            !cards.iter().any(|card| card["at"] == json!("bookA/5")),
            "a third step is beyond the depth bound: {value}"
        );

        // An inbound rests-on Link keeps its authored direction and kind.
        assert_eq!(
            at("bookA/2")["path"],
            json!([{
                "from": "bookA/2",
                "to": "bookA/1",
                "direction": "in",
                "kind": "rests-on",
                "sentence": "依赖 「甲的陈述」 的设定。",
            }]),
            "{value}"
        );
        // Two outbound steps keep their own kinds in order.
        assert_eq!(
            at("bookA/4")["path"],
            json!([
                {"from": "bookA/1", "to": "bookA/3", "direction": "out", "kind": "corresponds", "sentence": "对应 「丙」 的机制。"},
                {"from": "bookA/3", "to": "bookA/4", "direction": "out", "kind": "contrasts", "sentence": "对照 「丁」 的结果。"},
            ]),
            "{value}"
        );

        // Only authored Links between two selected Cards become threads, with their own kinds.
        let mut threads = value["threads"]
            .as_array()
            .expect("threads")
            .iter()
            .map(|thread| {
                format!(
                    "{}->{}:{}",
                    thread["from"].as_str().unwrap_or_default(),
                    thread["to"].as_str().unwrap_or_default(),
                    thread["kind"].as_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>();
        threads.sort();
        assert_eq!(
            threads,
            vec![
                "bookA/1->bookA/3:corresponds",
                "bookA/2->bookA/1:rests-on",
                "bookA/2->bookA/6:illustrates",
                "bookA/3->bookA/4:contrasts",
            ],
            "{value}"
        );
        assert_eq!(value["counts"]["depth"], json!(2));
        assert_eq!(value["counts"]["visited"], json!(5));
        assert_eq!(value["counts"]["links"], json!(6));
    }

    /// S15-AC04, S15-AC09, D216: a Hub Card an entry row names supplies its member Cards as
    /// entries, while a source locator, a navigation section, a Hub or Structure Card's member
    /// list, a Broken link and a source tree root never become a semantic step. Every exclusion is
    /// counted, so a reader can tell "nothing links here" from "everything that did was a way in".
    #[test]
    fn a_question_never_walks_through_sources_navigation_hubs_roots_or_structure_lists() {
        let (temp, _register) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 甲的陈述\n\n甲的陈述内容。\n\n来源：[[bookA]]\n",
                ),
                ("bookB", "# 乙的陈述\n\n乙的内容。\n\n来源：[[bookB]]\n"),
            ],
            &[
                (
                    "bookA/1",
                    crate::ops::Successor::Side,
                    CardType::Hub,
                    "# 甲的枢纽\n\n枢纽正文。\n",
                ),
                (
                    "bookA/1",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 丙的陈述\n\n丙的内容。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/2",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 丁的陈述\n\n丁的内容。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/3",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 戊的陈述\n\n戊的内容。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/4",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 己的陈述\n\n己的内容。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/5",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 要删的卡\n\n这张卡会被删掉，留下一个 Broken link。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookB/1",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 结构卡\n\n结构卡的正文。\n",
                ),
                (
                    "bookB/2",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 另一张\n\n另一张内容。\n\n来源：[[bookB]]\n",
                ),
            ],
            "## 词条\n- 甲：[[bookA/1|a]]\n- 乙：[[bookB/1]]\n",
        );
        edit(
            temp.path(),
            "bookA/1|a",
            "# 甲的枢纽\n\n- [[bookA/3]]：成员一。\n- [[bookA/4]]：成员二。\n",
        );
        edit(
            temp.path(),
            "bookA/3",
            "# 丁的陈述\n\n丁的陈述内容。对应 [[bookA/2]] 的机制。参见 [[bookA]] 与 [[bookB/1]]。这里引用了 [[bookA/6]]。\n\n## 邻近\n\n- [[bookA/5]] 相邻。\n\n来源：[[bookB/2]]\n",
        );
        // A Broken link is one whose target was deleted after it was written.
        delete(temp.path(), "bookA/6");
        edit(
            temp.path(),
            "bookB/1",
            "# 结构卡\n\n本卡是《Book B》的结构卡：按阅读顺序列出全书的论证。\n\n- [[bookB/2]] 第二步。\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "甲乙讲了什么？" }))
            .expect("a question");
        let value = gather(&snapshot, &args, 1, &json!({}));
        let cards = value["cards"].as_array().expect("cards");
        let addresses = cards
            .iter()
            .filter_map(|card| card["at"].as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            addresses,
            vec!["bookA/3", "bookA/4", "bookA/2"],
            "the hub's members are the entries, and only the authored semantic Link is followed: {value}"
        );
        assert_eq!(value["held"], json!("bookA/3"));
        assert_eq!(cards[0]["via_hub"], json!("bookA/1|a"), "{value}");
        assert_eq!(cards[0]["term"], json!("甲"));
        assert_eq!(
            value["excluded"]["source_root"], json!(1),
            "a source tree root is no step: {value}"
        );
        assert_eq!(
            value["excluded"]["structure"], json!(2),
            "the Structure Card is no entry and no step: {value}"
        );
        assert_eq!(
            value["excluded"]["broken"], json!(1),
            "a Broken link is no step: {value}"
        );
        assert_eq!(
            value["excluded"]["navigation"], json!(1),
            "a `## 邻近` member line is no step: {value}"
        );
        assert_eq!(
            value["excluded"]["source_locator"], json!(3),
            "every `来源` line is a locator, in a section or not: {value}"
        );
        // The Structure Card's own list is never read as entries or as steps.
        assert!(
            !addresses.contains(&"bookB/2"),
            "the structure list member is not a step: {value}"
        );
    }

    /// Create `count` Idea Cards as Direct successors in one tree, and return their addresses in
    /// order. The text is owned, because a bound test needs many Cards.
    fn chain(root: &std::path::Path, from: &str, count: usize) -> Vec<String> {
        let mut parent = from.to_string();
        let mut made = Vec::new();
        for index in 0..count {
            let address = crate::ops::create_successor(
                root,
                crate::Access::Owner,
                crate::ops::Successor::Direct,
                &parent,
                CardType::Idea,
                None,
                &format!("# 卡{index}\n\n第 {index} 张陈述。\n\n来源：[[bookA]]\n"),
            )
            .expect("a Card")
            .0;
            parent = address.clone();
            made.push(address);
        }
        made
    }

    /// S15-AC05, D217: a cycle is walked once, and the same question over the same read gives the
    /// same proposal, so a reader can retry a question without the answer drifting.
    #[test]
    fn a_question_walks_a_cycle_once_and_repeats_the_same_answer() {
        let (temp, _register) = archive(
            &[],
            &[("bookA", "# 甲的陈述\n\n甲的陈述内容。\n\n来源：[[bookA]]\n")],
            &[
                (
                    "bookA/1",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 乙\n\n乙的内容。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/2",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 丙\n\n丙的内容。\n\n来源：[[bookA]]\n",
                ),
            ],
            "## 词条\n- 甲：[[bookA/1]]\n",
        );
        edit(
            temp.path(),
            "bookA/1",
            "# 甲的陈述\n\n甲的陈述内容。依赖 [[bookA/2]]。\n\n来源：[[bookA]]\n",
        );
        edit(
            temp.path(),
            "bookA/2",
            "# 乙\n\n依赖 [[bookA/3]]。\n\n来源：[[bookA]]\n",
        );
        edit(
            temp.path(),
            "bookA/3",
            "# 丙\n\n依赖 [[bookA/2]]。\n\n来源：[[bookA]]\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "甲讲了什么？" }))
            .expect("a question");
        let first = gather(&snapshot, &args, 1, &json!({}));
        let second = gather(&snapshot, &args, 1, &json!({}));
        assert_eq!(first, second, "the same read gives the same proposal");
        let addresses = first["cards"]
            .as_array()
            .expect("cards")
            .iter()
            .filter_map(|card| card["at"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(addresses, vec!["bookA/1", "bookA/2", "bookA/3"], "{first}");
        assert_eq!(first["counts"]["visited"], json!(3));
        assert_eq!(first["truncated"], json!(false));
    }

    /// S15-AC05, D217, S15-AC06: the walk visits at most 200 substantive Cards, says so, and never
    /// lists a Card twice.
    #[test]
    fn a_question_stops_at_the_card_bound_and_names_it() {
        let (temp, _register) = archive(
            &[],
            &[("bookA", "# 甲的陈述\n\n甲的陈述内容。\n\n来源：[[bookA]]\n")],
            &[],
            "## 词条\n- 甲：[[bookA/1]]\n",
        );
        let made = chain(temp.path(), "bookA/1", ASK_CARD_MAX + 10);
        let links = made
            .iter()
            .map(|at| format!("依赖 [[{at}]]。"))
            .collect::<Vec<_>>()
            .join("\n");
        edit(
            temp.path(),
            "bookA/1",
            &format!("# 甲的陈述\n\n甲的陈述内容。\n{links}\n\n来源：[[bookA]]\n"),
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "甲讲了什么？" }))
            .expect("a question");
        let value = gather(&snapshot, &args, 1, &json!({}));

        assert_eq!(
            value["counts"]["visited"], json!(ASK_CARD_MAX),
            "the walk stops exactly at the bound: {value}"
        );
        assert_eq!(value["counts"]["cards_truncated"], json!(true), "{value}");
        assert_eq!(value["truncated"], json!(true), "{value}");
        assert_eq!(value["bounds"]["cards"], json!(ASK_CARD_MAX));
        let mut seen = BTreeSet::new();
        for card in value["cards"].as_array().expect("cards") {
            let at = card["at"].as_str().expect("address");
            assert!(seen.insert(at.to_string()), "{at} was listed twice");
        }
    }

    /// S15-AC05, D217: the walk examines at most 1,000 authored semantic Links, says so, and keeps
    /// what it already reached.
    #[test]
    fn a_question_stops_at_the_link_bound_and_names_it() {
        let (temp, _register) = archive(
            &[],
            &[("bookA", "# 甲的陈述\n\n甲的陈述内容。\n\n来源：[[bookA]]\n")],
            &[
                (
                    "bookA/1",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 乙\n\n乙的内容。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/2",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 丙\n\n丙的内容。\n\n来源：[[bookA]]\n",
                ),
            ],
            "## 词条\n- 甲：[[bookA/1]]\n",
        );
        edit(
            temp.path(),
            "bookA/1",
            "# 甲的陈述\n\n甲的陈述内容。依赖 [[bookA/2]]。\n\n来源：[[bookA]]\n",
        );
        let many = (0..ASK_LINK_MAX)
            .map(|_| "依赖 [[bookA/3]]。")
            .collect::<Vec<_>>()
            .join("\n");
        edit(
            temp.path(),
            "bookA/2",
            &format!("# 乙\n\n{many}\n\n来源：[[bookA]]\n"),
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "甲讲了什么？" }))
            .expect("a question");
        let value = gather(&snapshot, &args, 1, &json!({}));

        assert_eq!(
            value["counts"]["links"], json!(ASK_LINK_MAX),
            "exactly the bound is examined, never more: {value}"
        );
        assert_eq!(value["counts"]["links_truncated"], json!(true), "{value}");
        assert_eq!(value["truncated"], json!(true), "{value}");
        assert_eq!(value["bounds"]["links"], json!(ASK_LINK_MAX));
        assert_eq!(
            value["counts"]["visited"], json!(3),
            "the Cards reached before the bound stay: {value}"
        );
    }

    /// S15-AC06, D218-D224: the proposal holds the best entry card, prefers a distinct statement
    /// over another member of one restatement set, prefers a directly relevant Card and a shorter
    /// path, prefers a new source, tree or Relation kind among comparably relevant Cards, and
    /// breaks the last tie by address order.
    #[test]
    fn a_question_selects_distinct_statements_then_cross_source_cards_then_address_order() {
        let (temp, _register) = archive(
            &[],
            &[
                (
                    "bookA",
                    "# 甲的陈述\n\n甲的陈述内容。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookB",
                    "# 乙的陈述\n\n乙的内容。\n\n来源：[[bookB]]\n",
                ),
            ],
            &[
                (
                    "bookA/1",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 第二个陈述\n\n第二个内容。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/2",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 入口卡\n\n入口正文。\n\n来源：[[bookA]]\n",
                ),
                (
                    "bookA/3",
                    crate::ops::Successor::Direct,
                    CardType::Idea,
                    "# 第四个陈述\n\n第四个内容。\n\n来源：[[bookA]]\n",
                ),
            ],
            "## 词条\n- 甲：[[bookA/1]]\n",
        );
        edit(
            temp.path(),
            "bookA/1",
            "# 甲的陈述\n\n甲的陈述内容。对应 [[bookA/2]]。对应 [[bookA/3]]。对应 [[bookB/1]]。\n\n来源：[[bookA]]\n",
        );
        edit(
            temp.path(),
            "bookA/2",
            "# 第二个陈述\n\n第二个内容。依赖 [[bookA/4]]。\n\n来源：[[bookA]]\n",
        );
        // A restatement entry states no claim of its own: it holds the statement of `bookA/2`.
        edit(
            temp.path(),
            "bookA/3",
            "# 入口卡\n\n详见 [[bookA/2]]。\n\n## 同述入口\n\n- [[bookA/2]] 完整陈述。\n",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "甲讲了什么？", "limit": 5 }))
            .expect("a question");
        let value = gather(&snapshot, &args, 1, &json!({}));
        let addresses = value["cards"]
            .as_array()
            .expect("cards")
            .iter()
            .filter_map(|card| card["at"].as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            addresses,
            vec!["bookA/1", "bookB/1", "bookA/2", "bookA/4", "bookA/3"],
            "held, then the other book, then the nearer statement, then the new statement before \
             the restatement entry: {value}"
        );
        assert_eq!(value["held"], json!("bookA/1"));
        assert_eq!(value["cards"][0]["held"], json!(true));
        assert_eq!(value["cards"][1]["held"], json!(false));
        assert_eq!(value["sparse"], json!(false), "five Cards is a full set");
        assert_eq!(value["cards"][4]["at"], json!("bookA/3"));
        assert_eq!(
            value["cards"][4]["statement"],
            json!("bookA/2"),
            "the entry names the statement it holds: {value}"
        );
        // Only authored Links become threads, and a plain reference stays `refers` (§9.4, D225).
        let threads = value["threads"].as_array().expect("threads");
        assert!(
            threads.iter().any(|thread| thread["from"] == json!("bookA/3")
                && thread["to"] == json!("bookA/2")
                && thread["kind"] == json!("refers")),
            "an unkinded Link stays refers: {value}"
        );
        assert!(
            !threads.iter().any(|thread| thread["to"] == json!("bookA/4")
                && thread["from"] == json!("bookA/1")),
            "no thread is inferred between two selected Cards: {value}"
        );
    }

    /// S15-AC09, S15-AC07: a question whose entry row no longer names a Card is no question, and
    /// zero Cards is a valid answer: the proposal is empty, sparse, and names the register fault.
    #[test]
    fn a_question_with_no_usable_entry_returns_an_empty_sparse_proposal_and_the_fault() {
        let (temp, _register) = archive(
            &[],
            &[("bookA", "# 甲的陈述\n\n甲的陈述内容。\n\n来源：[[bookA]]\n")],
            &[],
            "## 词条\n- 甲：[[bookA/1]]\n",
        );
        delete(temp.path(), "bookA/1");
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "学习为什么提高生产率？" }))
            .expect("a question");
        let value = gather(&snapshot, &args, 1, &json!({}));

        assert_eq!(value["matched_terms"], json!([]), "{value}");
        assert_eq!(value["held"], Value::Null, "{value}");
        assert_eq!(value["cards"], json!([]), "{value}");
        assert_eq!(value["spread"]["cards"], json!([]), "{value}");
        assert_eq!(value["sparse"], json!(true), "{value}");
        assert_eq!(value["truncated"], json!(false), "{value}");
        assert_eq!(value["counts"]["visited"], json!(0), "{value}");
        assert_eq!(value["registers"][0]["complete"], json!(false), "{value}");
        assert!(
            value["faults"]
                .as_array()
                .expect("faults")
                .iter()
                .any(|fault| fault["fault"] == json!("broken")),
            "the row's missing target is named: {value}"
        );
    }

    /// S15-AC09, S15-AC03, D228: an archive without a register reports the missing register and
    /// answers with the labelled low-confidence fallback; a question changes no Card.
    #[test]
    fn a_question_without_a_register_reports_it_and_writes_nothing() {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let topic = crate::ops::create_topic(
            temp.path(),
            crate::Access::Owner,
            Workspace::Agent,
            "# 学习树\n\n问题树。",
        )
        .expect("topic")
        .0;
        crate::ops::create_successor(
            temp.path(),
            crate::Access::Owner,
            crate::ops::Successor::Direct,
            &topic,
            CardType::Idea,
            None,
            "# 多重均衡的出现\n\n多重均衡来自策略互补。\n",
        )
        .expect("a Card");
        let before = Snapshot::read(temp.path()).expect("snapshot").fingerprint();
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let args = AskArgs::from_args(&json!({ "question": "多重均衡从哪里来？" }))
            .expect("a question");
        let value = gather(&snapshot, &args, 1, &json!({}));

        assert_eq!(value["held"], json!("0/1"), "{value}");
        assert_eq!(value["fallback"]["kind"], json!("bigram"), "{value}");
        assert_eq!(value["fallback"]["confidence"], json!("low"), "{value}");
        assert_eq!(
            value["registers"],
            json!([
                {"workspace": "human", "card": Value::Null, "title": Value::Null, "missing": true, "complete": false, "entry_total": 0, "faults": 0},
                {"workspace": "agent", "card": Value::Null, "title": Value::Null, "missing": true, "complete": false, "entry_total": 0, "faults": 0},
            ]),
            "{value}"
        );
        assert!(
            value["faults"]
                .as_array()
                .expect("faults")
                .iter()
                .any(|fault| fault["fault"] == json!("register_missing")),
            "the missing register is named: {value}"
        );
        assert_eq!(
            Snapshot::read(temp.path()).expect("snapshot").fingerprint(),
            before,
            "gathering a question changes no Card"
        );
    }

    /// S15-AC07, D216, `CONTEXT.md` Register: a register row may name a Topic Card as "a standalone
    /// Topic claim", so a question may hold it and walk from it; a Literature tree root is the
    /// source root that would join a whole book, so it is never a seed.
    #[test]
    fn a_question_holds_a_topic_card_a_row_names_but_never_a_source_root() {
        let (temp, _register) = archive(
            &["# 多重均衡的树\n\n多重均衡取决于预期。"],
            &[("bookA", "# 甲的陈述\n\n甲的陈述内容。\n\n来源：[[bookA]]\n")],
            &[(
                "0/0",
                crate::ops::Successor::Direct,
                CardType::Idea,
                "# 预期渠道\n\n预期强化学习。\n",
            )],
            "## 词条\n- 多重均衡：[[0/0]]\n- 甲：[[bookA]]\n",
        );
        edit(
            temp.path(),
            "0/0",
            "# 多重均衡的树\n\n多重均衡取决于预期。入口：依赖 [[0/1]] 的预期渠道。",
        );
        let snapshot = Snapshot::read(temp.path()).expect("snapshot");
        let ask = |question: &str| {
            let args = AskArgs::from_args(&json!({ "question": question })).expect("a question");
            gather(&snapshot, &args, 1, &json!({}))
        };

        let value = ask("多重均衡讲了什么？");
        assert_eq!(value["held"], json!("0/0"), "{value}");
        assert_eq!(value["cards"][0]["type"], json!("idea"));
        assert_eq!(value["cards"][1]["at"], json!("0/1"), "{value}");
        assert_eq!(value["cards"][1]["path"][0]["kind"], json!("rests-on"));
        assert_eq!(value["cards"][1]["path"][0]["direction"], json!("out"));

        // The other row names a Literature Card: a source root is no entry.
        let value = ask("甲讲了什么？");
        assert_eq!(value["held"], Value::Null, "{value}");
        assert_eq!(value["cards"], json!([]), "{value}");
        assert_eq!(value["excluded"]["source_root"], json!(1), "{value}");
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
