//! Mathematical units: a Model Card may state one mathematical unit, named by
//! its Math form. A Theory Card (`theory`) is the way into one theory and takes
//! in whole units by Link. A unit (`definition`, `axiom`, `theorem`,
//! `proposition`, `lemma`, `corollary`) is a model whose Model Card states one
//! definition, axiom, or result, and whose parts carry its intuition, examples,
//! proof, and boundaries under a fixed facet set.
//!
//! Units chunk: a proof rests on the statement of an earlier unit (a cited
//! premise), never on a step inside its proof (a reach-in). Attention follows
//! the parts' Emphasis: `core` is the focus tier, no emphasis the support tier,
//! `aside` the detail tier. A unit asks for at most [`CORE_BUDGET`] core parts.
//! Everything here is computed from the Math form, facets, emphasis, and Links;
//! only the form is stored.

use anyhow::{Result, bail};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::model::{self, Emphasis, Facet, Level, LinkRead, RelationKind};
use crate::{Card, CardType};

/// What mathematical unit a Model Card states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum MathForm {
    Theory,
    Definition,
    Axiom,
    Theorem,
    Proposition,
    Lemma,
    Corollary,
}

/// The most `core` parts a unit asks for; its Model Card is the fourth chunk.
pub(crate) const CORE_BUDGET: usize = 3;
/// The most main results a Theory Card lists.
pub(crate) const MAIN_RESULTS: usize = 7;
/// A unit cited by this many units or models takes one tier more attention.
pub(crate) const HIGH_LOAD: usize = 3;
/// The most companions a study station shows beside its focus.
const COMPANIONS: usize = 3;

pub(crate) const FORM_NEEDS_MODEL_CARD: &str =
    "only a Model Card (a whole model, without a facet) has a Math form";

impl MathForm {
    pub(crate) const ALL: [MathForm; 7] = [
        MathForm::Theory,
        MathForm::Definition,
        MathForm::Axiom,
        MathForm::Theorem,
        MathForm::Proposition,
        MathForm::Lemma,
        MathForm::Corollary,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            MathForm::Theory => "theory",
            MathForm::Definition => "definition",
            MathForm::Axiom => "axiom",
            MathForm::Theorem => "theorem",
            MathForm::Proposition => "proposition",
            MathForm::Lemma => "lemma",
            MathForm::Corollary => "corollary",
        }
    }

    /// The Chinese name a Card uses for the form.
    pub(crate) fn name_zh(self) -> &'static str {
        match self {
            MathForm::Theory => "理论",
            MathForm::Definition => "定义",
            MathForm::Axiom => "公理",
            MathForm::Theorem => "定理",
            MathForm::Proposition => "命题",
            MathForm::Lemma => "引理",
            MathForm::Corollary => "推论",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        let value = value.trim().to_lowercase();
        MathForm::ALL
            .into_iter()
            .find(|form| value == form.id() || value == form.name_zh())
            .or(match value.as_str() {
                "def" => Some(MathForm::Definition),
                "thm" => Some(MathForm::Theorem),
                "prop" => Some(MathForm::Proposition),
                "lem" => Some(MathForm::Lemma),
                "cor" => Some(MathForm::Corollary),
                _ => None,
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown Math form `{value}`; expected theory, definition, axiom, theorem, proposition, lemma, corollary, or empty for none"
                )
            })
    }

    pub(crate) fn from_stored(value: Option<&str>) -> Option<Self> {
        value.and_then(|value| MathForm::parse(value).ok())
    }

    /// A unit states one definition, axiom, or result.
    pub(crate) fn is_unit(self) -> bool {
        self != MathForm::Theory
    }

    /// A result has a statement with hypotheses and, normally, a proof.
    pub(crate) fn is_result(self) -> bool {
        matches!(
            self,
            MathForm::Theorem | MathForm::Proposition | MathForm::Lemma | MathForm::Corollary
        )
    }
}

impl std::fmt::Display for MathForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Parse an optional form argument: empty or `none` clears it.
pub(crate) fn parse_optional_form(value: &str) -> Result<Option<MathForm>> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("none") {
        Ok(None)
    } else {
        MathForm::parse(value).map(Some)
    }
}

/// Store a Model Card's Math form; `None` clears it.
pub(crate) fn set_form(conn: &Connection, card: &Card, form: Option<MathForm>) -> Result<()> {
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Edit(card.row_id))?;
    if form.is_some() && (card.card_type != CardType::Model || card.facet.is_some()) {
        bail!(FORM_NEEDS_MODEL_CARD);
    }
    conn.execute(
        "UPDATE cards SET math_form = ?1 WHERE row_id = ?2",
        params![form.map(MathForm::id), card.row_id],
    )?;
    Ok(())
}

