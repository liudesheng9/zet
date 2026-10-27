//! The rule catalog and report policy: which rules a `--cards` report prints and
//! counts, the default severity and scope of every rule, and the legacy ratchet
//! that keeps an old Card editable without grandfathering a newly introduced
//! fault (design 5.4 and 6.3). Formatting and identity stay in the core.

use crate::core::{Diagnostic, Scope, Severity, SUBSTANTIVE};
use crate::text::{pattern, prop, ps_eq, ps_in, ps_string, text};
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

/// The rules self-check.ps1 reports by default: every hard rule but R6. The Term rules are
/// reported where they apply, which is only on a Term Card (S09, #152).
pub const DEFAULT_RULES: &str =
    "R2,R3,R4,R5,R7,R9,R12,R13,R14,R15,R16,R17,R20,R21,R22,I1,I2,I3,I4,I7,L1,L2,E1,E2,F1,F2,F3,T1,T2,T3,T4,T5,T6,T7,T9,T10";

/// The advisory rules of the current default policy: every line they raise is
/// advice. I7 is not here: it has one advisory subcase (`own_source_unknown`)
/// and one blocking one, so the core marks that subcase itself.
pub const ADVISORY_RULES: &[&str] = &["R20", "R21", "I1", "I6", "L3", "E5", "T4"];

/// The whole-archive graph rules: reported by the audit, not decided per write
/// (design 6.2). Their scope names that boundary in every structured report.
/// T8 is the book audit of one Term list (`get_terms`, S09).
pub const GRAPH_RULES: &[&str] = &["I5", "I6", "E4", "E5", "F4", "L3", "T8"];

/// The default severity of one rule before an archive policy overrides it (S03).
pub fn severity(rule: &str) -> Severity {
    if ps_in(rule, ADVISORY_RULES) {
        Severity::Advise
    } else {
        Severity::Block
    }
}

/// What evidence a rule reads under the default policy.
pub fn scope(rule: &str) -> Scope {
    if let Some(spec) = rule_of(rule) {
        return spec.scope;
    }
    if ps_in(rule, GRAPH_RULES) {
        Scope::Graph
    } else {
        Scope::Local
    }
}

// --- the gate policy catalog (S03, design 6.1 and 6.2) -----------------------------------------

/// One Workspace whose Cards the effective policy decides. A person's notes and
/// an agent's digest Cards can have different standards (design 6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Workspace {
    Human,
    Agent,
}

impl Workspace {
    /// Both Workspaces, in the order the policy output names them.
    pub const ALL: [Workspace; 2] = [Workspace::Human, Workspace::Agent];

    /// The name the `gate.<workspace>.<rule>` keys and the output use.
    pub fn as_str(self) -> &'static str {
        match self {
            Workspace::Human => "human",
            Workspace::Agent => "agent",
        }
    }

    /// The Workspace a configuration name denotes, exactly.
    pub fn parse(name: &str) -> Option<Self> {
        Workspace::ALL.into_iter().find(|workspace| workspace.as_str() == name)
    }
}

impl std::fmt::Display for Workspace {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One configured severity: refuse a write, report advice, or leave the rule out
/// of the report entirely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    Block,
    Advise,
    Off,
}

impl Setting {
    /// The policy name a `gate.<workspace>.<rule>` value spells.
    pub fn as_str(self) -> &'static str {
        match self {
            Setting::Block => "block",
            Setting::Advise => "advise",
            Setting::Off => "off",
        }
    }

    /// The setting a configuration value names, or `None` for an unknown value.
    pub fn parse(value: &str) -> Option<Self> {
        [Setting::Block, Setting::Advise, Setting::Off]
            .into_iter()
            .find(|setting| setting.as_str() == value)
    }

    /// What a diagnostic of this setting does: `off` reports nothing.
    pub fn severity(self) -> Option<Severity> {
        match self {
            Setting::Block => Some(Severity::Block),
            Setting::Advise => Some(Severity::Advise),
            Setting::Off => None,
        }
    }
}

impl std::fmt::Display for Setting {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// How one rule takes part in the write gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleState {
    /// Mechanically implemented and configurable.
    Configurable,
    /// An existing service invariant no policy can change (T7's Link target).
    Mandatory,
    /// Catalogued with its design default for a later slice: not mechanically
    /// implemented, not enforced, and not configurable yet.
    Pending,
}

impl RuleState {
    /// The word `zt config policy` prints for this state.
    pub fn as_str(self) -> &'static str {
        match self {
            RuleState::Configurable => "configurable",
            RuleState::Mandatory => "mandatory",
            RuleState::Pending => "pending",
        }
    }
}

/// One rule of the catalog with its default policy in each Workspace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleSpec {
    /// The rule id the core reports (`R2`, `E2`, ...).
    pub rule: &'static str,
    /// The default setting in the human Workspace.
    pub human: Setting,
    /// The default setting in the agent Workspace.
    pub agent: Setting,
    /// What evidence the rule reads (design 6.2's evaluation column).
    pub scope: Scope,
    /// Whether a policy can decide the rule at all.
    pub state: RuleState,
    /// The rule whose setting this rule shares: T10 is the Term-facing label of
    /// the same open-formula predicate as R22, so one setting decides both.
    pub alias_of: Option<&'static str>,
}

impl RuleSpec {
    /// The default setting of this rule in one Workspace.
    pub fn default(&self, workspace: Workspace) -> Setting {
        match workspace {
            Workspace::Human => self.human,
            Workspace::Agent => self.agent,
        }
    }

    /// Whether a check contract publishes this rule's effective setting: only a
    /// rule a policy decides. A mandatory invariant and a pending rule decide
    /// nothing yet, so publishing them as settings would claim a decision the
    /// gate never makes.
    pub fn is_configurable(&self) -> bool {
        self.state == RuleState::Configurable && self.alias_of.is_none()
    }
}

const fn spec(
    rule: &'static str,
    human: Setting,
    agent: Setting,
    scope: Scope,
    state: RuleState,
) -> RuleSpec {
    RuleSpec {
        rule,
        human,
        agent,
        scope,
        state,
        alias_of: None,
    }
}

