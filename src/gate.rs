//! The selected archive's effective Card-check policy (S03, #145).
//!
//! The checker owns the rule catalog and its defaults (`zt_check::policy`). This
//! module owns only what the archive stores: one `metadata` row per declared
//! `gate.<workspace>.<rule>` override, its validated write under the edit lock,
//! and the archive revision the change advances. A missing row means the
//! checker's default; `default` removes the row.
//!
//! The loaded `PolicySet` is the snapshot a write gate decides with. It covers
//! both Workspaces and is loaded once per operation, so a whole atomic batch
//! uses one policy (design 6.1, S03-AC05).

use crate::link_index;
use crate::model::{Emphasis, Facet, Level};
use crate::{Card, CardType, Workspace};
use anyhow::{Result, anyhow, bail};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use zt_check::core::{Check, Diagnostic, Limits, Severity};
use zt_check::policy::{self, Applicability, Change, CheckReport, PolicySet, Workspace as GateWorkspace};

/// The metadata prefix that is reserved for gate overrides.
pub(crate) const PREFIX: &str = policy::GATE_PREFIX;

/// The archive metadata key that holds the write gate's rollout stage.
pub(crate) const STAGE_KEY: &str = "gate.stage";

/// The stage of the shared write gate (design §17: new quality rules initially advise; a
/// deliberate owner promotion turns enforcement on). It is an archive decision, not a rule
/// severity: the policy still says what the rule would decide, and this stage says whether a
/// blocking quality diagnostic refuses a write now or is reported as advice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Stage {
    /// Every quality diagnostic is returned as advice; every mandatory invariant still refuses.
    Advisory,
    /// The effective policy decides, so a `block` setting refuses the write.
    Enforce,
}

impl Stage {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "advisory" => Ok(Stage::Advisory),
            "enforce" => Ok(Stage::Enforce),
            other => bail!("unknown gate stage `{other}`; expected advisory or enforce"),
        }
    }

    pub(crate) fn id(self) -> &'static str {
        match self {
            Stage::Advisory => "advisory",
            Stage::Enforce => "enforce",
        }
    }
}

/// The archive's stage: an absent row is the rollout default, advisory.
pub(crate) fn current_stage(conn: &Connection) -> Result<Stage> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = ?1",
            [STAGE_KEY],
            |row| row.get(0),
        )
        .optional()?;
    match value {
        Some(value) => Stage::parse(&value),
        None => Ok(Stage::Advisory),
    }
}

/// Validate and apply one `gate.stage` change. `default` removes the row and restores the
/// rollout default. Returns whether a row was written or removed.
pub(crate) fn change_stage(conn: &mut Connection, value: &str) -> Result<(Stage, bool)> {
    let (stage, write) = if value.trim().eq_ignore_ascii_case("default") {
        (Stage::Advisory, false)
    } else {
        (Stage::parse(value)?, true)
    };
    let current = current_stage(conn)?;
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = ?1",
            [STAGE_KEY],
            |row| row.get(0),
        )
        .optional()?;
    let changed = if write {
        row.as_deref() != Some(stage.id())
    } else {
        row.is_some()
    };
    if changed {
        let tx = conn.unchecked_transaction()?;
        if write {
            tx.execute(
                "INSERT INTO metadata(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [STAGE_KEY, stage.id()],
            )?;
        } else {
            tx.execute("DELETE FROM metadata WHERE key = ?1", [STAGE_KEY])?;
        }
        link_index::bump_revision(&tx)?;
        tx.commit()?;
    }
    Ok((if write { stage } else { current }, changed))
}

/// The selected archive's stored overrides as `(key, value)` rows.
fn rows(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut statement = conn.prepare(
        "SELECT key, value FROM metadata WHERE key LIKE 'gate.%' AND key <> ?1 ORDER BY key",
    )?;
    let rows = statement
        .query_map([STAGE_KEY], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The archive's effective policy: the checker's defaults plus these rows. A
/// stored row this binary cannot read is an error naming the row, never a
/// silently weaker default (design 6.1, S03-AC02).
pub(crate) fn load(conn: &Connection) -> Result<PolicySet> {
    let ready: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'metadata'",
        [],
        |row| row.get(0),
    )?;
    if ready == 0 {
        bail!("the archive is not initialized; run `zt up`");
    }
    let pairs = rows(conn)?;
    PolicySet::from_pairs(pairs).map_err(|error| anyhow!(error))
}

/// The revision a policy change advances, so a check-contract reader sees it.
pub(crate) fn revision(conn: &Connection) -> Result<u64> {
    link_index::revision(conn)
}

/// One accepted policy change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub(crate) workspace: GateWorkspace,
    pub(crate) rule: String,
    pub(crate) change: Change,
    /// Whether a metadata row was written or removed. A repeated request writes
    /// nothing and moves no revision.
    pub(crate) written: bool,
}

impl Outcome {
    /// The line `zt config set gate.…` prints.
    pub(crate) fn line(&self) -> String {
        let setting = match self.change {
            Change::Set(setting) => setting.as_str(),
            Change::Reset => "default",
        };
        format!(
            "{}={setting}",
            policy::gate_key(self.workspace, &self.rule)
        )
    }
}

/// Validate and apply one `gate.<workspace>.<rule>` change in one transaction.
///
/// Validation runs before any write, so a typo, an invariant, a pending rule, or
/// a graph-only blocking override leaves the metadata and the revision exactly
/// as they were.
pub(crate) fn change(conn: &mut Connection, key: &str, value: &str) -> Result<Outcome> {
    let (workspace, rule) = policy::parse_gate_key(key).map_err(|error| anyhow!(error))?;
    let change = Change::parse(value).map_err(|error| anyhow!(error))?;
    let mut policies = load(conn)?;
    let written = policies
        .change(workspace, rule, change)
        .map_err(|error| anyhow!(error))?;
    if written {
        let tx = conn.unchecked_transaction()?;
        match change {
            Change::Set(setting) => {
                tx.execute(
                    "INSERT INTO metadata(key, value) VALUES(?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    [key, setting.as_str()],
                )?;
            }
            Change::Reset => {
                tx.execute("DELETE FROM metadata WHERE key = ?1", [key])?;
            }
        }
        link_index::bump_revision(&tx)?;
        tx.commit()?;
    }
    Ok(Outcome {
        workspace,
        rule: rule.to_string(),
        change,
        written,
    })
}

/// The `zt config show` policy lines: the archive they belong to, the policy
/// hash and revision, the write-gate stage, every effective override, and their
/// count (R0118).
pub(crate) fn summary_lines(
    root: &Path,
    policies: &PolicySet,
    revision: u64,
    stage: Stage,
) -> Vec<String> {
    let mut lines = vec![
        format!("policy_archive={}", root.display()),
        format!("policy_hash={}", policies.hash()),
        format!("policy_revision={revision}"),
        format!("gate_stage={}", stage.id()),
        // The stable check contract a client compares with its own (S06, R0197).
        format!("rule_version={RULE_VERSION}"),
        format!(
            "diagnostic_schema={}",
            zt_check::contract::DIAGNOSTIC_SCHEMA
        ),
    ];
    for (workspace, rule, setting) in policies.overrides() {
        lines.push(format!(
            "{}={setting}",
            policy::gate_key(workspace, rule)
        ));
    }
    let count = policies.overrides().len();
    lines.push(format!(
        "overrides: {count} (a missing override means the checker's default; `zt config policy` shows every rule)"
    ));
    lines
}

/// The `zt config policy` table: every catalogued rule, its effective setting in
/// both Workspaces, the scope and the state. A `*` marks an explicit archive
/// override. The write gate's stage is named before the table, so a `block`
/// setting is never mistaken for an active refusal while the archive is
/// advisory.
pub(crate) fn table_lines(policies: &PolicySet, stage: Stage) -> Vec<String> {
    let mut lines = vec![
        "rule  human   agent   scope  state".to_string(),
    ];
    for spec in policy::catalog() {
        let cell = |workspace: GateWorkspace| {
            if spec.state == policy::RuleState::Mandatory {
                return "-".to_string();
            }
            let policy = policies.policy(workspace);
            let setting = policy
                .effective(spec.rule)
                .expect("every catalogued rule has a setting");
            // An alias has no separate override: its setting is the named rule's.
            if spec.alias_of.is_none() && policy.is_override(spec.rule) {
                format!("{setting}*")
            } else {
                setting.as_str().to_string()
            }
        };
        let state = match spec.alias_of {
            Some(target) => format!("alias {target}"),
            None => spec.state.as_str().to_string(),
        };
        lines.push(format!(
            "{:<5} {:<7} {:<7} {:<6} {}",
            spec.rule,
            cell(GateWorkspace::Human),
            cell(GateWorkspace::Agent),
            spec.scope.as_str(),
            state
        ));
    }
    lines.push(format!(
        "gate_stage={} ({}; promote a deliberate enforcement with `zt config set gate.stage enforce`)",
        stage.id(),
        match stage {
            Stage::Advisory => "every quality diagnostic is advice; mandatory invariants still refuse",
            Stage::Enforce => "the effective policy decides a write",
        }
    ));
    lines.push(
        "* = an explicit archive override; every other value is the checker's default".to_string(),
    );
    lines.push(
        "pending = catalogued for a later slice; not enforced and not configurable".to_string(),
    );
    lines.push(
        "mandatory = an existing service invariant; no gate policy can change it".to_string(),
    );
    lines.push(
        "alias <rule> = the same predicate under another label; configure the named rule".to_string(),
    );
    lines.push(format!(
        "invariants: {}",
        policy::invariants()
            .iter()
            .map(|invariant| invariant.name)
            .collect::<Vec<_>>()
            .join(", ")
    ));
    lines
}