/// The Model Card of a unit (not a Theory Card).
pub(crate) fn is_unit_head(card: &Card) -> bool {
    card.card_type == CardType::Model
        && card.facet.is_none()
        && card.form.is_some_and(MathForm::is_unit)
}

/// The Book notation table: an Idea Card whose title names it. A proof may rest on it.
pub(crate) fn is_notation_table(title: &str) -> bool {
    let title = title.trim();
    title.starts_with("符号表") || title.to_lowercase().starts_with("notation")
}

/// What a part presents in a unit, read from its facet (the text before a colon).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    Premises,
    Notation,
    Plan,
    Step,
    Derivation,
    Conclusion,
    Intuition,
    Picture,
    Example,
    NonExample,
    Counterexample,
    HypothesisRole,
    Equivalent,
    AltProof,
    Exercise,
    History,
}

/// The fixed facet set of units: each facet in Chinese and English, and its role.
pub(crate) const FACETS: [(&str, &str, Role); 16] = [
    ("前提", "premises", Role::Premises),
    ("记号", "notation", Role::Notation),
    ("证明思路", "proof plan", Role::Plan),
    ("证明", "proof step", Role::Step),
    ("推导", "derivation", Role::Derivation),
    ("结论", "conclusion", Role::Conclusion),
    ("直觉", "intuition", Role::Intuition),
    ("图示", "picture", Role::Picture),
    ("例子", "example", Role::Example),
    ("非例", "non-example", Role::NonExample),
    ("反例", "counterexample", Role::Counterexample),
    ("假设作用", "hypothesis role", Role::HypothesisRole),
    ("等价刻画", "equivalent form", Role::Equivalent),
    ("另证", "alternative proof", Role::AltProof),
    ("习题", "exercise", Role::Exercise),
    ("历史", "history", Role::History),
];

/// The role of a facet: its text before the first colon names one of [`FACETS`].
/// `证明思路` is the proof plan, any other `证明…` a proof step.
pub(crate) fn role(facet: &Facet) -> Option<Role> {
    let id = facet.id();
    let name = id
        .split(['：', ':'])
        .next()
        .unwrap_or(id)
        .trim()
        .to_lowercase();
    FACETS
        .iter()
        .find(|(zh, en, _)| name == *zh || name == *en)
        .or_else(|| {
            // A facet such as `证明x0存在` without a colon: the longest prefix wins.
            FACETS
                .iter()
                .filter(|(zh, en, _)| name.starts_with(zh) || name.starts_with(en))
                .max_by_key(|(zh, _, _)| zh.chars().count())
        })
        .map(|(_, _, role)| *role)
}

/// The labels a statement gives its hypotheses (H1, H2, …) or clauses (C1, C2, …).
pub(crate) fn labels(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    let chars = body.chars().collect::<Vec<_>>();
    for (at, ch) in chars.iter().enumerate() {
        if !matches!(ch, '(' | '（') {
            continue;
        }
        let Some(&letter) = chars.get(at + 1) else {
            continue;
        };
        if letter != 'H' && letter != 'C' {
            continue;
        }
        let digits = chars[at + 2..]
            .iter()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        let close = chars.get(at + 2 + digits.len());
        if !digits.is_empty() && matches!(close, Some(')' | '）')) {
            let label = format!("{letter}{digits}");
            if !found.contains(&label) {
                found.push(label);
            }
        }
    }
    found
}

/// The Links of a body section: the lines after a heading or bold label that
/// names one of `names` (`## 主要结果`, `**主要结果**：`), up to the next one.
pub(crate) fn section_links(body: &str, names: &[&str]) -> Vec<String> {
    let mut inside = false;
    let mut found = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        let label = trimmed
            .strip_prefix("**")
            .map(|rest| rest.split("**").next().unwrap_or(""))
            .or_else(|| {
                trimmed
                    .starts_with('#')
                    .then(|| trimmed.trim_start_matches('#').trim())
            });
        if let Some(label) = label {
            let label = label.trim().trim_end_matches(['：', ':']).trim();
            inside = names.iter().any(|name| label.eq_ignore_ascii_case(name));
            // A bold label may carry its list on the same line.
            if !inside {
                continue;
            }
        }
        if inside {
            // The shared lexical scan names each Link's target; a code span holds none.
            for occurrence in crate::link::LinkBody::for_rendering(line).occurrences() {
                let target = occurrence.target.to_string();
                if !found.contains(&target) {
                    found.push(target);
                }
            }
        }
    }
    found
}

/// Everything a unit or theory report reads: the Cards, their texts, and who links whom.
pub(crate) struct View<'a> {
    pub(crate) by_address: &'a BTreeMap<String, Card>,
    pub(crate) parsed: &'a BTreeMap<String, crate::ParsedCard>,
    pub(crate) inbound: &'a BTreeMap<String, BTreeSet<String>>,
}

