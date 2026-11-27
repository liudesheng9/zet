//! `zt-digest model-explain -Step plan|apply|finish -Model <head> [-Work <dir>] [-Force]`:
//!
//! Single-model Derivation explanation refresh (post-digest Stage 4 step 8),
//! internalizing `skills/zt-model-import/scripts/explain.mjs`.

use crate::args::Args;
use crate::lockcheck;
use crate::model;
use crate::term_link;
use crate::util;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const WORK_MARKER: &str = ".zt-model-explain-work";
const STEPS: [&str; 3] = ["plan", "apply", "finish"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveId {
    pub id: String,
    pub path: String,
}

/// Validates that `-Model` names exactly one address (no commas, semicolons, or whitespace).
pub fn validate_single_head(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.contains(',')
        || trimmed.contains(';')
        || trimmed.chars().any(char::is_whitespace)
        || !trimmed.contains('/')
    {
        bail!(
            "-Model must name a single Model Card address (e.g. Key/4|a), not `{raw}`"
        );
    }
    Ok(trimmed.to_string())
}

/// The book Citation key of a Model Card address.
pub fn key_of(head: &str) -> &str {
    head.split('/').next().unwrap_or(head)
}

/// Default work folder for a single-model explain refresh.
/// Resolves any relative `-Work` path against the current working directory once so saved job
/// paths remain valid when `apply`/`finish` is resumed from another working directory (M3).
pub fn work_of(head: &str, given: Option<&str>) -> PathBuf {
    if let Some(dir) = given.filter(|s| !s.trim().is_empty()) {
        let p = PathBuf::from(dir.trim());
        if p.is_absolute() {
            return p;
        }
        return std::env::current_dir()
            .map(|cwd| cwd.join(&p))
            .unwrap_or(p);
    }
    let mut slug = String::new();
    let mut prev_dash = false;
    for ch in head.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    std::env::temp_dir().join("zt-model-explain").join(slug)
}

/// Canonical assignment extractor for one saved explain job (H6): `compiler.rs` reads `addresses`,
/// so `addresses` must be a non-empty array of non-empty strings. If legacy `parts` is also present
/// and non-null, it must agree with `addresses` (no silent conflict between validation, settlement,
/// relinking, and apply).
fn job_assigned_addresses(job_obj: &serde_json::Map<String, Value>, id: &str) -> Result<Vec<String>> {
    let arr = job_obj
        .get("addresses")
        .and_then(Value::as_array)
        .with_context(|| format!("saved explain job `{id}` lacks an `addresses` array"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(addr) = item.as_str().filter(|s| !s.is_empty()) else {
            bail!("saved explain job `{id}` field `addresses` contains an invalid address");
        };
        out.push(addr.to_string());
    }
    if out.is_empty() {
        bail!("saved explain job `{id}` has no assigned addresses");
    }
    if let Some(parts_val) = job_obj.get("parts")
        && !parts_val.is_null()
    {
        let Some(parts_arr) = parts_val.as_array() else {
            bail!("saved explain job `{id}` field `parts` must be an array");
        };
        let mut parts_set: HashSet<&str> = HashSet::new();
        for item in parts_arr {
            let Some(addr) = item.as_str().filter(|s| !s.is_empty()) else {
                bail!("saved explain job `{id}` field `parts` contains an invalid address");
            };
            parts_set.insert(addr);
        }
        let addr_set: HashSet<&str> = out.iter().map(String::as_str).collect();
        if parts_set != addr_set {
            bail!("saved explain job `{id}` has conflicting `addresses` and `parts`");
        }
    }
    Ok(out)
}

/// The parts the explain jobs wrote: every job's `done` address that belongs to that job's own
/// assignment (`addresses`) and was neither refused nor blocked (H6).
pub fn changed_parts(state: &Value) -> Vec<String> {
    let ex = &state["explain"];
    let mut excluded: HashSet<String> = HashSet::new();
    for field in ["refused", "blocked"] {
        if let Some(map) = ex[field].as_object() {
            for texts in map.values() {
                for t in texts.as_array().into_iter().flatten() {
                    if let Some(text) = t.as_str() {
                        let addr = text.split(": ").next().unwrap_or(text);
                        excluded.insert(addr.to_string());
                    }
                }
            }
        }
    }
    let mut out: Vec<String> = Vec::new();
    if let Some(jobs) = ex["jobs"].as_object() {
        for job in jobs.values() {
            let assigned: Option<HashSet<&str>> = job
                .get("addresses")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).collect());
            for a in job["done"].as_array().into_iter().flatten() {
                let Some(addr) = a.as_str() else {
                    continue;
                };
                if let Some(ref set) = assigned
                    && !set.contains(addr)
                {
                    continue;
                }
                if !excluded.contains(addr) && !out.iter().any(|seen| seen == addr) {
                    out.push(addr.to_string());
                }
            }
        }
    }
    out
}

/// Resolves a work directory to its canonical physical path (collapsing `.`/`..` and resolving
/// directory junctions/symlinks via the deepest existing ancestor) so a junction and its target
/// always derive the exact same lock file (H-new).
fn resolve_lock_target(work: &Path) -> Result<PathBuf> {
    let raw_abs = if work.is_absolute() {
        work.to_path_buf()
    } else {
        std::env::current_dir()
            .context("failed to read current working directory for work lock")?
            .join(work)
    };
    let mut abs = PathBuf::new();
    for comp in raw_abs.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                abs.pop();
            }
            other => abs.push(other.as_os_str()),
        }
    }
    if let Ok(c) = std::fs::canonicalize(&abs) {
        return Ok(c);
    }
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = abs.as_path();
    let mut base = None;
    loop {
        if let Ok(c) = std::fs::canonicalize(cur) {
            base = Some(c);
            break;
        }
        let Some(name) = cur.file_name() else {
            break;
        };
        tail.push(name.to_os_string());
        let Some(parent) = cur.parent() else {
            break;
        };
        cur = parent;
    }
    let mut resolved = base.unwrap_or(abs);
    for seg in tail.into_iter().rev() {
        resolved.push(seg);
    }
    Ok(resolved)
}

