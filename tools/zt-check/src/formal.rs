//! Formal economic models (level 2, docs/formal-models-proposal.md §4–§5):
//! the 6 nonexclusive claim objects, the 15 canonical roles parsed from facet prefixes,
//! notation/symbol extraction shared with `src/model/formal.rs` and `zt-digest`, and the
//! card-format rules FM1–FM6 for every Level 2 formal model (excluding mathematical units
//! carrying `form`).

use crate::core::Faults;
use crate::text::{LinkForm, link_occurrences};
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;

pub use crate::empirical::{display_formulas, section, symbols, table_names};

/// The six nonexclusive claim/solution objects (proposal §4.1).
pub const CLAIM_OBJECTS: [&str; 6] = [
    "choice-optimum",
    "strategic-allocation",
    "market-equilibrium",
    "dynamics-limit",
    "representation-comparison",
    "identification",
];

/// Valid `source_kind` values on a `Result` plan entry (proposal §5.3).
pub const SOURCE_KINDS: [&str; 5] = [
    "proposition",
    "theorem",
    "lemma",
    "corollary",
    "unnumbered",
];

/// Valid `logical_force` values on a `Result` plan entry (proposal §5.3).
pub const LOGICAL_FORCES: [&str; 12] = [
    "necessary",
    "sufficient",
    "iff",
    "existence",
    "uniqueness",
    "existence-uniqueness",
    "characterization",
    "comparative-statics",
    "equivalence",
    "identification",
    "conjecture",
    "numerical-finding",
];

/// Valid `justifications[].force` values (proposal §5.3).
pub const JUSTIFICATION_FORCES: [&str; 8] = [
    "proof",
    "constructive",
    "diagrammatic",
    "heuristic",
    "illustration",
    "numerical",
    "conjectural",
    "omitted",
];

/// Valid `justifications[].location` values (proposal §5.3).
pub const JUSTIFICATION_LOCATIONS: [&str; 3] = ["local", "external", "supplied"];

/// Valid per-row provenance kinds on a `校准：` card (proposal §5.4).
pub const PROVENANCE_KINDS: [&str; 3] = [
    "assigned",
    "external-estimate",
    "moment-matched",
];

/// The 15 canonical roles of a Level 2 formal economic model part (proposal §5.1).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum FormalRole {
    Setting,
    Notation,
    Assumption,
    Definition,
    Criterion,
    Chapter,
    Benchmark,
    Result,
    Derivation,
    Argument,
    Boundary,
    Calibration,
    Experiment,
    Example,
    Limits,
}

impl FormalRole {
    /// The role's canonical machine id, as build reports and diagnostics name it.
    pub fn id(self) -> &'static str {
        match self {
            FormalRole::Setting => "setting",
            FormalRole::Notation => "notation",
            FormalRole::Assumption => "assumption",
            FormalRole::Definition => "definition",
            FormalRole::Criterion => "criterion",
            FormalRole::Chapter => "chapter",
            FormalRole::Benchmark => "benchmark",
            FormalRole::Result => "result",
            FormalRole::Derivation => "derivation",
            FormalRole::Argument => "argument",
            FormalRole::Boundary => "boundary",
            FormalRole::Calibration => "calibration",
            FormalRole::Experiment => "experiment",
            FormalRole::Example => "example",
            FormalRole::Limits => "limits",
        }
    }

    /// Whether this role belongs to a proof route under a result or benchmark.
    pub fn is_proof_step(self) -> bool {
        matches!(self, FormalRole::Derivation | FormalRole::Argument)
    }
}

/// The text of a facet before its first `：` (or `:`), trimmed.
fn prefix(facet: &str) -> &str {
    facet.find(['：', ':']).map_or(facet, |at| &facet[..at]).trim()
}

fn is_label_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || ".-′'一二三四五六七八九十".contains(c)
}

/// Whether `rest` (the text after `假设` in the compacted prefix) is a valid assumption label:
/// empty (bare `假设`), alphanumeric/prime label (`1`, `A2`, `1′`, `1.2`), or a single balanced
/// ASCII/full-width parenthesized label (`(B)`, `(A1)`, `（A1）`, `(1′)`).
/// Rejects non-assumption compounds like `假设前提` and malformed parens like `假设 ()` or `假设 (B`.
fn is_valid_assumption_suffix(rest: &str) -> bool {
    if rest.is_empty() {
        return true;
    }
    if let Some(inner) = rest.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        return !inner.is_empty() && inner.chars().all(is_label_char);
    }
    if let Some(inner) = rest.strip_prefix('（').and_then(|s| s.strip_suffix('）')) {
        return !inner.is_empty() && inner.chars().all(is_label_char);
    }
    rest.chars().all(is_label_char)
}

/// Whether `compact` is a chapter label such as `第1章`, `第12章`, `第一章`.
fn is_chapter_prefix(compact: &str) -> bool {
    compact
        .strip_prefix('第')
        .and_then(|rest| rest.strip_suffix('章'))
        .is_some_and(|inner| !inner.is_empty() && inner.chars().all(is_label_char))
}

/// Parse one of the 15 canonical `FormalRole`s from a facet prefix (proposal §5.1).
///
/// - `Criterion` accepts `{判据, 均衡定义, 解概念}` and explicitly leaves `规划` out.
/// - `Assumption` accepts bare `假设`, alphanumeric/prime labels (`假设 1`, `假设 A2`, `假设 1′`),
///   and a single balanced ASCII or full-width parenthesized source label (`假设 (B)`, `假设（A1）`).
/// - Non-canonical facet prefixes return `None`.
pub fn role_of(facet: &str) -> Option<FormalRole> {
    let compact = prefix(facet).split_whitespace().collect::<String>();
    Some(match compact.as_str() {
        "设定" => FormalRole::Setting,
        "符号表与读法" => FormalRole::Notation,
        "定义" => FormalRole::Definition,
        "判据" | "均衡定义" | "解概念" => FormalRole::Criterion,
        "基准" => FormalRole::Benchmark,
        "结果" => FormalRole::Result,
        "论证" => FormalRole::Argument,
        "边界" => FormalRole::Boundary,
        "校准" => FormalRole::Calibration,
        "数值实验" => FormalRole::Experiment,
        "例示" => FormalRole::Example,
        "局限" => FormalRole::Limits,
        _ if compact.starts_with("推导") => FormalRole::Derivation,
        _ if compact
            .strip_prefix("假设")
            .is_some_and(is_valid_assumption_suffix) =>
        {
            FormalRole::Assumption
        }
        _ if is_chapter_prefix(&compact) => FormalRole::Chapter,
        _ => return None,
    })
}

