//! Formal economic models (level 2, docs/formal-models-proposal.md §4–§5, §10):
//! the 15 canonical roles read from facet prefixes (shared from `zt_check::formal`), Layer 1
//! build-report faults (`formal_shape`, `setting_dependency`, `criterion_unbound`,
//! `result_unbound`, `proof_unbound`, `experiment_unbound`, `symbol_uncovered`), accurate
//! proof-route counting, and level-dispatched board badges.
//!
//! Applied to every Level 2 formal economic model (`level == Formal && form.is_none()`),
//! excluding mathematical units carrying `form`, concept models, and empirical models.

use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(crate) use zt_check::formal::{
    FormalRole, borrowed_notation_targets, claim_objects_of, display_formulas, family_tags_of,
    find_appendix_proof, find_bracket_cite, find_claim_label, has_justification,
    is_local_route_clause, notation_symbols, role_of, section, symbols,
};

/// What the build report reads about one Card for Level 2 formal checks.
pub(crate) struct PartView<'a> {
    pub(crate) facet: &'a dyn Fn(&str) -> Option<String>,
    pub(crate) title: &'a dyn Fn(&str) -> String,
    pub(crate) body: &'a dyn Fn(&str) -> String,
    pub(crate) rests_on: &'a dyn Fn(&str) -> Vec<String>,
}

/// Layer 1 build-report faults of a Level 2 formal economic model (proposal §5.5).
#[derive(Default)]
pub(crate) struct Faults {
    pub(crate) formal_shape: Vec<Value>,
    pub(crate) setting_dependency: Vec<Value>,
    pub(crate) criterion_unbound: Vec<Value>,
    pub(crate) result_unbound: Vec<Value>,
    pub(crate) proof_unbound: Vec<Value>,
    pub(crate) experiment_unbound: Vec<Value>,
    pub(crate) symbol_uncovered: Vec<Value>,
}

impl Faults {
    pub(crate) fn is_empty(&self) -> bool {
        self.formal_shape.is_empty()
            && self.setting_dependency.is_empty()
            && self.criterion_unbound.is_empty()
            && self.result_unbound.is_empty()
            && self.proof_unbound.is_empty()
            && self.experiment_unbound.is_empty()
            && self.symbol_uncovered.is_empty()
    }
}

fn is_under(address: &str, ancestor: &str) -> bool {
    address.len() > ancestor.len()
        && address.starts_with(ancestor)
        && address.as_bytes()[ancestor.len()] == b'|'
}

