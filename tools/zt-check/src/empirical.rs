//! The empirical model (level 3, docs/empirical-models-proposal.md §5): the
//! role vocabulary of its parts, read from their facet prefixes, the symbol
//! reading the build report and the evidence check share, and the card-format
//! rules EM1–EM5 for its parts.
//!
//! - EM1: an `估计：` part has the labels `估计`, `量级` and `依据`, and its
//!   `估计` section links its specification and names a table, a figure or the
//!   source's text.
//! - EM2: an `估计：` part states a value and an uncertainty (s.e., t, CI, p, a
//!   significance level) or says `原书未写明`.
//! - EM3: a `规格：` part has `估计式` (a display equation, or `书中没有写出估计式`)
//!   and `各项` naming the outcome, the regressor of interest and the standard errors.
//! - EM4: a `设计：` part has `变异来源` and `识别假设`.
//! - EM5 (advice): a causal verb in an `估计：` part of a model whose design is
//!   `descriptive` or `cross-section`.

use crate::core::Faults;
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

/// The role of one part of an empirical model (proposal §5.1).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmpiricalRole {
    Data,
    Variables,
    Sample,
    Measure,
    Descriptive,
    Hypothesis,
    Design,
    Background,
    Assumption,
    Spec,
    Chapter,
    Estimate,
    Table,
    Figure,
    Test,
    Conversion,
    Parameter,
    Counterfactual,
    Derivation,
    Limits,
}

impl EmpiricalRole {
    /// The role's id, as a build report names it.
    pub fn id(self) -> &'static str {
        match self {
            EmpiricalRole::Data => "data",
            EmpiricalRole::Variables => "variables",
            EmpiricalRole::Sample => "sample",
            EmpiricalRole::Measure => "measure",
            EmpiricalRole::Descriptive => "descriptive",
            EmpiricalRole::Hypothesis => "hypothesis",
            EmpiricalRole::Design => "design",
            EmpiricalRole::Background => "background",
            EmpiricalRole::Assumption => "assumption",
            EmpiricalRole::Spec => "spec",
            EmpiricalRole::Chapter => "chapter",
            EmpiricalRole::Estimate => "estimate",
            EmpiricalRole::Table => "table",
            EmpiricalRole::Figure => "figure",
            EmpiricalRole::Test => "test",
            EmpiricalRole::Conversion => "conversion",
            EmpiricalRole::Parameter => "parameter",
            EmpiricalRole::Counterfactual => "counterfactual",
            EmpiricalRole::Derivation => "derivation",
            EmpiricalRole::Limits => "limits",
        }
    }
}

/// The design types of an empirical model (proposal §4.3).
pub const DESIGNS: [&str; 12] = [
    "cross-section",
    "many-unit",
    "panel",
    "did",
    "event-study",
    "iv",
    "shift-share",
    "rd",
    "experiment",
    "structural",
    "measurement",
    "descriptive",
];

/// The text of a facet before its first `：` (or `:`), trimmed.
fn prefix(facet: &str) -> &str {
    facet.find(['：', ':']).map_or(facet, |at| &facet[..at]).trim()
}

/// The role a facet names, or none for a facet outside the empirical vocabulary.
pub fn role_of(facet: &str) -> Option<EmpiricalRole> {
    let compact = prefix(facet).split_whitespace().collect::<String>();
    let numbered = |word: &str| {
        compact.strip_prefix(word).is_some_and(|rest| {
            !rest.is_empty()
                && rest
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || ".-′'一二三四五六七八九十".contains(c))
        })
    };
    Some(match compact.as_str() {
        "数据" => EmpiricalRole::Data,
        "变量表" => EmpiricalRole::Variables,
        "样本" => EmpiricalRole::Sample,
        "测量" => EmpiricalRole::Measure,
        "描述" => EmpiricalRole::Descriptive,
        "假说" => EmpiricalRole::Hypothesis,
        "设计" => EmpiricalRole::Design,
        "背景" => EmpiricalRole::Background,
        "规格" => EmpiricalRole::Spec,
        "估计" => EmpiricalRole::Estimate,
        "检验" => EmpiricalRole::Test,
        "换算" => EmpiricalRole::Conversion,
        "参数" => EmpiricalRole::Parameter,
        "反事实" => EmpiricalRole::Counterfactual,
        "局限" | "外部有效性" => EmpiricalRole::Limits,
        _ if compact.starts_with("推导") => EmpiricalRole::Derivation,
        _ if numbered("假设") => EmpiricalRole::Assumption,
        _ if compact.starts_with('第') && compact.ends_with('章') => EmpiricalRole::Chapter,
        _ if numbered("表") => EmpiricalRole::Table,
        _ if numbered("图") => EmpiricalRole::Figure,
        _ => return None,
    })
}