/// Exclusive OS-backed lock on a sibling lock file derived from the resolved physical work
/// directory so concurrent `plan`/`apply`/`finish` invocations (including through junctions or
/// alternate path spellings) cannot race reset against apply (H-new).
pub struct WorkLock {
    _file: std::fs::File,
}

impl WorkLock {
    pub fn lock_path_for(work: &Path) -> Result<PathBuf> {
        let resolved = resolve_lock_target(work)?;
        let parent = resolved
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = resolved
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "work".to_string());
        Ok(parent.join(format!(".{name}.zt-model-explain.lock")))
    }

    pub fn acquire(work: &Path) -> Result<Self> {
        let lock_path = Self::lock_path_for(work)?;
        if let Some(parent) = lock_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create parent directory {}", parent.display()))?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&lock_path)
            .with_context(|| format!("failed to open work lock {}", lock_path.display()))?;
        file.try_lock().map_err(|_| {
            anyhow::anyhow!(
                "work directory {} is currently locked by another model-explain process ({})",
                work.display(),
                lock_path.display()
            )
        })?;
        Ok(Self { _file: file })
    }
}

/// A job is unfinished if `explain` or `explain.jobs` is missing/null/malformed, if any job entry
/// is malformed (missing `model`, non-string/null entries in `addresses`/`parts`/`done`, a
/// `done`-only job with no assigned addresses, or any `done` entry outside the job's assignment),
/// if `done` does not equal the job's assignment set, or if `finish` has not recorded
/// `finished_ok: true`. Only an empty `jobs: {}` object (a clean 0-part plan) or structurally
/// valid settled jobs alongside `finished_ok: true` is safe to reset without `-Force` (H6).
pub fn has_unfinished_state(state: &Value) -> bool {
    let Some(ex_obj) = state.get("explain").and_then(Value::as_object) else {
        return true;
    };
    if let Some(models_val) = ex_obj.get("models")
        && !models_val.is_null()
    {
        let Some(models) = models_val.as_array() else {
            return true;
        };
        if models.iter().any(|m| m.as_str().is_none_or(str::is_empty)) {
            return true;
        }
    }
    let Some(jobs) = ex_obj.get("jobs").and_then(Value::as_object) else {
        return true;
    };
    if jobs.is_empty() {
        return false;
    }
    let finished_ok = ex_obj.get("finished_ok").and_then(Value::as_bool) == Some(true);
    for (id, job) in jobs {
        let Some(job_obj) = job.as_object() else {
            return true;
        };
        if job_obj.get("model").and_then(Value::as_str).is_none_or(str::is_empty) {
            return true;
        }
        let Ok(assigned_vec) = job_assigned_addresses(job_obj, id) else {
            return true;
        };
        let assigned: HashSet<&str> = assigned_vec.iter().map(String::as_str).collect();
        let mut done: HashSet<&str> = HashSet::new();
        match job_obj.get("done") {
            Some(Value::Array(arr)) => {
                for item in arr {
                    let Some(s) = item.as_str().filter(|s| !s.is_empty()) else {
                        return true;
                    };
                    done.insert(s);
                }
            }
            None | Some(Value::Null) => {}
            Some(_) => return true,
        }
        if done != assigned {
            return true;
        }
    }
    !finished_ok
}

/// Verifies every saved job in `state.json` carries `model == head`, has a non-empty assignment
/// whose addresses belong to `head`'s live filed Derivation set (when supplied), and has every
/// `done` address inside that job's own assignment (H6).
pub fn assert_state_matches_head(
    state: &Value,
    head: &str,
    allowed_parts: Option<&HashSet<String>>,
) -> Result<()> {
    let Some(ex) = state.get("explain").and_then(Value::as_object) else {
        bail!("saved explain state lacks an `explain` object");
    };
    if let Some(models_val) = ex.get("models")
        && !models_val.is_null()
    {
        let Some(models) = models_val.as_array() else {
            bail!("saved explain state `explain.models` must be an array");
        };
        for m in models {
            let Some(addr) = m.as_str() else {
                bail!("saved explain state `explain.models` contains a non-string entry");
            };
            if addr != head {
                bail!("saved explain state lists model `{addr}`, not `{head}`");
            }
        }
    }
    let Some(jobs) = ex.get("jobs").and_then(Value::as_object) else {
        bail!("saved explain state lacks an `explain.jobs` object");
    };
    for (id, job) in jobs {
        let Some(job_obj) = job.as_object() else {
            bail!("saved explain job `{id}` must be an object");
        };
        let Some(model) = job_obj.get("model").and_then(Value::as_str) else {
            bail!("saved explain job `{id}` lacks a string `model` field");
        };
        if model != head {
            bail!("saved explain job `{id}` belongs to `{model}`, not `{head}`");
        }
        let assigned_vec = job_assigned_addresses(job_obj, id)?;
        let mut assigned_set: HashSet<&str> = HashSet::new();
        for addr in &assigned_vec {
            if let Some(allowed) = allowed_parts
                && !allowed.contains(addr.as_str())
            {
                bail!(
                    "saved explain job `{id}` references `{addr}`, which is not a live filed Derivation part of `{head}`"
                );
            }
            assigned_set.insert(addr.as_str());
        }
        if let Some(val) = job_obj.get("done")
            && !val.is_null()
        {
            let Some(arr) = val.as_array() else {
                bail!("saved explain job `{id}` field `done` must be an array");
            };
            for item in arr {
                let Some(addr) = item.as_str().filter(|s| !s.is_empty()) else {
                    bail!("saved explain job `{id}` field `done` contains an invalid address");
                };
                if !assigned_set.contains(addr) {
                    bail!(
                        "saved explain job `{id}` marks `{addr}` as done, which is outside its own assignment"
                    );
                }
                if let Some(allowed) = allowed_parts
                    && !allowed.contains(addr)
                {
                    bail!(
                        "saved explain job `{id}` references `{addr}`, which is not a live filed Derivation part of `{head}`"
                    );
                }
            }
        }
    }
    Ok(())
}

