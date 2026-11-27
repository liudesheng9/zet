//! Stage-scoped model build-fault gate (`zt-check --model <head> --stage fold|settings|restructure|final`).
//!
//! Mirrors `skills/zt-model-restructure/scripts/check-model.mjs`.

use crate::mcp::Zt;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::collections::BTreeMap;

const ALL_FAULT_KEYS: [&str; 20] = [
    "closure",
    "reach_in",
    "forward",
    "unanchored",
    "parts",
    "beyond_level",
    "import_target",
    "import_cycle",
    "formal_shape",
    "setting_dependency",
    "criterion_unbound",
    "result_unbound",
    "proof_unbound",
    "experiment_unbound",
    "symbol_uncovered",
    "empirical_shape",
    "estimate_unbound",
    "spec_unbound",
    "test_unbound",
    "variable_uncovered",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    Fold,
    Settings,
    Restructure,
    Final,
}

impl Stage {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "fold" => Ok(Stage::Fold),
            "settings" => Ok(Stage::Settings),
            "restructure" => Ok(Stage::Restructure),
            "final" => Ok(Stage::Final),
            other => bail!("--stage is fold, settings, restructure or final, not `{other}`"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Fold => "fold",
            Stage::Settings => "settings",
            Stage::Restructure => "restructure",
            Stage::Final => "final",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StageFault {
    pub key: String,
    pub fix: String,
    pub owner: String,
    pub fault: Value,
}

impl StageFault {
    pub fn to_json(&self) -> Value {
        let mut obj = json!({
            "key": self.key,
            "fix": self.fix,
            "owner": self.owner,
        });
        if !self.fault.is_null() {
            obj["fault"] = self.fault.clone();
        }
        obj
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StageReport {
    pub stage: Stage,
    pub ok: bool,
    pub blocking: Vec<StageFault>,
    pub deferred: Vec<StageFault>,
    pub self_sufficient: Value,
    pub formal_structured: Value,
    pub evidence_bound: Value,
}

impl StageReport {
    pub fn to_json(&self, head: &str) -> Value {
        let mut obj = json!({
            "head": head,
            "stage": self.stage.as_str(),
            "ok": self.ok,
            "blocking": self.blocking.iter().map(StageFault::to_json).collect::<Vec<_>>(),
            "deferred": self.deferred.iter().map(StageFault::to_json).collect::<Vec<_>>(),
            "self_sufficient": self.self_sufficient,
        });
        if !self.formal_structured.is_null() {
            obj["formal_structured"] = self.formal_structured.clone();
        }
        if !self.evidence_bound.is_null() {
            obj["evidence_bound"] = self.evidence_bound.clone();
        }
        obj
    }
}

fn is_cross_model_closure(f: &Value) -> bool {
    let Some(c) = f.get("card").filter(|v| v.is_object()) else {
        return false;
    };
    c.get("type").and_then(Value::as_str) == Some("model")
        && c.get("facet").and_then(Value::as_str).is_some_and(|s| !s.is_empty())
        && c.get("models").and_then(Value::as_array).is_some_and(|a| !a.is_empty())
}

pub(crate) fn is_criterion_card(c: &Value) -> bool {
    if c.get("home_role").and_then(Value::as_str) == Some("criterion") {
        return true;
    }
    let facet = c.get("facet").and_then(Value::as_str).unwrap_or("");
    crate::formal::role_of(facet) == Some(crate::formal::FormalRole::Criterion)
}

pub(crate) fn is_notation_card(c: &Value) -> bool {
    if c.get("home_role").and_then(Value::as_str) == Some("notation") {
        return true;
    }
    let facet = c.get("facet").and_then(Value::as_str).unwrap_or("");
    crate::formal::role_of(facet) == Some(crate::formal::FormalRole::Notation)
}

fn has_pending_import_of(build: &Value, pred: impl Fn(&Value) -> bool) -> bool {
    build
        .get("closure")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|f| is_cross_model_closure(f) && f.get("card").is_some_and(&pred))
}

/// Core `src/model/empirical.rs` classifies missing estimates via exact `lacks == "估计"` (M4).
pub(crate) fn is_stage3_empirical_shape(fault: &Value) -> bool {
    fault.get("lacks").and_then(Value::as_str) == Some("估计")
}

fn is_head_missing_notation(head: &str, fault: &Value) -> bool {
    let lacks = fault.get("lacks").and_then(Value::as_str).unwrap_or("");
    if lacks != "符号表与读法" {
        return false;
    }
    match fault.get("address").and_then(Value::as_str) {
        None | Some("") => true,
        Some(addr) => addr == head,
    }
}

fn owner_stage(key: &str, fault: &Value) -> &'static str {
    if key == "closure" {
        return if is_cross_model_closure(fault) {
            "Stage 4 (import)"
        } else {
            "Stage 1 (fold)"
        };
    }
    if matches!(key, "reach_in" | "beyond_level" | "import_target" | "import_cycle") {
        return "Stage 1 (fold)";
    }
    if matches!(
        key,
        "setting_dependency" | "symbol_uncovered" | "variable_uncovered" | "spec_unbound"
    ) {
        return "Stage 2 (settings)";
    }
    if key == "empirical_shape" {
        return if is_stage3_empirical_shape(fault) {
            "Stage 3 (restructure)"
        } else {
            "Stage 2 (settings)"
        };
    }
    if key == "formal_shape" {
        let lacks = fault.get("lacks").and_then(Value::as_str).unwrap_or("");
        return if matches!(lacks, "设定" | "符号表与读法") {
            "Stage 2 (settings)"
        } else {
            "Stage 3 (restructure)"
        };
    }
    if matches!(
        key,
        "forward"
            | "unanchored"
            | "parts"
            | "criterion_unbound"
            | "result_unbound"
            | "proof_unbound"
            | "experiment_unbound"
            | "estimate_unbound"
            | "test_unbound"
    ) {
        return "Stage 3 (restructure)";
    }
    "Stage 4 (final)"
}

fn blocks_stage(
    stage: Stage,
    head: &str,
    key: &str,
    fault: &Value,
    pending_criterion: bool,
    pending_notation: bool,
) -> bool {
    match stage {
        Stage::Final => true,
        Stage::Restructure => {
            if key == "closure" && is_cross_model_closure(fault) {
                return false;
            }
            if key == "criterion_unbound" && pending_criterion {
                return false;
            }
            true
        }
        Stage::Settings => {
            if matches!(
                key,
                "reach_in"
                    | "beyond_level"
                    | "import_target"
                    | "import_cycle"
                    | "setting_dependency"
                    | "symbol_uncovered"
                    | "variable_uncovered"
                    | "spec_unbound"
            ) {
                return true;
            }
            if key == "empirical_shape" {
                return !is_stage3_empirical_shape(fault);
            }
            if key == "closure" && !is_cross_model_closure(fault) {
                return true;
            }
            if key == "formal_shape" {
                let lacks = fault.get("lacks").and_then(Value::as_str).unwrap_or("");
                if lacks == "设定" {
                    return true;
                }
                if lacks == "符号表与读法" {
                    // Only defer the head-level missing-notation fault when a pending Notation
                    // import exists; never defer a broken 沿用 lender on an existing notation card.
                    if is_head_missing_notation(head, fault) && pending_notation {
                        return false;
                    }
                    return true;
                }
            }
            false
        }
        Stage::Fold => {
            if matches!(key, "reach_in" | "beyond_level" | "import_target" | "import_cycle") {
                return true;
            }
            key == "closure" && !is_cross_model_closure(fault)
        }
    }
}

/// Evaluate one `get_model` response under `stage`.
pub fn evaluate(model: &Value, stage: Stage) -> StageReport {
    let head = model
        .get("head")
        .and_then(|h| h.get("address"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let build = model.get("build").unwrap_or(&Value::Null);
    let pending_criterion = has_pending_import_of(build, is_criterion_card);
    let pending_notation = has_pending_import_of(build, is_notation_card);

    let mut blocking = Vec::new();
    let mut deferred = Vec::new();

    for key in ALL_FAULT_KEYS {
        for f in build.get(key).and_then(Value::as_array).into_iter().flatten() {
            let owner = if (key == "criterion_unbound" && pending_criterion)
                || (key == "formal_shape" && is_head_missing_notation(head, f) && pending_notation)
            {
                "Stage 4 (import)"
            } else {
                owner_stage(key, f)
            };
            let fix = f
                .get("fix")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| f.to_string());
            let item = StageFault {
                key: key.to_string(),
                fix,
                owner: owner.to_string(),
                fault: f.clone(),
            };
            if blocks_stage(stage, head, key, f, pending_criterion, pending_notation) {
                blocking.push(item);
            } else {
                deferred.push(item);
            }
        }
    }

    let level = model.get("level").and_then(Value::as_str).unwrap_or("");
    let has_form = model
        .get("form")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());
    let eligible_formal = level == "formal" && !has_form;
    let eligible_empirical = level == "empirical";

    let self_sufficient = build.get("self_sufficient").cloned().unwrap_or(Value::Null);
    let formal_structured = build.get("formal_structured").cloned().unwrap_or(Value::Null);
    let evidence_bound = build.get("evidence_bound").cloned().unwrap_or(Value::Null);

    if stage == Stage::Final {
        if self_sufficient.as_bool() != Some(true) {
            blocking.push(StageFault {
                key: "self_sufficient".to_string(),
                fix: format!("self_sufficient is {self_sufficient} (must be true)"),
                owner: "Stage 4 (final)".to_string(),
                fault: Value::Null,
            });
        }
        if eligible_formal && formal_structured.as_bool() != Some(true) {
            blocking.push(StageFault {
                key: "formal_structured".to_string(),
                fix: format!(
                    "formal_structured is {formal_structured} (eligible L2 formal model requires true)"
                ),
                owner: "Stage 4 (final)".to_string(),
                fault: Value::Null,
            });
        }
        if eligible_empirical && evidence_bound.as_bool() != Some(true) {
            blocking.push(StageFault {
                key: "evidence_bound".to_string(),
                fix: format!("evidence_bound is {evidence_bound} (L3 empirical model requires true)"),
                owner: "Stage 4 (final)".to_string(),
                fault: Value::Null,
            });
        }
    }

    StageReport {
        stage,
        ok: blocking.is_empty(),
        blocking,
        deferred,
        self_sufficient,
        formal_structured,
        evidence_bound,
    }
}

pub fn format_text(model: &Value, head: &str, report: &StageReport) -> Vec<String> {
    let build = model.get("build").unwrap_or(&Value::Null);
    let order = build.get("order").and_then(Value::as_array);
    let mut line_counts: BTreeMap<String, usize> = BTreeMap::new();
    for item in order.into_iter().flatten() {
        let line = item.get("line").and_then(Value::as_str).unwrap_or("");
        *line_counts.entry(line.to_string()).or_insert(0) += 1;
    }
    let sh = |a: &str| -> String {
        a.strip_prefix(head)
            .map(|rest| format!("H{rest}"))
            .unwrap_or_else(|| a.to_string())
    };
    let main_line = order
        .into_iter()
        .flatten()
        .filter(|o| o.get("line").and_then(Value::as_str) == Some("main"))
        .map(|o| {
            let addr = o.get("address").and_then(Value::as_str).unwrap_or("");
            let facet = o.get("facet").and_then(Value::as_str).unwrap_or("");
            format!("{} {facet}", sh(addr))
        })
        .collect::<Vec<_>>()
        .join(" | ");

    let mut out = vec![
        format!(
            "members {} {} (stage: {})",
            order.map_or(0, Vec::len),
            serde_json::to_string(&line_counts).unwrap_or_else(|_| "{}".to_string()),
            report.stage.as_str()
        ),
        format!("main line: {main_line}"),
    ];
    for f in &report.blocking {
        out.push(format!("[blocking] {}: {}", f.key, f.fix));
    }
    for f in &report.deferred {
        out.push(format!("[deferred -> {}] {}: {}", f.owner, f.key, f.fix));
    }
    let mut rel_counts: BTreeMap<String, usize> = BTreeMap::new();
    for r in model
        .get("relations")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let kind = r.get("kind").and_then(Value::as_str).unwrap_or("");
        *rel_counts.entry(kind.to_string()).or_insert(0) += 1;
    }
    let loose = model
        .get("loose")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let mut extras = vec![format!("self_sufficient {}", report.self_sufficient)];
    if !report.formal_structured.is_null() {
        extras.push(format!("formal_structured {}", report.formal_structured));
    }
    if !report.evidence_bound.is_null() {
        extras.push(format!("evidence_bound {}", report.evidence_bound));
    }
    out.push(format!(
        "relations {}; loose {}; {}; stage({}) {}",
        serde_json::to_string(&rel_counts).unwrap_or_else(|_| "{}".to_string()),
        loose,
        extras.join("; "),
        report.stage.as_str(),
        if report.ok { "pass" } else { "fail" }
    ));
    out
}

const ALLOWED_LEVELS: [&str; 3] = ["concept", "formal", "empirical"];
const ALLOWED_FORMS: [&str; 7] = [
    "theory",
    "definition",
    "axiom",
    "theorem",
    "proposition",
    "lemma",
    "corollary",
];

/// Validates top-level `level`, `build.level` agreement, and `form` against the core producer enums
/// (`src/model.rs::Level::id` and `src/math.rs::MathForm::id`) (H8).
pub(crate) fn validate_level_and_form<'a>(
    model: &'a Value,
    build: &serde_json::Map<String, Value>,
    head: &str,
) -> Result<(&'a str, bool)> {
    let Some(level_val) = model.get("level") else {
        bail!("get_model answer for {head} lacks top-level `level` field");
    };
    let level = match level_val {
        Value::Null => "",
        Value::String(s) if ALLOWED_LEVELS.contains(&s.as_str()) => s.as_str(),
        Value::String(s) => {
            bail!("get_model answer for {head} has unknown `level` `{s}`");
        }
        _ => bail!("get_model answer for {head} has invalid `level` (must be string or null)"),
    };
    if let Some(build_level) = build.get("level")
        && build_level != level_val
    {
        bail!(
            "get_model answer for {head} has contradictory `level` ({level_val}) vs `build.level` ({build_level})"
        );
    }
    let has_form = match model.get("form") {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) if ALLOWED_FORMS.contains(&s.as_str()) => true,
        Some(Value::String(s)) => {
            bail!("get_model answer for {head} has unknown `form` `{s}`");
        }
        Some(_) => bail!("get_model answer for {head} has invalid `form` (must be string or null)"),
    };
    Ok((level, has_form))
}