// ---------------------------------------------------------------------------------------------
// S04: the write gate
// ---------------------------------------------------------------------------------------------

/// The stable rule contract this service links: the `rule_version` of an accepted write's check
/// block, so a client knows which rules decided. It is independent of the executable version
/// (design 8.3, R0197); `zt_check_version` names the build that carries it.
pub(crate) const RULE_VERSION: &str = zt_check::contract::RULE_VERSION;

/// The most distinct Link targets one proposed Card may name. The bound is the checker's own bulk
/// read unit; a Card over it is refused as an incomplete check, never silently truncated (R0108).
pub(crate) const TARGET_BUDGET: usize = 1024;

/// What kind of Card is being written: a root or a Regular Card.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Topic,
    Literature,
    Regular,
}

impl Kind {
    pub(crate) fn of(card: &Card) -> Self {
        if card.is_topic {
            Kind::Topic
        } else if card.is_lit {
            Kind::Literature
        } else {
            Kind::Regular
        }
    }

    fn id(self) -> &'static str {
        match self {
            Kind::Topic => "topic",
            Kind::Literature => "literature",
            Kind::Regular => "regular",
        }
    }
}

/// One fully resolved proposed Card: the final text and metadata, the Workspace that decides its
/// policy, and the stored version when there is one (R0153). A create carries no `before`, and its
/// address is calculated without allocating anything.
#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    pub(crate) address: String,
    pub(crate) kind: Kind,
    pub(crate) workspace: Workspace,
    pub(crate) text: String,
    pub(crate) card_type: CardType,
    pub(crate) facet: Option<Facet>,
    pub(crate) emphasis: Option<Emphasis>,
    pub(crate) level: Option<Level>,
    pub(crate) hub_level: Option<crate::hub::HubLevel>,
    pub(crate) form: Option<crate::math::MathForm>,
    /// The Term Card's exact name, or `None` for a candidate Term list (S07, #149).
    pub(crate) term: Option<String>,
    pub(crate) before: Option<Card>,
}

impl Candidate {
    /// A proposed create: a Topic or Literature root, or a new Regular Card.
    pub(crate) fn create(
        address: &str,
        kind: Kind,
        workspace: Workspace,
        text: &str,
        card_type: CardType,
        part: Option<&crate::model::Part>,
    ) -> Self {
        Self {
            address: address.to_string(),
            kind,
            workspace,
            text: text.to_string(),
            card_type,
            facet: part.map(|part| part.facet.clone()),
            emphasis: part.and_then(|part| part.emphasis),
            level: None,
            hub_level: None,
            form: None,
            term: None,
            before: None,
        }
    }

    /// Give the proposed Card a Term Card's exact name. The name is part of the checked input, so
    /// the checker sees the Card that will be stored.
    pub(crate) fn with_term(mut self, term: Option<&str>) -> Self {
        self.term = term.map(str::to_string);
        self
    }

    /// A proposed edit of a stored Card: the final text and the final metadata together (R0160).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn edit(
        card: &Card,
        text: &str,
        card_type: CardType,
        facet: Option<Facet>,
        emphasis: Option<Emphasis>,
        level: Option<Level>,
        hub_level: Option<crate::hub::HubLevel>,
        form: Option<crate::math::MathForm>,
    ) -> Self {
        Self {
            address: card.address.clone(),
            kind: Kind::of(card),
            workspace: card.workspace,
            text: text.to_string(),
            card_type,
            facet,
            emphasis,
            level,
            hub_level,
            form,
            // An omitted term field preserves the stored name; the shared edit path overrides this
            // with the final resolved value.
            term: card.term_name.clone(),
            before: Some(card.clone()),
        }
    }

    /// The stored Card as a candidate of its own, for the old side of a legacy comparison.
    fn stored(card: &Card) -> Self {
        Self {
            address: card.address.clone(),
            kind: Kind::of(card),
            workspace: card.workspace,
            text: card.text.clone(),
            card_type: card.card_type,
            facet: card.facet.clone(),
            emphasis: card.emphasis,
            level: card.level,
            hub_level: card.hub_level,
            form: card.form,
            term: card.term_name.clone(),
            before: None,
        }
    }
}

/// The longest authored fragment a public reply carries in one diagnostic field. Identity and
/// context stay exact inside the operation; a transport reply is bounded so one pathological
/// sentence cannot exceed the MCP result limit (R0108, R0114). A clipped field says so.
pub(crate) const PUBLIC_TEXT_LIMIT: usize = 2000;

/// One diagnostic as the public reply carries it: the eight published fields with a bounded
/// `message` and `key`, marked `truncated` when either was longer.
fn public_diagnostic(diagnostic: &Diagnostic) -> Value {
    let mut value = diagnostic.to_json();
    let mut truncated = false;
    for field in ["message", "key"] {
        let Some(text) = value[field].as_str().map(str::to_string) else {
            continue;
        };
        if text.chars().count() > PUBLIC_TEXT_LIMIT {
            let clipped = text.chars().take(PUBLIC_TEXT_LIMIT - 1).collect::<String>() + "…";
            value[field] = json!(clipped);
            truncated = true;
        }
    }
    if truncated {
        value["truncated"] = json!(true);
    }
    value
}

/// The warning objects of an accepted write (R0171).
fn warning(diagnostic: &Diagnostic) -> Value {
    let public = public_diagnostic(diagnostic);
    json!({
        "rule": public["rule"],
        "fault": public["fault"],
        "message": public["message"],
        "legacy": public["legacy"],
    })
}

/// The check facts of one accepted write: its warnings and the public contract block (R0170).
#[derive(Clone, Debug, Default)]
pub(crate) struct Accepted {
    report: Option<CheckReport>,
    policy_hash: String,
    stage: Option<Stage>,
}

impl Accepted {
    /// No check ran: an operation that changed no authored content.
    pub(crate) fn none() -> Self {
        Self::default()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.report.is_none()
    }

    /// The warning lines in the checker's traversal order, for a shell diagnostic stream or a
    /// Session notice.
    pub(crate) fn lines(&self) -> Vec<String> {
        self.report
            .as_ref()
            .map(CheckReport::lines)
            .unwrap_or_default()
    }

    /// Add the operation-owned warnings and check contract to a JSON result, keeping every field
    /// the reply already had (R0170). A write that ran no check adds neither.
    pub(crate) fn attach(&self, value: &mut Value) {
        let Some(report) = &self.report else {
            return;
        };
        value["warnings"] = Value::Array(
            report
                .diagnostics
                .iter()
                .map(warning)
                .collect(),
        );
        value["check"] = json!({
            "complete": true,
            "rule_version": RULE_VERSION,
            "policy_hash": self.policy_hash,
            "stage": self.stage.map(Stage::id),
            // The build that carries the rule version (R0197, R0205).
            "checker_version": zt_check::VERSION,
        });
    }

    /// The reply of a write whose result is only its address.
    pub(crate) fn value(&self, address: &str) -> Value {
        let mut value = json!({ "address": address });
        self.attach(&mut value);
        value
    }
}

/// A blocking check result: the typed `card_check` refusal of design 7.3 (R0172–R0175). It is an
/// error value, so the ordinary transaction rollback leaves no Card, allocation, receipt or
/// revision behind.
#[derive(Clone, Debug)]
pub(crate) struct Refusal {
    pub(crate) address: String,
    pub(crate) report: CheckReport,
    pub(crate) policy_hash: String,
    /// The batch item identity when a real batch item raised this refusal (R0175).
    item: Option<Value>,
}

impl Refusal {
    /// The structured reply: the error code, the address, the blocker count, the ordered fault
    /// lines, every structured diagnostic, and the versions that decided.
    pub(crate) fn value(&self) -> Value {
        let mut value = json!({
            "error": "card_check",
            "address": self.address,
            "blocking": self.report.blockers,
            "faults": self.report.lines(),
            "diagnostics": self.report.diagnostics.iter().map(public_diagnostic).collect::<Vec<_>>(),
            "zt_check_version": zt_check::VERSION,
            "rule_version": RULE_VERSION,
            "policy_hash": self.policy_hash,
            "message": self.to_string(),
        });
        if let Some(item) = &self.item {
            value["item"] = item.clone();
        }
        value
    }

