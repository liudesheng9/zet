//! The empirical path of the model compiler (docs/empirical-models-proposal.md §8): the plan
//! schema of an empirical model, the role skeletons whose numbers come from the source's tables
//! (`tables.json`), the setting file every job of the model opens with, the Model Card's
//! script-written sections, and the evidence check: the role faults of the build report, the EM
//! card rules, and number fidelity (every number of an estimate, parameter, test or table part
//! stands in the source or in its tables).
//!
//! The formal path's machine is kept: the plan has the same parts (`id`, `line`, `of`, `facet`,
//! `title`, `spec`, `src`, `rests`), the compiler files them the same way, the writer fills them
//! in the model's own conversation. What differs is the role of each part, read from its facet
//! prefix (`zt_check::empirical::role_of`), and the fields a role needs.

use super::tables;
use crate::ps::{list, pipe, s, truthy};
use super::text::{derives, line_of, ref_of};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use zt_check::empirical::{DESIGNS, EmpiricalRole, allowed_derivation, role_of, symbols, table_names};

/// What a specification variant may change against its base (proposal §5.1).
pub(crate) const CHANGE_KINDS: [&str; 9] = [
    "controls",
    "fixed-effects",
    "sample",
    "estimator",
    "outcome",
    "interaction",
    "functional-form",
    "standard-errors",
    "instrument",
];

/// A plan of level `empirical`.
pub(crate) fn is_empirical(plan: &Value) -> bool {
    s(&plan["level"]).eq_ignore_ascii_case("empirical")
}

/// The role of a plan part, read from its facet.
pub(crate) fn role(part: &Value) -> Option<EmpiricalRole> {
    role_of(&s(&part["facet"]))
}

