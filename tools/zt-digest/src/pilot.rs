//! `zt-digest pilot`: the one-book pilot gate (S26, issue #117; design §15-phase-2, D369/D370).
//!
//! The gate qualifies one explicitly authorized book under the frozen digest variants. It has
//! three steps, each read-only over Cards: it writes only under `-Work`, and it never writes a
//! Card, a register, a hub or an archive row.
//!
//! ```text
//! zt-digest pilot -Step plan      -Staging <out> -Work <folder> -File <authorization.json>
//! zt-digest pilot -Step preflight -Staging <out> -Work <folder> [-Project <dir>]
//! zt-digest pilot -Step qualify   -Staging <out> -Work <folder> [-RunReport <run-report.json>]
//!                                 [-Audit <audit.json>] [-Cards <cards/report.json>] [-Sample 12]
//! ```
//!
//! `plan` (S26-AC01) names the source, archive, staging, book groups, baseline sample, installed
//! binaries, Workflow versions, caps and approved new-Card scope before launch, verifies every
//! hash it records, and refuses to write a plan that misses one of them. It also fixes the
//! protected scope: the authored fingerprint of every Card that exists before launch, so a later
//! step can prove that no old Card changed.
//!
//! `preflight` (S26-AC02) refuses a launch that would reuse old evidence: a run that already
//! started or was resumed, a live claim of another invocation, a prepared run whose identity or
//! contract moved, a protected Card that changed, a missing or changed backup, or an installed
//! frozen variant that no longer matches the prepared contract. It reads the ledger's own
//! dependency rows and reports the Link prerequisite of every registered Link unit.
//!
//! `qualify` (S26-AC03–S26-AC08) reads the retained run report, the audit, the Card cycle report
//! and the run's own receipts and completions, then states what the pilot produced: the new Cards
//! and their review outcomes, the earlier-filing and Link-ownership evidence, the protected scope
//! before and after, the measured Topic dependence with unknowns separate, the provider usage and
//! missing coverage, the interruption facts, and the concrete old-data proposals that wait for the
//! separate review gate. Its decision is always `pending-human`: the gate records evidence and
//! never accepts a pilot by itself.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::args::Args;
use crate::canon;
use crate::ledger::{self, Prepared};
use crate::ps;
use crate::util;
use crate::zt::Zt;

/// The record's own contract. A later build reads the same shapes, so a plan stays verifiable.
pub const CONTRACT: &str = "pilot/1";
/// The plan record, written by `-Step plan`.
pub const PLAN_FILE: &str = "pilot-plan.json";
/// The protected scope: every Card that existed at plan time, with its authored fingerprint.
pub const PROTECTED_FILE: &str = "pilot-protected.json";
/// The launch record, written by `-Step preflight`.
pub const PREFLIGHT_FILE: &str = "pilot-preflight.json";
/// The qualification record, written by `-Step qualify`.
pub const QUALIFICATION_FILE: &str = "pilot-qualification.json";
/// The one decision this gate can record. A human accepts or returns corrections; no tool does.
pub const DECISION: &str = "pending-human";
/// The owned artifacts of one completed group, read as the run's own receipts (S23/#115).
const OWNED: &str = "owned-g";
/// The installed binaries a pilot must name before it launches (S26-AC01).
pub const BINARIES: [&str; 3] = ["zt", "zt-check", "zt-digest"];

pub struct Params {
    pub step: String,
    pub staging: String,
    pub work: String,
    pub file: String,
    pub project: String,
    pub run_report: String,
    pub audit: String,
    pub cards: String,
    pub sample: usize,
}

/// `zt-digest pilot`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = [
        "Step",
        "Staging",
        "Work",
        "File",
        "Project",
        "RunReport",
        "Audit",
        "Cards",
        "Sample",
    ];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &[])?;
    let project = if a.has("Project") {
        a.required("Project")?
    } else {
        std::env::current_dir()?.display().to_string()
    };
    let p = Params {
        step: a.choice("Step", &["plan", "preflight", "qualify"])?.to_ascii_lowercase(),
        staging: a.required("Staging")?,
        work: a.required("Work")?,
        file: a.str("File"),
        project,
        run_report: a.str("RunReport"),
        audit: a.str("Audit"),
        cards: a.str("Cards"),
        sample: a.int_or("Sample", 12)?.clamp(1, 40) as usize,
    };
    let mut zt = crate::zt::start()?;
    let prepared = Prepared::read_mode(&p.staging, &mut zt, false)?;
    std::fs::create_dir_all(&p.work)
        .with_context(|| format!("cannot create the pilot work folder `{}`", p.work))?;
    match p.step.as_str() {
        "plan" => {
            if p.file.trim().is_empty() {
                bail!(
                    "-File is required: the authorization record that names the approved scope, backup, read-only baseline, preserved builds, installed binaries and launch caps"
                );
            }
            let record = plan(&prepared, &mut zt, &p, &p.file)?;
            util::print_line(&ps::json_compact(&record))?;
            Ok(0)
        }
        "preflight" => {
            let record = preflight(&prepared, &mut zt, &p)?;
            let ok = record["ok"] == json!(true);
            util::print_line(&ps::json_compact(&record))?;
            Ok(if ok { 0 } else { 1 })
        }
        _ => {
            let record = qualify(&prepared, &mut zt, &p)?;
            util::print_line(&ps::json_compact(&record))?;
            Ok(0)
        }
    }
}

// --- checks ---------------------------------------------------------------------------------------

/// One named check with its detail. A refused step names every failed check, not only the first,
/// so the operator corrects one launch record once.
#[derive(Default)]
struct Checks(Vec<Value>);

impl Checks {
    /// A failed check keeps the reason; a passed one keeps an empty detail, so the record reads
    /// as a list of gates rather than a list of messages.
    fn require(&mut self, name: &str, ok: bool, detail: impl Into<String>) {
        let detail = if ok { String::new() } else { detail.into() };
        self.0.push(json!({"name": name, "ok": ok, "detail": detail}));
    }

    fn require_that(&mut self, name: &str, ok: bool, detail: impl FnOnce() -> String) {
        if ok {
            self.0.push(json!({"name": name, "ok": true, "detail": ""}));
        } else {
            self.0.push(json!({"name": name, "ok": false, "detail": detail()}));
        }
    }

    fn failures(&self) -> Vec<String> {
        self.0
            .iter()
            .filter(|check| check["ok"] != json!(true))
            .map(|check| format!("{}: {}", ps::s(&check["name"]), ps::s(&check["detail"])))
            .collect()
    }

    /// Every check passed: the step may write its record. A refused step writes nothing.
    fn finish(self, what: &str) -> Result<Vec<Value>> {
        let failed = self.failures();
        if !failed.is_empty() {
            bail!("{what} refused: {}", failed.join("; "));
        }
        Ok(self.0)
    }

    /// The launch gate keeps its facts: it writes the record with `ok: false` and exits nonzero.
    fn reported(self) -> (bool, Value, Vec<String>) {
        let ok = self.failures().is_empty();
        let problems = self.failures();
        (ok, json!(self.0), problems)
    }
}

// --- small helpers --------------------------------------------------------------------------------

fn blank(value: &str) -> bool {
    value.trim().is_empty()
}

fn at(value: &Value, key: &str) -> String {
    ps::s(&value[key])
}

/// The bytes and the SHA-256 of one existing file.
fn file_identity(path: &Path) -> Result<(String, u64)> {
    let bytes = std::fs::read(path).with_context(|| format!("missing file `{}`", path.display()))?;
    Ok((canon::sha256_hex(&bytes), bytes.len() as u64))
}

/// One file named by an evidence record, verified: it must exist, and its recorded hash and byte
/// count must describe its current bytes.
fn check_file_record(checks: &mut Checks, name: &str, record: &Value) -> Result<Value> {
    let path = at(record, "path");
    let exists = !blank(&path) && Path::new(&path).is_file();
    let mut verified = json!({"path": path, "exists": exists});
    if !exists {
        checks.require(name, false, format!("the named file `{path}` does not exist"));
        return Ok(verified);
    }
    let (sha, bytes) = file_identity(Path::new(&path))?;
    verified["sha256"] = json!(sha);
    verified["bytes"] = json!(bytes);
    if let Some(named) = record["sha256"].as_str().filter(|value| !value.is_empty()) {
        checks.require_that(
            name,
            ps::eq(named, &sha),
            || format!("`{path}` has SHA-256 {sha}, not the recorded {named}"),
        );
    } else {
        checks.require(name, false, format!("`{path}` has no recorded SHA-256"));
    }
    if let Some(named) = record["bytes"].as_u64() {
        checks.require_that(name, named == bytes, || {
            format!("`{path}` has {bytes} bytes, not the recorded {named}")
        });
    }
    Ok(verified)
}

/// One bounded `<program> <argument>` run, so a version probe can never stall the gate.
fn bounded_version(program: &Path, argument: &str) -> Option<String> {
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .arg(argument)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Err(_) => return None,
        }
    }
    let output = child.wait_with_output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

// --- ledger facts ---------------------------------------------------------------------------------

/// The prepared run's own recorded state, read without writing. This is the whole of the gate's
/// view of scheduling: unit states, the dependency rows and the append-only event history.
struct Facts {
    run_id: String,
    invocation: i64,
    resume_count: i64,
    created_at: String,
    units: Vec<Value>,
    deps: Vec<Value>,
    events: Vec<Value>,
}

