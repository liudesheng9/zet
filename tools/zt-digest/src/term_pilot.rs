//! `zt-digest term-pilot`: the copied-book Term pilot gate (S30, issue #172; design §18.6).
//!
//! The gate qualifies one pilot of the Term work on a **copy of the box**. It has four steps and
//! writes only under `-Work`; it never writes a Card, a Term link, a Lock or an archive row:
//!
//! ```text
//! zt-digest term-pilot -Step plan      -Staging <prepared run> -Work <folder> -File <authorization.json>
//! zt-digest term-pilot -Step preflight -Staging <prepared run> -Work <folder> [-File <authorization.json>]
//! zt-digest term-pilot -Step qualify   -Staging <prepared run> -Work <folder>
//!                                      [-RunReport <run-report.json>] [-SecondPlan <plan.json>]
//!                                      [-ClaudeUsage <token-usage.json>]
//! zt-digest term-pilot -Step record    -Staging <prepared run> -Work <folder> -Decision <review.json>
//! ```
//!
//! `plan` (S30-AC01) verifies and records the pilot's identity before the run: the copied box and
//! the live snapshot it was copied from (`copy-tree.mjs`'s `live-tree.json` and `report.json`), the
//! book tree's address-by-address fingerprint, the prepared run and its frozen contract, the source
//! and the Term list's exact authored title/body, the installed frozen Workflow and the three
//! binaries, and the before state of every Card (bodies and the checker's diagnostics). It refuses
//! a plan that misses or fails to verify any of them. The live archive is never opened: its
//! identity and revision come from the authorization and the retained copy snapshot.
//!
//! `preflight` (S30-AC01/AC05) refuses a launch after the copy, the box, the prepared run, the
//! source, the list, the binaries or the frozen Workflow moved, a locked box tree, a box that is
//! not pinned by `<staging>\zt-box.txt`, and a tree that already published Term Cards. It keeps its
//! record with `ok: false` and exits 1, so a refused launch keeps its evidence.
//!
//! `qualify` (S30-AC02, AC03, AC05, AC06) reads only what the run retained and states: every row as
//! a verified head/parts or a named unresolved fault; the Term list's authored title/body still
//! exact; the preservation equation for every changed body; the zero-edit second pass; the
//! before/after diagnostics with the Term rules counted apart; the actual DeepSeek/Claude usage and
//! runtime from the run's own files; and a deterministic review sheet for the owner's three math
//! terms, two concept terms and ten GUI Cards. Its decision is always `pending-human`.
//!
//! `record` (S30-AC07) takes the owner's filled review sheet, refuses an incomplete approval, and
//! records the explicit approval or rejection with its unresolved work. It writes no Card.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::args::Args;
use crate::canon;
use crate::ledger::Prepared;
use crate::pilot::{bounded_version, file_identity, workflow_identity, Checks};
use crate::ps;
use crate::term_cards;
use crate::util;
use crate::zt::Zt;

/// The record's own contract.
pub const CONTRACT: &str = "term-pilot/1";
/// The plan record, written by `-Step plan`.
pub const PLAN_FILE: &str = "term-pilot-plan.json";
/// The before state: every Card's body and the checker's diagnostics at plan time.
pub const BEFORE_FILE: &str = "term-pilot-before.json";
/// The launch record, written by `-Step preflight`.
pub const PREFLIGHT_FILE: &str = "term-pilot-preflight.json";
/// The qualification record, written by `-Step qualify`.
pub const QUALIFICATION_FILE: &str = "term-pilot-qualification.json";
/// The owner's review sheet, written by `-Step qualify` and filled by the owner.
pub const REVIEW_FILE: &str = "term-pilot-review.md";
/// The machine review sheet the owner fills; `-Step record` reads it.
pub const REVIEW_JSON: &str = "term-pilot-review.json";
/// The decision record, written by `-Step record`.
pub const DECISION_FILE: &str = "term-pilot-decision.json";
/// The one decision the gate itself can record. A human accepts or returns corrections.
pub const DECISION: &str = "pending-human";
/// The three binaries a pilot must name.
pub const BINARIES: [&str; 3] = ["zt", "zt-check", "zt-digest"];
/// The S30 review minimums (design §18.6): three math terms, two concept terms, ten GUI Cards.
pub const MATH_TERMS: usize = 3;
pub const CONCEPT_TERMS: usize = 2;
pub const GUI_CARDS: usize = 10;

pub struct Params {
    pub step: String,
    pub staging: String,
    pub work: String,
    pub book: String,
    pub copy: String,
    pub file: String,
    pub run_report: String,
    pub second_plan: String,
    pub claude_usage: String,
    pub project: String,
    pub decision: String,
}

impl Params {
    fn work_path(&self, name: &str) -> PathBuf {
        Path::new(&self.work).join(name)
    }
}

/// `zt-digest term-pilot`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let names = [
        "Step",
        "Staging",
        "Work",
        "Book",
        "Copy",
        "File",
        "RunReport",
        "SecondPlan",
        "ClaudeUsage",
        "Project",
        "Decision",
    ];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &[])?;
    let project = if a.has("Project") {
        a.required("Project")?
    } else {
        project_root(&std::env::current_dir()?)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
            .display()
            .to_string()
    };
    let p = Params {
        step: a.choice("Step", &["plan", "preflight", "qualify", "record"])?.to_ascii_lowercase(),
        staging: a.required("Staging")?,
        work: a.required("Work")?,
        book: a.str("Book"),
        copy: a.str("Copy"),
        file: a.str("File"),
        run_report: a.str("RunReport"),
        second_plan: a.str("SecondPlan"),
        claude_usage: a.str("ClaudeUsage"),
        project,
        decision: a.str("Decision"),
    };
    std::fs::create_dir_all(&p.work)
        .with_context(|| format!("cannot create the pilot work folder `{}`", p.work))?;
    let mut zt = crate::zt::start()?;
    match p.step.as_str() {
        "plan" => {
            if p.file.trim().is_empty() {
                bail!(
                    "-File is required: the authorization record that names the copied box, the live snapshot, the book, the mode and the review counts"
                );
            }
            let prepared = Prepared::read(&p.staging, &mut zt)?;
            let record = plan(&prepared, &mut zt, &p, &p.file)?;
            util::print_line(&ps::json_compact(&record))?;
            Ok(0)
        }
        "preflight" => {
            let prepared = Prepared::read(&p.staging, &mut zt)?;
            let record = preflight(&prepared, &mut zt, &p)?;
            let ok = record["ok"] == json!(true);
            util::print_line(&ps::json_compact(&record))?;
            Ok(if ok { 0 } else { 1 })
        }
        "qualify" => {
            let prepared = Prepared::read(&p.staging, &mut zt)?;
            let record = qualify(&prepared, &mut zt, &p)?;
            util::print_line(&ps::json_compact(&record))?;
            Ok(0)
        }
        _ => {
            let record = record(&mut zt, &p)?;
            let ok = record["ok"] == json!(true);
            util::print_line(&ps::json_compact(&record))?;
            Ok(if ok { 0 } else { 1 })
        }
    }
}

// --- small helpers --------------------------------------------------------------------------------

fn blank(value: &str) -> bool {
    value.trim().is_empty()
}

fn at(value: &Value, key: &str) -> String {
    ps::s(&value[key])
}

/// A path as the gate compares it: no extended-length prefix, no trailing separator, lower case.
fn norm_path(path: &str) -> String {
    let mut text = path.trim().trim_matches('"').replace('/', "\\");
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        text = rest.to_string();
    }
    while text.ends_with('\\') && text.len() > 3 {
        text.pop();
    }
    text.to_ascii_lowercase()
}

fn compatible(run: &str, tool: &str) -> bool {
    let parts = |value: &str| -> Option<(u64, u64, u64)> {
        let mut numbers = value.split('.').map(|part| part.parse::<u64>().ok());
        let version = (numbers.next()??, numbers.next()??, numbers.next()??);
        numbers.next().is_none().then_some(version)
    };
    match (parts(run), parts(tool)) {
        (Some((major, minor, patch)), Some((tool_major, tool_minor, tool_patch))) => {
            major == tool_major && minor == tool_minor && patch <= tool_patch
        }
        _ => run == tool,
    }
}