impl View<'_> {
    fn title(&self, at: &str) -> String {
        self.parsed
            .get(at)
            .map(|parsed| parsed.title.clone())
            .unwrap_or_default()
    }

    fn body(&self, at: &str) -> &str {
        self.parsed
            .get(at)
            .map_or("", |parsed| parsed.body.as_str())
    }

    fn reads(&self, at: &str) -> Vec<LinkRead> {
        model::read_links(self.body(at), str::to_string)
    }

    fn form(&self, at: &str) -> Option<MathForm> {
        self.by_address.get(at).and_then(|card| card.form)
    }

    fn reference(&self, at: &str) -> Value {
        json!({ "address": at, "title": self.title(at), "form": self.form(at).map(MathForm::id) })
    }

    /// The models a Card belongs to; a Card in no model stands for itself.
    fn citer(&self, at: &str) -> String {
        model::heads(at, self.by_address, self.inbound)
            .into_iter()
            .next()
            .unwrap_or_else(|| at.to_string())
    }

    /// The Model Card of the unit a Card belongs to, if any.
    pub(crate) fn unit_of(&self, at: &str) -> Option<String> {
        model::heads(at, self.by_address, self.inbound)
            .into_iter()
            .find(|head| self.by_address.get(head).is_some_and(is_unit_head))
    }

    /// The Theory Cards that take a unit in.
    pub(crate) fn theories_of(&self, head: &str) -> Vec<String> {
        let mut theories = self
            .inbound
            .get(head)
            .into_iter()
            .flatten()
            .filter(|source| self.form(source) == Some(MathForm::Theory))
            .cloned()
            .collect::<Vec<_>>();
        theories.sort_by(|a, b| model::address_order(a, b));
        theories
    }

    /// The units and models that cite the unit `head` as a premise (rests-on to
    /// its Model Card), each once, outside the unit itself.
    pub(crate) fn citers(&self, head: &str) -> Vec<String> {
        let mut citers = BTreeSet::new();
        for source in self.inbound.get(head).into_iter().flatten() {
            let cites = self
                .reads(source)
                .iter()
                .any(|read| read.target == head && read.kind == RelationKind::RestsOn);
            if !cites {
                continue;
            }
            let citer = self.citer(source);
            if citer != head {
                citers.insert(citer);
            }
        }
        let mut citers = citers.into_iter().collect::<Vec<_>>();
        citers.sort_by(|a, b| model::address_order(a, b));
        citers
    }

    /// The unit heads that a set of Cards rests on, outside `own`: a cited
    /// premise counts its unit; a reach-in counts the unit of the part.
    fn premises_of(&self, cards: &[String], own: &str) -> BTreeSet<String> {
        let mut premises = BTreeSet::new();
        for at in cards {
            for read in self.reads(at) {
                if read.kind != RelationKind::RestsOn {
                    continue;
                }
                let target = self.by_address.get(&read.target);
                let unit = if target.is_some_and(is_unit_head) {
                    Some(read.target.clone())
                } else {
                    self.unit_of(&read.target)
                };
                if let Some(unit) = unit
                    && unit != own
                {
                    premises.insert(unit);
                }
            }
        }
        premises
    }
}

/// The tier of a part: its emphasis as attention.
fn tier(emphasis: Option<Emphasis>) -> &'static str {
    match emphasis {
        Some(Emphasis::Core) => "focus",
        None => "support",
        Some(Emphasis::Aside) => "detail",
    }
}

/// One stop of the study order: one focus, at most three companions.
fn station(pass: &str, focus: Vec<String>, companions: Vec<String>) -> Value {
    let mut companions = companions;
    companions.retain(|at| !focus.contains(at));
    let mut seen = BTreeSet::new();
    companions.retain(|at| seen.insert(at.clone()));
    companions.truncate(COMPANIONS);
    json!({ "pass": pass, "focus": focus, "companions": companions, "tools": [], "details": [] })
}