/// The design ids a plan names: a string (`did + iv`) or a list.
pub(crate) fn designs(plan: &Value) -> Vec<String> {
    let text = match &plan["design"] {
        Value::Array(items) => items.iter().map(s).collect::<Vec<_>>().join(" "),
        other => s(other),
    };
    text.to_lowercase()
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

fn id_of(parts: &[Value], id: &str) -> Option<Value> {
    parts.iter().find(|p| s(&p["id"]).eq_ignore_ascii_case(id)).cloned()
}

/// The symbols the plan's `variables` define (their first column).
fn defined(plan: &Value) -> BTreeSet<String> {
    let mut rows = String::from("| 变量 |\n| --- |\n");
    for row in list(&plan["variables"]) {
        let first = pipe(row).first().map(|v| s(v)).unwrap_or_default();
        rows.push_str(&format!("| {} |\n", first.replace('|', "\\|")));
    }
    table_names(&rows)
}

/// The empirical plan faults (proposal §8.3). `tables` is the model's `tables.json`, when the
/// brief wrote one; a cell is then checked against it.
pub(crate) fn plan_faults(plan: &Value, tables: Option<&Value>) -> Vec<String> {
    let mut faults = Vec::new();
    let named = designs(plan);
    if named.is_empty() {
        faults.push(format!("design is missing: one of {}", DESIGNS.join(", ")));
    }
    for d in &named {
        if !DESIGNS.contains(&d.as_str()) {
            faults.push(format!("design '{d}' is not a design type: {}", DESIGNS.join(", ")));
        }
    }
    if s(&plan["estimand"]).trim().is_empty() {
        faults.push("estimand is missing: what the model estimates, in words (the effect of what on what, for whom)".into());
    }
    if list(&plan["variables"]).is_empty() {
        faults.push("variables is missing: one row [symbol, meaning, measure, source, first part id] per variable and symbol of every specification".into());
    }
    let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
    let role_of_id = |id: &str| id_of(&parts, id).and_then(|p| role(&p));
    for p in &parts {
        let id = s(&p["id"]);
        if truthy(&p["take"]) {
            faults.push(format!(
                "{id}: an empirical model takes no part of another model while it is built (model-compiler v2, D2); name the theory part it tests in a hypothesis part's tests, and the import pass brings it in"
            ));
            continue;
        }
        if ref_of(p) != "new" && !truthy(&p["facet"]) {
            continue;
        }
        let Some(r) = role(p) else {
            faults.push(format!(
                "{id}: facet '{}' names no empirical role (数据, 变量表, 样本, 测量, 描述, 假说, 设计, 背景, 假设 k, 规格, 第 k 章, 估计, 表 n, 图 n, 检验, 换算, 参数, 反事实, 推导：估计式, 局限, 外部有效性)",
                s(&p["facet"])
            ));
            continue;
        };
        let line = line_of(p);
        match r {
            EmpiricalRole::Spec if line == "main" => {
                if s(&p["equation"]).trim().is_empty() && !truthy(&p["no_equation"]) {
                    faults.push(format!("{id}: a baseline specification needs equation (its TeX), or no_equation: true when the book writes none"));
                }
                for term in ["outcome", "regressor", "se"] {
                    if s(&p["terms"][term]).trim().is_empty() && !p["terms"][term].is_array() {
                        faults.push(format!("{id}: terms.{term} is missing (outcome, regressor, fe, controls, sample, estimator, se)"));
                    }
                }
            }
            EmpiricalRole::Spec => {
                let base = s(&p["base"]);
                if role_of_id(&base) != Some(EmpiricalRole::Spec) {
                    faults.push(format!("{id}: a specification variant needs base (the id of the specification it changes)"));
                }
                let kind = s(&p["change"]["kind"]);
                if !CHANGE_KINDS.contains(&kind.as_str()) {
                    faults.push(format!("{id}: change.kind '{kind}' is not one of {}", CHANGE_KINDS.join(", ")));
                }
                if s(&p["change"]["text"]).trim().is_empty() {
                    faults.push(format!("{id}: change.text is missing (the one change, in one or two lines)"));
                }
            }
            EmpiricalRole::Estimate | EmpiricalRole::Parameter => {
                if role_of_id(&s(&p["specification"])) != Some(EmpiricalRole::Spec) {
                    faults.push(format!("{id}: an estimate needs specification (the id of the specification that produced it)"));
                }
                let has_cell = p["cell"].is_object();
                if !has_cell && !truthy(&p["figure"]) && !truthy(&p["text"]) {
                    faults.push(format!(
                        "{id}: an estimate needs cell {{table,row,col}}, or figure (the figure it is read from), or text: true for a number the source states only in prose"
                    ));
                }
                if has_cell && let Some(tables) = tables {
                    let c = &p["cell"];
                    if tables::cell(tables, &s(&c["table"]), &s(&c["row"]), &s(&c["col"])).is_none() {
                        faults.push(format!(
                            "{id}: cell {} / {} / {} is not in tables.json (read its table label, row label and column head there)",
                            s(&c["table"]),
                            s(&c["row"]),
                            s(&c["col"])
                        ));
                    }
                }
                if line != "side" {
                    faults.push(format!("{id}: an estimate stands on a Side branch of its chapter (line side, of the chapter)"));
                }
            }
            EmpiricalRole::Test => {
                if !matches!(role_of_id(&s(&p["assumption"])), Some(EmpiricalRole::Assumption | EmpiricalRole::Parameter)) {
                    faults.push(format!("{id}: a test needs assumption (the id of the identifying assumption it tests)"));
                }
            }
            EmpiricalRole::Table => {
                if let Some(tables) = tables
                    && tables::find(tables, &s(&p["table"])).is_none()
                {
                    faults.push(format!("{id}: table '{}' is not in tables.json", s(&p["table"])));
                }
                if let Some(columns) = p["columns"].as_object() {
                    for (col, spec) in columns {
                        if role_of_id(&s(spec)) != Some(EmpiricalRole::Spec) {
                            faults.push(format!("{id}: column {col} maps to '{}', which is no specification of the plan", s(spec)));
                        }
                    }
                }
            }
            EmpiricalRole::Derivation if !allowed_derivation(&s(&p["facet"])) => {
                faults.push(format!(
                    "{id}: an empirical model derives only an estimating equation or an estimator (推导：估计式, 推导：估计量); a magnitude is a 换算 part, a reported number an 估计 part"
                ));
            }
            _ => {}
        }
        if derives(p) && r != EmpiricalRole::Derivation {
            faults.push(format!("{id}: a part with derive: true needs a 推导：估计式 or 推导：估计量 facet"));
        }
    }
    let main: Vec<&Value> = parts.iter().filter(|p| line_of(p) == "main" && !truthy(&p["take"])).collect();
    if main.first().and_then(|p| role(p)) != Some(EmpiricalRole::Data) && !main.first().is_some_and(|p| truthy(&p["existing"]) && !truthy(&p["facet"])) {
        faults.push("the first main-line part is the data (facet 数据)".into());
    }
    if !main.iter().any(|p| role(p) == Some(EmpiricalRole::Design)) {
        faults.push("the main line needs a design part (facet 设计)".into());
    }
    if !main.iter().any(|p| role(p) == Some(EmpiricalRole::Spec)) {
        faults.push("the main line needs the baseline specification (facet 规格：…)".into());
    }
    if !parts.iter().any(|p| matches!(role(p), Some(EmpiricalRole::Estimate | EmpiricalRole::Parameter))) {
        faults.push("the plan reports no estimate (facet 估计：…, or 参数：… for a structural parameter)".into());
    }
    for chapter in parts.iter().filter(|p| role(p) == Some(EmpiricalRole::Chapter)) {
        let id = s(&chapter["id"]);
        let holds = parts.iter().any(|p| {
            s(&p["of"]).eq_ignore_ascii_case(&id)
                && matches!(
                    role(p),
                    Some(EmpiricalRole::Estimate | EmpiricalRole::Test | EmpiricalRole::Table | EmpiricalRole::Figure | EmpiricalRole::Parameter | EmpiricalRole::Counterfactual)
                )
        });
        if !holds {
            faults.push(format!("{id}: a chapter holds at least one estimate or test (leave out a chapter the source does not have)"));
        }
    }
    if parts.iter().any(|p| role(p) == Some(EmpiricalRole::Spec)) && !parts.iter().any(|p| role(p) == Some(EmpiricalRole::Variables)) {
        faults.push("the plan has no 变量表 part (the first Side successor of the data part)".into());
    }
    // Every symbol of every equation has a variables row.
    let names = defined(plan);
    let mut missing: Vec<String> = Vec::new();
    for p in &parts {
        for symbol in symbols(&s(&p["equation"])) {
            if !names.contains(&symbol) && !missing.contains(&symbol) {
                missing.push(symbol);
            }
        }
    }
    if !missing.is_empty() {
        faults.push(format!("variables has no row for: {} (every symbol of every equation)", missing.join(", ")));
    }
    faults
}

/// The rests-on Links the compiler adds for a role (proposal §5.2): an estimate rests on its
/// specification, a variant on its base, a test on its assumption and its specification.
pub(crate) fn role_rests(part: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut add = |field: &str, why: &str| {
        let id = s(&part[field]);
        if !id.is_empty() {
            out.push((id, why.to_string()));
        }
    };
    match role(part) {
        Some(EmpiricalRole::Estimate | EmpiricalRole::Parameter) => add("specification", "这个估计由该规格得出"),
        Some(EmpiricalRole::Spec) => add("base", "本规格是它的变体"),
        Some(EmpiricalRole::Test) => {
            add("assumption", "检验这条识别假设");
            add("specification", "检验所用的规格");
        }
        _ => {}
    }
    out
}

/// What a cell's parenthesized number is, in the card's words.
fn paren_label(cell: &Value) -> &'static str {
    match s(&cell["paren"]).as_str() {
        "t" => "t 统计量",
        "z" => "z 统计量",
        _ => "标准误",
    }
}