/// A Derivation part an empirical model may hold: one that derives an
/// estimating equation or an estimator (`推导：估计式`, `推导：估计量`).
pub fn allowed_derivation(facet: &str) -> bool {
    facet
        .trim()
        .strip_prefix("推导")
        .map(|rest| rest.trim_start_matches(['：', ':', ' ']))
        .is_some_and(|rest| rest.starts_with("估计式") || rest.starts_with("估计量"))
}

const GREEK: [&str; 40] = [
    "alpha", "beta", "gamma", "delta", "epsilon", "varepsilon", "zeta", "eta", "theta", "vartheta",
    "iota", "kappa", "lambda", "mu", "nu", "xi", "pi", "varpi", "rho", "varrho", "sigma",
    "varsigma", "tau", "upsilon", "phi", "varphi", "chi", "psi", "omega", "Gamma", "Delta",
    "Theta", "Lambda", "Xi", "Pi", "Sigma", "Upsilon", "Phi", "Psi", "Omega",
];

/// Font commands whose group names one symbol (`\text{Post}`, `\mathbf{X}`).
const NAMING: [&str; 10] = [
    "text", "textrm", "textit", "mathrm", "mathit", "mathbf", "boldsymbol", "mathcal", "mathsf", "bm",
];

/// Commands whose group is not a symbol (`\tag{1}`, `\operatorname{Var}`).
const SKIPPED_GROUP: [&str; 4] = ["tag", "label", "operatorname", "mathbb"];

/// Letter runs that name a function, not a variable.
const FUNCTIONS: [&str; 12] = [
    "log", "ln", "exp", "max", "min", "lim", "sup", "inf", "cov", "var", "corr", "Var",
];

/// The group that starts at `chars[at]` (`{…}`, balanced), or the one token
/// there (a letter, a digit, a command): its end index and its text.
fn group(chars: &[char], at: usize) -> (usize, String) {
    if at >= chars.len() {
        return (at, String::new());
    }
    if chars[at] == '{' {
        let mut depth = 0usize;
        for (i, c) in chars.iter().enumerate().skip(at) {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return (i + 1, chars[at + 1..i].iter().collect());
                    }
                }
                _ => {}
            }
        }
        return (chars.len(), chars[at + 1..].iter().collect());
    }
    if chars[at] == '\\' {
        let mut end = at + 1;
        while end < chars.len() && chars[end].is_ascii_alphabetic() {
            end += 1;
        }
        if end == at + 1 {
            end = (at + 2).min(chars.len());
        }
        return (end, chars[at..end].iter().collect());
    }
    (at + 1, chars[at].to_string())
}