fn meta_opt(conn: &rusqlite::Connection, key: &str) -> Option<Value> {
    let text: String = conn
        .query_row("SELECT value FROM meta WHERE key=?1", [key], |row| row.get(0))
        .ok()?;
    serde_json::from_str(&text).ok()
}
fn facts(out: &Path) -> Result<Facts> {
    let ledger = out.join("ledger.sqlite3");
    if !ledger.is_file() {
        bail!(
            "`{}` holds no prepared ledger; a pilot starts from a fresh prepared run made by the frozen Workflow's prepare stage",
            out.display()
        );
    }
    let conn = ledger::connect(&ledger, false)?;
    let run_id = meta_opt(&conn, "run_id")
        .and_then(|value| value.as_str().map(str::to_string))
        .context("the prepared ledger names no run id")?;
    let invocation = meta_opt(&conn, "invocation")
        .and_then(|value| value.as_i64())
        .context("the prepared ledger names no invocation number")?;
    let resume_count = meta_opt(&conn, "resume_count")
        .and_then(|value| value.as_i64())
        .unwrap_or(0);
    let created_at = meta_opt(&conn, "created_at")
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();
    let mut units = Vec::new();
    {
        let mut statement = conn.prepare(
            "SELECT id, layer, state, attempts, COALESCE(owner,''), claim_token, COALESCE(lease_until,0), COALESCE(completed_at,'') FROM units ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "layer": row.get::<_, String>(1)?,
                "state": row.get::<_, String>(2)?,
                "attempts": row.get::<_, i64>(3)?,
                "owner": row.get::<_, String>(4)?,
                "claim_token": row.get::<_, i64>(5)?,
                "lease_until": row.get::<_, i64>(6)?,
                "completed_at": row.get::<_, String>(7)?,
            }))
        })?;
        for row in rows {
            units.push(row?);
        }
    }
    let mut deps = Vec::new();
    {
        let mut statement =
            conn.prepare("SELECT unit_id, needs_id, condition FROM deps ORDER BY unit_id, needs_id")?;
        let rows = statement.query_map([], |row| {
            Ok(json!({
                "unit": row.get::<_, String>(0)?,
                "needs": row.get::<_, String>(1)?,
                "condition": row.get::<_, String>(2)?,
            }))
        })?;
        for row in rows {
            deps.push(row?);
        }
    }
    let mut events = Vec::new();
    {
        let mut statement = conn.prepare(
            "SELECT seq, COALESCE(unit_id,''), invocation, utc, COALESCE(owner,''), COALESCE(event,''), facts_json FROM events ORDER BY seq",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(json!({
                "seq": row.get::<_, i64>(0)?,
                "unit": row.get::<_, String>(1)?,
                "invocation": row.get::<_, i64>(2)?,
                "utc": row.get::<_, String>(3)?,
                "owner": row.get::<_, String>(4)?,
                "event": row.get::<_, String>(5)?,
                "facts": serde_json::from_str::<Value>(&row.get::<_, String>(6)?).unwrap_or(Value::Null),
            }))
        })?;
        for row in rows {
            events.push(row?);
        }
    }
    Ok(Facts {
        run_id,
        invocation,
        resume_count,
        created_at,
        units,
        deps,
        events,
    })
}

fn unit_states(facts: &Facts) -> BTreeMap<String, i64> {
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    for unit in &facts.units {
        *counts.entry(at(unit, "state")).or_default() += 1;
    }
    counts
}

/// A run that has not started: every content unit still waits, and the one resume is unspent.
/// The preparation unit is the one unit a fresh prepared run has already completed: it records
/// the frozen inputs, not digest work, so it is no sign of a launch.
fn launch_state(facts: &Facts) -> Value {
    json!({
        "run_id": facts.run_id,
        "invocation": facts.invocation,
        "resume_count": facts.resume_count,
        "created_at": facts.created_at,
        "units": unit_states(facts),
        "units_total": facts.units.len(),
    })
}