/// Validates required fields and fault-array types in a `get_model` payload (fail-closed, H8).
pub fn validate_model_payload(model: &Value, head: &str) -> Result<()> {
    let Some(build) = model.get("build").and_then(Value::as_object) else {
        bail!("get_model answer for {head} lacks `build` object");
    };
    if !build.get("order").is_some_and(Value::is_array) {
        bail!("get_model answer for {head} lacks `build.order` array");
    }
    if !build.get("self_sufficient").is_some_and(Value::is_boolean) {
        bail!("get_model answer for {head} lacks boolean `build.self_sufficient`");
    }
    for key in [
        "closure",
        "reach_in",
        "forward",
        "unanchored",
        "parts",
        "beyond_level",
        "import_target",
        "import_cycle",
    ] {
        if !build.get(key).is_some_and(Value::is_array) {
            bail!("get_model answer for {head} lacks required `build.{key}` array");
        }
    }
    let (level, has_form) = validate_level_and_form(model, build, head)?;
    if level == "formal" && !has_form {
        if !build.get("formal_structured").is_some_and(Value::is_boolean) {
            bail!("get_model answer for {head} lacks boolean `build.formal_structured`");
        }
        for key in [
            "formal_shape",
            "setting_dependency",
            "criterion_unbound",
            "result_unbound",
            "proof_unbound",
            "experiment_unbound",
            "symbol_uncovered",
        ] {
            if !build.get(key).is_some_and(Value::is_array) {
                bail!("get_model answer for {head} lacks required formal `build.{key}` array");
            }
        }
    }
    if level == "empirical" {
        if !build.get("evidence_bound").is_some_and(Value::is_boolean) {
            bail!("get_model answer for {head} lacks boolean `build.evidence_bound`");
        }
        for key in [
            "empirical_shape",
            "estimate_unbound",
            "spec_unbound",
            "test_unbound",
            "variable_uncovered",
        ] {
            if !build.get(key).is_some_and(Value::is_array) {
                bail!("get_model answer for {head} lacks required empirical `build.{key}` array");
            }
        }
    }
    for key in ALL_FAULT_KEYS {
        if let Some(val) = build.get(key)
            && !val.is_null()
            && !val.is_array()
        {
            bail!("get_model answer for {head} has non-array `build.{key}`");
        }
    }
    Ok(())
}