/// The symbols of one TeX formula, in order of first appearance: Greek
/// letters (`\beta`), letter runs (`SR`, `y`), and font groups (`\text{Post}`).
/// Subscripts, superscripts, tags and functions are no symbols, so `y_{ct}`
/// names `y` and `\gamma_j` names `\gamma`.
pub fn symbols(tex: &str) -> Vec<String> {
    let chars = tex.chars().collect::<Vec<_>>();
    let mut out: Vec<String> = Vec::new();
    let mut push = |name: String| {
        if !name.is_empty() && !out.contains(&name) {
            out.push(name);
        }
    };
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            let mut end = i + 1;
            while end < chars.len() && chars[end].is_ascii_alphabetic() {
                end += 1;
            }
            let name = chars[i + 1..end].iter().collect::<String>();
            if name.is_empty() {
                i = (i + 2).min(chars.len());
                continue;
            }
            if GREEK.contains(&name.as_str()) {
                push(format!("\\{name}"));
                i = end;
            } else if NAMING.contains(&name.as_str()) {
                let (next, inner) = group(&chars, end);
                let inner = inner.trim().to_string();
                if inner.chars().all(|c| c.is_ascii_alphanumeric()) {
                    push(inner);
                } else {
                    for symbol in symbols(&inner) {
                        push(symbol);
                    }
                }
                i = next;
            } else if SKIPPED_GROUP.contains(&name.as_str()) {
                i = group(&chars, end).0;
            } else {
                i = end;
            }
        } else if c == '_' || c == '^' {
            // OCR writes `_ {f t}`: the group may stand after spaces.
            let mut at = i + 1;
            while at < chars.len() && chars[at] == ' ' {
                at += 1;
            }
            i = group(&chars, at).0;
        } else if c.is_ascii_alphabetic() {
            let mut end = i;
            while end < chars.len() && chars[end].is_ascii_alphabetic() {
                end += 1;
            }
            let mut run = chars[i..end].iter().collect::<String>();
            // OCR spells a name out letter by letter (`S o c i a l`): TeX shows one name.
            if run.len() == 1 {
                let mut letters = vec![chars[i]];
                let mut at = end;
                loop {
                    let mut next = at;
                    while next < chars.len() && chars[next] == ' ' {
                        next += 1;
                    }
                    let single = next < chars.len()
                        && next > at
                        && chars[next].is_ascii_alphabetic()
                        && !chars.get(next + 1).is_some_and(|c| c.is_ascii_alphabetic());
                    if !single {
                        break;
                    }
                    letters.push(chars[next]);
                    at = next + 1;
                }
                if letters.len() >= 3 {
                    run = letters.into_iter().collect();
                    end = at;
                }
            }
            if !FUNCTIONS.contains(&run.as_str()) {
                push(run);
            }
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// The display formulas (`$$ … $$`) of a body.
pub fn display_formulas(body: &str) -> Vec<String> {
    body.split("$$")
        .enumerate()
        .filter(|(index, _)| index % 2 == 1)
        .map(|(_, formula)| formula.to_string())
        .collect()
}

/// The names a 变量表 card defines: the symbols of the first cell of every
/// table row, and a plain first cell as one name.
pub fn table_names(body: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in body.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }
        let first = line.trim_start_matches('|').split('|').next().unwrap_or("").trim();
        if first.is_empty() || first.chars().all(|c| c == '-' || c == ':' || c == ' ') {
            continue;
        }
        if first.contains('$') {
            for (index, piece) in first.split('$').enumerate() {
                if index % 2 == 1 {
                    names.extend(symbols(piece));
                }
            }
        } else {
            names.insert(first.to_string());
            names.extend(symbols(first));
        }
    }
    names
}

/// A model's design types, read from the first line of its Model Card's
/// `**设计**` section (proposal §5.4): every design id the line names.
pub fn design_of(model_body: &str) -> Vec<String> {
    let Some(section) = section(model_body, "设计") else {
        return Vec::new();
    };
    let first = section.lines().find(|line| !line.trim().is_empty()).unwrap_or("");
    let lower = first.to_lowercase();
    DESIGNS
        .iter()
        .filter(|design| {
            lower
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '-'))
                .any(|word| word == **design)
        })
        .map(|design| design.to_string())
        .collect()
}

static LABEL_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\*\*([^*]+)\*\*\s*[：:]?\s*(.*)$").expect("a label line"));

/// The text under one bold label (`**估计**`) up to the next bold label, or
/// none when the body has no such label. Text after the label on its own line
/// belongs to the section.
pub fn section(body: &str, name: &str) -> Option<String> {
    let mut out: Option<Vec<String>> = None;
    for line in body.lines() {
        // The source line ends every section.
        if out.is_some() && (line.trim_start().starts_with("来源：") || line.trim_start().starts_with("来源:")) {
            break;
        }
        if let Some(caps) = LABEL_LINE.captures(line) {
            if out.is_some() {
                break;
            }
            if caps[1].trim() == name {
                let rest = caps[2].trim();
                out = Some(if rest.is_empty() { Vec::new() } else { vec![rest.to_string()] });
            }
            continue;
        }
        if let Some(lines) = out.as_mut() {
            lines.push(line.to_string());
        }
    }
    out.map(|lines| lines.join("\n"))
}

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[[^\]]+\]\]").expect("a link"));
static TABLE_OR_FIGURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(表|图|Table|Figure|table|figure)\s*[A-Za-z]?\d|正文|原书文字").expect("a table reference")
});
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d").expect("a digit"));
static UNCERTAINTY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"标准误|s\.\s?e\.|\bse\b|t\s*[=＝]|t\s*值|t\s*统计量|z\s*[=＝]|置信区间|\bCI\b|p\s*[=＝<＜]|p\s*值|显著|不显著|原书未写明")
        .expect("an uncertainty word")
});
static CAUSAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"导致|造成|使得|引起|提高了|降低了|促进了|抑制了|\bcauses?\b|\bcaused\b|\bleads? to\b")
        .expect("a causal verb")
});