/// The claim objects declared in a formal Model Card's `**目标对象与标签**` section.
pub fn claim_objects_of(model_body: &str) -> Vec<String> {
    let Some(sec) = section(model_body, "目标对象与标签") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in sec.lines() {
        let trimmed = line.trim().trim_start_matches(['-', '*', '•']).trim();
        let rest = if let Some(r) = trimmed.strip_prefix("对象") {
            r.trim_start_matches(['：', ':']).trim()
        } else if trimmed.starts_with("标签") {
            continue;
        } else {
            trimmed
        };
        let lower = rest.to_lowercase();
        for token in lower.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
            if CLAIM_OBJECTS.contains(&token) && !out.iter().any(|existing| existing == token) {
                out.push(token.to_string());
            }
        }
    }
    out
}

/// Optional descriptive `family_tags` declared in `**目标对象与标签**`.
pub fn family_tags_of(model_body: &str) -> Vec<String> {
    let Some(sec) = section(model_body, "目标对象与标签") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in sec.lines() {
        let trimmed = line.trim().trim_start_matches(['-', '*', '•']).trim();
        let Some(rest) = trimmed.strip_prefix("标签") else {
            continue;
        };
        let rest = rest.trim_start_matches(['：', ':']).trim();
        for raw in rest.split(['、', '；', ';', ',', '，']) {
            let tag = raw.trim().trim_end_matches(['。', '.']).trim();
            if !tag.is_empty() && !out.iter().any(|existing| existing == tag) {
                out.push(tag.to_string());
            }
        }
    }
    out
}

/// Targets bound by `沿用 [[…]] 的全部符号` (or `沿用 [[…]] … 符号`) in a `符号表与读法` Card.
/// Parses only links attached to the `沿用` clause (ignoring earlier `参见 [[...]]` clauses on the same line).
pub fn borrowed_notation_targets(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        for clause in line.split(['。', '；', ';']) {
            let Some(pos) = clause.find("沿用") else {
                continue;
            };
            let tail = &clause[pos..];
            if !tail.contains("符号") {
                continue;
            }
            for occ in link_occurrences(tail) {
                if occ.form == LinkForm::Ordinary {
                    let target = occ.target.to_string();
                    if !out.contains(&target) {
                        out.push(target);
                    }
                }
            }
        }
    }
    out
}

/// The symbols defined in a `符号表与读法` (or legacy `符号表`) Card, ignoring table headers and
/// any `统一用语` / `术语表` table at the bottom of the Card so headers and prose terms are not
/// confused with math symbols.
pub fn notation_symbols(body: &str) -> BTreeSet<String> {
    let mut filtered = Vec::new();
    let mut in_terms = false;
    for line in body.lines() {
        let t = line.trim();
        if t.contains("统一用语") || t.contains("术语表") {
            in_terms = true;
            continue;
        }
        if in_terms && t.starts_with("**") && t.ends_with("**") {
            in_terms = false;
        }
        if !in_terms {
            if t.starts_with('|') {
                let first = t.trim_start_matches('|').split('|').next().unwrap_or("").trim();
                if matches!(
                    first,
                    "符号" | "记号" | "变量" | "参数" | "术语" | "Symbol" | "Variable" | "Parameter" | "Term"
                ) {
                    continue;
                }
            }
            filtered.push(line);
        }
    }
    table_names(&filtered.join("\n"))
}

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[[^\]]+\]\]").expect("a link"));
static SOURCE_ID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:Proposition|Theorem|Lemma|Corollary|Claim|Remark|Appendix|Section)\s*[A-Za-z0-9.()]+|(?:命题|定理|引理|推论|附录|原文)\s*[A-Za-z0-9一二三四五六七八九十.()（）]+|未编号|§\s*[A-Za-z0-9.]+|式\s*\([A-Za-z0-9.]+\)")
        .expect("a source identity cue")
});
static LOGICAL_FORCE_CUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:necessary|sufficient|iff|characterization|identification|existence|uniqueness|comparative[- ]?statics|equivalence|bound|counterexample|conjecture|mechanism|numerical-finding)\b|必要条件|充分条件|充要条件|刻画|识别|存在性|唯一性|比较静态|等价|上下界|上界|下界|反例|猜想|机制|数值发现|数值结论|必要|充分|充要")
        .expect("a logical force cue")
});
static JUSTIFICATION_CUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"局部证明|外部证明|外部引证|引证|启发式|补写|原文未证|构造|图示|数值实验|局部例示|例示|论证|推导|依赖\s*\[\[")
        .expect("a justification cue")
});
static SCOPE_CUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)条件|作用域|情形|范围|假设|全模型|稳态|局部|一般|均衡下|依赖\s*\[\[|\bscope\b|\bregime\b|\bunder\b|\bif\b|\bwhen\b")
        .expect("a scope cue")
});
static EXTERNAL_LOCATOR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\[[^\]]+\]|附录|补充材料|在线附录|supplement|appendix|\[\[[^\]]+\]\]|\(\d{4}\)|\d{4}")
        .expect("an external locator")
});
static PROVENANCE_CUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bassigned\b|\bexternal-estimate\b|\bmoment-matched\b|赋值|设定值|归一化|外部估计|矩匹配")
        .expect("a provenance cue")
});
static NUMERICAL_CUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)数值|校准|参数化|模拟|反事实|脉冲响应|numerical|calibrated|simulation")
        .expect("a numerical status cue")
});
static CLAIM_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(命题|定理|引理|推论|Proposition|Theorem|Lemma|Corollary)\s*[A-Za-z]?\d+(?:[.\-]\d+)*[A-Za-z′']*")
        .expect("a claim label")
});
static APPENDIX_PROOF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"附录\s*[A-Za-z0-9]+").expect("an appendix tag"));
static BRACKET_CITE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[[^\]]{1,24}\]").expect("a bracket citation"));

/// Whether a `**依据**` section declares at least one justification disposition.
pub fn has_justification(just: &str) -> bool {
    JUSTIFICATION_CUE.is_match(just)
}