fn started(facts: &Facts) -> Option<String> {
    if facts.invocation != 1 {
        return Some(format!(
            "the run is on invocation {}, not its first launch",
            facts.invocation
        ));
    }
    if facts.resume_count != 0 {
        return Some(format!("the one resume is already spent (`resume_count` {})", facts.resume_count));
    }
    let moved: Vec<String> = facts
        .units
        .iter()
        .filter(|unit| !at(unit, "id").starts_with("prepare/"))
        .filter(|unit| !matches!(at(unit, "state").as_str(), "registered" | "ready"))
        .map(|unit| format!("{} is {}", at(unit, "id"), at(unit, "state")))
        .collect();
    if !moved.is_empty() {
        let shown: Vec<&String> = moved.iter().take(5).collect();
        let more = if moved.len() > shown.len() {
            format!(" and {} more", moved.len() - shown.len())
        } else {
            String::new()
        };
        return Some(format!(
            "the run already moved past launch: {}{more}",
            shown.iter().map(|value| value.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    None
}

/// A claim of another invocation that has not expired: the run belongs to a live writer.
fn live_claim(facts: &Facts, now: u64) -> Option<String> {
    let live: Vec<String> = facts
        .units
        .iter()
        .filter(|unit| at(unit, "state") == "claimed" && unit["lease_until"].as_u64().unwrap_or(0) > now)
        .map(|unit| format!("{} held by `{}`", at(unit, "id"), at(unit, "owner")))
        .collect();
    if live.is_empty() {
        None
    } else {
        Some(format!("a live claim is open: {}", live.join(", ")))
    }
}

/// The ledger's own Link prerequisite (S26-AC02, #92). Every registered `link/gNNN@1` unit must
/// depend on its group's digest unit with `success`, so no Link job exists before that group's
/// work is done. `link-check` must cover every prepared group, and its own prerequisite rows are
/// reported so the launch record names the actual path.
fn link_prerequisite(facts: &Facts) -> Value {
    let mut link_units = Vec::new();
    for unit in &facts.units {
        let id = at(unit, "id");
        if !id.starts_with("link/g") {
            continue;
        }
        let number = id
            .trim_start_matches("link/g")
            .split('@')
            .next()
            .unwrap_or_default()
            .to_string();
        let needs: Vec<Value> = facts
            .deps
            .iter()
            .filter(|dep| ps::eq(&at(dep, "unit"), &id))
            .collect::<Vec<_>>()
            .into_iter()
            .cloned()
            .collect();
        link_units.push(json!({"unit": id, "group": number, "needs": needs}));
    }
    let check_deps: Vec<Value> = facts
        .deps
        .iter()
        .filter(|dep| ps::eq(&at(dep, "unit"), "link-check/book@1"))
        .cloned()
        .collect();
    json!({
        "link_units": link_units,
        "link_check_needs": check_deps,
        "link_units_total": link_units.len(),
    })
}

// --- the archive and the protected scope ----------------------------------------------------------

/// Every Card of the archive at one revision: `export_titles` names them, one complete bulk read
/// returns their text and metadata. A bounded read returns every requested Card or it fails.
fn read_archive(zt: &mut Zt) -> Result<Value> {
    let export = zt.call("export_titles", json!({}))?;
    if export["complete"] != json!(true) {
        bail!("incomplete title enumeration: the archive's Card list is not complete");
    }
    let addresses: Vec<String> = export["titles"]
        .as_array()
        .context("incomplete title enumeration: no titles")?
        .iter()
        .map(|row| at(row, "address"))
        .filter(|address| !blank(address))
        .collect();
    let read = zt.cards(&addresses, "text")?;
    if read.cards.len() != addresses.len() {
        bail!(
            "incomplete Card read: {} of {} Cards",
            read.cards.len(),
            addresses.len()
        );
    }
    Ok(json!({"revision": read.archive_revision, "cards": read.cards}))
}

/// The protected scope: one authored fingerprint per Card that exists now. A Card counts as
/// changed when its fingerprint differs, and as removed when it no longer exists.
fn protected_view(cards: &[Value]) -> Value {
    let mut pairs: Vec<(String, String)> = cards
        .iter()
        .map(|card| (at(card, "address"), crate::calls::authored_fingerprint(card)))
        .collect();
    pairs.sort();
    let mut by_workspace: BTreeMap<String, i64> = BTreeMap::new();
    let mut by_type: BTreeMap<String, i64> = BTreeMap::new();
    for card in cards {
        *by_workspace.entry(at(card, "workspace")).or_default() += 1;
        *by_type.entry(at(card, "type")).or_default() += 1;
    }
    let digest = pairs_digest(&pairs);
    json!({
        "count": pairs.len(),
        "digest": digest,
        "by_workspace": by_workspace,
        "by_type": by_type,
        "models": cards.iter().filter(|card| at(card, "type") == "model").count(),
        "human": cards.iter().filter(|card| at(card, "workspace") == "human").count(),
    })
}

fn pairs_digest(pairs: &[(String, String)]) -> String {
    let list: Vec<Value> = pairs
        .iter()
        .map(|(address, fingerprint)| json!([address, fingerprint]))
        .collect();
    canon::sha256_hex(canon::canonical_json(&json!(list)).as_bytes())
}

/// Both scopes in one sorted list: the old Cards and this run's own prepared placeholders.
fn all_pairs(old: &[(String, String)], prepared: &[(String, String)]) -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = old.iter().chain(prepared.iter()).cloned().collect();
    all.sort();
    all
}

fn pairs_of_key(view: &Value, key: &str) -> Vec<(String, String)> {
    view[key]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let pair = row.as_array()?;
                    Some((ps::s(pair.first()?), ps::s(pair.get(1)?)))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn pairs_of(view: &Value) -> Vec<(String, String)> {
    pairs_of_key(view, "cards")
}

/// What happened to the protected scope: added (new Cards), removed and changed, each exact.
fn diff_protected(before: &[(String, String)], after: &[(String, String)]) -> Value {
    let before: BTreeMap<&str, &str> = before.iter().map(|(a, f)| (a.as_str(), f.as_str())).collect();
    let after: BTreeMap<&str, &str> = after.iter().map(|(a, f)| (a.as_str(), f.as_str())).collect();
    let added: Vec<String> = after.keys().filter(|address| !before.contains_key(**address)).map(|a| a.to_string()).collect();
    let removed: Vec<String> = before.keys().filter(|address| !after.contains_key(**address)).map(|a| a.to_string()).collect();
    let changed: Vec<String> = before
        .iter()
        .filter(|(address, fingerprint)| after.get(**address).is_some_and(|now| now != *fingerprint))
        .map(|(address, _)| address.to_string())
        .collect();
    json!({"added": added, "removed": removed, "changed": changed})
}

// --- the installed release ------------------------------------------------------------------------

/// The installed frozen Workflow that declares this prepared contract: its registry lock, its
/// script bytes and its fingerprint. A pilot that cannot name them has not qualified a release.
fn workflow_identity(project: &Path, variant: &str) -> Result<Value> {
    let name = format!("zt-{variant}-digest");
    let mut roots = vec![project.to_path_buf()];
    if let Some(home) = home_dir() {
        roots.push(home);
    }
    let mut tried = Vec::new();
    for root in roots {
        let registry = root.join(".claude").join("workflow-framework").join(&name);
        let lock_path = registry.join("lock.json");
        if !lock_path.is_file() {
            tried.push(lock_path.display().to_string());
            continue;
        }
        let lock = util::read_json(&lock_path)?;
        let relative = at(&lock, "script");
        let script = if Path::new(&relative).is_absolute() {
            PathBuf::from(&relative)
        } else {
            root.join(&relative)
        };
        if !script.is_file() {
            tried.push(script.display().to_string());
            continue;
        }
        let (sha, bytes) = file_identity(&script)?;
        let script = std::fs::canonicalize(&script).unwrap_or(script);
        return Ok(json!({
            "name": at(&lock, "name"),
            "version": at(&lock, "version"),
            "scope": at(&lock, "scope"),
            "fingerprint": at(&lock, "fingerprint"),
            "registry": registry.display().to_string(),
            "lock": lock_path.display().to_string(),
            "script": script.display().to_string(),
            "script_sha256": sha,
            "script_bytes": bytes,
            "recorded_sha256": at(&lock, "sha256"),
        }));
    }
    bail!(
        "no installed frozen Workflow `{name}`: looked for {}; install the frozen variants before a pilot",
        tried.join(", ")
    )
}

/// The binaries the authorization names, each verified by bytes and by its own version. The
/// running `zt-digest` is this process, so a different file is refused rather than recorded.
fn binary_identities(checks: &mut Checks, auth: &Value) -> Result<Vec<Value>> {
    let named = auth["binaries"]
        .as_array()
        .context("binaries: the authorization record names no binaries")?;
    let mut found: BTreeMap<String, Value> = BTreeMap::new();
    for record in named {
        let name = at(record, "name").to_ascii_lowercase();
        let path = at(record, "path");
        let known = BINARIES.iter().any(|expected| ps::eq(expected, &name));
        checks.require(
            "binaries.named",
            known,
            format!("`{name}` is not one of {}", BINARIES.join(", ")),
        );
        if !known {
            continue;
        }
        let exists = !blank(&path) && Path::new(&path).is_file();
        checks.require(
            "binaries.exist",
            exists,
            format!("the named `{name}` at `{path}` does not exist"),
        );
        if !exists {
            continue;
        }
        let (sha, bytes) = file_identity(Path::new(&path))?;
        let version = bounded_version(Path::new(&path), if name == "zt-check" { "--version" } else { "version" })
            .unwrap_or_default();
        found.insert(
            name.clone(),
            json!({"name": name, "path": path, "sha256": sha, "bytes": bytes, "version": version}),
        );
    }
    for expected in BINARIES {
        checks.require(
            "binaries.complete",
            found.contains_key(expected),
            format!("the authorization record does not name `{expected}`"),
        );
    }
    if let Some(record) = found.get_mut("zt-digest") {
        let running = std::env::current_exe().ok();
        let same = running.as_deref().is_some_and(|exe| {
            std::fs::read(exe).ok().map(|bytes| canon::sha256_hex(&bytes)) == record["sha256"].as_str().map(str::to_string)
        });
        checks.require_that("binaries.running", same, || {
            format!(
                "the named `zt-digest` at `{}` is not the binary running this gate",
                at(record, "path")
            )
        });
        record["version"] = json!(format!("zt-digest {}", env!("CARGO_PKG_VERSION")));
    }
    for name in ["zt-check", "zt"] {
        if let Some(record) = found.get_mut(name) {
            let version = at(record, "version");
            if blank(&version) {
                checks.require(name, false, format!("`{name} version` answered nothing"));
                continue;
            }
            let expected = if name == "zt-check" {
                format!("zt-check {}", zt_check::VERSION)
            } else {
                String::new()
            };
            if !expected.is_empty() {
                checks.require_that("binaries.linked", ps::eq(&version, &expected), || {
                    format!("the installed `zt-check` reports `{version}`, not `{expected}`")
                });
            }
        }
    }
    Ok(found.into_values().collect())
}

// --- plan (S26-AC01) ------------------------------------------------------------------------------

fn plan(prepared: &Prepared, zt: &mut Zt, p: &Params, file: &str) -> Result<Value> {
    let auth = util::read_json(file).with_context(|| format!("`{file}` is not a JSON authorization record"))?;
    let contract = &prepared.contract;
    let key = at(contract, "citation_key");
    let variant = at(contract, "variant");
    let staging = prepared.out.display().to_string();
    let mut checks = Checks::default();

    // --- the authorization record itself ---
    checks.require(
        "authorization.schema",
        auth["schema_version"] == json!(1),
        "the authorization record needs `schema_version: 1`",
    );
    let scope = &auth["approved_scope"];
    let statement = ps::s(&scope["statement"]).trim().to_string();
    checks.require(
        "scope.statement",
        !statement.is_empty(),
        "approved_scope.statement must state exactly what new work the pilot authorizes",
    );
    let prepared_groups: BTreeSet<i64> = contract["groups"]
        .as_array()
        .map(|groups| groups.iter().filter_map(|group| group["n"].as_i64()).collect())
        .unwrap_or_default();
    let approved: BTreeSet<i64> = scope["groups"]
        .as_array()
        .map(|groups| groups.iter().filter_map(|group| group.as_i64()).collect())
        .unwrap_or_default();
    checks.require_that("scope.groups", approved == prepared_groups, || {
        format!(
            "approved_scope.groups is {approved:?}, but the prepared run has {:?}",
            prepared_groups
        )
    });
    let old_edits = scope["old_card_edits"].as_array().map(Vec::len).unwrap_or(0);
    checks.require(
        "scope.old_card_edits",
        old_edits == 0,
        "a pilot authorizes no old-Card edit: list proposed old-data edits in `qualify` for the separate review gate",
    );
    let launch = &auth["launch"];
    checks.require(
        "launch.mode",
        at(launch, "mode") == "full",
        "launch.mode must be `full`: the pilot runs the whole frozen flow",
    );
    let out = at(&launch["args"], "out");
    let launch_out = if blank(&out) {
        String::new()
    } else {
        std::fs::canonicalize(&out)
            .map(|path| path.display().to_string())
            .unwrap_or(out.clone())
    };
    checks.require_that("launch.out", ps::eq(&launch_out, &staging), || {
        format!("launch.args.out is `{launch_out}`, not the prepared staging folder `{staging}`")
    });
    checks.require_that("launch.citation_key", ps::eq(&at(&launch["args"], "citationKey"), &key), || {
        format!("launch.args.citationKey must be the prepared Citation key `{key}`")
    });
    let settings = &contract["settings"];
    for (arg, setting, low, high) in [
        ("maxAttempts", "max_attempts", 1_i64, 10_i64),
        ("dshRunCap", "dsh_run_cap", 60_i64, 3600_i64),
    ] {
        let named = launch["args"][arg].as_i64();
        let recorded = settings[setting].as_i64();
        checks.require_that("launch.caps", named.is_some(), || {
            format!("launch.args.{arg} must name the cap before launch")
        });
        if let Some(named) = named {
            checks.require_that("launch.caps", (low..=high).contains(&named), || {
                format!("launch.args.{arg} is {named}, outside {low}..={high}")
            });
        }
        if let (Some(named), Some(recorded)) = (named, recorded) {
            checks.require_that("launch.caps", named == recorded, || {
                format!("launch.args.{arg} is {named}, but the prepared contract recorded {recorded}")
            });
        }
    }

    // --- the run has not started ---
    let ledger_facts = facts(&prepared.out)?;
    if let Some(reason) = started(&ledger_facts) {
        checks.require("run.fresh", false, reason);
    } else {
        checks.require("run.fresh", true, "");
    }
    let owned_files = ps::child_files(&staging, OWNED, ".txt");
    checks.require(
        "run.fresh",
        owned_files.is_empty(),
        format!(
            "the staging folder already holds {} owned manifest(s): a plan describes a run before its first Card",
            owned_files.len()
        ),
    );

    // --- the installed release ---
    let workflow = workflow_identity(&PathBuf::from(&p.project), &variant)?;
    let contract_version = at(&contract["contracts"], "variant_version");
    checks.require_that("workflow.version", ps::eq(&at(&workflow, "version"), &contract_version), || {
        format!(
            "the installed `{}` is {}, but the prepared contract froze {contract_version}; re-prepare under the installed variant",
            at(&workflow, "name"),
            at(&workflow, "version")
        )
    });
    checks.require_that(
        "workflow.script",
        ps::eq(&at(&workflow, "recorded_sha256"), &at(&workflow, "script_sha256")),
        || {
            format!(
                "the installed script `{}` does not match its registry hash",
                at(&workflow, "script")
            )
        },
    );
    let binaries = binary_identities(&mut checks, &auth)?;

    // --- the read-only baseline and the rollback point ---
    let baseline = auth["baseline"].clone();
    let mut baseline_record = check_file_record(&mut checks, "baseline.report", &baseline["report"])?;
    if let Some(record) = baseline["addresses"].as_object() {
        baseline_record["addresses"] = check_file_record(&mut checks, "baseline.addresses", &json!(record))?;
    } else {
        checks.require("baseline.addresses", false, "baseline.addresses must name the baseline audit's `audit-addresses.txt`");
    }
    if !blank(&at(&baseline["report"], "path")) && Path::new(&at(&baseline["report"], "path")).is_file() {
        let report = util::read_json(&at(&baseline["report"], "path"))?;
        let archive_id = at(&report["baseline"]["evidence"]["archive"], "id");
        baseline_record["facts"] = json!({
            "scope": report["scope"],
            "archive_id": archive_id,
            "archive_revision": report["archive_revision"],
            "sample": report["baseline"]["evidence"]["sample"],
            "complete": report["baseline"]["complete"],
        });
    }
    let backup = &auth["backup"];
    let mut backup_record = check_file_record(&mut checks, "backup", backup)?;
    let integrity = at(backup, "integrity");
    checks.require_that("backup.integrity", ps::eq(&integrity, "ok"), || {
        format!("backup.integrity is `{integrity}`; a rollback point needs a verified `ok`")
    });
    backup_record["integrity"] = json!(integrity);
    let preserved = auth["preserved_builds"]
        .as_array()
        .context("preserved_builds: the authorization record preserves no previous build")?;
    checks.require_that("preserved_builds", !preserved.is_empty(), || {
        "preserved_builds must name the previous installed builds".to_string()
    });
    let mut preserved_records = Vec::new();
    for (index, record) in preserved.iter().enumerate() {
        let verified = check_file_record(&mut checks, "preserved_builds", record)?;
        if blank(&at(record, "version")) {
            checks.require("preserved_builds", false, format!("preserved_builds[{index}] names no version"));
        }
        preserved_records.push(verified);
    }
    let comparison = auth["comparison"].clone();
    let previous = at(&comparison, "previous_run");
    let previous_exists = !blank(&previous) && Path::new(&previous).is_dir();
    checks.require_that("comparison.previous_run", previous_exists, || {
        format!("comparison.previous_run `{previous}` is not a retained staging folder; keep the old run for comparison")
    });

    let mut checks = checks.finish("pilot plan")?;
    let archive = zt.archive_view()?;
    let read = read_archive(zt)?;
    let cards = read["cards"].as_array().cloned().unwrap_or_default();
    // The prepared root and anchors are this run's own placeholders: the approved new-Card scope
    // holds them, so the protected scope is every *other* Card that exists before launch.
    let mut prepared_cards: BTreeSet<String> = BTreeSet::new();
    prepared_cards.insert(key.clone());
    for group in contract["groups"].as_array().into_iter().flatten() {
        prepared_cards.insert(at(group, "anchor"));
    }
    let old: Vec<&Value> = cards
        .iter()
        .filter(|card| !prepared_cards.contains(&at(card, "address")))
        .collect();
    let protected = protected_view(&old.iter().map(|card| (*card).clone()).collect::<Vec<_>>());
    let mut pairs_sorted: Vec<(String, String)> = old
        .iter()
        .map(|card| (at(card, "address"), crate::calls::authored_fingerprint(card)))
        .collect();
    pairs_sorted.sort();
    let mut prepared_pairs: Vec<(String, String)> = cards
        .iter()
        .filter(|card| prepared_cards.contains(&at(card, "address")))
        .map(|card| (at(card, "address"), crate::calls::authored_fingerprint(card)))
        .collect();
    prepared_pairs.sort();
    let protected_file = Path::new(&p.work).join(PROTECTED_FILE);
    util::write_text(
        &protected_file,
        &ps::json_compact(&json!({
            "schema_version": 1,
            "contract": CONTRACT,
            "archive": archive["archive_identity"],
            "revision": archive["archive_revision"],
            "digest": pairs_digest(&all_pairs(&pairs_sorted, &prepared_pairs)),
            "cards": pairs_sorted.iter().map(|(address, fingerprint)| json!([address, fingerprint])).collect::<Vec<_>>(),
            "prepared": prepared_pairs.iter().map(|(address, fingerprint)| json!([address, fingerprint])).collect::<Vec<_>>(),
        })),
    )?;
    checks.push(json!({
        "name": "protected.scope",
        "ok": true,
        "detail": format!("{} old Cards fingerprinted into {}", pairs_sorted.len(), protected_file.display()),
    }));

    let old_digest = pairs_digest(&pairs_sorted);
    let plan_id = plan_id(&prepared.hash, &at(&archive["archive_identity"], "id"), &old_digest, &statement);
    let record = json!({
        "contract": CONTRACT,
        "step": "plan",
        "plan_id": plan_id,
        "created": canon::utc_now_o(),
        "decision": DECISION,
        "identity": {
            "run_id": ledger_facts.run_id,
            "citation_key": key,
            "variant": variant,
            "book_title": at(&contract["options"], "book_title"),
            "staging": staging,
            "source": contract["source"],
            "groups": contract["groups"],
            "group_manifest_hash": contract["group_manifest_hash"],
            "preparation_hash": prepared.hash,
            "archive": {
                "id": at(&archive["archive_identity"], "id"),
                "path": at(&archive["archive_identity"], "path"),
                "revision": archive["archive_revision"],
                "cards": pairs_sorted.len(),
            },
            "contracts": contract["contracts"],
            "settings": contract["settings"],
            "layers": contract["options"]["run_options"],
            "workflow": workflow,
            "launch": {
                "mode": at(launch, "mode"),
                "args": launch["args"],
                "invocation": format!(
                    "Workflow({{scriptPath: \"{}\", args: {}}})",
                    at(&workflow, "script"),
                    ps::json_compact(&launch["args"])
                ),
            },
        },
        "binaries": binaries,
        "approved_scope": {
            "statement": statement,
            "groups": approved.iter().collect::<Vec<_>>(),
            "old_card_edits": [],
            "prepared_cards": prepared_cards.iter().collect::<Vec<_>>(),
        },
        "backup": backup_record,
        "baseline": baseline_record,
        "preserved_builds": preserved_records,
        "comparison": {
            "previous_run": previous,
            "variant_version": at(&comparison, "variant_version"),
        },
        "protected": {
            "scope": "every old Card that exists before launch, with its authored fingerprint",
            "count": protected["count"],
            "digest": pairs_digest(&all_pairs(&pairs_sorted, &prepared_pairs)),
            "old_digest": old_digest,
            "by_workspace": protected["by_workspace"],
            "by_type": protected["by_type"],
            "models": protected["models"],
            "human": protected["human"],
            "prepared": prepared_pairs.len(),
            "file": protected_file.display().to_string(),
            "revision": archive["archive_revision"],
        },
        "fresh": launch_state(&ledger_facts),
        "checks": checks,
        "missing": [],
    });
    util::write_text(Path::new(&p.work).join(PLAN_FILE), &ps::json_pretty(&record))?;
    util::write_text(&Path::new(&p.work).join("pilot-plan.md"), &plan_markdown(&record))?;
    Ok(record)
}

/// The plan's identity: the preparation, the archive, the protected scope and the approved
/// statement. A later step compares it, so an edited plan is a different plan.
fn plan_id(preparation: &str, archive: &str, protected: &str, statement: &str) -> String {
    let digest = canon::sha256_hex(
        canon::canonical_json(&json!([CONTRACT, preparation, archive, protected, statement])).as_bytes(),
    );
    format!("p-{}", &digest[..16])
}

fn plan_markdown(record: &Value) -> String {
    let identity = &record["identity"];
    let mut out = String::new();
    out.push_str("# One-book pilot plan (S26, #117)\n\n");
    out.push_str(&format!(
        "Plan `{}` · created {} · decision **{}**\n\n",
        ps::s(&record["plan_id"]),
        ps::s(&record["created"]),
        ps::s(&record["decision"])
    ));
    out.push_str("## Identity\n\n| Fact | Value |\n| --- | --- |\n");
    for (label, value) in [
        ("Source", format!("{} (SHA-256 {})", at(&identity["source"], "path"), at(&identity["source"], "sha256"))),
        ("Citation key", at(identity, "citation_key")),
        ("Book title", at(identity, "book_title")),
        ("Variant", at(identity, "variant")),
        ("Staging", at(identity, "staging")),
        ("Groups", format!("{}", identity["groups"].as_array().map(Vec::len).unwrap_or(0))),
        ("Archive", format!("{} ({}) at revision {}", at(&identity["archive"], "id"), at(&identity["archive"], "path"), at(&identity["archive"], "revision"))),
        ("Contract", format!("base {}, variant {}", at(&identity["contracts"], "base_version"), at(&identity["contracts"], "variant_version"))),
        ("Workflow", format!("{} {} (fingerprint {})", at(&identity["workflow"], "name"), at(&identity["workflow"], "version"), at(&identity["workflow"], "fingerprint"))),
        ("Caps", format!("attempts {}, run cap {} s", at(&identity["settings"], "max_attempts"), at(&identity["settings"], "dsh_run_cap"))),
        ("Protected", format!("{} old Cards and {} prepared placeholders, old digest {}", at(&record["protected"], "count"), at(&record["protected"], "prepared"), at(&record["protected"], "old_digest"))),
        ("Backup", format!("{} ({}, SHA-256 {})", at(&record["backup"], "path"), at(&record["backup"], "integrity"), at(&record["backup"], "sha256"))),
    ] {
        out.push_str(&format!("| {label} | {value} |\n"));
    }
    out.push_str("\n## Approved new-Card scope\n\n");
    out.push_str(&format!("{}\n\n", at(&record["approved_scope"], "statement")));
    out.push_str("## Installed binaries\n\n| Binary | Version | SHA-256 |\n| --- | --- | --- |\n");
    for record in record["binaries"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "| `{}` | {} | `{}` |\n",
            at(record, "name"),
            at(record, "version"),
            at(record, "sha256")
        ));
    }
    out.push_str("\n## Launch\n\n```text\n");
    out.push_str(&at(&identity["launch"], "invocation"));
    out.push_str("\n```\n\n## Checks\n\n");
    for check in record["checks"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "- {} {} {}\n",
            if check["ok"] == json!(true) { "ok" } else { "FAILED" },
            at(check, "name"),
            at(check, "detail")
        ));
    }
    out.push_str("\nThis plan authorizes nothing by itself. The human approves the run; `preflight` then proves the launch is still the plan.\n");
    out
}