    /// Identify the zero-based batch item that raised this refusal, without forcing a client to
    /// parse the human message (R0175).
    pub(crate) fn in_batch(mut self, index: usize, tool: &str, op_key: &str) -> Self {
        self.item = Some(json!({ "index": index, "tool": tool, "op_key": op_key }));
        self
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(item) = &self.item {
            write!(
                f,
                "batch item {} ({}): ",
                item["index"].as_u64().unwrap_or_default(),
                item["tool"].as_str().unwrap_or_default()
            )?;
        }
        write!(
            f,
            "card_check: {} blocking {}",
            self.report.blockers,
            if self.report.blockers == 1 {
                "fault"
            } else {
                "faults"
            }
        )?;
        for line in self.report.lines() {
            write!(f, "\n{line}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Refusal {}

/// A check that could not be completed: an unavailable policy or context, or a context over its
/// explicit budget. It refuses the write instead of pretending the Card passed (R0106, R0108,
/// S04-AC07).
#[derive(Clone, Debug)]
pub(crate) struct Incomplete {
    pub(crate) address: String,
    pub(crate) reason: &'static str,
    pub(crate) detail: String,
}

impl Incomplete {
    pub(crate) fn value(&self) -> Value {
        json!({
            "error": "check_incomplete",
            "address": self.address,
            "reason": self.reason,
            "message": self.to_string(),
            "zt_check_version": zt_check::VERSION,
            "rule_version": RULE_VERSION,
        })
    }
}

impl std::fmt::Display for Incomplete {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "check_incomplete: {}", self.detail)
    }
}

impl std::error::Error for Incomplete {}

/// The bounded context one proposed Card needs: its parent and tree, the book it belongs to and
/// that book's term table, its model's setting and level, the distinct targets it names, and the
/// Term facts the T rules read (R0098–R0108; design 11.3). Each family is either present or
/// legitimately absent.
#[derive(Clone, Debug)]
struct Facts {
    tree: String,
    parent: String,
    book: Option<String>,
    category: String,
    level: String,
    terms: Option<String>,
    targets: BTreeSet<String>,
    term: TermFacts,
}

/// The Term facts of one proposed Card (S09, #152): its computed role and list, the exact rows of
/// that list, the head and the head's category for a part, and the names or wrong roles of the
/// Term-link targets its body names.
#[derive(Clone, Debug, Default)]
struct TermFacts {
    role: Option<String>,
    list: Option<String>,
    rows: Vec<String>,
    head: Option<String>,
    head_category: Option<String>,
    targets: BTreeMap<String, String>,
    invalid_targets: Vec<String>,
}

impl Facts {
    fn new(address: &str) -> Self {
        Self {
            tree: crate::tree_id(address).to_string(),
            parent: crate::parent_location(address).unwrap_or_default(),
            book: None,
            category: "idea".to_string(),
            level: String::new(),
            terms: None,
            targets: BTreeSet::new(),
            term: TermFacts::default(),
        }
    }
}

/// Check one proposed Card and either accept it with its warnings or refuse it. This is the shared
/// decision function of every authoring route (R0152): the caller has already enforced Access and
/// Locks, and the same write transaction supplies the context.
pub(crate) fn check_proposed(conn: &Connection, candidate: &Candidate) -> Result<Accepted> {
    let parsed = crate::validate_card_text(
        conn,
        &candidate.address,
        &candidate.text,
        candidate.before.as_ref().map(|card| card.text.as_str()),
    )?;
    let policies = match load(conn) {
        Ok(policies) => policies,
        Err(error) => {
            return Err(anyhow::Error::new(Incomplete {
                address: candidate.address.clone(),
                reason: "policy",
                detail: format!("the archive's Card-check policy is unavailable: {error:#}"),
            }));
        }
    };
    let stage = match current_stage(conn) {
        Ok(stage) => stage,
        Err(error) => {
            return Err(anyhow::Error::new(Incomplete {
                address: candidate.address.clone(),
                reason: "stage",
                detail: format!("the archive's Card-check stage is unavailable: {error:#}"),
            }));
        }
    };
    let facts = gather(conn, candidate, &parsed)?;
    let proposed = entry(candidate, &parsed, &facts);
    let proposed_report = check_with(&facts, &proposed, &policies)?;
    let mut report = match &candidate.before {
        None => CheckReport::of(proposed_report),
        Some(before) => {
            let stored = Candidate::stored(before);
            let old_parsed = crate::parse_card_text(&before.text)?;
            let old_facts = gather(conn, &stored, &old_parsed)?;
            let old_entry = entry(&stored, &old_parsed, &old_facts);
            let mut old_report = check_with(&old_facts, &old_entry, &policies)?;
            // A `before` entry is this Card's own old version even when the proposal names a new
            // address: pair the old diagnostics with the proposed address, so a pure remap does
            // not make an unchanged fault new (design 6.3, R0149).
            for diagnostic in &mut old_report {
                diagnostic.address = candidate.address.clone();
            }
            let old_applicability = Applicability::of(&old_entry);
            let proposed_applicability = Applicability::of(&proposed);
            policy::ratchet(&old_report, proposed_report, |diagnostic| {
                old_applicability.comparable(&proposed_applicability, &diagnostic.rule)
            })
        }
    };
    // The rollout stage: an advisory archive reports every quality diagnostic and refuses none;
    // an enforcing archive keeps the policy's blockers (design §17, S04-AC04).
    if stage == Stage::Advisory {
        for diagnostic in &mut report.diagnostics {
            if diagnostic.severity == Severity::Block {
                diagnostic.severity = Severity::Advise;
            }
        }
        report.blockers = 0;
    }
    let policy_hash = policies.hash();
    if report.blockers > 0 {
        return Err(anyhow::Error::new(Refusal {
            address: candidate.address.clone(),
            report,
            policy_hash,
            item: None,
        }));
    }
    Ok(Accepted {
        report: Some(report),
        policy_hash,
        stage: Some(stage),
    })
}

/// Run the core rules on one entry under its Workspace's policy and the entry's term table.
fn check_with(
    facts: &Facts,
    entry: &Value,
    policies: &PolicySet,
) -> Result<Vec<Diagnostic>> {
    let check = Check::new(Limits::default(), false, "").with_terms(facts.terms.as_deref());
    let diagnostics = check.report(std::slice::from_ref(entry));
    let entries = [entry.clone()];
    Ok(policies.apply(&entries, diagnostics))
}

/// The proposed entry in the checker's existing JSON Card shape (R0094, R0097).
fn entry(candidate: &Candidate, parsed: &crate::ParsedCard, facts: &Facts) -> Value {
    let links = link_targets(&parsed.body);
    let relations = relations(&parsed.body);
    let mut value = json!({
        "address": candidate.address,
        "title": parsed.title,
        "body": parsed.body,
        "type": candidate.card_type.id(),
        "facet": candidate.facet.as_ref().map(Facet::id),
        "category": facts.category,
        "workspace": candidate.workspace.id(),
        "tree": facts.tree,
        "parent": facts.parent,
        "links": links,
        "kinds": relations.keys().cloned().collect::<Vec<_>>(),
        "relations": relations,
    });
    if candidate.workspace == Workspace::Agent
        && let Some(book) = &facts.book
    {
        value["book"] = json!(book);
    }
    if !facts.level.is_empty() {
        value["level"] = json!(facts.level);
    }
    // The resolved type fields travel with the entry, so a later rule that reads one needs no
    // second candidate family (R0097). The current rules read none of them.
    if let Some(emphasis) = candidate.emphasis {
        value["emphasis"] = json!(emphasis.id());
    }
    if let Some(hub_level) = candidate.hub_level {
        value["hub_level"] = json!(hub_level.id());
    }
    if let Some(form) = candidate.form {
        value["form"] = json!(form.id());
    }
    // A Term Card's exact name travels with the checked entry (S07, #149): the pending T rules
    // read it, and the current rules still ignore it.
    if let Some(term) = &candidate.term {
        value["term"] = json!(term);
    }
    // The bounded Term context the T rules read (S09, #152).
    if let Some(role) = &facts.term.role {
        value["term_role"] = json!(role);
    }
    if let Some(list) = &facts.term.list {
        value["term_list"] = json!(list);
    }
    if !facts.term.rows.is_empty() {
        value["term_rows"] = json!(facts.term.rows);
    }
    if let Some(head) = &facts.term.head {
        value["term_head"] = json!(head);
    }
    if let Some(category) = &facts.term.head_category {
        value["term_head_category"] = json!(category);
    }
    if !facts.term.targets.is_empty() {
        value["term_targets"] = json!(facts.term.targets);
    }
    if !facts.term.invalid_targets.is_empty() {
        value["term_invalid_targets"] = json!(facts.term.invalid_targets);
    }
    value
}

/// The distinct Link targets a body names, in order.
fn link_targets(body: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut targets = Vec::new();
    if let Ok(parsed) = crate::link::LinkBody::parse(body) {
        for occurrence in parsed.occurrences() {
            if seen.insert(occurrence.target.to_string()) {
                targets.push(occurrence.target.to_string());
            }
        }
    }
    targets
}

/// The targets a body links with a relation word, by their kind: the checker's `relations` map. A
/// Term use is no reasoning kind (S16, #158; design 10.2), so the checker never counts it as an
/// announced relation and never reports it as an unkinded Link.
fn relations(body: &str) -> serde_json::Map<String, Value> {
    let mut relations = serde_json::Map::new();
    for read in crate::model::read_links(body, str::to_string) {
        if read.kind.names_a_relation() {
            relations
                .entry(read.target)
                .or_insert_with(|| Value::String(read.kind.id().to_string()));
        }
    }
    relations
}

/// Gather the bounded context with the transaction connection: ancestry, model setting/level,
/// book identity and term table, and the distinct target count (R0107). A family that a rule needs
/// but cannot read is an incomplete check, not a default.
fn gather(
    conn: &Connection,
    candidate: &Candidate,
    parsed: &crate::ParsedCard,
) -> Result<Facts> {
    let mut facts = Facts::new(&candidate.address);
    facts.targets = link_targets(&parsed.body).into_iter().collect();
    if facts.targets.len() > TARGET_BUDGET {
        return Err(anyhow::Error::new(Incomplete {
            address: candidate.address.clone(),
            reason: "context_budget",
            detail: format!(
                "the proposed Card names {} distinct Link targets; one write is limited to {TARGET_BUDGET}",
                facts.targets.len()
            ),
        }));
    }
    facts.book = zt_check::text::literature_tree(&facts.tree);
    let head = head_model(conn, candidate)?;
    facts.level = match (&candidate.facet, &head) {
        (Some(_), Some(head)) => crate::load_card(conn, head)
            .map(|card| card.level.map(Level::id).unwrap_or_default().to_string())
            .unwrap_or_default(),
        (Some(_), None) => String::new(),
        (None, _) => candidate.level.map(Level::id).unwrap_or_default().to_string(),
    };
    facts.terms = term_table(conn, candidate, parsed, head.as_deref(), facts.book.as_deref())?;
    facts.term = term_facts(conn, candidate, parsed)?;
    facts.category = category(conn, candidate, parsed, &facts)?;
    Ok(facts)
}

/// Gather the bounded Term context of one proposed Card (design 11.2–11.3): the computed role
/// and list, the list's exact rows, the head and its category for a part, and the name or role
/// of every Term-link target the body names. A Card of another type gathers nothing.
fn term_facts(
    conn: &Connection,
    candidate: &Candidate,
    parsed: &crate::ParsedCard,
) -> Result<TermFacts> {
    let mut facts = TermFacts::default();
    if candidate.card_type == CardType::Term {
        if let Some(name) = candidate.term.as_deref() {
            let lookup = |at: &str| -> Result<Option<(CardType, Option<String>)>> {
                Ok(load_optional(conn, at)?.map(|card| (card.card_type, card.term_name)))
            };
            let placed = crate::term::place(&candidate.address, name, &lookup)?;
            facts.role = Some(
                placed
                    .as_ref()
                    .map(|placed| placed.role.id())
                    .unwrap_or("unfiled")
                    .to_string(),
            );
            if let Some(placed) = &placed {
                facts.list = Some(placed.list.clone());
                let list_body = if placed.list == candidate.address {
                    parsed.body.clone()
                } else {
                    let list = crate::load_card(conn, &placed.list)?;
                    crate::parse_card_text(&list.text)?.body
                };
                facts.rows = zt_check::text::term_table(&list_body)
                    .into_iter()
                    .map(|row| row.term)
                    .collect();
                if placed.role == crate::term::Role::Part {
                    facts.head = Some(placed.head.clone());
                    let head = crate::load_card(conn, &placed.head)?;
                    let head_body = crate::parse_card_text(&head.text)?.body;
                    facts.head_category = zt_check::term::parse_template(&head_body)
                        .line("类别")
                        .map(|line| line.value.clone());
                }
            }
        } else {
            facts.role = Some("list".to_string());
        }
    }
    // The Term-link targets the proposed body names (S09, #152; T7): their exact names when they
    // are heads, and the retained legacy targets that are not. A new invalid target was already
    // refused by the Link write invariant; this context only lets the checker report advice. It
    // applies to a Term use on any Card, not only a Term Card.
    let mut seen = BTreeSet::new();
    for occurrence in crate::link::LinkBody::for_rendering(&parsed.body).occurrences() {
        if occurrence.form != crate::link::LinkForm::Term {
            continue;
        }
        let target = occurrence.target.to_string();
        if !seen.insert(target.clone()) {
            continue;
        }
        if crate::term::is_head(conn, &target)? {
            let card = crate::load_card(conn, &target)?;
            if let Some(name) = card.term_name {
                facts.targets.insert(target, name);
            }
        } else {
            facts.invalid_targets.push(target);
        }
    }
    Ok(facts)
}

/// The Model Card a proposed part belongs to: itself for a head, else the nearest Model Card with
/// no facet that is a segment-boundary prefix of its address. One indexed query replaces the
/// parent-chain walk: a legitimate long Direct-successor chain (a Topic with hundreds of Cards)
/// must not be mistaken for unbounded context.
fn head_model(conn: &Connection, candidate: &Candidate) -> Result<Option<String>> {
    if candidate.card_type == CardType::Model && candidate.facet.is_none() {
        return Ok(Some(candidate.address.clone()));
    }
    if candidate.facet.is_none() {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            "SELECT COALESCE(location, citation_key) FROM cards
             WHERE card_type = 'model' AND model_facet IS NULL
               AND (COALESCE(location, citation_key) = ?1
                    OR ?1 GLOB (COALESCE(location, citation_key) || '/*')
                    OR ?1 GLOB (COALESCE(location, citation_key) || '|*'))
             ORDER BY length(COALESCE(location, citation_key)) DESC
             LIMIT 1",
            [candidate.address.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?)
}

/// The current body of a Card, or `None` when no Card is at that address.
fn load_optional(conn: &Connection, address: &str) -> Result<Option<Card>> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT row_id, COALESCE(location, citation_key), citation_key,
                is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis, model_level, hub_level, math_form, term_name
         FROM cards WHERE location = ?1 OR citation_key = ?1",
        [address],
        crate::card_from_row,
    )
    .optional()
    .map_err(Into::into)
}

/// The applicable term table: a model's setting Card (the first Side successor of its Model Card,
/// `<head>|a`) for a model, else the Book's `术语表` Card (a Side successor of `<key>/1`) for a
/// Card in a Literature tree. Absent when the archive has no such Card; the proposed text itself
/// when that Card is the one being written.
fn term_table(
    conn: &Connection,
    candidate: &Candidate,
    parsed: &crate::ParsedCard,
    head: Option<&str>,
    book: Option<&str>,
) -> Result<Option<String>> {
    let mut addresses = Vec::new();
    if let Some(head) = head {
        addresses.push(format!("{head}|a"));
    } else if let Some(book) = book {
        let pattern = format!("{book}/1|%");
        let mut statement =
            conn.prepare("SELECT COALESCE(location, citation_key) FROM cards WHERE location LIKE ?1 ORDER BY location")?;
        let rows = statement.query_map([pattern], |row| row.get::<_, String>(0))?;
        for row in rows {
            addresses.push(row?);
        }
    }
    for address in addresses {
        if address == candidate.address {
            if parsed.title.trim_start().starts_with("术语表") || head.is_some() {
                return Ok(Some(parsed.body.clone()));
            }
            continue;
        }
        let Some(card) = load_optional(conn, &address)? else {
            continue;
        };
        let card_parsed = crate::parse_card_text(&card.text)?;
        if head.is_some() || card_parsed.title.trim_start().starts_with("术语表") {
            return Ok(Some(card_parsed.body));
        }
    }
    Ok(None)
}

/// The shared Card classification (D086, D188), computed from the proposed metadata and bounded
/// archive facts: the candidate's own tree and, inside a Literature tree, its anchor count.
fn category(
    conn: &Connection,
    candidate: &Candidate,
    parsed: &crate::ParsedCard,
    facts: &Facts,
) -> Result<String> {
    let mut literature = std::collections::HashSet::new();
    let mut anchors = HashMap::new();
    if let Some(book) = &facts.book {
        literature.insert(book.clone());
        let pattern = format!("{book}/%");
        let mut statement =
            conn.prepare("SELECT COALESCE(location, citation_key) FROM cards WHERE location LIKE ?1")?;
        let rows = statement.query_map([pattern], |row| row.get::<_, String>(0))?;
        let own = format!("{book}/");
        let mut count = 0usize;
        for row in rows {
            let address = row?;
            let rest = address.strip_prefix(&own).unwrap_or_default();
            if !rest.is_empty() && rest.chars().all(|character| character.is_ascii_digit()) {
                count += 1;
            }
        }
        anchors.insert(book.clone(), count);
    }
    let card = json!({
        "address": candidate.address,
        "kind": candidate.kind.id(),
        "type": candidate.card_type.id(),
        "term": candidate.term,
        "facet": candidate.facet.as_ref().map(Facet::id),
        "tree": facts.tree,
        "title": parsed.title,
        "body": parsed.body,
        "excerpt": crate::ops::excerpt(&parsed.body),
        "parent": facts.parent,
        "edge": edge(candidate),
        "links": facts.targets.iter().collect::<Vec<_>>(),
    });
    Ok(zt_check::core::category_with_parts(
        &card,
        &literature,
        &anchors,
        &std::collections::HashSet::new(),
    ))
}

/// The edge a Card has at its Location: a root, a Side successor, or a Direct successor.
fn edge(candidate: &Candidate) -> &'static str {
    if candidate.kind != Kind::Regular {
        "root"
    } else if crate::valid_side_label(crate::last_segment(&candidate.address)) {
        "side"
    } else {
        "direct"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zt_check::policy::Setting;

    /// A fresh isolated archive, as every test suite's own scratch box.
    fn archive() -> (tempfile::TempDir, Connection) {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        (temp, conn)
    }

    fn overrides(conn: &Connection) -> Vec<(String, String)> {
        rows(conn).expect("override rows")
    }

    /// S03-AC01/S03-AC02: one override is one metadata row in the selected
    /// archive; setting or resetting it moves the revision once, and every
    /// refusal writes no row and moves no revision.
    #[test]
    fn an_override_is_persisted_in_the_metadata_and_moves_the_revision() {
        let (_temp, mut conn) = archive();
        let before = revision(&conn).expect("revision");
        let default_hash = load(&conn).expect("policy").hash();
        assert!(overrides(&conn).is_empty());
        assert_eq!(
            load(&conn)
                .expect("policy")
                .policy(GateWorkspace::Human)
                .effective("R2"),
            Some(Setting::Advise),
            "the human default is advice"
        );

        let outcome = change(&mut conn, "gate.human.R2", "block").expect("set");
        assert_eq!(outcome.line(), "gate.human.R2=block");
        assert!(outcome.written);
        assert_eq!(
            overrides(&conn),
            [("gate.human.R2".to_string(), "block".to_string())]
        );
        assert_eq!(revision(&conn).expect("revision"), before + 1);
        let set = load(&conn).expect("policy");
        assert_eq!(
            set.policy(GateWorkspace::Human).effective("R2"),
            Some(Setting::Block)
        );
        assert_ne!(set.hash(), default_hash, "a policy change gets a new hash");
        assert_eq!(
            set.policy(GateWorkspace::Agent).effective("R2"),
            Some(Setting::Block),
            "the other Workspace keeps its own default"
        );

        // A repeated value writes nothing and moves no revision.
        let outcome = change(&mut conn, "gate.human.R2", "block").expect("repeat");
        assert!(!outcome.written);
        assert_eq!(revision(&conn).expect("revision"), before + 1);

        // An explicit reset removes the row and moves the revision once.
        let outcome = change(&mut conn, "gate.human.R2", "default").expect("reset");
        assert_eq!(outcome.line(), "gate.human.R2=default");
        assert!(outcome.written);
        assert!(overrides(&conn).is_empty());
        assert_eq!(revision(&conn).expect("revision"), before + 2);
        assert_eq!(load(&conn).expect("policy").hash(), default_hash);

        // A refused change writes no row and moves no revision.
        let cases = [
            ("gate.human.R2", "warn"),
            ("gate.human.R1", "block"),
            ("gate.human.access", "off"),
            ("gate.human.envelope", "off"),
            ("gate.human.T7", "block"),
            ("gate.human.T10", "block"),
            ("gate.human.T8", "block"),
            ("gate.aliens.R2", "block"),
            ("gate.agent.I5", "block"),
        ];
        for (key, value) in cases {
            let error = change(&mut conn, key, value)
                .expect_err(&format!("{key}={value} must be refused"))
                .to_string();
            assert!(!error.is_empty(), "{key}");
            assert!(overrides(&conn).is_empty(), "{key} wrote a row");
            assert_eq!(
                revision(&conn).expect("revision"),
                before + 2,
                "{key} moved the revision"
            );
        }
        let error = change(&mut conn, "gate.human.R1", "block")
            .expect_err("unknown rule")
            .to_string();
        assert!(error.contains("unknown rule `R1`"), "{error}");
        let error = change(&mut conn, "gate.human.access", "off")
            .expect_err("invariant")
            .to_string();
        assert!(error.contains("mandatory service invariant"), "{error}");
        let error = change(&mut conn, "gate.agent.I5", "block")
            .expect_err("graph block")
            .to_string();
        assert!(error.contains("graph-only audit rule"), "{error}");

        // A graph rule may be silenced and restored without any graph scan.
        let outcome = change(&mut conn, "gate.agent.I5", "off").expect("silence a graph rule");
        assert!(outcome.written);
        assert_eq!(
            load(&conn)
                .expect("policy")
                .policy(GateWorkspace::Agent)
                .effective("I5"),
            Some(Setting::Off)
        );
        change(&mut conn, "gate.agent.I5", "default").expect("restore");
    }

    /// S03-AC05: two archives hold their own rows and revision; changing one
    /// changes neither the other's policy nor its revision.
    #[test]
    fn two_archives_keep_their_own_policy_and_revision() {
        let (_a_temp, mut a) = archive();
        let (_b_temp, b) = archive();
        let (a_revision, b_revision) = (
            revision(&a).expect("revision"),
            revision(&b).expect("revision"),
        );
        let b_hash = load(&b).expect("policy").hash();
        change(&mut a, "gate.agent.R6", "off").expect("set a");
        assert_eq!(overrides(&a).len(), 1);
        assert!(overrides(&b).is_empty());
        assert_eq!(load(&b).expect("policy").hash(), b_hash);
        assert_eq!(revision(&b).expect("revision"), b_revision);
        assert_eq!(revision(&a).expect("revision"), a_revision + 1);
    }

    /// S03-AC07/R0120: a policy change rewrites no Card and saves no legacy
    /// baseline. It writes the metadata row and advances the revision, nothing
    /// else.
    #[test]
    fn a_policy_change_rewrites_no_card_and_saves_no_baseline() {
        let (_temp, mut conn) = archive();
        conn.execute(
            "INSERT INTO cards(location, is_topic, is_lit, text, workspace, card_type)
             VALUES('1/0', 1, 0, '# 主题\n\n正文。\n', 'agent', 'idea')",
            [],
        )
        .expect("insert a Card");
        let count = |conn: &Connection| -> i64 {
            conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
                .expect("count")
        };
        let cards = count(&conn);
        let revision_before = revision(&conn).expect("revision");
        change(&mut conn, "gate.agent.R6", "block").expect("set");
        assert_eq!(count(&conn), cards, "no Card is created or removed");
        let text: String = conn
            .query_row("SELECT text FROM cards WHERE location = '1/0'", [], |row| row.get(0))
            .expect("stored text");
        assert_eq!(text, "# 主题\n\n正文。\n", "no Card is rewritten");
        assert_eq!(revision(&conn).expect("revision"), revision_before + 1);
        let baselines: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
                 AND (name LIKE '%baseline%' OR name LIKE '%fault%')",
                [],
                |row| row.get(0),
            )
            .expect("schema");
        assert_eq!(baselines, 0, "no legacy-fault baseline table is created");
        let gate_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM metadata WHERE key LIKE 'gate.%'", [], |row| row.get(0))
            .expect("metadata");
        assert_eq!(gate_rows, 1, "one metadata row holds the override");
    }