/// Extract a printed claim label (`命题 1`, `Theorem 2`, `引理 1`, ...) from text, if any.
pub fn find_claim_label(text: &str) -> Option<String> {
    CLAIM_LABEL.find(text).map(|m| m.as_str().trim().to_string())
}

/// Extract an appendix proof reference (`附录 B`, ...) from text, if any.
pub fn find_appendix_proof(text: &str) -> Option<String> {
    APPENDIX_PROOF.find(text).map(|m| m.as_str().replace(' ', ""))
}

/// Extract a bracketed external citation (`[4]`, ...) from text, if any.
pub fn find_bracket_cite(text: &str) -> Option<String> {
    BRACKET_CITE.find(text).map(|m| m.as_str().to_string())
}

/// Whether a single justification clause (split on `；`/`;` inside a line of `**依据**`)
/// declares a non-inline local proof/argument/heuristic route that must link a direct Side
/// proof-step Card (`[[...]]`).
pub fn is_local_route_clause(clause: &str) -> bool {
    let t = clause.trim();
    if t.is_empty()
        || t.contains("外部")
        || t.contains("（行内）")
        || t.contains("(行内)")
        || t.contains("inline:")
    {
        return false;
    }
    t.contains("局部证明")
        || t.contains("局部论证")
        || t.contains("局部构造")
        || t.contains("局部图示")
        || t.contains("局部例示")
        || t.contains("局部数值")
        || t.contains("启发式推导")
}

/// Whether a line inside `**依据**` contains at least one non-inline local route clause.
pub fn is_local_route_line(line: &str) -> bool {
    line.split(['；', ';']).any(is_local_route_clause)
}

/// Card-format rules FM1–FM6 for Level 2 formal economic models (proposal §5.5 Layer 2).
/// Callers invoke this on every Level 2 formal model (never on concept models, empirical models,
/// or mathematical units carrying `form`).
pub(crate) fn rules(
    facet: &str,
    address: &str,
    body: &str,
    is_head: bool,
    has_parts: bool,
    faults: &mut Faults,
) {
    if is_head {
        head_rules(address, body, has_parts, faults);
        return;
    }
    // Check malformed assumption facet before role dispatch (e.g. `假设 ()`, `假设 (B`).
    let pref = prefix(facet).split_whitespace().collect::<String>();
    if let Some(rest) = pref.strip_prefix("假设")
        && !is_valid_assumption_suffix(rest)
    {
        faults.add(
            address,
            "FM2",
            "malformed_assumption_label",
            &format!("facet:{facet}"),
            &format!(
                "malformed assumption label in facet `{facet}`: use bare 假设, alphanumeric label (假设 1, 假设 A2, 假设 1′), or one balanced parenthesized label (假设 (B), 假设（A1）)"
            ),
        );
        return;
    }
    match role_of(facet) {
        Some(FormalRole::Assumption) => assumption_rules(facet, address, body, faults),
        Some(FormalRole::Definition) => definition_rules(address, body, faults),
        Some(FormalRole::Criterion) => criterion_rules(address, body, faults),
        Some(FormalRole::Result) => result_rules(address, body, faults),
        Some(FormalRole::Argument) => argument_rules(address, body, faults),
        Some(FormalRole::Notation) => notation_rules(address, body, faults),
        Some(FormalRole::Calibration) => calibration_rules(address, body, faults),
        Some(FormalRole::Experiment) => experiment_rules(address, body, faults),
        _ => {}
    }
}

/// Whether a formal Model Card has entered the structured build state (it owns parts, or its
/// body already carries any structured head section), as opposed to an unbuilt prose Model Card
/// created during the Digest stage before the Models stage compiles its parts.
fn is_built_head(body: &str, has_parts: bool) -> bool {
    has_parts
        || ["结构", "目标对象与标签", "原语与判据", "主要结果"]
            .into_iter()
            .any(|label| section(body, label).is_some())
}

fn has_problem_statement(body: &str) -> bool {
    section(body, "问题").is_some()
        || body.lines().any(|line| {
            let t = line.trim_start();
            t.starts_with("问题：") || t.starts_with("问题:")
        })
}

/// FM1: Level 2 formal Model Card format.
fn head_rules(address: &str, body: &str, has_parts: bool, faults: &mut Faults) {
    if !is_built_head(body, has_parts) {
        return;
    }
    let mut missing = Vec::new();
    if !has_problem_statement(body) {
        missing.push("问题");
    }
    for label in ["目标对象与标签", "原语与判据", "主要结果", "结构"] {
        if section(body, label).is_none() {
            missing.push(label);
        }
    }
    if !missing.is_empty() {
        faults.add(
            address,
            "FM1",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!(
                "a formal Model Card lacks the sections: {}",
                missing.join("、")
            ),
        );
    }
    if section(body, "目标对象与标签").is_some() && claim_objects_of(body).is_empty() {
        faults.add(
            address,
            "FM1",
            "no_claim_object",
            "",
            &format!(
                "the 目标对象与标签 section declares no valid claim object: name one or more of {}",
                CLAIM_OBJECTS.join(", ")
            ),
        );
    }
}

/// FM2 for `假设`.
fn assumption_rules(facet: &str, address: &str, body: &str, faults: &mut Faults) {
    let missing = ["起点", "假设"]
        .into_iter()
        .filter(|label| section(body, label).is_none())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        faults.add(
            address,
            "FM2",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!(
                "an assumption part lacks the sections: {}",
                missing.join("、")
            ),
        );
    }
    let is_variant = facet.contains("变体") || facet.contains("体制");
    if is_variant && section(body, "作用域").is_none() && !body.contains("作用域") {
        faults.add(
            address,
            "FM2",
            "missing_scope",
            "variant_assumption",
            "a variant/regime assumption lacks an explicit 作用域 stating which variant or regime it restricts",
        );
    }
}

/// FM2 for `定义`.
fn definition_rules(address: &str, body: &str, faults: &mut Faults) {
    let missing = ["起点", "定义"]
        .into_iter()
        .filter(|label| section(body, label).is_none())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        faults.add(
            address,
            "FM2",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!(
                "a definition part lacks the sections: {}",
                missing.join("、")
            ),
        );
    }
}