/// The EM rules for one part of an empirical model. `designs` are the model's
/// design types (EM5 reads them; none when the caller does not know them).
pub(crate) fn rules(facet: &str, address: &str, body: &str, designs: &[String], faults: &mut Faults) {
    match role_of(facet) {
        Some(EmpiricalRole::Estimate) => estimate_rules(address, body, designs, faults),
        Some(EmpiricalRole::Spec) => spec_rules(address, body, faults),
        Some(EmpiricalRole::Design) => {
            let missing = ["变异来源", "识别假设"]
                .into_iter()
                .filter(|label| section(body, label).is_none())
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                faults.add(
                    address,
                    "EM4",
                    "missing_labels",
                    &format!("labels:{}", missing.join(",")),
                    &format!("a design part lacks the labels: {}", missing.join("、")),
                );
            }
        }
        _ => {}
    }
}

fn estimate_rules(address: &str, body: &str, designs: &[String], faults: &mut Faults) {
    let missing = ["估计", "量级", "依据"]
        .into_iter()
        .filter(|label| section(body, label).is_none())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        faults.add(
            address,
            "EM1",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!("an estimate part lacks the labels: {}", missing.join("、")),
        );
    }
    if let Some(estimate) = section(body, "估计") {
        if !LINK.is_match(&estimate) {
            faults.add(
                address,
                "EM1",
                "no_spec_link",
                "",
                "the 估计 section links no specification: name the 规格 part that produced the estimate",
            );
        }
        if !TABLE_OR_FIGURE.is_match(&estimate) {
            faults.add(
                address,
                "EM1",
                "no_cell",
                "",
                "the 估计 section names no table, figure or text of the source: say where the number stands (表 n 第 k 列, 图 n, 正文)",
            );
        }
        let unknown = estimate.contains("原书未写明");
        if !NUMBER.is_match(&estimate) && !unknown {
            faults.add(address, "EM2", "no_value", "", "the 估计 section states no value");
        }
        if !UNCERTAINTY.is_match(&estimate) {
            faults.add(
                address,
                "EM2",
                "no_uncertainty",
                "",
                "the 估计 section states no uncertainty (标准误, t, 置信区间, p, a significance level) and does not say 原书未写明",
            );
        }
    }
    if designs.iter().any(|design| design == "descriptive" || design == "cross-section") {
        if let Some(found) = CAUSAL.find(body) {
            faults.add(
                address,
                "EM5",
                "causal_verb",
                found.as_str(),
                &format!(
                    "`{}` reads as a causal claim, but the model's design ({}) estimates an association: say 相关 or 与……有关",
                    found.as_str(),
                    designs.join(", ")
                ),
            );
        }
    }
}