/// The number lines of an estimate: the script writes them from the cell, so the writer never
/// types a number a cell holds. Every line is one list item (R3 forbids a semicolon).
pub(crate) fn estimate_lines(part: &Value, tables: Option<&Value>, spec: &str) -> String {
    let regressor = s(&part["regressor"]);
    let of = if regressor.is_empty() { String::new() } else { format!(" 中 {regressor}") };
    let c = &part["cell"];
    let found = tables.and_then(|t| tables::cell(t, &s(&c["table"]), &s(&c["row"]), &s(&c["col"])));
    match found {
        Some(cell) => {
            let mut lines = vec![format!(
                "- 估计量：[[{spec}]]{of}，{} {}（{} 行）。",
                s(&cell["table"]),
                s(&cell["col"]),
                s(&cell["row"])
            )];
            lines.push(format!("- 值：{}。", s(&cell["value"])));
            // The notes say what the parentheses hold: a standard error, or a t (z) statistic.
            if truthy(&cell["se"]) {
                lines.push(format!("- {}：{}。", paren_label(&cell), s(&cell["se"])));
            } else {
                lines.push("- 标准误：表中没有给出。".into());
            }
            let stars = cell["stars"].as_u64().unwrap_or(0);
            if truthy(&cell["significance"]) {
                lines.push(format!("- 显著性：{}（表中标 {stars} 颗星）。", s(&cell["significance"])));
            } else if stars > 0 {
                lines.push(format!("- 显著性：表中标 {stars} 颗星，见表注。"));
            } else {
                lines.push("- 显著性：表中没有标星。".into());
            }
            if truthy(&cell["n"]) {
                lines.push(format!("- 样本量：N = {}。", s(&cell["n"])));
            }
            lines.join("\n")
        }
        None if truthy(&part["figure"]) => format!(
            "- 估计量：[[{spec}]]{of}，{}。\n- 值：待写（从图和原文抄录）。\n- 不确定性：待写（置信区间或标准误，原书未写明时写 原书未写明）。",
            s(&part["figure"])
        ),
        None => format!(
            "- 估计量：[[{spec}]]{of}，原书正文。\n- 值：待写（从原文抄录）。\n- 不确定性：待写（标准误、t 值、置信区间或显著性，原书未写明时写 原书未写明）。"
        ),
    }
}