/// The math report of a unit: its form, labels, tiers, faults, citations, and
/// study order. `parts` are its members in build order (from the build report).
pub(crate) fn unit_report(
    view: &View<'_>,
    head: &str,
    form: MathForm,
    level: Option<Level>,
    order: &[Value],
) -> Value {
    let members = order
        .iter()
        .filter_map(|value| {
            let address = value["address"].as_str()?.to_string();
            let line = value["line"].as_str().unwrap_or("off").to_string();
            let part = view.by_address.get(&address)?;
            Some((address, line, part))
        })
        .collect::<Vec<_>>();
    let role_of = |part: &Card| part.facet.as_ref().and_then(role);
    let of_role = |wanted: Role| {
        members
            .iter()
            .filter(|(_, _, part)| role_of(part) == Some(wanted))
            .map(|(address, _, _)| address.clone())
            .collect::<Vec<_>>()
    };
    let examples = of_role(Role::Example);
    let non_examples = of_role(Role::NonExample);
    let intuition = of_role(Role::Intuition)
        .into_iter()
        .chain(of_role(Role::Picture))
        .collect::<Vec<_>>();
    let plans = of_role(Role::Plan);
    let steps = members
        .iter()
        .filter(|(_, line, part)| line == "main" && role_of(part) == Some(Role::Step))
        .map(|(address, _, part)| (address.clone(), part.emphasis))
        .collect::<Vec<_>>();

    // Attention: the Model Card is always the focus; parts take their emphasis.
    let mut tiers = serde_json::Map::new();
    tiers.insert(head.to_string(), json!("focus"));
    for (address, _, part) in &members {
        tiers.insert(address.clone(), json!(tier(part.emphasis)));
    }
    let core = members
        .iter()
        .filter(|(_, line, part)| line != "shared" && part.emphasis == Some(Emphasis::Core))
        .map(|(address, _, _)| address.clone())
        .collect::<Vec<_>>();
    // The first example and the first non-example form one contrast pair: one chunk.
    let pair = matches!((examples.first(), non_examples.first()), (Some(e), Some(n)) if core.contains(e) && core.contains(n));
    let chunks = core.len() - usize::from(pair);

    let mut faults = Vec::new();
    let mut fault = |name: &str, fix: String| faults.push(json!({ "fault": name, "fix": fix }));
    if matches!(form, MathForm::Definition) && examples.is_empty() {
        fault(
            "no_example",
            format!(
                "`{head}` is a definition with no `例子` part: add one object that meets it, with the computation that shows it"
            ),
        );
    }
    if matches!(form, MathForm::Definition) && non_examples.is_empty() {
        fault(
            "no_nonexample",
            format!(
                "`{head}` is a definition with no `非例` part: add one object that fails exactly one clause, and name the clause"
            ),
        );
    }
    if matches!(form, MathForm::Theorem | MathForm::Definition) && intuition.is_empty() {
        fault(
            "no_intuition",
            format!(
                "`{head}` has no `直觉` or `图示` part: say why it should be true, in words or a chart"
            ),
        );
    }
    let formal = level != Some(Level::Concept);
    if matches!(form, MathForm::Theorem | MathForm::Proposition) && formal && plans.is_empty() {
        fault(
            "no_plan",
            format!(
                "`{head}` is a formal result with no `证明思路` part: state the method, the key idea, the crux step, and the top-level steps"
            ),
        );
    }
    let crux = steps
        .iter()
        .filter(|(_, emphasis)| *emphasis == Some(Emphasis::Core))
        .map(|(address, _)| address.clone())
        .collect::<Vec<_>>();
    if form.is_result() && formal && steps.len() >= 3 && crux.is_empty() {
        fault(
            "no_crux",
            format!(
                "`{head}` has {} proof steps and none is `core`: mark the crux step, the one that carries the idea",
                steps.len()
            ),
        );
    }
    let crux_limit = if steps.len() >= 8 { 2 } else { 1 };
    if crux.len() > crux_limit {
        fault(
            "too_many_crux",
            format!(
                "`{head}` marks {} proof steps `core`; a proof of {} steps has at most {crux_limit} crux step(s): make the others regular",
                crux.len(),
                steps.len()
            ),
        );
    }
    if chunks > CORE_BUDGET {
        fault(
            "over_budget",
            format!(
                "`{head}` has {chunks} `core` chunks ({}); a unit asks for at most {CORE_BUDGET}: make the rest regular or aside",
                core.join(", ")
            ),
        );
    }

    let labels = labels(view.body(head));
    let citers = view.citers(head);
    let used_by = group_by_theory(view, &citers);
    let theories = view.theories_of(head);
    let study = unit_study(
        view,
        head,
        form,
        &members,
        &examples,
        &non_examples,
        &intuition,
        &plans,
        &crux,
        &citers,
    );
    json!({
        "form": form.id(),
        "labels": labels,
        "tiers": tiers,
        "core": core,
        "crux": crux,
        "budget": { "chunks": chunks, "limit": CORE_BUDGET, "pair": pair },
        "faults": faults,
        "theories": theories.iter().map(|at| view.reference(at)).collect::<Vec<_>>(),
        "load": citers.len(),
        "building_block": building_block(view, head, &citers),
        "used_by": used_by,
        "study": study,
    })
}

/// A unit that units of two or more theories cite, beside its own.
fn building_block(view: &View<'_>, head: &str, citers: &[String]) -> bool {
    let own = view.theories_of(head).into_iter().collect::<BTreeSet<_>>();
    let mut others = BTreeSet::new();
    for citer in citers {
        for theory in view.theories_of(citer) {
            if !own.contains(&theory) {
                others.insert(theory);
            }
        }
    }
    others.len() + usize::from(!own.is_empty()) >= 2
}