// --- preflight (S26-AC02) -------------------------------------------------------------------------

fn preflight(prepared: &Prepared, zt: &mut Zt, p: &Params) -> Result<Value> {
    let plan_path = Path::new(&p.work).join(PLAN_FILE);
    if !plan_path.is_file() {
        bail!(
            "no pilot plan at `{}`; run `zt-digest pilot -Step plan -Staging <out> -Work <work> -File <authorization.json>` first",
            plan_path.display()
        );
    }
    let plan = util::read_json(&plan_path)?;
    let mut checks = Checks::default();
    let statement = at(&plan["approved_scope"], "statement");
    let expected = plan_id(
        &at(&plan["identity"], "preparation_hash"),
        &at(&plan["identity"]["archive"], "id"),
        &at(&plan["protected"], "old_digest"),
        &statement,
    );
    checks.require_that("plan.intact", ps::eq(&expected, &at(&plan, "plan_id")), || {
        "the plan record changed since it was written; write a new plan".to_string()
    });
    checks.require_that(
        "plan.staging",
        ps::eq(&at(&plan["identity"], "staging"), &prepared.out.display().to_string()),
        || "the plan describes another staging folder".to_string(),
    );

    let ledger_facts = facts(&prepared.out)?;
    if let Some(reason) = started(&ledger_facts) {
        checks.require("run.fresh", false, reason);
    } else {
        checks.require("run.fresh", true, "");
    }
    let now = crate::zt::now_seconds();
    if let Some(reason) = live_claim(&ledger_facts, now) {
        checks.require("run.lease", false, reason);
    } else {
        checks.require("run.lease", true, "");
    }
    checks.require_that(
        "run.staging_clean",
        ps::child_files(&prepared.out.display().to_string(), OWNED, ".txt").is_empty(),
        || format!("`{}` already holds this run's owned manifests", prepared.out.display()),
    );
    let link = link_prerequisite(&ledger_facts);
    let broken: Vec<String> = link["link_units"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|unit| {
            let group = at(unit, "group");
            !unit["needs"].as_array().into_iter().flatten().any(|need| {
                at(need, "needs") == format!("digest/g{group:0>3}@1") && at(need, "condition") == "success"
            })
        })
        .map(|unit| at(unit, "unit"))
        .collect();
    checks.require_that("link.prerequisite", broken.is_empty(), || {
        format!(
            "these Link units are registered without the `success` prerequisite on their group's digest unit: {}",
            broken.join(", ")
        )
    });
    checks.require_that(
        "link.check_covers_groups",
        link["link_check_needs"].as_array().map(Vec::len).unwrap_or(0)
            >= prepared.contract["groups"].as_array().map(Vec::len).unwrap_or(0),
        || "`link-check/book@1` does not cover every prepared group".to_string(),
    );

    // The rollback point and the installed release, verified again at launch time.
    check_file_record(&mut checks, "backup", &plan["backup"])?;
    for record in plan["preserved_builds"].as_array().into_iter().flatten() {
        check_file_record(&mut checks, "preserved_builds", record)?;
    }
    let workflow = workflow_identity(&Path::new(&p.project).to_path_buf(), &at(&prepared.contract, "variant"))?;
    checks.require_that(
        "workflow.script",
        ps::eq(&at(&workflow, "script_sha256"), &at(&plan["identity"]["workflow"], "script_sha256")),
        || "the installed frozen script changed since the plan".to_string(),
    );
    let mut binaries_ok = true;
    for record in plan["binaries"].as_array().into_iter().flatten() {
        let path = at(record, "path");
        match file_identity(Path::new(&path)) {
            Ok((sha, _)) if ps::eq(&sha, &at(record, "sha256")) => {}
            Ok((sha, _)) => {
                binaries_ok = false;
                checks.require(
                    "binaries.unchanged",
                    false,
                    format!("`{path}` is now {sha}, not the recorded {}", at(record, "sha256")),
                );
            }
            Err(error) => {
                binaries_ok = false;
                checks.require("binaries.unchanged", false, format!("{error:#}"));
            }
        }
    }
    checks.require("binaries.unchanged", binaries_ok, "");

    // The archive and the protected scope.
    let archive = zt.archive_view()?;
    checks.require_that(
        "archive.identity",
        ps::eq(&at(&archive["archive_identity"], "id"), &at(&plan["identity"]["archive"], "id")),
        || "the service answers for another archive".to_string(),
    );
    checks.require_that(
        "archive.revision",
        archive["archive_revision"].as_u64() == plan["identity"]["archive"]["revision"].as_u64(),
        || format!(
            "the archive is at revision {}, not the plan's {}; the authorized scope moved, so write a new plan",
            archive["archive_revision"],
            plan["identity"]["archive"]["revision"]
        ),
    );
    let read = read_archive(zt)?;
    let cards = read["cards"].as_array().cloned().unwrap_or_default();
    let after: Vec<(String, String)> = cards
        .iter()
        .map(|card| (at(card, "address"), crate::calls::authored_fingerprint(card)))
        .collect();
    let mut after_sorted = after.clone();
    after_sorted.sort();
    checks.require_that(
        "protected.unchanged",
        pairs_digest(&after_sorted) == at(&plan["protected"], "digest"),
        || "a protected Card changed since the plan; no launch over a moved scope".to_string(),
    );

    let (ok, checks, problems) = checks.reported();
    let record = json!({
        "contract": CONTRACT,
        "step": "preflight",
        "plan_id": plan["plan_id"],
        "created": canon::utc_now_o(),
        "ok": ok,
        "problems": problems,
        "archive": {"id": archive["archive_identity"]["id"], "revision": archive["archive_revision"]},
        "run": launch_state(&ledger_facts),
        "link": link,
        "checks": checks,
    });
    util::write_text(Path::new(&p.work).join(PREFLIGHT_FILE), &ps::json_pretty(&record))?;
    Ok(record)
}