fn terms_lines(terms: &Value) -> String {
    let join = |v: &Value| match v {
        Value::Array(items) => items.iter().map(s).filter(|x| !x.is_empty()).collect::<Vec<_>>().join("、"),
        other => s(other),
    };
    let rows = [
        ("因变量", "outcome"),
        ("关注的回归元", "regressor"),
        ("固定效应", "fe"),
        ("控制变量", "controls"),
        ("样本与权重", "sample"),
        ("估计方法", "estimator"),
        ("标准误", "se"),
    ];
    rows.iter()
        .enumerate()
        .map(|(k, (label, field))| {
            let value = join(&terms[*field]);
            format!("{}. {label}：{}。", k + 1, if value.trim().is_empty() { "无".to_string() } else { value })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The skeleton of one part of an empirical model: its Links (written by the caller), then the
/// sections of its role, with the script-written content filled and `待写` where the writer
/// writes. A skeleton meets the EM rules, so the write gate accepts it.
pub(crate) fn skeleton(
    part: &Value,
    plan: &Value,
    links: &[String],
    resolve: &dyn Fn(&str) -> Result<String>,
    tables: Option<&Value>,
) -> Result<String> {
    let lead = links.join("\n");
    let body = match role(part) {
        Some(EmpiricalRole::Variables) => {
            let mut rows = vec![
                "本表列出本模型各规格用到的每个变量和符号。".to_string(),
                String::new(),
                "| 变量 | 含义 | 度量 | 来源 | 首次出现 |".into(),
                "| --- | --- | --- | --- | --- |".into(),
            ];
            let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
            for row in list(&plan["variables"]) {
                let cells = pipe(row).into_iter().map(s).collect::<Vec<_>>();
                let first = cells.get(4).and_then(|id| id_of(&parts, id)).map(|p| s(&p["facet"])).unwrap_or_default();
                let cell = |k: usize| cells.get(k).cloned().unwrap_or_default().replace('|', "\\|");
                rows.push(format!("| {} | {} | {} | {} | {} |", cell(0), cell(1), cell(2), cell(3), first.replace('|', "\\|")));
            }
            format!("{}\n\n{lead}", rows.join("\n"))
        }
        Some(EmpiricalRole::Design) => {
            let assumptions = list(&plan["parts"])
                .into_iter()
                .filter(|p| role(p) == Some(EmpiricalRole::Assumption) && s(&p["of"]).eq_ignore_ascii_case(&s(&part["id"])))
                .map(|p| Ok(format!("- [[{}]] {}", resolve(&s(&p["id"]))?, s(&p["title"]))))
                .collect::<Result<Vec<_>>>()?;
            let assumptions = if assumptions.is_empty() { "待写".to_string() } else { assumptions.join("\n") };
            format!("{lead}\n\n待写\n\n**变异来源**\n待写\n\n**比较**\n待写\n\n**识别假设**\n{assumptions}\n\n**威胁**\n待写")
        }
        Some(EmpiricalRole::Spec) => {
            let equation = s(&part["equation"]);
            let equation = if equation.trim().is_empty() {
                "书中没有写出估计式。".to_string()
            } else {
                format!("本规格的估计式是\n$$ {} $$", equation.trim())
            };
            let change = if truthy(&part["change"]) {
                format!("\n\n**相对基准的改动**\n{}", s(&part["change"]["text"]))
            } else {
                String::new()
            };
            format!("{lead}\n\n待写\n\n**估计式**\n{equation}\n\n**各项**\n{}{change}", terms_lines(&part["terms"]))
        }
        Some(EmpiricalRole::Estimate | EmpiricalRole::Parameter) => {
            let spec = resolve(&s(&part["specification"]))?;
            let mut supports = Vec::new();
            for h in pipe(&part["supports"]) {
                let items = pipe(h);
                let id = items.first().map(|v| s(v)).unwrap_or_default();
                if id.is_empty() {
                    continue;
                }
                let word = items.get(1).map(|v| s(v)).filter(|w| !w.is_empty()).unwrap_or_else(|| "印证".into());
                supports.push(format!("{word} [[{}]]：待写。", resolve(&id)?));
            }
            let supports = if supports.is_empty() { String::new() } else { format!("\n\n**对应**\n{}", supports.join("\n")) };
            format!(
                "待写\n\n**估计**\n{}\n\n**量级**\n待写\n\n**为什么**\n待写\n\n**依据**\n{lead}{supports}",
                estimate_lines(part, tables, &spec)
            )
        }
        Some(EmpiricalRole::Test) => format!("待写\n\n**检验什么**\n{lead}\n\n**做法**\n待写\n\n**结果**\n待写\n\n**结论**\n待写"),
        Some(EmpiricalRole::Hypothesis) => {
            let mut lines = vec![lead];
            for at in pipe(&part["tests"]).into_iter().map(s).filter(|a| !a.is_empty()) {
                lines.push(format!("参照 [[{at}]]：本假说来自它的预测。"));
            }
            lines.push(String::new());
            lines.push("待写".into());
            lines.join("\n")
        }
        Some(EmpiricalRole::Table) => {
            let mut heads = Vec::new();
            if let Some(columns) = part["columns"].as_object() {
                for (col, spec) in columns {
                    heads.push(format!("- 第 {col} 列：[[{}]]", resolve(&s(spec))?));
                }
            }
            let heads = if heads.is_empty() { String::new() } else { format!("\n\n**各列**\n{}", heads.join("\n")) };
            format!("{lead}\n\n待写{heads}")
        }
        _ => format!("{lead}\n\n待写"),
    };
    Ok(body.trim().to_string())
}

/// `setting.md` of an empirical model: what every job of the model reads after the shared
/// instructions: the question, the estimand, the design, the data part, the variables and the
/// source tables the model uses.
pub(crate) fn setting_text(plan: &Value, model: &str, resolve: &dyn Fn(&str) -> Result<String>, tables: Option<&Value>) -> Result<String> {
    let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
    let data = parts.iter().find(|p| role(p) == Some(EmpiricalRole::Data));
    let mut out = format!(
        "# 本模型（实证模型）：[[{model}]]\n\n- 设计：{}\n- 估计对象：{}\n- 模型卡：{}\n",
        designs(plan).join(" + "),
        s(&plan["estimand"]),
        s(&plan["card"])
    );
    if let Some(data) = data {
        out += &format!("- 数据：[[{}]] {}：{}\n", resolve(&s(&data["id"]))?, s(&data["title"]), s(&data["spec"]));
    }
    out += "\n## 变量表（脚本写好，各卡的符号都用这里的写法）\n\n| 变量 | 含义 | 度量 | 来源 |\n| --- | --- | --- | --- |\n";
    for row in list(&plan["variables"]) {
        let cells = pipe(row).into_iter().map(s).collect::<Vec<_>>();
        let cell = |k: usize| cells.get(k).cloned().unwrap_or_default().replace('|', "\\|");
        out += &format!("| {} | {} | {} | {} |\n", cell(0), cell(1), cell(2), cell(3));
    }
    // The tables the plan uses, as the script read them.
    let mut used: Vec<String> = Vec::new();
    for p in &parts {
        for label in [s(&p["cell"]["table"]), s(&p["table"])] {
            if !label.is_empty() && !used.iter().any(|u| u.eq_ignore_ascii_case(&label)) {
                used.push(label);
            }
        }
    }
    if let Some(tables) = tables {
        for label in used {
            if let Some(t) = tables::find(tables, &label) {
                out += &format!("\n## 原书{}（{} 第 {} 行，脚本读出）\n\n", s(&t["label"]), s(&t["file"]), s(&t["line"]));
                if !s(&t["caption"]).is_empty() {
                    out += &format!("{}\n\n", s(&t["caption"]));
                }
                for title in pipe(&t["titles"]) {
                    out += &format!("{}\n\n", s(title));
                }
                let header = pipe(&t["header"]).into_iter().map(s).collect::<Vec<_>>();
                out += &format!("| 行 | {} |\n|{}\n", header.join(" | "), " --- |".repeat(header.len() + 1));
                for row in pipe(&t["rows"]) {
                    let cells = pipe(&row["cells"]).into_iter().map(|c| s(c).replace('|', "\\|")).collect::<Vec<_>>();
                    out += &format!("| {} | {} |\n", s(&row["label"]).replace('|', "\\|"), cells.join(" | "));
                }
                if !s(&t["notes"]).is_empty() {
                    out += &format!("\n{}\n", s(&t["notes"]));
                }
            }
        }
    }
    Ok(out)
}

/// The Model Card's script-written sections: `**估计对象**`, `**设计**` (its first line names the
/// design ids, which EM5 reads) and `**主要估计**` (the core estimates with their cells).
pub(crate) fn head_sections(plan: &Value, resolve: &dyn Fn(&str) -> Result<String>, tables: Option<&Value>) -> Result<Vec<(String, String)>> {
    let mut out = vec![
        ("估计对象".to_string(), s(&plan["estimand"])),
        ("设计".to_string(), format!("{}：待写", designs(plan).join(" + "))),
    ];
    let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
    let mut core: Vec<&Value> = parts
        .iter()
        .filter(|p| matches!(role(p), Some(EmpiricalRole::Estimate | EmpiricalRole::Parameter)) && s(&p["emphasis"]).eq_ignore_ascii_case("core"))
        .collect();
    if core.is_empty() {
        core = parts.iter().filter(|p| matches!(role(p), Some(EmpiricalRole::Estimate | EmpiricalRole::Parameter))).collect();
    }
    let mut rows = vec!["| 估计 | 值 | 不确定性 | 规格 |".to_string(), "| --- | --- | --- | --- |".to_string()];
    for p in core.into_iter().take(5) {
        let at = resolve(&s(&p["id"]))?;
        let spec = resolve(&s(&p["specification"]))?;
        let c = &p["cell"];
        let found = tables.and_then(|t| tables::cell(t, &s(&c["table"]), &s(&c["row"]), &s(&c["col"])));
        let (value, uncertainty, place) = match &found {
            Some(cell) => (
                s(&cell["value"]),
                if truthy(&cell["se"]) { format!("{} {}", paren_label(cell), s(&cell["se"])) } else { "表中没有给出".into() },
                format!("{} {}", s(&cell["table"]), s(&cell["col"])),
            ),
            None => ("见估计卡".into(), "见估计卡".into(), if truthy(&p["figure"]) { s(&p["figure"]) } else { "原书正文".into() }),
        };
        let title = s(&p["title"]).replace('|', "\\|");
        rows.push(format!("| [[{at}]] {title} | {value} | {uncertainty} | [[{spec}]] {place} |"));
    }
    out.push(("主要估计".to_string(), rows.join("\n")));
    Ok(out)
}

/// The writer's lines for the script-written part of one card: the numbers it must keep
/// character for character.
pub(crate) fn kept_lines(part: &Value, tables: Option<&Value>, spec: &str) -> Option<String> {
    match role(part) {
        Some(EmpiricalRole::Estimate | EmpiricalRole::Parameter) if part["cell"].is_object() => Some(estimate_lines(part, tables, spec)),
        _ => None,
    }
}

/// Number fidelity (proposal §8.2 step 6): the numbers of an estimate, parameter, test or table
/// card that stand neither in its source lines nor in the model's tables. Numbers inside Links,
/// on the source line, after a reference word (表, 图, 式, 第, 列, 章, §, 假设, 规格) and integers up
/// to 12 are not read.
pub(crate) fn unfaithful(facet: &str, body: &str, source: &BTreeSet<String>) -> Vec<String> {
    if !matches!(
        role_of(facet),
        Some(EmpiricalRole::Estimate | EmpiricalRole::Parameter | EmpiricalRole::Test | EmpiricalRole::Table)
    ) {
        return Vec::new();
    }
    static LINK: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| regex::Regex::new(r"\[\[[^\]]*\]\]").unwrap());
    static REFERENCE: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(表|图|式|第|列|章|§|假设|规格|Table|Figure|tag\{)\s*[\(（]?\s*[A-Za-z]?\d+(?:[.\-]\d+)*|\(\d+\)|（\d+）|\d+\s*(颗星|列|章|节|行)").unwrap()
    });
    let mut out = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("来源：") || trimmed.starts_with("来源:") {
            continue;
        }
        let plain = LINK.replace_all(line, " ");
        let plain = REFERENCE.replace_all(&plain, " ");
        for number in tables::numbers(&plain) {
            let small = number.parse::<f64>().is_ok_and(|v| v.fract() == 0.0 && v.abs() <= 12.0);
            if small || source.contains(&number) || out.contains(&number) {
                continue;
            }
            out.push(number);
        }
    }
    out
}

/// The evidence check's 修正 notes for one model: number fidelity and the EM card rules of its
/// own parts, one note per card. `cards` are `(address, facet, body)` of the model's own parts.
pub(crate) fn evidence_entries(
    model: &str,
    cards: &[(String, String, String)],
    designs: &[String],
    source: &BTreeSet<String>,
) -> Vec<(String, String)> {
    let keep = "不要增删其余内容，不要移动卡片。改完照常运行 CHECK。";
    let mut out = Vec::new();
    for (address, facet, body) in cards {
        let mut lines = Vec::new();
        let wrong = unfaithful(facet, body, source);
        if !wrong.is_empty() {
            lines.push(format!(
                "数字核对：这张卡的数字 {} 不在它的原文行里，也不在本模型的原书表格里。每个数字都从原文或表格抄录；脚本写好的数字行原样保留。抄错的改对，原文没有的删去。",
                wrong.join("、")
            ));
        }
        let entry = json!({ "address": address, "type": "model", "category": "part", "facet": facet,
            "level": "empirical", "design": designs.join(","), "title": "", "body": body });
        let em = zt_check::core::Check::new(zt_check::core::Limits::default(), false, "")
            .run(&[entry])
            .into_iter()
            .filter(|line| line.contains(" EM"))
            .collect::<Vec<_>>();
        for line in em {
            let text = line.split_once(' ').map(|(_, rest)| rest).unwrap_or(&line);
            lines.push(format!("卡片格式：{text}"));
        }
        if !lines.is_empty() {
            out.push((
                address.clone(),
                format!("## 修正：[[{address}]]\n\n实证检查（模型 [[{model}]]）：\n{}\n{keep}\n", lines.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")),
            ));
        }
    }
    out
}

/// The build report's empirical role faults as 修正 notes (`close`), one per faulty card.
pub(crate) fn role_entries(model: &str, build: &Value) -> Vec<(String, String)> {
    let keep = "不要增删其余内容，不要移动卡片。改完照常运行 CHECK。";
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in ["estimate_unbound", "spec_unbound", "test_unbound", "variable_uncovered"] {
        for f in list(&build[name]) {
            out.entry(s(&f["address"])).or_default().push(s(&f["fix"]));
        }
    }
    if s(&build["level"]) == "empirical" {
        for f in list(&build["beyond_level"]) {
            out.entry(s(&f["address"])).or_default().push(s(&f["fix"]));
        }
    }
    out.into_iter()
        .map(|(at, fixes)| {
            (
                at.clone(),
                format!(
                    "## 修正：[[{at}]]\n\n构建检查（实证模型 [[{model}]]）：\n{}\n{keep}\n",
                    fixes.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n")
                ),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> Value {
        let text = "表 2：社交集群与企业特征\n\n<table><tr><td></td><td>(1)</td><td>(2)</td></tr><tr><td>Log(Employment)</td><td>0.014***(0.003)</td><td>0.012***(0.003)</td></tr><tr><td>MNE</td><td></td><td>0.047***(0.017)</td></tr><tr><td>Observations</td><td>4,618</td><td>4,618</td></tr></table>\n\n注：\\* p<0.1，\\*\\* p<0.05，\\*\\*\\* p<0.01。";
        json!({ "tables": tables::extract(text, "sections/s12.md") })
    }

    fn plan() -> Value {
        json!({
            "model": "K/5|c", "level": "empirical", "design": "cross-section", "estimand": "企业规模与社交技能要求的关联",
            "title": "社交技能需求的横截面估计", "card": "问题与主要估计。",
            "variables": [["$Social_{ft}$", "社交集群", "0/1", "职位描述", "S0"], ["$X_{ft}$", "企业特征", "对数就业", "Orbis", "S0"], ["$\\beta$", "系数", "", "", "S0"], ["$\\varepsilon$", "误差", "", "", "S0"]],
            "parts": [
                {"id": "D", "line": "main", "facet": "数据", "title": "数据：4,618 个高管搜寻", "spec": "单位与样本", "src": ["sections/s12.md:1-3"]},
                {"id": "V", "line": "side", "of": "D", "facet": "变量表", "title": "变量表", "spec": "变量", "rests": [["D", "变量来自数据"]]},
                {"id": "G", "line": "main", "facet": "设计", "title": "设计：横截面比较", "spec": "比较", "rests": [["D", "用到样本"]]},
                {"id": "A1", "line": "side", "of": "G", "facet": "假设 1", "title": "假设 1：特征先于职位描述", "spec": "时序", "rests": [["G", "设计的假设"]]},
                {"id": "S0", "line": "main", "facet": "规格：基准", "title": "规格：基准 (7)", "spec": "式 (7)", "rests": [["D", "样本"], ["G", "设计"]],
                 "equation": "Social_{ft} = \\beta X_{ft} + \\varepsilon_{ft}", "terms": {"outcome": "社交集群虚拟变量", "regressor": "企业特征", "fe": ["职位", "行业"], "se": "按企业聚类", "estimator": "OLS"}},
                {"id": "S1", "line": "side", "of": "S0", "facet": "规格：Probit", "title": "规格：Probit", "spec": "Probit", "base": "S0", "change": {"kind": "estimator", "text": "Probit 代替 OLS"}},
                {"id": "C1", "line": "main", "facet": "第 1 章：主要估计", "title": "第 1 章：大企业更要社交技能", "spec": "主要估计", "rests": [["S0", "估计"]]},
                {"id": "E1", "line": "side", "of": "C1", "facet": "估计：企业规模", "title": "估计：规模大一个标准差，概率高 3.5 个百分点", "spec": "规模系数", "emphasis": "core",
                 "specification": "S0", "regressor": "$\\beta$（对数就业）", "cell": {"table": "表 2", "row": "Log(Employment)", "col": "1"}},
                {"id": "K1", "line": "side", "of": "E1", "facet": "检验：时序", "title": "检验：特征先于描述", "spec": "时序", "assumption": "A1", "specification": "S0"}
            ]
        })
    }

    #[test]
    fn a_well_formed_empirical_plan_has_no_fault() {
        assert_eq!(plan_faults(&plan(), Some(&tables())), Vec::<String>::new());
    }

    #[test]
    fn each_empirical_plan_fault_is_named() {
        let mut p = plan();
        p["design"] = json!("regression");
        p["estimand"] = json!("");
        p["parts"][5]["change"] = json!({"kind": "luck", "text": ""});
        p["parts"][7]["specification"] = json!("");
        p["parts"][7]["cell"]["row"] = json!("Exports");
        p["parts"][8]["assumption"] = json!("");
        p["parts"][4]["equation"] = json!("Social_{ft} = \\beta X_{ft} + \\gamma Z_{ft} + \\varepsilon_{ft}");
        p["parts"].as_array_mut().unwrap().push(json!({"id": "T", "line": "side", "of": "E1", "facet": "推导：系数换算", "title": "推导", "derive": true}));
        p["parts"].as_array_mut().unwrap().push(json!({"id": "X", "line": "side", "of": "D", "facet": "设定", "title": "设定", "spec": "x"}));
        p["parts"].as_array_mut().unwrap().push(json!({"id": "Y", "take": "Other/1|a"}));
        let faults = plan_faults(&p, Some(&tables())).join("\n");
        for wanted in [
            "design 'regression' is not a design type",
            "estimand is missing",
            "change.kind 'luck'",
            "change.text is missing",
            "E1: an estimate needs specification",
            "E1: cell 表 2 / Exports / 1 is not in tables.json",
            "K1: a test needs assumption",
            "variables has no row for: \\gamma, Z",
            "T: an empirical model derives only an estimating equation",
            "X: facet '设定' names no empirical role",
            "Y: an empirical model takes no part of another model",
        ] {
            assert!(faults.contains(wanted), "missing `{wanted}` in:\n{faults}");
        }
        let mut bare = plan();
        bare["parts"] = json!([{ "id": "S0", "line": "main", "facet": "规格：基准", "title": "规格", "spec": "x", "no_equation": true,
            "terms": {"outcome": "y", "regressor": "x", "se": "稳健"} }]);
        let faults = plan_faults(&bare, None).join("\n");
        for wanted in ["the first main-line part is the data", "the main line needs a design part", "the plan reports no estimate", "no 变量表 part"] {
            assert!(faults.contains(wanted), "missing `{wanted}` in:\n{faults}");
        }
    }

    #[test]
    fn an_estimate_skeleton_writes_its_numbers_from_the_cell_and_meets_the_em_rules() {
        let resolve = |id: &str| -> Result<String> { Ok(format!("K/5|c|{}", id.to_lowercase())) };
        let p = plan();
        let estimate = &p["parts"][7];
        let body = skeleton(estimate, &p, &["依赖 [[K/5|c|s0]]：这个估计由该规格得出。".into()], &resolve, Some(&tables())).unwrap();
        assert!(body.contains("- 值：0.014。"), "{body}");
        assert!(body.contains("- 标准误：0.003。"), "{body}");
        assert!(body.contains("- 显著性：p < 0.01（表中标 3 颗星）。"), "{body}");
        assert!(body.contains("- 样本量：N = 4,618。"), "{body}");
        assert!(body.contains("表 2 (1)（Log(Employment) 行）"), "{body}");
        let entry = json!({ "address": "K/5|c|e1", "type": "model", "category": "part", "facet": "估计：企业规模",
            "level": "empirical", "title": "估计", "body": body });
        let em = zt_check::core::Check::new(zt_check::core::Limits::default(), false, "").run(&[entry]);
        assert!(!em.iter().any(|l| l.contains(" EM")), "{em:?}");
        let spec = skeleton(&p["parts"][4], &p, &[], &resolve, None).unwrap();
        assert!(spec.contains("**估计式**\n本规格的估计式是\n$$ Social_{ft}"), "{spec}");
        assert!(spec.contains("7. 标准误：按企业聚类。"), "{spec}");
        let design = skeleton(&p["parts"][2], &p, &[], &resolve, None).unwrap();
        assert!(design.contains("**识别假设**\n- [[K/5|c|a1]] 假设 1：特征先于职位描述"), "{design}");
    }

    #[test]
    fn number_fidelity_reads_only_numbers_the_source_does_not_hold() {
        let mut source: BTreeSet<String> = tables::table_numbers(&tables()).into_iter().collect();
        source.extend(tables::numbers("对数就业的一个标准差变化与概率上升 3.5 个百分点相关。"));
        let body = "规模大的企业更要社交技能。\n\n**估计**\n- 估计量：[[K/5|c|s0]] 中 β，表 2 第 (1) 列。\n- 值：0.014。\n- 标准误：0.004。\n\n**量级**\n概率高 3.5 个百分点，约 25%。\n\n来源：[[K]] §5.2";
        assert_eq!(unfaithful("估计：规模", body, &source), [".004", "25"]);
        assert!(unfaithful("数据", body, &source).is_empty());
    }
}