/// FM2 for `判据` / `均衡定义` / `解概念`.
fn criterion_rules(address: &str, body: &str, faults: &mut Faults) {
    let mut missing = Vec::new();
    if section(body, "起点").is_none() {
        missing.push("起点");
    }
    let has_content = ["判据", "均衡定义", "解概念"]
        .into_iter()
        .any(|label| section(body, label).is_some());
    if !has_content {
        missing.push("判据/均衡定义/解概念");
    }
    if section(body, "作用域").is_none() {
        missing.push("作用域");
    }
    if !missing.is_empty() {
        faults.add(
            address,
            "FM2",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!(
                "a criterion part lacks the sections: {}",
                missing.join("、")
            ),
        );
    }
}

/// Split a Markdown table row into trimmed cells without splitting on `|` inside `[[...]]` Links
/// or escaped Markdown `\|`.
fn split_table_cells(line: &str) -> Vec<&str> {
    let inner = line.trim().trim_matches('|');
    let bytes = inner.as_bytes();
    let mut cells = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut in_link = false;
    while i < bytes.len() {
        if !in_link && bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'|' {
            i += 2;
            continue;
        }
        if !in_link && i + 1 < bytes.len() && bytes[i] == b'[' && bytes[i + 1] == b'[' {
            in_link = true;
            i += 2;
            continue;
        }
        if in_link && i + 1 < bytes.len() && bytes[i] == b']' && bytes[i + 1] == b']' {
            in_link = false;
            i += 2;
            continue;
        }
        if !in_link && bytes[i] == b'|' {
            cells.push(inner[start..i].trim());
            start = i + 1;
        }
        i += 1;
    }
    cells.push(inner[start..].trim());
    cells
}

/// FM3 for `结果`.
fn result_rules(address: &str, body: &str, faults: &mut Faults) {
    let missing = ["陈述", "为什么", "依据"]
        .into_iter()
        .filter(|label| section(body, label).is_none())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        faults.add(
            address,
            "FM3",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!(
                "a result part lacks the sections: {}",
                missing.join("、")
            ),
        );
    }
    if let Some(why) = section(body, "为什么")
        && why.trim().is_empty()
    {
        faults.add(
            address,
            "FM3",
            "empty_intuition",
            "",
            "the 为什么 section is empty: state in one to three sentences why the claim holds",
        );
    }
    if let Some(stmt) = section(body, "陈述") {
        if !SOURCE_ID.is_match(&stmt) {
            faults.add(
                address,
                "FM3",
                "no_source_identity",
                "",
                "the 陈述 section names no source identity or scope (e.g. 命题 1 / 未编号 / 原文 §n / 条件与作用域)",
            );
        }
        if !LOGICAL_FORCE_CUE.is_match(&stmt) {
            faults.add(
                address,
                "FM3",
                "no_logical_force",
                "",
                "the 陈述 section states no logical force (e.g. 力度：necessary / sufficient / iff / characterization / identification / conjecture)",
            );
        }
        if !SCOPE_CUE.is_match(&stmt) {
            faults.add(
                address,
                "FM3",
                "no_scope",
                "",
                "the 陈述 section states no scope or maintaining conditions (e.g. 条件与作用域：… / 依赖 [[...]])",
            );
        }
    }
    if let Some(just) = section(body, "依据") {
        if !JUSTIFICATION_CUE.is_match(&just) {
            faults.add(
                address,
                "FM3",
                "no_justification",
                "",
                "the 依据 section declares no justification disposition (局部证明 / 外部证明 / 启发式 / 补写 / 数值实验 / 原文未证)",
            );
        } else {
            let mut local_unlinked = false;
            let mut ext_no_locator = false;
            for line in just.lines() {
                for clause in line.split(['；', ';']) {
                    let t = clause.trim();
                    if t.is_empty() {
                        continue;
                    }
                    if is_local_route_clause(t) && !LINK.is_match(t) {
                        local_unlinked = true;
                    }
                    let line_external = t.contains("外部证明")
                        || t.contains("外部引用")
                        || t.contains("外部引证")
                        || t.contains("外部论证")
                        || t.contains("外部构造")
                        || t.contains("外部图示")
                        || t.contains("外部启发式")
                        || t.contains("外部数值")
                        || t.contains("外部例示")
                        || t.contains("外部猜想");
                    if line_external && !EXTERNAL_LOCATOR.is_match(t) {
                        ext_no_locator = true;
                    }
                }
            }
            if local_unlinked {
                faults.add(
                    address,
                    "FM3",
                    "local_justification_unlinked",
                    "",
                    "the 依据 section declares a local proof/heuristic entry without a proof route Card Link ([[...]])",
                );
            }
            if ext_no_locator {
                faults.add(
                    address,
                    "FM3",
                    "external_justification_no_locator",
                    "",
                    "the 依据 section declares an external justification entry without a locator (e.g. [4], 补充材料 A3.1)",
                );
            }
        }
    }
}

/// FM4 for `论证` (non-algebraic argument; `推导` keeps existing R14–R17 rules).
fn argument_rules(address: &str, body: &str, faults: &mut Faults) {
    let has_goal = section(body, "目标").is_some()
        || body.lines().any(|line| {
            let t = line.trim();
            t.starts_with("目标：") || t.starts_with("目标:")
        });
    let mut missing = Vec::new();
    if !has_goal {
        missing.push("目标");
    }
    for label in ["依据", "论证", "结论"] {
        if section(body, label).is_none() {
            missing.push(label);
        }
    }
    if !missing.is_empty() {
        faults.add(
            address,
            "FM4",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!(
                "an argument part lacks the sections: {}",
                missing.join("、")
            ),
        );
    }
}