/// The rule catalog at full enforcement (design 6.2). The checker owns it; an
/// archive stores only its overrides. A rule is configurable only when it is
/// mechanically implemented, so the pending Term rules carry their design
/// defaults without being enforced or settable yet.
static CATALOG: &[RuleSpec] = &[
    // Local/bounded-context format: the Card must be well formed in both
    // Workspaces.
    spec("R2", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R3", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R4", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R5", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    // R6 keeps the existing meaning-sensitive remediation decision: a synonym
    // change is not made by a style check.
    spec("R6", Setting::Off, Setting::Advise, Scope::Local, RuleState::Configurable),
    spec("R7", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R9", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R12", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    // Formal model format: advice in the human Workspace, block for a digest.
    spec("R13", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R14", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R15", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R16", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R17", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R18", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("R19", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    // Advisory subcases stay advisory in both Workspaces.
    spec("R20", Setting::Advise, Setting::Advise, Scope::Local, RuleState::Configurable),
    spec("R21", Setting::Advise, Setting::Advise, Scope::Local, RuleState::Configurable),
    spec("R22", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("I1", Setting::Advise, Setting::Advise, Scope::Local, RuleState::Configurable),
    spec("I2", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("I3", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("I4", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    // Graph facts are audit advice, not a write gate.
    spec("I5", Setting::Advise, Setting::Advise, Scope::Graph, RuleState::Configurable),
    spec("I6", Setting::Advise, Setting::Advise, Scope::Graph, RuleState::Configurable),
    spec("I7", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("E1", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("E2", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("E4", Setting::Advise, Setting::Advise, Scope::Graph, RuleState::Configurable),
    spec("E5", Setting::Advise, Setting::Advise, Scope::Graph, RuleState::Configurable),
    spec("F1", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("F2", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("F3", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("F4", Setting::Advise, Setting::Advise, Scope::Graph, RuleState::Configurable),
    spec("L1", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("L2", Setting::Advise, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("L3", Setting::Advise, Setting::Advise, Scope::Graph, RuleState::Configurable),
    // The Term rules (S09, #152): T1, T2, T3, T5, T6, T9 and T10 enforce the applicable Term
    // format; T4 is the approximate symbol scan and stays advice; T8 is the book audit of one
    // Term list, so it keeps the graph scope and never blocks one write.
    spec("T1", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("T2", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("T3", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("T4", Setting::Advise, Setting::Advise, Scope::Local, RuleState::Configurable),
    spec("T5", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    spec("T6", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    // T7's wrong/nonexistent Link target is the existing Link write invariant.
    spec("T7", Setting::Block, Setting::Block, Scope::Local, RuleState::Mandatory),
    // Computed term structure is audit advice.
    spec("T8", Setting::Advise, Setting::Advise, Scope::Graph, RuleState::Configurable),
    spec("T9", Setting::Block, Setting::Block, Scope::Local, RuleState::Configurable),
    RuleSpec {
        rule: "T10",
        human: Setting::Block,
        agent: Setting::Block,
        scope: Scope::Local,
        state: RuleState::Configurable,
        alias_of: Some("R22"),
    },
];

/// Every rule the policy catalog names, in rule-id order.
pub fn catalog() -> &'static [RuleSpec] {
    CATALOG
}

// --- the Literature-note rows (issue #184) -----------------------------------------------------

/// The fixed level each named rule takes on a Literature note, in catalog order: the reading
/// prose of a Literature tree follows the source's own argument, so the Idea-card title, size and
/// sentence rules leave it alone (or only advise), while the source line, the formula rule, the
/// layer formats and the Term rules keep the level they have. The row is not an archive setting:
/// every archive uses it, so it is part of the published policy and its hash, never a
/// `gate.<workspace>.<rule>` override. A rule absent from this table keeps its Workspace setting.
static LITERATURE_ROWS: &[(&str, Setting)] = &[
    ("R2", Setting::Off),
    ("R3", Setting::Off),
    ("R4", Setting::Off),
    ("R5", Setting::Off),
    ("R7", Setting::Off),
    ("R9", Setting::Advise),
    ("R12", Setting::Advise),
    ("I1", Setting::Off),
    ("I2", Setting::Advise),
    ("I4", Setting::Advise),
    ("I6", Setting::Off),
    ("I7", Setting::Off),
    ("R20", Setting::Advise),
    ("R21", Setting::Advise),
    ("L1", Setting::Advise),
    ("L2", Setting::Advise),
    ("L3", Setting::Advise),
];

/// The Literature-note row of one rule, or `None` when the rule keeps its Workspace setting there.
pub fn literature_setting(rule: &str) -> Option<Setting> {
    LITERATURE_ROWS
        .iter()
        .find(|(id, _)| *id == rule)
        .map(|(_, setting)| *setting)
}

/// Every Literature-note row, in catalog order, as the published policy covers it.
pub fn literature_rows() -> &'static [(&'static str, Setting)] {
    LITERATURE_ROWS
}

/// The catalog entry of one rule id.
pub fn rule_of(rule: &str) -> Option<&'static RuleSpec> {
    CATALOG.iter().find(|spec| spec.rule == rule)
}

/// One existing service invariant: mandatory, with no rule id and no configurable
/// severity (design 6.2's first row).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Invariant {
    /// The configuration name a typo or an attempt to disable it would use.
    pub name: &'static str,
    /// What the invariant protects.
    pub label: &'static str,
}

static INVARIANTS: &[Invariant] = &[
    Invariant {
        name: "envelope",
        label: "the Card envelope and its single leading title",
    },
    Invariant {
        name: "fields",
        label: "the Card type and its valid fields",
    },
    Invariant {
        name: "address",
        label: "the Location and Citation-key grammar",
    },
    Invariant {
        name: "links",
        label: "the Link grammar and a newly introduced Link's target",
    },
    Invariant {
        name: "targets",
        label: "the existing target a Link or Reference names",
    },
    Invariant {
        name: "access",
        label: "Workspace Access: an agent writes only agent Cards",
    },
    Invariant {
        name: "locks",
        label: "an active Tree Lock",
    },
];

/// The mandatory service invariants no gate policy can change.
pub fn invariants() -> &'static [Invariant] {
    INVARIANTS
}

/// The invariant a configuration name denotes.
pub fn invariant(name: &str) -> Option<&'static Invariant> {
    INVARIANTS.iter().find(|invariant| invariant.name == name)
}

// --- the effective policy of one archive (S03, design 6.1) -------------------------------------

/// The metadata prefix of a declared override key.
pub const GATE_PREFIX: &str = "gate.";

/// The declared metadata key of one Workspace's rule: `gate.<workspace>.<rule>`.
pub fn gate_key(workspace: Workspace, rule: &str) -> String {
    format!("{GATE_PREFIX}{}.{rule}", workspace.as_str())
}

/// Read a `gate.<workspace>.<rule>` key. The Workspace must be exact and the
/// rule is returned as written, so a typo reaches the catalog's own validation
/// instead of being silently normalized.
pub fn parse_gate_key(key: &str) -> Result<(Workspace, &str), String> {
    let usage = || {
        format!(
            "`{key}` is not a gate policy key; use gate.<workspace>.<rule>, for example gate.agent.R2"
        )
    };
    let rest = key.strip_prefix(GATE_PREFIX).ok_or_else(usage)?;
    let mut parts = rest.split('.');
    let (name, rule) = match (parts.next(), parts.next(), parts.next()) {
        (Some(name), Some(rule), None) => (name, rule),
        _ => return Err(usage()),
    };
    let Some(workspace) = Workspace::parse(name) else {
        return Err(format!(
            "unknown Workspace `{name}` in `{key}`; expected human or agent"
        ));
    };
    Ok((workspace, rule))
}

/// What one configuration value asks a policy to do: hold a setting, or remove
/// the override so the rule falls back to its default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Set(Setting),
    Reset,
}

impl Change {
    /// The value of a `gate.<workspace>.<rule>` argument.
    pub fn parse(value: &str) -> Result<Self, String> {
        if value == "default" {
            return Ok(Change::Reset);
        }
        match Setting::parse(value) {
            Some(setting) => Ok(Change::Set(setting)),
            None => Err(format!(
                "unknown policy value `{value}`; expected block, advise, off, or default"
            )),
        }
    }
}

/// The effective policy of one Workspace: the checker's defaults plus the
/// archive's overrides. Missing override means the default (design 6.1, R0118).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    workspace: Workspace,
    overrides: BTreeMap<String, Setting>,
}

impl Policy {
    /// The default policy of one Workspace, with no archive override.
    pub fn new(workspace: Workspace) -> Self {
        Self {
            workspace,
            overrides: BTreeMap::new(),
        }
    }

    /// The Workspace this policy decides.
    pub fn workspace(&self) -> Workspace {
        self.workspace
    }

    /// The stored overrides, in rule order.
    pub fn overrides(&self) -> &BTreeMap<String, Setting> {
        &self.overrides
    }

    /// Build one Workspace's policy from stored `(rule, value)` pairs.
    pub fn from_overrides(
        workspace: Workspace,
        overrides: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, String> {
        let mut policy = Policy::new(workspace);
        for (rule, value) in overrides {
            policy.change(&rule, Change::parse(&value)?)?;
        }
        Ok(policy)
    }

    /// Whether this Workspace stores an explicit override for the rule.
    pub fn is_override(&self, rule: &str) -> bool {
        self.overrides.contains_key(rule)
    }

    /// The setting that decides the rule: the override when one is stored, else
    /// the catalog's default. `None` for a name the catalog does not hold, so a
    /// rule added by a later checker keeps its own severity rather than an
    /// invented policy decision.
    pub fn effective(&self, rule: &str) -> Option<Setting> {
        if let Some(setting) = self.overrides.get(rule) {
            return Some(*setting);
        }
        let spec = rule_of(rule)?;
        match spec.alias_of {
            Some(target) => self.effective(target),
            None => Some(spec.default(self.workspace)),
        }
    }

    /// Validate one configured change and apply it. Returns whether the stored
    /// overrides changed, so a no-op request writes no metadata row and moves no
    /// revision.
    pub fn change(&mut self, rule: &str, change: Change) -> Result<bool, String> {
        let spec = validate_rule(rule)?;
        if let Some(target) = spec.alias_of {
            return Err(format!(
                "{rule} is the Term-facing label of {target}; configure gate.<workspace>.{target} instead"
            ));
        }
        match spec.state {
            RuleState::Mandatory => {
                return Err(format!("{rule} is a mandatory rule; it cannot be configured"));
            }
            RuleState::Pending => {
                return Err(format!(
                    "{rule} is not mechanically implemented yet; its default is {} and an override is refused until the rule exists",
                    spec.default(self.workspace)
                ));
            }
            RuleState::Configurable => {}
        }
        match change {
            Change::Reset => Ok(self.overrides.remove(rule).is_some()),
            Change::Set(setting) => {
                if spec.scope == Scope::Graph && setting == Setting::Block {
                    return Err(format!(
                        "{rule} is a graph-only audit rule; it cannot block a single write"
                    ));
                }
                if self.overrides.get(rule) == Some(&setting) {
                    return Ok(false);
                }
                self.overrides.insert(rule.to_string(), setting);
                Ok(true)
            }
        }
    }

    /// The effective policy as the stable text its hash covers: one line per
    /// catalogued rule, so a catalog default, an alias, or an override that
    /// changes a decision changes the text and its hash.
    pub fn canonical(&self) -> String {
        let mut text = format!("{}\n", self.workspace.as_str());
        for spec in catalog() {
            let setting = self
                .effective(spec.rule)
                .expect("every catalogued rule has an effective setting")
                .as_str();
            text.push_str(&format!("{}={setting}\n", spec.rule));
        }
        text
    }

    /// The SHA-256 of this Workspace's effective policy.
    pub fn hash(&self) -> String {
        sha256_hex(self.canonical().as_bytes())
    }

    /// Apply this policy to one diagnostic. `None` when the rule is off; an
    /// advisory subcase stays advice whatever the setting (design 6.2). A rule
    /// the catalog does not name keeps the severity the core gave it, so a rule
    /// added by a later checker is never silently disabled.
    pub fn decide(&self, mut diagnostic: Diagnostic) -> Option<Diagnostic> {
        if diagnostic.subcase {
            diagnostic.severity = Severity::Advise;
            return Some(diagnostic);
        }
        match self.effective(&diagnostic.rule) {
            Some(Setting::Off) => None,
            Some(setting) => {
                diagnostic.severity = setting.severity().expect("off was handled above");
                Some(diagnostic)
            }
            None => Some(diagnostic),
        }
    }

    /// The diagnostics this policy keeps, at the severities it decides.
    pub fn apply(&self, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        diagnostics
            .into_iter()
            .filter_map(|diagnostic| self.decide(diagnostic))
            .collect()
    }

    /// The assembled report at this policy's severities.
    pub fn report(&self, diagnostics: Vec<Diagnostic>) -> CheckReport {
        CheckReport::of(self.apply(diagnostics))
    }
}

/// Validate a rule name against the catalog: a required service invariant and an
/// unknown id are both refused, with the invariant's own explanation.
fn validate_rule(rule: &str) -> Result<&'static RuleSpec, String> {
    if let Some(invariant) = invariant(rule) {
        return Err(format!(
            "`{rule}` is a mandatory service invariant: {}; no gate policy can change it",
            invariant.label
        ));
    }
    rule_of(rule).ok_or_else(|| {
        format!("unknown rule `{rule}`; the gate policy names only the rules of its catalog")
    })
}

/// The schema marker of the hashed policy text, so a later layout is a different
/// policy hash rather than a silent reinterpretation.
///
/// 2 (issue #184): the text also covers the Literature-note rows, so the level a rule takes on a
/// note is part of the contract a check-contract reader compares.
pub const POLICY_SCHEMA: &str = "zt-gate-policy/2";

/// One archive's effective policy: both Workspaces, loaded once per operation
/// (design 6.1, S03-AC05).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicySet {
    human: Policy,
    agent: Policy,
}

impl Default for PolicySet {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicySet {
    /// Both Workspaces at their catalog defaults, with no override.
    pub fn new() -> Self {
        Self {
            human: Policy::new(Workspace::Human),
            agent: Policy::new(Workspace::Agent),
        }
    }

    /// One Workspace's effective policy.
    pub fn policy(&self, workspace: Workspace) -> &Policy {
        match workspace {
            Workspace::Human => &self.human,
            Workspace::Agent => &self.agent,
        }
    }

    fn policy_mut(&mut self, workspace: Workspace) -> &mut Policy {
        match workspace {
            Workspace::Human => &mut self.human,
            Workspace::Agent => &mut self.agent,
        }
    }

    /// Validate and apply one configured change to one Workspace.
    pub fn change(
        &mut self,
        workspace: Workspace,
        rule: &str,
        change: Change,
    ) -> Result<bool, String> {
        self.policy_mut(workspace).change(rule, change)
    }

    /// Both Workspaces' stored overrides, in Workspace then rule order.
    pub fn overrides(&self) -> Vec<(Workspace, &str, Setting)> {
        let mut out = Vec::new();
        for workspace in Workspace::ALL {
            for (rule, setting) in self.policy(workspace).overrides() {
                out.push((workspace, rule.as_str(), *setting));
            }
        }
        out
    }

    /// Build the archive policy from stored `(key, value)` rows. A row this
    /// binary cannot read is an error naming the row, never a silently weaker
    /// default (S03-AC02).
    pub fn from_pairs(
        pairs: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, String> {
        let mut set = PolicySet::new();
        for (key, value) in pairs {
            let (workspace, rule) = parse_gate_key(&key).map_err(|error| format!("{key}={value}: {error}"))?;
            let change = Change::parse(&value).map_err(|error| format!("{key}={value}: {error}"))?;
            set.change(workspace, rule, change)
                .map_err(|error| format!("{key}={value}: {error}"))?;
        }
        Ok(set)
    }

    /// The effective policy as the stable text its hash covers: the schema marker, both
    /// Workspaces, and the Literature-note rows every archive uses (issue #184), so a changed row
    /// is a changed check contract.
    pub fn canonical(&self) -> String {
        let mut text = format!(
            "{POLICY_SCHEMA}\n{}\n{}",
            self.human.canonical(),
            self.agent.canonical()
        );
        text.push_str("\nliterature\n");
        for (rule, setting) in LITERATURE_ROWS {
            text.push_str(&format!("{rule}={}\n", setting.as_str()));
        }
        text
    }

    /// The policy hash a check-contract reader compares (design 6.1, R0120).
    pub fn hash(&self) -> String {
        sha256_hex(self.canonical().as_bytes())
    }

    /// Apply the archive policy to the diagnostics of these entries: each entry's
    /// own Workspace decides its rules, and an entry that names none is a draft
    /// in the agent Workspace (design 8.1).
    pub fn apply(&self, cards: &[Value], diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        let workspaces = cards
            .iter()
            .filter_map(|card| {
                let workspace = Workspace::parse(&ps_string(prop(card, "workspace")))?;
                Some((ps_string(prop(card, "address")), workspace))
            })
            .collect::<HashMap<_, _>>();
        diagnostics
            .into_iter()
            .filter_map(|diagnostic| {
                let workspace = workspaces
                    .get(&diagnostic.address)
                    .copied()
                    .unwrap_or(Workspace::Agent);
                self.policy(workspace).decide(diagnostic)
            })
            .collect()
    }
}

/// A lower-case SHA-256 digest, the policy hash's published spelling.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The assembled result of one Card check: every diagnostic in the core's
/// traversal order, and the number that still block after the legacy ratchet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckReport {
    pub diagnostics: Vec<Diagnostic>,
    pub blockers: usize,
}

impl CheckReport {
    /// A report with no legacy allowance: a new Card, or an audit run. Blocking
    /// diagnostics block, advisory diagnostics stay advice (design 6.3).
    pub fn of(diagnostics: Vec<Diagnostic>) -> Self {
        let blockers = diagnostics.iter().filter(|diagnostic| diagnostic.blocking()).count();
        Self {
            diagnostics,
            blockers,
        }
    }

    /// The familiar lines, in the report's order.
    pub fn lines(&self) -> Vec<String> {
        self.diagnostics
            .iter()
            .map(Diagnostic::line)
            .collect::<Vec<_>>()
    }
}

/// The facts that decide whether one rule applies to a Card and means the same
/// thing after a metadata change (design 6.3, R0146). A rule is compared only
/// when its signature is unchanged; the old version is read with its own
/// metadata, so a retyped Card must satisfy the requirements its type makes
/// applicable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applicability {
    category: String,
    level: String,
    part: bool,
    derivation: bool,
    shared: bool,
    book: String,
    term: String,
    /// The Card's type is `term` (S09): a Term Card's content rules apply to it alone.
    term_type: bool,
    /// The computed Term role (`card`, `part`, `list`, `unfiled`) when the entry names one.
    term_role: String,
    /// The Card is a Literature note (issue #184): several rules take their Literature-note row,
    /// so the same text means another thing in a book and in a Topic tree.
    literature: bool,
}

impl Applicability {
    /// The facts one entry declares about itself. An entry that names no type,
    /// level, or book is compared by what it does say: the bundle contract is
    /// explicit that absent context is not a pass.
    pub fn of(entry: &Value) -> Self {
        let facet = text(prop(entry, "facet"));
        Self {
            category: ps_string(prop(entry, "category")).to_lowercase(),
            level: ps_string(prop(entry, "level")).to_lowercase(),
            part: !facet.is_empty(),
            derivation: crate::core::is_derivation_facet(&facet),
            shared: ps_eq(&text(prop(entry, "line")), "shared"),
            book: ps_string(prop(entry, "book")),
            // The Term rules decide their role from this authored name;
            // keeping it here makes an unnamed term rule follow a term change.
            term: ps_string(prop(entry, "term")),
            term_type: ps_eq(&text(prop(entry, "type")), "term"),
            term_role: ps_string(prop(entry, "term_role")).to_lowercase(),
            literature: crate::core::is_literature_note(entry),
        }
    }

    /// Whether `rule` has the same applicability and meaning for both versions.
    pub fn comparable(&self, proposed: &Applicability, rule: &str) -> bool {
        self.signature(rule) == proposed.signature(rule)
    }

    fn signature(&self, rule: &str) -> String {
        match rule {
            // The plain text rules read the authored text the same way whatever
            // the Card's type is: a moved or retyped Card keeps those faults. The rules a
            // Literature-note row changes mean another thing in a book (issue #184).
            "R2" | "R3" | "R4" | "R5" | "R7" | "R9" | "R12" => {
                format!("text:{}", self.literature)
            }
            "R18" | "R19" | "R20" | "R21" | "R22" => "text".to_string(),
            // A shared part follows its home model's term table; leaving the
            // shared list or changing the table changes what R6 means.
            "R6" => format!("terms:{}", self.shared),
            // The model rules apply by facet, derivation, and formal level.
            "R13" => format!("part:{}:{}", self.part, self.level),
            "R14" | "R15" | "R16" | "R17" => {
                format!("derivation:{}:{}", self.derivation, self.level)
            }
            // The Idea rules apply to an Idea or an anchor.
            "I1" => format!(
                "claim:{}:{}",
                self.in_category(&["idea", "anchor"]),
                self.literature
            ),
            "I2" | "I4" | "I7" => format!(
                "idea:{}:{}",
                self.in_category(&["idea"]),
                self.literature
            ),
            "I3" => format!(
                "source:{}:{}",
                self.in_category(&["idea", "event", "restatement-entry"]),
                self.book
            ),
            "I5" => format!("substantive:{}", self.in_category(SUBSTANTIVE)),
            // The Event and Finding rules apply by type; F2 and I3 also read the
            // book they name, so another book is another requirement.
            "E1" | "E2" | "E4" | "E5" => {
                format!("event:{}", self.in_category(&["event"]))
            }
            "F1" | "F3" | "F4" => format!("finding:{}", self.in_category(&["finding"])),
            "F2" => format!(
                "source:{}:{}",
                self.in_category(&["finding"]),
                self.book
            ),
            "L1" | "L2" | "L3" => format!(
                "layer:{}:{}",
                self.in_category(&[
                    "idea",
                    "anchor",
                    "event",
                    "finding",
                    "restatement-entry",
                ]),
                self.literature
            ),
            // The Term content rules apply by the authored name and the computed role: a Card
            // that is retyped away, renamed, or moved between a head and a part is a different
            // requirement (S09, design 11.2).
            "T1" | "T2" | "T3" | "T5" | "T6" | "T9" => format!(
                "term:{}:{}:{}",
                self.term_type, self.term, self.term_role
            ),
            "T4" => format!("term-symbols:{}:{}", self.term_type, self.term),
            "T7" => format!("term-links:{}:{}", self.term_type, self.term),
            "T8" => "graph".to_string(),
            "T10" => format!("text-term:{}", self.term_type),
            // A rule this policy does not name keeps the conservative treatment:
            // it is compared only when every metadata fact is unchanged, so a new
            // rule cannot inherit across a retype, a shared-list, book or term
            // transition before its own signature is named here.
            _ => format!(
                "other:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                self.category,
                self.level,
                self.part,
                self.derivation,
                self.shared,
                self.book,
                self.term,
                self.term_type,
                self.term_role,
                self.literature
            ),
        }
    }

    fn in_category(&self, names: &[&str]) -> bool {
        names.contains(&self.category.as_str())
    }
}

/// Mark one proposed report against the report of the same Cards' old authored
/// versions (design 6.3). Every comparable old occurrence is consumed once, in
/// multiplicity, by an occurrence with the same `(address, rule, fault, key)`;
/// a consumed diagnostic becomes `legacy: true` advice. An occurrence the old
/// version did not carry keeps its severity and can block.
///
/// `comparable` answers whether a rule keeps its applicability and meaning
/// across this Card's metadata change; a new Card passes a closure that answers
/// `false` for every rule.
pub fn ratchet(
    before: &[Diagnostic],
    proposed: Vec<Diagnostic>,
    comparable: impl Fn(&Diagnostic) -> bool,
) -> CheckReport {
    let mut old: HashMap<(String, String, String, String), usize> = HashMap::new();
    for diagnostic in before {
        *old.entry(identity(diagnostic)).or_default() += 1;
    }
    let mut remaining = proposed;
    for diagnostic in &mut remaining {
        if !comparable(diagnostic) {
            continue;
        }
        if let Some(count) = old.get_mut(&identity(diagnostic))
            && *count > 0
        {
            *count -= 1;
            diagnostic.legacy = true;
            diagnostic.severity = Severity::Advise;
        }
    }
    CheckReport::of(remaining)
}

fn identity(diagnostic: &Diagnostic) -> (String, String, String, String) {
    (
        diagnostic.address.clone(),
        diagnostic.rule.clone(),
        diagnostic.fault.clone(),
        diagnostic.key.clone(),
    )
}

// self-check.ps1.
pattern!(FAULT_LINE, r"^\S+ ([RIELFT]\d+) ");

pattern!(LIST_SPLIT, r"[,\s]+");

/// The lines of the rules asked for; advisory lines are shown but not counted.
pub fn filter_rules(lines: &[String], rules: &str) -> (Vec<String>, usize) {
    let allowed = LIST_SPLIT
        .split(rules)
        .filter(|rule| !rule.is_empty())
        .collect::<Vec<_>>();
    let mut kept = Vec::new();
    let mut faults = 0;
    for line in lines {
        let Some(caps) = FAULT_LINE.captures(line) else {
            continue;
        };
        if !ps_in(&caps[1], &allowed) {
            continue;
        }
        kept.push(line.clone());
        if !line.contains("(advisory)") {
            faults += 1;
        }
    }
    (kept, faults)
}

/// The diagnostics of the rules asked for. The `--cards` mode filters by rule id;
/// the count that follows is the blocking diagnostics among the kept ones
/// (design 8.1, R0187), so no caller keeps a second advisory list.
pub fn select_rules(diagnostics: Vec<Diagnostic>, rules: &str) -> Vec<Diagnostic> {
    let allowed = LIST_SPLIT
        .split(rules)
        .filter(|rule| !rule.is_empty())
        .collect::<Vec<_>>();
    diagnostics
        .into_iter()
        .filter(|diagnostic| ps_in(&diagnostic.rule, &allowed))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn diagnostic(address: &str, rule: &str, fault: &str, key: &str, message: &str) -> Diagnostic {
        Diagnostic {
            address: address.to_string(),
            rule: rule.to_string(),
            fault: fault.to_string(),
            key: key.to_string(),
            message: message.to_string(),
            severity: severity(rule),
            legacy: false,
            subcase: false,
            scope: scope(rule),
        }
    }

    #[test]
    fn the_rule_filter_keeps_asked_rules_and_counts_hard_faults() {
        let lines = vec![
            "a/1 R6 uses x".to_string(),
            "a/1 R3 x；y".to_string(),
            "a/1 I1 (advisory) the title states no claim: x".to_string(),
        ];
        let (kept, count) = filter_rules(&lines, DEFAULT_RULES);
        assert_eq!(
            kept,
            [
                "a/1 R3 x；y",
                "a/1 I1 (advisory) the title states no claim: x"
            ]
        );
        assert_eq!(count, 1);
    }

    /// S02-AC02/AC06: a fault the old authored version also carried becomes
    /// legacy advice and stops blocking; a new Card has no allowance at all.
    #[test]
    fn a_retained_fault_becomes_legacy_advice_and_stops_blocking() {
        let before = [diagnostic("K/1", "R3", "semicolon", "甲；乙。", "甲；乙。")];
        let report = ratchet(&before, before.to_vec(), |_| true);
        assert_eq!(report.diagnostics.len(), 1);
        assert!(report.diagnostics[0].legacy);
        assert_eq!(report.diagnostics[0].severity, Severity::Advise);
        assert_eq!(report.blockers, 0);
        assert_eq!(report.lines(), ["K/1 R3 (advisory) 甲；乙。"]);

        // A new Card has no old version, so the same fault still blocks.
        let fresh = ratchet(&[], before.to_vec(), |_| true);
        assert!(!fresh.diagnostics[0].legacy);
        assert_eq!(fresh.blockers, 1);
    }

    /// S02-AC03: a second copy of an already faulty sentence is a new occurrence,
    /// while removing one of two old copies leaves the remaining one inherited.
    #[test]
    fn the_ratchet_counts_occurrences_not_merely_identities() {
        let one = diagnostic("K/1", "R3", "semicolon", "甲；乙。", "甲；乙。");
        let twice = [one.clone(), one.clone()];
        let duplicated = ratchet(&[one.clone()], twice.to_vec(), |_| true);
        assert!(duplicated.diagnostics[0].legacy);
        assert!(!duplicated.diagnostics[1].legacy);
        assert_eq!(duplicated.diagnostics[1].severity, Severity::Block);
        assert_eq!(duplicated.blockers, 1);

        let removed = ratchet(&twice, vec![one.clone()], |_| true);
        assert!(removed.diagnostics[0].legacy);
        assert_eq!(removed.blockers, 0);
    }

    /// S02-AC03: identity is the semantic key, not the printed line. A paragraph
    /// moved to another line keeps its fault; two different fragments that clip
    /// to the same prefix stay distinct.
    #[test]
    fn the_ratchet_compares_semantic_keys_not_printed_lines() {
        let moved = ratchet(
            &[diagnostic(
                "K/1",
                "R18",
                "repeated_prose",
                "同一段。",
                "repeated prose paragraph at lines 2-2 and 6-6",
            )],
            vec![diagnostic(
                "K/1",
                "R18",
                "repeated_prose",
                "同一段。",
                "repeated prose paragraph at lines 4-4 and 9-9",
            )],
            |_| true,
        );
        assert!(moved.diagnostics[0].legacy);
        assert_eq!(moved.blockers, 0);

        let prefix = "同".repeat(88);
        let old = diagnostic(
            "K/1",
            "R2",
            "sentence_length",
            &format!("41 characters > 40: {prefix}甲和乙"),
            &format!("(41 characters > 40) {prefix}甲和乙"),
        );
        let new = diagnostic(
            "K/1",
            "R2",
            "sentence_length",
            &format!("41 characters > 40: {prefix}丙和丁"),
            &format!("(41 characters > 40) {prefix}丙和丁"),
        );
        assert_eq!(old.line().len(), new.line().len());
        let distinct = ratchet(&[old], vec![new], |_| true);
        assert!(!distinct.diagnostics[0].legacy);
        assert_eq!(distinct.blockers, 1);
    }

    /// S02-AC05: a rule whose applicability and meaning changed is not
    /// comparable, so a newly relevant requirement cannot be called legacy.
    #[test]
    fn applicability_signatures_grandfather_only_unchanged_meaning() {
        let idea = Applicability::of(&json!({ "category": "idea" }));
        let event = Applicability::of(&json!({ "category": "event" }));
        assert!(!idea.comparable(&event, "E2"), "Event rules are newly applicable");
        assert!(!idea.comparable(&event, "I2"), "Idea rules no longer apply");
        assert!(idea.comparable(&event, "R3"), "the text rules mean the same");

        let concept = Applicability::of(&json!({ "category": "part", "facet": "推导", "level": "concept" }));
        let formal = Applicability::of(&json!({ "category": "part", "facet": "推导", "level": "formal" }));
        assert!(!concept.comparable(&formal, "R13"));
        assert!(!concept.comparable(&formal, "R15"));
        assert!(concept.comparable(&formal, "R2"));

        let shared = Applicability::of(&json!({ "category": "part", "facet": "推导", "line": "shared" }));
        let own = Applicability::of(&json!({ "category": "part", "facet": "推导" }));
        assert!(!shared.comparable(&own, "R6"), "a term-table transition changes R6");

        let book_a = Applicability::of(&json!({ "category": "idea", "book": "A" }));
        let book_b = Applicability::of(&json!({ "category": "idea", "book": "B" }));
        assert!(!book_a.comparable(&book_b, "I3"), "another book is another source requirement");
        assert!(book_a.comparable(&book_b, "R2"));

        let same = Applicability::of(&json!({ "category": "event", "level": "formal", "facet": "设定" }));
        let also = Applicability::of(&json!({ "category": "event", "level": "formal", "facet": "设定" }));
        assert!(same.comparable(&also, "E2"));
        // Issue #184: the same text in a Literature tree and in a Topic tree is a different
        // requirement for the rules the Literature-note row changes, while the source line, the
        // open formula and the exact-corruption rules mean the same in both.
        let note = Applicability::of(&json!({ "category": "idea", "tree": "K" }));
        let topic = Applicability::of(&json!({ "category": "idea", "tree": "40" }));
        for rule in ["R2", "R3", "R4", "R5", "R7", "R9", "R12", "I1", "I2", "I4", "I7", "L1", "L2", "L3", "I6"] {
            assert!(!note.comparable(&topic, rule), "{rule} takes a Literature-note row");
        }
        for rule in ["I3", "R18", "R19", "R20", "R21", "R22"] {
            assert!(note.comparable(&topic, rule), "{rule} means the same on both");
        }
        // A card retyped from Idea to Event keeps a style fault but not an Idea one.
        let before = [diagnostic("K/9", "E2", "missing_labels", "labels:参与者", "missing labels: 参与者")];
        let report = ratchet(&before, before.to_vec(), |diagnostic| {
            idea.comparable(&event, &diagnostic.rule)
        });
        assert!(!report.diagnostics[0].legacy);
        assert_eq!(report.blockers, 1);
    }

    /// S02-AC05 (R0146/R0151): a rule the catalog does not name yet keeps the
    /// conservative treatment: every metadata fact a future rule could read must
    /// be unchanged, so a shared-list, book or term transition never grandfathers
    /// an unnamed rule. The named rules keep their own narrower signatures.
    #[test]
    fn an_unnamed_rule_compares_every_metadata_fact() {
        // A rule id the catalog does not name keeps the whole-metadata signature.
        let own = Applicability::of(&json!({ "category": "part", "facet": "推导" }));
        let shared = Applicability::of(&json!({ "category": "part", "facet": "推导", "line": "shared" }));
        assert!(!own.comparable(&shared, "T99"), "a shared transition is no comparison");
        let book_a = Applicability::of(&json!({ "category": "idea", "book": "A" }));
        let book_b = Applicability::of(&json!({ "category": "idea", "book": "B" }));
        assert!(!book_a.comparable(&book_b, "T99"), "another book is no comparison");
        let term = Applicability::of(&json!({ "category": "part", "facet": "推导", "term": "替代弹性" }));
        assert!(!own.comparable(&term, "T99"), "a term transition is no comparison");
        // A Term rule follows the authored name and the computed role, not a model facet.
        let head = Applicability::of(&json!({ "type": "term", "term": "替代弹性", "term_role": "card" }));
        let part = Applicability::of(&json!({ "type": "term", "term": "替代弹性", "term_role": "part" }));
        assert!(own.comparable(&own, "T9"));
        assert!(!head.comparable(&part, "T9"), "a head and a part are different requirements");
        // A named rule keeps its own narrower signature.
        assert!(!book_a.comparable(&book_b, "I3"));
        assert!(own.comparable(&shared, "R13"));
    }

    #[test]
    fn a_report_counts_only_blocking_diagnostics_and_prints_the_familiar_lines() {
        let report = CheckReport::of(vec![
            diagnostic("K/1", "R3", "semicolon", "甲；乙。", "甲；乙。"),
            diagnostic("K/1", "I1", "no_claim_title", "税", "the title states no claim: 税"),
        ]);
        assert_eq!(report.blockers, 1);
        assert_eq!(
            report.lines(),
            ["K/1 R3 甲；乙。", "K/1 I1 (advisory) the title states no claim: 税"]
        );
    }

    /// S02-AC07: a rule-version change can change which fault is recognized.
    /// Two versions that agree on the text but name different fault codes are not
    /// the same occurrence, so an update cannot silently inherit the old finding.
    #[test]
    fn a_changed_fault_code_is_not_inherited() {
        let old = diagnostic("K/1", "R12", "title_size", "甲；乙", "two claims: 甲；乙");
        let new = diagnostic("K/1", "R12", "two_claims", "甲；乙", "two claims: 甲；乙");
        let report = ratchet(&[old], vec![new], |_| true);
        assert!(!report.diagnostics[0].legacy);
        assert_eq!(report.blockers, 1);
    }

    /// S03-AC03 (design 6.2): the catalog owns the default policy of both
    /// Workspaces. Human notes and agent digest Cards differ exactly where the
    /// design says they do.
    #[test]
    fn the_catalog_names_the_design_defaults_for_both_workspaces() {
        let default = |rule: &str, workspace: Workspace| {
            rule_of(rule)
                .unwrap_or_else(|| panic!("the catalog names {rule}"))
                .default(workspace)
        };
        // Local/bounded-context format blocks in both Workspaces.
        for rule in ["E1", "E2", "F1", "F2", "F3", "R18", "R19", "R22"] {
            assert_eq!(default(rule, Workspace::Human), Setting::Block, "{rule} human");
            assert_eq!(default(rule, Workspace::Agent), Setting::Block, "{rule} agent");
        }
        // Formal model rules: advice for human notes, blocking for digest Cards.
        for rule in ["R13", "R14", "R15", "R16", "R17"] {
            assert_eq!(default(rule, Workspace::Human), Setting::Advise, "{rule} human");
            assert_eq!(default(rule, Workspace::Agent), Setting::Block, "{rule} agent");
        }
        // The text and layer rules: advice for the person, block for an agent.
        for rule in [
            "R2", "R3", "R4", "R5", "R7", "R9", "R12", "I2", "I3", "I4", "I7", "L1", "L2",
        ] {
            assert_eq!(default(rule, Workspace::Human), Setting::Advise, "{rule} human");
            assert_eq!(default(rule, Workspace::Agent), Setting::Block, "{rule} agent");
        }
        // Advisory subcases stay advisory in both Workspaces.
        for rule in ["I1", "R20", "R21"] {
            assert_eq!(default(rule, Workspace::Human), Setting::Advise, "{rule} human");
            assert_eq!(default(rule, Workspace::Agent), Setting::Advise, "{rule} agent");
        }
        // R6 keeps the existing meaning-sensitive remediation decision.
        assert_eq!(default("R6", Workspace::Human), Setting::Off);
        assert_eq!(default("R6", Workspace::Agent), Setting::Advise);
        // Graph facts are audit advice, never a write gate.
        for rule in ["I5", "I6", "E4", "E5", "F4", "L3"] {
            assert_eq!(default(rule, Workspace::Human), Setting::Advise, "{rule} human");
            assert_eq!(default(rule, Workspace::Agent), Setting::Advise, "{rule} agent");
            assert_eq!(scope(rule), Scope::Graph, "{rule} is a graph rule");
        }
        // The Term rules are mechanically implemented since S09 (#152); T1, T2, T3, T5, T6,
        // T9 and T10 enforce the applicable format in both Workspaces.
        for rule in ["T1", "T2", "T3", "T5", "T6", "T9"] {
            assert_eq!(default(rule, Workspace::Human), Setting::Block, "{rule} human");
            assert_eq!(default(rule, Workspace::Agent), Setting::Block, "{rule} agent");
            assert_eq!(rule_of(rule).unwrap().state, RuleState::Configurable, "{rule}");
        }
        for rule in ["T4", "T8"] {
            assert_eq!(default(rule, Workspace::Human), Setting::Advise, "{rule} human");
            assert_eq!(default(rule, Workspace::Agent), Setting::Advise, "{rule} agent");
            assert_eq!(rule_of(rule).unwrap().state, RuleState::Configurable, "{rule}");
        }
        assert_eq!(scope("T8"), Scope::Graph, "T8 structure facts are audit advice");
        // T10 is the Term-facing label of the open-formula predicate R22.
        assert_eq!(rule_of("T10").unwrap().alias_of, Some("R22"));
        assert_eq!(default("T10", Workspace::Agent), Setting::Block);
        // T7 wrong/nonexistent targets are the existing Link write invariant.
        assert_eq!(rule_of("T7").unwrap().state, RuleState::Mandatory);
    }

    /// Issue #184: the Literature-note rows are the design levels — the Idea-card title, size and
    /// sentence rules off or advice, the source, formula, layer and Term rules unchanged — and
    /// every row names a catalogued rule the archive can also configure for other Cards.
    #[test]
    fn the_literature_note_rows_are_the_design_levels() {
        let row = |rule: &str| literature_setting(rule);
        // Off: the note states no claim, keeps no Idea title, size, duplicate-title or
        // own-source rule, and the Idea-card sentence rules do not read its prose.
        for rule in ["R2", "R3", "R4", "R5", "R7", "I1", "I6", "I7"] {
            assert_eq!(row(rule), Some(Setting::Off), "{rule}");
        }
        // Advice: a long title, a thin body, a hedge, a link with no relation word or reason,
        // and the existing advisory rows.
        for rule in ["R9", "R12", "I2", "I4", "R20", "R21", "L1", "L2", "L3"] {
            assert_eq!(row(rule), Some(Setting::Advise), "{rule}");
        }
        // Unchanged: the source line, the open formula, the Event/Finding formats, the link
        // target invariant and the Term rules keep the level their Workspace setting gives them.
        for rule in [
            "R6", "R13", "R14", "R15", "R16", "R17", "R18", "R19", "R22", "I3", "I5", "E1", "E2",
            "E4", "E5", "F1", "F2", "F3", "F4", "T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8",
            "T9", "T10",
        ] {
            assert_eq!(row(rule), None, "{rule} keeps its Workspace setting on a note");
        }
        // Every row names a catalogued rule, so a row can never outlive its rule, and the row is
        // not an archive setting: the declared overrides hold only the `gate.<workspace>.<rule>`
        // keys, so no archive can promote or silence one.
        for (rule, _) in literature_rows() {
            assert!(rule_of(rule).is_some(), "the catalog names {rule}");
        }
        assert_eq!(literature_rows().len(), 17, "every changed row is named once");
        let mut seen = std::collections::HashSet::new();
        for (rule, _) in literature_rows() {
            assert!(seen.insert(*rule), "{rule} is named once");
        }
        let set = PolicySet::new();
        assert!(set.overrides().is_empty(), "the rows are no stored override");
    }

    /// Issue #184: the policy hash covers the Literature-note rows. Two policies with the same
    /// Workspace settings therefore publish different hashes once a row changes, which is why the
    /// check contract moves with this change.
    #[test]
    fn the_policy_hash_covers_the_literature_note_rows() {
        let set = PolicySet::new();
        let canonical = set.canonical();
        assert!(canonical.starts_with("zt-gate-policy/2\n"), "{canonical}");
        let literature = canonical
            .split_once("\nliterature\n")
            .map(|(_, rows)| rows)
            .expect("the hashed policy text names the Literature-note rows");
        for (rule, setting) in literature_rows() {
            assert!(
                literature.lines().any(|line| line == format!("{rule}={}", setting.as_str())),
                "the hash covers {rule}={setting}: {literature}"
            );
        }
        // The same Workspace settings without the rows hash differently: the rows are covered.
        let workspaces = canonical.split_once("\nliterature\n").unwrap().0;
        assert_ne!(set.hash(), sha256_hex(workspaces.as_bytes()));
        // A changed row changes the hash, a stored override equal to a default does not.
        assert_eq!(set.hash(), PolicySet::new().hash());
    }

    /// S03-AC02/AC06: the catalog names every mechanically implemented rule once,
    /// and a mandatory service invariant has no rule id at all.
    #[test]
    fn the_catalog_is_complete_and_unambiguous() {
        let mut seen = std::collections::HashSet::new();
        for spec in catalog() {
            assert!(seen.insert(spec.rule), "{} is catalogued once", spec.rule);
            assert!(!spec.rule.is_empty());
        }
        for rule in [
            "R2", "R3", "R4", "R5", "R6", "R7", "R9", "R12", "R13", "R14", "R15", "R16",
            "R17", "R18", "R19", "R20", "R21", "R22", "I1", "I2", "I3", "I4", "I5", "I6",
            "I7", "E1", "E2", "E4", "E5", "F1", "F2", "F3", "F4", "L1", "L2", "L3", "T1",
            "T2", "T3", "T4", "T5", "T6", "T8", "T9",
        ] {
            assert_eq!(
                rule_of(rule).map(|spec| spec.state),
                Some(RuleState::Configurable),
                "{rule} is enforced and configurable"
            );
        }
        for name in ["envelope", "fields", "address", "links", "targets", "access", "locks"] {
            assert!(invariant(name).is_some(), "{name} is a mandatory invariant");
        }
        assert!(invariant("R2").is_none(), "a rule id is not an invariant");
    }

    /// S03-AC01/AC02/R0117-R0119: one declared key names a Workspace and a rule,
    /// and every typo, invariant, mandatory rule, alias, and graph-only blocking
    /// override is refused with an explanation.
    #[test]
    fn a_gate_key_and_its_change_are_validated_before_they_reach_a_policy() {
        assert_eq!(gate_key(Workspace::Agent, "R2"), "gate.agent.R2");
        assert_eq!(
            parse_gate_key("gate.human.E2"),
            Ok((Workspace::Human, "E2"))
        );
        assert_eq!(
            parse_gate_key("gate.agent.R2"),
            Ok((Workspace::Agent, "R2"))
        );
        let error = parse_gate_key("gate.agents.R2").unwrap_err();
        assert!(error.contains("unknown Workspace `agents`"), "{error}");
        let error = parse_gate_key("gate.agent").unwrap_err();
        assert!(error.contains("gate.<workspace>.<rule>"), "{error}");
        assert!(parse_gate_key("archive_root").is_err());
        assert!(parse_gate_key("gui_search").is_err());
        assert!(parse_gate_key("gate.agent.R2.extra").is_err());

        let mut agent = Policy::new(Workspace::Agent);
        assert_eq!(
            Change::parse("advise"),
            Ok(Change::Set(Setting::Advise))
        );
        assert_eq!(Change::parse("default"), Ok(Change::Reset));
        let error = Change::parse("warn").unwrap_err();
        assert!(error.contains("unknown policy value `warn`"), "{error}");
        assert!(error.contains("block, advise, off, or default"), "{error}");

        let error = agent
            .change("R1", Change::Set(Setting::Advise))
            .unwrap_err();
        assert!(error.contains("unknown rule `R1`"), "{error}");
        let error = agent
            .change("access", Change::Set(Setting::Off))
            .unwrap_err();
        assert!(error.contains("mandatory service invariant"), "{error}");
        assert!(error.contains("no gate policy can change it"), "{error}");
        let error = agent
            .change("T7", Change::Set(Setting::Advise))
            .unwrap_err();
        assert!(error.contains("T7 is a mandatory rule"), "{error}");
        assert!(error.contains("cannot be configured"), "{error}");
        // T4 is implemented and configurable since S09; T8 stays a graph audit rule.
        assert!(agent.change("T4", Change::Set(Setting::Block)).is_ok());
        assert!(agent.change("T4", Change::Reset).is_ok());
        let error = agent
            .change("T8", Change::Set(Setting::Block))
            .unwrap_err();
        assert!(error.contains("graph-only audit rule"), "{error}");
        let error = agent
            .change("T10", Change::Set(Setting::Advise))
            .unwrap_err();
        assert!(error.contains("Term-facing label of R22"), "{error}");
        assert!(error.contains("gate.<workspace>.R22"), "{error}");
        let error = agent
            .change("I5", Change::Set(Setting::Block))
            .unwrap_err();
        assert!(error.contains("graph-only audit rule"), "{error}");
        assert!(error.contains("cannot block a single write"), "{error}");

        // A graph rule may stay advice or be silenced; nothing starts a graph scan.
        assert!(agent.change("I5", Change::Set(Setting::Advise)).is_ok());
        assert!(agent.change("I5", Change::Set(Setting::Off)).is_ok());
        assert!(agent.change("I5", Change::Reset).is_ok());
        // Every configurable rule accepts every value it may hold.
        for rule in ["R2", "R6", "I1", "E2", "L1", "R18"] {
            let mut policy = Policy::new(Workspace::Agent);
            for setting in [Setting::Block, Setting::Advise, Setting::Off] {
                policy
                    .change(rule, Change::Set(setting))
                    .unwrap_or_else(|error| panic!("{rule} {setting}: {error}"));
            }
        }
    }

    /// S03-AC01 (R0118/R0120): a missing override means the default, an explicit
    /// reset removes it, and the policy hash names the effective policy of both
    /// Workspaces, so a change in one Workspace is a different archive policy.
    #[test]
    fn a_policy_holds_defaults_resets_and_one_stable_hash() {
        let mut agent = Policy::new(Workspace::Agent);
        assert_eq!(agent.effective("R2"), Some(Setting::Block));
        assert!(!agent.is_override("R2"));
        assert_eq!(agent.change("R2", Change::Set(Setting::Advise)), Ok(true));
        assert!(agent.is_override("R2"));
        assert_eq!(agent.effective("R2"), Some(Setting::Advise));
        // Repeating the same value changes nothing.
        assert_eq!(agent.change("R2", Change::Set(Setting::Advise)), Ok(false));
        assert_eq!(agent.change("R2", Change::Reset), Ok(true));
        assert_eq!(agent.effective("R2"), Some(Setting::Block));
        assert_eq!(agent.change("R2", Change::Reset), Ok(false));
        // A rule the catalog does not name has no policy opinion.
        assert_eq!(agent.effective("R1"), None);
        // The human Workspace keeps the design default for the same rule.
        let human = Policy::new(Workspace::Human);
        assert_eq!(human.effective("R2"), Some(Setting::Advise));
        assert_eq!(human.effective("R6"), Some(Setting::Off));
        // T10 reads R22's setting: one predicate, one decision.
        assert_eq!(agent.effective("T10"), Some(Setting::Block));
        agent.change("R22", Change::Set(Setting::Advise)).unwrap();
        assert_eq!(agent.effective("T10"), Some(Setting::Advise));

        // One archive policy: two Workspaces with independent overrides.
        let mut set = PolicySet::new();
        let before = set.hash();
        assert_eq!(
            set.change(Workspace::Human, "R2", Change::Set(Setting::Block)),
            Ok(true)
        );
        let human_blocked = set.hash();
        assert_ne!(before, human_blocked, "an effective change gets a new hash");
        assert_eq!(
            set.policy(Workspace::Human).effective("R2"),
            Some(Setting::Block)
        );
        assert_eq!(
            set.policy(Workspace::Agent).effective("R2"),
            Some(Setting::Block),
            "the other Workspace keeps its own default"
        );
        let mut agent_changed = set.clone();
        agent_changed
            .change(Workspace::Agent, "I1", Change::Set(Setting::Block))
            .unwrap();
        assert_ne!(
            agent_changed.hash(),
            human_blocked,
            "an override in the other Workspace is another policy"
        );
        // The same effective policy has one stable hash.
        let mut copy = PolicySet::from_pairs([(
            "gate.human.R2".to_string(),
            "block".to_string(),
        )])
        .unwrap();
        assert_eq!(copy.hash(), human_blocked);
        // Removing the override restores the default hash.
        copy.change(Workspace::Human, "R2", Change::Reset).unwrap();
        assert_eq!(copy.hash(), before);
        // Pinning a rule to its current default stores the row but decides the
        // same, so the effective hash does not move.
        let mut pinned = PolicySet::new();
        pinned
            .change(Workspace::Human, "R2", Change::Set(Setting::Advise))
            .unwrap();
        assert!(pinned.policy(Workspace::Human).is_override("R2"));
        assert_eq!(pinned.hash(), before);
        // The hash is a lower-case SHA-256.
        assert_eq!(before.len(), 64);
        assert!(before.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));

        // A stored row this binary cannot read is refused by name, never
        // silently weakened.
        let error = PolicySet::from_pairs([(
            "gate.agent.R2".to_string(),
            "warn".to_string(),
        )])
        .unwrap_err();
        assert!(error.contains("gate.agent.R2=warn"), "{error}");
        assert!(error.contains("unknown policy value `warn`"), "{error}");
        let error =
            PolicySet::from_pairs([("gate.aliens.R2".to_string(), "block".to_string())]).unwrap_err();
        assert!(error.contains("unknown Workspace `aliens`"), "{error}");
        let error = PolicySet::from_pairs([("gate.agent.I5".to_string(), "block".to_string())])
            .unwrap_err();
        assert!(error.contains("graph-only audit rule"), "{error}");
    }

    /// S03-AC03/AC05: the policy decides the severity of one diagnostic per rule
    /// and Workspace, drops an off rule, and never promotes an advisory subcase
    /// of an otherwise blocking rule (I7 unknown source).
    #[test]
    fn a_policy_applies_block_advise_and_off_to_a_report() {
        let report_of = |policy: &Policy| {
            policy.report(vec![
                diagnostic(
                    "K/1",
                    "R2",
                    "sentence_length",
                    "41 characters > 40: 甲",
                    "(41 characters > 40) 甲",
                ),
                diagnostic("K/1", "E2", "missing_labels", "labels:参与者", "missing labels: 参与者"),
                diagnostic("K/1", "I1", "no_claim_title", "税", "the title states no claim: 税"),
            ])
        };
        // The agent defaults: R2 and E2 block, I1 advises.
        let agent = Policy::new(Workspace::Agent);
        let report = report_of(&agent);
        assert_eq!(report.blockers, 2);
        assert_eq!(
            report.lines(),
            [
                "K/1 R2 (41 characters > 40) 甲",
                "K/1 E2 missing labels: 参与者",
                "K/1 I1 (advisory) the title states no claim: 税",
            ]
        );
        // The human defaults: R2 advises, E2 blocks.
        let human = Policy::new(Workspace::Human);
        assert_eq!(report_of(&human).blockers, 1);
        assert_eq!(
            report_of(&human).lines()[0],
            "K/1 R2 (advisory) (41 characters > 40) 甲"
        );
        // An off rule disappears from the report entirely.
        let mut quiet = human.clone();
        quiet.change("E2", Change::Set(Setting::Off)).unwrap();
        let report = report_of(&quiet);
        assert_eq!(report.diagnostics.len(), 2);
        assert!(report.diagnostics.iter().all(|diagnostic| diagnostic.rule != "E2"));
        // A block override promotes an advice rule in the human Workspace.
        let mut strict = human.clone();
        strict.change("R2", Change::Set(Setting::Block)).unwrap();
        assert_eq!(report_of(&strict).blockers, 2);
        // I7's unknown-source subcase is advice even though agent I7 blocks.
        let mut subcase = diagnostic(
            "K/2",
            "I7",
            "own_source_unknown",
            "[[b]]",
            "(advisory) source evidence unknown, so [[b]] may be an own-source premise",
        );
        subcase.severity = Severity::Block;
        subcase.subcase = true;
        let mut agent = Policy::new(Workspace::Agent);
        let report = agent.report(vec![subcase.clone()]);
        assert_eq!(report.blockers, 0, "an advisory subcase never blocks");
        assert_eq!(report.diagnostics[0].severity, Severity::Advise);
        agent.change("I7", Change::Set(Setting::Block)).unwrap();
        let report = agent.report(vec![subcase]);
        assert_eq!(report.blockers, 0, "an explicit block cannot promote it");
        assert_eq!(report.diagnostics[0].severity, Severity::Advise);
        // A rule the catalog does not name keeps the severity the core gave it.
        let mut plain = diagnostic("K/3", "R2", "sentence_length", "x", "x");
        assert_eq!(agent.decide(plain.clone()).unwrap().severity, Severity::Block);
        let unknown = Diagnostic {
            rule: "Q1".to_string(),
            ..plain.clone()
        };
        plain = unknown;
        assert_eq!(agent.decide(plain).unwrap().severity, Severity::Block);
    }
}