    /// S03-AC01/R0118: the config output names the archive, the policy hash, the
    /// revision, and every effective override; the full table shows the defaults,
    /// the implemented Term rules, the mandatory rule, and the graph scope.
    #[test]
    fn the_policy_output_names_the_archive_its_overrides_and_the_defaults() {
        let (_temp, mut conn) = archive();
        change(&mut conn, "gate.agent.R2", "advise").expect("set");
        let policies = load(&conn).expect("policy");
        let summary = summary_lines(
            Path::new("D:/box/archive"),
            &policies,
            revision(&conn).expect("revision"),
            current_stage(&conn).expect("stage"),
        );
        assert_eq!(summary[0], "policy_archive=D:/box/archive");
        let hash = summary
            .iter()
            .find(|line| line.starts_with("policy_hash="))
            .expect("hash");
        assert_eq!(hash.len(), "policy_hash=".len() + 64, "{hash}");
        assert!(
            summary
                .iter()
                .any(|line| line.starts_with("policy_revision=")),
            "{summary:#?}"
        );
        assert!(summary.contains(&"gate.agent.R2=advise".to_string()), "{summary:#?}");
        let rule_line = format!("rule_version={RULE_VERSION}");
        assert!(
            summary.iter().any(|line| line == &rule_line),
            "the summary names the stable rule contract: {summary:#?}"
        );
        assert!(summary.last().expect("footer").starts_with("overrides: 1"), "{summary:#?}");

        let table = table_lines(&policies, Stage::Advisory);
        let header = table.first().expect("header");
        assert!(header.contains("human") && header.contains("agent"), "{header}");
        let row = |rule: &str| {
            table
                .iter()
                .find(|line| line.starts_with(&format!("{rule} ")))
                .unwrap_or_else(|| panic!("{rule} is in the table: {table:#?}"))
                .clone()
        };
        assert!(row("R2").contains("advise*"), "{}", row("R2"));
        assert!(!row("R2").contains("block"), "{}", row("R2"));
        assert!(row("E2").contains("block") && !row("E2").contains('*'), "{}", row("E2"));
        assert!(row("R6").contains("off") && row("R6").contains("advise"), "{}", row("R6"));
        assert!(row("I5").contains("graph"), "{}", row("I5"));
        assert!(
            row("T4").contains("advise") && row("T4").contains("configurable"),
            "{}",
            row("T4")
        );
        assert!(row("T1").contains("block"), "{}", row("T1"));
        assert!(row("T7").contains("mandatory") && !row("T7").contains("block"), "{}", row("T7"));
        assert!(row("T10").contains("alias R22") && !row("T10").contains('*'), "{}", row("T10"));
        assert!(table.iter().any(|line| line.starts_with("invariants: ")), "{table:#?}");
    }
}