fn read_marker(work: &Path) -> Option<Value> {
    let marker_path = work.join(WORK_MARKER);
    let raw = std::fs::read_to_string(&marker_path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed: Value = serde_json::from_str(trimmed).ok()?;
    parsed.is_object().then_some(parsed)
}

fn marker_is_valid_owned(work: &Path) -> bool {
    let Some(m) = read_marker(work) else {
        return false;
    };
    let head = m["head"].as_str().unwrap_or("");
    let id = m["archive"]["id"].as_str().unwrap_or("");
    let path = m["archive"]["path"].as_str().unwrap_or("");
    !head.is_empty() && !id.is_empty() && !path.is_empty()
}

fn write_marker(work: &Path, head: &str, archive: &ArchiveId) -> Result<()> {
    let text = format!(
        "{}\n",
        serde_json::to_string(&json!({
            "head": head,
            "archive": { "id": archive.id, "path": archive.path }
        }))?
    );
    util::write_atomic(work.join(WORK_MARKER), &text)
}

/// Verifies `.zt-model-explain-work` binds `work` to `head` and the live archive `{id, path}`.
pub fn assert_work_identity(head: &str, work: &Path, archive: Option<&ArchiveId>) -> Result<()> {
    let Some(marker) = read_marker(work) else {
        bail!(
            "work directory {} has no valid {WORK_MARKER} marker (missing head): run model-explain plan or pass -Force",
            work.display()
        );
    };
    let marker_head = marker["head"].as_str().unwrap_or("");
    if marker_head.is_empty() {
        bail!(
            "work directory {} has no valid {WORK_MARKER} marker (missing head): run model-explain plan or pass -Force",
            work.display()
        );
    }
    if marker_head != head {
        bail!(
            "work directory {} belongs to {marker_head}, not {head}: use a separate -Work directory or pass -Force",
            work.display()
        );
    }
    let id = marker["archive"]["id"].as_str().unwrap_or("");
    let path = marker["archive"]["path"].as_str().unwrap_or("");
    if id.is_empty() || path.is_empty() {
        bail!(
            "work directory {} marker lacks archive {{id, path}} identity: refusing to resume without verified archive binding (pass -Force to reset)",
            work.display()
        );
    }
    if let Some(live) = archive
        && (id != live.id || path != live.path)
    {
        bail!(
            "work directory {} belongs to archive {id} ({path}), not {} ({}): refusing to resume across archives",
            work.display(),
            live.id,
            live.path
        );
    }
    Ok(())
}

/// Safely validates and (re)initializes `work` before running `model -Step explain`.
/// Even with `-Force`, refuses to wipe a non-empty directory that lacks a valid structured marker.
/// Without `-Force`, revalidates both job/head membership and strict settlement before reset (H6).
pub fn prepare_plan_dir(
    head: &str,
    work: &Path,
    force: bool,
    archive: &ArchiveId,
    allowed_parts: Option<&HashSet<String>>,
) -> Result<()> {
    if let Ok(meta) = std::fs::symlink_metadata(work) {
        if meta.file_type().is_symlink() {
            bail!(
                "refusing to reset work path {} because it is a symbolic link/junction; pass the resolved target directory instead",
                work.display()
            );
        }
        let has_entries = std::fs::read_dir(work)
            .with_context(|| format!("failed to read work directory {}", work.display()))?
            .next()
            .is_some();
        let is_tool_owned = marker_is_valid_owned(work);
        if has_entries && !is_tool_owned {
            bail!(
                "refusing to wipe existing non-empty directory {} (no valid {WORK_MARKER} marker): choose a fresh -Work path",
                work.display()
            );
        }
        if is_tool_owned && !force {
            assert_work_identity(head, work, Some(archive))?;
            let state_file = work.join("state.json");
            if !state_file.is_file() {
                bail!(
                    "refusing to wipe work directory {} without state.json: inspect it or pass -Force to reset",
                    work.display()
                );
            }
            let prev = util::read_json(&state_file).with_context(|| {
                format!(
                    "refusing to wipe unreadable state.json in {}: inspect it or pass -Force to reset",
                    work.display()
                )
            })?;
            assert_state_matches_head(&prev, head, allowed_parts).with_context(|| {
                format!(
                    "refusing to wipe work directory {} with invalid/mismatched state.json: inspect it or pass -Force to reset",
                    work.display()
                )
            })?;
            if has_unfinished_state(&prev) {
                bail!(
                    "refusing to wipe unfinished explain state in {}: resume apply/finish or pass -Force to reset",
                    work.display()
                );
            }
        }
        std::fs::remove_dir_all(work)
            .with_context(|| format!("failed to clean work directory {}", work.display()))?;
    }
    std::fs::create_dir_all(work)
        .with_context(|| format!("failed to create work directory {}", work.display()))?;
    write_marker(work, head, archive)?;
    Ok(())
}

/// Clears any stale `explain.finished_ok: true` flag on an owned work directory BEFORE outer
/// head/lock checks run during `finish`, so a failed repeated `finish` can never leave an old
/// `finished_ok: true` bit authorizing a later reset (H6).
pub fn invalidate_prior_finished_ok(head: &str, work: &Path, archive: &ArchiveId) -> Result<()> {
    if !work.is_dir() || assert_work_identity(head, work, Some(archive)).is_err() {
        return Ok(());
    }
    let state_path = work.join("state.json");
    if !state_path.is_file() {
        return Ok(());
    }
    let Ok(mut state) = util::read_json(&state_path) else {
        return Ok(());
    };
    if state["explain"]["finished_ok"].as_bool() == Some(true) {
        state["explain"]["finished_ok"] = json!(false);
        util::write_atomic(&state_path, &serde_json::to_string_pretty(&state)?)?;
    }
    Ok(())
}

fn read_live_archive_identity(zt: &mut crate::zt::Zt) -> Result<ArchiveId> {
    let res = zt.call("get_cards", json!({ "addresses": [], "view": "text" }))?;
    let id = res["archive_identity"]["id"]
        .as_str()
        .filter(|s| !s.is_empty())
        .context("could not read archive_identity.id from zt service")?
        .to_string();
    let path = res["archive_identity"]["path"]
        .as_str()
        .filter(|s| !s.is_empty())
        .context("could not read archive_identity.path from zt service")?
        .to_string();
    Ok(ArchiveId { id, path })
}

fn verify_model_head(zt: &mut crate::zt::Zt, head: &str) -> Result<HashSet<String>> {
    let m = zt.call("get_model", json!({ "at": head }))?;
    let got = m["head"]["address"].as_str().unwrap_or("");
    if got != head {
        if got.is_empty() {
            bail!("{head} is not a Model Card");
        }
        bail!("{head} is not a Model Card: it belongs to the model {got}; give that address");
    }
    Ok(model::compiler::own_derivations_strict(&m)?.into_iter().collect())
}

fn run_model_step(
    zt: Option<crate::zt::Zt>,
    step: &str,
    work: &Path,
    head: Option<&str>,
    preloaded_state: Option<Value>,
) -> Result<(i32, Value)> {
    let mut argv = vec![
        "-Step".to_string(),
        step.to_string(),
        "-Work".to_string(),
        work.display().to_string(),
    ];
    if let Some(h) = head {
        argv.push("-Models".to_string());
        argv.push(h.to_string());
    }
    model::compiler::run_silent_with_zt_and_state(&argv, zt, preloaded_state)
}

fn step_plan(
    zt: crate::zt::Zt,
    head: &str,
    work: &Path,
    force: bool,
    archive: &ArchiveId,
    allowed_parts: Option<&HashSet<String>>,
) -> Result<(i32, Value)> {
    prepare_plan_dir(head, work, force, archive, allowed_parts)?;
    let (code, out) = run_model_step(Some(zt), "explain", work, Some(head), None)?;
    if code != 0 {
        return Ok((code, out));
    }
    let parts = out["parts"]
        .as_u64()
        .or_else(|| out["parts"].as_f64().map(|f| f as u64))
        .unwrap_or(0);
    let jobs = out["jobs"].as_array().cloned().unwrap_or_default();
    Ok((
        0,
        json!({
            "work": work.display().to_string(),
            "parts": parts,
            "jobs": jobs,
        }),
    ))
}

fn step_apply(
    zt: crate::zt::Zt,
    head: &str,
    work: &Path,
    archive: &ArchiveId,
    allowed_parts: &HashSet<String>,
) -> Result<(i32, Value)> {
    assert_work_identity(head, work, Some(archive))?;
    let state_path = work.join("state.json");
    if !state_path.is_file() {
        bail!(
            "work directory {} has no state.json: run model-explain plan first",
            work.display()
        );
    }
    let state = util::read_json(&state_path)?;
    assert_state_matches_head(&state, head, Some(allowed_parts))?;
    // Pass the exact validated `state` into the compiler (eliminates TOCTOU second read).
    let (code, out) = run_model_step(Some(zt), "explain-apply", work, None, Some(state))?;
    if code != 0 {
        return Ok((code, out));
    }
    let applied = out["applied"].as_u64().unwrap_or(0);
    let refused = out["refused"].as_array().cloned().unwrap_or_default();
    let blocked = out["blocked"].as_array().cloned().unwrap_or_default();
    let jobs = out["jobs"].as_array().cloned().unwrap_or_default();
    Ok((
        0,
        json!({
            "applied": applied,
            "refused": refused,
            "blocked": blocked,
            "jobs": jobs,
        }),
    ))
}

#[derive(Clone, Debug, PartialEq)]
pub enum FinishOutput {
    Summary(Value),
    RecheckStopped(Value),
    InnerStopped,
}

pub fn format_finish_lines(output: &FinishOutput) -> Vec<String> {
    match output {
        FinishOutput::InnerStopped => Vec::new(),
        FinishOutput::RecheckStopped(again) => vec![util::compact(again)],
        FinishOutput::Summary(res) => vec![
            format!(
                "explained {} part(s); Term links changed {} Card(s), second plan {}, refused {}; left {}; refused {}; blocked {}",
                res["changed"].as_array().map_or(0, Vec::len),
                res["link"]["cards"].as_u64().unwrap_or(0),
                res["link"]["second"].as_u64().unwrap_or(0),
                res["link"]["refused"].as_u64().unwrap_or(0),
                res["left"].as_u64().unwrap_or(0),
                res["refused"].as_array().map_or(0, Vec::len),
                res["blocked"].as_array().map_or(0, Vec::len),
            ),
            util::compact(res),
        ],
    }
}

fn flatten_string_map(map: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(obj) = map.as_object() {
        for v in obj.values() {
            if let Some(arr) = v.as_array() {
                out.extend(arr.iter().cloned());
            }
        }
    }
    out
}

fn step_finish(
    mut zt: crate::zt::Zt,
    head: &str,
    work: &Path,
    archive: &ArchiveId,
    allowed_parts: &HashSet<String>,
) -> Result<(i32, FinishOutput)> {
    assert_work_identity(head, work, Some(archive))?;
    let state_path = work.join("state.json");
    if !state_path.is_file() {
        bail!(
            "work directory {} has no state.json: run model-explain plan first",
            work.display()
        );
    }
    let mut state = util::read_json(&state_path)?;
    // Invalidate any prior finished_ok immediately after verifying directory ownership and reading
    // state (BEFORE fallible live membership revalidation, H6) so a failed finish can never leave
    // stale `finished_ok: true` permission to wipe the directory.
    if state["explain"]["finished_ok"].as_bool() == Some(true) {
        state["explain"]["finished_ok"] = json!(false);
        util::write_atomic(&state_path, &serde_json::to_string_pretty(&state)?)?;
    }
    assert_state_matches_head(&state, head, Some(allowed_parts))?;

    let changed = changed_parts(&state);
    let key = key_of(head);

    let mut link_cards = 0u64;
    let mut link_second = 0u64;
    let mut link_refused = 0u64;
    let mut link_complete = true;
    if !changed.is_empty() {
        let staging = work.join("term-staging");
        std::fs::create_dir_all(&staging)?;
        let root = zt.call("get_card", json!({ "at": key }))?;
        let book_title = root["title"].as_str().unwrap_or(key);
        util::write_atomic(
            staging.join("manifest.json"),
            &serde_json::to_string(&json!({ "citation_key": key, "book_title": book_title }))?,
        )?;
        let plan_p = term_link::Params {
            step: "plan".to_string(),
            staging: staging.display().to_string(),
            citation_key: None,
            occurrences: term_link::matches::Policy::First,
            cards: changed.clone(),
            round: None,
            token: None,
            apply: false,
            plan_only: false,
        };
        let first = util::retry_changing_view(
            true,
            &util::CHANGING_VIEW_WAITS,
            |seconds| std::thread::sleep(std::time::Duration::from_secs(seconds)),
            || term_link::plan_with(&mut zt, &plan_p, true),
        )?;
        link_cards = first["cards_changed"].as_u64().unwrap_or(0);
        if link_cards > 0 {
            let apply_p = term_link::Params {
                step: "apply".to_string(),
                apply: true,
                ..plan_p.clone()
            };
            let (apply_code, _) = term_link::apply_with(&mut zt, &apply_p, true)?;
            if apply_code != 0 {
                // Inner apply already printed the typed refusal as the final line; do not overwrite it.
                return Ok((apply_code, FinishOutput::InnerStopped));
            }
        }
        let second = util::retry_changing_view(
            true,
            &util::CHANGING_VIEW_WAITS,
            |seconds| std::thread::sleep(std::time::Duration::from_secs(seconds)),
            || term_link::plan_with(&mut zt, &plan_p, true),
        )?;
        link_second = second["cards_changed"].as_u64().unwrap_or(0);
        link_refused = second["cards_refused"].as_u64().unwrap_or(0);
        link_complete = second["complete"]
            .as_bool()
            .unwrap_or(link_refused == 0);
    }

    // Run the trailing recheck in an exclusive private scratch directory under the already-locked
    // `work` folder so `finish` never touches an externally addressable `<work>/recheck` path (H-new).
    let recheck_scratch = tempfile::Builder::new()
        .prefix(".recheck-")
        .tempdir_in(work)
        .with_context(|| format!("failed to create private recheck scratch under {}", work.display()))?;
    let (recheck_code, again) =
        step_plan(zt, head, recheck_scratch.path(), true, archive, Some(allowed_parts))?;
    if recheck_code != 0 {
        return Ok((recheck_code, FinishOutput::RecheckStopped(again)));
    }
    let left = again["parts"].as_u64().unwrap_or(0);
    let refused = flatten_string_map(&state["explain"]["refused"]);
    let blocked = flatten_string_map(&state["explain"]["blocked"]);

    let clean = left == 0
        && link_second == 0
        && link_refused == 0
        && link_complete
        && refused.is_empty()
        && blocked.is_empty();
    if clean {
        if !state["explain"].is_object() {
            state["explain"] = json!({});
        }
        state["explain"]["finished_ok"] = json!(true);
        util::write_atomic(&state_path, &serde_json::to_string_pretty(&state)?)?;
    }

    Ok((
        if clean { 0 } else { 1 },
        FinishOutput::Summary(json!({
            "changed": changed,
            "link": { "cards": link_cards, "second": link_second, "refused": link_refused, "complete": link_complete },
            "left": left,
            "refused": refused,
            "blocked": blocked,
        })),
    ))
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(
        &util::bind_positional(argv, &["Step", "Model", "Work"])?,
        &["Step", "Model", "Work"],
        &["Force"],
    )?;
    let step = a.choice("Step", &STEPS)?;
    let head = validate_single_head(&a.required("Model")?)?;
    let given = a.has("Work").then(|| a.str("Work"));
    // Resolve the work directory once before locking AND use the exact resolved target for all
    // subsequent operations so a directory link/junction can never shift lock identity mid-reset (H-new).
    let work = resolve_lock_target(&work_of(&head, given.as_deref()))?;
    let force = a.flag("Force");

    // Serialize reset/apply/finish on this resolved work directory before any state read or mutation.
    let _work_lock = WorkLock::acquire(&work)?;

    let mut zt = crate::zt::start()?;
    let archive = read_live_archive_identity(&mut zt)?;
    // On `finish`, clear any prior `finished_ok: true` BEFORE outer head/lock checks can fail
    // so a failed repeated finish never leaves stale reset authorization behind (H6).
    if step == "finish" {
        invalidate_prior_finished_ok(&head, &work, &archive)?;
    }
    let allowed_parts = verify_model_head(&mut zt, &head)?;

    // Fail-closed Literature lock preflight before resetting work or writing any Card (H7).
    if let Some(refusal) = lockcheck::preflight_strict(&mut zt, "model-explain", &step, key_of(&head))? {
        return lockcheck::stop("model-explain", &refusal);
    }

    match step.as_str() {
        "plan" => {
            let (code, res) = step_plan(zt, &head, &work, force, &archive, Some(&allowed_parts))?;
            if code == 0 {
                let parts = res["parts"].as_u64().unwrap_or(0);
                let jobs = res["jobs"].as_array().map_or(0, Vec::len);
                let note = if parts > 0 {
                    format!(
                        "{parts} Derivation part(s) to explain in {jobs} job(s); work {}",
                        work.display()
                    )
                } else {
                    "nothing to explain".to_string()
                };
                util::print_line(&note)?;
            }
            util::print_line(&util::compact(&res))?;
            Ok(code)
        }
        "apply" => {
            let (code, res) = step_apply(zt, &head, &work, &archive, &allowed_parts)?;
            if code == 0 {
                util::print_line(&format!(
                    "applied {}, refused {}, blocked {}, follow-up jobs {}",
                    res["applied"].as_u64().unwrap_or(0),
                    res["refused"].as_array().map_or(0, Vec::len),
                    res["blocked"].as_array().map_or(0, Vec::len),
                    res["jobs"].as_array().map_or(0, Vec::len),
                ))?;
            }
            util::print_line(&util::compact(&res))?;
            Ok(code)
        }
        _ => {
            let (code, output) = step_finish(zt, &head, &work, &archive, &allowed_parts)?;
            for line in format_finish_lines(&output) {
                util::print_line(&line)?;
            }
            Ok(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn validate_single_head_rejects_plural_and_blank_inputs() {
        assert_eq!(validate_single_head("K/2|a").unwrap(), "K/2|a");
        assert!(validate_single_head("A/1,B/1").is_err());
        assert!(validate_single_head("A/1 B/1").is_err());
        assert!(validate_single_head("   ").is_err());
        assert!(validate_single_head("K").is_err());
    }

    #[test]
    fn changed_parts_excludes_refused_and_blocked_and_dedups() {
        let state = json!({
            "explain": {
                "jobs": {
                    "K-2-a:x1": { "done": ["K/2|a|a|1|a", "K/2|a|a|1|a|1", "K/2|a|a|1|a|2"] },
                    "K-2-a:x2": { "done": ["K/2|a|a|3|a", "K/2|a|a|1|a"] },
                },
                "refused": { "K/2|a": ["K/2|a|a|1|a|1: 步骤改了"] },
                "blocked": { "K/2|a": ["K/2|a|a|3|a: card_locked"] },
            }
        });
        assert_eq!(changed_parts(&state), vec!["K/2|a|a|1|a", "K/2|a|a|1|a|2"]);
        assert!(changed_parts(&json!({})).is_empty());
    }

    #[test]
    fn key_and_work_folder_conventions_match_explain_mjs() {
        assert_eq!(key_of("hart1995firms/11|a|7"), "hart1995firms");
        assert_eq!(
            work_of("hart1995firms/11|a|7", None)
                .file_name()
                .and_then(|s| s.to_str()),
            Some("hart1995firms-11-a-7")
        );
        assert_eq!(work_of("K/2|a", Some("D:\\w")), PathBuf::from("D:\\w"));
    }

    #[test]
    fn work_directory_identity_and_unfinished_state_guards_refuse_unsafe_wipes() {
        let arch_a = ArchiveId {
            id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            path: "D:/box-a".into(),
        };
        let arch_b = ArchiveId {
            id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            path: "D:/box-b".into(),
        };

        let foreign = TempDir::new().unwrap();
        std::fs::write(foreign.path().join("keep.txt"), "do not delete").unwrap();
        let err = prepare_plan_dir("K/2|a", foreign.path(), false, &arch_a, None).unwrap_err();
        assert!(format!("{err:#}").contains("refusing to wipe existing non-empty directory"));
        // Even with -Force, a non-empty directory with an empty/invalid marker must NOT be wiped!
        std::fs::write(foreign.path().join(WORK_MARKER), "invalid\n").unwrap();
        assert!(prepare_plan_dir("K/2|a", foreign.path(), true, &arch_a, None).is_err());
        assert_eq!(
            std::fs::read_to_string(foreign.path().join("keep.txt")).unwrap(),
            "do not delete"
        );

        let owned = TempDir::new().unwrap();
        std::fs::write(owned.path().join(WORK_MARKER), "{}\n").unwrap();
        assert!(format!("{:#}", assert_work_identity("K/2|a", owned.path(), Some(&arch_a)).unwrap_err()).contains("missing head"));

        std::fs::write(owned.path().join(WORK_MARKER), "K/2|a\n").unwrap();
        assert!(format!("{:#}", assert_work_identity("K/2|a", owned.path(), Some(&arch_a)).unwrap_err()).contains("no valid"));

        std::fs::write(owned.path().join(WORK_MARKER), r#"{"head":"K/2|a"}"#).unwrap();
        assert!(format!("{:#}", assert_work_identity("K/2|a", owned.path(), Some(&arch_a)).unwrap_err()).contains("lacks archive {id, path}"));

        std::fs::write(
            owned.path().join(WORK_MARKER),
            json!({ "head": "K/2|a", "archive": { "id": arch_a.id, "path": arch_a.path } }).to_string(),
        )
        .unwrap();
        // Wrong-typed explain.jobs must be treated as unfinished!
        assert!(has_unfinished_state(&json!({ "explain": { "jobs": ["bad"] } })));
        assert!(has_unfinished_state(&json!({ "explain": "bad" })));

        std::fs::write(
            owned.path().join("state.json"),
            json!({ "explain": { "jobs": { "j1": { "model": "K/2|a", "addresses": ["K/2|a|a|1|a", "K/2|a|a|1|a|1"], "done": ["K/2|a|a|1|a"] } } } }).to_string(),
        )
        .unwrap();
        assert!(format!("{:#}", prepare_plan_dir("K/2|a", owned.path(), false, &arch_a, None).unwrap_err()).contains("unfinished explain state"));
        assert!(format!("{:#}", prepare_plan_dir("K/9|a", owned.path(), false, &arch_a, None).unwrap_err()).contains("belongs to K/2|a, not K/9|a"));
        assert!(format!("{:#}", prepare_plan_dir("K/2|a", owned.path(), false, &arch_b, None).unwrap_err()).contains("belongs to archive aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        std::fs::write(owned.path().join("state.json"), "{broken json").unwrap();
        assert!(format!("{:#}", prepare_plan_dir("K/2|a", owned.path(), false, &arch_a, None).unwrap_err()).contains("unreadable state.json"));
        prepare_plan_dir("K/2|a", owned.path(), true, &arch_a, None).unwrap();
        assert!(assert_work_identity("K/2|a", owned.path(), Some(&arch_a)).is_ok());

        // State scope check catches foreign model jobs, missing job.model, and addresses outside
        // head's live filed Derivation set.
        let allowed = HashSet::from(["K/2|a|a|1|a".to_string()]);
        assert!(assert_state_matches_head(
            &json!({ "explain": { "jobs": { "j1": { "model": "Other/1|a" } } } }),
            "K/2|a",
            Some(&allowed),
        ).is_err());
        assert!(assert_state_matches_head(
            &json!({ "explain": { "jobs": { "j1": { "addresses": ["K/2|a|a|1|a"] } } } }),
            "K/2|a",
            Some(&allowed),
        ).is_err());
        assert!(assert_state_matches_head(
            &json!({ "explain": { "jobs": { "j1": { "model": "K/2|a", "addresses": ["Other/9|a"] } } } }),
            "K/2|a",
            Some(&allowed),
        ).is_err());
        assert!(assert_state_matches_head(
            &json!({ "explain": { "jobs": { "j1": { "model": "K/2|a", "addresses": ["K/2|a|a|1|a"], "done": ["K/2|a|a|1|a"] } } } }),
            "K/2|a",
            Some(&allowed),
        ).is_ok());

        // Missing state.json, jobs: null, or malformed jobs beside finished_ok: true must require -Force (H6).
        assert!(has_unfinished_state(&json!({ "explain": { "jobs": null } })));
        assert!(has_unfinished_state(&json!({})));
        assert!(has_unfinished_state(&json!({ "explain": { "finished_ok": true, "jobs": "bad" } })));
        assert!(has_unfinished_state(&json!({
            "explain": {
                "finished_ok": true,
                "jobs": { "j1": { "model": "K/2|a", "addresses": ["K/2|a|a|1|a"], "done": [] } }
            }
        })));
        // H6: null inside addresses, done-only job without assignment, or missing job.model must NOT pass.
        assert!(has_unfinished_state(&json!({
            "explain": {
                "finished_ok": true,
                "jobs": { "j1": { "model": "K/2|a", "addresses": ["K/2|a|a|1|a", null], "done": ["K/2|a|a|1|a"] } }
            }
        })));
        assert!(has_unfinished_state(&json!({
            "explain": {
                "finished_ok": true,
                "jobs": { "j1": { "done": ["K/2|a|a|1|a"] } }
            }
        })));
        assert!(!has_unfinished_state(&json!({
            "explain": {
                "finished_ok": true,
                "jobs": { "j1": { "model": "K/2|a", "addresses": ["K/2|a|a|1|a"], "done": ["K/2|a|a|1|a"] } }
            }
        })));
        // H6: prepare_plan_dir revalidates state against allowed_parts even when finished_ok is true,
        // and invalidate_prior_finished_ok clears finished_ok before outer checks can fail.
        let settled_dir = TempDir::new().unwrap();
        write_marker(settled_dir.path(), "K/2|a", &arch_a).unwrap();
        std::fs::write(
            settled_dir.path().join("state.json"),
            json!({
                "explain": {
                    "finished_ok": true,
                    "jobs": { "j1": { "model": "K/2|a", "addresses": ["K/2|a|a|9|a"], "done": ["K/2|a|a|9|a"] } }
                }
            })
            .to_string(),
        )
        .unwrap();
        assert!(prepare_plan_dir("K/2|a", settled_dir.path(), false, &arch_a, Some(&allowed)).is_err());
        invalidate_prior_finished_ok("K/2|a", settled_dir.path(), &arch_a).unwrap();
        let after_inv = util::read_json(settled_dir.path().join("state.json")).unwrap();
        assert_eq!(after_inv["explain"]["finished_ok"], json!(false));

        // H6: a job marking an unassigned address as `done` (even if it is a live Derivation of
        // the same model) must be rejected by assert_state_matches_head, has_unfinished_state, and
        // ignored by changed_parts.
        let allowed_two = HashSet::from(["K/2|a|a|1|a".to_string(), "K/2|a|a|2|a".to_string()]);
        let extra_done = json!({
            "explain": {
                "finished_ok": true,
                "jobs": {
                    "j1": {
                        "model": "K/2|a",
                        "addresses": ["K/2|a|a|1|a"],
                        "done": ["K/2|a|a|1|a", "K/2|a|a|2|a"]
                    }
                }
            }
        });
        assert!(assert_state_matches_head(&extra_done, "K/2|a", Some(&allowed_two)).is_err());
        assert!(has_unfinished_state(&extra_done));
        assert_eq!(changed_parts(&extra_done), vec!["K/2|a|a|1|a"]);

        // H6: parts-only job (no addresses) and conflicting addresses/parts must fail validation & settlement.
        let parts_only = json!({
            "explain": {
                "finished_ok": true,
                "jobs": { "j1": { "model": "K/2|a", "parts": ["K/2|a|a|1|a"], "done": ["K/2|a|a|1|a"] } }
            }
        });
        assert!(assert_state_matches_head(&parts_only, "K/2|a", Some(&allowed_two)).is_err());
        assert!(has_unfinished_state(&parts_only));
        let conflicting_ap = json!({
            "explain": {
                "finished_ok": true,
                "jobs": {
                    "j1": {
                        "model": "K/2|a",
                        "addresses": ["K/2|a|a|1|a"],
                        "parts": ["K/2|a|a|2|a"],
                        "done": ["K/2|a|a|1|a"]
                    }
                }
            }
        });
        assert!(assert_state_matches_head(&conflicting_ap, "K/2|a", Some(&allowed_two)).is_err());
        assert!(has_unfinished_state(&conflicting_ap));

        // WorkLock serializes concurrent access to the same work directory, including alternate
        // path spellings resolving to the same directory (H-new).
        let sub_dir = settled_dir.path().join("sub");
        std::fs::create_dir_all(&sub_dir).unwrap();
        let alt_spelling = sub_dir.join("..");
        assert_eq!(
            WorkLock::lock_path_for(settled_dir.path()).unwrap(),
            WorkLock::lock_path_for(&alt_spelling).unwrap()
        );
        let lock1 = WorkLock::acquire(settled_dir.path()).unwrap();
        assert!(WorkLock::acquire(&alt_spelling).is_err());
        drop(lock1);
        assert!(WorkLock::acquire(settled_dir.path()).is_ok());

        // M3: relative explain output path resolution accepts `out/<file>` inside work and refuses
        // cwd-dependent relative paths or `..` traversal.
        let resolved_out = model::compiler::resolve_explain_out_path(
            settled_dir.path().to_str().unwrap(),
            "out/explain-1.json",
        )
        .unwrap();
        assert!(Path::new(&resolved_out).is_absolute(), "{resolved_out}");
        assert!(model::compiler::resolve_explain_out_path(
            settled_dir.path().to_str().unwrap(),
            "rel-work/out/explain-1.json",
        )
        .is_err());
        assert!(model::compiler::resolve_explain_out_path(
            settled_dir.path().to_str().unwrap(),
            "../other/explain-1.json",
        )
        .is_err());

        // M7: unified own_derivations_strict accepts core Setting rows (which omit `derivation`),
        // requires `filed == true` and explicit non-empty non-shared `line`, and rejects non-boolean `derivation`.
        let model_with_setting_and_unfiled = json!({
            "build": {
                "order": [
                    { "address": "K/2|a|a", "line": "main" },
                    { "address": "K/2|a|a|1|a", "derivation": true, "line": "side" },
                    { "address": "K/2|a|a|2|a", "derivation": true, "line": "side" },
                    { "address": "K/1|a|a|1|a", "derivation": true, "line": "shared" }
                ]
            },
            "parts": [
                { "address": "K/2|a|a", "filed": true },
                { "address": "K/2|a|a|1|a", "filed": true },
                { "address": "K/2|a|a|2|a", "filed": false },
                { "address": "K/1|a|a|1|a", "filed": false }
            ]
        });
        assert_eq!(
            model::compiler::own_derivations_strict(&model_with_setting_and_unfiled).unwrap(),
            vec!["K/2|a|a|1|a"]
        );
        let missing_filed = json!({
            "build": { "order": [{ "address": "K/2|a|a|1|a", "derivation": true, "line": "side" }] },
            "parts": [{ "address": "K/2|a|a|1|a" }]
        });
        assert!(model::compiler::own_derivations_strict(&missing_filed).is_err());
        let string_deriv = json!({
            "build": { "order": [{ "address": "K/2|a|a|1|a", "derivation": "false", "line": "side" }] },
            "parts": [{ "address": "K/2|a|a|1|a", "filed": true }]
        });
        assert!(model::compiler::own_derivations_strict(&string_deriv).is_err());
        let missing_line = json!({
            "build": { "order": [{ "address": "K/2|a|a|1|a", "derivation": true }] },
            "parts": [{ "address": "K/2|a|a|1|a", "filed": true }]
        });
        assert!(model::compiler::own_derivations_strict(&missing_line).is_err());

        // Relative -Work must resolve to an absolute path (M3).
        assert!(work_of("K/2|a", Some("rel-explain-work")).is_absolute());
        let no_state = TempDir::new().unwrap();
        write_marker(no_state.path(), "K/2|a", &arch_a).unwrap();
        let err = prepare_plan_dir("K/2|a", no_state.path(), false, &arch_a, None).unwrap_err();
        assert!(format!("{err:#}").contains("state.json"), "{err:#}");
        prepare_plan_dir("K/2|a", no_state.path(), true, &arch_a, None).unwrap();

        // Trailing recheck failure must NOT format as a bogus "explained 0 part(s)" summary.
        let lines = format_finish_lines(&FinishOutput::RecheckStopped(json!({"ok": false, "code": "card_locked"})));
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].contains("explained 0 part(s)"));
        assert!(lines[0].contains("card_locked"));
    }
}
