//! The Level 2 formal economic model path of the model compiler (docs/formal-models-proposal.md §5, §8):
//! plan validation (6 claim objects, 15 `FormalRole`s, flat `of`/`after` placement invariants,
//! multi-justification claims, `inventory.json` grounding), unified branch-aware Derivation
//! chain enumeration, role skeletons, Model Card sections, and close-round formal/inventory checks.

use super::inventory;
use super::text::{self, derives, line_of, ref_of};
use crate::ps::{self, CiMap, list, pipe, s, truthy};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use zt_check::formal::{
    CLAIM_OBJECTS, FormalRole, JUSTIFICATION_FORCES, JUSTIFICATION_LOCATIONS, LOGICAL_FORCES,
    SOURCE_KINDS, role_of,
};

/// Single authoritative relation-requirements extractor shared by skeleton generation, close
/// (`planned_link_entries`), and `model-finish` (`ledger.rs`, Astra #1):
/// in a Level 2 formal model, plain filing edges (`of`, `after`) establish placement only and
/// never require an intellectual Link unless `kind` is explicitly set.
pub(crate) fn planned_relations(part: &Value, formal_model: bool) -> Vec<(String, String)> {
    let mut wanted = Vec::new();
    for rest in list(&part["rests"]) {
        let reference = s(&ps::at(&pipe(rest), 0));
        if !reference.is_empty() {
            wanted.push((reference, s(&ps::at(&pipe(rest), 1))));
        }
    }
    if truthy(&part["of"]) && (!formal_model || truthy(&part["kind"])) {
        wanted.push((s(&part["of"]), s(&part["why"])));
    }
    wanted
}

/// Whether a `plan.json` describes a Level 2 formal economic model (level `"formal"` and not a
/// mathematical unit carrying `form`). All Level 2 formal economic models use this unified design.
pub(crate) fn is_formal_model(plan: &Value, is_math_unit: bool) -> bool {
    !is_math_unit && ps::eq(&s(&plan["level"]), "formal")
}

/// The canonical `FormalRole` of a plan part, read from its facet.
pub(crate) fn role(part: &Value) -> Option<FormalRole> {
    role_of(&s(&part["facet"]))
}

fn id_of<'a>(parts: &'a [Value], id: &str) -> Option<&'a Value> {
    parts.iter().find(|p| ps::eq(&s(&p["id"]), id))
}

/// Whether a part participates in algebraic Derivation chains (either explicitly `derive: true`
/// or carrying the `推导` formal role; Astra M2).
pub(crate) fn is_deriving_part(part: &Value) -> bool {
    derives(part) || role(part) == Some(FormalRole::Derivation)
}

/// Unified branch-aware Derivation chain enumerator (proposal §5.2 & §12 F4):
/// returns every maximal contiguous run of `derive: true` (or `推导`) parts on BOTH:
/// 1. the `main` line (math units / flat models), and
/// 2. any `side` branch linked by `of` / `after` (breaking at non-Derivation cards such as `论证`).
pub(crate) fn plan_derivation_chains(plan: &Value) -> Vec<Vec<String>> {
    let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
    let mut chains: Vec<Vec<String>> = Vec::new();

    // 1. Main-line Derivation runs (math units).
    let main: Vec<&Value> = parts
        .iter()
        .filter(|p| ps::eq(&line_of(p), "main"))
        .collect();
    let mut run: Vec<String> = Vec::new();
    for part in main.into_iter().map(Some).chain(std::iter::once(None)) {
        if let Some(part) = part.filter(|p| is_deriving_part(p)) {
            let id = s(&part["id"]);
            if !id.is_empty() {
                run.push(id);
            }
            continue;
        }
        if !run.is_empty() {
            chains.push(std::mem::take(&mut run));
        }
    }

    // 2. Side-branch maximal contiguous Derivation runs (`of` root -> `after` continuations).
    let side_parts: Vec<&Value> = parts
        .iter()
        .filter(|p| ps::eq(&line_of(p), "side"))
        .collect();
    let mut by_id: CiMap<&Value> = CiMap::new();
    let mut next_after: CiMap<String> = CiMap::new();
    for p in &side_parts {
        let id = s(&p["id"]);
        if !id.is_empty() {
            by_id.insert(&id, *p);
        }
        let after = s(&p["after"]);
        if !after.is_empty() && !id.is_empty() && !next_after.contains(&after) {
            next_after.insert(&after, id);
        }
    }
    for p in &side_parts {
        if !is_deriving_part(p) {
            continue;
        }
        let id = s(&p["id"]);
        if id.is_empty() {
            continue;
        }
        let after = s(&p["after"]);
        // Start a run only if this part is NOT continuing another Side Derivation part.
        if !after.is_empty()
            && by_id.get(&after).is_some_and(|prev| is_deriving_part(prev))
        {
            continue;
        }
        let mut branch_run = vec![id.clone()];
        let mut cur = id;
        while let Some(next_id) = next_after.get(&cur).cloned() {
            if branch_run.iter().any(|seen| ps::eq(seen, &next_id)) {
                break;
            }
            if by_id.get(&next_id).is_some_and(|np| is_deriving_part(np)) {
                branch_run.push(next_id.clone());
                cur = next_id;
            } else {
                break;
            }
        }
        chains.push(branch_run);
    }

    chains
}

/// Map each Derivation plan id -> `(k, n, tos)` across all main-line and Side-branch runs.
pub(crate) fn plan_chains(plan: &Value) -> CiMap<(usize, usize, Vec<String>)> {
    let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
    let mut out = CiMap::new();
    for chain in plan_derivation_chains(plan) {
        let tos: Vec<String> = chain
            .iter()
            .map(|id| id_of(&parts, id).map(|p| s(&p["to"])).unwrap_or_default())
            .collect();
        let n = chain.len();
        for (i, id) in chain.iter().enumerate() {
            out.insert(id, (i + 1, n, tos.clone()));
        }
    }
    out
}

/// Compute a safe placement order for `side` parts so every `of` and `after` parent is created
/// before any child that references it. Returns `Err(cycle_ids)` if a placement cycle exists.
pub(crate) fn side_placement_order(parts: &[Value]) -> std::result::Result<Vec<usize>, Vec<String>> {
    let mut id_to_idx: CiMap<usize> = CiMap::new();
    let mut side_indices = Vec::new();
    for (idx, p) in parts.iter().enumerate() {
        let id = s(&p["id"]);
        if !id.is_empty() {
            id_to_idx.insert(&id, idx);
        }
        if ps::eq(&line_of(p), "side") {
            side_indices.push(idx);
        }
    }
    let side_set: BTreeSet<usize> = side_indices.iter().copied().collect();
    let mut placed: BTreeSet<usize> = BTreeSet::new();
    let mut ordered = Vec::with_capacity(side_indices.len());
    loop {
        let mut progressed = false;
        for &idx in &side_indices {
            if placed.contains(&idx) {
                continue;
            }
            let p = &parts[idx];
            let dep_id = if truthy(&p["after"]) {
                s(&p["after"])
            } else {
                s(&p["of"])
            };
            let dep_pending = id_to_idx
                .get(&dep_id)
                .is_some_and(|&d| side_set.contains(&d) && !placed.contains(&d));
            if !dep_pending {
                placed.insert(idx);
                ordered.push(idx);
                progressed = true;
            }
        }
        if ordered.len() == side_indices.len() {
            return Ok(ordered);
        }
        if !progressed {
            let stuck = side_indices
                .into_iter()
                .filter(|idx| !placed.contains(idx))
                .map(|idx| s(&parts[idx]["id"]))
                .collect();
            return Err(stuck);
        }
    }
}

