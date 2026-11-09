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
                Some(table) => format!("`{symbol}` in the equation of `{at}` has no row in the 变量表 `{table}`: write the equation with the symbols the 变量表 defines, or keep the script's line 书中没有写出估计式 when the source writes no equation; a symbol the source does define gets its row in the 变量表 (the plan's variables)"),
                None => format!("`{symbol}` in the equation of `{at}` has no row: the model has no `变量表` part; file one as the first Side successor of its `数据：` part"),
            };
            out.variable_uncovered.push(json!({ "address": at, "symbol": symbol, "fix": fix }));
        }
    }
    out
}

/// What the model board shows on an empirical card's strip (docs/mockups/empirical-board.html,
/// proposal §11 P5): an estimate's value with its uncertainty as the source writes it and its
/// stars, a table card's size, a specification variant's change and the column it gives. The
/// numbers are the card's own lines, which the compiler wrote from the table cells; nothing is
/// computed. `None` for every other card, a prose value, and the baseline.
pub(crate) fn badge(facet: &str, body: &str) -> Option<Value> {
    let facet = facet.trim();
    let lines: Vec<&str> = body.lines().map(str::trim).collect();
    // `- 标签：值。` → the value without its full stop.
    let item = |label: &str| {
        lines.iter().find_map(|l| {
            let rest = l.strip_prefix("- ")?.strip_prefix(label)?;
            let rest = rest.strip_prefix('：').or_else(|| rest.strip_prefix(':'))?;
            Some(rest.trim().trim_end_matches(['。', '.']).trim().to_string())
        })
    };
    let plain = |v: &str| {
        let v = v.trim_start_matches(['-', '−', '–']);
        !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',' || c == '%') && v.chars().any(|c| c.is_ascii_digit())
    };
    if facet.starts_with("估计") {
        let value = item("值").filter(|v| plain(v))?;
        let uncertainty = item("标准误")
            .filter(|v| plain(v))
            .map(|v| format!("({v})"))
            .or_else(|| item("t 统计量").filter(|v| plain(v)).map(|v| format!("t {v}")))
            .or_else(|| item("z 统计量").filter(|v| plain(v)).map(|v| format!("z {v}")));
        let stars = item("显著性")
            .and_then(|v| {
                let at = v.find("标 ")?;
                v[at + "标 ".len()..].chars().next()?.to_digit(10)
            })
            .map(|n| "*".repeat(n as usize))
            .unwrap_or_default();
        let n = item("样本量").map(|v| v.trim_start_matches("N").trim_start_matches([' ', '=']).trim().to_string()).filter(|v| plain(v));
        return Some(json!({ "kind": "estimate", "value": value, "uncertainty": uncertainty, "stars": stars, "n": n }));
    }
    if facet.starts_with('表') {
        let rows: Vec<&str> = lines.iter().copied().filter(|l| l.starts_with('|') && l.ends_with('|')).collect();
        let (head, data) = rows.split_first()?;
        let columns = head.trim_matches('|').split('|').count().saturating_sub(1);
        let data = data.iter().filter(|r| !r.trim_matches('|').split('|').all(|c| c.trim().chars().all(|ch| ch == '-' || ch == ':'))).count();
        return Some(json!({ "kind": "table", "columns": columns, "rows": data }));
    }
    if facet.starts_with("规格") {
        let at = lines.iter().position(|l| l.trim_matches('*').trim() == "相对基准的改动")?;
        let change = lines[at + 1..].iter().find(|l| !l.is_empty())?.to_string();
        // 「对应表 2 第 (2) 列」 in the opening sentence names the column the variant gives.
        let column = lines.iter().find_map(|l| {
            let at = l.find("对应表")?;
            let rest = &l[at + "对应".len()..];
            let end = rest.find('列')?;
            Some(rest[..end].replace("第 ", "").replace('第', "").trim().to_string())
        });
        return Some(json!({ "kind": "specification", "change": change, "column": column }));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // hansen2021demand/5|c|e on the pilot box (docs/mockups/empirical-board.html).
    #[test]
    fn an_estimate_shows_its_value_its_uncertainty_as_the_source_writes_it_and_its_stars() {
        let body = "对数就业的系数是 0.014。\n\n**估计**\n- 估计量：基准中 β。\n- 值：0.014。\n- 标准误：0.003。\n- 显著性：p < 0.01（表中标 3 颗星）。\n- 样本量：N = 4,618。\n";
        assert_eq!(badge("估计：就业", body), Some(json!({"kind": "estimate", "value": "0.014", "uncertainty": "(0.003)", "stars": "***", "n": "4,618"})));
        let t = "**估计**\n- 值：0.70。\n- t 统计量：12.09。\n- 显著性：5% 水平（表中标 1 颗星）。\n- 样本量：N = 69。\n";
        assert_eq!(badge("估计：税率", t).unwrap()["uncertainty"], "t 12.09");
        let none = "**估计**\n- 值：0.005。\n- 标准误：表中没有给出。\n- 显著性：表中没有标星。\n";
        assert_eq!(badge("估计：开放度", none), Some(json!({"kind": "estimate", "value": "0.005", "uncertainty": null, "stars": "", "n": null})));
        assert_eq!(badge("估计：散文", "**估计**\n- 值：99 国中 55 国更高。\n"), None, "a prose value has no stamp");
    }

    #[test]
    fn a_table_card_shows_its_size_and_a_variant_its_change_and_column() {
        let table = "表 2 的五列都是单独回归。\n\n| 行 | (1) | (2) | (3) |\n| --- | --- | --- | --- |\n| Log(Employment) | 0.014 | 0.012 | 0.015 |\n| MNE |  | 0.047 |  |\n";
        assert_eq!(badge("表 2：社交集群与企业特征", table), Some(json!({"kind": "table", "columns": 3, "rows": 2})));
        let variant = "本规格在基准上加入 MNE 虚拟变量，对应表 2 第 (2) 列。\n\n**相对基准的改动**\n在对数就业之外加入 MNE 虚拟变量。\n\n`MNE` 的系数是 0.047。\n";
        assert_eq!(badge("规格：就业加跨国经营", variant), Some(json!({"kind": "specification", "change": "在对数就业之外加入 MNE 虚拟变量。", "column": "表 2 (2)"})));
        assert_eq!(badge("规格：基准", "本规格按式 (7) 估计。\n"), None, "the baseline changes nothing");
        assert_eq!(badge("设计", "cross-section。"), None);
    }
}