// --- qualify (S26-AC03 – S26-AC08) ----------------------------------------------------------------

fn qualify(prepared: &Prepared, zt: &mut Zt, p: &Params) -> Result<Value> {
    let plan_path = Path::new(&p.work).join(PLAN_FILE);
    if !plan_path.is_file() {
        bail!(
            "no pilot plan at `{}`; qualify a pilot against the plan that authorized it",
            plan_path.display()
        );
    }
    let plan = util::read_json(&plan_path)?;
    let protected_path = Path::new(&p.work).join(PROTECTED_FILE);
    if !protected_path.is_file() {
        bail!("no protected scope at `{}`; the plan's baseline is gone", protected_path.display());
    }
    let protected = util::read_json(&protected_path)?;
    let before = pairs_of(&protected);
    let prepared_before = pairs_of_key(&protected, "prepared");
    let prepared_addresses: BTreeSet<String> = prepared_before.iter().map(|(address, _)| address.clone()).collect();
    let before_addresses: BTreeSet<String> = before.iter().map(|(address, _)| address.clone()).collect();

    let run_report = if blank(&p.run_report) {
        prepared.out.join("run-report.json").display().to_string()
    } else {
        p.run_report.clone()
    };
    if !Path::new(&run_report).is_file() {
        bail!(
            "no run report at `{run_report}`; a qualification reads the run's own verified report, never a simulation"
        );
    }
    let report = util::read_json(&run_report)?;
    if report["identity"]["run_id"].as_str().is_none() && report["run_id"].as_str().is_none() {
        bail!("`{run_report}` is not a verified run report: it names no run");
    }
    let report_sha = file_identity(Path::new(&run_report))?.0;

    let archive = zt.archive_view()?;
    let read = read_archive(zt)?;
    let cards = read["cards"].as_array().cloned().unwrap_or_default();
    let after: Vec<(String, String)> = cards
        .iter()
        .map(|card| (at(card, "address"), crate::calls::authored_fingerprint(card)))
        .collect();
    let mut after_sorted = after.clone();
    after_sorted.sort();
    let after_by_address: BTreeMap<String, String> = after.into_iter().collect();
    // The old scope is compared on its own addresses: the run's own prepared placeholders and its
    // new Cards are its authorized work, not old-Card changes (S26-AC06).
    let old_after: Vec<(String, String)> = before
        .iter()
        .filter_map(|(address, _)| after_by_address.get(address).map(|now| (address.clone(), now.clone())))
        .collect();
    let diff = diff_protected(&before, &old_after);

    let facts = facts(&prepared.out)?;
    let mut checks = Checks::default();
    checks.require(
        "scope.no_old_change",
        diff["changed"].as_array().map(Vec::len).unwrap_or(0) == 0
            && diff["removed"].as_array().map(Vec::len).unwrap_or(0) == 0,
        "no old Card changed, and none was removed",
    );

    // --- the new Cards, their reviews, filing and Link ownership ---
    let book = at(&prepared.contract, "citation_key");
    let new_addresses: Vec<String> = after_sorted
        .iter()
        .map(|(address, _)| address.clone())
        .filter(|address| !before_addresses.contains(address))
        .collect();
    let mut new_cards = Vec::new();
    let by_address: BTreeMap<String, &Value> =
        cards.iter().map(|card| (at(card, "address"), card)).collect();
    let owned = owned_manifests(&prepared.out)?;
    let reviews = reviews_by_address(&p.cards, &prepared.out)?;
    let mut unreviewed = Vec::new();
    let mut unowned = Vec::new();
    let mut earlier_filed = Vec::new();
    for address in &new_addresses {
        let Some(card) = by_address.get(address) else { continue };
        let owner = owned.iter().find(|(_, list)| list.contains(address)).map(|(group, _)| *group);
        let review = reviews.get(address).cloned();
        if review.is_none() {
            unreviewed.push(address.clone());
        }
        if owner.is_none() {
            unowned.push(address.clone());
        }
        let parent = ps::s(&card["parent_address"]);
        // Filed beside earlier thought: the parent is a Card that existed before the launch, so
        // the new Card continues an earlier line instead of a placeholder of this run.
        let earlier = !blank(&parent) && before_addresses.contains(&parent);
        if earlier {
            earlier_filed.push(address.clone());
        }
        new_cards.push(json!({
            "address": address,
            "tree": at(card, "tree"),
            "type": at(card, "type"),
            "workspace": at(card, "workspace"),
            "prepared": prepared_addresses.contains(address),
            "group": owner,
            "parent": if blank(&parent) { Value::Null } else { json!(parent) },
            "earlier_filing": earlier,
            "review": review.unwrap_or(Value::Null),
        }));
    }
    checks.require_that("cards.reviewed", unreviewed.is_empty(), || {
        format!(
            "{} new Cards have no recorded review outcome: {}",
            unreviewed.len(),
            unreviewed.iter().take(8).cloned().collect::<Vec<_>>().join(", ")
        )
    });
    checks.require_that("cards.owned", unowned.is_empty(), || {
        format!(
            "{} new Cards are named by no group's owned manifest: {}",
            unowned.len(),
            unowned.iter().take(8).cloned().collect::<Vec<_>>().join(", ")
        )
    });

    // --- the run's own Link completions, group by group ---
    let groups = group_results(&prepared.out, prepared.contract["groups"].as_array().map(Vec::len).unwrap_or(0), &facts)?;
    let missing_links: Vec<i64> = groups
        .iter()
        .filter(|group| group["link_completion"].is_null())
        .map(|group| group["n"].as_i64().unwrap_or(0))
        .collect();
    let link_faults: i64 = groups
        .iter()
        .filter_map(|group| group["link_quality"]["faults"].as_i64())
        .sum();

    // --- the audit's measured baseline ---
    let audit_path = if blank(&p.audit) { String::new() } else { p.audit.clone() };
    let measures = if audit_path.is_empty() {
        // A missing audit is stated as an unknown measure with its reason: never a zero.
        json!({
            "read": false,
            "complete": false,
            "reason": "no -Audit: the measured baseline is unknown",
            "own_source": {
                "known": false,
                "target": 30.0,
                "under_target": Value::Null,
                "statement": "the measured Topic dependence is unknown: no audit report was supplied",
            },
            "restatements": {"known": false},
            "cross_book": {"known": false},
            "tails": {"known": false},
            "provider": {"known": false},
            "book": {"found": false},
            "missing": [{"kind": "audit", "reason": "no -Audit was supplied"}],
        })
    } else {
        measure_section(&audit_path, &book)?
    };

    // --- the interruption facts ---
    let interruption = interruption_section(&facts, &prepared.out, &run_report);

    // --- old-data proposals: never applied here ---
    let cycle = card_report(&p.cards, &prepared.out);
    let proposals = proposals_section(&report, &diff, cycle.as_ref());
    let restatements = cycle
        .as_ref()
        .map(|report| {
            json!({
                "read": report["restatement"].is_object(),
                "counts": report["restatement"]["counts"],
                "complete": report["restatement"]["complete"],
                "path": report["restatement"]["path"],
            })
        })
        .unwrap_or_else(|| json!({"read": false, "reason": "no Card cycle report: the restatement resolution is unknown"}));

    let sample = review_sample(&new_cards, p.sample);
    let request = format!(
        "Review the representative pilot Cards ({}): accept, correct or defer each with evidence, and approve or correct the measured Topic dependence ({}). A partial pilot stays partial.",
        sample["addresses"].as_array().map(Vec::len).unwrap_or(0),
        measures["own_source"]["statement"].clone()
    );
    let mut checks_out = Vec::new();
    checks_out.append(&mut (checks.0));
    let completeness = json!({
        "settled": report["completeness"]["settled"],
        "content_complete": report["completeness"]["content_complete"],
        "quality_complete": report["completeness"]["quality_complete"],
        "cleanup_complete": report["completeness"]["cleanup_complete"],
        "complete": report["completeness"]["complete"],
        "quality_reason": report["completeness"]["quality_reason"],
    });
    let record = json!({
        "contract": CONTRACT,
        "step": "qualify",
        "plan_id": plan["plan_id"],
        "created": canon::utc_now_o(),
        "decision": DECISION,
        "run": {
            "report": {"path": run_report, "sha256": report_sha},
            "identity": report["identity"],
            "counts": report["counts"],
            "completeness": completeness,
            "gaps": report["gaps"],
            "usage": report["usage"],
            "timing": report["timing"],
            "pilot": prepared.contract["citation_key"],
        },
        "scope": {
            "protected": before.len(),
            "new": new_addresses.iter().filter(|address| !prepared_addresses.contains(*address)).count(),
            "prepared": prepared_addresses.len(),
            "run_scope": new_addresses.len(),
            "changed": diff["changed"],
            "removed": diff["removed"],
            "new_by_workspace": count_by(&new_cards, "workspace"),
            "new_by_type": count_by(&new_cards, "type"),
            "new_by_group": count_by(&new_cards, "group"),
            "unowned": unowned,
        },
        "cards": {
            "new": new_cards,
            "reviewed": new_addresses.len().saturating_sub(unreviewed.len()),
            "unreviewed": unreviewed,
            "verdicts": verdict_counts(&reviews),
            "earlier_filing": earlier_filed,
            "groups": groups,
            "link_completions_missing": missing_links,
            "link_faults": link_faults,
        },
        "measures": measures,
        "interruption": interruption,
        "old_data": {
            "changed": diff["changed"],
            "removed": diff["removed"],
            "restatements": restatements,
            "proposals": proposals,
            "applied": [],
        },
        "review": {
            "sample": sample,
            "decision": DECISION,
            "requested": request,
        },
        "archive": {"id": archive["archive_identity"]["id"], "revision": archive["archive_revision"]},
        "checks": checks_out,
    });
    let record = qualify_checks(record, &groups, &missing_links, &measures);
    util::write_text(Path::new(&p.work).join(QUALIFICATION_FILE), &ps::json_pretty(&record))?;
    util::write_text(&Path::new(&p.work).join("pilot-qualification.md"), &qualification_markdown(&record))?;
    Ok(record)
}