/// FM5 for `符号表与读法`.
fn notation_rules(address: &str, body: &str, faults: &mut Faults) {
    let mut valid_symbol_rows = 0usize;
    let mut missing_first_use = 0usize;
    let mut in_terms = false;
    for line in body.lines() {
        let t = line.trim();
        if t.contains("统一用语") || t.contains("术语表") {
            in_terms = true;
            continue;
        }
        if in_terms && t.starts_with("**") && t.ends_with("**") {
            in_terms = false;
        }
        if !in_terms && t.starts_with('|') {
            let cells = split_table_cells(t);
            if cells.len() >= 2
                && !cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '))
                && !matches!(
                    cells[0],
                    "符号" | "记号" | "变量" | "参数" | "术语" | "Symbol" | "Variable" | "Parameter" | "Term"
                )
                && !cells[0].is_empty()
                && !cells[1].is_empty()
            {
                // A completed symbol row (meaning != "待写") must carry an ORDINARY Card Link [[target]]
                // (not a Term link [[label@target]]) in the 3rd column (首次出现); the creation-time
                // placeholder row (`| $x$ | 待写 | 本模型 |`) stays admissible until filled.
                let is_placeholder = cells[1] == "待写";
                let has_first = cells.get(2).is_some_and(|c| {
                    !c.is_empty()
                        && *c != "—"
                        && *c != "-"
                        && (is_placeholder
                            || link_occurrences(c)
                                .into_iter()
                                .any(|occ| occ.form == LinkForm::Ordinary))
                });
                if has_first {
                    valid_symbol_rows += 1;
                } else {
                    missing_first_use += 1;
                }
            }
        }
    }
    let has_symbols = valid_symbol_rows > 0 && !notation_symbols(body).is_empty();
    let has_borrow = !borrowed_notation_targets(body).is_empty();
    if !has_symbols && !has_borrow {
        let msg = if missing_first_use > 0 {
            "symbol table rows in 符号表与读法 must include the 3rd column (首次出现 Card Link): | 符号 | 含义 | 首次出现 |"
        } else {
            "a 符号表与读法 card must define symbol table rows (| 符号 | 含义 | 首次出现 | with non-empty symbol, meaning, and first-use link) or bind imported symbols via 沿用 [[…]] 的全部符号"
        };
        faults.add(address, "FM5", "no_symbols", "", msg);
    } else if missing_first_use > 0 {
        faults.add(
            address,
            "FM5",
            "missing_first_use",
            &format!("rows:{missing_first_use}"),
            &format!(
                "{missing_first_use} symbol row(s) in 符号表与读法 lack a 首次出现 column (| 符号 | 含义 | 首次出现 |)"
            ),
        );
    }
    let term_rows = crate::text::term_table(body);
    let has_term_rows = term_rows
        .iter()
        .any(|r| !r.term.trim().is_empty() && !matches!(r.term.trim(), "无" | "—" | "-"));
    let explicit_none = section(body, "统一用语")
        .or_else(|| section(body, "术语表"))
        .is_some_and(|s| {
            s.lines().any(|l| {
                let t = l.trim();
                !t.starts_with('|')
                    && (t.contains("沿用")
                        || t.contains("无额外统一用语")
                        || t.contains("无特殊用语约定")
                        || t.contains("无专用术语"))
            })
        });
    if !has_term_rows && !has_borrow && !explicit_none {
        faults.add(
            address,
            "FM5",
            "no_term_table",
            "",
            "a 符号表与读法 card lacks populated **统一用语** (| 术语 | 不用 |) rows or borrowed notation",
        );
    }
}

/// FM6 for `校准`.
fn calibration_rules(address: &str, body: &str, faults: &mut Faults) {
    let missing = ["起点", "参数与出处"]
        .into_iter()
        .filter(|label| section(body, label).is_none())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        faults.add(
            address,
            "FM6",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!(
                "a calibration part lacks the sections: {}",
                missing.join("、")
            ),
        );
        return;
    }
    let table = section(body, "参数与出处").unwrap_or_default();
    let mut data_rows = 0usize;
    let mut missing_prov = 0usize;
    // Detect column indices from the header row when present; default to standard 6-col layout:
    // | 参数 (0) | 含义 (1) | 取值 (2) | 单位/频率 (3) | 来源类型 (4) | 目标或出处 (5) |
    let mut value_col: Option<usize> = Some(2);
    let mut unit_col: Option<usize> = Some(3);
    let mut prov_col: Option<usize> = Some(4);
    let mut target_col: Option<usize> = Some(5);
    let mut saw_header = false;
    for line in table.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let cells = split_table_cells(t);
        if cells.is_empty()
            || cells.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '))
        {
            continue;
        }
        if !saw_header && matches!(cells[0], "参数" | "符号" | "Parameter" | "Symbol") {
            saw_header = true;
            value_col = cells
                .iter()
                .position(|c| c.contains("取值") || c.eq_ignore_ascii_case("value"));
            unit_col = cells
                .iter()
                .position(|c| c.contains("单位") || c.contains("频率") || c.eq_ignore_ascii_case("unit"));
            prov_col = cells.iter().position(|c| {
                c.contains("来源类型")
                    || c.contains("来源类别")
                    || c.eq_ignore_ascii_case("provenance")
                    || *c == "来源"
            });
            target_col = cells.iter().position(|c| {
                !c.contains("来源类型")
                    && !c.contains("来源类别")
                    && !c.eq_ignore_ascii_case("provenance")
                    && *c != "来源"
                    && (c.contains("目标")
                        || c.contains("出处")
                        || c.contains("来源")
                        || c.eq_ignore_ascii_case("target")
                        || c.eq_ignore_ascii_case("source"))
            });
            continue;
        }
        data_rows += 1;
        let non_empty = |idx: Option<usize>| -> bool {
            idx.and_then(|i| cells.get(i))
                .is_some_and(|c| !c.is_empty() && *c != "—" && *c != "-")
        };
        let value_ok = non_empty(value_col);
        let unit_ok = unit_col.is_none() || non_empty(unit_col);
        let prov_ok = prov_col
            .and_then(|i| cells.get(i))
            .is_some_and(|c| PROVENANCE_CUE.is_match(c));
        let target_ok = non_empty(target_col);
        if cells.len() < 4 || !value_ok || !unit_ok || !prov_ok || !target_ok {
            missing_prov += 1;
        }
    }
    if data_rows == 0 {
        faults.add(
            address,
            "FM6",
            "no_parameter_rows",
            "",
            "the 参数与出处 section holds no parameter table rows",
        );
    } else if missing_prov > 0 {
        faults.add(
            address,
            "FM6",
            "missing_provenance",
            &format!("rows:{missing_prov}"),
            &format!(
                "{missing_prov} calibration row(s) lack populated value, unit, target/source, or provenance (assigned / external-estimate / moment-matched) in the 来源类型 column"
            ),
        );
    }
}