pub fn run_live(zt: &mut Zt, head: &str, stage: Stage, json_out: bool) -> Result<(Vec<String>, u8, usize)> {
    let model = zt.call("get_model", json!({ "at": head }))?;
    let got_head = model
        .get("head")
        .and_then(|h| h.get("address"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if got_head != head {
        if got_head.is_empty() {
            bail!("{head} is not a Model Card");
        }
        bail!("{head} is not a Model Card: it belongs to the model {got_head}; give that address");
    }
    validate_model_payload(&model, head)?;
    let report = evaluate(&model, stage);
    let fault_count = report.blocking.len();
    let exit = if report.ok { 0 } else { 1 };
    let lines = if json_out {
        vec![serde_json::to_string_pretty(&report.to_json(head))?]
    } else {
        format_text(&model, head, &report)
    };
    Ok((lines, exit, fault_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base_formal(overrides: Value) -> Value {
        let mut build = json!({
            "order": [{ "address": "K/2|a|a", "line": "main", "facet": "设定" }],
            "closure": [],
            "reach_in": [],
            "forward": [],
            "unanchored": [],
            "parts": [],
            "beyond_level": [],
            "import_target": [],
            "import_cycle": [],
            "formal_shape": [],
            "setting_dependency": [],
            "criterion_unbound": [],
            "result_unbound": [],
            "proof_unbound": [],
            "experiment_unbound": [],
            "symbol_uncovered": [],
            "self_sufficient": true,
            "formal_structured": true,
        });
        if let Some(map) = overrides.as_object() {
            for (k, v) in map {
                build[k] = v.clone();
            }
        }
        json!({
            "level": "formal",
            "form": null,
            "head": { "address": "K/2|a" },
            "build": build,
        })
    }

    #[test]
    fn fold_defers_later_stage_faults_and_blocks_on_structural_errors() {
        let m = base_formal(json!({
            "self_sufficient": false,
            "formal_structured": false,
            "closure": [{
                "from": "K/2|a|a|1",
                "to": "K/1|a|1",
                "card": { "type": "model", "facet": "结果：命题 1", "models": ["K/1|a"] },
                "fix": "import K/1|a|1"
            }],
            "setting_dependency": [{ "address": "K/2|a|a|1|a", "fix": "move setting" }],
            "result_unbound": [{ "address": "K/2|a|a|2|a", "fix": "add source" }],
        }));
        let fold = evaluate(&m, Stage::Fold);
        assert!(fold.ok, "fold should pass when only later-stage faults exist: {fold:?}");
        assert_eq!(fold.deferred.len(), 3);

        let bad_fold = evaluate(
            &base_formal(json!({
                "reach_in": [{ "from": "K/2|a|a", "to": "K/1|a|a", "fix": "reach_in" }]
            })),
            Stage::Fold,
        );
        assert!(!bad_fold.ok);
        assert_eq!(bad_fold.blocking.len(), 1);
        assert_eq!(bad_fold.blocking[0].key, "reach_in");

        let non_model_closure = evaluate(
            &base_formal(json!({
                "closure": [{ "from": "K/2|a|a", "to": "K/9", "card": { "type": "idea" }, "fix": "non-model" }]
            })),
            Stage::Fold,
        );
        assert!(!non_model_closure.ok);
        assert_eq!(non_model_closure.blocking[0].owner, "Stage 1 (fold)");
    }

    #[test]
    fn settings_blocks_setting_faults_and_defers_restructure_and_pending_notation() {
        let pending = base_formal(json!({
            "self_sufficient": false,
            "formal_structured": false,
            "closure": [{
                "from": "K/2|a|a|1",
                "to": "K/1|a|1",
                "card": { "type": "model", "facet": "结果：命题 1", "models": ["K/1|a"] },
                "fix": "import K/1|a|1"
            }],
            "result_unbound": [{ "address": "K/2|a|a|2|a", "fix": "add source" }],
        }));
        assert!(evaluate(&pending, Stage::Settings).ok);

        let bad = base_formal(json!({
            "formal_structured": false,
            "setting_dependency": [{ "address": "K/2|a|a|1|a", "fix": "setting under chapter" }],
        }));
        assert!(!evaluate(&bad, Stage::Settings).ok);

        let pending_notation = base_formal(json!({
            "closure": [{
                "from": "K/2|a|a",
                "to": "K/1|a|a|a",
                "card": { "type": "model", "facet": "符号表与读法", "home_role": "notation", "models": ["K/1|a"] },
                "fix": "import notation"
            }],
            "formal_shape": [{ "address": "K/2|a", "lacks": "符号表与读法", "fix": "missing notation" }],
        }));
        let res = evaluate(&pending_notation, Stage::Settings);
        assert!(res.ok, "pending notation import should defer missing notation shape: {res:?}");
        assert!(res.deferred.iter().any(|f| f.key == "formal_shape" && f.owner == "Stage 4 (import)"));

        // Non-head formal_shape with lacks="符号表与读法" (e.g. broken local 沿用 lender) must NOT defer.
        let broken_lender = base_formal(json!({
            "closure": [{
                "from": "K/2|a|a",
                "to": "K/1|a|a|a",
                "card": { "type": "model", "facet": "符号表与读法", "home_role": "notation", "models": ["K/1|a"] },
                "fix": "import notation"
            }],
            "formal_shape": [{ "address": "K/2|a|a|a", "lacks": "符号表与读法", "fix": "沿用 target is not notation" }],
        }));
        let res_lender = evaluate(&broken_lender, Stage::Settings);
        assert!(!res_lender.ok, "broken local notation lender must block Stage 2");
        assert_eq!(res_lender.blocking[0].key, "formal_shape");
    }

    #[test]
    fn settings_blocks_empirical_stage2_faults_and_defers_stage3_faults() {
        let emp_stage3_only = json!({
            "level": "empirical",
            "head": { "address": "K/5|a" },
            "build": {
                "order": [{ "address": "K/5|a|a", "line": "main", "role": "data" }],
                "empirical_shape": [{ "address": "K/5|a", "lacks": "估计", "fix": "zero 估计" }],
                "estimate_unbound": [{ "address": "K/5|a|a|3|a", "fix": "bind estimate" }],
                "test_unbound": [{ "address": "K/5|a|a|3|b", "fix": "bind test" }],
                "self_sufficient": true,
                "evidence_bound": false
            }
        });
        let ok_s2 = evaluate(&emp_stage3_only, Stage::Settings);
        assert!(ok_s2.ok, "Stage 3 empirical faults should defer at Stage 2: {ok_s2:?}");
        assert_eq!(ok_s2.deferred.len(), 3);

        let emp_stage2_bad = json!({
            "level": "empirical",
            "head": { "address": "K/5|a" },
            "build": {
                "order": [{ "address": "K/5|a|a", "line": "main", "role": "data" }],
                "empirical_shape": [{ "address": "K/5|a", "lacks": "变量表", "fix": "missing 变量表" }],
                "spec_unbound": [{ "address": "K/5|a|a|2", "fix": "spec needs design" }],
                "self_sufficient": true,
                "evidence_bound": false
            }
        });
        let bad_s2 = evaluate(&emp_stage2_bad, Stage::Settings);
        assert!(!bad_s2.ok);
        assert_eq!(bad_s2.blocking.len(), 2);
        assert!(bad_s2.blocking.iter().all(|f| f.owner == "Stage 2 (settings)"));
    }

    #[test]
    fn restructure_defers_only_cross_model_closure_and_pending_criterion_import() {
        let pending_crit = base_formal(json!({
            "self_sufficient": false,
            "closure": [{
                "from": "K/2|a|a|1",
                "to": "K/1|a|b",
                "card": { "type": "model", "facet": "均衡定义：竞争均衡", "home_role": "criterion", "models": ["K/1|a"] },
                "fix": "import K/1|a|b"
            }],
            "criterion_unbound": [{ "address": "K/2|a", "fix": "needs imported criterion" }],
        }));
        assert!(evaluate(&pending_crit, Stage::Restructure).ok);

        let pending_non_crit = base_formal(json!({
            "self_sufficient": false,
            "closure": [{
                "from": "K/2|a|a|1",
                "to": "K/1|a|1",
                "card": { "type": "model", "facet": "结果：命题 1", "home_role": "result", "models": ["K/1|a"] },
                "fix": "import K/1|a|1"
            }],
            "criterion_unbound": [{ "address": "K/2|a", "fix": "no criterion anywhere" }],
        }));
        assert!(!evaluate(&pending_non_crit, Stage::Restructure).ok);

        let bad_result = base_formal(json!({
            "formal_structured": false,
            "result_unbound": [{ "address": "K/2|a|a|2|a", "fix": "missing justification" }],
        }));
        assert!(!evaluate(&bad_result, Stage::Restructure).ok);
    }

    #[test]
    fn final_requires_strict_flags_and_zero_parts_faults() {
        assert!(evaluate(&base_formal(json!({})), Stage::Final).ok);

        let missing_formal = base_formal(json!({ "formal_structured": null }));
        let res = evaluate(&missing_formal, Stage::Final);
        assert!(!res.ok);
        assert!(res.blocking.iter().any(|b| b.key == "formal_structured"));

        let bad_parts = base_formal(json!({
            "parts": [{ "line": "第一部分", "fix": "starts off main line" }]
        }));
        let res_parts = evaluate(&bad_parts, Stage::Final);
        assert!(!res_parts.ok);
        assert!(res_parts.blocking.iter().any(|b| b.key == "parts"));

        let emp_missing = json!({
            "level": "empirical",
            "head": { "address": "K/5|a" },
            "build": {
                "order": [],
                "self_sufficient": true,
                "evidence_bound": false
            }
        });
        let res_emp = evaluate(&emp_missing, Stage::Final);
        assert!(!res_emp.ok);
        assert!(res_emp.blocking.iter().any(|b| b.key == "evidence_bound"));

        // P1-1 & H8: wrong-typed or omitted required fault arrays, missing/contradictory level,
        // or malformed form must fail payload validation.
        let bad_payload = base_formal(json!({ "closure": "not-an-array" }));
        assert!(validate_model_payload(&bad_payload, "K/2|a").is_err());
        let mut missing_parts = base_formal(json!({}));
        missing_parts["build"].as_object_mut().unwrap().remove("parts");
        assert!(validate_model_payload(&missing_parts, "K/2|a").is_err());
        let mut missing_level = base_formal(json!({ "level": "formal", "formal_structured": false }));
        missing_level.as_object_mut().unwrap().remove("level");
        assert!(validate_model_payload(&missing_level, "K/2|a").is_err());
        let mismatch_level = base_formal(json!({ "level": "empirical" }));
        assert!(validate_model_payload(&mismatch_level, "K/2|a").is_err());
        let mut bad_form = base_formal(json!({ "formal_structured": false }));
        bad_form["form"] = json!(true);
        assert!(validate_model_payload(&bad_form, "K/2|a").is_err());
        let mut typo_form = base_formal(json!({ "formal_structured": false }));
        typo_form["form"] = json!("typo");
        assert!(validate_model_payload(&typo_form, "K/2|a").is_err());
        let mut typo_level = base_formal(json!({}));
        typo_level["level"] = json!("forml");
        assert!(validate_model_payload(&typo_level, "K/2|a").is_err());
        let mut math_unit = base_formal(json!({ "formal_structured": false }));
        math_unit["form"] = json!("theorem");
        assert!(validate_model_payload(&math_unit, "K/2|a").is_ok());
        assert!(validate_model_payload(&base_formal(json!({})), "K/2|a").is_ok());

        // M4: book key containing "zero" with missing design ("设计") must NOT defer as Stage 3.
        let zero_book = json!({
            "level": "empirical",
            "head": { "address": "zero2020/1" },
            "build": {
                "order": [{ "address": "zero2020/1|a", "line": "main", "role": "data" }],
                "empirical_shape": [{
                    "address": "zero2020/1",
                    "lacks": "设计",
                    "fix": "the main line of the empirical model `zero2020/1` holds no `设计：` part (the comparison that identifies the estimate)"
                }],
                "self_sufficient": true,
                "evidence_bound": false
            }
        });
        assert!(!evaluate(&zero_book, Stage::Settings).ok);

        // P1-6: non-canonical 符号 facet (e.g. "符号说明") must NOT defer missing notation via formal::role_of.
        let non_canonical_lender = base_formal(json!({
            "closure": [{
                "from": "K/2|a|a",
                "to": "K/1|a|a|a",
                "card": { "type": "model", "facet": "符号说明", "models": ["K/1|a"] },
                "fix": "import"
            }],
            "formal_shape": [{ "address": "K/2|a", "lacks": "符号表与读法", "fix": "missing notation" }],
        }));
        assert!(!evaluate(&non_canonical_lender, Stage::Settings).ok);
    }
}