/// The gate's own consistency checks, kept apart from the run's facts: a qualification that
/// cannot name a per-Card outcome says so instead of reporting a complete pilot.
fn qualify_checks(mut record: Value, groups: &[Value], missing_links: &[i64], measures: &Value) -> Value {
    let mut checks = Vec::new();
    let unreviewed = record["cards"]["unreviewed"].as_array().map(Vec::len).unwrap_or(0);
    checks.push(json!({"name": "cards.reviewed", "ok": unreviewed == 0, "detail": if unreviewed == 0 { String::new() } else { format!("{unreviewed} new Cards have no review outcome") }}));
    checks.push(json!({"name": "link.ownership", "ok": missing_links.is_empty(), "detail": if missing_links.is_empty() { String::new() } else { format!("groups without a Link completion: {missing_links:?}") }}));
    let groups_done = groups.iter().filter(|group| group["digest_state"] == json!("done")).count();
    checks.push(json!({"name": "groups.done", "ok": groups_done == groups.len(), "detail": format!("{groups_done} of {} groups are done", groups.len())}));
    let unknown_measures: Vec<String> = ["own_source", "restatements", "tails", "provider"]
        .into_iter()
        .filter(|name| measures[*name]["known"] != json!(true))
        .map(str::to_string)
        .collect();
    checks.push(json!({"name": "measures.known", "ok": unknown_measures.is_empty(), "detail": if unknown_measures.is_empty() { String::new() } else { format!("unmeasured: {}", unknown_measures.join(", ")) }}));
    if let Some(list) = record["checks"].as_array_mut() {
        list.extend(checks.clone());
    }
    record["verdict"] = json!(if checks.iter().all(|check| check["ok"] == json!(true)) { "evidence_complete" } else { "partial" });
    record
}

fn count_by(cards: &[Value], field: &str) -> BTreeMap<String, i64> {
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    for card in cards {
        *counts.entry(ps::s(&card[field])).or_default() += 1;
    }
    counts
}

fn verdict_counts(reviews: &BTreeMap<String, Value>) -> BTreeMap<String, i64> {
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    for review in reviews.values() {
        *counts.entry(at(review, "verdict")).or_default() += 1;
    }
    counts
}

/// The group's own retained evidence: its digest state, its owned manifest and the Link
/// completion the Link pass published. A missing Link completion is a named gap.
fn group_results(out: &Path, groups: usize, facts: &Facts) -> Result<Vec<Value>> {
    let mut results = Vec::new();
    for number in 1..=groups {
        let id = format!("digest/g{number:03}@1");
        let owned = ps::read_lines(&ps::join_path(&out.display().to_string(), &format!("{OWNED}{number:02}.txt")))
            .map(|lines| lines.into_iter().map(|line| line.trim().to_string()).filter(|line| !line.is_empty()).collect::<Vec<_>>())
            .unwrap_or_default();
        let completion_path = out.join(format!("link-completion-g{number:03}.json"));
        let completion = if completion_path.is_file() {
            Some(util::read_json(&completion_path)?)
        } else {
            None
        };
        let quality = completion
            .as_ref()
            .map(|completion| completion["quality"].clone())
            .unwrap_or(Value::Null);
        let state = facts
            .units
            .iter()
            .find(|unit| ps::eq(&at(unit, "id"), &id))
            .map(|unit| at(unit, "state"))
            .unwrap_or_default();
        results.push(json!({
            "n": number,
            "unit": id,
            "digest_state": state,
            "owned": owned.len(),
            "link_completion": completion.as_ref().map(|value| json!({"path": completion_path.display().to_string(), "counts": value["counts"]})).unwrap_or(Value::Null),
            "link_quality": quality,
        }));
    }
    Ok(results)
}

