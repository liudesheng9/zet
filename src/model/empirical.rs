//! The empirical model (level 3, docs/empirical-models-proposal.md §4–§5): the
//! roles of its parts, read from their facet prefixes, and the build rules
//! that the build report adds for a Model Card of level `empirical`.
//!
//! The roles are a fixed vocabulary, as the math units have one: `数据：`,
//! `变量表`, `样本：`, `测量：`, `描述：`, `假说：`, `设计：`, `背景：`,
//! `假设 k`, `规格：`, `第 k 章：`, `估计：`, `表 n：`, `图 n：`, `检验：`,
//! `换算：`, `参数：`, `反事实：`, `推导：估计式`, `局限：`, `外部有效性：`.
//! ZT stores no role: a part's role is its facet's prefix, so it follows the
//! facet when the author changes it.

use serde_json::{Value, json};
use std::collections::BTreeSet;

// The role vocabulary and the symbol reading live in zt-check, which the evidence check of the
// digest shares; the build report reads them from there.
pub(crate) use zt_check::empirical::{EmpiricalRole, allowed_derivation, display_formulas, role_of, symbols, table_names};

/// What the build report reads about one part.
pub(crate) struct PartView<'a> {
    pub(crate) facet: &'a dyn Fn(&str) -> Option<String>,
    pub(crate) title: &'a dyn Fn(&str) -> String,
    pub(crate) body: &'a dyn Fn(&str) -> String,
    pub(crate) rests_on: &'a dyn Fn(&str) -> Vec<String>,
}

/// The role faults of an empirical model (proposal §5.5).
#[derive(Default)]
pub(crate) struct Faults {
    pub(crate) shape: Vec<Value>,
    pub(crate) estimate_unbound: Vec<Value>,
    pub(crate) spec_unbound: Vec<Value>,
    pub(crate) test_unbound: Vec<Value>,
    pub(crate) variable_uncovered: Vec<Value>,
}

impl Faults {
    pub(crate) fn is_empty(&self) -> bool {
        self.shape.is_empty()
            && self.estimate_unbound.is_empty()
            && self.spec_unbound.is_empty()
            && self.test_unbound.is_empty()
            && self.variable_uncovered.is_empty()
    }
}

/// The role checks of one empirical model: `head` its Model Card, `main` its
/// main line in order, `in_range` the parts filed under it, `members` every
/// member (its own parts and the parts it imports).
pub(crate) fn faults(
    head: &str,
    main: &[String],
    in_range: &BTreeSet<String>,
    members: &BTreeSet<String>,
    view: &PartView<'_>,
) -> Faults {
    let role = |at: &str| (view.facet)(at).as_deref().and_then(role_of);
    let has = |at: &str, wanted: &[EmpiricalRole]| {
        (view.rests_on)(at)
            .iter()
            .any(|target| members.contains(target) && role(target).is_some_and(|r| wanted.contains(&r)))
    };
    let entry = |at: &str, fix: String| json!({ "address": at, "title": (view.title)(at), "fix": fix });
    let mut out = Faults::default();

    // empirical_shape: the main line starts with 数据, holds 设计 and 规格; the model holds an estimate.
    let main_roles = main.iter().map(|at| role(at)).collect::<Vec<_>>();
    if main_roles.first().copied().flatten() != Some(EmpiricalRole::Data) {
        let at = main.first().cloned().unwrap_or_else(|| head.to_string());
        out.shape.push(json!({
            "address": at, "lacks": "数据",
            "fix": format!("the main line of the empirical model `{head}` starts with its data: make the Model Card's first Side successor a `数据：` part (unit, sample, period, sources)"),
        }));
    }
    for (wanted, lacks, what) in [
        (EmpiricalRole::Design, "设计", "a `设计：` part (the comparison that identifies the estimate)"),
        (EmpiricalRole::Spec, "规格", "a `规格：` part (the baseline estimating equation)"),
    ] {
        if !main_roles.contains(&Some(wanted)) {
            out.shape.push(json!({
                "address": head, "lacks": lacks,
                "fix": format!("the main line of the empirical model `{head}` holds no {what}: file one on the main line after the data"),
            }));
        }
    }
    if !in_range
        .iter()
        .any(|at| matches!(role(at), Some(EmpiricalRole::Estimate | EmpiricalRole::Parameter)))
    {
        out.shape.push(json!({
            "address": head, "lacks": "估计",
            "fix": format!("the empirical model `{head}` reports no estimate: file each reported estimate as an `估计：` part (or a structural one as `参数：`) under its chapter"),
        }));
    }

    let main_set = main.iter().cloned().collect::<BTreeSet<_>>();
    for at in in_range {
        match role(at) {
            Some(EmpiricalRole::Estimate | EmpiricalRole::Parameter) if !has(at, &[EmpiricalRole::Spec]) => {
                out.estimate_unbound.push(entry(at, format!(
                    "`{at}` reports an estimate but rests on no `规格：` part of this model: write `依赖 [[<规格>]]` for the specification that produced it"
                )));
            }
            Some(EmpiricalRole::Spec) if main_set.contains(at) => {
                if !has(at, &[EmpiricalRole::Data, EmpiricalRole::Measure, EmpiricalRole::Sample]) {
                    out.spec_unbound.push(entry(at, format!(
                        "`{at}` is the baseline specification but rests on no `数据：` or `测量：` part: write `依赖 [[<数据>]]` for the data it estimates on"
                    )));
                }
            }
            Some(EmpiricalRole::Spec) if !has(at, &[EmpiricalRole::Spec]) => {
                out.spec_unbound.push(entry(at, format!(
                    "`{at}` is a specification variant but rests on no `规格：` part: a variant rests on the specification it changes (`放宽 [[<规格>]]` or `依赖 [[<规格>]]`) and states the one change"
                )));
            }
            Some(EmpiricalRole::Test) if !has(at, &[EmpiricalRole::Assumption, EmpiricalRole::Parameter]) => {
                out.test_unbound.push(entry(at, format!(
                    "`{at}` is a test but rests on no `假设` part: write `依赖 [[<假设 k>]]` for the identifying assumption it tests"
                )));
            }
            _ => {}
        }
    }

    // variable_uncovered: every symbol of a specification's display equation has a 变量表 row.
    let table = members
        .iter()
        .find(|at| role(at) == Some(EmpiricalRole::Variables))
        .cloned();
    let names = table.as_deref().map(|at| table_names(&(view.body)(at))).unwrap_or_default();
    for at in in_range {
        if role(at) != Some(EmpiricalRole::Spec) {
            continue;
        }
        let mut seen = Vec::new();
        for formula in display_formulas(&(view.body)(at)) {
            for symbol in symbols(&formula) {
                if !names.contains(&symbol) && !seen.contains(&symbol) {
                    seen.push(symbol);
                }
            }
        }
        for symbol in seen {
            let fix = match &table {
                Some(table) => format!("`{symbol}` in the equation of `{at}` has no row in the 变量表 `{table}`: add its row (meaning, measure, source, first card)"),
                None => format!("`{symbol}` in the equation of `{at}` has no row: the model has no `变量表` part; file one as the first Side successor of its `数据：` part"),
            };
            out.variable_uncovered.push(json!({ "address": at, "symbol": symbol, "fix": fix }));
        }
    }
    out
}