/// The repository root: the nearest ancestor that holds the installed workflow registry.
fn project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = std::fs::canonicalize(start).unwrap_or_else(|_| start.to_path_buf());
    loop {
        if dir.join(".claude").join("workflow-framework").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// The installed program named `name`: the environment override first, then this binary's own
/// folder, then PATH.
fn find_program(name: &str, env_name: &str) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(env_name).map(PathBuf::from).filter(|path| path.is_file()) {
        return Some(path);
    }
    if let Ok(exe) = std::env::current_exe() {
        for candidate in [
            exe.with_file_name(format!("{name}.exe")),
            exe.with_file_name(name),
        ] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for candidate in [dir.join(name), dir.join(format!("{name}.exe"))] {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn binary_fact(name: &str, path: &Path) -> Result<Value> {
    let (sha, bytes) = file_identity(path)?;
    let argument = if name == "zt-check" { "--version" } else { "version" };
    let version = bounded_version(path, argument).unwrap_or_default();
    Ok(json!({
        "name": name,
        "path": std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()).display().to_string(),
        "sha256": sha,
        "bytes": bytes,
        "version": version,
    }))
}

/// The running service's GUI URL, when `zt gui` answers one.
fn gui_url(zt_program: &Path) -> Option<String> {
    let output = std::process::Command::new(zt_program).arg("gui").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .find_map(|line| line.trim().strip_prefix("zt gui: "))
        .map(str::trim)
        .filter(|url| url.starts_with("http"))
        .map(str::to_string)
}

// --- the box and the copy -------------------------------------------------------------------------

/// The Cards of a `get_tree` reply (or of the retained live snapshot), in address order.
fn cards_of(reply: &Value) -> Vec<Value> {
    let list = reply
        .get("cards")
        .or_else(|| reply.get("nodes"))
        .or_else(|| reply.get("tree").and_then(|tree| tree.get("cards")))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut cards = list;
    cards.sort_by(|a, b| canon::ordinal_cmp(&at(a, "address"), &at(b, "address")));
    cards
}

/// One Card's authored identity, as the copy and the pilot compare it. Generated Reverse/Term-use
/// text and read projections are excluded: a copy is verified on what the author wrote.
fn card_signature(card: &Value) -> Value {
    let field = |name: &str| card.get(name).cloned().unwrap_or(Value::Null);
    json!({
        "address": at(card, "address"),
        "title": at(card, "title"),
        "body": at(card, "body"),
        "type": at(card, "type"),
        "facet": field("facet"),
        "emphasis": field("emphasis"),
        "level": field("level"),
        "form": field("form"),
        "parent": field("parent"),
        "edge": at(card, "edge"),
    })
}

/// The tree's fingerprint: its cards' authored identity, in address order.
fn tree_fingerprint(cards: &[Value]) -> String {
    let mut signatures: Vec<Value> = cards.iter().map(card_signature).collect();
    signatures.sort_by(|a, b| canon::ordinal_cmp(&at(a, "address"), &at(b, "address")));
    canon::hash_value(&json!(signatures))
}

fn card_map(cards: &[Value]) -> BTreeMap<String, Value> {
    cards
        .iter()
        .map(|card| (at(card, "address"), card.clone()))
        .collect()
}

/// The one Term list of a tree by role: a nameless Term Card, or a legacy `术语表` Card. An
/// ambiguous tree names both and is refused, never guessed.
fn list_of(cards: &[Value]) -> Result<Option<Value>> {
    let (lists, legacy) = term_cards::list_candidates(cards);
    match (lists.as_slice(), legacy.as_slice()) {
        ([], []) => Ok(None),
        ([one], []) => Ok(Some(list_facts(one, false))),
        ([], [one]) => Ok(Some(list_facts(one, true))),
        _ => bail!(
            "the book's Literature tree has {} Term list Cards and {} legacy 术语表 Cards; exactly one list is required",
            lists.len(),
            legacy.len()
        ),
    }
}

fn list_facts(card: &Value, legacy: bool) -> Value {
    let body = at(card, "body");
    let rows = zt_check::text::term_rows_raw(&body);
    json!({
        "address": at(card, "address"),
        "type": at(card, "type"),
        "legacy": legacy,
        "title": at(card, "title"),
        "body_sha256": canon::sha256_hex(body.as_bytes()),
        "rows": rows.len(),
        "terms": rows.iter().map(|row| row.term.clone()).collect::<Vec<_>>(),
        "meaning_rows": rows.iter().map(|row| json!({
            "term": row.term, "avoid": row.avoid, "meaning": row.meaning, "defined": row.defined,
        })).collect::<Vec<_>>(),
    })
}

struct BoxView {
    identity: Value,
    revision: u64,
    tree: Option<Value>,
    cards: Vec<Value>,
    fingerprint: String,
    list: Option<Value>,
}

/// One coherent read of the box: its archive identity, the book tree's authored Cards and the one
/// Term list.
fn box_view(zt: &mut Zt, book: &str) -> Result<BoxView> {
    let view = zt.archive_view()?;
    let revision = view["archive_revision"]
        .as_u64()
        .context("the box's archive view carries no revision")?;
    let identity = view["archive_identity"].clone();
    let tree = zt
        .call("get_tree", json!({"tree": book, "bodies": true}))
        .with_context(|| format!("reading the copied book tree `{book}`"))?;
    let cards = cards_of(&tree);
    let fingerprint = tree_fingerprint(&cards);
    let list = list_of(&cards)?;
    Ok(BoxView {
        identity,
        revision,
        tree: Some(tree),
        cards,
        fingerprint,
        list,
    })
}

// --- authorization ---------------------------------------------------------------------------------

fn hex32(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The owner's authorization record: the copied box, the live snapshot, the book, the mode and the
/// review counts. A missing or contradictory fact is a named check, never a default.
fn authorization(checks: &mut Checks, auth: &Value, book: &str, observed_root: &str) -> Value {
    checks.require("authorization.schema", auth["schema_version"] == json!(1), "the authorization record must carry schema_version 1");
    checks.require("authorization.pilot", at(auth, "pilot") == "S30", "the authorization record must name pilot S30");
    checks.require("authorization.approved", auth["approved"] == json!(true), "the owner's authorization must record approved: true");
    checks.require("authorization.note", !blank(&at(auth, "note")), "the authorization record needs a note that says what was authorized");
    checks.require("authorization.book", at(auth, "book") == book, format!("the authorization names book `{}`, not `{book}`", at(auth, "book")));
    let mode = at(auth, "mode");
    checks.require(
        "authorization.mode",
        ps::eq(&mode, "backfill"),
        format!("the authorization mode `{mode}` is not `backfill`"),
    );
    let box_root = at(&auth["box"], "archive_root");
    checks.require("authorization.box.root", !blank(&box_root), "the authorization must name the copied box's archive_root");
    checks.require_that(
        "authorization.box.observed",
        blank(&box_root) || norm_path(&box_root) == norm_path(observed_root),
        || {
            format!(
                "the authorization names the box archive `{box_root}`, but the service serves `{observed_root}`; point the run at the copied box"
            )
        },
    );
    let config = std::env::var("ZT_CONFIG_DIR").unwrap_or_default();
    let named_config = at(&auth["box"], "config_dir");
    checks.require_that("authorization.box.pinned", !config.is_empty(), || {
        "ZT_CONFIG_DIR is unset: a pilot runs on a copied box with its own config, never on the live box".to_string()
    });
    checks.require_that(
        "authorization.box.config",
        blank(&named_config) || norm_path(&named_config) == norm_path(&config),
        || format!("the authorization names box config `{named_config}`, but ZT_CONFIG_DIR is `{config}`"),
    );
    let live_id = at(&auth["live"], "archive_id");
    let live_root = at(&auth["live"], "archive_root");
    let live_revision = auth["live"]["revision"].as_u64().unwrap_or(0);
    checks.require("authorization.live.id", hex32(&live_id), "the authorization must name the live archive's 32-hex id");
    checks.require("authorization.live.root", !blank(&live_root), "the authorization must name the live archive_root");
    checks.require("authorization.live.revision", live_revision > 0, "the authorization must name the live archive revision the snapshot was taken at");
    checks.require_that(
        "authorization.live.separate",
        norm_path(&box_root) != norm_path(&live_root),
        || "the authorization names the live archive as the pilot box; the pilot must run on a copy".to_string(),
    );
    let box_id = at(&auth["box"], "archive_id");
    checks.require_that(
        "authorization.box.id",
        blank(&box_id) || box_id != live_id,
        || "the copied box and the live archive carry the same archive id; the box is not a copy".to_string(),
    );
    let review = &auth["review"];
    let math = review["math_terms"].as_u64().unwrap_or(0) as usize;
    let concept = review["concept_terms"].as_u64().unwrap_or(0) as usize;
    let gui = review["gui_cards"].as_u64().unwrap_or(0) as usize;
    checks.require_that(
        "authorization.review",
        math >= MATH_TERMS && concept >= CONCEPT_TERMS && gui >= GUI_CARDS,
        || {
            format!(
                "the review must ask for at least {MATH_TERMS} math terms, {CONCEPT_TERMS} concept terms and {GUI_CARDS} GUI Cards (got {math}/{concept}/{gui})"
            )
        },
    );
    json!({
        "pilot": at(auth, "pilot"),
        "approved": auth["approved"],
        "note": at(auth, "note"),
        "book": at(auth, "book"),
        "mode": mode,
        "box": auth["box"].clone(),
        "live": auth["live"].clone(),
        "copy": auth.get("copy").cloned().unwrap_or(Value::Null),
        "review": review.clone(),
    })
}

// --- plan ------------------------------------------------------------------------------------------

fn plan_id(parts: &Value) -> String {
    format!("tp-{}", &canon::hash_value(parts)[..16])
}

fn plan(prepared: &Prepared, zt: &mut Zt, p: &Params, file: &str) -> Result<Value> {
    let auth_value = util::read_json(file).with_context(|| format!("`{file}` is not a JSON authorization record"))?;
    let contract = &prepared.contract;
    let book = if p.book.trim().is_empty() {
        at(contract, "citation_key")
    } else {
        p.book.trim().to_string()
    };
    if at(contract, "citation_key") != book {
        bail!(
            "the prepared run is for `{}`, not the pilot book `{book}`",
            at(contract, "citation_key")
        );
    }
    let mode = at(&auth_value, "mode");
    let staging = prepared.out.display().to_string();
    let mut checks = Checks::default();

    let observed = zt_check::mcp::observed_service().context("the copy's service was not observed; start the copied box first")?;
    if !observed.up {
        bail!("the copied box's service is not up; start it before planning the pilot");
    }
    let observed_root = observed
        .archive_root
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let auth = authorization(&mut checks, &auth_value, &book, &observed_root);
    let (auth_sha, _) = file_identity(Path::new(file))?;

    let view = box_view(zt, &book)?;
    let box_id = at(&view.identity, "id");
    let box_path = at(&view.identity, "path");
    checks.require("box.identity", hex32(&box_id), "the box's archive read named no 32-hex archive id");
    checks.require_that(
        "box.separate",
        norm_path(&box_path) != norm_path(&at(&auth["live"], "archive_root")),
        || format!("the box archive `{box_path}` is the live archive; a pilot runs on a copy"),
    );
    checks.require_that(
        "box.not_live_id",
        box_id != at(&auth["live"], "archive_id"),
        || "the box's archive id equals the live archive id; the box is not a copy".to_string(),
    );
    let lock = crate::lockcheck::lock_of(zt, &book)?;
    checks.require_that("box.unlocked", lock.is_none(), || {
        format!(
            "the copied book tree `{book}` is locked (lock {}); a pilot copy must be writable",
            lock.as_ref().map(|lock| lock.lock_id).unwrap_or_default()
        )
    });
    checks.require_that("box.tree", view.tree.is_some(), || format!("the box has no book tree `{book}` to backfill"));

    // --- the copied snapshot and its comparison ---
    let copy_tree = at(&auth["copy"], "tree");
    let mut copy = Value::Null;
    if !blank(&p.copy) {
        let folder = Path::new(&p.copy);
        let report_path = folder.join("report.json");
        let live_path = folder.join("live-tree.json");
        checks.require_that("copy.report", report_path.is_file(), || format!("the copy report `{}` is missing", report_path.display()));
        checks.require_that("copy.snapshot", live_path.is_file(), || format!("the live tree snapshot `{}` is missing", live_path.display()));
        checks.require_that("copy.tree", !blank(&copy_tree), || {
            "the authorization must name the copied tree (copy.tree); copy-tree.mjs copies one tree".to_string()
        });
        if report_path.is_file() && live_path.is_file() && !blank(&copy_tree) {
            let report = util::read_json(&report_path)?;
            let (report_sha, _) = file_identity(&report_path)?;
            let (live_sha, _) = file_identity(&live_path)?;
            let live = util::read_json(&live_path)?;
            let live_cards = cards_of(&live);
            let live_fingerprint = tree_fingerprint(&live_cards);
            let (copied_cards, copied_fingerprint) = if ps::eq(&copy_tree, &book) {
                (view.cards.clone(), view.fingerprint.clone())
            } else {
                let tree = zt.call("get_tree", json!({"tree": copy_tree, "bodies": true}))?;
                let cards = cards_of(&tree);
                let fingerprint = tree_fingerprint(&cards);
                (cards, fingerprint)
            };
            let differences = report["differences"].as_array().map(Vec::len).unwrap_or(usize::MAX);
            checks.require("copy.differences", differences == 0, format!("the copy report names {differences} differences"));
            let live_count = report["live_count"].as_u64().unwrap_or(0);
            let test_count = report["test_count"].as_u64().unwrap_or(0);
            checks.require("copy.counts", live_count == test_count && live_count > 0, format!("the copy report counts {live_count} live and {test_count} copied Cards"));
            checks.require_that(
                "copy.identical",
                copied_fingerprint == live_fingerprint,
                || {
                    format!(
                        "the box tree `{copy_tree}` fingerprint {copied_fingerprint} is not the retained live snapshot's {live_fingerprint}; recopy the book"
                    )
                },
            );
            checks.require_that(
                "copy.revision",
                live["archive_revision"].as_u64() == auth["live"]["revision"].as_u64(),
                || {
                    format!(
                        "the retained live snapshot is at revision {}, not the authorized {}",
                        live["archive_revision"].as_u64().unwrap_or(0),
                        auth["live"]["revision"].as_u64().unwrap_or(0)
                    )
                },
            );
            copy = json!({
                "folder": folder.display().to_string(),
                "tree": copy_tree,
                "report": {"path": report_path.display().to_string(), "sha256": report_sha, "live_count": live_count, "test_count": test_count, "differences": differences},
                "live_tree": {"path": live_path.display().to_string(), "sha256": live_sha, "revision": live["archive_revision"], "cards": live_cards.len(), "fingerprint": live_fingerprint},
                "copied_tree": {"cards": copied_cards.len(), "fingerprint": copied_fingerprint},
            });
        }
    } else {
        checks.require("copy.present", false, "a backfill pilot needs -Copy: copy-tree.mjs's report and live-tree snapshot");
    }

    // --- the Term list and its exact authored identity ---
    let list = view.list.clone();
    {
        checks.require_that("list.present", list.is_some(), || {
            format!("the copied book `{book}` has no Term list and no legacy 术语表 table")
        });
        if let (Some(list), true) = (&list, ps::eq(&copy_tree, &book)) {
            let report_path = Path::new(&p.copy).join("live-tree.json");
            if report_path.is_file() {
                let live_cards = cards_of(&util::read_json(&report_path)?);
                let live_list = list_of(&live_cards)?;
                checks.require_that(
                    "list.copied",
                    live_list.as_ref().map(|live| at(live, "address")) == Some(at(list, "address"))
                        && live_list.as_ref().map(|live| at(live, "body_sha256")) == Some(at(list, "body_sha256")),
                    || "the box's Term list is not the live snapshot's list".to_string(),
                );
            }
        }
    }

    // --- the prepared run ---
    let manifest_path = Path::new(&staging).join("manifest.json");
    let (manifest_sha, _) = file_identity(&manifest_path)?;
    let manifest = util::read_json(&manifest_path)?;
    let source = &contract["source"];
    let source_path = at(source, "path");
    checks.require_that("staging.source", Path::new(&source_path).is_file(), || {
        format!("the frozen source `{source_path}` is missing; the backfill must bind the same source")
    });
    let groups = manifest["groups"].as_array().map(Vec::len).unwrap_or(0);
    checks.require("staging.groups", groups > 0, "the prepared run names no groups");
    checks.require("staging.contract", contract["contracts"]["artifact_schema"] == json!(1), "the prepared run's artifact schema is not 1");

    // --- the installed frozen Workflow and the binaries ---
    let variant = at(contract, "variant");
    let workflow = workflow_identity(Path::new(&p.project), &variant)?;
    // The workflow that drives the run must freeze the same contract line as the prepared run. The
    // installed global workflow is recorded too: the operator may load it instead of the project
    // one, and a stale global set is a release fact (S31/#173), not a reason to guess here.
    let contract_version = at(&contract["contracts"], "variant_version");
    checks.require_that("workflow.contract", compatible(&contract_version, &at(&workflow, "version")), || {
        format!(
            "the installed frozen Workflow is {}, but the prepared run froze {contract_version}; install the matching variant",
            at(&workflow, "version")
        )
    });
    let global = crate::pilot::home_dir().and_then(|home| workflow_identity(&home, &variant).ok());
    let global_compatible = global.as_ref().is_some_and(|global| compatible(&contract_version, &at(global, "version")));
    let mut workflow = workflow;
    workflow["global"] = global.unwrap_or(Value::Null);
    workflow["global_compatible"] = json!(global_compatible);
    let zt_path = find_program("zt", "ZT_CHECK_ZT");
    let check_path = find_program("zt-check", "ZT_CHECK");
    let digest_path = std::env::current_exe().ok();
    let mut binaries = Vec::new();
    for (name, path) in [
        ("zt", zt_path.clone()),
        ("zt-check", check_path.clone()),
        ("zt-digest", digest_path.clone()),
    ] {
        match path {
            Some(path) if path.is_file() => binaries.push(binary_fact(name, &path)?),
            _ => checks.require("binaries.present", false, format!("`{name}` is not installed or not on PATH")),
        }
    }
    let found: BTreeMap<String, Value> = binaries
        .iter()
        .map(|binary| (at(binary, "name"), binary.clone()))
        .collect();
    for name in BINARIES {
        if let Some(binary) = found.get(name) {
            let version = at(binary, "version");
            if name == "zt-check" {
                checks.require_that("binaries.checker", ps::eq(&version, &format!("zt-check {}", zt_check::VERSION)), || {
                    format!("the installed `zt-check` reports `{version}`, not `zt-check {}`", zt_check::VERSION)
                });
            } else if blank(&version) {
                checks.require("binaries.version", false, format!("`{name} version` answered nothing"));
            }
        }
    }

    // --- the before state: bodies and the checker's diagnostics ---
    let before_diagnostics = match check_path.as_ref() {
        Some(checker) => {
            let report = p.work_path("term-pilot-before-check.json");
            let log = p.work_path("term-pilot-before-check.log");
            let addresses: Vec<String> = view.cards.iter().map(|card| at(card, "address")).collect();
            let report_value = run_checker(checker, &addresses, &log, &report, &book)?;
            let (sha, _) = file_identity(&report)?;
            json!({
                "path": report.display().to_string(),
                "sha256": sha,
                "checker_version": report_value["contract"]["checker_version"],
                "diagnostics": report_value["diagnostics"].as_array().map(Vec::len).unwrap_or(0),
            })
        }
        None => Value::Null,
    };
    let before = json!({
        "contract": CONTRACT,
        "kind": "term_pilot_before",
        "plan_id": Value::Null,
        "cards": view.cards.iter().map(|card| json!({
            "address": at(card, "address"),
            "title": at(card, "title"),
            "type": at(card, "type"),
            "body": at(card, "body"),
        })).collect::<Vec<_>>(),
        "diagnostics": before_diagnostics,
    });
    let before_digest = canon::hash_value(&before["cards"]);
    let before_path = p.work_path(BEFORE_FILE);
    if before_diagnostics.is_null() {
        checks.require("before.diagnostics", false, "no `zt-check` binary: the before/language diagnostics cannot be recorded");
    }
    let checks_done = checks.finish("the term pilot plan")?;

    let key_parts = json!({
        "contract": CONTRACT,
        "book": book,
        "mode": mode,
        "box": {"id": box_id, "path": box_path, "revision": view.revision},
        "tree": view.fingerprint,
        "staging": prepared.hash,
        "copy": copy["live_tree"]["sha256"],
        "list": list.as_ref().map(|list| at(list, "body_sha256")),
        "before": before_digest,
        "authorization": auth_sha,
    });
    let plan_id = plan_id(&key_parts);
    let gui = zt_path.as_deref().and_then(gui_url);
    let record = json!({
        "contract": CONTRACT,
        "kind": "term_pilot_plan",
        "plan_id": plan_id,
        "created_at": canon::utc_now_o(),
        "book": {
            "citation_key": book,
            "title": view.tree.as_ref().map(|tree| at(tree, "title")).unwrap_or_default(),
            "variant": variant,
            "mode": mode,
        },
        "box": {
            "config_dir": std::env::var("ZT_CONFIG_DIR").unwrap_or_default(),
            "archive": {"id": box_id, "path": box_path, "revision": view.revision},
            "service": {"version": observed.version, "pid": observed.pid},
            "tree": {"present": view.tree.is_some(), "cards": view.cards.len(), "fingerprint": view.fingerprint},
            "cards": view.cards.len(),
            "list": list,
            "gui": gui,
        },
        "live": auth["live"].clone(),
        "copy": copy,
        "staging": {
            "path": staging,
            "manifest_sha256": manifest_sha,
            "preparation_hash": prepared.hash,
            "contracts": contract["contracts"].clone(),
            "source": {"path": source_path, "sha256": at(source, "sha256"), "file_id": source["file_id"]},
            "groups": groups,
        },
        "workflow": workflow,
        "binaries": binaries,
        "before": {
            "file": BEFORE_FILE,
            "digest": before_digest,
            "cards": view.cards.len(),
            "diagnostics": before_diagnostics,
        },
        "review": auth["review"].clone(),
        "authorization": {"path": file, "sha256": auth_sha, "record": auth},
        "checks": checks_done,
        "decision": DECISION,
    });
    util::write_atomic(&p.work_path(PLAN_FILE), &ps::json_pretty(&record))?;
    let mut before = before;
    before["plan_id"] = json!(plan_id);
    util::write_atomic(&before_path, &ps::json_pretty(&before))?;
    Ok(record)
}

/// `zt-check --cards` over the exact addresses, with its JSON report under the work folder.
fn run_checker(checker: &Path, addresses: &[String], log: &Path, report: &Path, book: &str) -> Result<Value> {
    if addresses.is_empty() {
        return Ok(json!({"diagnostics": [], "contract": {"checker_version": zt_check::VERSION}}));
    }
    let output = std::process::Command::new(checker)
        .arg("--cards")
        .arg(addresses.join(","))
        .arg("--log")
        .arg(log)
        .arg("--report")
        .arg(report)
        .arg("--book")
        .arg(book)
        .env("ZT_GUI_PORT", "0")
        .output()
        .with_context(|| format!("running `{}`", checker.display()))?;
    let said = String::from_utf8_lossy(&output.stderr);
    if !report.is_file() {
        bail!(
            "`zt-check --cards` wrote no report (exit {}): {}",
            output.status.code().unwrap_or(-1),
            said.trim()
        );
    }
    util::read_json(report)
}

// --- preflight -------------------------------------------------------------------------------------

fn preflight(prepared: &Prepared, zt: &mut Zt, p: &Params) -> Result<Value> {
    let plan_path = p.work_path(PLAN_FILE);
    let plan = util::read_json(&plan_path).with_context(|| {
        format!("`{}` is not a term pilot plan; run -Step plan first", plan_path.display())
    })?;
    let before = util::read_json(&p.work_path(BEFORE_FILE))?;
    let mut checks = Checks::default();
    let book = at(&plan["book"], "citation_key");
    let mode = at(&plan["book"], "mode");

    // The plan itself still describes its own key.
    let recomputed = plan_id(&json!({
        "contract": CONTRACT,
        "book": book,
        "mode": mode,
        "box": {"id": plan["box"]["archive"]["id"], "path": plan["box"]["archive"]["path"], "revision": plan["box"]["archive"]["revision"]},
        "tree": plan["box"]["tree"]["fingerprint"],
        "staging": plan["staging"]["preparation_hash"],
        "copy": plan["copy"]["live_tree"]["sha256"],
        "list": plan["box"]["list"].as_object().map(|_| plan["box"]["list"]["body_sha256"].clone()),
        "before": plan["before"]["digest"],
        "authorization": plan["authorization"]["sha256"],
    }));
    checks.require("plan.id", recomputed == at(&plan, "plan_id"), "the plan record was edited");

    if !p.file.trim().is_empty() {
        let (auth_sha, _) = file_identity(Path::new(&p.file))?;
        checks.require("plan.authorization", auth_sha == at(&plan["authorization"], "sha256"), "the authorization record changed since the plan");
    }

    // The copy and the box did not move.
    let view = box_view(zt, &book)?;
    checks.require("box.id", at(&view.identity, "id") == at(&plan["box"]["archive"], "id"), "the box archive changed since the plan");
    checks.require("box.revision", view.revision == plan["box"]["archive"]["revision"].as_u64().unwrap_or(u64::MAX), "the box archive moved since the plan");
    checks.require("box.tree", view.fingerprint == at(&plan["box"]["tree"], "fingerprint"), "the copied book tree changed since the plan");
    checks.require("box.unlocked", crate::lockcheck::lock_of(zt, &book)?.is_none(), "the copied book tree is locked");
    checks.require("box.list", view.list.as_ref().map(|list| at(list, "body_sha256")) == plan["box"]["list"].as_object().map(|_| at(&plan["box"]["list"], "body_sha256")), "the Term list changed since the plan");
    let pin = Path::new(&p.staging).join("zt-box.txt");
    checks.require_that("box.pin", pin.is_file(), || {
        format!("`{}` is missing; the run must be pinned to the copied box", pin.display())
    });
    if pin.is_file() {
        let named = util::read_text(&pin)?.lines().next().unwrap_or_default().trim().to_string();
        let config = std::env::var("ZT_CONFIG_DIR").unwrap_or_default();
        checks.require_that("box.pin.box", !named.is_empty() && norm_path(&named) == norm_path(&config), || {
            format!("`{}` names `{named}`, not the box `{config}`", pin.display())
        });
    }

    // The prepared run, the source and the frozen Workflow did not move.
    let staging = prepared.out.display().to_string();
    let manifest_path = Path::new(&staging).join("manifest.json");
    let (manifest_sha, _) = file_identity(&manifest_path)?;
    checks.require("staging.path", norm_path(&staging) == norm_path(&at(&plan["staging"], "path")), "the prepared run moved");
    checks.require("staging.manifest", manifest_sha == at(&plan["staging"], "manifest_sha256"), "the prepared run's manifest changed");
    checks.require("staging.hash", prepared.hash == at(&plan["staging"], "preparation_hash"), "the prepared run's frozen contract changed");
    let source_path = at(&plan["staging"]["source"], "path");
    checks.require("staging.source", Path::new(&source_path).is_file(), format!("the frozen source `{source_path}` is missing"));
    let workflow = workflow_identity(Path::new(&p.project), &at(&plan["book"], "variant"))?;
    checks.require("workflow.version", at(&workflow, "version") == at(&plan["workflow"], "version"), "the installed frozen Workflow version changed");
    checks.require("workflow.script", at(&workflow, "script_sha256") == at(&plan["workflow"], "script_sha256"), "the installed frozen Workflow script changed");
    let check_path = find_program("zt-check", "ZT_CHECK");
    let zt_path = find_program("zt", "ZT_CHECK_ZT");
    for (name, path) in [("zt", zt_path), ("zt-check", check_path)] {
        let planned = plan["binaries"]
            .as_array()
            .and_then(|binaries| binaries.iter().find(|binary| at(binary, "name") == name))
            .cloned();
        match (path, planned) {
            (Some(path), Some(planned)) => {
                let (sha, _) = file_identity(&path)?;
                checks.require("binaries.unchanged", sha == at(&planned, "sha256"), format!("the installed `{name}` changed since the plan"));
            }
            (None, _) => checks.require("binaries.present", false, format!("`{name}` is not installed or not on PATH")),
            (_, None) => checks.require("binaries.named", false, format!("the plan does not name `{name}`")),
        }
    }

    // A fresh run: no Term Card was published yet, and no Term run started.
    if let Ok(terms) = zt.call("get_terms", json!({"at": book})) {
        let heads = terms["heads"].as_array().map(Vec::len).unwrap_or(0);
        let parts = terms["parts"].as_array().map(Vec::len).unwrap_or(0);
        checks.require("run.fresh", heads == 0 && parts == 0, format!("the book already publishes {heads} Term head(s) and {parts} part(s); the pilot run already happened"));
    }
    let term_report = Path::new(&staging).join("term-cards").join("report.json");
    checks.require_that("run.started", !term_report.is_file(), || {
        format!("`{}` already exists; the pilot's one run already started", term_report.display())
    });
    let _ = before;

    let (ok, checks, problems) = checks.reported();
    let record = json!({
        "contract": CONTRACT,
        "kind": "term_pilot_preflight",
        "ok": ok,
        "plan_id": at(&plan, "plan_id"),
        "at": canon::utc_now_o(),
        "staging": staging,
        "box": {"archive": view.identity, "revision": view.revision, "cards": view.cards.len()},
        "checks": checks,
        "problems": problems,
    });
    util::write_atomic(&p.work_path(PREFLIGHT_FILE), &ps::json_pretty(&record))?;
    Ok(record)
}

// --- qualify ---------------------------------------------------------------------------------------

fn read(path: &Path, what: &str) -> Result<Value> {
    util::read_json(path).with_context(|| format!("`{}` is not {what}", path.display()))
}

fn artifact(path: &Path) -> Result<Value> {
    let (sha, bytes) = file_identity(path)?;
    Ok(json!({"path": path.display().to_string(), "sha256": sha, "bytes": bytes}))
}

fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.as_bytes() {
        let keep = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if keep {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn card_url(gui: &str, address: &str) -> String {
    if gui.trim().is_empty() {
        return format!("#/card/{}", percent_encode(address));
    }
    format!("{}#/card/{}", gui.trim_end_matches('/'), percent_encode(address))
}

/// The checker's diagnostics of one Card, as the pilot compares them: the rule, its fault code and
/// semantic key, the severity and the full message. The display clip and the line are not identity.
fn diagnostic_key(diagnostic: &Value) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        at(diagnostic, "rule"),
        at(diagnostic, "fault"),
        at(diagnostic, "key"),
        at(diagnostic, "severity"),
        at(diagnostic, "message")
    )
}

/// One checker report's per-Card diagnostics, in semantic-key order. Rules whose id starts with `T`
/// are excluded: a Term Card's own T rules are the feature, not a prose fault.
fn diagnostics_by_card(report: &Value, exclude_term_rules: bool) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for diagnostic in report["diagnostics"].as_array().into_iter().flatten() {
        if exclude_term_rules && at(diagnostic, "rule").starts_with('T') {
            continue;
        }
        out.entry(at(diagnostic, "address")).or_default().push(diagnostic_key(diagnostic));
    }
    for keys in out.values_mut() {
        keys.sort();
    }
    out
}

/// The review sample: the frozen rows' committed math and concept terms, and the Cards the
/// Term-link pass actually edited, in a deterministic order. Later rows and addresses never
/// displace an earlier one, so a re-qualification of the same evidence names the same review.
fn review_sheet(
    plan: &Value,
    cards_plan: &Value,
    cards_report: &Value,
    link_plan: &Value,
) -> (Value, String) {
    let gui = at(&plan["box"], "gui");
    let committed: BTreeMap<i64, Value> = cards_report["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| at(&row["publication"], "status") == "committed")
        .map(|row| (row["row"].as_i64().unwrap_or(0), row.clone()))
        .collect();
    let review = &plan["review"];
    let want_math = (review["math_terms"].as_u64().unwrap_or(0) as usize).max(MATH_TERMS);
    let want_concept = (review["concept_terms"].as_u64().unwrap_or(0) as usize).max(CONCEPT_TERMS);
    let want_cards = (review["gui_cards"].as_u64().unwrap_or(0) as usize).max(GUI_CARDS);
    let mut math = Vec::new();
    let mut concept = Vec::new();
    for row in cards_plan["rows"].as_array().into_iter().flatten() {
        let number = row["row"].as_i64().unwrap_or(0);
        let Some(published) = committed.get(&number) else { continue };
        let kind = at(&row["classification"], "kind");
        let item = json!({
            "row": number,
            "term": at(row, "term"),
            "kind": kind,
            "meaning": at(row, "meaning"),
            "defined": at(row, "defined"),
            "head": at(&published["publication"], "head"),
            "parts": published["publication"]["parts"],
            "url": card_url(&gui, &at(&published["publication"], "head")),
        });
        let bucket = if kind == "math" { &mut math } else { &mut concept };
        let cap = if kind == "math" { want_math } else { want_concept };
        if bucket.len() < cap {
            bucket.push(item);
        }
    }
    let mut cards = Vec::new();
    for card in link_plan["cards"].as_array().into_iter().flatten() {
        let edits = card["edits"].as_array().map(Vec::len).unwrap_or(0);
        if card["changed"] != json!(true) || edits == 0 {
            continue;
        }
        cards.push(json!({
            "address": at(card, "address"),
            "title": at(card, "title"),
            "term_uses": edits,
            "terms": card["edits"].as_array().into_iter().flatten().map(|edit| at(edit, "term")).collect::<Vec<_>>(),
            "url": card_url(&gui, &at(card, "address")),
        }));
    }
    cards.sort_by(|a, b| {
        b["term_uses"]
            .as_u64()
            .unwrap_or(0)
            .cmp(&a["term_uses"].as_u64().unwrap_or(0))
            .then_with(|| canon::ordinal_cmp(&at(a, "address"), &at(b, "address")))
    });
    cards.truncate(want_cards);
    let shortfall = json!({
        "math_terms": MATH_TERMS.saturating_sub(math.len()),
        "concept_terms": CONCEPT_TERMS.saturating_sub(concept.len()),
        "gui_cards": GUI_CARDS.saturating_sub(cards.len()),
    });
    let complete = math.len() >= MATH_TERMS && concept.len() >= CONCEPT_TERMS && cards.len() >= GUI_CARDS;
    let review_id = format!(
        "tr-{}",
        &canon::hash_value(&json!({
            "plan": plan["plan_id"],
            "math": math, "concept": concept, "cards": cards,
        }))[..16]
    );
    let value = json!({
        "schema_version": 1,
        "kind": "term_pilot_review",
        "review_id": review_id,
        "plan_id": plan["plan_id"],
        "complete": complete,
        "shortfall": shortfall,
        "math": math,
        "concept": concept,
        "cards": cards,
        "decision": Value::Null,
        "unresolved": [],
    });
    let text = review_markdown(&value, plan);
    (value, text)
}

fn review_markdown(review: &Value, plan: &Value) -> String {
    let mut out = String::new();
    out.push_str("# S30 Term pilot review\n\n");
    out.push_str(&format!(
        "Book: `{}` ({})\n\nBox archive: `{}` at revision {}\n\nPrepared run: `{}` with {} group(s), frozen as `{}` {}.\n\n",
        at(&plan["book"], "citation_key"),
        at(&plan["book"], "title"),
        at(&plan["box"]["archive"], "id"),
        plan["box"]["archive"]["revision"].as_u64().unwrap_or(0),
        at(&plan["staging"], "path"),
        plan["staging"]["groups"].as_u64().unwrap_or(0),
        at(&plan["workflow"], "name"),
        at(&plan["workflow"], "version"),
    ));
    out.push_str("Read every item in the copied box's GUI, against the book's source and notation. Fill `term-pilot-review.json`, then run `-Step record`.\n\n");
    for (title, section) in [
        (
            "A. Mathematical terms (the explanation, the symbols, the derivation and the limits)",
            "math",
        ),
        (
            "B. Concept terms (the explanation, the distinctions and the examples)",
            "concept",
        ),
    ] {
        out.push_str(&format!("## {title}\\n\\n"));
        for (index, item) in review[section].as_array().into_iter().flatten().enumerate() {
            out.push_str(&format!(
                "{}. `{}` — row {} — head `{}`\n   Source meaning: {}\n   Defined at: {}\n   URL: {}\n   [ ] accept  [ ] correct: ______  note: ______\n\n",
                index + 1,
                at(item, "term"),
                item["row"].as_u64().unwrap_or(0),
                at(item, "head"),
                at(item, "meaning"),
                at(item, "defined"),
                at(item, "url"),
            ));
        }
    }
    out.push_str("## C. Ten linked Cards (label-only layout, first-occurrence linking, ordinary-Link distinction and keyboard readability)\n\n");
    for (index, item) in review["cards"].as_array().into_iter().flatten().enumerate() {
        let terms: Vec<String> = item["terms"].as_array().into_iter().flatten().map(|term| ps::s(term)).collect();
        out.push_str(&format!(
            "{}. `{}` — {} — {} Term use(s): {}\n   URL: {}\n   [ ] accept  [ ] correct: ______  note: ______\n\n",
            index + 1,
            at(item, "address"),
            at(item, "title"),
            item["term_uses"].as_u64().unwrap_or(0),
            terms.join("、"),
            at(item, "url"),
        ));
    }
    out.push_str("## D. Measurements\n\n");
    out.push_str("The qualification record states the measured facts: every row's head/parts, the preserved list, the changed-body equation, the second pass, the before/after diagnostics and the provider usage/runtime.\n\n");
    out.push_str("## E. Decision\n\n");
    out.push_str("[ ] approve  [ ] reject\n\nUnresolved work: ______\n");
    out
}

fn qualify(prepared: &Prepared, zt: &mut Zt, p: &Params) -> Result<Value> {
    let plan = read(&p.work_path(PLAN_FILE), "a term pilot plan")?;
    let before = read(&p.work_path(BEFORE_FILE), "the term pilot's before state")?;
    let preflight = read(&p.work_path(PREFLIGHT_FILE), "the term pilot's preflight record")?;
    if preflight["ok"] != json!(true) {
        bail!("the preflight refused this pilot; fix its problems and plan again");
    }
    if prepared.hash != at(&plan["staging"], "preparation_hash") {
        bail!("the prepared run changed since the plan");
    }
    let book = at(&plan["book"], "citation_key");
    let staging = Path::new(&at(&plan["staging"], "path")).to_path_buf();
    let view = box_view(zt, &book)?;
    let current = card_map(&view.cards);

    let cards_plan_path = staging.join("term-cards").join("plan.json");
    let cards_report_path = staging.join("term-cards").join("report.json");
    let link_plan_path = if staging.join("term-link").join("first-plan.json").is_file() {
        staging.join("term-link").join("first-plan.json")
    } else {
        staging.join("term-link").join("plan.json")
    };
    let link_apply_path = staging.join("term-link").join("apply.json");
    let cards_plan = read(&cards_plan_path, "the Term-card plan")?;
    let cards_report = read(&cards_report_path, "the Term-card publication report")?;
    let link_plan = read(&link_plan_path, "the Term-link plan")?;
    let link_apply = read(&link_apply_path, "the Term-link apply report")?;
    let run_report_path = if p.run_report.trim().is_empty() {
        staging.join("run-report.json")
    } else {
        PathBuf::from(&p.run_report)
    };
    let second_plan_path = if p.second_plan.trim().is_empty() {
        staging.join("term-link").join("second-plan.json")
    } else {
        PathBuf::from(&p.second_plan)
    };

    // --- every row as a verified head/parts or a named unresolved fault ---
    let mut parts_addresses: BTreeSet<String> = BTreeSet::new();
    let published: BTreeMap<i64, Value> = cards_report["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| (row["row"].as_i64().unwrap_or(0), row.clone()))
        .collect();
    let mut rows = Vec::new();
    let mut faults = Vec::new();
    for row in cards_plan["rows"].as_array().into_iter().flatten() {
        let number = row["row"].as_i64().unwrap_or(0);
        let term = at(row, "term");
        let Some(published) = published.get(&number) else {
            faults.push(json!({"row": number, "term": term, "status": "missing", "reason": "the publication report has no row"}));
            continue;
        };
        let status = at(&published["publication"], "status");
        let head = at(&published["publication"], "head");
        let parts: Vec<String> = published["publication"]["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|part| ps::s(part))
            .collect();
        let reason = published["publication"]["reason"].clone();
        if status != "committed" || head.is_empty() {
            faults.push(json!({"row": number, "term": term, "status": status, "reason": reason}));
            continue;
        }
        parts_addresses.insert(head.clone());
        for part in &parts {
            parts_addresses.insert(part.clone());
        }
        rows.push(json!({"row": number, "term": term, "status": status, "head": head, "parts": parts}));
    }
    let read_addresses: Vec<String> = parts_addresses.iter().cloned().collect();
    let read_cards = if read_addresses.is_empty() {
        Vec::new()
    } else {
        zt.cards(&read_addresses, "text")?.cards
    };
    let read_map = card_map(&read_cards);
    for row in &mut rows {
        let number = row["row"].as_i64().unwrap_or(0);
        let term = at(row, "term");
        let head = at(row, "head");
        match read_map.get(&head) {
            Some(card) if at(card, "type") == "term" && at(card, "term") == term => {}
            Some(card) => {
                faults.push(json!({"row": number, "term": term, "status": "wrong_head", "reason": format!("`{head}` is `{}` `{}`, not the Term `{term}`", at(card, "type"), at(card, "term"))}));
                continue;
            }
            None => {
                faults.push(json!({"row": number, "term": term, "status": "missing_head", "reason": format!("`{head}` is not in the book tree")}));
                continue;
            }
        }
        for part in row["parts"].as_array().into_iter().flatten() {
            let part = ps::s(part);
            let ok = read_map
                .get(&part)
                .is_some_and(|card| at(card, "type") == "term" && at(card, "term") == term);
            if !ok {
                faults.push(json!({"row": number, "term": term, "status": "missing_part", "reason": format!("`{part}` is not a part of `{term}`")}));
            }
        }
    }
    let rows_total = cards_plan["rows"].as_array().map(Vec::len).unwrap_or(0);
    let rows_complete = faults.is_empty() && rows.len() == rows_total && rows_total > 0;

    // --- the Term list's authored title/body, exactly ---
    let authored_list = if plan["box"]["list"].is_object() {
        plan["box"]["list"].clone()
    } else {
        cards_plan["list"].clone()
    };
    let list_preserved = match view.list.as_ref() {
        Some(current_list) => {
            let same_address = at(&authored_list, "address").is_empty()
                || at(current_list, "address") == at(&authored_list, "address");
            let same_body = at(current_list, "body_sha256") == at(&authored_list, "body_sha256");
            let same_title = at(current_list, "title") == at(&authored_list, "title");
            same_address && same_body && same_title
        }
        None => false,
    };
    let list = json!({
        "address": authored_list["address"],
        "title": authored_list["title"],
        "authored_body_sha256": authored_list["body_sha256"],
        "current_body_sha256": view.list.as_ref().map(|list| at(list, "body_sha256")),
        "current_title": view.list.as_ref().map(|list| at(list, "title")),
        "preserved": list_preserved,
    });

    // --- the preservation equation for every changed body ---
    let before_cards: BTreeMap<String, String> = before["cards"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|card| (at(card, "address"), at(card, "body")))
        .collect();
    let mut changed = 0;
    let mut stripped_equal = 0;
    let mut violations = Vec::new();
    for (address, old) in &before_cards {
        let Some(card) = current.get(address) else {
            violations.push(json!({"address": address, "reason": "the Card is missing from the book tree"}));
            continue;
        };
        let new = at(card, "body");
        if &new == old {
            continue;
        }
        changed += 1;
        if zt_check::text::strip_term_links(&new) == *old {
            stripped_equal += 1;
        } else {
            violations.push(json!({"address": address, "reason": "stripping the Term links does not restore the planned body"}));
        }
    }
    let preservation = json!({
        "cards": before_cards.len(),
        "changed": changed,
        "stripped_equal": stripped_equal,
        "violations": violations,
        "complete": violations.is_empty(),
    });

    // --- zero writes on the second pass ---
    let second = if second_plan_path.is_file() {
        let second = read(&second_plan_path, "the second Term-link plan")?;
        let changed = second["counts"]["cards_changed"].as_u64().or_else(|| second["cards_changed"].as_u64()).unwrap_or(u64::MAX);
        let refused = second["counts"]["cards_refused"].as_u64().or_else(|| second["cards_refused"].as_u64()).unwrap_or(u64::MAX);
        let revision = second["archive"]["revision"].as_u64().or_else(|| second["archive_revision"].as_u64()).unwrap_or(0);
        let applied = link_apply["archive"]["revision_after"].as_u64().unwrap_or(0);
        let same_revision = revision == applied;
        let no_new_write = view.revision == revision;
        let complete = changed == 0 && refused == 0 && same_revision && no_new_write;
        json!({
            "path": second_plan_path.display().to_string(),
            "artifact": artifact(&second_plan_path)?,
            "cards_changed": changed,
            "cards_refused": refused,
            "archive_revision": revision,
            "applied_revision": applied,
            "no_writes": complete,
            "complete": complete,
        })
    } else {
        json!({"path": second_plan_path.display().to_string(), "complete": false, "no_writes": false, "reason": "the second Term-link plan was not retained"})
    };

    // --- the before/after diagnostics ---
    let before_diagnostics = plan["before"]["diagnostics"].clone();
    let mut diagnostics = json!({"status": "unknown", "reason": "the plan recorded no before diagnostics", "complete": false});
    if before_diagnostics.is_object() {
        if let Some(checker) = find_program("zt-check", "ZT_CHECK") {
            let after_log = p.work_path("term-pilot-after-check.log");
            let after_report = p.work_path("term-pilot-after-check.json");
            let addresses: Vec<String> = before_cards.keys().cloned().collect();
            let before_report_path = at(&before_diagnostics, "path");
            let after = run_checker(&checker, &addresses, &after_log, &after_report, &book);
            let old = util::read_json(&before_report_path);
            match (after, old) {
                (Ok(after), Ok(old)) => {
                    let old_diagnostics = diagnostics_by_card(&old, true);
                    let new_diagnostics = diagnostics_by_card(&after, true);
                    let mut differences = Vec::new();
                    for address in before_cards.keys() {
                        let old = old_diagnostics.get(address).cloned().unwrap_or_default();
                        let new = new_diagnostics.get(address).cloned().unwrap_or_default();
                        if old != new {
                            differences.push(json!({"address": address, "before": old, "after": new}));
                        }
                    }
                    diagnostics = json!({
                        "status": if differences.is_empty() { "complete" } else { "different" },
                        "before": artifact(Path::new(&before_report_path))?,
                        "after": artifact(&after_report)?,
                        "compared": before_cards.len(),
                        "differences": differences,
                        "complete": differences.is_empty(),
                        "excluded_rules": "T*",
                    });
                }
                (Err(error), _) => {
                    diagnostics = json!({"status": "unknown", "reason": format!("the after check failed: {error:#}"), "complete": false});
                }
                (_, Err(error)) => {
                    diagnostics = json!({"status": "unknown", "reason": format!("the before check cannot be read: {error:#}"), "complete": false});
                }
            }
        }
    }

    // --- the run report: actual usage, runtime and cleanup ---
    let run = if run_report_path.is_file() {
        let report = read(&run_report_path, "the run report")?;
        json!({
            "path": run_report_path.display().to_string(),
            "artifact": artifact(&run_report_path)?,
            "identity": report["identity"],
            "completeness": report["completeness"],
            "usage": {
                "status": report["usage"]["status"],
                "totals": report["usage"]["totals"],
                "known_runs": report["usage"]["known_runs"],
                "missing_runs": report["usage"]["missing_runs"],
            },
            "timing": {
                "wall_seconds": report["timing"]["wall_seconds"],
                "tool_seconds": report["timing"]["tool_seconds"],
                "external_run_seconds": report["timing"]["external_run_seconds"],
                "critical_path": report["timing"]["critical_path"],
            },
            "cleanup": {
                "content_complete": report["cleanup"]["content_complete"],
                "cleanup_complete": report["cleanup"]["cleanup_complete"],
                "run_complete": report["cleanup"]["run_complete"],
                "open": report["cleanup"]["open"],
            },
            "status": "measured",
        })
    } else {
        json!({"path": run_report_path.display().to_string(), "status": "unknown", "reason": "the run report was not retained"})
    };
    let claude = if !p.claude_usage.trim().is_empty() && Path::new(&p.claude_usage).is_file() {
        let usage = read(Path::new(&p.claude_usage), "the Claude usage record")?;
        let mut runs: Vec<Value> = Vec::new();
        if let Some(map) = usage["runs"].as_object() {
            for (run, facts) in map {
                runs.push(json!({"run": run, "agents": facts["agents"], "weighted": facts["weighted"]}));
            }
        }
        let agents: u64 = runs.iter().map(|run| run["agents"].as_u64().unwrap_or(0)).sum();
        let weighted: u64 = runs.iter().map(|run| run["weighted"].as_u64().unwrap_or(0)).sum();
        json!({
            "path": p.claude_usage,
            "artifact": artifact(Path::new(&p.claude_usage))?,
            "runs": runs,
            "agents": agents,
            "weighted": weighted,
            "status": "measured",
        })
    } else {
        json!({"status": "unknown", "reason": "no Claude usage record was given; run token-usage.mjs on the run's transcripts"})
    };

    // --- the owner's review sheet ---
    let (review, review_markdown) = review_sheet(&plan, &cards_plan, &cards_report, &link_plan);
    let review_path = p.work_path(REVIEW_JSON);
    util::write_atomic(&review_path, &ps::json_pretty(&review))?;
    util::write_atomic(&p.work_path(REVIEW_FILE), &review_markdown)?;

    // --- the verdict ---
    let mut checks = Checks::default();
    checks.require("rows.complete", rows_complete, format!("{} row(s) are not verified", rows_total.saturating_sub(rows.len()) + faults.len()));
    checks.require("list.preserved", list_preserved, "the Term list's authored title/body changed");
    checks.require("preservation.complete", preservation["complete"] == json!(true), "a changed body does not strip back to its planned text");
    checks.require("second.complete", second["complete"] == json!(true), "the second Term-link pass would write again");
    checks.require("diagnostics.complete", diagnostics["complete"] == json!(true), "the Term markup changed the language/Link diagnostics or no diagnostics were compared");
    checks.require("run.measured", run["status"] == json!("measured"), "the run report was not retained");
    checks.require_that("run.workflow", run["identity"]["workflow"] == plan["staging"]["contracts"], || {
        format!("the run report names contract {:?}, not the prepared {:?}", run["identity"]["workflow"], plan["staging"]["contracts"])
    });
    checks.require_that("run.book", run["identity"]["citation_key"] == json!(book), || {
        format!("the run report is for `{}`, not `{book}`", at(&run["identity"], "citation_key"))
    });
    let (complete, checks_value, problems) = checks.reported();
    let qualification_id = format!(
        "tq-{}",
        &canon::hash_value(&json!({
            "plan": plan["plan_id"],
            "rows": rows, "faults": faults, "list": list,
            "preservation": preservation, "second": second, "diagnostics": diagnostics,
            "run": run["artifact"], "claude": claude["artifact"],
            "review": review["review_id"],
        }))[..16]
    );
    let record = json!({
        "contract": CONTRACT,
        "kind": "term_pilot_qualification",
        "qualification_id": qualification_id,
        "plan_id": plan["plan_id"],
        "at": canon::utc_now_o(),
        "book": plan["book"],
        "box": {"archive": view.identity, "revision": view.revision, "cards": view.cards.len()},
        "rows": {
            "total": rows_total,
            "verified": rows.len(),
            "verified_rows": rows,
            "faults": faults,
            "complete": rows_complete,
        },
        "list": list,
        "preservation": preservation,
        "second": second,
        "diagnostics": diagnostics,
        "run": run,
        "claude": claude,
        "review": {
            "id": review["review_id"],
            "json": REVIEW_JSON,
            "markdown": REVIEW_FILE,
            "complete": review["complete"],
            "shortfall": review["shortfall"],
            "math": review["math"].as_array().map(Vec::len).unwrap_or(0),
            "concept": review["concept"].as_array().map(Vec::len).unwrap_or(0),
            "cards": review["cards"].as_array().map(Vec::len).unwrap_or(0),
        },
        "checks": checks_value,
        "problems": problems,
        "verdict": if complete { "complete" } else { "partial" },
        "decision": DECISION,
    });
    util::write_atomic(&p.work_path(QUALIFICATION_FILE), &ps::json_pretty(&record))?;
    Ok(record)
}

// --- record ----------------------------------------------------------------------------------------

/// The owner's decisions: every item is accepted or corrected, and a correction names its change.
/// An approval still needs the full S30 sample; a rejection names its unresolved work.
fn review_checks(review: &Value, checks: &mut Checks) -> Vec<Value> {
    let mut corrections = Vec::new();
    for section in ["math", "concept", "cards"] {
        for item in review[section].as_array().into_iter().flatten() {
            let verdict = at(item, "decision");
            let note = at(item, "note");
            let label = if at(item, "term").is_empty() { at(item, "address") } else { at(item, "term") };
            checks.require_that(
                "review.decided",
                ps::is_in(&verdict, &["accept", "correct"]),
                || format!("`{label}` has no decision (accept or correct)"),
            );
            if verdict == "correct" {
                let named = !blank(&note);
                checks.require_that("review.correction", named, || format!("`{label}` is corrected but names no correction"));
                if named {
                    corrections.push(json!({"item": label, "note": note}));
                }
            }
        }
    }
    corrections
}

fn record(zt: &mut Zt, p: &Params) -> Result<Value> {
    if p.decision.trim().is_empty() {
        bail!("-Decision is required: the owner's filled `term-pilot-review.json`");
    }
    let plan = read(&p.work_path(PLAN_FILE), "a term pilot plan")?;
    let qualification = read(&p.work_path(QUALIFICATION_FILE), "the term pilot qualification")?;
    if qualification["decision"] != json!(DECISION) {
        bail!("the qualification already carries a decision");
    }
    let review = read(Path::new(&p.decision), "the owner's review sheet")?;
    if at(&review, "review_id") != at(&qualification["review"], "id") {
        bail!(
            "the review sheet is for `{}`, not this qualification `{}`",
            at(&review, "review_id"),
            at(&qualification["review"], "id")
        );
    }
    let mut checks = Checks::default();
    let decision = at(&review, "decision");
    let corrections = review_checks(&review, &mut checks);
    let decided = review["math"].as_array().map(Vec::len).unwrap_or(0)
        + review["concept"].as_array().map(Vec::len).unwrap_or(0)
        + review["cards"].as_array().map(Vec::len).unwrap_or(0);
    checks.require_that("review.items", decided > 0, || "the review sheet decides no item".to_string());
    if decision == "approve" {
        checks.require("review.complete", review["complete"] == json!(true), "an approval needs the full three math terms, two concept terms and ten GUI Cards");
    } else {
        let unresolved = review["unresolved"].as_array().map(Vec::len).unwrap_or(0);
        checks.require("review.unresolved", unresolved > 0, "a rejection must name its unresolved work");
    }
    let _ = zt;
    let (ok, checks_value, problems) = checks.reported();
    let record = json!({
        "contract": CONTRACT,
        "kind": "term_pilot_decision",
        "ok": ok,
        "plan_id": plan["plan_id"],
        "qualification_id": qualification["qualification_id"],
        "review_id": at(&review, "review_id"),
        "at": canon::utc_now_o(),
        "decision": decision,
        "decided_items": decided,
        "corrections": corrections,
        "unresolved": review["unresolved"],
        "checks": checks_value,
        "problems": problems,
        "applies": [],
    });
    util::write_atomic(&p.work_path(DECISION_FILE), &ps::json_pretty(&record))?;
    Ok(record)
}

// --- unit tests ------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn card(address: &str, title: &str, body: &str) -> Value {
        json!({
            "address": address, "title": title, "body": body, "type": "idea",
            "facet": null, "emphasis": null, "level": null, "form": null,
            "parent": "K", "edge": "direct",
        })
    }

    #[test]
    fn a_tree_fingerprint_ignores_generated_projection_and_orders_addresses() {
        let a = card("K/2", "二", "正文二");
        let b = card("K/1", "一", "正文一");
        let mut noisy = b.clone();
        noisy["text"] = json!("generated text");
        noisy["term_use_targets"] = json!(["K/9"]);
        noisy["inbound"] = json!(4);
        assert_eq!(tree_fingerprint(&[a.clone(), noisy]), tree_fingerprint(&[b.clone(), a.clone()]));
        let mut changed = b;
        changed["body"] = json!("改过的正文");
        assert_ne!(tree_fingerprint(&[changed, a]), tree_fingerprint(&[card("K/1", "一", "正文一"), card("K/2", "二", "正文二")]));
    }

    #[test]
    fn a_plan_id_covers_every_part_it_authorizes() {
        let base = json!({"book": "K", "tree": "a", "staging": "b", "copy": "c", "list": "d", "before": "e", "authorization": "f"});
        let one = plan_id(&base);
        assert!(one.starts_with("tp-") && one.len() == 19);
        for key in ["book", "tree", "staging", "copy", "list", "before", "authorization"] {
            let mut changed = base.clone();
            changed[key] = json!("x");
            assert_ne!(plan_id(&changed), one, "{key} did not change the plan id");
        }
    }

    #[test]
    fn normalized_paths_compare_the_box_and_the_live_archive() {
        assert_eq!(norm_path(r"\\?\D:\zt\" ), "d:\\zt");
        assert_eq!(norm_path("D:/zt"), "d:\\zt");
        assert_ne!(norm_path(r"\\?\D:\zt"), norm_path(r"D:\zt-pilot"));
    }

    #[test]
    fn a_card_address_is_percent_encoded_for_the_gui_route() {
        assert_eq!(percent_encode("K/1|a"), "K%2F1%7Ca");
        assert_eq!(card_url("http://127.0.0.1:4717/", "K/1|a"), "http://127.0.0.1:4717#/card/K%2F1%7Ca");
        assert_eq!(card_url("", "K/1"), "#/card/K%2F1");
    }

    fn diagnostic(address: &str, rule: &str, key: &str) -> Value {
        json!({"address": address, "rule": rule, "fault": "f", "key": key, "severity": "block", "message": format!("{rule} {key}")})
    }

    #[test]
    fn term_rules_are_counted_apart_from_the_prose_diagnostics() {
        let before = json!({"diagnostics": [
            diagnostic("K/1", "R3", "a"),
            diagnostic("K/1", "T7", "b"),
            diagnostic("K/2", "L1", "c"),
        ]});
        let after = json!({"diagnostics": [
            diagnostic("K/1", "R3", "a"),
            diagnostic("K/1", "T7", "changed"),
            diagnostic("K/2", "L1", "c"),
            diagnostic("K/3", "T1", "d"),
        ]});
        let before = diagnostics_by_card(&before, true);
        let after = diagnostics_by_card(&after, true);
        assert_eq!(before, after, "the T rules must not count as prose differences");
        assert_eq!(before.get("K/1").map(Vec::len), Some(1));
    }

    #[test]
    fn a_changed_prose_diagnostic_is_named() {
        let before = diagnostics_by_card(&json!({"diagnostics": [diagnostic("K/1", "R3", "a")]}), true);
        let after = diagnostics_by_card(&json!({"diagnostics": [diagnostic("K/1", "R3", "b")]}), true);
        assert_ne!(before, after);
    }

    fn review_fixture() -> (Value, Value, Value, Value) {
        let plan = json!({
            "plan_id": "tp-0123456789abcdef",
            "box": {"gui": "http://127.0.0.1:4717/"},
            "review": {"math_terms": 3, "concept_terms": 2, "gui_cards": 10},
        });
        let rows: Vec<Value> = (1..=5)
            .map(|row| json!({
                "row": row, "term": format!("词{row}"), "meaning": format!("意思{row}"), "defined": "§1",
                "classification": {"kind": if row <= 3 { "math" } else { "concept" }},
            }))
            .collect();
        let report_rows: Vec<Value> = (1..=5)
            .map(|row| json!({
                "row": row, "term": format!("词{row}"),
                "publication": {"status": "committed", "head": format!("K/1|a|{row}"), "parts": [format!("K/1|a|{row}|a")]},
            }))
            .collect();
        let cards: Vec<Value> = (1..=12)
            .map(|n| json!({
                "address": format!("K/1|b|{n:02}"), "title": format!("卡片{n}"), "changed": true,
                "edits": (0..n).map(|e| json!({"term": format!("词{}", e % 5 + 1)})).collect::<Vec<_>>(),
            }))
            .collect();
        (
            plan,
            json!({"rows": rows}),
            json!({"rows": report_rows}),
            json!({"cards": cards}),
        )
    }

    #[test]
    fn the_review_sheet_names_three_math_two_concept_and_ten_cards() {
        let (plan, cards_plan, report, link) = review_fixture();
        let (review, markdown) = review_sheet(&plan, &cards_plan, &report, &link);
        assert_eq!(review["complete"], json!(true));
        assert_eq!(review["math"].as_array().map(Vec::len), Some(3));
        assert_eq!(review["concept"].as_array().map(Vec::len), Some(2));
        assert_eq!(review["cards"].as_array().map(Vec::len), Some(10));
        assert_eq!(review["shortfall"]["math_terms"], json!(0));
        let first = at(&review["cards"][0], "address");
        let second = at(&review["cards"][1], "address");
        assert_eq!(first, "K/1|b|12", "the most-used Card comes first");
        assert_eq!(second, "K/1|b|11");
        assert!(markdown.contains("## A. Mathematical terms"));
        assert!(markdown.contains("## C. Ten linked Cards"));
        assert!(markdown.contains("http://127.0.0.1:4717#/card/K%2F1%7Cb%7C12"));
    }

    #[test]
    fn a_short_review_names_every_missing_item() {
        let (plan, mut cards_plan, mut report, mut link) = review_fixture();
        cards_plan["rows"].as_array_mut().unwrap().truncate(1);
        report["rows"].as_array_mut().unwrap().truncate(1);
        link["cards"].as_array_mut().unwrap().truncate(2);
        let (review, _) = review_sheet(&plan, &cards_plan, &report, &link);
        assert_eq!(review["complete"], json!(false));
        assert_eq!(review["shortfall"]["math_terms"], json!(2));
        assert_eq!(review["shortfall"]["concept_terms"], json!(2));
        assert_eq!(review["shortfall"]["gui_cards"], json!(8));
    }

    #[test]
    fn a_decision_accepts_every_item_and_refuses_an_unnamed_correction() {
        let mut review = json!({
            "complete": true,
            "decision": "approve",
            "unresolved": [],
            "math": [{"term": "均衡", "decision": "accept", "note": ""}],
            "concept": [{"term": "剩余控制权", "decision": "accept", "note": ""}],
            "cards": [{"address": "K/1", "decision": "accept", "note": ""}],
        });
        let mut checks = Checks::default();
        let corrections = review_checks(&review, &mut checks);
        assert!(checks.failures().is_empty());
        assert!(corrections.is_empty());
        review["math"][0]["decision"] = json!("correct");
        review["math"][0]["note"] = json!("");
        let mut checks = Checks::default();
        let corrections = review_checks(&review, &mut checks);
        assert_eq!(checks.failures().len(), 1, "a correction must name the change");
        assert!(corrections.is_empty());
        review["math"][0]["note"] = json!("把「均衡」的公式改为 T1");
        let mut checks = Checks::default();
        let corrections = review_checks(&review, &mut checks);
        assert!(checks.failures().is_empty());
        assert_eq!(corrections.len(), 1);
        assert_eq!(at(&corrections[0], "note"), "把「均衡」的公式改为 T1");
    }
}