/// FM6 for `数值实验`.
fn experiment_rules(address: &str, body: &str, faults: &mut Faults) {
    let missing = ["实验设定", "数值结果", "边界"]
        .into_iter()
        .filter(|label| section(body, label).is_none())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        faults.add(
            address,
            "FM6",
            "missing_labels",
            &format!("labels:{}", missing.join(",")),
            &format!(
                "a numerical experiment part lacks the sections: {}",
                missing.join("、")
            ),
        );
    }
    if let Some(setup) = section(body, "实验设定")
        && !LINK.is_match(&setup)
    {
        faults.add(
            address,
            "FM6",
            "no_baseline_link",
            "",
            "the 实验设定 section links no calibration or baseline setting card ([[...]])",
        );
    }
    if let Some(res) = section(body, "数值结果")
        && res.trim().is_empty()
    {
        faults.add(
            address,
            "FM6",
            "empty_numerical_results",
            "",
            "the 数值结果 section is empty: state the quantitative outcome and change",
        );
    }
    // Strip bold section headers (`**数值结果**`, `**实验设定**`, `**边界**`) so the heading
    // itself does not vacuously satisfy the numerical-status cue.
    let prose_only: String = body
        .lines()
        .filter(|l| {
            let t = l.trim();
            !(t.starts_with("**") && t.ends_with("**"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    if !NUMERICAL_CUE.is_match(&prose_only) {
        faults.add(
            address,
            "FM6",
            "no_numerical_status",
            "",
            "a 数值实验 card must explicitly mark its numerical/calibrated status in prose (not an analytic theorem)",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Check, Limits};
    use serde_json::{Value, json};

    fn formal_part(facet: &str, body: &str) -> Value {
        json!({
            "address": "K/1|a|a",
            "type": "model",
            "category": "part",
            "facet": facet,
            "level": "formal",
            "title": format!("{facet}：标题"),
            "body": body,
        })
    }

    fn formal_head(body: &str) -> Value {
        json!({
            "address": "K/1",
            "type": "model",
            "category": "model",
            "facet": null,
            "level": "formal",
            "title": "形式模型标题",
            "body": body,
        })
    }

    fn fm(lines: Vec<String>) -> Vec<String> {
        lines.into_iter().filter(|line| line.contains(" FM")).collect()
    }

    fn run(entry: Value) -> Vec<String> {
        fm(Check::new(Limits::default(), false, "").run(&[entry]))
    }

    #[test]
    fn all_fifteen_canonical_roles_parse_and_reject_non_canonical_aliases() {
        assert_eq!(role_of("设定"), Some(FormalRole::Setting));
        assert_eq!(role_of("设定：H 国部门"), Some(FormalRole::Setting));
        assert_eq!(role_of("符号表与读法"), Some(FormalRole::Notation));
        assert_eq!(role_of("符号表与读法：本模型"), Some(FormalRole::Notation));
        assert_eq!(role_of("假设"), Some(FormalRole::Assumption));
        assert_eq!(role_of("假设 1"), Some(FormalRole::Assumption));
        assert_eq!(role_of("假设 A2：单调性"), Some(FormalRole::Assumption));
        assert_eq!(role_of("假设 1′"), Some(FormalRole::Assumption));
        assert_eq!(role_of("假设 (B)：单交叉与有界"), Some(FormalRole::Assumption));
        assert_eq!(role_of("假设（A1）：正则性"), Some(FormalRole::Assumption));
        assert_eq!(role_of("假设：(B) 单交叉"), Some(FormalRole::Assumption));
        assert_eq!(role_of("定义：前沿因子"), Some(FormalRole::Definition));
        assert_eq!(role_of("判据：最优所得税配置"), Some(FormalRole::Criterion));
        assert_eq!(role_of("均衡定义：两国竞争均衡"), Some(FormalRole::Criterion));
        assert_eq!(role_of("解概念：子博弈精炼均衡"), Some(FormalRole::Criterion));
        assert_eq!(role_of("第 1 章：激励相容"), Some(FormalRole::Chapter));
        assert_eq!(role_of("基准：无信号混同工资"), Some(FormalRole::Benchmark));
        assert_eq!(role_of("结果：命题 1 区制依赖"), Some(FormalRole::Result));
        assert_eq!(role_of("推导：便利收益楔子"), Some(FormalRole::Derivation));
        assert_eq!(role_of("论证：阈值工资自我选择"), Some(FormalRole::Argument));
        assert_eq!(role_of("边界：个体稳定不排除联盟"), Some(FormalRole::Boundary));
        assert_eq!(role_of("校准：稳态识别"), Some(FormalRole::Calibration));
        assert_eq!(role_of("数值实验：基准外逃脉冲响应"), Some(FormalRole::Experiment));
        assert_eq!(role_of("例示：两期数值代入"), Some(FormalRole::Example));
        assert_eq!(role_of("局限：忽略违约风险"), Some(FormalRole::Limits));

        // Non-canonical and legacy aliases are unconditionally rejected
        assert_eq!(role_of("规划：最优税"), None);
        assert_eq!(role_of("假设前提"), None);
        assert_eq!(role_of("假设 ()"), None);
        assert_eq!(role_of("假设 (B"), None);
        assert_eq!(role_of("假设 (B)(C)"), None);
        assert_eq!(role_of("命题 1"), None);
        assert_eq!(role_of("结论：x"), None);
        assert_eq!(role_of("数值例子"), None);
        assert_eq!(role_of("均衡条件"), None);
        assert_eq!(role_of("符号表"), None);
    }

    #[test]
    fn fm1_enforces_formal_head_and_skips_math_units() {
        let good = "**问题**\n教育信号如何分离生产率。\n\n**目标对象与标签**\n- 对象：market-equilibrium；choice-optimum\n- 标签：signaling\n\n**原语与判据**\n依赖 [[K/1|a|c]]。\n\n**主要结果**\n| 结果 | 逻辑力度 | 作用域 / 条件 | 依据状态 |\n| --- | --- | --- | --- |\n| [[K/1|a|1|a]] | sufficient | 表 I | 构造论证 |\n\n**结构**\n- 设定 [[K/1|a]]：基础设定\n\n来源：[[K]] §2";
        assert_eq!(claim_objects_of(good), ["market-equilibrium", "choice-optimum"]);
        assert_eq!(family_tags_of(good), ["signaling"]);
        assert_eq!(run(formal_head(good)), Vec::<String>::new());

        let bad = "**问题**\n只有问题。\n\n**目标对象与标签**\n- 对象：unknown-bucket\n";
        let lines = run(formal_head(bad)).join("\n");
        assert!(lines.contains("FM1"), "{lines}");
        assert!(lines.contains("原语与判据"), "{lines}");
        assert!(lines.contains("no valid claim object"), "{lines}");

        // An L2 formal head without required sections is flagged by FM1 (no legacy bypass).
        let bare_formal = formal_head("问题：只有一段话。\n\n**结构**\n- 设定 [[K/1|a]]：设定\n\n来源：[[K]] §1");
        assert!(run(bare_formal).join("\n").contains("FM1"));

        // Mathematical unit (`form: "theorem"`) is untouched by FM1.
        let math_unit = json!({
            "address": "K/1",
            "type": "model",
            "category": "model",
            "facet": null,
            "level": "formal",
            "form": "theorem",
            "title": "定理 1：中值定理",
            "body": "**陈述**\n若函数连续可微，则存在中值点。\n\n**思路**\n构造辅助函数。",
        });
        assert_eq!(run(math_unit), Vec::<String>::new());
    }

    #[test]
    fn fm2_through_fm6_enforce_formal_part_formats_and_inline_local_justifications() {
        // FM2: Criterion
        let good_crit = "定义竞争均衡。\n\n**起点**\n- 依赖 [[K/1|a]]。\n\n**均衡定义**\n1. 价格与配置。\n\n**作用域**\n基准两国情形。\n\n来源：[[K]] §3";
        assert_eq!(run(formal_part("均衡定义：两国竞争均衡", good_crit)), Vec::<String>::new());
        assert!(run(formal_part("均衡定义：两国", "没有各节。")).join("\n").contains("FM2"));

        // FM2: malformed assumption label
        assert!(run(formal_part("假设 (B", "**起点**\n无\n\n**假设**\n单交叉。")).join("\n").contains("FM2"));
        assert_eq!(
            run(formal_part("假设 (B)：单交叉", "**起点**\n- 依赖 [[K/1|a]]。\n\n**假设**\n满足单交叉与有界。\n\n来源：[[K]] §4")),
            Vec::<String>::new()
        );

        // FM3: Result (Mirrlees Thm 1 with dual external + local heuristic justifications)
        let good_res = "激励相容当且仅当收入非降且有界。\n\n**陈述**\n- 出处与力度：定理 1（原文 §4）；力度：iff。\n- 条件与作用域：依赖 [[K/1|a|b]]。\n\n**为什么**\n单交叉保证高能力者选更高收入。\n\n**依据**\n- 外部证明：严格证明见原文引证 [4]。\n- 启发式推导：见 [[K/1|a|1|a|a]]（额外假定二阶可微）。\n\n来源：[[K]] §4";
        assert_eq!(run(formal_part("结果：定理 1 激励相容", good_res)), Vec::<String>::new());
        // Inline local justification (no separate Side proof card) passes FM3 cleanly.
        let inline_res = "存在分离均衡。\n\n**陈述**\n- 出处与力度：未编号（原文 §5）；力度：sufficient。\n- 条件与作用域：依赖 [[K/1|a|b]]。\n\n**为什么**\n单交叉保证高能力者选择更高教育水平。\n\n**依据**\n- 局部构造（行内）：见 §5 构造。\n\n来源：[[K]] §5";
        assert_eq!(run(formal_part("结果：分离均衡", inline_res)), Vec::<String>::new());

        let bad_res = "一个结论。\n\n**陈述**\n收入随能力增。\n\n**依据**\n局部证明省略。\n";
        let res_lines = run(formal_part("结果：定理 1", bad_res)).join("\n");
        assert!(res_lines.contains("为什么"), "{res_lines}");
        assert!(res_lines.contains("source identity"), "{res_lines}");
        assert!(res_lines.contains("logical force"), "{res_lines}");
        assert!(res_lines.contains("proof route"), "{res_lines}");

        // FM4: Argument (Spence constructive example)
        let good_arg = "目标：证明阈值工资下两群体自我选择。\n\n**依据**\n- 依赖 [[K/1|a|b]]。\n\n**论证**\n第一组选 0，第二组选阈值，如图 II。\n\n**结论**\n构成一族分离均衡。\n\n来源：[[K]] §5";
        assert_eq!(run(formal_part("论证：阈值自我选择", good_arg)), Vec::<String>::new());
        assert!(run(formal_part("论证：缺少小节", "一段散文。")).join("\n").contains("FM4"));

        // FM5: Notation
        let good_not = "**怎样读卡片上的记号**\n下标表时期。\n\n| 符号 | 含义 | 首次出现 |\n| --- | --- | --- |\n| $\\nu$ | 便利收益份额 | [[K/1|a]] |\n\n**统一用语**\n| 术语 | 不用 |\n| --- | --- |\n| 便利收益 | 流动性溢价 |";
        assert_eq!(run(formal_part("符号表与读法", good_not)), Vec::<String>::new());
        let borrow_not = "沿用 [[K/2|a|a]] 的全部符号。\n\n**统一用语**\n| 术语 | 不用 |\n| --- | --- |\n| 均衡 | 平衡 |";
        assert_eq!(run(formal_part("符号表与读法", borrow_not)), Vec::<String>::new());
        assert_eq!(borrowed_notation_targets(borrow_not), ["K/2|a|a"]);
        assert!(run(formal_part("符号表与读法", "空白卡。")).join("\n").contains("FM5"));

        // FM6: Calibration & Experiment
        let good_cal = "**起点**\n- 依赖 [[K/1|a]]。\n\n**参数与出处**\n| 参数 | 含义 | 取值 | 单位/频率 | 来源类型 | 目标或出处 |\n| --- | --- | --- | --- | --- | --- |\n| $\\sigma$ | 能力离散度 | 0.39 | 无量纲 | external-estimate | Lydall 估计 |\n| $\\beta$ | 贴现因子 | 0.99 | 季度 | assigned | 标准设定 |";
        assert_eq!(run(formal_part("校准：稳态参数", good_cal)), Vec::<String>::new());
        let bad_cal = "**起点**\n- 依赖 [[K/1|a]]。\n\n**参数与出处**\n| 参数 | 取值 |\n| --- | --- |\n| $\\sigma$ | 0.39 |";
        assert!(run(formal_part("校准：缺出处", bad_cal)).join("\n").contains("FM6"));

        let good_exp = "数值实验显示固定供给下通胀上行。\n\n**实验设定**\n- 基准：依赖 [[K/1|a|2|a]]，施加一标准差外逃冲击。\n\n**数值结果**\n产出下降 1.2%。\n\n**边界**\n该结果为校准参数下的数值结论。";
        assert_eq!(run(formal_part("数值实验：外逃冲击", good_exp)), Vec::<String>::new());
        assert!(run(formal_part("数值实验：缺节", "产出下降。")).join("\n").contains("FM6"));

        // Mathematical unit part (`form: "theorem"`) gets ZERO FM faults.
        let math_part = json!({
            "address": "K/1|a",
            "type": "model",
            "category": "part",
            "facet": "结果：旧结果",
            "level": "formal",
            "form": "theorem",
            "title": "旧结果",
            "body": "数学单元分卡。",
        });
        assert_eq!(run(math_part), Vec::<String>::new());

        // Astra #11: borrowed_notation_targets ignores earlier `参见 [[A]]` clause on the same line.
        assert_eq!(
            borrowed_notation_targets("参见 [[K/1|a|a]]。沿用 [[K/2|a|a]] 的全部符号。"),
            vec!["K/2|a|a"]
        );

        // Astra #18: empty cues / header-only tables / 1-cell calibration rows / heading-only numerical cue fail.
        let hollow_res = "**陈述**\n条件与作用域，力度。\n\n**为什么**\n直觉。\n\n**依据**\n- 外部证明：未写出处。";
        let hollow_lines = run(formal_part("结果：空壳", hollow_res)).join("\n");
        assert!(hollow_lines.contains("source identity"), "{hollow_lines}");
        assert!(hollow_lines.contains("logical force"), "{hollow_lines}");
        assert!(hollow_lines.contains("locator"), "{hollow_lines}");

        let header_only_not = "| 符号 | 含义 |\n| --- | --- |\n\n**统一用语**\n";
        let not_lines = run(formal_part("符号表与读法", header_only_not)).join("\n");
        assert!(not_lines.contains("no_symbols") || not_lines.contains("symbol table rows"), "{not_lines}");
        assert!(not_lines.contains("统一用语"), "{not_lines}");

        let one_cell_cal = "**起点**\n设定。\n\n**参数与出处**\n| 参数 | 含义 | 取值 | 来源 |\n| --- | --- | --- | --- |\n| assigned |";
        assert!(run(formal_part("校准：单格行", one_cell_cal)).join("\n").contains("FM6"));

        let heading_only_exp = "产出下降。\n\n**实验设定**\n- 依赖 [[K/1|a]]。\n\n**数值结果**\n产出下降 1%。\n\n**边界**\n限定情形。";
        assert!(run(formal_part("数值实验：缺标注", heading_only_exp)).join("\n").contains("FM6"));

        // Astra P5 review regressions:
        // 1) FM3 rejects an empty **为什么** section.
        let empty_why = "定理 1。\n\n**陈述**\n- 出处与力度：定理 1；力度：iff。\n- 条件与作用域：依赖 [[K/1|a|b]]。\n\n**为什么**\n\n**依据**\n- 外部证明：见原文引证 [4]。";
        assert!(run(formal_part("结果：定理 1 空直觉", empty_why)).join("\n").contains("为什么"));

        // 2) FM5 accepts constructor's `无特殊用语约定。` and rejects 2-column symbol rows lacking 首次出现.
        let ctor_not = "**怎样读卡片上的记号**\n说明。\n\n| 符号 | 含义 | 首次出现 |\n| --- | --- | --- |\n| $\\nu$ | 份额 | [[K/1|a]] |\n\n**统一用语**\n\n无特殊用语约定。";
        assert_eq!(run(formal_part("符号表与读法", ctor_not)), Vec::<String>::new());
        let two_col_not = "| 符号 | 含义 |\n| --- | --- |\n| $\\nu$ | 份额 |\n\n**统一用语**\n\n无特殊用语约定。";
        assert!(run(formal_part("符号表与读法", two_col_not)).join("\n").contains("FM5"));

        // 3) FM6 handles `|` inside `[[addr]]` Links (287 bp -> 300 bp calibration), escaped `\|`,
        //    reordered `目标或来源` column before `来源类型`, and rejects provenance in the wrong column.
        let linked_cal = "**起点**\n- 依赖 [[K/1|a]]。\n\n**参数与出处**\n| 参数 | 含义 | 取值 | 单位/频率 | 来源类型 | 目标或出处 |\n| --- | --- | --- | --- | --- | --- |\n| $\\nu$ | 便利收益\\|基准 | 300 bp | 年化 | external-estimate | 对应实证 [[K/4|a|1|a]]（287 bp） |";
        assert_eq!(run(formal_part("校准：稳态识别", linked_cal)), Vec::<String>::new());
        let wrong_col_cal = "**起点**\n- 依赖 [[K/1|a]]。\n\n**参数与出处**\n| 参数 | 含义 | 取值 | 单位/频率 | 目标或来源 | 来源类型 |\n| --- | --- | --- | --- | --- | --- |\n| $\\nu$ | 份额 | 0.3 | 年化 | assigned 原文 §4 | 未知 |";
        assert!(run(formal_part("校准：错列", wrong_col_cal)).join("\n").contains("FM6"));

        // 4) FM5 rejects a filled symbol row with `待写` OR a Term link `[[label@addr]]` in column 3.
        let unlinked_first_not = "**怎样读卡片上的记号**\n说明。\n\n| 符号 | 含义 | 首次出现 |\n| --- | --- | --- |\n| $x$ | wealth | 待写 |\n\n**统一用语**\n\n无特殊用语约定。";
        assert!(run(formal_part("符号表与读法", unlinked_first_not)).join("\n").contains("FM5"));
        let term_link_first_not = "**怎样读卡片上的记号**\n说明。\n\n| 符号 | 含义 | 首次出现 |\n| --- | --- | --- |\n| $x$ | wealth | [[财富@K/1|a]] |\n\n**统一用语**\n\n无特殊用语约定。";
        assert!(run(formal_part("符号表与读法", term_link_first_not)).join("\n").contains("FM5"));
    }
}