/// Compute Layer 1 role and dependency faults for one Level 2 formal economic model.
pub(crate) fn faults(
    head: &str,
    head_body: &str,
    main: &[String],
    in_range: &BTreeSet<String>,
    members: &BTreeSet<String>,
    view: &PartView<'_>,
) -> Faults {
    let role = |at: &str| (view.facet)(at).as_deref().and_then(role_of);
    let entry = |at: &str, fix: String| json!({ "address": at, "title": (view.title)(at), "fix": fix });
    let mut out = Faults::default();

    // 1. formal_shape: main starts with <head>|a (Setting), has <head>|a|a (Notation, or imported/bound),
    // and holds at least one Result part.
    let expected_setting = format!("{head}|a");
    let expected_notation = format!("{head}|a|a");
    let first_main = main.first().map(String::as_str);
    if first_main != Some(expected_setting.as_str())
        || role(&expected_setting) != Some(FormalRole::Setting)
    {
        let at = first_main.unwrap_or(head);
        out.formal_shape.push(json!({
            "address": at,
            "lacks": "设定",
            "fix": format!("the formal model `{head}` must start its main line at `{expected_setting}` with a `设定` facet"),
        }));
    }
    let has_local_notation = in_range.contains(&expected_notation)
        && role(&expected_notation) == Some(FormalRole::Notation);
    let has_imported_notation = members
        .iter()
        .any(|at| !in_range.contains(at) && role(at) == Some(FormalRole::Notation));
    if !has_local_notation && !has_imported_notation {
        out.formal_shape.push(json!({
            "address": head,
            "lacks": "符号表与读法",
            "fix": format!("the formal model `{head}` lacks `{expected_notation}` (`符号表与读法`, or an imported notation card)"),
        }));
    }
    let has_result = in_range.iter().any(|at| role(at) == Some(FormalRole::Result));
    if !has_result {
        out.formal_shape.push(json!({
            "address": head,
            "lacks": "结果",
            "fix": format!("the formal model `{head}` holds zero `结果：` parts"),
        }));
    }

    // 2. setting_dependency: no Setting/Assumption under a Chapter, and no Setting/Assumption
    // resting on a Chapter or Result/Derivation/Argument/Experiment/Calibration.
    let chapters: Vec<&String> = main
        .iter()
        .filter(|at| role(at) == Some(FormalRole::Chapter))
        .collect();
    for at in in_range {
        let r = role(at);
        if matches!(r, Some(FormalRole::Setting | FormalRole::Assumption)) {
            if let Some(ch) = chapters.iter().find(|ch| is_under(at, ch)) {
                out.setting_dependency.push(entry(
                    at,
                    format!(
                        "`{at}` ({}) is filed under chapter `{ch}`: move maintained settings and assumptions into the setting tree under `{expected_setting}`",
                        r.unwrap().id()
                    ),
                ));
            }
            for target in (view.rests_on)(at) {
                if in_range.contains(&target)
                    && matches!(
                        role(&target),
                        Some(
                            FormalRole::Chapter
                                | FormalRole::Result
                                | FormalRole::Derivation
                                | FormalRole::Argument
                                | FormalRole::Experiment
                        )
                    )
                {
                    out.setting_dependency.push(entry(
                        at,
                        format!(
                            "`{at}` is a setting/assumption but rests on later local chapter/result part `{target}`: settings must precede results"
                        ),
                    ));
                }
            }
        }
    }

    // 2b. Duplicate local assumption labels across the same model (e.g. two owned `假设 1` or `假设 (B)`).
    let mut assumption_labels: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for at in in_range {
        if role(at) == Some(FormalRole::Assumption)
            && let Some(facet) = (view.facet)(at)
        {
            let pref = facet
                .find(['：', ':'])
                .map_or(facet.as_str(), |i| &facet[..i])
                .split_whitespace()
                .collect::<String>();
            if let Some(label) = pref.strip_prefix("假设")
                && !label.is_empty()
            {
                let norm = label.replace('（', "(").replace('）', ")").to_lowercase();
                if let Some(prev) = assumption_labels.insert(norm, at.clone()) {
                    out.setting_dependency.push(entry(
                        at,
                        format!(
                            "`{at}` duplicates assumption label `{pref}` already owned by `{prev}`: keep one assumption per label or disambiguate"
                        ),
                    ));
                }
            }
        }
    }

    // 3. criterion_unbound: primary claim object requires a reachable Criterion in members/imports,
    // or a Result declares a criterion when none is reachable.
    let claim_objects = claim_objects_of(head_body);
    let has_criterion = members.iter().any(|at| role(at) == Some(FormalRole::Criterion));
    let needs_criterion = claim_objects.iter().any(|obj| {
        matches!(
            obj.as_str(),
            "market-equilibrium" | "strategic-allocation" | "identification"
        )
    });
    if needs_criterion && !has_criterion {
        out.criterion_unbound.push(entry(
            head,
            format!(
                "the formal model `{head}` declares {} but has no reachable `判据` / `均衡定义` / `解概念` card in its members or imports",
                claim_objects.join(", ")
            ),
        ));
    }

    // 4. result_unbound & 5. proof_unbound
    let results_and_benchmarks: Vec<&String> = in_range
        .iter()
        .filter(|at| matches!(role(at), Some(FormalRole::Result | FormalRole::Benchmark)))
        .collect();
    for at in in_range {
        if role(at) == Some(FormalRole::Result) {
            let body = (view.body)(at);
            let has_source = body.lines().any(|l| {
                let t = l.trim_start();
                t.starts_with("来源：") || t.starts_with("来源:")
            });
            let just = section(&body, "依据");
            let has_just = just
                .as_deref()
                .is_some_and(has_justification);
            if !has_source || !has_just {
                let missing = match (has_source, has_just) {
                    (false, false) => "source attribution (`来源：`) and a justification disposition in `**依据**`",
                    (false, true) => "source attribution (`来源：`)",
                    (true, false) => "a declared justification disposition in `**依据**`",
                    (true, true) => unreachable!(),
                };
                out.result_unbound.push(entry(
                    at,
                    format!("`{at}` is a `结果` part without {missing}"),
                ));
            }
            // If any non-inline local route clause in `**依据**` declares a local proof/argument/heuristic
            // route, verify that clause links a direct Side proof-step route root (`推导`/`论证`) under this result.
            if let Some(j) = just.as_deref() {
                let mut missing_route = false;
                for line in j.lines() {
                    for clause in line.split(['；', ';']) {
                        if !is_local_route_clause(clause) {
                            continue;
                        }
                        let linked_local_proof = zt_check::text::link_occurrences(clause)
                            .into_iter()
                            .filter(|occ| occ.form == zt_check::text::LinkForm::Ordinary)
                            .any(|occ| {
                                let target = occ.target;
                                in_range.contains(target)
                                    && is_direct_side_child(target, at)
                                    && role(target).is_some_and(FormalRole::is_proof_step)
                            });
                        if !linked_local_proof {
                            missing_route = true;
                            break;
                        }
                    }
                    if missing_route {
                        break;
                    }
                }
                if missing_route {
                    out.proof_unbound.push(entry(
                        at,
                        format!(
                            "`{at}` declares a local proof route in `**依据**` that links no `推导`/`论证` route root filed directly on its Side branch"
                        ),
                    ));
                }
            }
            if !has_criterion
                && let Some(stmt) = section(&body, "陈述")
                && (stmt.contains("均衡定义") || stmt.contains("解概念") || stmt.contains("判据"))
            {
                out.criterion_unbound.push(entry(
                    at,
                    format!("`{at}` references a solution criterion in `**陈述**`, but the model has no reachable `判据`/`均衡定义`/`解概念` card"),
                ));
            }
        }
    }

    // Check every Derivation/Argument in range: must be filed under a Result or Benchmark,
    // must belong to a declared proof route of its owning Result (catching orphan steps under
    // external-only Results; H10), and must not rest on its owning Result/Benchmark circularly.
    let declared_routes_for_result = |owner_at: &str| -> BTreeSet<String> {
        let mut reachable = BTreeSet::new();
        let owner_body = (view.body)(owner_at);
        let Some(just) = section(&owner_body, "依据") else {
            return reachable;
        };
        let mut roots: Vec<String> = Vec::new();
        for line in just.lines() {
            for clause in line.split(['；', ';']) {
                if !is_local_route_clause(clause) {
                    continue;
                }
                for occ in zt_check::text::link_occurrences(clause) {
                    if occ.form == zt_check::text::LinkForm::Ordinary
                        && in_range.contains(occ.target)
                        && is_direct_side_child(occ.target, owner_at)
                        && role(occ.target).is_some_and(FormalRole::is_proof_step)
                        && !roots.iter().any(|r| r == occ.target)
                    {
                        roots.push(occ.target.to_string());
                    }
                }
            }
        }
        for root in roots {
            let mut cur = root;
            while in_range.contains(&cur)
                && role(&cur).is_some_and(FormalRole::is_proof_step)
                && reachable.insert(cur.clone())
            {
                cur = next_direct_address(&cur);
            }
        }
        reachable
    };
    let mut routes_by_result: std::collections::BTreeMap<&str, BTreeSet<String>> = std::collections::BTreeMap::new();
    for at in in_range {
        if role(at).is_some_and(FormalRole::is_proof_step) {
            let owner = results_and_benchmarks
                .iter()
                .filter(|r| is_under(at, r))
                .max_by_key(|r| r.len())
                .copied();
            match owner {
                None => {
                    out.proof_unbound.push(entry(
                        at,
                        format!(
                            "`{at}` is a proof step (`推导`/`论证`) not filed on any `结果` or `基准` Side branch"
                        ),
                    ));
                }
                Some(owner_at) => {
                    if (view.rests_on)(at).iter().any(|t| t == owner_at) {
                        out.proof_unbound.push(entry(
                            at,
                            format!(
                                "`{at}` rests on its own goal `{owner_at}` as a premise (circular): name `{owner_at}` as 目标, never 依赖"
                            ),
                        ));
                    }
                    if role(owner_at) == Some(FormalRole::Result) {
                        let declared = routes_by_result
                            .entry(owner_at.as_str())
                            .or_insert_with(|| declared_routes_for_result(owner_at));
                        if !declared.contains(at) {
                            out.proof_unbound.push(entry(
                                at,
                                format!(
                                    "`{at}` is filed under Result `{owner_at}` but is not on any local proof route declared in `{owner_at}`'s `**依据**`"
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }

    // 6. experiment_unbound: a 数值实验 card must rest on a Calibration or Setting/Benchmark/Criterion.
    for at in in_range {
        if role(at) == Some(FormalRole::Experiment) {
            let bound = (view.rests_on)(at).iter().any(|target| {
                members.contains(target)
                    && matches!(
                        role(target),
                        Some(
                            FormalRole::Calibration
                                | FormalRole::Setting
                                | FormalRole::Benchmark
                                | FormalRole::Criterion
                        )
                    )
            });
            if !bound {
                out.experiment_unbound.push(entry(
                    at,
                    format!(
                        "`{at}` is a `数值实验` card resting on no `校准` or `设定` member: link its baseline parameterization and regime"
                    ),
                ));
            }
        }
    }

    // 7. symbol_uncovered: every symbol in Setting or Criterion display equations appears in local
    // or imported/borrowed 符号表与读法.
    let notation_cards: Vec<String> = members
        .iter()
        .filter(|at| role(at) == Some(FormalRole::Notation))
        .cloned()
        .collect();
    let primary_notation = notation_cards
        .iter()
        .find(|at| in_range.contains(*at))
        .or_else(|| notation_cards.first())
        .cloned();
    let mut defined_symbols = BTreeSet::new();
    for at in &notation_cards {
        let body = (view.body)(at);
        defined_symbols.extend(notation_symbols(&body));
        for target in borrowed_notation_targets(&body) {
            let target_body = (view.body)(&target);
            if !target_body.is_empty() {
                defined_symbols.extend(notation_symbols(&target_body));
            }
        }
    }
    for at in in_range {
        if !matches!(role(at), Some(FormalRole::Setting | FormalRole::Criterion)) {
            continue;
        }
        let body = (view.body)(at);
        let mut seen = Vec::new();
        for formula in display_formulas(&body) {
            for symbol in symbols(&formula) {
                if !defined_symbols.contains(&symbol) && !seen.contains(&symbol) {
                    seen.push(symbol);
                }
            }
        }
        for symbol in seen {
            let fix = match &primary_notation {
                Some(not_at) => format!(
                    "`{symbol}` in a display equation of `{at}` has no row in `{not_at}` (`符号表与读法`): add `{symbol}` with its meaning and first-use Link"
                ),
                None => format!(
                    "`{symbol}` in a display equation of `{at}` has no notation row: add `{expected_notation}` (`符号表与读法`)"
                ),
            };
            out.symbol_uncovered
                .push(json!({ "address": at, "symbol": symbol, "fix": fix }));
        }
    }

    out
}

/// Whether `address` is a direct Side child of `parent` (`<parent>|<letters>`).
fn is_direct_side_child(address: &str, parent: &str) -> bool {
    address
        .strip_prefix(parent)
        .and_then(|rest| rest.strip_prefix('|'))
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_lowercase()))
}

/// The next Direct successor of `address` along a Side branch (`<root>` -> `<root>|1`, `<root>|k` -> `<root>|(k+1)`).
fn next_direct_address(address: &str) -> String {
    if let Some((prefix, last)) = address.rsplit_once('|')
        && !last.is_empty()
        && last.chars().all(|c| c.is_ascii_digit())
        && let Ok(n) = last.parse::<u64>()
    {
        return format!("{prefix}|{}", n + 1);
    }
    format!("{address}|1")
}

/// Filter the descendants of a `Result` or `Benchmark` on a Level 2 formal board so `proof.steps`
/// counts ONLY `Derivation` and `Argument` Cards along its declared proof route(s) (proposal §10
/// item 2 & Astra #13):
/// - For a `Result` with `**依据**`, resolve each ordinary Link in `**依据**` whose target is a
///   direct Side proof-step child of `parent`, and walk ONLY those declared route roots and their
///   Direct continuations (`<root>|1`, `<root>|2`, ...). An external-only Result with no declared
///   local route therefore reports 0 proof steps even if unrelated descendants exist.
/// - For a `Benchmark` (which has no `**依据**` section), walk direct Side proof-step branches of
///   `parent` and their Direct continuations, excluding `边界`, `例示`, and nested sub-Benchmarks.
pub(crate) fn proof_route_addresses<'a>(
    parent: &str,
    body: &str,
    block: &'a [&'a Value],
) -> Vec<&'a str> {
    let mut by_addr: std::collections::BTreeMap<&'a str, &'a Value> = std::collections::BTreeMap::new();
    for &m in block {
        if let Some(addr) = m["address"].as_str() {
            by_addr.insert(addr, m);
        }
    }
    let is_proof_node = |m: &Value| -> bool {
        let facet = m["facet"].as_str().unwrap_or_default();
        m["derivation"] == json!(true) || role_of(facet).is_some_and(FormalRole::is_proof_step)
    };
    let mut roots: Vec<&'a str> = Vec::new();
    if let Some(just) = section(body, "依据") {
        // Parse only non-external, non-inline local route clauses so links in external/inline
        // citations never inflate proof.steps (H10).
        for line in just.lines() {
            for clause in line.split(['；', ';']) {
                if !is_local_route_clause(clause) {
                    continue;
                }
                for occ in zt_check::text::link_occurrences(clause) {
                    if occ.form == zt_check::text::LinkForm::Ordinary
                        && is_direct_side_child(occ.target, parent)
                        && let Some((&addr, &node)) = by_addr.get_key_value(occ.target)
                        && is_proof_node(node)
                        && !roots.contains(&addr)
                    {
                        roots.push(addr);
                    }
                }
            }
        }
    } else if role_of(
        block
            .iter()
            .find(|m| m["address"].as_str() == Some(parent))
            .and_then(|m| m["facet"].as_str())
            .unwrap_or_default(),
    ) != Some(FormalRole::Result)
    {
        for &m in block {
            if let Some(addr) = m["address"].as_str()
                && is_direct_side_child(addr, parent)
                && is_proof_node(m)
                && !roots.contains(&addr)
            {
                roots.push(addr);
            }
        }
    }
    let mut out = Vec::new();
    for root in roots {
        let mut cur: Option<&'a str> = Some(root);
        while let Some(addr) = cur {
            if out.contains(&addr) {
                break;
            }
            let Some(&node) = by_addr.get(addr) else {
                break;
            };
            if !is_proof_node(node) {
                break;
            }
            out.push(addr);
            let next = next_direct_address(addr);
            cur = by_addr.get_key_value(next.as_str()).map(|(&k, _)| k);
        }
    }
    out
}

/// Level 2 formal board strip badge (proposal §10):
/// - `Result`: claim label (`命题 1`, `定理 2`, `未编号`), optional case/scope tag, and accurate
///   justification stamp (`证明 · 3 步`, `构造 · 1 步`, `外部证明 [4]`, `启发式 · 2 步`, `数值实验`, `原文未证`).
///   Never emits a fabricated green "proved" checkmark.
/// - `Criterion`: criterion kind (`均衡定义`, `解概念`, `判据`).
/// - `Calibration`: parameter count + external-estimate count.
/// - `Experiment`: `数值` + short case tag.
/// - Variant `Setting`: `变体` + qualifier.
pub(crate) fn badge(facet: &str, body: &str, proof_steps: usize) -> Option<Value> {
    let role = role_of(facet)?;
    let qualifier = facet
        .find(['：', ':'])
        .map(|at| facet[at + facet[at..].chars().next().unwrap().len_utf8()..].trim())
        .filter(|s| !s.is_empty());
    match role {
        FormalRole::Result => {
            let stmt = section(body, "陈述").unwrap_or_default();
            // Prefer authoritative claim label from **陈述**, falling back to facet (M3).
            let label = find_claim_label(&stmt)
                .or_else(|| find_claim_label(facet))
                .unwrap_or_else(|| "未编号".to_string());
            let scope = extract_scope_tag(&stmt);
            let just = section(body, "依据").unwrap_or_default();
            let justification = if proof_steps > 0 {
                if just.contains("启发式") && !just.contains("局部证明") {
                    Some(format!("启发式 · {proof_steps} 步"))
                } else if just.contains("局部数值") && !just.contains("局部证明") {
                    Some(format!("数值 · {proof_steps} 步"))
                } else if just.contains("局部构造") && !just.contains("局部证明") {
                    Some(format!("构造 · {proof_steps} 步"))
                } else if just.contains("局部图示") && !just.contains("局部证明") {
                    Some(format!("图示 · {proof_steps} 步"))
                } else if let Some(app) = find_appendix_proof(&just) {
                    Some(format!("{app}证明 · {proof_steps} 步"))
                } else {
                    Some(format!("证明 · {proof_steps} 步"))
                }
            } else if just.contains("外部证明") || just.contains("外部引证") || just.contains("引证") {
                if let Some(c) = find_bracket_cite(&just) {
                    Some(format!("外部证明 {c}"))
                } else {
                    Some("外部证明".to_string())
                }
            } else if just.contains("外部构造") || just.contains("外部图示") || just.contains("外部启发式") {
                if let Some(c) = find_bracket_cite(&just) {
                    Some(format!("外部依据 {c}"))
                } else {
                    Some("外部依据".to_string())
                }
            } else if just.contains("数值实验") {
                Some("数值实验".to_string())
            } else if just.contains("补写") {
                Some("补写证明".to_string())
            } else if just.contains("局部证明") || just.contains("局部构造") || just.contains("局部图示") || just.contains("局部论证") {
                Some("行内论证".to_string())
            } else if just.contains("原文未证") || just.contains("猜想") {
                Some("原文未证".to_string())
            } else {
                None
            };
            Some(json!({
                "kind": "result",
                "label": label,
                "scope": scope,
                "justification": justification,
            }))
        }
        FormalRole::Criterion => {
            let label = facet
                .find(['：', ':'])
                .map_or(facet, |at| &facet[..at])
                .trim();
            Some(json!({ "kind": "criterion", "label": label }))
        }
        FormalRole::Calibration => {
            let table = section(body, "参数与出处").unwrap_or_default();
            let mut params = 0usize;
            let mut external = 0usize;
            for line in table.lines() {
                let t = line.trim();
                if !t.starts_with('|') {
                    continue;
                }
                let cells: Vec<&str> = t.trim_matches('|').split('|').map(str::trim).collect();
                if cells.is_empty()
                    || cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '))
                    || matches!(cells[0], "参数" | "符号" | "Parameter" | "Symbol")
                {
                    continue;
                }
                params += 1;
                if t.to_lowercase().contains("external-estimate") || t.contains("外部估计") {
                    external += 1;
                }
            }
            if params == 0 {
                return None;
            }
            Some(json!({ "kind": "calibration", "params": params, "external": external }))
        }
        FormalRole::Experiment => {
            let case_tag = qualifier.map(|q| {
                let clipped: String = q.chars().take(18).collect();
                clipped
            });
            Some(json!({ "kind": "experiment", "case": case_tag }))
        }
        FormalRole::Setting => {
            let q = qualifier?;
            if q.contains("变体") || q.contains("体制") {
                let cleaned = q
                    .trim_start_matches("变体")
                    .trim_start_matches("体制")
                    .trim_start_matches(['一', '二', '三', '四', '五', '：', ':', ' ']);
                let label = if cleaned.is_empty() { q } else { cleaned };
                let clipped: String = label.chars().take(18).collect();
                Some(json!({ "kind": "variant", "label": clipped }))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_scope_tag(stmt: &str) -> Option<String> {
    for line in stmt.lines() {
        let t = line.trim().trim_start_matches(['-', '*', '•']).trim();
        if let Some(rest) = t.strip_prefix("作用域").or_else(|| t.strip_prefix("情形")) {
            let val = rest
                .trim_start_matches(['：', ':', ' '])
                .trim_end_matches(['。', '.'])
                .trim();
            if !val.is_empty() {
                let clipped: String = val.chars().take(18).collect();
                return Some(clipped);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formal_badges_render_claim_label_and_honest_justification() {
        let prop1 = "固定供给下国内块按比例缩放。\n\n**陈述**\n- 出处与力度：命题 1（原文 §5.2）；力度：characterization。\n- 作用域：固定供给\n\n**依据**\n- 局部证明：附录 B 证明见 [[K/1|a|1|a|a]]。\n";
        assert_eq!(
            badge("结果：命题 1 条件比例性", prop1, 3),
            Some(json!({
                "kind": "result",
                "label": "命题 1",
                "scope": "固定供给",
                "justification": "附录B证明 · 3 步"
            }))
        );

        let mirrlees_thm1 = "激励相容充要条件。\n\n**陈述**\n- 出处与力度：定理 1；力度：iff。\n\n**依据**\n- 外部证明：严格证明见原文引证 [4]。\n";
        assert_eq!(
            badge("结果：定理 1 激励相容", mirrlees_thm1, 0),
            Some(json!({
                "kind": "result",
                "label": "定理 1",
                "scope": null,
                "justification": "外部证明 [4]"
            }))
        );

        let spence_unnumbered = "存在一族分离均衡。\n\n**陈述**\n- 出处与力度：未编号（§5）；力度：sufficient。\n\n**依据**\n- 局部论证：见 [[K/1|a|1|a|a]]。\n";
        assert_eq!(
            badge("结果：一族分离均衡", spence_unnumbered, 1),
            Some(json!({
                "kind": "result",
                "label": "未编号",
                "scope": null,
                "justification": "证明 · 1 步"
            }))
        );

        assert_eq!(
            badge("均衡定义：两国竞争均衡", "", 0),
            Some(json!({ "kind": "criterion", "label": "均衡定义" }))
        );
        assert_eq!(
            badge("设定：变体一：ZLB", "", 0),
            Some(json!({ "kind": "variant", "label": "ZLB" }))
        );
    }

    #[test]
    fn proof_route_excludes_boundary_and_example_siblings() {
        let r = "K/1|a|1|a";
        let d1 = json!({ "address": "K/1|a|1|a|a", "facet": "推导：第一步", "derivation": true });
        let a2 = json!({ "address": "K/1|a|1|a|a|1", "facet": "论证：排除爆炸解", "derivation": false });
        let b1 = json!({ "address": "K/1|a|1|a|b", "facet": "边界：线性近似", "derivation": false });
        let e1 = json!({ "address": "K/1|a|1|a|c", "facet": "例示：数值例", "derivation": false });
        let block = vec![&d1, &a2, &b1, &e1];
        let local_body = "**依据**\n- 局部证明：见 [[K/1|a|1|a|a]]。";
        assert_eq!(proof_route_addresses(r, local_body, &block), ["K/1|a|1|a|a", "K/1|a|1|a|a|1"]);
        // Astra #13: an external-only Result declares no local route in **依据** and reports 0 steps.
        let external_body = "**依据**\n- 外部证明：见 [4]。";
        assert_eq!(proof_route_addresses(r, external_body, &block), Vec::<&str>::new());
    }
}