/// Validate a Level 2 formal `plan.json` (proposal §5.1–§5.5, §8.4–§8.5).
pub(crate) fn plan_faults(
    plan: &Value,
    inv: Option<&Value>,
    staging: Option<&str>,
) -> Vec<String> {
    let mut faults = Vec::new();

    // 1. claim_objects (non-empty, all valid)
    let claim_objects: Vec<String> = pipe(&plan["claim_objects"])
        .into_iter()
        .map(|v| s(v).to_lowercase())
        .filter(|v| !v.is_empty())
        .collect();
    if claim_objects.is_empty() {
        faults.push(format!(
            "claim_objects is missing: list one or more of {}",
            CLAIM_OBJECTS.join(", ")
        ));
    }
    for obj in &claim_objects {
        if !CLAIM_OBJECTS.contains(&obj.as_str()) {
            faults.push(format!(
                "claim_objects contains unknown object `{obj}` (expected one of {})",
                CLAIM_OBJECTS.join(", ")
            ));
        }
    }

    let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
    let mut by_id: CiMap<&Value> = CiMap::new();
    for p in &parts {
        let id = s(&p["id"]);
        if !id.is_empty() {
            by_id.insert(&id, p);
        }
    }

    // 2. Placement invariants (mutually exclusive of/after, valid targets, no duplicate after, no cycles).
    let mut after_used: CiMap<String> = CiMap::new();
    for p in &parts {
        let id = s(&p["id"]);
        let line = line_of(p);
        if ps::eq(&line, "side") {
            let has_of = truthy(&p["of"]);
            let has_after = truthy(&p["after"]);
            if has_of && has_after {
                faults.push(format!(
                    "{id}: a formal side part must specify exactly one of `of` or `after`, not both"
                ));
            } else if !has_of && !has_after {
                faults.push(format!(
                    "{id}: a formal side part needs `of` (branch root) or `after` (direct continuation)"
                ));
            }
            if has_after {
                let prev = s(&p["after"]);
                if ps::eq(&prev, &id) {
                    faults.push(format!("{id}: `after` cannot point to itself"));
                } else {
                    match by_id.get(&prev) {
                        None => faults.push(format!(
                            "{id}: `after` target `{prev}` is not a local part of this plan"
                        )),
                        Some(target) if !ps::eq(&line_of(target), "side") => faults.push(format!(
                            "{id}: `after` target `{prev}` must be a local `side` part of the same model"
                        )),
                        Some(_) => {}
                    }
                }
                if let Some(other) = after_used.get(&prev) {
                    faults.push(format!(
                        "{id}: duplicate `after` target `{prev}` (already continued by `{other}`)"
                    ));
                } else {
                    after_used.insert(&prev, id.clone());
                }
            }
        } else if truthy(&p["after"]) {
            faults.push(format!(
                "{id}: `after` is only valid on `line: \"side\"` parts"
            ));
        }

        // Canonical 15 roles on every new, existing, or moved part carrying a facet (Astra H12).
        let pref = ref_of(p);
        if pref == "new" || ((pref == "existing" || pref == "move") && truthy(&p["facet"])) {
            let Some(r) = role(p) else {
                faults.push(format!(
                    "{id}: facet '{}' names no canonical formal role (设定, 符号表与读法, 假设, 定义, 判据/均衡定义/解概念, 第 k 章, 基准, 结果, 推导, 论证, 边界, 校准, 数值实验, 例示, 局限)",
                    s(&p["facet"])
                ));
                continue;
            };
            if pref != "new" {
                continue;
            }
            match r {
                FormalRole::Derivation => {
                    if !ps::eq(&line, "side") {
                        faults.push(format!(
                            "{id}: in a formal model, a `推导` part stands on a Side proof branch under its `结果` or `基准` (`line: \"side\"` with `of` or `after`)"
                        ));
                    }
                    if !derives(p) {
                        faults.push(format!(
                            "{id}: a `推导` part in a formal model should set `derive: true` (or use `论证` for non-algebraic arguments)"
                        ));
                    }
                }
                FormalRole::Argument => {
                    if !ps::eq(&line, "side") {
                        faults.push(format!(
                            "{id}: an `论证` part stands on a Side proof branch under its `结果` or `基准`"
                        ));
                    }
                    if derives(p) {
                        faults.push(format!(
                            "{id}: an `论证` part is a non-algebraic argument and must not set `derive: true`"
                        ));
                    }
                }
                FormalRole::Result => {
                    if !ps::eq(&line, "side") {
                        faults.push(format!(
                            "{id}: a `结果` part stands on a Side branch of its chapter (`line: \"side\"`, `of: <chapter>`)"
                        ));
                    } else if truthy(&p["of"]) {
                        let parent_id = s(&p["of"]);
                        if let Some(parent) = by_id.get(&parent_id)
                            && truthy(&parent["facet"])
                            && role(parent) != Some(FormalRole::Chapter)
                        {
                            faults.push(format!(
                                "{id}: a `结果` part's `of` parent `{parent_id}` must be a chapter (`第 k 章`), not '{}'",
                                s(&parent["facet"])
                            ));
                        }
                    }
                    let sk = s(&p["source_kind"]).to_lowercase();
                    if sk.is_empty() || !SOURCE_KINDS.contains(&sk.as_str()) {
                        faults.push(format!(
                            "{id}: a formal `结果` needs `source_kind` ({})",
                            SOURCE_KINDS.join(" | ")
                        ));
                    } else if sk != "unnumbered" && s(&p["source_label"]).trim().is_empty() {
                        faults.push(format!(
                            "{id}: `结果` with source_kind `{sk}` needs `source_label` (e.g. Proposition 1, Theorem 2)"
                        ));
                    }
                    if truthy(&p["criterion"]) {
                        let cid = s(&p["criterion"]);
                        match by_id.get(&cid) {
                            Some(target) if truthy(&target["facet"]) && role(target) != Some(FormalRole::Criterion) => {
                                faults.push(format!(
                                    "{id}: criterion `{cid}` must name a `判据`/`均衡定义`/`解概念` part, not '{}'",
                                    s(&target["facet"])
                                ));
                            }
                            None if !cid.contains('/') && !cid.contains('#') => {
                                faults.push(format!("{id}: criterion `{cid}` is not a part of this plan"));
                            }
                            _ => {}
                        }
                    }
                    for a in pipe(&p["assumptions"]) {
                        let aid = s(a);
                        if aid.is_empty() {
                            continue;
                        }
                        match by_id.get(&aid) {
                            Some(target) if truthy(&target["facet"]) && role(target) != Some(FormalRole::Assumption) => {
                                faults.push(format!(
                                    "{id}: assumption `{aid}` must name an `假设` part, not '{}'",
                                    s(&target["facet"])
                                ));
                            }
                            None if !aid.contains('/') && !aid.contains('#') => {
                                faults.push(format!("{id}: assumption `{aid}` is not a part of this plan"));
                            }
                            _ => {}
                        }
                    }
                    let lf = s(&p["logical_force"]).to_lowercase();
                    if lf.is_empty() || !LOGICAL_FORCES.contains(&lf.as_str()) {
                        faults.push(format!(
                            "{id}: a formal `结果` needs `logical_force` ({})",
                            LOGICAL_FORCES.join(" | ")
                        ));
                    }
                    let justs = list(&p["justifications"]);
                    if justs.is_empty() {
                        faults.push(format!(
                            "{id}: a formal `结果` needs `justifications[]` (1+ entries with force, location, ref)"
                        ));
                    }
                    for (j_idx, j) in justs.iter().enumerate() {
                        let force = s(&j["force"]).to_lowercase();
                        let loc = s(&j["location"]).to_lowercase();
                        let jref = s(&j["ref"]);
                        if !JUSTIFICATION_FORCES.contains(&force.as_str()) {
                            faults.push(format!(
                                "{id}: justifications[{j_idx}].force `{force}` is not one of {}",
                                JUSTIFICATION_FORCES.join(", ")
                            ));
                        }
                        if !JUSTIFICATION_LOCATIONS.contains(&loc.as_str()) {
                            faults.push(format!(
                                "{id}: justifications[{j_idx}].location `{loc}` is not one of {}",
                                JUSTIFICATION_LOCATIONS.join(", ")
                            ));
                        }
                        if force != "omitted" && jref.is_empty() {
                            faults.push(format!(
                                "{id}: justifications[{j_idx}].ref is missing"
                            ));
                        }
                        if loc == "local" && !jref.is_empty() {
                            let is_inline = jref.starts_with("inline:") || jref.starts_with("§");
                            if !is_inline {
                                match by_id.get(&jref) {
                                    None => faults.push(format!(
                                        "{id}: local justification ref `{jref}` is not a part of this plan (or an `inline:`/`§` locator)"
                                    )),
                                    Some(root) => {
                                        let rrole = role(root);
                                        if !rrole.is_some_and(FormalRole::is_proof_step) {
                                            faults.push(format!(
                                                "{id}: local justification ref `{jref}` must be a `推导` or `论证` part"
                                            ));
                                        }
                                        if !ps::eq(&s(&root["of"]), &id) {
                                            faults.push(format!(
                                                "{id}: local justification route root `{jref}` must set `of: \"{id}\"`"
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                FormalRole::Limits => {
                    if !ps::eq(&line, "off") {
                        faults.push(format!(
                            "{id}: whole-model `局限` stands off the main line (`line: \"off\"`); use `边界` for result-level Side caveats"
                        ));
                    }
                }
                FormalRole::Experiment => {
                    let mut cal_valid = false;
                    if truthy(&p["calibration"]) {
                        let cid = s(&p["calibration"]);
                        match by_id.get(&cid) {
                            Some(target) => {
                                if !truthy(&target["facet"]) || matches!(
                                    role(target),
                                    Some(FormalRole::Calibration | FormalRole::Setting | FormalRole::Benchmark | FormalRole::Criterion)
                                ) {
                                    cal_valid = true;
                                } else {
                                    faults.push(format!(
                                        "{id}: calibration `{cid}` must name a `校准`/`设定`/`基准`/`判据` part, not '{}'",
                                        s(&target["facet"])
                                    ));
                                }
                            }
                            None if cid.contains('/') || cid.contains('#') => {
                                cal_valid = true;
                            }
                            None => {
                                faults.push(format!("{id}: calibration `{cid}` is not a part of this plan"));
                            }
                        }
                    }
                    let has_base = cal_valid
                        || list(&p["rests"]).iter().any(|r| {
                            let target = s(&ps::at(&pipe(r), 0));
                            by_id.get(&target).and_then(|t| role(t)).is_some_and(|tr| {
                                matches!(
                                    tr,
                                    FormalRole::Calibration
                                        | FormalRole::Setting
                                        | FormalRole::Benchmark
                                        | FormalRole::Criterion
                                )
                            })
                        });
                    if !has_base {
                        faults.push(format!(
                            "{id}: a `数值实验` part must rest on a `校准` or `设定` part"
                        ));
                    }
                }
                _ => {}
            }
        }
    }

    if let Err(stuck) = side_placement_order(&parts) {
        faults.push(format!(
            "placement cycle in side parts `of`/`after`: {}",
            stuck.join(", ")
        ));
    }

    // 2b. First Derivation of every Side chain needs `from` or `rests`; no proof step may rest on its own Result.
    for chain in plan_derivation_chains(plan) {
        if let Some(first_id) = chain.first()
            && let Some(first) = by_id.get(first_id)
            && ps::eq(&line_of(first), "side")
            && !truthy(&first["from"])
            && list(&first["rests"]).is_empty()
        {
            faults.push(format!(
                "{first_id}: the first Derivation part of a Side chain needs `from` or `rests` (its starting point)"
            ));
        }
    }
    let owning_result = |start_id: &str| -> Option<String> {
        let mut cur = start_id.to_string();
        let mut seen = BTreeSet::new();
        while seen.insert(cur.to_lowercase()) {
            let p = by_id.get(&cur)?;
            let parent = if truthy(&p["after"]) { s(&p["after"]) } else { s(&p["of"]) };
            if parent.is_empty() {
                return None;
            }
            if by_id.get(&parent).and_then(|pp| role(pp)) == Some(FormalRole::Result) {
                return Some(parent);
            }
            cur = parent;
        }
        None
    };
    for p in &parts {
        let id = s(&p["id"]);
        if role(p).is_some_and(FormalRole::is_proof_step)
            && let Some(owner) = owning_result(&id)
        {
            for r in list(&p["rests"]) {
                let target = s(&ps::at(&pipe(r), 0));
                if ps::eq(&target, &owner) {
                    faults.push(format!(
                        "{id}: a proof step under `{owner}` must not rest on `{owner}` itself (the claim it proves)"
                    ));
                }
            }
        }
    }
    // 2c. Detect forward/self rests-on within any Side `of` -> `after` continuation chain (Astra #14).
    let mut next_after: CiMap<String> = CiMap::new();
    for p in &parts {
        let id = s(&p["id"]);
        let after = s(&p["after"]);
        if ps::eq(&line_of(p), "side") && !after.is_empty() && !id.is_empty() && !next_after.contains(&after) {
            next_after.insert(&after, id);
        }
    }
    for p in &parts {
        if ps::eq(&line_of(p), "side") && truthy(&p["of"]) {
            let mut branch: Vec<String> = vec![s(&p["id"])];
            let mut cur = s(&p["id"]);
            while let Some(nxt) = next_after.get(&cur).cloned() {
                if branch.iter().any(|b| ps::eq(b, &nxt)) {
                    break;
                }
                branch.push(nxt.clone());
                cur = nxt;
            }
            for (idx, step_id) in branch.iter().enumerate() {
                if let Some(sp) = by_id.get(step_id) {
                    for r in list(&sp["rests"]) {
                        let target = s(&ps::at(&pipe(r), 0));
                        if let Some(t_idx) = branch.iter().position(|b| ps::eq(b, &target))
                            && t_idx >= idx
                        {
                            faults.push(format!(
                                "forward: Side step `{step_id}` rests on `{target}`, which comes at or later in the same Side branch"
                            ));
                        }
                    }
                }
            }
        }
    }

    // 3. Structural shape: <head>|a = Setting, <head>|a|a = Notation (first side of setting, unless imported),
    // >= 1 Result, no Setting/Assumption filed under a Chapter, Criterion when claim_objects require one.
    let main_parts: Vec<&Value> = parts
        .iter()
        .filter(|p| ps::eq(&line_of(p), "main"))
        .collect();
    let setting_id = main_parts.first().map(|p| s(&p["id"])).unwrap_or_default();
    if main_parts.first().and_then(|p| role(p)) != Some(FormalRole::Setting)
        && !main_parts.first().is_some_and(|p| truthy(&p["existing"]))
    {
        faults.push("formal: the first main-line part must be the setting root (facet `设定`)".into());
    }
    let first_setting_side = parts
        .iter()
        .find(|p| ps::eq(&line_of(p), "side") && ps::eq(&s(&p["of"]), &setting_id));
    let has_imported_notation = parts
        .iter()
        .any(|p| truthy(&p["take"]) && role(p) == Some(FormalRole::Notation));
    if first_setting_side.and_then(role) != Some(FormalRole::Notation) && !has_imported_notation {
        faults.push(format!(
            "formal: the first Side child of `{setting_id}` must be `符号表与读法` (`<head>|a|a`), unless notation is imported"
        ));
    }
    if !parts.iter().any(|p| !truthy(&p["take"]) && role(p) == Some(FormalRole::Result)) {
        faults.push("formal: the plan must hold at least one owned (non-take) `结果：` part".into());
    }
    let needs_criterion = claim_objects.iter().any(|obj| {
        matches!(
            obj.as_str(),
            "market-equilibrium" | "strategic-allocation" | "identification"
        )
    });
    if needs_criterion && !parts.iter().any(|p| role(p) == Some(FormalRole::Criterion)) {
        faults.push(format!(
            "formal: claim_objects ({}) requires at least one `判据` / `均衡定义` / `解概念` part",
            claim_objects.join(", ")
        ));
    }

    // Check setting_dependency: no Setting/Assumption under a Chapter.
    let root_ancestor = |start_id: &str| -> Option<String> {
        let mut cur = start_id.to_string();
        let mut seen = BTreeSet::new();
        while seen.insert(cur.to_lowercase()) {
            let Some(p) = by_id.get(&cur) else { break };
            if ps::eq(&line_of(p), "main") {
                return Some(cur);
            }
            let next = if truthy(&p["after"]) {
                s(&p["after"])
            } else {
                s(&p["of"])
            };
            if next.is_empty() {
                break;
            }
            cur = next;
        }
        None
    };
    for p in &parts {
        let id = s(&p["id"]);
        if ps::eq(&line_of(p), "side")
            && matches!(role(p), Some(FormalRole::Setting | FormalRole::Assumption))
            && let Some(anc) = root_ancestor(&id)
            && by_id.get(&anc).and_then(|a| role(a)) == Some(FormalRole::Chapter)
        {
            faults.push(format!(
                "setting_dependency: {id} ({}) is filed under chapter `{anc}`; move maintained settings and assumptions under `{setting_id}`",
                role(p).unwrap().id()
            ));
        }
    }

    // 4. Pinned inventory.json validation (Layer 3).
    if let Some(inv) = inv {
        faults.extend(inventory::validate_plan(plan, inv, staging));
    }

    faults
}

/// Model Card script-written sections for a Level 2 formal model (proposal §5.4).
pub(crate) fn head_sections(
    plan: &Value,
    resolve: &dyn Fn(&str) -> Result<String>,
) -> Result<Vec<(String, String)>> {
    let claim_objects: Vec<String> = pipe(&plan["claim_objects"])
        .into_iter()
        .map(s)
        .filter(|v| !v.is_empty())
        .collect();
    let family_tags: Vec<String> = pipe(&plan["family_tags"])
        .into_iter()
        .map(s)
        .filter(|v| !v.is_empty())
        .collect();
    let mut obj_lines = vec![format!("- 对象：{}", claim_objects.join("、"))];
    if !family_tags.is_empty() {
        obj_lines.push(format!("- 标签：{}", family_tags.join("、")));
    }

    let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
    let criteria: Vec<String> = parts
        .iter()
        .filter(|p| role(p) == Some(FormalRole::Criterion))
        .map(|p| Ok(format!("[[{}]]", resolve(&s(&p["id"]))?)))
        .collect::<Result<Vec<_>>>()?;
    let crit_hint = if criteria.is_empty() {
        "待写（核心参与者、作用域与求解判据）。".to_string()
    } else {
        format!("待写（核心参与者、作用域与判据 {}）。", criteria.join("、"))
    };

    let mut results: Vec<&Value> = parts
        .iter()
        .filter(|p| role(p) == Some(FormalRole::Result) && ps::eq(&s(&p["emphasis"]), "core"))
        .collect();
    if results.is_empty() {
        results = parts
            .iter()
            .filter(|p| role(p) == Some(FormalRole::Result))
            .collect();
    }
    let mut rows = vec![
        "| 结果 | 逻辑力度 | 作用域 / 条件 | 依据状态 |".to_string(),
        "| --- | --- | --- | --- |".to_string(),
    ];
    for p in results.into_iter().take(7) {
        let at = resolve(&s(&p["id"]))?;
        let title = super::empirical::table_cell(&s(&p["title"]));
        let lf = s(&p["logical_force"]);
        let just_summary: Vec<String> = list(&p["justifications"])
            .into_iter()
            .map(|j| {
                let f = s(&j["force"]);
                let loc = s(&j["location"]);
                format!("{f}/{loc}")
            })
            .collect();
        let scope = if truthy(&p["scope"]) {
            super::empirical::table_cell(&s(&p["scope"]))
        } else {
            "见结果卡".to_string()
        };
        rows.push(format!(
            "| [[{at}]] {title} | {lf} | {scope} | {} |",
            just_summary.join("、")
        ));
    }

    Ok(vec![
        ("目标对象与标签".to_string(), obj_lines.join("\n")),
        ("原语与判据".to_string(), crit_hint),
        ("主要结果".to_string(), rows.join("\n")),
    ])
}

/// Minimal `FM2`–`FM6`-admissible initial placeholder body used at `create_successor` /
/// `insert_card` time (before final addresses exist), so creation passes `Stage::Enforce` while
/// still carrying bare `待写` lines for unwritten/placeholder detection (Astra C1).
pub(crate) fn initial_skeleton(part: &Value, _plan: &Value) -> String {
    match role(part) {
        Some(FormalRole::Notation) => {
            "**怎样读卡片上的记号**\n待写\n\n| 符号 | 含义 | 首次出现 |\n| --- | --- | --- |\n| $x$ | 待写 | 本模型 |\n\n**统一用语**\n\n| 术语 | 不用 |\n| --- | --- |\n| 术语待写 | 待写 |".to_string()
        }
        Some(FormalRole::Setting) => {
            "待写\n\n**起点**\n- 原始设定：待写。\n\n**设定**\n待写\n\n**直觉**\n待写\n\n**说明**\n待写".to_string()
        }
        Some(FormalRole::Assumption) => {
            "待写\n\n**起点**\n- 待写\n\n**假设**\n待写\n\n**直觉**\n待写\n\n**说明**\n待写".to_string()
        }
        Some(FormalRole::Definition) => {
            "待写\n\n**起点**\n- 待写\n\n**定义**\n待写\n\n**直觉**\n待写\n\n**说明**\n待写".to_string()
        }
        Some(FormalRole::Criterion) => {
            let facet = s(&part["facet"]);
            let heading = facet
                .find(['：', ':'])
                .map_or(facet.as_str(), |at| &facet[..at])
                .trim();
            let heading = match heading {
                "均衡定义" | "解概念" => heading,
                _ => "判据",
            };
            format!("待写\n\n**起点**\n- 待写\n\n**{heading}**\n待写\n\n**作用域**\n待写\n\n**说明**\n待写")
        }
        Some(FormalRole::Chapter) => {
            "**直觉**\n待写\n\n**本章结果**\n- 待写\n\n**承上启下**\n待写".to_string()
        }
        Some(FormalRole::Result) => {
            let sl = s(&part["source_label"]);
            let source_desc = if !sl.is_empty() { sl } else { "未编号结论".to_string() };
            let lf = s(&part["logical_force"]);
            let lf = if !lf.is_empty() { lf } else { "characterization".to_string() };
            format!(
                "待写\n\n**陈述**\n- 出处与力度：{source_desc}，力度：{lf}。\n- 条件与作用域：待写（假定在模型设定下）。\n待写\n\n**为什么**\n待写\n\n**依据**\n- 原文未证：待写。"
            )
        }
        Some(FormalRole::Derivation) => {
            "**思路**\n待写\n\n**起点**\n- 待写\n\n**步骤**\n待写\n\n**结果**\n待写\n\n**解读**\n待写".to_string()
        }
        Some(FormalRole::Argument) => {
            "目标：待写。\n\n**依据**\n- 待写\n\n**论证**\n待写\n\n**结论**\n待写".to_string()
        }
        Some(FormalRole::Calibration) => {
            "**起点**\n- 待写\n\n**参数与出处**\n| 参数 | 含义 | 取值 | 单位/频率 | 来源类型 | 目标或出处 |\n| --- | --- | --- | --- | --- | --- |\n| 待写 | 待写 | 0 | 无 | assigned | 待写 |\n\n**映射说明**\n待写".to_string()
        }
        Some(FormalRole::Experiment) => {
            "数值实验：待写。\n\n**实验设定**\n- 待写\n\n**数值结果**\n待写\n\n**边界**\n本结果为特定校准参数下的数值结论。".to_string()
        }
        _ => "待写".to_string(),
    }
}

/// Generate a Level 2 formal role skeleton that satisfies `FM1`–`FM6` (proposal §5.4).
pub(crate) fn skeleton(
    part: &Value,
    plan: &Value,
    links: &[String],
    resolve: &dyn Fn(&str) -> Result<String>,
) -> Result<String> {
    let lead = links.join("\n");
    let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
    let body = match role(part) {
        Some(FormalRole::Notation) => {
            let mut out = vec![
                "**怎样读卡片上的记号**".to_string(),
                "待写（说明下标、上标与约定记号的读法）。".to_string(),
                String::new(),
            ];
            let has_borrow = truthy(&part["borrows_notation"]);
            if has_borrow {
                let target = resolve(&s(&part["borrows_notation"]))?;
                out.push(format!("沿用 [[{target}]] 的全部符号。"));
            }
            let sym_rows = list(&plan["symbols"]);
            if !has_borrow || !sym_rows.is_empty() {
                if has_borrow {
                    out.push(String::new());
                }
                out.push("| 符号 | 含义 | 首次出现 |".into());
                out.push("| --- | --- | --- |".into());
                for row in sym_rows {
                    let cells = pipe(row);
                    let sym = super::empirical::table_cell(&s(&ps::at(&cells, 0)));
                    let mean = super::empirical::table_cell(&s(&ps::at(&cells, 1)));
                    let first_raw = if cells.len() > 2 && truthy(cells[2]) {
                        s(cells[2])
                    } else {
                        String::new()
                    };
                    let first = if first_raw.is_empty() {
                        "待写".to_string()
                    } else if let Ok(addr) = resolve(&first_raw) {
                        format!("[[{addr}]]")
                    } else {
                        super::empirical::table_cell(&first_raw)
                    };
                    out.push(format!("| {sym} | {mean} | {first} |"));
                }
            }
            out.push(String::new());
            out.push("**统一用语**".into());
            out.push(String::new());
            let term_rows = list(&plan["terms"]);
            if term_rows.is_empty() {
                out.push("无特殊用语约定。".into());
            } else {
                out.push("| 术语 | 不用 |".into());
                out.push("| --- | --- |".into());
                for row in term_rows {
                    let cells = pipe(row);
                    let term = super::empirical::table_cell(&s(&ps::at(&cells, 0)));
                    let avoid = cells
                        .iter()
                        .skip(1)
                        .map(|v| super::empirical::table_cell(&s(v)))
                        .collect::<Vec<_>>()
                        .join("、");
                    out.push(format!("| {term} | {avoid} |"));
                }
            }
            if !lead.is_empty() {
                out.push(String::new());
                out.push(lead);
            }
            out.join("\n")
        }
        Some(FormalRole::Setting) => {
            let start = if lead.is_empty() {
                "- 原始设定：不依赖其他设定。".to_string()
            } else {
                links.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
            };
            format!("待写\n\n**起点**\n{start}\n\n**设定**\n待写\n\n{}\n\n**说明**\n待写", text::intuition_skeleton())
        }
        Some(FormalRole::Assumption) => {
            let start = if lead.is_empty() {
                "- 待写".to_string()
            } else {
                links.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
            };
            let facet = s(&part["facet"]);
            let scope_sec = if facet.contains("变体") || facet.contains("体制") || truthy(&part["scope"]) {
                format!("\n\n**作用域**\n{}", if truthy(&part["scope"]) { s(&part["scope"]) } else { "待写".to_string() })
            } else {
                String::new()
            };
            format!("待写\n\n**起点**\n{start}\n\n**假设**\n待写{scope_sec}\n\n{}\n\n**说明**\n待写", text::intuition_skeleton())
        }
        Some(FormalRole::Definition) => {
            let start = if lead.is_empty() {
                "- 待写".to_string()
            } else {
                links.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
            };
            format!("待写\n\n**起点**\n{start}\n\n**定义**\n待写\n\n{}\n\n**说明**\n待写", text::intuition_skeleton())
        }
        Some(FormalRole::Criterion) => {
            let facet = s(&part["facet"]);
            let heading = facet
                .find(['：', ':'])
                .map_or(facet.as_str(), |at| &facet[..at])
                .trim();
            let heading = match heading {
                "均衡定义" | "解概念" => heading,
                _ => "判据",
            };
            let start = if lead.is_empty() {
                "- 待写".to_string()
            } else {
                links.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
            };
            let scope = if truthy(&part["scope"]) { s(&part["scope"]) } else { "待写".to_string() };
            format!(
                "待写\n\n**起点**\n{start}\n\n**{heading}**\n1. 候选对象：待写。\n2. 必须满足的条件：待写。\n\n**作用域**\n{scope}\n\n**说明**\n待写"
            )
        }
        Some(FormalRole::Chapter) => {
            let my_id = s(&part["id"]);
            let children: Vec<String> = parts
                .iter()
                .filter(|p| ps::eq(&s(&p["of"]), &my_id))
                .map(|p| Ok(format!("- {} [[{}]]：{}", s(&p["facet"]), resolve(&s(&p["id"]))?, s(&p["title"]))))
                .collect::<Result<Vec<_>>>()?;
            let results = if children.is_empty() {
                "- 待写".to_string()
            } else {
                children.join("\n")
            };
            let intro = if lead.is_empty() { String::new() } else { format!("{lead}\n\n") };
            format!("{intro}**直觉**\n待写\n\n**本章结果**\n{results}\n\n**承上启下**\n待写")
        }
        Some(FormalRole::Result) => {
            let sk = s(&part["source_kind"]);
            let sl = s(&part["source_label"]);
            let lf = s(&part["logical_force"]);
            let source_desc = if !sl.is_empty() {
                sl
            } else if sk == "unnumbered" {
                "未编号结论".to_string()
            } else {
                sk
            };
            let mut cond_links = Vec::new();
            if truthy(&part["criterion"]) {
                let c_addr = resolve(&s(&part["criterion"]))?;
                cond_links.push(format!("依赖 [[{c_addr}]]"));
            }
            for a in pipe(&part["assumptions"]) {
                let aid = s(a);
                if !aid.is_empty() {
                    let a_addr = resolve(&aid)?;
                    cond_links.push(format!("依赖 [[{a_addr}]]"));
                }
            }
            for l in links {
                if !cond_links.iter().any(|c| c.contains(l.as_str())) {
                    cond_links.push(l.clone());
                }
            }
            let cond_line = if cond_links.is_empty() {
                "待写（注明作用域与成立条件）。".to_string()
            } else {
                format!("{}。", cond_links.join("、"))
            };
            let mut just_lines = Vec::new();
            for j in list(&part["justifications"]) {
                let force = s(&j["force"]).to_lowercase();
                let loc = s(&j["location"]).to_lowercase();
                let jref = s(&j["ref"]);
                let extra = s(&j["extra_assumptions"]);
                let extra_note = if extra.is_empty() {
                    String::new()
                } else {
                    format!("（额外假设：{extra}）")
                };
                // Render `force` and `location` independently so an external numerical illustration
                // never becomes `外部证明` and a local constructive argument never becomes `局部证明` (Astra #4).
                let force_zh = match force.as_str() {
                    "proof" => "证明",
                    "constructive" => "构造",
                    "diagrammatic" => "图示论证",
                    "heuristic" => "启发式推导",
                    "illustration" => "例示",
                    "numerical" => "数值实验",
                    "conjectural" => "猜想",
                    "omitted" => "原文未证",
                    _ => "依据",
                };
                if force == "omitted" {
                    let note = if jref.is_empty() { "原文未给出证明".to_string() } else { jref };
                    just_lines.push(format!("- 原文未证：{note}{extra_note}。"));
                } else {
                    match loc.as_str() {
                        "local" => {
                            let label = if force == "heuristic" {
                                "启发式推导".to_string()
                            } else {
                                format!("局部{force_zh}")
                            };
                            if jref.starts_with("inline:") || jref.starts_with("§") {
                                let inline_text = jref.strip_prefix("inline:").unwrap_or(&jref);
                                just_lines.push(format!("- {label}（行内）：见 {inline_text}{extra_note}。"));
                            } else {
                                let target = resolve(&jref).unwrap_or(jref);
                                just_lines.push(format!("- {label}：见 [[{target}]]{extra_note}。"));
                            }
                        }
                        "external" => {
                            let label = format!("外部{force_zh}");
                            just_lines.push(format!("- {label}：见 {jref}{extra_note}。"));
                        }
                        "supplied" => {
                            let label = format!("补写{force_zh}");
                            just_lines.push(format!("- {label}：{jref}{extra_note}（补：书中略去）。"));
                        }
                        _ => {
                            just_lines.push(format!("- {force_zh}：{jref}{extra_note}。"));
                        }
                    }
                }
            }
            if just_lines.is_empty() {
                just_lines.push("- 待写".to_string());
            }
            format!(
                "待写\n\n**陈述**\n- 出处与力度：{source_desc}，力度：{lf}。\n- 条件与作用域：{cond_line}\n待写（原样陈述结论与公式）。\n\n**为什么**\n待写\n\n**依据**\n{}",
                just_lines.join("\n")
            )
        }
        Some(FormalRole::Argument) => {
            let basis = if lead.is_empty() {
                "- 待写".to_string()
            } else {
                links.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
            };
            format!("目标：待写（一句话说明本论证要确立的结论，不作前提）。\n\n**依据**\n{basis}\n\n**论证**\n待写\n\n**结论**\n待写")
        }
        Some(FormalRole::Calibration) => {
            let start = if lead.is_empty() {
                "- 待写".to_string()
            } else {
                links.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
            };
            format!(
                "**起点**\n{start}\n\n**参数与出处**\n| 参数 | 含义 | 取值 | 单位/频率 | 来源类型 | 目标或出处 |\n| --- | --- | --- | --- | --- | --- |\n| 待写 | 待写 | 待写 | 待写 | assigned | 待写 |\n\n**映射说明**\n待写"
            )
        }
        Some(FormalRole::Experiment) => {
            let mut setup_lines: Vec<String> = Vec::new();
            if truthy(&part["calibration"]) {
                let cal_addr = resolve(&s(&part["calibration"]))?;
                setup_lines.push(format!("- 校准：依赖 [[{cal_addr}]]"));
            }
            for l in links {
                if !setup_lines.iter().any(|x| x.contains(l.as_str())) {
                    setup_lines.push(format!("- {l}"));
                }
            }
            let setup = if setup_lines.is_empty() {
                "- 基准：待写。".to_string()
            } else {
                setup_lines.join("\n")
            };
            format!(
                "数值实验：待写（明确标示为数值结果）。\n\n**实验设定**\n{setup}\n\n**数值结果**\n待写\n\n**边界**\n本结果为特定校准参数下的数值结论。"
            )
        }
        _ => format!("{lead}\n\n待写"),
    };
    Ok(body.trim().to_string())
}

/// `setting.md` of a Level 2 formal model: claim objects, tags, notation table, term table, and
/// pinned inventory summary.
pub(crate) fn setting_text(
    plan: &Value,
    model: &str,
    resolve: &dyn Fn(&str) -> Result<String>,
    inv: Option<&Value>,
) -> Result<String> {
    let claim_objects: Vec<String> = pipe(&plan["claim_objects"]).into_iter().map(s).collect();
    let family_tags: Vec<String> = pipe(&plan["family_tags"]).into_iter().map(s).collect();
    let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
    let setting = parts.iter().find(|p| ps::eq(&line_of(p), "main"));
    let mut out = format!(
        "# 本模型（形式模型）：[[{model}]]\n\n- 目标对象：{}\n- 标签：{}\n- 模型卡：{}\n",
        claim_objects.join("；"),
        if family_tags.is_empty() { "无".to_string() } else { family_tags.join("、") },
        s(&plan["card"]),
    );
    if let Some(st) = setting {
        out += &format!(
            "- 设定根卡：[[{}]] {}：{}\n",
            resolve(&s(&st["id"]))?,
            s(&st["title"]),
            s(&st["spec"])
        );
    }
    out += "\n";
    out += &text::tables(plan);
    out += "\n";
    if let Some(inv) = inv {
        let eqs = list(&inv["equations"]);
        let items = list(&inv["items"]);
        out += &format!(
            "\n## 源文本清单摘要（inventory.json：{} 条展示公式，{} 条编号条目）\n",
            eqs.len(),
            items.len()
        );
        for item in items.iter().take(40) {
            out += &format!(
                "- [{}] {}（{}:{}-{}）\n",
                s(&item["id"]),
                s(&item["label"]),
                s(&item["file"]),
                s(&item["start_line"]),
                s(&item["end_line"])
            );
        }
    }
    Ok(out)
}

/// Build-report Layer 1 formal-model faults formatted as `## 修正：` notes for `step_close`.
pub(crate) fn role_entries(model: &str, build: &Value) -> Vec<(String, String)> {
    let keep = "不要增删其余内容，不要移动卡片。改完照常运行 CHECK。";
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in [
        "formal_shape",
        "setting_dependency",
        "criterion_unbound",
        "result_unbound",
        "proof_unbound",
        "experiment_unbound",
        "symbol_uncovered",
    ] {
        for f in list(&build[name]) {
            let at = s(&f["address"]);
            let fix = s(&f["fix"]);
            if !at.is_empty() && !fix.is_empty() {
                out.entry(at).or_default().push(fix);
            }
        }
    }
    out.into_iter()
        .map(|(at, fixes)| {
            (
                at.clone(),
                format!(
                    "## 修正：[[{at}]]\n\n构建检查（形式模型 [[{model}]]）：\n{}\n{keep}\n",
                    fixes
                        .iter()
                        .map(|f| format!("- {f}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            )
        })
        .collect()
}

/// Close-round `FM1`–`FM6` card-format checks and pinned `inventory.json` equation fidelity checks
/// for a Level 2 formal model. Each card tuple is `(address, facet, body, src_files)`.
pub(crate) fn formal_entries_scoped(
    model: &str,
    head_body: &str,
    cards: &[(String, String, String, Vec<String>)],
    inv: Option<&Value>,
) -> Vec<(String, String)> {
    let keep = "不要增删其余内容，不要移动卡片。改完照常运行 CHECK。";
    let mut out = Vec::new();

    // Check head with FM1.
    let head_entry = json!({
        "address": model,
        "type": "model",
        "category": "model",
        "facet": null,
        "level": "formal",
        "has_parts": true,
        "title": "",
        "body": head_body,
    });
    let head_fm: Vec<String> = zt_check::core::Check::new(zt_check::core::Limits::default(), false, "")
        .run(&[head_entry])
        .into_iter()
        .filter(|line| line.contains(" FM"))
        .collect();
    if !head_fm.is_empty() {
        out.push((
            model.to_string(),
            format!(
                "## 修正：[[{model}]]\n\n形式模型卡格式检查（FM1，模型 [[{model}]]）：\n{}\n{keep}\n",
                head_fm
                    .iter()
                    .map(|l| format!("- {}", l.split_once(' ').map(|(_, r)| r).unwrap_or(l)))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        ));
    }

    for (address, facet, body, src_files) in cards {
        let mut lines = Vec::new();
        let entry = json!({
            "address": address,
            "type": "model",
            "category": "part",
            "facet": facet,
            "level": "formal",
            "title": "",
            "body": body,
        });
        let fm: Vec<String> = zt_check::core::Check::new(zt_check::core::Limits::default(), false, "")
            .run(&[entry])
            .into_iter()
            .filter(|line| line.contains(" FM"))
            .collect();
        for line in fm {
            let text = line.split_once(' ').map(|(_, rest)| rest).unwrap_or(&line);
            lines.push(format!("卡片格式：{text}"));
        }
        if let Some(inv) = inv {
            for mismatch in inventory::card_printed_mismatches_scoped(body, inv, src_files) {
                lines.push(format!("公式核对：{mismatch}"));
            }
        }
        if !lines.is_empty() {
            out.push((
                address.clone(),
                format!(
                    "## 修正：[[{address}]]\n\n形式检查（模型 [[{model}]]）：\n{}\n{keep}\n",
                    lines
                        .iter()
                        .map(|l| format!("- {l}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            ));
        }
    }
    out
}

#[cfg(test)]
pub(crate) fn formal_entries(
    model: &str,
    head_body: &str,
    cards: &[(String, String, String)],
    inv: Option<&Value>,
) -> Vec<(String, String)> {
    let scoped: Vec<(String, String, String, Vec<String>)> = cards
        .iter()
        .map(|(a, f, b)| (a.clone(), f.clone(), b.clone(), Vec::new()))
        .collect();
    formal_entries_scoped(model, head_body, &scoped, inv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arro_plan() -> Value {
        json!({
            "model": "K/3|e",
            "level": "formal",
            "claim_objects": ["market-equilibrium", "dynamics-limit"],
            "family_tags": ["open-economy-dsge"],
            "title": "两国便利收益外逃模型",
            "card": "问题：便利收益外逃如何传导。",
            "symbols": [["$\\nu$", "稳态便利收益份额", "S0"]],
            "terms": [["便利收益", "流动性溢价"]],
            "parts": [
                {"id": "S0", "line": "main", "facet": "设定", "title": "设定：两国三资产", "spec": "环境", "src": ["sections/s02.md:1-10"]},
                {"id": "N", "line": "side", "of": "S0", "facet": "符号表与读法", "title": "符号表与读法", "spec": "符号", "src": ["sections/s02.md:1-10"]},
                {"id": "EQ", "line": "side", "of": "S0", "facet": "均衡定义：两国竞争均衡", "title": "均衡定义", "spec": "均衡", "src": ["sections/s02.md:1-10"]},
                {"id": "C1", "line": "main", "facet": "第 1 章：区制依赖", "title": "第 1 章：区制依赖", "spec": "区制", "rests": [["EQ", "在均衡下"]], "src": ["sections/s02.md:1-10"]},
                {"id": "R1", "line": "side", "of": "C1", "facet": "结果：命题 1 区制依赖", "emphasis": "core",
                 "title": "结果：命题 1", "spec": "命题 1",
                 "source_kind": "proposition", "source_label": "Proposition 1", "logical_force": "characterization",
                 "justifications": [{"force": "proof", "location": "local", "ref": "D1"}],
                 "src": ["sections/s02.md:1-10"]},
                {"id": "D1", "line": "side", "of": "R1", "derive": true, "facet": "推导：便利收益楔子", "title": "推导一", "rests": [["EQ", "由均衡条件出发"]], "to": "$$y=a$$", "steps": 4, "src": ["sections/s02.md:1-10"]},
                {"id": "W2", "line": "side", "after": "D1", "facet": "论证：泰勒原理排除爆炸解", "title": "论证二", "spec": "排除爆炸解", "rests": [["D1", "由第一步"]], "src": ["sections/s02.md:1-10"]},
                {"id": "D3", "line": "side", "after": "W2", "derive": true, "facet": "推导：国内块缩放", "title": "推导三", "rests": [["W2", "排除爆炸解后"]], "to": "$$\\pi=\\nu x$$", "steps": 4, "src": ["sections/s02.md:1-10"]}
            ]
        })
    }

    #[test]
    fn mixed_derivation_argument_route_splits_into_two_maximal_algebra_runs() {
        let p = arro_plan();
        assert_eq!(plan_faults(&p, None, None), Vec::<String>::new());
        let chains = plan_derivation_chains(&p);
        assert_eq!(chains, vec![vec!["D1".to_string()], vec!["D3".to_string()]]);
        let order = side_placement_order(&list(&p["parts"]).into_iter().cloned().collect::<Vec<_>>()).unwrap();
        let ids: Vec<String> = order.into_iter().map(|i| s(&p["parts"][i]["id"])).collect();
        let pos = |name: &str| ids.iter().position(|x| x == name).unwrap();
        assert!(pos("R1") < pos("D1") && pos("D1") < pos("W2") && pos("W2") < pos("D3"));
    }

    #[test]
    fn placement_invariants_reject_both_of_and_after_duplicate_after_and_cycles() {
        let mut p = arro_plan();
        // Both of and after
        p["parts"][6]["of"] = json!("R1");
        // Duplicate after
        p["parts"].as_array_mut().unwrap().push(json!({
            "id": "W2_dup", "line": "side", "after": "D1", "facet": "论证：冲突", "title": "冲突", "spec": "x"
        }));
        let faults = plan_faults(&p, None, None).join("\n");
        assert!(faults.contains("exactly one of `of` or `after`"), "{faults}");
        assert!(faults.contains("duplicate `after` target `D1`"), "{faults}");
    }

    #[test]
    fn spence_1973_unnumbered_constructive_argument_and_negative_control() {
        let spence = json!({
            "model": "spence1973/2|a",
            "level": "formal",
            "claim_objects": ["market-equilibrium"],
            "family_tags": ["signaling"],
            "symbols": [["$y$", "教育信号", "S0"], ["$c$", "单位信号成本", "S0"]],
            "terms": [["信号均衡", "筛选均衡"]],
            "parts": [
                {"id": "S0", "line": "main", "facet": "设定", "title": "设定：两类工人与雇主", "spec": "环境", "src": ["s01.md:1-10"]},
                {"id": "N", "line": "side", "of": "S0", "facet": "符号表与读法", "title": "符号表与读法", "src": ["s01.md:1-10"]},
                {"id": "A", "line": "side", "of": "S0", "facet": "假设：单交叉成本条件", "title": "假设：高能力边际成本更低", "spec": "单交叉", "rests": [["S0", "基于设定"]], "src": ["s01.md:1-10"]},
                {"id": "EQ", "line": "side", "of": "S0", "facet": "均衡定义：自证信念均衡", "title": "均衡定义：信号均衡", "spec": "信念自证", "rests": [["S0", "基于设定"]], "src": ["s01.md:1-10"]},
                {"id": "C1", "line": "main", "facet": "第 1 章：分离均衡与无限多重性", "title": "第 1 章：分离均衡存在且不唯一", "spec": "分离均衡", "rests": [["EQ", "按均衡定义"]], "src": ["s01.md:1-10"]},
                {"id": "BM", "line": "side", "of": "C1", "facet": "基准：完全信息基准", "title": "基准：无教育信号时的工资", "spec": "基准", "rests": [["S0", "基准"]], "src": ["s01.md:1-10"]},
                {"id": "R1", "line": "side", "of": "C1", "facet": "结果：分离均衡临界区间", "emphasis": "core", "title": "结果：存在连续统多个分离均衡", "spec": "临界区间",
                 "source_kind": "unnumbered", "logical_force": "existence", "criterion": "EQ", "assumptions": ["A"],
                 "justifications": [{"force": "constructive", "location": "local", "ref": "W1"}],
                 "rests": [["EQ", "按均衡定义"], ["A", "单交叉"]], "src": ["s01.md:1-10"]},
                {"id": "W1", "line": "side", "of": "R1", "facet": "论证：阶梯工资表构造分离均衡", "title": "论证：构造临界值 y*", "spec": "构造", "rests": [["EQ", "验证自证"], ["A", "比较成本"]], "src": ["s01.md:1-10"]},
                {"id": "BD", "line": "side", "of": "R1", "facet": "边界：端点不取等与帕累托排序", "title": "边界：区间端点与无效率", "spec": "边界", "rests": [["R1", "说明边界"]], "src": ["s01.md:1-10"]}
            ]
        });
        assert_eq!(plan_faults(&spence, None, None), Vec::<String>::new());
        // Negative control: proof step resting circularly on its own Result is rejected.
        let mut bad = spence.clone();
        bad["parts"][7]["rests"] = json!([["R1", "循环依赖"]]);
        let faults = plan_faults(&bad, None, None);
        assert!(faults.iter().any(|f| f.contains("W1") && f.contains("R1")), "{faults:?}");
    }

    #[test]
    fn mirrlees_1971_dual_justifications_assumption_b_and_calibration_audit() {
        let mirrlees = json!({
            "model": "mirrlees1971/2|a",
            "level": "formal",
            "claim_objects": ["choice-optimum"],
            "family_tags": ["optimal-taxation", "mechanism-design"],
            "symbols": [["$u$", "效用", "S0"], ["$n$", "技能", "S0"]],
            "terms": [["边际税率", "税率斜率"]],
            "parts": [
                {"id": "S0", "line": "main", "facet": "设定", "title": "设定：连续技能经济", "spec": "环境", "src": ["s02.md:1-20"]},
                {"id": "N", "line": "side", "of": "S0", "facet": "符号表与读法", "title": "符号表与读法", "src": ["s02.md:1-20"]},
                {"id": "AB", "line": "side", "of": "S0", "facet": "假设 (B)：消费严格正且效用单调", "title": "假设 (B)：消费与劳动正则性", "spec": "正则性", "rests": [["S0", "设定"]], "src": ["s02.md:1-20"]},
                {"id": "CR", "line": "side", "of": "S0", "facet": "判据：社会福利最优分配", "title": "判据：计划者最优分配", "spec": "最优判据", "rests": [["S0", "设定"]], "src": ["s02.md:1-20"]},
                {"id": "C1", "line": "main", "facet": "第 1 章：最优税率存在性与一阶条件", "title": "第 1 章：存在性与必要条件", "spec": "主结果", "rests": [["CR", "按判据"]], "src": ["s02.md:1-20"]},
                {"id": "T1", "line": "side", "of": "C1", "facet": "结果：定理 1 最优分配存在性", "emphasis": "core", "title": "结果：定理 1", "spec": "存在性",
                 "source_kind": "theorem", "source_label": "Theorem 1", "logical_force": "existence", "criterion": "CR", "assumptions": ["AB"],
                 "justifications": [
                     {"force": "proof", "location": "external", "ref": "Mirrlees [4]"},
                     {"force": "heuristic", "location": "local", "ref": "D1", "extra_assumptions": "u 连续可微"}
                 ],
                 "rests": [["CR", "判据"], ["AB", "假设 (B)"]], "src": ["s02.md:1-20"]},
                {"id": "D1", "line": "side", "of": "T1", "derive": true, "facet": "推导：光滑情形启发式一阶条件", "title": "推导：启发式导数", "rests": [["CR", "由计划者目标"]], "to": "$$u_x = \\lambda$$", "steps": 3, "src": ["s02.md:1-20"]},
                {"id": "CAL", "line": "side", "of": "C1", "facet": "校准：对数正态技能分布", "title": "校准：技能参数赋值", "spec": "校准", "rests": [["S0", "参数"]], "src": ["s02.md:1-20"]},
                {"id": "EXP", "line": "side", "of": "C1", "facet": "数值实验：最优边际税率曲线", "title": "数值实验：边际税率", "spec": "数值表", "rests": [["CAL", "使用校准"], ["T1", "解一阶条件"]], "src": ["s02.md:1-20"]}
            ]
        });
        assert_eq!(plan_faults(&mirrlees, None, None), Vec::<String>::new());

        // Negative controls: numbered Result missing source_label; formal_entries catches FM2/FM5.
        let mut missing_label = mirrlees.clone();
        missing_label["parts"][5]["source_label"] = json!("");
        let pf = plan_faults(&missing_label, None, None);
        assert!(pf.iter().any(|f| f.contains("source_label")), "{pf:?}");

        let bad_cards = vec![
            (
                "mirrlees1971/2|a|a|1|a".to_string(),
                "结果：定理 1".to_string(),
                "**陈述**\n存在最优解。\n\n**为什么**\n紧性。\n\n**依据**\n无。".to_string(),
            ),
            (
                "mirrlees1971/2|a|a|1|b".to_string(),
                "校准：参数".to_string(),
                "**起点**\n设定。\n\n**参数与出处**\n| 参数 | 含义 | 取值 | 单位 | 来源类型 | 出处 |\n| --- | --- | --- | --- | --- | --- |\n| $\\sigma$ | 标准差 | 0.39 | 无 | guessed | 自己猜的 |".to_string(),
            ),
        ];
        let notes = formal_entries(
            "mirrlees1971/2|a",
            "目标。\n\n**目标对象与标签**\n- 对象：choice-optimum\n\n**原语与判据**\n计划者。\n\n**主要结果**\n见下。",
            &bad_cards,
            None,
        );
        assert!(notes.iter().any(|(_, t)| t.contains("FM3")), "{notes:?}");
        assert!(notes.iter().any(|(_, t)| t.contains("FM6")), "{notes:?}");
    }

    #[test]
    fn ozhan_chs_inventory_catches_unsettled_obligation_and_ownership_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let sec = dir.path().join("sections");
        std::fs::create_dir_all(&sec).unwrap();
        let text = "\
# 3. Model
## 3.1 Theorem
Theorem 1. Identification holds.
$$y_t = \\alpha + \\beta x_t \\tag{1}$$

## 3.4 Unnumbered Identification Derivation
In this section we derive the moment restriction:
$$E[z_t (y_t - \\beta x_t)] = 0 \\tag{2}$$
";
        std::fs::write(sec.join("s03.md"), text).unwrap();
        let mut inv = inventory::build_inventory(
            "chs2010/3|a",
            None,
            None,
            None,
            &[("sections/s03.md".to_string(), text.to_string())],
        );
        // Add §3.4 unnumbered obligation as an accepted obligation in inventory.json (proposal §8.2).
        inv["obligations"] = json!([
            {"id": "ob:s03:3.4", "label": "§3.4 Unnumbered Identification Derivation", "accepted": true}
        ]);
        let eq_ids: Vec<String> = list(&inv["equations"]).into_iter().map(|e| s(&e["id"])).collect();
        assert_eq!(eq_ids.len(), 2, "{inv}");

        let base_plan = |p1_refs: Value, p2_refs: Value| {
            json!({
                "model": "chs2010/3|a",
                "level": "formal",
                "claim_objects": ["identification", "choice-optimum"],
                "parts": [
                    {"id": "S0", "line": "main", "facet": "设定", "title": "设定", "spec": "环境", "src": ["sections/s03.md:1-5"]},
                    {"id": "N", "line": "side", "of": "S0", "facet": "符号表与读法", "title": "符号表与读法", "src": ["sections/s03.md:1-5"]},
                    {"id": "ID", "line": "side", "of": "S0", "facet": "判据：非参数识别", "title": "判据：识别", "spec": "识别判据", "rests": [["S0", "设定"]], "src": ["sections/s03.md:1-5"]},
                    {"id": "C1", "line": "main", "facet": "第 1 章：识别", "title": "第 1 章：识别", "spec": "识别", "rests": [["ID", "判据"]], "src": ["sections/s03.md:1-8"]},
                    {"id": "R1", "line": "side", "of": "C1", "facet": "结果：定理 1", "emphasis": "core", "title": "结果：定理 1", "spec": "定理 1",
                     "source_kind": "theorem", "source_label": "Theorem 1", "logical_force": "identification",
                     "justifications": [{"force": "proof", "location": "local", "ref": "D1"}],
                     "inventory_refs": p1_refs, "rests": [["ID", "判据"]], "src": ["sections/s03.md:2-5"]},
                    {"id": "D1", "line": "side", "of": "R1", "derive": true, "facet": "推导：矩条件", "title": "推导：矩条件", "rests": [["ID", "出发"]], "to": "$$E[z_t (y_t - \\beta x_t)] = 0$$", "steps": 3,
                     "inventory_refs": p2_refs, "src": ["sections/s03.md:6-8"]}
                ]
            })
        };

        // 1. Omitting the §3.4 unnumbered obligation triggers obligation_unsettled.
        let omitted = base_plan(json!([eq_ids[0]]), json!([eq_ids[1]]));
        let f1 = plan_faults(&omitted, Some(&inv), Some(dir.path().to_str().unwrap()));
        assert!(f1.iter().any(|f| f.contains("obligation_unsettled") && f.contains("ob:s03:3.4")), "{f1:?}");

        // 2. Two parts claiming the same eq1 triggers ownership_conflict.
        let conflict = base_plan(json!([eq_ids[0]]), json!([eq_ids[0], eq_ids[1], "ob:s03:3.4"]));
        let f2 = plan_faults(&conflict, Some(&inv), Some(dir.path().to_str().unwrap()));
        assert!(f2.iter().any(|f| f.contains("ownership_conflict") && f.contains(&eq_ids[0])), "{f2:?}");

        // 3. Settling all inventory items and accepted obligations cleanly passes with 0 faults.
        let clean = base_plan(json!([eq_ids[0]]), json!([eq_ids[1], "ob:s03:3.4"]));
        let f3 = plan_faults(&clean, Some(&inv), Some(dir.path().to_str().unwrap()));
        assert_eq!(f3, Vec::<String>::new());
    }

    #[test]
    fn astra_1_4_14_placement_relations_honest_justifications_and_side_forward_detection() {
        // Astra #1: plain `of` is placement-only in Level 2 formal models (never requires linking R1 from D1).
        let d1 = json!({"id": "D1", "line": "side", "of": "R1", "rests": [["EQ", "均衡"]]});
        assert_eq!(planned_relations(&d1, true), vec![("EQ".to_string(), "均衡".to_string())]);
        assert_eq!(planned_relations(&d1, false).len(), 2);

        // Astra #4: external numerical renders as `外部数值实验` (never `外部证明`); inline local accepted.
        let res_part = json!({
            "id": "R1", "line": "side", "of": "C1", "facet": "结果：命题 1",
            "source_kind": "proposition", "source_label": "Proposition 1", "logical_force": "numerical-finding",
            "justifications": [
                {"force": "numerical", "location": "external", "ref": "Online Appendix Table 2"},
                {"force": "constructive", "location": "local", "ref": "inline:§3.2 构造"}
            ]
        });
        let skel = skeleton(&res_part, &json!({"parts": []}), &[], &|id| Ok(id.to_string())).unwrap();
        assert!(skel.contains("外部数值实验：见 Online Appendix Table 2"), "{skel}");
        assert!(!skel.contains("外部证明"), "{skel}");
        assert!(skel.contains("局部构造（行内）：见 §3.2 构造"), "{skel}");

        // Astra #14: Side proof step resting on a later step in the same Side branch is rejected.
        let mut p = arro_plan();
        p["parts"][5]["rests"] = json!([["D3", "前向依赖后一步"]]);
        let faults = plan_faults(&p, None, None).join("\n");
        assert!(faults.contains("forward:") && faults.contains("D1") && faults.contains("D3"), "{faults}");
    }

    #[test]
    fn astra_c1_and_h12_initial_skeletons_pass_fm_and_role_targets_are_validated() {
        // C1: every role's initial_skeleton passes zt-check FM2-FM6 AND still flags as placeholder/待写.
        let plan = arro_plan();
        for p in list(&plan["parts"]) {
            let body = initial_skeleton(p, &plan);
            assert!(body.contains("待写"), "initial_skeleton must stay detectable as unwritten: {body}");
            let entry = json!({
                "address": "K/3|e|a",
                "type": "model",
                "category": "part",
                "facet": p["facet"],
                "level": "formal",
                "title": p["title"],
                "body": body,
            });
            let fm: Vec<String> = zt_check::core::Check::new(zt_check::core::Limits::default(), false, "")
                .run(&[entry])
                .into_iter()
                .filter(|line| line.contains(" FM"))
                .collect();
            assert!(fm.is_empty(), "initial_skeleton for {} must pass FM2-FM6 under Stage::Enforce: {fm:?}", s(&p["facet"]));
        }

        // H12: Result `of` must be Chapter; criterion must be Criterion; calibration must exist & be emitted.
        let mut bad = arro_plan();
        bad["parts"][4]["of"] = json!("S0"); // Setting, not Chapter
        bad["parts"][4]["criterion"] = json!("S0"); // Setting, not Criterion
        let faults = plan_faults(&bad, None, None).join("\n");
        assert!(faults.contains("must be a chapter"), "{faults}");
        assert!(faults.contains("must name a `判据`"), "{faults}");

        let exp = json!({"id": "EXP", "facet": "数值实验：敏感性", "calibration": "CAL"});
        let skel = skeleton(&exp, &json!({"parts": []}), &[], &|id| Ok(format!("K/1|{id}"))).unwrap();
        assert!(skel.contains("[[K/1|CAL]]"), "Experiment skeleton must emit calibration link: {skel}");
    }
}