/// Citers grouped by the theory that takes them in (`null` for a model in no theory).
fn group_by_theory(view: &View<'_>, citers: &[String]) -> Vec<Value> {
    let mut groups: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
    for citer in citers {
        let theories = view.theories_of(citer);
        if theories.is_empty() {
            groups.entry(None).or_default().push(citer.clone());
        }
        for theory in theories {
            groups.entry(Some(theory)).or_default().push(citer.clone());
        }
    }
    groups
        .into_iter()
        .map(|(theory, units)| {
            json!({
                "theory": theory.as_ref().map(|at| view.reference(at)),
                "units": units.iter().map(|at| view.reference(at)).collect::<Vec<_>>(),
            })
        })
        .collect()
}

/// The units a Card cites, as tool tabs: a lemma comes just in time.
fn tools(view: &View<'_>, at: &str, own: &str) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    view.reads(at)
        .into_iter()
        .filter(|read| read.kind == RelationKind::RestsOn && read.target != own)
        .filter(|read| view.by_address.get(&read.target).is_some_and(is_unit_head))
        .filter(|read| seen.insert(read.target.clone()))
        .map(|read| {
            let form = view.form(&read.target);
            json!({
                "address": read.target,
                "title": view.title(&read.target),
                "form": form.map(MathForm::id),
                "just_in_time": form == Some(MathForm::Lemma),
            })
        })
        .collect()
}

/// The study order of a unit in Learn mode: passes of stations, each with one
/// focus and at most three companions.
#[allow(clippy::too_many_arguments)]
fn unit_study(
    view: &View<'_>,
    head: &str,
    form: MathForm,
    members: &[(String, String, &Card)],
    examples: &[String],
    non_examples: &[String],
    intuition: &[String],
    plans: &[String],
    crux: &[String],
    citers: &[String],
) -> Vec<Value> {
    let role_of = |part: &Card| part.facet.as_ref().and_then(role);
    let of_role = |wanted: Role| {
        members
            .iter()
            .filter(|(_, _, part)| role_of(part) == Some(wanted))
            .map(|(address, _, _)| address.clone())
            .collect::<Vec<_>>()
    };
    let h = head.to_string();
    let mut stations = Vec::new();
    // The unit with the most citers among those that cite this one.
    let top_user = citers
        .iter()
        .max_by_key(|citer| view.citers(citer).len())
        .cloned();
    // Narrower and broader concepts, generalizations and special cases.
    let hierarchy = view
        .reads(head)
        .into_iter()
        .filter(|read| matches!(read.kind, RelationKind::Specializes | RelationKind::Relaxes))
        .map(|read| read.target)
        .collect::<Vec<_>>();
    if form == MathForm::Definition || form == MathForm::Axiom {
        let first_pair = examples
            .first()
            .into_iter()
            .chain(non_examples.first())
            .cloned()
            .collect::<Vec<_>>();
        if !first_pair.is_empty() {
            stations.push(station("meet", first_pair.clone(), vec![h.clone()]));
        }
        stations.push(station("meet", vec![h.clone()], first_pair.clone()));
        if let Some(first) = intuition.first() {
            stations.push(station("meet", vec![first.clone()], vec![h.clone()]));
        }
        for at in non_examples.iter().skip(1).chain(examples.iter().skip(1)) {
            stations.push(station("boundary", vec![at.clone()], vec![h.clone()]));
        }
        let mut connect = hierarchy.clone();
        connect.extend(of_role(Role::Equivalent));
        connect.extend(top_user.clone());
        stations.push(station("connect", vec![h.clone()], connect));
    } else {
        if let Some(first) = examples.first() {
            stations.push(station("meet", vec![first.clone()], vec![h.clone()]));
        }
        let mut beside = intuition.to_vec();
        beside.extend(examples.first().cloned());
        stations.push(station("meet", vec![h.clone()], beside));
        // Why: the premises, the plan, then each proof step in build order, face down.
        let mut previous: Option<String> = None;
        let mut why = Vec::new();
        for (index, (address, line, part)) in members.iter().enumerate() {
            if line != "main" {
                continue;
            }
            let role = role_of(part);
            if role == Some(Role::Derivation) {
                continue;
            }
            let mut companions = vec![h.clone()];
            let is_crux = crux.contains(address);
            if role == Some(Role::Plan) {
                companions = vec![h.clone()];
            } else if is_crux {
                companions = intuition
                    .iter()
                    .take(1)
                    .cloned()
                    .chain(plans.iter().take(1).cloned())
                    .collect();
                companions.extend(previous.clone());
            } else if let Some(previous) = &previous {
                companions = vec![previous.clone()];
                companions.extend(plans.iter().take(1).cloned());
            }
            let mut value = station("why", vec![address.clone()], companions);
            value["tools"] = json!(tools(view, address, head));
            // The Derivation parts after a step, and the parts beside it, wait as slips.
            let details = members[index + 1..]
                .iter()
                .take_while(|(_, line, part)| {
                    line == "side" || (line == "main" && role_of(part) == Some(Role::Derivation))
                })
                .map(|(address, _, _)| address.clone())
                .collect::<Vec<_>>();
            value["details"] = json!(details);
            if matches!(role, Some(Role::Step) | Some(Role::Conclusion) | None) {
                value["face_down"] = json!(true);
            }
            if is_crux {
                value["crux"] = json!(true);
            }
            why.push(value);
            previous = Some(address.clone());
        }
        stations.extend(why);
        // Boundary: each hypothesis role with the counterexamples it names.
        let counterexamples = of_role(Role::Counterexample);
        let mut met = BTreeSet::new();
        for at in of_role(Role::HypothesisRole) {
            let linked = view
                .reads(&at)
                .into_iter()
                .map(|read| read.target)
                .filter(|target| members.iter().any(|(address, _, _)| address == target))
                .collect::<Vec<_>>();
            let mut companions = counterexamples
                .iter()
                .filter(|ce| {
                    linked.contains(ce) || view.reads(ce).iter().any(|read| read.target == at)
                })
                .cloned()
                .collect::<Vec<_>>();
            met.extend(companions.iter().cloned());
            companions.extend(
                linked
                    .into_iter()
                    .filter(|target| !counterexamples.contains(target)),
            );
            stations.push(station("boundary", vec![at.clone()], companions));
        }
        for at in counterexamples.iter().filter(|ce| !met.contains(*ce)) {
            stations.push(station("boundary", vec![at.clone()], vec![h.clone()]));
        }
        let mut connect = citers
            .iter()
            .filter(|citer| view.form(citer) == Some(MathForm::Corollary))
            .cloned()
            .collect::<Vec<_>>();
        connect.extend(hierarchy.clone());
        connect.extend(top_user.clone());
        let mut value = station("connect", vec![h.clone()], connect);
        value["details"] = json!(
            members
                .iter()
                .filter(|(_, _, part)| matches!(
                    role_of(part),
                    Some(Role::AltProof | Role::History | Role::Notation)
                ))
                .map(|(address, _, _)| address.clone())
                .collect::<Vec<_>>()
        );
        stations.push(value);
    }
    // Recall (Review mode): the statement face down, then the exercises.
    let mut recall = station("recall", vec![h.clone()], Vec::new());
    recall["face_down"] = json!(true);
    stations.push(recall);
    for at in of_role(Role::Exercise) {
        let mut value = station("recall", vec![at], vec![h.clone()]);
        value["face_down"] = json!(true);
        stations.push(value);
    }
    stations
}