/// The Cards a group's own receipts name, by group number.
fn owned_manifests(out: &Path) -> Result<Vec<(i64, BTreeSet<String>)>> {
    let mut owned = Vec::new();
    let directory = out.display().to_string();
    for name in ps::child_files(&directory, OWNED, ".txt") {
        let number: i64 = name
            .trim_start_matches(OWNED)
            .trim_end_matches(".txt")
            .trim_start_matches('0')
            .parse()
            .unwrap_or(0);
        let list: BTreeSet<String> = ps::read_lines(&ps::join_path(&directory, &name))?
            .into_iter()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        owned.push((number, list));
    }
    Ok(owned)
}

/// The Card cycle's retained report: one outcome per reviewed Card plus the run's restatement
/// resolution. A missing report is an unknown, never a pass.
fn card_report(named: &str, out: &Path) -> Option<Value> {
    let path = if !blank(named) {
        PathBuf::from(named)
    } else {
        out.join("cards").join("report.json")
    };
    if !path.is_file() {
        return None;
    }
    util::read_json(&path).ok()
}

/// The Card cycle's review records: one outcome per reviewed Card.
fn reviews_by_address(named: &str, out: &Path) -> Result<BTreeMap<String, Value>> {
    let Some(report) = card_report(named, out) else {
        return Ok(BTreeMap::new());
    };
    let mut reviews = BTreeMap::new();
    for record in report["review"].as_array().into_iter().flatten() {
        let address = at(record, "address");
        if blank(&address) {
            continue;
        }
        reviews.insert(
            address,
            json!({
                "verdict": at(record, "verdict"),
                "kind": at(record, "kind"),
                "category": at(record, "category"),
                "fingerprint": at(record, "fingerprint"),
            }),
        );
    }
    Ok(reviews)
}

/// The measured baseline: the audit's own `baseline` section, restated with the pilot's readings.
/// An unknown measure stays unknown, and the Topic target is compared only when the rate exists.
fn measure_section(path: &str, book: &str) -> Result<Value> {
    if !Path::new(path).is_file() {
        bail!("no audit report at `{path}`; the measured baseline cannot be read");
    }
    let report = util::read_json(path)?;
    let section = &report["baseline"];
    let measures = &section["measures"];
    let own = &measures["P1"];
    let rate = own["rate"]["per_100"].as_f64();
    let target = own["target"].as_f64().unwrap_or(30.0);
    let own_known = own["known"] == json!(true);
    let own_source = json!({
        "known": own_known,
        "reason": own["reason"],
        "value": own["values"]["value"],
        "denominator": own["values"]["denominator"],
        "with_rests_on": own["values"]["with_rests_on"],
        "unknown_source": own["values"]["unknown_source"],
        "per_100": own["rate"]["per_100"],
        "target": target,
        "under_target": rate.map(|value| value < target),
        "addresses": own["values"]["addresses"],
        "statement": if own_known {
            format!(
                "{} of {} new Topic Idea Cards rest on their own book (per 100: {}; target below {target}); unknown-source Cards are counted apart",
                own["values"]["value"].as_i64().unwrap_or(0),
                own["values"]["denominator"].as_i64().unwrap_or(0),
                own["rate"]["per_100"].as_f64().map(|value| format!("{value:.2}")).unwrap_or_else(|| "unknown".to_string()),
            )
        } else {
            format!(
                "the measured Topic dependence is unknown: {}",
                own["reason"].as_str().unwrap_or("the audit could not compute it")
            )
        },
    });
    // The pilot book's own Card-language and Link-layer faults, from the same audit read.
    let pilot_book = report["books"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|row| ps::eq(&at(row, "key"), book))
        .cloned()
        .unwrap_or(Value::Null);
    Ok(json!({
        "path": path,
        "sha256": file_identity(Path::new(path))?.0,
        "complete": section["complete"],
        "read": true,
        "own_source": own_source,
        "restatements": {
            "known": measures["P2"]["known"] == json!(true),
            "values": measures["P2"]["values"],
            "rate": measures["P2"]["rate"],
        },
        "cross_book": {
            "known": measures["P3_P5"]["known"] == json!(true),
            "values": measures["P3_P5"]["values"],
        },
        "tails": {
            "known": measures["P4"]["known"] == json!(true),
            "values": measures["P4"]["tails"],
        },
        "provider": measures["P4"]["run"].clone(),
        "sources": {
            "cards": section["sources"]["cards"],
            "direct": section["sources"]["direct"],
            "differences": section["sources"]["differences"]["total"],
            "human": section["sources"]["human"],
        },
        "book": {
            "found": !pilot_book.is_null(),
            "cards": pilot_book["cards"],
            "style": pilot_book["style"],
            "layer": pilot_book["layer"],
            "orphans": pilot_book["orphans"],
            "unkinded": pilot_book["unkinded"],
            "models": pilot_book["models"],
        },
        "missing": section["evidence"]["missing"],
    }))
}

/// The supported interruption: the fault, the one resume and the redeemed receipt, read from the
/// run's own ledger events and retained group jobs. No resume is reported as exercised unless
/// the events show it, and a run without one says so.
fn interruption_section(facts: &Facts, out: &Path, run_report: &str) -> Value {
    let mut faults = Vec::new();
    let mut resets = Vec::new();
    let mut by_name: BTreeMap<String, i64> = BTreeMap::new();
    for event in &facts.events {
        let name = at(event, "event");
        *by_name.entry(name.clone()).or_default() += 1;
        if name.contains("fail") {
            faults.push(json!({"unit": event["unit"], "event": name, "facts": event["facts"]}));
        }
        if name.contains("reset") || name.contains("resume") {
            resets.push(json!({"unit": event["unit"], "event": name, "invocation": event["invocation"], "facts": event["facts"]}));
        }
    }
    let mut recovered = Vec::new();
    let jobs = out.join("group-jobs");
    if jobs.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&jobs) {
            for entry in entries.flatten() {
                let start = entry.path().join("start.json");
                if !start.is_file() {
                    continue;
                }
                if let Ok(record) = util::read_json(&start) {
                    if record["recovered"] == json!(true) || record["recovery"]["reason"] == json!("receipts_committed") {
                        recovered.push(json!({"group": ps::leaf(&entry.path().display().to_string()), "recovery": record["recovery"]}));
                    }
                }
            }
        }
    }
    let resume_reported = {
        let path = Path::new(run_report);
        util::read_json(path)
            .ok()
            .map(|report| report["resume"].clone())
            .unwrap_or(Value::Null)
    };
    json!({
        "resume_count": facts.resume_count,
        "invocation": facts.invocation,
        "events": by_name,
        "faults": faults,
        "resets": resets,
        "recovered_groups": recovered,
        "run_report_resume": resume_reported,
        "exercised": facts.resume_count > 0,
        "reason": if facts.resume_count > 0 { String::new() } else { "no resume is recorded: the supported interruption was not exercised".to_string() },
    })
}

/// The concrete old-data proposals: every old-Card change the run itself would need (a recorded
/// merge that needs an existing Card rewritten, an accepted Bridge proposal, a register plan).
/// The gate copies their evidence; it applies none of them.
fn proposals_section(report: &Value, diff: &Value, cycle: Option<&Value>) -> Vec<Value> {
    let mut proposals = Vec::new();
    for row in report["gaps"].as_array().into_iter().flatten() {
        let kind = at(row, "kind");
        if kind.contains("existing") || kind.contains("bridge") || kind.contains("register") || kind.contains("hub") {
            proposals.push(json!({"kind": kind, "unit": row["unit"], "detail": row["reason"]}));
        }
    }
    if let Some(counts) = cycle.map(|report| &report["restatement"]["counts"]) {
        let pending = counts["pending_existing_card_change"].as_i64().unwrap_or(0);
        if pending > 0 {
            proposals.push(json!({
                "kind": "restatement-pending-existing-card-change",
                "count": pending,
                "detail": format!("{pending} recorded merge verdict(s) wait for the separate existing-Card review gate"),
            }));
        }
        let unresolved = counts["unresolved"].as_i64().unwrap_or(0);
        if unresolved > 0 {
            proposals.push(json!({
                "kind": "restatement-unresolved",
                "count": unresolved,
                "detail": format!("{unresolved} restatement pair(s) have no resolution"),
            }));
        }
    }
    for address in diff["changed"].as_array().into_iter().flatten() {
        proposals.push(json!({"kind": "old-card-change", "address": address, "detail": "the Card's authored fingerprint changed outside the approved new-Card scope"}));
    }
    proposals
}

/// A deterministic representative sample for the human review: the first Card of every group,
/// one Model Card and one anchor when they exist, then address order. It is a sample, never a
/// proof for every Card.
fn review_sample(cards: &[Value], size: usize) -> Value {
    let mut picked: Vec<String> = Vec::new();
    let push = |picked: &mut Vec<String>, address: String| {
        if picked.len() < size && !picked.contains(&address) {
            picked.push(address);
        }
    };
    let mut groups_seen: BTreeSet<i64> = BTreeSet::new();
    for card in cards {
        if let Some(group) = card["group"].as_i64() {
            if groups_seen.insert(group) {
                push(&mut picked, ps::s(&card["address"]));
            }
        }
    }
    for card in cards {
        if at(card, "type") == "model" {
            push(&mut picked, ps::s(&card["address"]));
        }
    }
    for card in cards {
        if card["parent"].is_null() {
            push(&mut picked, ps::s(&card["address"]));
        }
    }
    for card in cards {
        push(&mut picked, ps::s(&card["address"]));
    }
    json!({"seed": 1, "size": picked.len(), "addresses": picked, "proves": "sample"})
}