fn spec_rules(address: &str, body: &str, faults: &mut Faults) {
    let mut missing = Vec::new();
    match section(body, "估计式") {
        None => missing.push("估计式"),
        Some(equation) if !equation.contains("$$") && !equation.contains("书中没有写出估计式") => {
            faults.add(
                address,
                "EM3",
                "no_equation",
                "",
                "the 估计式 section holds no display equation and does not say 书中没有写出估计式",
            );
        }
        Some(_) => {}
    }
    match section(body, "各项") {
        None => missing.push("各项"),
        Some(terms) => {
            let lacks = [("因变量", "因变量"), ("回归元", "关注的回归元"), ("标准误", "标准误")]
                .into_iter()
                .filter(|(word, _)| !terms.contains(word))
                .map(|(_, label)| label)
                .collect::<Vec<_>>();
            // A variant states only its change, so it may name fewer terms.
            if !lacks.is_empty() && section(body, "相对基准的改动").is_none() {
                faults.add(
                    address,
                    "EM3",
                    "missing_terms",
                    &format!("terms:{}", lacks.join(",")),
                    &format!("the 各项 section does not name: {}", lacks.join("、")),
                );
            }
        }
    }
    if !missing.is_empty() {
        faults.add(
            address,
            "EM3",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!("a specification part lacks the labels: {}", missing.join("、")),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Check, Limits};
    use serde_json::{Value, json};

    fn part(facet: &str, body: &str) -> Value {
        json!({ "address": "K/1|a|a", "type": "model", "category": "part", "facet": facet,
                "level": "empirical", "title": format!("{facet}：标题"), "body": body })
    }

    fn em(lines: Vec<String>) -> Vec<String> {
        lines.into_iter().filter(|line| line.contains(" EM")).collect()
    }

    fn run(entry: Value) -> Vec<String> {
        em(Check::new(Limits::default(), false, "").run(&[entry]))
    }

    const GOOD_ESTIMATE: &str = "税率高 10%，收入消费比高约 7%。\n\n**估计**\n- 估计量：[[K/1|a|a|1]] 中 $\\beta$（式 (1)），表 4.2 A 列。\n- 值：0.70。\n- 标准误：0.21（White 稳健）。\n- 样本量：N = 69。\n\n**量级**\n税率升 10%，收入升约 7%。\n\n**为什么**\n税基随税率变化不大。\n\n**依据**\n依赖 [[K/1|a|a|1]]。\n\n来源：[[K]] §4.2";

    #[test]
    fn roles_are_read_from_the_facet_prefix() {
        assert_eq!(role_of("数据"), Some(EmpiricalRole::Data));
        assert_eq!(role_of("数据：99 个国家"), Some(EmpiricalRole::Data));
        assert_eq!(role_of("规格：B 列"), Some(EmpiricalRole::Spec));
        assert_eq!(role_of("假设 1"), Some(EmpiricalRole::Assumption));
        assert_eq!(role_of("假设 2′"), Some(EmpiricalRole::Assumption));
        assert_eq!(role_of("第 3 章：稳健性"), Some(EmpiricalRole::Chapter));
        assert_eq!(role_of("表 4.2"), Some(EmpiricalRole::Table));
        assert_eq!(role_of("表"), None);
        assert_eq!(role_of("图 3：事件研究"), Some(EmpiricalRole::Figure));
        assert_eq!(role_of("推导：估计式"), Some(EmpiricalRole::Derivation));
        assert_eq!(role_of("外部有效性"), Some(EmpiricalRole::Limits));
        assert_eq!(role_of("设定"), None);
        assert_eq!(role_of("假设前提"), None);
    }

    #[test]
    fn symbols_skip_indices_tags_and_functions() {
        assert_eq!(
            symbols(r"y_{ct}=\beta D_{ct}+\sum_j\gamma_j y_{c,t-j}+\alpha_c+\delta_t+\varepsilon_{ct} \tag{1}"),
            ["y", r"\beta", "D", r"\gamma", r"\alpha", r"\delta", r"\varepsilon"]
        );
        assert_eq!(
            symbols(r"\log Q_{st}=\beta^T\log(1+A_{d(s)t}) + \text{Post}_t"),
            ["Q", r"\beta", "A", "Post"]
        );
    }

    /// OCR writes `S o c i a l _ {f t}`: TeX ignores the spaces and shows one name, so three or
    /// more single letters separated by spaces read as one symbol; two stay apart (`p q`).
    #[test]
    fn letters_an_ocr_spaced_out_read_as_one_name() {
        assert_eq!(
            symbols(r"S o c i a l _ {f t} = \alpha + \beta X _ {f t} + \psi_ {o} + \varepsilon_ {j t} \tag {7}"),
            ["Social", r"\alpha", r"\beta", "X", r"\psi", r"\varepsilon"]
        );
        assert_eq!(symbols("p q + r"), ["p", "q", "r"]);
    }

    #[test]
    fn a_variable_table_names_its_first_column() {
        let names = table_names("| 变量 | 含义 |\n| --- | --- |\n| $SR$ | 税率 |\n| $\\beta$ | 系数 |\n| OPEN | 开放度 |");
        assert!(names.contains("SR") && names.contains(r"\beta") && names.contains("OPEN"));
    }

    #[test]
    fn only_an_estimating_equation_or_estimator_may_be_derived() {
        assert!(allowed_derivation("推导：估计式"));
        assert!(allowed_derivation("推导：估计量的一致性"));
        assert!(!allowed_derivation("推导：系数换算"));
    }

    #[test]
    fn the_design_is_read_from_the_model_cards_design_section() {
        let body = "**问题**\n税率与收入。\n\n**设计**\ncross-section：99 个国家的横截面比较。\n\n**数据**\n…";
        assert_eq!(design_of(body), ["cross-section"]);
        assert_eq!(design_of("**设计**\ndid + iv：分阶段推行。"), ["did", "iv"]);
        assert!(design_of("没有设计一节。").is_empty());
    }

    /// EM1 and EM2: a well-formed estimate passes; each missing piece is named.
    #[test]
    fn an_estimate_states_its_labels_link_cell_value_and_uncertainty() {
        assert_eq!(run(part("估计：税率弹性", GOOD_ESTIMATE)), Vec::<String>::new());
        let lines = run(part("估计：税率弹性", "税率高，收入高。\n\n**估计**\n- 值：很大。\n\n来源：[[K]] §4.2"));
        let joined = lines.join("\n");
        assert!(joined.contains("EM1 an estimate part lacks the labels: 量级、依据"), "{joined}");
        assert!(joined.contains("EM1 the 估计 section links no specification"), "{joined}");
        assert!(joined.contains("EM1 the 估计 section names no table"), "{joined}");
        assert!(joined.contains("EM2 the 估计 section states no value"), "{joined}");
        assert!(joined.contains("EM2 the 估计 section states no uncertainty"), "{joined}");
        // 原书未写明 answers both the value and the uncertainty rule.
        let unknown = GOOD_ESTIMATE.replace("- 值：0.70。\n- 标准误：0.21（White 稳健）。\n- 样本量：N = 69。", "- 值：原书未写明。");
        assert_eq!(run(part("估计：税率弹性", &unknown)), Vec::<String>::new());
    }

    /// EM3: a baseline specification names the outcome, the regressor and the standard errors.
    #[test]
    fn a_specification_has_its_equation_and_terms() {
        let good = "本规格估计税率弹性。\n\n**估计式**\n$$ \\ln R = \\alpha + \\beta \\ln SR + \\varepsilon \\tag{1} $$\n\n**各项**\n1. 因变量：收入消费比。\n2. 关注的回归元：标准税率。\n3. 标准误：White 稳健。\n\n来源：[[K]] §4.2";
        assert_eq!(run(part("规格：基准", good)), Vec::<String>::new());
        let no_equation = good.replace("$$ \\ln R = \\alpha + \\beta \\ln SR + \\varepsilon \\tag{1} $$", "收入对税率回归。");
        assert!(run(part("规格：基准", &no_equation)).join("\n").contains("EM3 the 估计式 section holds no display equation"));
        let said = good.replace("$$ \\ln R = \\alpha + \\beta \\ln SR + \\varepsilon \\tag{1} $$", "书中没有写出估计式。");
        assert_eq!(run(part("规格：基准", &said)), Vec::<String>::new());
        let thin = good.replace("3. 标准误：White 稳健。\n", "");
        assert!(run(part("规格：基准", &thin)).join("\n").contains("EM3 the 各项 section does not name: 标准误"));
        // A variant states its change and may leave the unchanged terms out.
        let variant = format!("{thin}\n\n**相对基准的改动**\n删去文盲率。");
        assert_eq!(run(part("规格：B 列", &variant)), Vec::<String>::new());
        assert!(run(part("规格：基准", "只有一句。")).join("\n").contains("EM3 a specification part lacks the labels: 估计式、各项"));
    }

    /// EM4: a design part names the source of variation and the identifying assumptions.
    #[test]
    fn a_design_names_its_variation_and_assumptions() {
        assert!(run(part("设计", "横截面比较。\n\n**比较**\n国家之间。")).join("\n").contains("EM4 a design part lacks the labels: 变异来源、识别假设"));
        assert_eq!(run(part("设计", "横截面比较。\n\n**变异来源**\n税率。\n\n**识别假设**\n- [[K/1]] 一条。")), Vec::<String>::new());
    }

    /// EM5 is advice and needs the model's design; a formal model's parts take no EM rule.
    #[test]
    fn a_causal_verb_in_a_correlational_estimate_is_advice() {
        let causal = GOOD_ESTIMATE.replace("税率高 10%，收入消费比高约 7%。", "税率提高导致收入上升。");
        assert!(run(part("估计：税率弹性", &causal)).is_empty(), "no design, no EM5");
        let mut entry = part("估计：税率弹性", &causal);
        entry["design"] = json!("cross-section");
        let lines = run(entry);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("EM5"), "{lines:?}");
        let mut formal = part("估计：税率弹性", "没有标签。");
        formal["level"] = json!("formal");
        assert!(run(formal).is_empty());
    }
}