/// The theory report of a Theory Card: its units in dependency order with
/// their weights, the cycles and forward citations among them, the tools it
/// takes from other theories, the theories that use it, and its study passes.
pub(crate) fn theory_report(view: &View<'_>, head: &str) -> Value {
    let body = view.body(head);
    let mut units = Vec::new();
    for read in model::read_links(body, str::to_string) {
        if view.by_address.get(&read.target).is_some_and(is_unit_head)
            && !units.contains(&read.target)
        {
            units.push(read.target);
        }
    }
    let member_set = units.iter().cloned().collect::<BTreeSet<_>>();
    let main_results = section_links(body, &["主要结果", "main results"])
        .into_iter()
        .filter(|at| member_set.contains(at))
        .collect::<Vec<_>>();
    let objects = section_links(body, &["对象", "objects"])
        .into_iter()
        .filter(|at| member_set.contains(at))
        .collect::<Vec<_>>();
    // Each unit's Cards: its Model Card and its parts.
    let cards_of = |unit: &str| {
        let mut cards = vec![unit.to_string()];
        cards.extend(
            model::parts_of(unit, view.by_address, view.parsed)
                .into_iter()
                .map(|part| part.address.clone()),
        );
        cards
    };
    // The Cards of a unit's statement: its Model Card and its `前提` parts.
    let statement_of = |unit: &str| {
        let mut cards = vec![unit.to_string()];
        cards.extend(
            model::parts_of(unit, view.by_address, view.parsed)
                .into_iter()
                .filter(|part| part.facet.as_ref().and_then(role) == Some(Role::Premises))
                .map(|part| part.address.clone()),
        );
        cards
    };
    // All premises (statement and proof) decide the order; the statement
    // premises alone are what a reader needs to read a unit at a step.
    let premises = units
        .iter()
        .map(|unit| (unit.clone(), view.premises_of(&cards_of(unit), unit)))
        .collect::<BTreeMap<_, _>>();
    let statement_premises = units
        .iter()
        .map(|unit| (unit.clone(), view.premises_of(&statement_of(unit), unit)))
        .collect::<BTreeMap<_, _>>();
    // Dependency order: each unit after the units it cites; book order breaks ties.
    let mut placed = Vec::new();
    let mut done = BTreeSet::new();
    loop {
        let mut ready = units
            .iter()
            .filter(|unit| !done.contains(*unit))
            .filter(|unit| {
                premises[*unit]
                    .iter()
                    .all(|premise| !member_set.contains(premise) || done.contains(premise))
            })
            .cloned()
            .collect::<Vec<_>>();
        if ready.is_empty() {
            break;
        }
        ready.sort_by(|a, b| model::address_order(a, b));
        let next = ready.remove(0);
        done.insert(next.clone());
        placed.push(next);
    }
    // What is left lies on or behind a cycle.
    let mut left = units
        .iter()
        .filter(|unit| !done.contains(*unit))
        .cloned()
        .collect::<Vec<_>>();
    left.sort_by(|a, b| model::address_order(a, b));
    let reaches = |from: &str, to: &str| {
        let mut stack = vec![from.to_string()];
        let mut seen = BTreeSet::new();
        while let Some(at) = stack.pop() {
            for premise in premises.get(&at).into_iter().flatten() {
                if premise == to {
                    return true;
                }
                if member_set.contains(premise) && seen.insert(premise.clone()) {
                    stack.push(premise.clone());
                }
            }
        }
        false
    };
    let cycle = left
        .iter()
        .filter(|unit| reaches(unit, unit))
        .map(|unit| {
            json!({
                "unit": view.reference(unit),
                "fix": format!("`{unit}` rests, through other units, on itself: one proof must cite an earlier statement instead"),
            })
        })
        .collect::<Vec<_>>();
    placed.extend(left);
    let mut forward = Vec::new();
    for unit in &units {
        for premise in &premises[unit] {
            if member_set.contains(premise)
                && model::address_order(premise, unit) == std::cmp::Ordering::Greater
            {
                forward.push(json!({
                    "from": unit,
                    "to": premise,
                    "note": format!("`{unit}` cites `{premise}`, which the book states later"),
                }));
            }
        }
    }
    // Tools: units of other theories that this theory's units cite.
    let mut tools = BTreeSet::new();
    for unit in &units {
        for premise in &premises[unit] {
            if !member_set.contains(premise) {
                tools.insert(premise.clone());
            }
        }
    }
    let tools = tools
        .into_iter()
        .map(|tool| {
            let mut value = view.reference(&tool);
            value["theories"] = json!(view.theories_of(&tool));
            value
        })
        .collect::<Vec<_>>();
    // Users: theories (or models in none) whose units cite this theory's units.
    let mut users: BTreeMap<Option<String>, BTreeSet<String>> = BTreeMap::new();
    for unit in &units {
        for citer in view.citers(unit) {
            if member_set.contains(&citer) {
                continue;
            }
            let theories = view.theories_of(&citer);
            if theories.is_empty() {
                users.entry(None).or_default().insert(citer.clone());
            }
            for theory in theories.into_iter().filter(|theory| theory != head) {
                users.entry(Some(theory)).or_default().insert(citer.clone());
            }
        }
    }
    let users = users
        .into_iter()
        .map(|(theory, citers)| {
            json!({
                "theory": theory.as_ref().map(|at| view.reference(at)),
                "units": citers.iter().map(|at| view.reference(at)).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    // Weights on the theory board: form, place in the Theory Card, and load.
    let weight = |unit: &str| {
        let form = view.form(unit);
        let base =
            if main_results.iter().any(|at| at == unit) || objects.iter().any(|at| at == unit) {
                2
            } else {
                match form {
                    Some(MathForm::Lemma) => 0,
                    _ => 1,
                }
            };
        let load = view.citers(unit).len();
        let raised = (base + usize::from(load >= HIGH_LOAD)).min(2);
        (["detail", "support", "focus"][raised], load)
    };
    let order = placed
        .iter()
        .map(|unit| {
            let (weight, load) = weight(unit);
            let mut value = view.reference(unit);
            value["weight"] = json!(weight);
            value["load"] = json!(load);
            value["level"] = json!(view.by_address[unit].level.map(Level::id));
            value["cites"] = json!(
                premises[unit]
                    .iter()
                    .filter(|premise| member_set.contains(*premise))
                    .collect::<Vec<_>>()
            );
            value["statement_cites"] = json!(
                statement_premises[unit]
                    .iter()
                    .filter(|premise| member_set.contains(*premise))
                    .collect::<Vec<_>>()
            );
            if main_results.contains(unit) {
                value["main"] = json!(true);
            }
            if objects.contains(unit) {
                value["object"] = json!(true);
            }
            value
        })
        .collect::<Vec<_>>();
    let mut faults = Vec::new();
    if main_results.len() > MAIN_RESULTS {
        faults.push(json!({
            "fault": "over_budget",
            "fix": format!("`{head}` lists {} main results; a theory lists at most {MAIN_RESULTS}: split the chapter into two or more theories", main_results.len()),
        }));
    }
    if units.is_empty() {
        faults.push(json!({
            "fault": "empty",
            "fix": format!("`{head}` takes in no unit: list its units in its 结构 section"),
        }));
    }
    // Study passes: orient, then each main result's prerequisite path. A lemma
    // comes just in time, at its step, by its statement only, so the path
    // follows a lemma's statement premises and leaves its proof premises.
    let position = placed
        .iter()
        .enumerate()
        .map(|(index, unit)| (unit.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut met = BTreeSet::new();
    let mut path = Vec::new();
    let mut mains = main_results.clone();
    mains.sort_by(|a, b| model::address_order(a, b));
    for main in &mains {
        let mut needed = BTreeSet::new();
        let mut stack = vec![main.clone()];
        while let Some(at) = stack.pop() {
            let followed = if at != *main && view.form(&at) == Some(MathForm::Lemma) {
                &statement_premises
            } else {
                &premises
            };
            for premise in followed.get(&at).into_iter().flatten() {
                if member_set.contains(premise) && needed.insert(premise.clone()) {
                    stack.push(premise.clone());
                }
            }
        }
        let mut steps = needed
            .into_iter()
            .filter(|unit| view.form(unit) != Some(MathForm::Lemma))
            .filter(|unit| !met.contains(unit))
            .collect::<Vec<_>>();
        steps.sort_by_key(|unit| position.get(unit).copied().unwrap_or(usize::MAX));
        met.extend(steps.iter().cloned());
        met.insert(main.clone());
        path.push(json!({ "result": main, "path": steps }));
    }
    let study = json!([
        { "pass": "orient", "focus": [head], "companions": main_results.iter().take(COMPANIONS).collect::<Vec<_>>() },
        { "pass": "path", "results": path },
        { "pass": "connect", "focus": [head], "tools": tools.iter().map(|tool| tool["address"].clone()).collect::<Vec<_>>(), "users": users.iter().map(|user| user["theory"]["address"].clone()).collect::<Vec<_>>() },
        { "pass": "recall", "focus": main_results, "face_down": true },
    ]);
    json!({
        "form": MathForm::Theory.id(),
        "units": units.len(),
        "order": order,
        "main_results": main_results,
        "objects": objects,
        "cycle": cycle,
        "forward": forward,
        "tools": tools,
        "users": users,
        "faults": faults,
        "study": study,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_parse_in_both_languages_and_only_theories_are_not_units() {
        assert_eq!(MathForm::parse("定理").unwrap(), MathForm::Theorem);
        assert_eq!(MathForm::parse(" Lemma ").unwrap(), MathForm::Lemma);
        assert_eq!(MathForm::parse("prop").unwrap(), MathForm::Proposition);
        assert!(MathForm::parse("formal").is_err());
        assert_eq!(parse_optional_form("none").unwrap(), None);
        assert!(!MathForm::Theory.is_unit());
        assert!(MathForm::Corollary.is_result());
        assert!(!MathForm::Definition.is_result());
    }

    #[test]
    fn facets_name_their_role_by_the_text_before_the_colon() {
        let role_of = |facet: &str| role(&Facet::parse(facet).unwrap());
        assert_eq!(role_of("证明思路"), Some(Role::Plan));
        assert_eq!(role_of("证明：x0 存在"), Some(Role::Step));
        assert_eq!(role_of("证明x0存在"), Some(Role::Step));
        assert_eq!(role_of("推导：右侧点"), Some(Role::Derivation));
        assert_eq!(role_of("假设作用：H1"), Some(Role::HypothesisRole));
        assert_eq!(role_of("反例：有理数域"), Some(Role::Counterexample));
        assert_eq!(role_of("non-example"), Some(Role::NonExample));
        assert_eq!(role_of("Proof plan"), Some(Role::Plan));
        assert_eq!(role_of("设定"), None);
    }

    #[test]
    fn labels_and_sections_are_read_from_the_body() {
        let body = "**陈述**：\n- (H1) $f$ 连续。\n- （H2） $f(a)<c$。\n- (C1) 定义。见 (H1)。";
        assert_eq!(labels(body), ["H1", "H2", "C1"]);
        let theory = "**问题**：为什么？[[b/1]]\n\n**主要结果**：\n- [[b/4|d]]：介值定理。\n- [[b/4|e]]\n\n**对象**：[[b/4|b]]\n\n## 结构\n- [[b/4|c]]";
        assert_eq!(section_links(theory, &["主要结果"]), ["b/4|d", "b/4|e"]);
        assert_eq!(section_links(theory, &["对象"]), ["b/4|b"]);
        assert_eq!(section_links(theory, &["结构"]), ["b/4|c"]);
    }

    #[test]
    fn notation_tables_are_named_by_their_title() {
        assert!(is_notation_table("符号表：《分析》"));
        assert!(is_notation_table("Notation of the book"));
        assert!(!is_notation_table("连续"));
    }
}