fn qualification_markdown(record: &Value) -> String {
    let mut out = String::new();
    out.push_str("# One-book pilot qualification (S26, #117)\n\n");
    out.push_str(&format!(
        "Plan `{}` · {} · decision **{}** · verdict **{}**\n\n",
        ps::s(&record["plan_id"]),
        ps::s(&record["created"]),
        ps::s(&record["decision"]),
        ps::s(&record["verdict"])
    ));
    out.push_str("## The run\n\n| Fact | Value |\n| --- | --- |\n");
    let completeness = &record["run"]["completeness"];
    for (label, value) in [
        ("Citation key".to_string(), at(&record["run"], "pilot")),
        ("Run id".to_string(), at(&record["run"]["identity"], "run_id")),
        ("Invocation".to_string(), at(&record["run"]["identity"], "invocation")),
        ("Complete".to_string(), flag(&completeness["complete"])),
        ("Quality complete".to_string(), flag(&completeness["quality_complete"])),
        ("Cleanup complete".to_string(), flag(&completeness["cleanup_complete"])),
        ("New Cards".to_string(), at(&record["scope"], "new")),
        ("Run-scope Cards".to_string(), at(&record["scope"], "run_scope")),
        ("Old Cards".to_string(), at(&record["scope"], "protected")),
        ("Old Cards changed".to_string(), format!("{}", record["scope"]["changed"].as_array().map(Vec::len).unwrap_or(0))),
    ] {
        out.push_str(&format!("| {label} | {value} |\n"));
    }
    out.push_str("\n## Measured Topic dependence\n\n");
    out.push_str(&format!("{}\n\n", at(&record["measures"]["own_source"], "statement")));
    out.push_str(&format!(
        "Under the target: {} · measure known: {} · audit complete: {}\n\n",
        flag(&record["measures"]["own_source"]["under_target"]),
        flag(&record["measures"]["own_source"]["known"]),
        flag(&record["measures"]["complete"]),
    ));
    out.push_str("## Interruption and recovery\n\n");
    out.push_str(&format!(
        "Resume count {} · exercised {} · recovered groups {}\n\n{}\n\n",
        at(&record["interruption"], "resume_count"),
        flag(&record["interruption"]["exercised"]),
        record["interruption"]["recovered_groups"].as_array().map(Vec::len).unwrap_or(0),
        at(&record["interruption"], "reason"),
    ));
    out.push_str("## Old-data proposals (never applied here)\n\n");
    out.push_str(&format!(
        "Restatement resolution read: {} · pending existing-Card change {} · unresolved {}\n\n",
        flag(&record["old_data"]["restatements"]["read"]),
        at(&record["old_data"]["restatements"]["counts"], "pending_existing_card_change"),
        at(&record["old_data"]["restatements"]["counts"], "unresolved"),
    ));
    let proposals = record["old_data"]["proposals"].as_array().cloned().unwrap_or_default();
    if proposals.is_empty() {
        out.push_str("None recorded by this run.\n\n");
    } else {
        for proposal in &proposals {
            let subject = if !blank(&at(proposal, "address")) {
                at(proposal, "address")
            } else {
                at(proposal, "unit")
            };
            out.push_str(&format!(
                "- `{}` {} — {}\n",
                at(proposal, "kind"),
                subject,
                at(proposal, "detail")
            ));
        }
        out.push('\n');
    }
    out.push_str("## The decision\n\n");
    out.push_str(&at(&record["review"], "requested"));
    out.push_str("\n\nA partial pilot stays partial: a missing review outcome or an unknown measure is named above, never filled.\n");
    out
}

/// A JSON flag as a word a reader can scan, never PowerShell's `Bool(false)`.
fn flag(value: &Value) -> String {
    match value {
        Value::Bool(true) => "yes".to_string(),
        Value::Bool(false) => "no".to_string(),
        Value::Null => "unknown".to_string(),
        other => ps::s(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(address: &str, fingerprint: &str) -> (String, String) {
        (address.to_string(), fingerprint.to_string())
    }

    #[test]
    fn the_plan_id_covers_the_scope_it_authorizes() {
        let one = plan_id("prep", "archive", "digest", "one book");
        assert_eq!(one, plan_id("prep", "archive", "digest", "one book"));
        assert!(one.starts_with("p-") && one.len() == 18);
        assert_ne!(one, plan_id("prep", "archive", "digest", "another book"));
        assert_ne!(one, plan_id("prep", "archive", "other", "one book"));
    }

    #[test]
    fn a_changed_protected_card_is_named_not_counted() {
        let before = vec![pair("1/0", "a"), pair("1/1", "b"), pair("2/1", "c")];
        let after = vec![pair("1/0", "a"), pair("1/1", "changed"), pair("3/1", "new")];
        let diff = diff_protected(&before, &after);
        assert_eq!(diff["added"], json!(["3/1"]));
        assert_eq!(diff["changed"], json!(["1/1"]));
        assert_eq!(diff["removed"], json!(["2/1"]));
    }

    #[test]
    fn the_protected_digest_is_the_sorted_card_pairs() {
        let mut one = vec![pair("1/1", "b"), pair("1/0", "a")];
        let mut two = vec![pair("1/0", "a"), pair("1/1", "b")];
        one.sort();
        two.sort();
        assert_eq!(pairs_digest(&one), pairs_digest(&two));
        let mut moved = two.clone();
        moved[0].1 = "changed".into();
        assert_ne!(pairs_digest(&one), pairs_digest(&moved));
    }

    #[test]
    fn the_protected_view_counts_the_two_workspaces_and_the_models() {
        let cards = vec![
            json!({"address":"1/0","workspace":"human","type":"idea","title":"a","body":"b"}),
            json!({"address":"2/1","workspace":"agent","type":"model","title":"m","body":"n"}),
            json!({"address":"2/1|a","workspace":"agent","type":"model","title":"p","body":"q"}),
        ];
        let view = protected_view(&cards);
        assert_eq!(view["count"], json!(3));
        assert_eq!(view["by_workspace"]["human"], json!(1));
        assert_eq!(view["models"], json!(2));
        assert_eq!(view["human"], json!(1));
    }

    #[test]
    fn a_launch_that_moved_is_refused_with_its_units() {
        let facts = |invocation: i64, state: &str| Facts {
            run_id: "r".into(),
            invocation,
            resume_count: 0,
            created_at: String::new(),
            units: vec![json!({"id":"digest/g001@1","state":state,"lease_until":0,"owner":""})],
            deps: Vec::new(),
            events: Vec::new(),
        };
        assert!(started(&facts(1, "registered")).is_none());
        assert!(started(&facts(1, "ready")).is_none());
        assert!(started(&facts(2, "registered")).unwrap().contains("invocation 2"));
        assert!(started(&facts(1, "done")).unwrap().contains("digest/g001@1 is done"));
    }

    #[test]
    fn a_live_foreign_claim_blocks_the_launch() {
        let now = 1_000u64;
        let mut facts = Facts {
            run_id: "r".into(),
            invocation: 1,
            resume_count: 0,
            created_at: String::new(),
            units: vec![json!({"id":"digest/g001@1","state":"claimed","lease_until":now + 60,"owner":"group 1 a1"})],
            deps: Vec::new(),
            events: Vec::new(),
        };
        assert!(live_claim(&facts, now).is_some());
        facts.units[0]["lease_until"] = json!(now - 1);
        assert!(live_claim(&facts, now).is_none());
    }

    #[test]
    fn a_link_unit_without_its_group_prerequisite_is_reported() {
        let facts = Facts {
            run_id: "r".into(),
            invocation: 1,
            resume_count: 0,
            created_at: String::new(),
            units: vec![
                json!({"id":"link/g001@1","state":"registered","lease_until":0,"owner":""}),
                json!({"id":"link-check/book@1","state":"registered","lease_until":0,"owner":""}),
            ],
            deps: vec![json!({"unit":"link/g001@1","needs":"digest/g001@1","condition":"success"})],
            events: Vec::new(),
        };
        let link = link_prerequisite(&facts);
        assert_eq!(link["link_units_total"], json!(1));
        assert_eq!(link["link_units"][0]["needs"][0]["condition"], json!("success"));
    }

    #[test]
    fn the_review_sample_is_deterministic_and_bounded() {
        let cards = vec![
            json!({"address":"1/1","group":1,"type":"idea","parent":null}),
            json!({"address":"1/2","group":1,"type":"idea","parent":"1/1"}),
            json!({"address":"2/1","group":2,"type":"model","parent":null}),
            json!({"address":"2/1|a","group":2,"type":"model","parent":"2/1"}),
        ];
        let sample = review_sample(&cards, 2);
        assert_eq!(sample["size"], json!(2));
        assert_eq!(sample["proves"], json!("sample"));
        let again = review_sample(&cards, 2);
        assert_eq!(sample, again);
        assert_eq!(sample["addresses"][0], json!("1/1"));
    }

    #[test]
    fn a_refused_step_names_every_failed_check() {
        let mut checks = Checks::default();
        checks.require("one", false, "the first problem");
        checks.require("two", true, "");
        checks.require("three", false, "the second problem");
        let error = checks.finish("pilot plan").unwrap_err().to_string();
        assert!(error.contains("one: the first problem"));
        assert!(error.contains("three: the second problem"));
        assert!(!error.contains("two:"));
    }
}