// ---------------------------------------------------------------------------------------------
// S04: the write gate's own tests
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod write_gate_tests {
    use super::*;
    use crate::model::{Facet, Level};
    use rusqlite::params;
    use tempfile::TempDir;

    fn archive() -> (TempDir, Connection) {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        (temp, conn)
    }

    fn enforce(conn: &mut Connection) {
        change_stage(conn, "enforce").expect("enforce the write gate");
    }

    fn count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
            .expect("card count")
    }

    fn card_text(conn: &Connection, at: &str) -> String {
        crate::load_card(conn, at).expect("card").text
    }

    /// A stored Card written outside the gate, as an archive's older content.
    fn legacy(conn: &Connection, at: &str, workspace: Workspace, text: &str) {
        conn.execute(
            "INSERT INTO cards(location, is_topic, is_lit, text, workspace, card_type)
             VALUES(?1, 0, 0, ?2, ?3, 'idea')",
            params![at, text, workspace.id()],
        )
        .expect("legacy card");
    }

    fn regular(at: &str, workspace: Workspace, text: &str) -> Candidate {
        Candidate::create(at, Kind::Regular, workspace, text, CardType::Idea, None)
    }

    fn refusal(error: anyhow::Error) -> Refusal {
        error
            .downcast::<Refusal>()
            .unwrap_or_else(|other| panic!("expected a card_check refusal, got {other:#}"))
    }

    fn incomplete(error: anyhow::Error) -> Incomplete {
        error
            .downcast::<Incomplete>()
            .unwrap_or_else(|other| panic!("expected an incomplete check, got {other:#}"))
    }

    fn has_rule(refusal: &Refusal, rule: &str) -> bool {
        refusal
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == rule && diagnostic.blocking())
    }

    /// S04-AC04/US015: in the advisory rollout a fault is returned as advice; a deliberate
    /// enforcement makes the same fault a typed refusal that writes nothing.
    #[test]
    fn the_stage_turns_a_reported_fault_into_a_refusal_without_residue() {
        let (_temp, mut conn) = archive();
        crate::create_topic(&mut conn, Workspace::Agent, "# 主题\n\n主题说明。\n").expect("root");
        let candidate = regular("0/1", Workspace::Agent, "# 短的标题\n\n甲；乙。\n");
        let accepted = check_proposed(&conn, &candidate).expect("advisory accepts");
        let value = accepted.value("0/1");
        assert_eq!(value["check"]["stage"], json!("advisory"));
        assert_eq!(value["check"]["complete"], json!(true));
        assert_eq!(value["check"]["rule_version"], json!(RULE_VERSION));
        assert_eq!(value["check"]["checker_version"], json!(zt_check::VERSION));
        assert_eq!(
            value["check"]["policy_hash"].as_str().map(str::len),
            Some(64)
        );
        assert!(
            value["warnings"]
                .as_array()
                .expect("warnings")
                .iter()
                .any(|warning| warning["rule"] == json!("R3") && warning["legacy"] == json!(false)),
            "{value}"
        );

        enforce(&mut conn);
        let before_revision = revision(&conn).expect("revision");
        let before_cards = count(&conn);
        let refusal = refusal(check_proposed(&conn, &candidate).expect_err("enforcing refuses"));
        assert_eq!(refusal.value()["error"], json!("card_check"));
        assert_eq!(refusal.value()["address"], json!("0/1"));
        assert!(refusal.value()["blocking"].as_u64().expect("blocking") >= 1);
        assert!(
            refusal.value()["faults"]
                .as_array()
                .expect("fault lines")
                .iter()
                .any(|line| line.as_str().is_some_and(|line| line.contains(" R3 "))),
            "{refusal:?}"
        );
        assert_eq!(
            refusal.value()["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .len(),
            refusal.report.diagnostics.len()
        );
        assert_eq!(refusal.value()["zt_check_version"], json!(zt_check::VERSION));
        assert_eq!(refusal.value()["rule_version"], json!(RULE_VERSION));
        assert_eq!(refusal.value()["policy_hash"].as_str().map(str::len), Some(64));
        assert!(refusal.to_string().starts_with("card_check: "), "{refusal}");
        assert!(has_rule(&refusal, "R3"), "{refusal:?}");
        assert_eq!(count(&conn), before_cards, "a refused write adds no Card");
        assert_eq!(
            revision(&conn).expect("revision"),
            before_revision,
            "a refused write moves no revision"
        );
    }

    /// S04-AC04/US014: a retained old fault is legacy advice; a second occurrence of the same
    /// fault is new and blocks while the gate is enforcing.
    #[test]
    fn a_legacy_fault_is_advice_and_a_second_copy_blocks() {
        let (_temp, mut conn) = archive();
        enforce(&mut conn);
        let body = "甲；乙。这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。";
        assert!(body.chars().count() >= 40);
        legacy(&conn, "0/1", Workspace::Agent, &format!("# 标题\n\n{body}\n"));
        let card = crate::load_card(&conn, "0/1").expect("card");
        let same = Candidate::edit(&card, &card.text, CardType::Idea, None, None, None, None, None);
        let accepted = check_proposed(&conn, &same).expect("the retained fault is advice");
        let warning = accepted
            .value("0/1")["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .find(|warning| warning["rule"] == json!("R3"))
            .cloned()
            .expect("the retained R3 warning");
        assert_eq!(warning["legacy"], json!(true));

        let text = format!("# 标题\n\n{body}\n\n丙；丁。\n");
        let proposed =
            Candidate::edit(&card, &text, CardType::Idea, None, None, None, None, None);
        let refusal = refusal(check_proposed(&conn, &proposed).expect_err("the new copy blocks"));
        assert!(has_rule(&refusal, "R3"), "{refusal:?}");
        assert_eq!(card_text(&conn, "0/1"), card.text, "the stored Card did not change");
    }

    /// S04-AC03: a retype is checked with the proposed metadata, so an Event body must meet the
    /// Event format even when its Idea body was accepted.
    #[test]
    fn a_retype_must_meet_the_new_types_format() {
        let (_temp, mut conn) = archive();
        enforce(&mut conn);
        let body = "这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。";
        legacy(&conn, "0/1", Workspace::Agent, &format!("# 标题\n\n{body}\n"));
        let card = crate::load_card(&conn, "0/1").expect("card");
        let text = "# 1999 年的一笔交易\n\n这一段说明背景并给出充分理由。\n";
        let proposed = Candidate::edit(
            &card,
            text,
            CardType::Event,
            None,
            None,
            None,
            None,
            None,
        );
        let refusal = refusal(check_proposed(&conn, &proposed).expect_err("E2 blocks"));
        assert!(has_rule(&refusal, "E2"), "{refusal:?}");
    }

    /// S04-AC01/S04-AC02: the effective policy decides, and a policy override changes one
    /// archive's decision without a second writer path.
    #[test]
    fn an_archive_policy_override_decides_an_enforced_write() {
        let (_temp, mut conn) = archive();
        enforce(&mut conn);
        legacy(&conn, "2/1", Workspace::Agent, "# 目标\n\n内容。\n");
        let sentence = "甲".repeat(41);
        let text = format!("# 标题\n\n{sentence}。对应 [[2/1]]：这条理由说明选择。\n");
        let candidate = regular("0/1", Workspace::Agent, &text);
        let refusal = refusal(check_proposed(&conn, &candidate).expect_err("agent R2 blocks"));
        assert!(has_rule(&refusal, "R2"), "{refusal:?}");

        change(&mut conn, "gate.agent.R2", "advise").expect("advise R2");
        let policies = load(&conn).expect("policy");
        let accepted = check_proposed(&conn, &candidate).expect("advice is accepted");
        let value = accepted.value("0/1");
        assert_eq!(value["check"]["policy_hash"], json!(policies.hash()));
        assert!(
            value["warnings"]
                .as_array()
                .expect("warnings")
                .iter()
                .any(|warning| warning["rule"] == json!("R2")),
            "{value}"
        );
        assert_eq!(value["check"]["stage"], json!("enforce"));
        assert_eq!(value["check"]["complete"], json!(true));
    }

    /// S04-AC07/R0106: an unreadable policy is an incomplete check, never a default acceptance.
    #[test]
    fn a_corrupt_policy_row_makes_the_check_incomplete() {
        let (_temp, conn) = archive();
        conn.execute(
            "INSERT INTO metadata(key, value) VALUES('gate.agent.R2', 'warn')",
            [],
        )
        .expect("corrupt row");
        let before_revision = revision(&conn).expect("revision");
        let before_cards = count(&conn);
        let candidate = regular("0/1", Workspace::Agent, "# 标题\n\n说明。\n");
        let error = check_proposed(&conn, &candidate).expect_err("the policy is unavailable");
        let incomplete = incomplete(error);
        assert_eq!(incomplete.value()["error"], json!("check_incomplete"));
        assert_eq!(incomplete.value()["reason"], json!("policy"));
        assert!(incomplete.to_string().starts_with("check_incomplete: "));
        assert_eq!(count(&conn), before_cards);
        assert_eq!(revision(&conn).expect("revision"), before_revision);
    }

    /// S04-AC07/R0108: context over the explicit target budget is refused as incomplete rather
    /// than truncated or turned into an archive-wide scan.
    #[test]
    fn an_over_budget_target_set_makes_the_check_incomplete() {
        let (_temp, mut conn) = archive();
        let tx = conn.transaction().expect("transaction");
        let mut insert = tx
            .prepare(
                "INSERT INTO cards(location, is_topic, is_lit, text, workspace, card_type)
                 VALUES(?1, 0, 0, '# 目标\n\n内容。\n', 'agent', 'idea')",
            )
            .expect("prepare");
        for n in 1..=TARGET_BUDGET + 1 {
            insert
                .execute([format!("1/{n}")])
                .expect("target Card");
        }
        drop(insert);
        tx.commit().expect("commit");
        let links = (1..=TARGET_BUDGET + 1)
            .map(|n| format!("[[1/{n}]]"))
            .collect::<Vec<_>>()
            .join(" ");
        let candidate = regular("0/1", Workspace::Agent, &format!("# 标题\n\n{links}\n"));
        let error = check_proposed(&conn, &candidate).expect_err("the context is too large");
        let incomplete = incomplete(error);
        assert_eq!(incomplete.value()["reason"], json!("context_budget"));
        assert!(
            incomplete.to_string().contains(&TARGET_BUDGET.to_string()),
            "{incomplete}"
        );
        assert_eq!(count(&conn), TARGET_BUDGET as i64 + 1);
    }

    /// S04-AC01/US013: a part's bounded model context reads its head's level, so the formal size
    /// rule applies only when the model is formal.
    #[test]
    fn a_part_reads_its_heads_level_for_the_size_rule() {
        let (_temp, mut conn) = archive();
        enforce(&mut conn);
        crate::create_topic(&mut conn, Workspace::Agent, "# 模型主题\n\n说明。\n").expect("topic");
        crate::insert_card(
            &mut conn,
            "0/1",
            false,
            Workspace::Agent,
            CardType::Model,
            None,
            "# 模型\n\n一个模型。\n",
        )
        .expect("head");
        let head = crate::load_card(&conn, "0/1").expect("head");
        crate::model::set_level(&conn, &head, Some(Level::Concept)).expect("concept");
        let facet = Facet::parse("设定").expect("facet");
        let part = crate::model::Part {
            facet: facet.clone(),
            emphasis: None,
        };
        let body = (1..=13)
            .map(|n| format!("第 {n} 句说明这一步。"))
            .collect::<String>();
        let text = format!("# 设定\n\n{body}");
        let concept = Candidate::create(
            "0/1|a",
            Kind::Regular,
            Workspace::Agent,
            &text,
            CardType::Model,
            Some(&part),
        );
        check_proposed(&conn, &concept).expect("a concept model skips R13");
        let head = crate::load_card(&conn, "0/1").expect("head");
        crate::model::set_level(&conn, &head, Some(Level::Formal)).expect("formal");
        let formal = Candidate::create(
            "0/1|b",
            Kind::Regular,
            Workspace::Agent,
            &text,
            CardType::Model,
            Some(&part),
        );
        let refusal = refusal(check_proposed(&conn, &formal).expect_err("R13 blocks"));
        assert!(has_rule(&refusal, "R13"), "{refusal:?}");
    }

    /// S04-AC02/R0101: a model part is checked against its home model's term table.
    #[test]
    fn a_model_term_table_gates_its_parts() {
        let (_temp, mut conn) = archive();
        enforce(&mut conn);
        crate::create_topic(&mut conn, Workspace::Agent, "# 模型主题\n\n说明。\n").expect("topic");
        crate::insert_card(
            &mut conn,
            "0/1",
            false,
            Workspace::Agent,
            CardType::Model,
            None,
            "# 模型\n\n一个模型。\n",
        )
        .expect("head");
        let head = crate::load_card(&conn, "0/1").expect("head");
        crate::model::set_level(&conn, &head, Some(Level::Formal)).expect("formal");
        let facet = Facet::parse("设定").expect("facet");
        let part = crate::model::Part {
            facet,
            emphasis: None,
        };
        let setting = "# 设定\n\n**术语表**\n\n| 术语 | 不用 | 意思 | 首次定义 |\n|---|---|---|---|\n| 均衡 | 平衡 | 稳定状态 | 1 |\n";
        crate::insert_card(
            &mut conn,
            "0/1|a",
            false,
            Workspace::Agent,
            CardType::Model,
            Some(&part),
            setting,
        )
        .expect("setting Card with a term table");
        change(&mut conn, "gate.agent.R6", "block").expect("block R6");
        let uses = Candidate::create(
            "0/1|b",
            Kind::Regular,
            Workspace::Agent,
            "# 设定\n\n这里的平衡来自策略互补。\n",
            CardType::Model,
            Some(&part),
        );
        let refusal = refusal(check_proposed(&conn, &uses).expect_err("R6 blocks"));
        assert!(has_rule(&refusal, "R6"), "{refusal:?}");
    }

    /// S04-AC03/S04-AC08: a Literature title is checked, while a pure Citation-key remap keeps
    /// the existing structural path and runs no prose check.
    #[test]
    fn a_literature_title_is_checked_and_a_key_remap_stays_structural() {
        let (_temp, mut conn) = archive();
        enforce(&mut conn);
        crate::insert_literature_card(
            &mut conn,
            "bookA",
            Workspace::Agent,
            "@book{bookA, title={Book A}}",
            "# Book A\n\n该书。\n",
        )
        .expect("book");
        let card = crate::load_card(&conn, "bookA").expect("book Card");
        let long = "书".repeat(41);
        let text = format!("# {long}\n\n该书。\n");
        let proposed =
            Candidate::edit(&card, &text, CardType::Idea, None, None, None, None, None);
        let refusal = refusal(check_proposed(&conn, &proposed).expect_err("R12 blocks"));
        assert!(has_rule(&refusal, "R12"), "{refusal:?}");

        let accepted = crate::update_literature_metadata(
            &mut conn,
            &card,
            "bookB",
            "@book{bookB, title={Book A}}",
            "Book A",
        )
        .expect("a key remap is structural");
        assert!(accepted.is_empty(), "a key remap runs no prose check");
        assert!(crate::load_card(&conn, "bookB").is_ok(), "the key moved");
    }

    /// S04-AC03: a Literature tree's direct child is an anchor, not an Idea, so the Idea size
    /// rule does not apply to it.
    #[test]
    fn an_anchor_in_a_book_keeps_its_navigation_role() {
        let (_temp, mut conn) = archive();
        enforce(&mut conn);
        crate::insert_literature_card(
            &mut conn,
            "bookA",
            Workspace::Agent,
            "@book{bookA, title={Book A}}",
            "# Book A\n\n该书。\n",
        )
        .expect("book");
        crate::insert_card(
            &mut conn,
            "bookA/1",
            false,
            Workspace::Agent,
            CardType::Idea,
            None,
            "# 税\n\n说明。\n",
        )
        .expect("an anchor is navigation, not an undersized Idea");
    }

    /// The rollout stage is validated, persisted, promoted and reset as one archive decision.
    #[test]
    fn the_gate_stage_is_persisted_promoted_and_reset() {
        let (_temp, mut conn) = archive();
        assert_eq!(current_stage(&conn).expect("stage"), Stage::Advisory);
        let before = revision(&conn).expect("revision");
        assert!(change_stage(&mut conn, "enforce").expect("promote").1);
        assert_eq!(current_stage(&conn).expect("stage"), Stage::Enforce);
        assert_eq!(revision(&conn).expect("revision"), before + 1);
        assert!(!change_stage(&mut conn, "enforce").expect("repeat").1);
        assert_eq!(revision(&conn).expect("revision"), before + 1);
        assert!(change_stage(&mut conn, "sometimes").is_err());
        assert_eq!(current_stage(&conn).expect("stage"), Stage::Enforce);
        assert!(change_stage(&mut conn, "default").expect("reset").1);
        assert_eq!(current_stage(&conn).expect("stage"), Stage::Advisory);
        assert_eq!(revision(&conn).expect("revision"), before + 2);
        let shown = summary_lines(
            Path::new("D:/box/archive"),
            &load(&conn).expect("policy"),
            revision(&conn).expect("revision"),
            current_stage(&conn).expect("stage"),
        );
        assert!(shown.contains(&"gate_stage=advisory".to_string()), "{shown:#?}");
    }
}

/// S29 (#171): the measured bound on the gate's added work (R0552/R0553).
///
/// The gate's decision is one function, `check_proposed`, inside the caller's write transaction. It
/// parses the proposed Card, loads the bounded context, runs the checker's local rules, and decides
/// the ratchet. That is exactly the "additional gate work" the design bounds; lock waiting, file
/// I/O and provider time are outside it by construction. Two tests live here:
///
/// * `the_context_statements_are_bounded_and_constant_in_archive_size` runs in the ordinary suite
///   and counts the SQL statements one decision issues through SQLite's statement trace, so an
///   accidental archive-size-dependent read fails the suite.
/// * `measure_the_gate_work_per_typical_card` is `#[ignore]`d because it is a measurement, not a
///   pass/fail case. It prints one JSON line with median/p95/p99/max microseconds per Card class,
///   the statement count, and the archive size. Run it in release mode:
///
///   cargo test --release --bin zt gate::perf -- --ignored --nocapture
#[cfg(test)]
mod perf {
    use super::*;
    use crate::model::Level;
    use crate::workspace::Access;
    use std::sync::Mutex;
    use std::time::Instant;

    /// Every statement SQLite traced on the one measured connection. Only the measurement test
    /// installs the tracer, so no other test's connection is recorded.
    static TRACED: Mutex<Vec<String>> = Mutex::new(Vec::new());

    fn trace_statement(sql: &str) {
        TRACED.lock().expect("trace lock").push(sql.to_string());
    }

    /// The statements one gate decision issues on `conn`.
    fn statements_of(conn: &mut Connection, candidate: &Candidate) -> Vec<String> {
        TRACED.lock().expect("trace lock").clear();
        conn.trace(Some(trace_statement));
        let outcome = check_proposed(conn, candidate);
        conn.trace(None);
        let traced = std::mem::take(&mut *TRACED.lock().expect("trace lock"));
        outcome.expect("the check completes");
        traced
    }

    /// The four-row term table the book stores as its nameless Term list.
    const TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n| 弹性 | 弹性系数 | 价格变化的反应程度 | §2.3 |\n| 生产函数 | 产量函数 | 投入与产出的关系 | §3.1 |\n| 替代弹性 | 替换弹性 | 要素比例对价格的反应 | §4.2 |\n";

    /// One book with its term list and rows, a concept Model Card with a part, and `extra`
    /// unrelated Cards in a second Topic tree. The unrelated Cards are what an archive-size
    /// dependent read would have to scan.
    fn measured_archive(extra: usize) -> (tempfile::TempDir, Connection) {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        let owner = Access::Owner;
        crate::ops::create_literature_tx(
            &conn,
            owner,
            Workspace::Agent,
            "@book{Book2026, title={Book 2026}}",
            "# Book 2026\n\n这本书说明一个模型。\n",
        )
        .expect("literature");
        crate::ops::create_successor_tx_with_term(
            &conn,
            owner,
            crate::ops::Successor::Direct,
            "Book2026",
            CardType::Idea,
            None,
            None,
            "# 第一章\n\n设置与假设。\n",
        )
        .expect("anchor");
        crate::ops::create_successor_tx_with_term(
            &conn,
            owner,
            crate::ops::Successor::Side,
            "Book2026/1",
            CardType::Term,
            None,
            None,
            &format!("# 术语表：精度\n\n{TABLE}\n"),
        )
        .expect("term list");
        crate::ops::create_successor_tx_with_term(
            &conn,
            owner,
            crate::ops::Successor::Side,
            "Book2026/1|a",
            CardType::Term,
            None,
            Some("均衡"),
            "# 均衡\n\n## 释义\n\n两个参与者都不愿单方面改变策略。\n",
        )
        .expect("term head");
        crate::ops::create_successor_tx_with_term(
            &conn,
            owner,
            crate::ops::Successor::Direct,
            "Book2026/1",
            CardType::Model,
            None,
            None,
            "# 均衡模型\n\n## 问题\n\n价格如何决定。\n\n## 结论\n\n价格等于边际成本。\n",
        )
        .expect("model");
        let model = "Book2026/2";
        crate::ops::create_successor_tx_with_term(
            &conn,
            owner,
            crate::ops::Successor::Side,
            model,
            CardType::Model,
            Some(&crate::model::Part {
                facet: crate::model::Facet::parse("设定").expect("facet"),
                emphasis: None,
            }),
            None,
            "# 设定\n\n## 假设\n\n存在一个代表性家庭。\n",
        )
        .expect("model part");
        // A supported concept level, so a promoted formal edit has a level to compare with.
        conn.execute(
            "UPDATE cards SET model_level = 'concept' WHERE location = ?1",
            [model],
        )
        .expect("concept level");
        // The unrelated Cards: rows an archive-size-dependent read would have to visit.
        conn.execute(
            "INSERT INTO cards(location, is_topic, is_lit, text, workspace, card_type)
             VALUES('90/0', 1, 0, '# 无关主题\n\n无关。\n', 'agent', 'idea')",
            [],
        )
        .expect("unrelated root");
        for n in 1..=extra {
            conn.execute(
                "INSERT INTO cards(location, is_topic, is_lit, text, workspace, card_type)
                 VALUES(?1, 0, 0, '# 无关卡\n\n这一段说明背景并给出充分理由。\n', 'agent', 'idea')",
                [(format!("90/{n}"))],
            )
            .expect("unrelated card");
        }
        (temp, conn)
    }

    /// The Card classes a digest writes most: a new Idea Card in a book, an edit of stored prose
    /// (the ratchet path), a Term head edit (the T rules), and a Model part edit (the model level
    /// and term table).
    fn candidates(conn: &Connection) -> Vec<(&'static str, Candidate)> {
        let idea = "# 价格等于边际成本\n\n这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。对应 [[Book2026/1]]：这条理由说明选择。\n";
        let anchor = crate::load_card(conn, "Book2026/1").expect("anchor");
        let head = crate::load_card(conn, "Book2026/1|a|a").expect("term head");
        let part = crate::load_card(conn, "Book2026/2|a").expect("model part");
        vec![
            (
                "idea create",
                Candidate::create(
                    "Book2026/5",
                    Kind::Regular,
                    Workspace::Agent,
                    idea,
                    CardType::Idea,
                    None,
                ),
            ),
            (
                "idea edit",
                Candidate::edit(
                    &anchor,
                    "# 第一章\n\n这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。\n",
                    CardType::Idea,
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
            ),
            (
                "term head edit",
                Candidate::edit(
                    &head,
                    "# 均衡\n\n## 释义\n\n两个参与者都不愿单方面改变策略，因此没有一方愿意偏离。\n",
                    CardType::Term,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .with_term(Some("均衡")),
            ),
            (
                "model part edit",
                Candidate::edit(
                    &part,
                    "# 设定\n\n## 假设\n\n存在一个代表性家庭，它选择消费与劳动的分配。\n",
                    CardType::Model,
                    Some(crate::model::Facet::parse("设定").expect("facet")),
                    None,
                    None,
                    None,
                    None,
                )
                .with_term(None),
            ),
        ]
    }

    /// The one statement count a typical decision issues, at two archive sizes.
    fn counts() -> (usize, usize) {
        let (_small_temp, mut small) = measured_archive(0);
        let (_big_temp, mut big) = measured_archive(300);
        let (_, candidate) = candidates(&small).remove(0);
        let small_count = statements_of(&mut small, &candidate).len();
        let big_count = statements_of(&mut big, &candidate).len();
        (small_count, big_count)
    }

    /// R0553: the bounded context does not grow with the archive. The same proposal issues the
    /// same number of SQL statements against a 6-Card archive and a 306-Card one, so no decision
    /// reads an archive-wide slice.
    #[test]
    fn the_context_statements_are_bounded_and_constant_in_archive_size() {
        let (small, big) = counts();
        assert_eq!(
            small, big,
            "the gate's statement count depends on archive size: {small} vs {big}"
        );
        assert!(
            small <= 32,
            "one decision issues {small} statements, over the bounded-context budget"
        );
    }

    /// The measurement itself. Prints one JSON line and asserts the design's target; run it in
    /// release mode because a debug build measures the unoptimised checker.
    #[test]
    #[ignore = "measurement: run with --release --ignored --nocapture"]
    fn measure_the_gate_work_per_typical_card() {
        const RUNS: usize = 200;
        const WARMUP: usize = 20;
        let (temp, mut conn) = measured_archive(300);
        let (small, big) = counts();
        let archive_bytes = std::fs::metadata(temp.path().join("zt.sqlite3"))
            .expect("archive file")
            .len();
        let mut classes = Vec::new();
        for (name, candidate) in candidates(&conn) {
            for _ in 0..WARMUP {
                drop(check_proposed(&conn, &candidate));
            }
            let mut samples = Vec::with_capacity(RUNS);
            let mut accepted = 0usize;
            let mut diagnostics = 0usize;
            for _ in 0..RUNS {
                let started = Instant::now();
                let outcome = check_proposed(&conn, &candidate);
                samples.push(started.elapsed().as_micros() as u64);
                match outcome {
                    Ok(accepted_result) => {
                        accepted += 1;
                        diagnostics += accepted_result.lines().len();
                    }
                    Err(error) => {
                        if let Some(refusal) = error.downcast_ref::<Refusal>() {
                            diagnostics += refusal.report.diagnostics.len();
                        }
                    }
                }
            }
            samples.sort_unstable();
            let at = |fraction: f64| samples[((samples.len() - 1) as f64 * fraction).round() as usize];
            let median = at(0.5);
            classes.push(serde_json::json!({
                "card": name,
                "runs": RUNS,
                "median_us": median,
                "p95_us": at(0.95),
                "p99_us": at(0.99),
                "max_us": *samples.last().expect("samples"),
                "accepted": accepted,
                "advice_per_run": diagnostics / RUNS,
            }));
        }
        let (_name, probe) = candidates(&conn).remove(0);
        let statements = statements_of(&mut conn, &probe).len();
        let report = serde_json::json!({
            "measurement": "gate work per typical Card",
            "issue": "#171 (S29)",
            "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
            "archive_cards": 6 + 302 + 300,
            "archive_bytes": archive_bytes,
            "context_statements_one_card": statements,
            "context_statements_small_vs_large": [small, big],
            "classes": classes,
        });
        println!("{}", serde_json::to_string(&report).expect("report JSON"));
        let typical = report["classes"][0]["median_us"].as_u64().expect("median");
        assert!(
            typical < 5_000,
            "the typical Card's gate work median is {typical} us, over the 5 ms target"
        );
    }

    /// The three Workspaces' policies load once per decision; the measurement's fixture uses the
    /// default policy, so a stray override would change the numbers.
    #[test]
    fn the_measured_archive_holds_the_default_policy_and_one_level() {
        let (_temp, conn) = measured_archive(0);
        assert_eq!(current_stage(&conn).expect("stage"), Stage::Advisory);
        assert!(rows(&conn).expect("overrides").is_empty());
        let level = crate::load_card(&conn, "Book2026/2").expect("model").level;
        assert_eq!(level, Some(Level::Concept));
    }
}
