//! `zt-digest publish`: the human review and live publication gate for changes to existing Cards
//! (S27, issue #119; design §8.5, §11.3, §12.3, §15-phase-3, D371/D372).
//!
//! Existing register rebuilds and changes to old hubs are changes to existing Cards, so they stay
//! review-first (D191, S12-AC08, S26-AC06). This tool is that review: it presents the exact
//! proposed changes of an accepted register plan and an accepted hub plan, records the reviewer's
//! decision on each item, publishes only the accepted ones through the same identified, receipt-
//! bound operations the qualified compilers use, and verifies the result by fresh reads.
//!
//! ```text
//! zt-digest publish -Step plan  -Work <folder> [-Register <register-plan.json>] [-Hub <hub-plan.json>]
//! zt-digest publish -Step apply -Work <folder> [-File <selection.json>]
//! zt-digest publish -Step check -Work <folder> [-File <publish-result.json>]
//! ```
//!
//! `plan` (S27-AC01, S27-AC03, S27-AC05) reads the archive through one `zt mcp` and writes
//! `<Work>\publish-review.json` with an `xp-` content id, its Markdown view, and one retained body
//! per read Card under `<Work>\publish-endpoints\`. It changes no Card. For the register it lists
//! every planned term with its targets (each target's Workspace, Zone, type, facet, title and
//! whether it is a substantive Card), its source rows, its sense evidence, the exact row the
//! register would carry, the row it carries today, the plan's unresolved terms and readings, the
//! accepted pinyin bands with their terms, and every part's old and planned body. For hubs it lists
//! every staged change: the exact old body and authored fingerprint of an existing Card, the new
//! body it would receive, whether an existing hub is reused, the crossing lines it adds, and the
//! `get_hub` policy evidence (mirror, `one-book`, anchor title). It also lists the protected scope
//! (every human Card and every agent Model Card with its authored fingerprint), the register-level
//! cycle check, and every target's membership.
//!
//! `apply` (S27-AC02, S27-AC04, S27-AC06) consumes `-File` (default `<Work>\publish-selection.json`):
//! the review id, the archive id, a nonempty note, the verified backup, whether the pinyin and
//! grouping evidence was reviewed, and the accepted and declined item ids. An item the selection
//! does not name is `declined` (silence is never permission). It re-reads the archive, refuses a
//! moved archive identity, a stale old base or a changed protected Card (a revised plan is the
//! answer, never a bypass), publishes the accepted hub changes in plan order and then the register
//! through [`crate::register::Register::publish_plan`] — parts first, the root last, each call
//! under a receipt derived from this review — and verifies the persisted bodies, the accepted
//! entries, the parts and the protected scope by fresh reads. Every item ends
//! `applied-and-verified`, `declined`, `conflict`, `pending` or `refused`; a tool that returned
//! without an error is never enough.
//!
//! `check` (S27-AC07) reads the result and the archive again and reports the current post-
//! publication revision, the accepted scope, the published entries and parts, and the protected
//! scope, so reader qualification has the evidence it needs.
//!
//! The register is a total rebuild, so declining one of its rows would move or drop an entry the
//! register already serves: such a decline is a `conflict` and publishes no register row, while a
//! declined Hub Card change simply leaves that Card as it was. Historical live execution is a
//! separate gate: this tool publishes only the exact plans and the exact items a reviewed
//! selection names, and it writes no Card the plans do not identify.

use crate::args::Args;
use crate::canon;
use crate::calls;
use crate::ps;
use crate::register;
use crate::util;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

/// The review's own contract. A later build reads the same shapes.
pub const CONTRACT: &str = "publish/1";
/// The immutable review, written by `-Step plan`.
pub const REVIEW_FILE: &str = "publish-review.json";
/// The review's Markdown view.
pub const REVIEW_MD: &str = "publish-review.md";
/// The reviewed selection, read by `-Step apply`.
pub const SELECTION_FILE: &str = "publish-selection.json";
/// The publication evidence, written by `-Step apply`.
pub const RESULT_FILE: &str = "publish-result.json";
/// The retained bodies of the Cards a review read.
const ENDPOINTS: &str = "publish-endpoints";
const SCHEMA_VERSION: i64 = 1;
/// A review id: `xp-` and the SHA-256 hex of its content.
const REVIEW_PREFIX: &str = "xp-";
/// The most Cards one protected-scope read names.
const MAX_PROTECTED: usize = 20_000;
/// One `get_cards` read chunk.
const READ_CHUNK: usize = 200;
/// The most items one review may name.
const MAX_ITEMS: usize = 4_000;
/// The most characters of a target Card body a review shows beside the row that names it.
const EXCERPT_CHARS: usize = 200;
/// `@@N`: a reference to the N-th call's own result (the identified-call contract).
static RESULT_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"@@(\d+)").unwrap());

struct Params {
    step: String,
    work: String,
    register: String,
    hub: String,
    file: String,
}

/// `zt-digest publish`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["Step", "Work", "Register", "Hub", "File"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &[])?;
    let p = Params {
        step: a.choice("Step", &["plan", "apply", "check"])?.to_ascii_lowercase(),
        work: a.required("Work")?,
        register: a.str("Register"),
        hub: a.str("Hub"),
        file: a.str("File"),
    };
    let work = full_path(&p.work)?;
    match p.step.as_str() {
        "plan" => plan(&work, &p),
        "apply" => apply(&work, &p),
        _ => check(&work, &p),
    }
}

// --- paths and small helpers ----------------------------------------------------------------------

fn full_path(path: &str) -> Result<String> {
    let path = std::path::Path::new(path);
    let full = std::path::absolute(path).with_context(|| format!("resolve {}", path.display()))?;
    Ok(full.to_string_lossy().to_string())
}

/// The SHA-256 of one file's bytes, as a recorded backup names it.
pub(crate) fn file_sha256(path: &str) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {path}"))?;
    Ok(canon::sha256_hex(&bytes))
}

fn tree_of(address: &str) -> String {
    address.split('/').next().unwrap_or_default().to_string()
}

fn is_topic_tree(tree: &str) -> bool {
    !tree.is_empty() && tree.chars().all(|ch| ch.is_ascii_digit())
}

pub(crate) fn blank(text: &str) -> bool {
    text.trim().is_empty()
}

/// The staged hub bodies are built with PowerShell`s line ending (`NL`); a Card stores `\n`. The
/// review shows, and the gate compares, the text the Card actually carries.
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// The content id a review carries: the SHA-256 of its content without its own id and creation
/// time, so an edited file is detected rather than trusted.
fn content_id(value: &Value) -> String {
    format!("{REVIEW_PREFIX}{}", canon::content_hash(value, &["review_id", "created_at"]))
}

fn is_review_id(value: &str) -> bool {
    value.len() == REVIEW_PREFIX.len() + 64
        && value.starts_with(REVIEW_PREFIX)
        && value[REVIEW_PREFIX.len()..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn term_id(term: &str) -> String {
    format!("term:{term}")
}

fn short_of(review: &Value) -> String {
    ps::s(&review["review_id"])
        .trim_start_matches(REVIEW_PREFIX)
        .chars()
        .take(16)
        .collect()
}

/// Every `[[address]]` a body names.
fn links_in(body: &str) -> Vec<String> {
    static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]*)?\]\]").unwrap());
    let mut out = Vec::new();
    for captures in LINK.captures_iter(body) {
        let target = captures[1].trim().to_string();
        // A `@@N` reference is a Card this plan creates; it has no address until publication.
        if !target.is_empty() && !target.starts_with("@@") && !out.contains(&target) {
            out.push(target);
        }
    }
    out
}

// --- the archive reads ----------------------------------------------------------------------------

/// The archive reads a review-first gate needs: identity, both Workspaces' title rows and one
/// coherent revision. `publish` (S27) and the historical batch gate (S29, [`crate::resolve`]) read
/// through it, so both compare the same facts about the same archive.
pub(crate) struct Archive {
    zt: register::Register,
    pub(crate) identity: Value,
    pub(crate) revision: u64,
    /// address → title row (`export_titles`) of both Workspaces.
    pub(crate) titles: BTreeMap<String, Value>,
}

impl Archive {
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            zt: register::publisher(),
            identity: Value::Null,
            revision: 0,
            titles: BTreeMap::new(),
        })
    }

    pub(crate) fn call(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        self.zt.call_tool(tool, arguments)
    }

    pub(crate) fn view(&mut self) -> Result<Value> {
        self.zt.client()?.archive_view()
    }

    /// One coherent read set: identity, both title lists. A revision that moves under the read is
    /// retried once, then reported as a changing view.
    pub(crate) fn read(&mut self) -> Result<()> {
        let mut last = String::new();
        for _ in 0..2 {
            let view = self.view()?;
            let identity = view["archive_identity"].clone();
            let revision = view["archive_revision"].as_u64().unwrap_or_default();
            let mut titles = BTreeMap::new();
            for workspace in ["agent", "human"] {
                let list = self.call("export_titles", json!({ "workspace": workspace }))?;
                for row in ps::pipe(&list["titles"]) {
                    let address = ps::s(&row["address"]);
                    let mut row = row.clone();
                    row["workspace"] = json!(workspace);
                    titles.insert(address, row);
                }
            }
            let after = self.view()?;
            if after["archive_revision"].as_u64() == Some(revision) {
                self.identity = identity;
                self.revision = revision;
                self.titles = titles;
                return Ok(());
            }
            last = format!("the archive changed while it was read ({revision} then {})", after["archive_revision"]);
        }
        bail!("{last}; run -Step plan again")
    }

    pub(crate) fn card(&mut self, address: &str) -> Result<Value> {
        self.call("get_card", json!({ "at": address }))
    }

    /// The authored bodies of the Cards a review names, tolerating a missing one: a target the
    /// archive no longer holds is a fault the review reports, not a plan that cannot be read.
    pub(crate) fn excerpts(&mut self, addresses: &[String]) -> Result<BTreeMap<String, Value>> {
        let mut out = BTreeMap::new();
        for chunk in addresses.chunks(READ_CHUNK) {
            let wanted = chunk.iter().map(|at| json!(at)).collect::<Vec<_>>();
            let read = self.call("get_cards", json!({ "addresses": wanted, "view": "text" }))?;
            for card in ps::pipe(&read["cards"]) {
                out.insert(ps::s(&card["address"]), card.clone());
            }
        }
        Ok(out)
    }

    pub(crate) fn cards(&mut self, addresses: &[String]) -> Result<BTreeMap<String, Value>> {
        let mut out = BTreeMap::new();
        for chunk in addresses.chunks(READ_CHUNK) {
            let wanted = chunk.iter().map(|at| json!(at)).collect::<Vec<_>>();
            let read = self.call("get_cards", json!({ "addresses": wanted, "view": "text" }))?;
            if read["complete"] != json!(true) {
                bail!("a reviewed Card could not be read: missing={}", ps::json_compact(&read["missing"]));
            }
            for card in ps::pipe(&read["cards"]) {
                out.insert(ps::s(&card["address"]), card.clone());
            }
        }
        Ok(out)
    }
}

/// One target of a register term, with the evidence a reviewer decides on (§8.3, AC03).
fn target_evidence(
    titles: &BTreeMap<String, Value>,
    bodies: &BTreeMap<String, Value>,
    address: &str,
    register_address: &str,
    part_of_register: &BTreeSet<String>,
) -> Value {
    let tree = tree_of(address);
    let topic = is_topic_tree(&tree);
    let row = titles.get(address);
    let kind = row.map(|r| ps::s(&r["type"])).unwrap_or_default();
    let facet = row.map(|r| r["facet"].clone()).unwrap_or(Value::Null);
    let title = row.map(|r| r["title"].clone()).unwrap_or(Value::Null);
    let workspace = row.map(|r| ps::s(&r["workspace"])).unwrap_or_default();
    let topic_root = topic && address == format!("{tree}/0");
    let literature_root = !topic && address == tree;
    let term_table = title.as_str().is_some_and(|text| text.trim_start().starts_with("术语表"));
    let hub = kind == "hub";
    let model_head = kind == "model" && facet.is_null();
    // The target's own text is the evidence the reviewer decides on (S27-AC01: target texts).
    let body = bodies.get(address).map(|card| ps::s(&card["body"])).unwrap_or_default();
    let excerpt = body.chars().take(EXCERPT_CHARS).collect::<String>();
    let tier = if hub || term_table || literature_root || part_of_register.contains(address) || address == register_address {
        Value::Null
    } else if topic_root {
        json!(3)
    } else if model_head {
        json!(2)
    } else {
        json!(1)
    };
    json!({
        "address": address,
        "workspace": workspace,
        "tree": tree,
        "zone": if topic { "topic" } else { "literature" },
        "type": kind,
        "facet": facet,
        "title": title,
        "topic_root": topic_root,
        "literature_root": literature_root,
        "hub": hub,
        "term_table": term_table,
        "exists": row.is_some(),
        "substantive": !tier.is_null(),
        "tier": tier,
        "body_chars": body.chars().count(),
        "excerpt": excerpt,
        "body_truncated": body.chars().count() > EXCERPT_CHARS,
    })
}

/// Every human Card and every agent Model Card with its authored fingerprint: the scope no
/// review may change (AC05, §11.3).
pub(crate) fn protected_scope(archive: &mut Archive) -> Result<Value> {
    let mut addresses: Vec<String> = Vec::new();
    for (address, row) in &archive.titles {
        let workspace = ps::s(&row["workspace"]);
        let model = ps::s(&row["type"]) == "model";
        if (workspace == "human" || model) && addresses.len() < MAX_PROTECTED {
            addresses.push(address.clone());
        }
    }
    addresses.sort_by(|a, b| canon::ordinal_cmp(a, b));
    let cards = archive.cards(&addresses)?;
    let mut rows = Vec::new();
    let mut digest = Vec::new();
    for address in &addresses {
        let card = cards.get(address);
        let fingerprint = card.map(calls::authored_fingerprint).unwrap_or_default();
        digest.push(format!("{address}\t{fingerprint}"));
        // The authored fields the fingerprint covers, so a reviewer sees the exact content the
        // gate protects and a check can name the field that moved.
        rows.push(json!({
            "address": address,
            "fingerprint": fingerprint,
            "type": card.map(|card| card["type"].clone()).unwrap_or(Value::Null),
            "title": card.map(|card| card["title"].clone()).unwrap_or(Value::Null),
            "facet": card.map(|card| card["facet"].clone()).unwrap_or(Value::Null),
            "level": card.map(|card| card["level"].clone()).unwrap_or(Value::Null),
            "form": card.map(|card| card["form"].clone()).unwrap_or(Value::Null),
        }));
    }
    Ok(json!({
        "cards": rows.len(),
        "truncated": addresses.len() >= MAX_PROTECTED,
        "digest": canon::sha256_hex(digest.join("\n").as_bytes()),
        "addresses": rows,
    }))
}

/// The protected Cards that no longer carry the reviewed fingerprint.
pub(crate) fn protected_diff(review: &Value, after: &Value) -> Vec<Value> {
    let before: BTreeMap<String, String> = ps::pipe(&review["protected"]["addresses"])
        .into_iter()
        .map(|row| (ps::s(&row["address"]), ps::s(&row["fingerprint"])))
        .collect();
    let mut changed = Vec::new();
    for row in ps::pipe(&after["addresses"]) {
        let address = ps::s(&row["address"]);
        let fingerprint = ps::s(&row["fingerprint"]);
        if before.get(&address) != Some(&fingerprint) {
            changed.push(json!({ "address": address, "reviewed": before.get(&address), "now": fingerprint }));
        }
    }
    changed
}

// --- plan: the review ------------------------------------------------------------------------------

fn plan(work: &str, p: &Params) -> Result<i32> {
    let out = ps::join(work, "out");
    let register_path = if p.register.trim().is_empty() {
        ps::join(&out, "register-plan.json")
    } else {
        full_path(&p.register)?
    };
    let hub_path = if p.hub.trim().is_empty() {
        ps::join(work, "hub-plan.json")
    } else {
        full_path(&p.hub)?
    };
    if !ps::exists(&register_path) {
        bail!("no register plan at `{register_path}`; stage one with `zt-digest register -Step plan`");
    }
    let review = build_review(work, &register_path, &hub_path)?;
    canon::save_json(&ps::join(work, REVIEW_FILE), &review)?;
    util::write_text(&ps::join(work, REVIEW_MD), &render_review(&review))?;
    util::print_last(&json!({
        "ok": true,
        "step": "plan",
        "review_id": review["review_id"],
        "contract": CONTRACT,
        "archive": review["archive"],
        "register": {
            "plan_id": review["register"]["plan_id"],
            "address": review["register"]["address"],
            "terms": review["register"]["terms"].as_array().map_or(0, Vec::len),
            "unresolved": review["register"]["unresolved"].as_array().map_or(0, Vec::len),
            "parts": review["register"]["parts"].as_array().map_or(0, Vec::len),
            "unresolved_readings": review["register"]["readings"]["unresolved"].as_array().map_or(0, Vec::len),
        },
        "hubs": {
            "plan_id": review["hubs"]["plan_id"],
            "items": review["hubs"]["items"].as_array().map_or(0, Vec::len),
        },
        "items": review["items"].as_array().map_or(0, Vec::len),
        "checks_failed": review["checks"].as_array().map_or(0, |checks| checks.iter().filter(|check| check["ok"] != json!(true)).count()),
        "protected": review["protected"]["cards"],
        "partial": review["partial"],
        "path": ps::join(work, REVIEW_FILE),
        "markdown": ps::join(work, REVIEW_MD),
    }))?;
    Ok(0)
}

fn build_review(work: &str, register_path: &str, hub_path: &str) -> Result<Value> {
    let plan = canon::read_json(register_path)?;
    if plan["schema_version"] != json!(SCHEMA_VERSION) {
        bail!("the register plan `{register_path}` is not schema version {SCHEMA_VERSION}");
    }
    if !ps::s(&plan["plan_id"]).starts_with("rp-") {
        bail!("the register plan `{register_path}` names no `rp-` plan id; run `zt-digest register -Step plan`");
    }
    let hub_plan = if ps::exists(hub_path) { Some(canon::read_json(hub_path)?) } else { None };
    let mut archive = Archive::new()?;
    let mut partial = false;
    let mut current = Value::Null;
    let mut published: Vec<(String, Vec<String>)> = Vec::new();
    for attempt in 0..2 {
        if let Err(error) = archive.read() {
            if attempt == 1 {
                bail!("{error:#}");
            }
            partial = true;
            continue;
        }
        let read = archive.zt.read_current(true);
        let entries = archive.zt.read_published();
        match (read, entries) {
            (Ok(view), Ok((_, rows))) => {
                current = view;
                published = rows;
                partial = false;
                break;
            }
            (Err(error), _) | (_, Err(error)) => {
                if attempt == 1 {
                    bail!("{error:#}");
                }
                partial = true;
            }
        }
    }
    if current.is_null() {
        bail!("the register could not be read; run -Step plan again");
    }
    let planned_identity = ps::s(&plan["archive"]["identity"]["id"]);
    if !planned_identity.is_empty() && planned_identity != ps::s(&archive.identity["id"]) {
        bail!(
            "the register plan belongs to archive `{planned_identity}`, not `{}`; stage it again",
            ps::s(&archive.identity["id"])
        );
    }
    let protected = protected_scope(&mut archive)?;
    let mut items: Vec<String> = Vec::new();
    let mut checks: Vec<Value> = Vec::new();
    let register = register_section(&mut archive, &plan, register_path, &current, &published, &mut items, &mut checks)?;
    let hubs = hub_section(&mut archive, hub_plan.as_ref(), hub_path, &mut items, &mut checks)?;
    if items.len() > MAX_ITEMS {
        bail!("the review names {} items; decide a smaller scope", items.len());
    }
    let mut review = json!({
        "schema_version": SCHEMA_VERSION,
        "contract": CONTRACT,
        "review_id": "",
        "created_at": canon::utc_now_o(),
        "archive": {"identity": archive.identity, "revision": archive.revision},
        "partial": partial,
        "scope": {
            "workspace": "agent",
            "writes": "existing Cards only: the register (its root and parts) and each accepted Hub Card body",
            "ordering": ["hubs", "register-parts", "register-root"],
            "not_published_here": "a new Card a qualified compiler creates for a new book is not this gate's; only the exact items below are published",
        },
        "register": register,
        "hubs": hubs,
        "protected": protected,
        "items": items,
        "checks": checks,
        "selection": {
            "path": ps::join(work, SELECTION_FILE),
            "requires": {"note": true, "backup": true, "readings_reviewed": true},
            "rule": "an item the selection does not name is declined: no implementation ticket, no new-book scope and no silence is permission for a change here",
        },
        "paths": {"work": work, "review": ps::join(work, REVIEW_FILE), "markdown": ps::join(work, REVIEW_MD), "endpoints": ps::join(work, ENDPOINTS)},
    });
    review["review_id"] = json!(content_id(&review));
    retain_bodies(work, &review)?;
    Ok(review)
}

/// Retain the exact bodies a review read, so a reviewer can compare them after the publication
/// without another archive read.
fn retain_bodies(work: &str, review: &Value) -> Result<()> {
    let dir = ps::join(work, ENDPOINTS);
    std::fs::create_dir_all(&dir).with_context(|| format!("create {dir}"))?;
    let mut rows: Vec<(String, String)> = Vec::new();
    rows.push(("register-root-before".into(), ps::s(&review["register"]["body_before"])));
    rows.push(("register-root-planned".into(), ps::s(&review["register"]["staged_body"])));
    for part in ps::pipe(&review["register"]["parts"]) {
        let label = file_key(&ps::s(&part["label"]));
        rows.push((format!("register-part-{label}-before"), ps::s(&part["body_before"])));
        rows.push((format!("register-part-{label}-planned"), ps::s(&part["body_planned"])));
    }
    for item in ps::pipe(&review["hubs"]["items"]) {
        let id = file_key(&ps::s(&item["id"]));
        rows.push((format!("hub-{id}-old"), ps::s(&item["old_body"])));
        rows.push((format!("hub-{id}-new"), ps::s(&item["new_body"])));
    }
    for (name, body) in rows {
        if body.trim().is_empty() {
            continue;
        }
        util::write_text(&ps::join(&dir, &format!("{name}.md")), &body)?;
    }
    Ok(())
}

/// A file name for one retained body.
fn file_key(text: &str) -> String {
    text.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
        .collect()
}

/// The register half of the review: every planned term, its evidence and its current row, the
/// accepted readings and bands, and each part's old and planned body.
fn register_section(
    archive: &mut Archive,
    plan: &Value,
    register_path: &str,
    current: &Value,
    published: &[(String, Vec<String>)],
    items: &mut Vec<String>,
    checks: &mut Vec<Value>,
) -> Result<Value> {
    // Every Card the planned rows name, read once so each target shows its own authored text.
    let mut wanted = ps::pipe(&plan["terms"])
        .into_iter()
        .flat_map(|row| ps::pipe(&row["targets"]).into_iter().map(ps::s).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    wanted.sort_by(|a, b| canon::ordinal_cmp(a, b));
    wanted.dedup();
    wanted.truncate(MAX_PROTECTED);
    let bodies = archive.excerpts(&wanted)?;
    let missing = current["missing"] == json!(true);
    let address = ps::s(&current["address"]);
    let root_card = if missing { Value::Null } else { archive.card(&address)? };
    let parts_current = ps::pipe(&current["parts"]).into_iter().cloned().collect::<Vec<_>>();
    let part_of_register: BTreeSet<String> = parts_current.iter().map(|part| ps::s(&part["address"])).collect();
    let published_by_term: BTreeMap<String, Vec<String>> = published.iter().cloned().collect();
    let mut terms = Vec::new();
    let mut literature_terms = 0;
    let mut topic_terms = 0;
    let mut faults = 0;
    for row in ps::pipe(&plan["terms"]) {
        let term = ps::s(&row["term"]);
        if term.is_empty() {
            continue;
        }
        let targets = ps::pipe(&row["targets"]).into_iter().map(ps::s).collect::<Vec<_>>();
        let evidence = targets
            .iter()
            .map(|target| target_evidence(&archive.titles, &bodies, target, &address, &part_of_register))
            .collect::<Vec<_>>();
        let substantive = evidence.iter().filter(|row| row["substantive"] == json!(true)).count();
        let literature = evidence
            .iter()
            .any(|row| row["zone"] == json!("literature") && row["substantive"] == json!(true));
        let topic = evidence
            .iter()
            .any(|row| row["zone"] == json!("topic") && row["substantive"] == json!(true));
        if literature {
            literature_terms += 1;
        }
        if topic {
            topic_terms += 1;
        }
        let mut term_checks = Vec::new();
        if evidence.iter().any(|row| row["exists"] != json!(true)) {
            term_checks.push(json!({"id": "target-membership", "ok": false, "detail": "a target is in neither Workspace"}));
        }
        if substantive != evidence.len() {
            term_checks.push(json!({"id": "target-substantive", "ok": false, "detail": "a target is a Hub Card, a term table, a Literature root or a register part"}));
        }
        if targets.len() != ps::select_unique(targets.clone()).len() {
            term_checks.push(json!({"id": "target-distinct", "ok": false, "detail": "the same target is named twice"}));
        }
        if targets.len() > 3 {
            term_checks.push(json!({"id": "target-count", "ok": false, "detail": "more than three targets"}));
        }
        if row["ambiguous"] == json!(true) {
            term_checks.push(json!({"id": "concept-ambiguous", "ok": false, "detail": "the concept review did not settle this spelling; two senses stay apart"}));
        }
        if term_checks.iter().any(|check| check["ok"] != json!(true)) {
            faults += 1;
        }
        let id = term_id(&term);
        items.push(id.clone());
        terms.push(json!({
            "id": id,
            "term": term,
            "targets": evidence,
            "sources": row["sources"],
            "ambiguous": row["ambiguous"],
            "reviewed": row["reviewed"],
            "read_only_targets": row["read_only_targets"],
            "overflow": row["overflow"],
            "line": register_line(&term, &targets),
            "published": published_by_term.get(&term)
                .map(|targets| json!({"targets": targets, "line": register_line(&term, targets)}))
                .unwrap_or(Value::Null),
            "checks": term_checks,
        }));
    }
    let declared = ps::pipe(&plan["parts"]).into_iter().cloned().collect::<Vec<_>>();
    let mut parts = Vec::new();
    for part in &declared {
        let label = ps::s(&part["label"]);
        let staged_address = ps::s(&part["address"]);
        let now = parts_current
            .iter()
            .find(|row| ps::s(&row["label"]) == label)
            .or_else(|| parts_current.iter().find(|row| ps::s(&row["address"]) == staged_address))
            .cloned()
            .unwrap_or(Value::Null);
        // A part the register already declares is a Card this gate would rewrite: its authored base
        // is shown beside its old body, so "all old addresses and fingerprints" holds for parts too.
        let fingerprint_before = if now["card"].is_object() {
            json!(calls::authored_fingerprint(&now["card"]))
        } else {
            Value::Null
        };
        parts.push(json!({
            "label": label,
            "address": if staged_address.is_empty() { now["address"].clone() } else { json!(staged_address) },
            "title": part["title"],
            "rows_planned": ps::pipe(&part["terms"]).len(),
            "body_before": now["body"],
            "fingerprint_before": fingerprint_before,
            "body_planned": part["body"],
            "reused": part["reused"],
            "terms": part["terms"],
        }));
    }
    let mut obsolete = Vec::new();
    for part in ps::pipe(&plan["obsolete_parts"]) {
        let part_address = ps::s(&part["address"]);
        let now = parts_current.iter().find(|row| ps::s(&row["address"]) == part_address).cloned().unwrap_or(Value::Null);
        obsolete.push(json!({
            "label": part["label"],
            "address": part_address,
            "body_before": now["body"],
            "body_planned": part["body"],
        }));
    }
    let readings = json!({
        "accepted": plan["readings"]["accepted"],
        "unresolved": plan["readings"]["unresolved"],
        "count": plan["readings"]["count"],
        "bands": band_evidence(&declared, &plan["readings"]["accepted"]),
    });
    checks.push(json!({
        "id": "register-targets",
        "ok": faults == 0,
        "detail": format!("{} of {} planned terms name only substantive, distinct, current targets", terms.len() - faults, terms.len()),
    }));
    checks.push(json!({
        "id": "register-destinations",
        "ok": topic_terms > 0 && literature_terms > 0,
        "detail": format!("{topic_terms} term(s) reach a substantive Topic Card and {literature_terms} reach a substantive Literature-tree Card"),
    }));
    checks.push(json!({
        "id": "register-unresolved-visible",
        "ok": true,
        "detail": format!(
            "{} unresolved term(s) and {} unresolved reading(s) stay visible",
            plan["unresolved"].as_array().map_or(0, Vec::len),
            plan["readings"]["unresolved"].as_array().map_or(0, Vec::len)
        ),
    }));
    let root_before = if missing { String::new() } else { ps::s(&root_card["body"]) };
    Ok(json!({
        "plan_id": plan["plan_id"],
        "path": register_path,
        "sha256": file_sha256(register_path)?,
        "archive_id": plan["archive"]["identity"]["id"],
        "missing": missing,
        "address": if missing { Value::Null } else { json!(address) },
        "title": root_card["title"],
        "body_before": root_before,
        "body_rest": body_rest(&root_before),
        "fingerprint_before": if missing { Value::Null } else { json!(calls::authored_fingerprint(&root_card)) },
        "staged_body": plan["body"],
        "terms_on_root": plan["terms_on_root"],
        "parts": parts,
        "obsolete_parts": obsolete,
        "terms": terms,
        "unresolved": plan["unresolved"],
        "faults": plan["faults"],
        "excluded": plan["excluded"],
        "review": plan["review"],
        "readings": readings,
        "register_faults": current["faults"],
        "entries_now": published.len(),
    }))
}

/// The exact register row a term writes (§8.4).
fn register_line(term: &str, targets: &[String]) -> String {
    format!(
        "- {term}：{}",
        targets.iter().map(|address| format!("[[{address}]]")).collect::<Vec<_>>().join("、")
    )
}

/// The first line two register frames differ in, so a refused rebuild names what moved.
fn frame_diff(expected: &str, actual: &str) -> String {
    let want: Vec<&str> = expected.lines().collect();
    let have: Vec<&str> = actual.lines().collect();
    for index in 0..want.len().max(have.len()) {
        let left = want.get(index).copied().unwrap_or("");
        let right = have.get(index).copied().unwrap_or("");
        if left != right {
            return format!("line {}: expected `{left}`, read `{right}`", index + 1);
        }
    }
    "the frames differ only in line endings".to_string()
}

/// The register body's own preamble and the groups the register does not own: the part of the body
/// a rebuild must preserve (D189, §8.4).
fn body_rest(body: &str) -> String {
    let mut preamble = Vec::new();
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    let mut inside: Option<usize> = None;
    for line in body.lines() {
        let trimmed = line.trim_end();
        if let Some(heading) = trimmed.trim().strip_prefix("## ") {
            sections.push((heading.trim().to_string(), Vec::new()));
            inside = Some(sections.len() - 1);
            continue;
        }
        match inside {
            None => preamble.push(trimmed.to_string()),
            Some(index) => sections[index].1.push(trimmed.to_string()),
        }
    }
    let mut out = vec![preamble.join("\n").trim().to_string()];
    for (heading, lines) in sections {
        if ["词条", "关键词", "词条分册"].contains(&heading.as_str()) {
            continue;
        }
        out.push(format!("## {heading}\n{}", lines.join("\n").trim()));
    }
    out.join("\n\n").trim().to_string()
}

/// The accepted pinyin bands: each part's label, its terms and their readings, so a reviewer can
/// check the grouping and the polyphonic cases (§8.4, D183).
fn band_evidence(parts: &[Value], accepted: &Value) -> Vec<Value> {
    let readings: BTreeMap<String, Value> = ps::pipe(accepted)
        .into_iter()
        .map(|row| (ps::s(&row["term"]), row.clone()))
        .collect();
    let mut bands = Vec::new();
    for part in parts {
        let terms = ps::pipe(&part["terms"])
            .into_iter()
            .map(|term| {
                let name = ps::s(term);
                json!({
                    "term": name,
                    "reading": readings.get(&name).map(|row| row["reading"].clone()).unwrap_or(Value::Null),
                    "initial": readings.get(&name).map(|row| row["initial"].clone()).unwrap_or(Value::Null),
                })
            })
            .collect::<Vec<_>>();
        bands.push(json!({
            "label": part["label"],
            "bucket": part["bucket"],
            "address": part["address"],
            "rows": terms.len(),
            "terms": terms,
        }));
    }
    bands
}

/// The hub half of the review: each staged change, with the old body and fingerprint it replaces,
/// the new body, the hub it reuses and the policy evidence §8.2 asks for.
fn hub_section(archive: &mut Archive, hub_plan: Option<&Value>, hub_path: &str, items: &mut Vec<String>, checks: &mut Vec<Value>) -> Result<Value> {
    let Some(plan) = hub_plan else {
        return Ok(json!({
            "plan_id": Value::Null,
            "path": Value::Null,
            "items": [],
            "note": "no staged hub plan: this review publishes register changes only (stage one with `zt-digest hub -Step stage`)",
        }));
    };
    if plan["schema_version"] != json!(SCHEMA_VERSION) {
        bail!("the hub plan is not schema version {SCHEMA_VERSION}");
    }
    if !ps::s(&plan["plan_id"]).starts_with("hp-") {
        bail!("the hub plan names no `hp-` plan id; run `zt-digest hub -Step stage`");
    }
    let archive_id = ps::s(&plan["archive"]["identity"]["id"]);
    if !archive_id.is_empty() && archive_id != ps::s(&archive.identity["id"]) {
        bail!("the hub plan belongs to archive `{archive_id}`; stage it again");
    }
    let mut rows = Vec::new();
    let mut policy_faults = 0;
    for item in ps::pipe(&plan["items"]) {
        let id = ps::s(&item["id"]);
        let code = ps::s(&item["kind"]);
        let address = ps::s(&item["address"]);
        let mut item_checks = Vec::new();
        if code == "edit" {
            let card = archive.card(&address)?;
            let reviewed = ps::s(&item["old_fingerprint"]);
            if reviewed.is_empty() {
                bail!("the hub plan's item `{id}` names no authored base for `{address}`; run `zt-digest hub -Step stage` again");
            }
            // The review presents changes the archive still holds. A base that moved since the plan
            // was staged makes every item it touches a change nobody can review as written (§11.3).
            let now = if card.is_null() { String::new() } else { calls::authored_fingerprint(&card) };
            if now != reviewed {
                bail!(
                    "the hub plan is stale: the authored base of `{address}` changed since it was staged; run `zt-digest hub -Step stage` again, then -Step plan"
                );
            }
            if item["policy"]["mirror"] == json!(true) {
                item_checks.push(json!({"id": "mirror", "ok": false, "detail": "this topic hub only repeats its tree map; the entry-section alternative is reported, not written"}));
            }
            if !item["policy"]["one_book"].as_array().is_none_or(Vec::is_empty) {
                item_checks.push(json!({"id": "one-book", "ok": true, "detail": "a supported other-book treatment is reported for the reviewer"}));
            }
        }
        if !item["anchor_title"].is_null() && item["anchor_title"] != json!("") {
            item_checks.push(json!({"id": "anchor-title", "ok": false, "detail": "the hub title repeats its anchor's title"}));
        }
        if ps::s(&item["level"]) == "register" && code == "create" {
            item_checks.push(json!({"id": "register-cycle", "ok": false, "detail": "a second Hub Card of level register would be created"}));
        }
        let body = ps::s(&item["new_body"]);
        let mut members = Vec::new();
        for target in links_in(&body) {
            let member = archive.titles.contains_key(&target);
            if !member {
                item_checks.push(json!({"id": "link-membership", "ok": false, "detail": format!("the new body links `{target}`, which is in neither Workspace")}));
            }
            members.push(json!({"target": target, "member": member}));
        }
        if item_checks.iter().any(|check| check["ok"] != json!(true)) {
            policy_faults += 1;
        }
        items.push(id.clone());
        // `@@N` is a Card this plan creates; the review shows which staged change it is.
        let body = normalize(&body);
        let display = RESULT_REF
            .replace_all(&body, |captures: &regex::Captures| {
                let n = captures[1].parse::<usize>().unwrap_or(usize::MAX);
                format!("〈待建枢纽 hub:{n:03}〉")
            })
            .to_string();
        rows.push(json!({
            "id": id,
            "kind": code,
            "new_body_display": display,
            "calls": item["calls"],
            "level": item["level"],
            "tree": item["tree"],
            "address": if code == "edit" { json!(address) } else { Value::Null },
            "title": item["title"],
            "old_body": normalize(&ps::s(&item["old_body"])),
            "old_fingerprint": item["old_fingerprint"],
            "new_body": normalize(&body),
            "reuse": item["reuse"],
            "why": item["why"],
            "crossings": item["crossings"],
            "policy": item["policy"],
            "members": members,
            "checks": item_checks,
        }));
    }
    checks.push(json!({
        "id": "hub-policy",
        "ok": policy_faults == 0,
        "detail": format!("{} of {} staged hub change(s) pass the base, mirror, anchor-title, membership and register-cycle checks", rows.len() - policy_faults, rows.len()),
    }));
    Ok(json!({
        "plan_id": plan["plan_id"],
        "path": hub_path,
        "sha256": file_sha256(hub_path)?,
        "items": rows,
        "dropped": plan["dropped"],
        "trees": plan["trees"],
    }))
}

// --- the review's Markdown -----------------------------------------------------------------------

fn render_review(review: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Existing register and crossing-hub review (S27, #119)\n\n");
    out.push_str(&format!(
        "Review `{}` · {} · archive `{}` · revision {}\n\n",
        ps::s(&review["review_id"]),
        ps::s(&review["created_at"]),
        ps::s(&review["archive"]["identity"]["id"]),
        review["archive"]["revision"]
    ));
    out.push_str(&format!(
        "Register plan `{}` · hub plan `{}` · {} item(s) · protected scope: {} human and Model Card(s)\n\n",
        ps::s(&review["register"]["plan_id"]),
        if review["hubs"]["plan_id"].is_null() { "none".to_string() } else { ps::s(&review["hubs"]["plan_id"]) },
        review["items"].as_array().map_or(0, Vec::len),
        review["protected"]["cards"]
    ));
    out.push_str("## Policy checks\n\n| Check | Result | Detail |\n| --- | --- | --- |\n");
    for check in ps::pipe(&review["checks"]) {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            cell(&ps::s(&check["id"])),
            if check["ok"] == json!(true) { "ok" } else { "review" },
            cell(&ps::s(&check["detail"]))
        ));
    }
    out.push('\n');
    let register = &review["register"];
    out.push_str("## The register\n\n");
    out.push_str(&format!(
        "Root `{}` · {} term(s) · {} part(s) · {} published entries now\n\n",
        if register["address"].is_null() { "missing".to_string() } else { ps::s(&register["address"]) },
        register["terms"].as_array().map_or(0, Vec::len),
        register["parts"].as_array().map_or(0, Vec::len),
        register["entries_now"]
    ));
    out.push_str("### Terms\n\n| Item | Term | Proposed row | Targets | Checks |\n| --- | --- | --- | --- | --- |\n");
    for term in ps::pipe(&register["terms"]) {
        let targets = ps::pipe(&term["targets"])
            .into_iter()
            .map(|target| {
                format!(
                    "{} ({}/{}/{})",
                    ps::s(&target["address"]),
                    ps::s(&target["zone"]),
                    ps::s(&target["type"]),
                    if target["substantive"] == json!(true) { "substantive" } else { "not substantive" }
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let failed = term["checks"].as_array().is_some_and(|checks| checks.iter().any(|check| check["ok"] != json!(true)));
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            cell(&ps::s(&term["id"])),
            cell(&ps::s(&term["term"])),
            cell(&ps::s(&term["line"])),
            cell(&targets),
            if failed { "review" } else { "ok" }
        ));
    }
    out.push('\n');
    if let Some(bands) = register["readings"]["bands"].as_array() {
        if !bands.is_empty() {
            out.push_str("### Pinyin grouping\n\n| Part | Bucket | Rows | Terms |\n| --- | --- | --- | --- |\n");
            for band in bands {
                let terms = ps::pipe(&band["terms"])
                    .into_iter()
                    .map(|term| format!("{}({})", ps::s(&term["term"]), ps::s(&term["reading"])))
                    .collect::<Vec<_>>()
                    .join("、");
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    cell(&ps::s(&band["label"])),
                    cell(&ps::s(&band["bucket"])),
                    band["rows"],
                    cell(&terms)
                ));
            }
            out.push('\n');
        }
    }
    for part in ps::pipe(&register["parts"]) {
        out.push_str(&format!(
            "### Part {} ({})\n\nAddress `{}` · {} planned row(s) · reused: {}\n\nBefore:\n\n```\n{}\n```\n\nPlanned:\n\n```\n{}\n```\n\n",
            ps::s(&part["label"]),
            ps::s(&part["title"]),
            ps::s(&part["address"]),
            part["rows_planned"],
            part["reused"],
            ps::s(&part["body_before"]),
            ps::s(&part["body_planned"])
        ));
    }
    if !register["unresolved"].as_array().is_none_or(Vec::is_empty) || !register["readings"]["unresolved"].as_array().is_none_or(Vec::is_empty) {
        out.push_str("### Unresolved\n\n");
        for row in ps::pipe(&register["unresolved"]) {
            out.push_str(&format!("- term `{}`: {}\n", ps::s(&row["term"]), ps::s(&row["reason"])));
        }
        for row in ps::pipe(&register["readings"]["unresolved"]) {
            out.push_str(&format!("- reading `{}`: {}\n", ps::s(&row["term"]), ps::s(&row["reason"])));
        }
        out.push('\n');
    }
    if !review["hubs"]["items"].as_array().is_none_or(Vec::is_empty) {
        out.push_str("## The hub changes\n\n");
        for item in ps::pipe(&review["hubs"]["items"]) {
            out.push_str(&format!(
                "### {} · {} · {} `{}`\n\nReuse: {} · {}\n\nBefore:\n\n```\n{}\n```\n\nPlanned:\n\n```\n{}\n```\n\n",
                ps::s(&item["id"]),
                ps::s(&item["kind"]),
                ps::s(&item["level"]),
                if item["address"].is_null() { "(new)".to_string() } else { ps::s(&item["address"]) },
                item["reuse"],
                ps::s(&item["why"]),
                ps::s(&item["old_body"]),
                ps::s(&item["new_body_display"])
            ));
            for crossing in ps::pipe(&item["crossings"]) {
                out.push_str(&format!("- crossing: {}\n", ps::s(crossing)));
            }
            out.push('\n');
        }
    }
    out.push_str("## How to decide\n\n");
    out.push_str(&format!(
        "Write `{}` with `schema_version: 1`, this review id, the archive id, a nonempty note, the verified backup, `readings_reviewed: true`, and the item ids to `accept` and `decline`. An item the selection does not name is declined.\n",
        ps::s(&review["selection"]["path"])
    ));
    out
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

// --- apply: the publication ----------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Decision {
    id: String,
    accept: bool,
    note: String,
}

#[derive(Debug)]
struct Selection {
    note: String,
    readings_reviewed: bool,
    backup: Value,
    decisions: Vec<Decision>,
}

fn apply(work: &str, p: &Params) -> Result<i32> {
    let review_path = ps::join(work, REVIEW_FILE);
    if !ps::exists(&review_path) {
        bail!("no review at `{review_path}`; run -Step plan first");
    }
    let review = canon::read_json(&review_path)?;
    if review["contract"] != json!(CONTRACT) {
        bail!("the review at `{review_path}` is not `{CONTRACT}`");
    }
    let claimed = ps::s(&review["review_id"]);
    if !is_review_id(&claimed) || content_id(&review) != claimed {
        bail!("the review at `{review_path}` no longer matches its own content id; run -Step plan again");
    }
    let selection_path = if p.file.trim().is_empty() {
        ps::join(work, SELECTION_FILE)
    } else {
        full_path(&p.file)?
    };
    if !ps::exists(&selection_path) {
        bail!("no reviewed selection at `{selection_path}`; write one from the review and run again");
    }
    let asked = canon::read_json(&selection_path)?;
    let selection = validate_selection(&review, &asked, &selection_path)?;
    let mut archive = Archive::new()?;
    archive.read()?;
    if ps::s(&archive.identity["id"]) != ps::s(&review["archive"]["identity"]["id"]) {
        bail!(
            "the review belongs to archive `{}`, not `{}`",
            ps::s(&review["archive"]["identity"]["id"]),
            ps::s(&archive.identity["id"])
        );
    }
    let result = publish(&mut archive, work, &review, &selection, &selection_path)?;
    canon::save_json(&ps::join(work, RESULT_FILE), &result)?;
    let ok = result["requires_revision"] == json!(false) && result["summary"]["conflict"] == json!(0);
    util::print_last(&json!({
        "ok": ok,
        "step": "apply",
        "review_id": review["review_id"],
        "archive": result["archive"],
        "summary": result["summary"],
        "register": result["register"],
        "hubs": result["hubs"],
        "protected": result["protected"],
        "backup": result["backup"],
        "requires_revision": result["requires_revision"],
        "path": ps::join(work, RESULT_FILE),
    }))?;
    Ok(if ok { 0 } else { 1 })
}

fn validate_selection(review: &Value, asked: &Value, path: &str) -> Result<Selection> {
    if asked["schema_version"] != json!(1) {
        bail!("the selection `{path}` is not schema version 1");
    }
    if ps::s(&asked["review_id"]) != ps::s(&review["review_id"]) {
        bail!(
            "the selection names review `{}`, not `{}`; decide the review it was written from",
            ps::s(&asked["review_id"]),
            ps::s(&review["review_id"])
        );
    }
    if ps::s(&asked["archive_id"]) != ps::s(&review["archive"]["identity"]["id"]) {
        bail!("the selection names another archive than the review");
    }
    let note = ps::s(&asked["note"]);
    if blank(&note) {
        bail!("the selection needs a nonempty note: why these changes are accepted");
    }
    let known = item_ids(review);
    let known_set: BTreeSet<String> = known.iter().cloned().collect();
    let mut decisions: BTreeMap<String, Decision> = BTreeMap::new();
    for (list, accept) in [("accept", true), ("decline", false)] {
        for id in ps::pipe(&asked[list]) {
            let id = ps::s(id);
            if !known_set.contains(&id) {
                bail!("the selection {list}s `{id}`, which this review does not name");
            }
            if decisions
                .insert(id.clone(), Decision { id: id.clone(), accept, note: String::new() })
                .is_some()
            {
                bail!("the selection names `{id}` twice");
            }
        }
    }
    if let Some(notes) = asked["notes"].as_object() {
        for (id, text) in notes {
            match decisions.get_mut(id) {
                Some(decision) => decision.note = ps::s(text),
                None => bail!("the selection carries a note for `{id}`, which it does not decide"),
            }
        }
    }
    // Silence is never permission: every item is declined unless the selection accepts it.
    for id in &known {
        decisions.entry(id.clone()).or_insert(Decision {
            id: id.clone(),
            accept: false,
            note: "not-selected".to_string(),
        });
    }
    let accepted = decisions.values().filter(|decision| decision.accept).count();
    let readings_reviewed = asked["readings_reviewed"] == json!(true);
    let has_terms = decisions.values().any(|decision| decision.accept && decision.id.starts_with("term:"));
    if has_terms && !readings_reviewed {
        bail!("the selection accepts register terms without confirming the pinyin/grouping evidence (`readings_reviewed: true`)");
    }
    let backup = asked["backup"].clone();
    if accepted > 0 {
        if !backup.is_object() {
            bail!("the selection accepts changes without a verified backup (`backup`: path, sha256, bytes)");
        }
        let backup_path = ps::s(&backup["path"]);
        if blank(&backup_path) || !ps::exists(&backup_path) {
            bail!("the selected backup `{backup_path}` does not exist");
        }
        let size = std::fs::metadata(&backup_path).map(|meta| meta.len()).unwrap_or_default();
        if let Some(bytes) = backup["bytes"].as_u64() {
            if bytes != size {
                bail!("the selected backup is {size} bytes, not the recorded {bytes}");
            }
        }
        let sha = file_sha256(&backup_path)?;
        if ps::s(&backup["sha256"]) != sha {
            bail!("the selected backup's SHA-256 is {sha}, not the recorded `{}`", ps::s(&backup["sha256"]));
        }
        verify_backup(&backup_path, review)?;
    }
    Ok(Selection {
        note,
        readings_reviewed,
        backup,
        decisions: decisions.into_values().collect(),
    })
}

fn item_ids(review: &Value) -> Vec<String> {
    ps::pipe(&review["items"]).into_iter().map(ps::s).collect()
}

/// The selected backup must be this archive's own and hold Cards: a backup of another archive or
/// of nothing protects nothing (AC06).
fn verify_backup(path: &str, review: &Value) -> Result<()> {
    verify_backup_of(path, &ps::s(&review["archive"]["identity"]["id"]))
}

/// The same check against a named archive identity, for a gate whose review records the archive
/// id directly.
pub(crate) fn verify_backup_of(path: &str, expected: &str) -> Result<()> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("the selected backup `{path}` is not a readable SQLite database"))?;
    let ident: Option<String> = conn
        .query_row("SELECT value FROM metadata WHERE key='archive_id'", [], |row| row.get(0))
        .ok()
        .or_else(|| conn.query_row("SELECT value FROM metadata WHERE key='archive_identity'", [], |row| row.get(0)).ok());
    if let Some(archived) = ident {
        if !archived.contains(&expected[..expected.len().min(16)]) {
            bail!("the selected backup is another archive (`{archived}`), not `{expected}`");
        }
    }
    let cards: i64 = conn.query_row("SELECT count(*) FROM cards", [], |row| row.get(0)).unwrap_or(0);
    if cards <= 0 {
        bail!("the selected backup holds no Cards");
    }
    Ok(())
}

fn publish(archive: &mut Archive, work: &str, review: &Value, selection: &Selection, selection_path: &str) -> Result<Value> {
    let before = archive.revision;
    let mut outcomes: Vec<Value> = Vec::new();
    let mut requires_revision = false;
    // The protected scope first: a human or Model Card that changed since the review means the
    // archive moved under the decision, so nothing is written (D270, AC05).
    let protected_now = protected_scope(archive)?;
    let changed = protected_diff(review, &protected_now);
    if !changed.is_empty() {
        for decision in &selection.decisions {
            outcomes.push(outcome(&decision.id, decision.accept, "pending", "the protected human/Model scope changed since the review: a quiet window is required", Value::Null));
        }
        return Ok(result_record(review, selection, selection_path, before, before, &outcomes, &Value::Null, &Value::Null, json!({
            "before": review["protected"]["digest"],
            "after": protected_now["digest"],
            "unchanged": false,
            "changed": changed,
        }), true, archive));
    }
    // The hub changes first: the register body keeps the field navigation those changes write.
    let hub = hub_items(review, selection);
    let hub_plan = if review["hubs"]["plan_id"].is_null() {
        None
    } else {
        let path = ps::s(&review["hubs"]["path"]);
        let plan = canon::read_json(&path).with_context(|| format!("read the staged hub plan `{path}`"))?;
        if ps::s(&review["hubs"]["sha256"]) != file_sha256(&path)? {
            bail!("the staged hub plan `{path}` changed since the review; run -Step plan again");
        }
        if ps::s(&plan["plan_id"]) != ps::s(&review["hubs"]["plan_id"]) {
            bail!("the staged hub plan `{path}` is no longer the reviewed plan; run -Step plan again");
        }
        Some(plan)
    };
    let declined_calls: BTreeSet<usize> = hub
        .iter()
        .filter(|(_, accept)| !accept)
        .flat_map(|(item, _)| item["calls"].as_array().cloned().unwrap_or_default())
        .filter_map(|call| call.as_u64().map(|n| n as usize))
        .collect();
    let mut addresses: BTreeMap<usize, String> = BTreeMap::new();
    // A retry or a resume of an interrupted apply: a previous result of this same review names the
    // address each applied item reached, and an item whose Card already carries the reviewed body
    // is not written again (S27-AC04: no duplicate Card, no invented completion).
    let previous = previous_addresses(work, review);
    for (id, (address, _)) in &previous {
        if let Some((item, _)) = hub.iter().find(|(item, _)| ps::s(&item["id"]) == *id) {
            for call in item["calls"].as_array().into_iter().flatten().filter_map(Value::as_u64) {
                addresses.insert(call as usize, address.clone());
            }
        }
    }
    let mut wrote = 0usize;
    let mut replayed = 0usize;
    for (item, accept) in &hub {
        let id = ps::s(&item["id"]);
        if !accept {
            outcomes.push(outcome(&id, false, "declined", "the selection does not accept this hub change", Value::Null));
            continue;
        }
        if let Some((address, superseded)) = previous.get(&id) {
            let expected = normalize(&ps::s(&rewrite_refs(
                &json!(ps::s(&item["new_body"])),
                &mut |n| addresses.get(&n).cloned(),
            )));
            if let Ok(card) = archive.card(address)
                && !card.is_null()
                && (*superseded || normalize(&ps::s(&card["body"])) == expected)
            {
                replayed += 1;
                for call in item["calls"].as_array().into_iter().flatten().filter_map(Value::as_u64) {
                    addresses.insert(call as usize, address.clone());
                }
                outcomes.push(outcome(
                    &id,
                    true,
                    "applied-and-verified",
                    "the reviewed body is already persisted at the address its own result recorded",
                    json!({"address": address, "fingerprint": calls::authored_fingerprint(&card), "already": true}),
                ));
                continue;
            }
        }
        let Some(plan) = hub_plan.as_ref() else {
            outcomes.push(outcome(&id, true, "refused", "the review names a hub change but no staged hub plan is readable", Value::Null));
            requires_revision = true;
            continue;
        };
        match publish_hub_item(archive, review, plan, item, &declined_calls, &mut addresses) {
            Ok((value, item_wrote, item_replayed)) => {
                wrote += item_wrote;
                replayed += item_replayed;
                outcomes.push(value);
            }
            Err(error) => {
                let status = if canon::is_refusal(&error) { "refused" } else { "conflict" };
                outcomes.push(outcome(&id, true, status, &format!("{error:#}"), Value::Null));
                requires_revision = true;
            }
        }
    }
    let state = match hub_plan.as_ref() {
        Some(plan) => hub_state_after(work, plan, &addresses, &hub)?,
        None => json!({"state": "no-plan"}),
    };
    // Then the register: re-stage the accepted rows against the current register body and publish
    // the parts first and the root last, through the same identified operations (S27-AC04). An
    // accepted hub change may have rewritten the register's own field-navigation groups first, so
    // that body is the frame the rebuild must find, not the one the review read.
    let register_address = ps::s(&review["register"]["address"]);
    let mut expected_rest = ps::s(&review["register"]["body_rest"]);
    for (item, accept) in &hub {
        if *accept && ps::s(&item["address"]) == register_address && ps::s(&item["kind"]) == "edit" {
            // The planned body names a hub this plan creates as `@@N`; the Card carries its address.
            let body = rewrite_refs(&json!(ps::s(&item["new_body"])), &mut |n| addresses.get(&n).cloned());
            expected_rest = body_rest(&ps::s(&body));
        }
    }
    let register = publish_register(archive, review, selection, &mut outcomes, &expected_rest)?;
    if register["applied"] != json!(true) {
        requires_revision = true;
    }
    // The register publication rewrites the register Card last, so an accepted hub change to that
    // Card is now part of the verified register rather than the end of its own body: say so instead
    // of leaving a fingerprint that no longer holds.
    let register_address = ps::s(&review["register"]["address"]);
    if register["applied"] == json!(true) {
        for outcome in outcomes.iter_mut() {
            if outcome["accepted"] == json!(true) && ps::s(&outcome["verified"]["address"]) == register_address {
                outcome["verified"]["superseded_by_register"] = json!(true);
                outcome["detail"] = json!("the field-navigation body is persisted; the accepted register publication then rewrote the same Card and verified it");
            }
        }
    }
    let protected_after = protected_scope(archive)?;
    let after = archive.view()?;
    let after_revision = after["archive_revision"].as_u64().unwrap_or_default();
    let hub_summary = json!({
        "accepted": hub.iter().filter(|(_, accept)| *accept).count(),
        "declined": hub.iter().filter(|(_, accept)| !accept).count(),
        "wrote": wrote,
        "replayed": replayed,
        "state": state["state"],
    });
    Ok(result_record(
        review,
        selection,
        selection_path,
        before,
        after_revision,
        &outcomes,
        &register,
        &hub_summary,
        json!({
            "before": review["protected"]["digest"],
            "after": protected_after["digest"],
            "unchanged": protected_after["digest"] == review["protected"]["digest"],
            "changed": protected_diff(review, &protected_after),
        }),
        requires_revision,
        archive,
    ))
}

fn outcome(id: &str, accept: bool, status: &str, detail: &str, verified: Value) -> Value {
    json!({
        "id": id,
        "accepted": accept,
        "status": status,
        "detail": detail,
        "verified": verified,
    })
}

/// The addresses a previous result of this same review recorded for its applied items: a retry
/// recognises what is already published instead of writing it twice (AC04).
fn previous_addresses(work: &str, review: &Value) -> BTreeMap<String, (String, bool)> {
    let path = ps::join(work, RESULT_FILE);
    let mut out = BTreeMap::new();
    if !ps::exists(&path) {
        return out;
    }
    let Ok(result) = canon::read_json(&path) else {
        return out;
    };
    if ps::s(&result["review_id"]) != ps::s(&review["review_id"]) {
        return out;
    }
    for outcome in ps::pipe(&result["outcomes"]) {
        if outcome["accepted"] != json!(true) || ps::s(&outcome["status"]) != "applied-and-verified" {
            continue;
        }
        let address = ps::s(&outcome["verified"]["address"]);
        if !address.is_empty() {
            out.insert(ps::s(&outcome["id"]), (address, outcome["verified"]["superseded_by_register"] == json!(true)));
        }
    }
    out
}

fn hub_items<'a>(review: &'a Value, selection: &Selection) -> Vec<(&'a Value, bool)> {
    let accepted: BTreeSet<&str> = selection
        .decisions
        .iter()
        .filter(|decision| decision.accept)
        .map(|decision| decision.id.as_str())
        .collect();
    ps::pipe(&review["hubs"]["items"])
        .into_iter()
        .map(|item| (item, accepted.contains(ps::s(&item["id"]).as_str())))
        .collect()
}

/// Publish one accepted hub change: its exact staged calls, under this review's receipt, and only
/// after the persisted body reads back as the reviewed one.
fn publish_hub_item(
    archive: &mut Archive,
    review: &Value,
    plan: &Value,
    item: &Value,
    declined: &BTreeSet<usize>,
    addresses: &mut BTreeMap<usize, String>,
) -> Result<(Value, usize, usize)> {
    let id = ps::s(&item["id"]);
    let calls_idx = item["calls"]
        .as_array()
        .context("a staged hub change names no calls")?
        .iter()
        .filter_map(|call| call.as_u64().map(|n| n as usize))
        .collect::<Vec<_>>();
    if calls_idx.is_empty() {
        bail!("{id} names no staged call");
    }
    let all = plan["calls"].as_array().context("the staged hub plan has no calls")?;
    let first = calls_idx[0];
    let last = *calls_idx.last().unwrap();
    let mut calls = Vec::new();
    for index in &calls_idx {
        let call = all.get(*index).cloned().context("the hub plan stages no such call")?;
        let mut unresolved = None;
        let rewritten = rewrite_refs(&call, &mut |n| {
            if let Some(address) = addresses.get(&n) {
                return Some(address.clone());
            }
            if calls_idx.contains(&n) {
                // A call inside this change refers to an earlier one of its own: local index.
                return Some(format!("@@{}", calls_idx.iter().position(|at| *at == n).unwrap_or(0)));
            }
            if declined.contains(&n) {
                return Some("@@DECLINED".to_string());
            }
            unresolved = Some(n);
            None
        });
        if let Some(n) = unresolved {
            bail!("{id} depends on call {n}, which the plan does not publish");
        }
        if rewritten.to_string().contains("@@DECLINED") {
            bail!("{id} names a Card the selection declines; accept that change or drop this one");
        }
        calls.push(rewritten);
    }
    let prefix = format!("publish/{}/1/hub{last:04}", short_of(review));
    let receipts = calls::invoke_identified(
        archive.zt.client()?,
        &calls,
        &prefix,
        None,
    )?;
    let receipt = receipts.last().context("the hub publisher answered no receipt")?;
    let address = ps::s(&receipt["result"]["address"]);
    if address.is_empty() {
        bail!("{id}: the publication returned no address");
    }
    addresses.insert(last, address.clone());
    if calls_idx.len() > 1 {
        addresses.insert(first, address.clone());
    }
    // The reviewed body still names a Card this plan creates as `@@N`; the publication resolved it,
    // so the exact expected text is the reviewed one with every reference at its published address.
    let expected = rewrite_refs(&json!(ps::s(&item["new_body"])), &mut |n| addresses.get(&n).cloned());
    let expected = normalize(&ps::s(&expected));
    if expected.contains("##UNRESOLVED##") {
        bail!("{id}: a reference in the reviewed body has no published address");
    }
    let card = archive.card(&address)?;
    if ps::s(&card["body"]) != expected {
        bail!(
            "{id}: the persisted body at `{address}` differs from the reviewed body (reviewed {} bytes, persisted {} bytes)",
            expected.len(),
            ps::s(&card["body"]).len()
        );
    }
    let replayed = receipts.iter().filter(|receipt| receipt["replayed"] == json!(true)).count();
    Ok((
        outcome(
            &id,
            true,
            "applied-and-verified",
            "the reviewed body is persisted at its reviewed address",
            json!({"address": address, "fingerprint": calls::authored_fingerprint(&card)}),
        ),
        receipts.len() - replayed,
        replayed,
    ))
}

/// `@@N` inside a staged call: a reviewed address once published, a local index inside one change,
/// or an error the caller reports as a refused dependency.
fn rewrite_refs(value: &Value, address_of: &mut impl FnMut(usize) -> Option<String>) -> Value {
    match value {
        Value::String(text) if text.contains("@@") => {
            let rewritten = RESULT_REF.replace_all(text, |capture: &regex::Captures| {
                let n = capture[1].parse::<usize>().unwrap_or(usize::MAX);
                address_of(n).unwrap_or_else(|| "##UNRESOLVED##".to_string())
            });
            json!(rewritten.into_owned())
        }
        Value::Array(items) => Value::Array(items.iter().map(|item| rewrite_refs(item, address_of)).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), rewrite_refs(v, address_of))).collect()),
        other => other.clone(),
    }
}

/// The hub state after the accepted changes: a published creation records its address, a declined
/// one leaves its old value, and a tree with a declined item keeps its hash so the next plan picks
/// it up again.
fn hub_state_after(work: &str, plan: &Value, addresses: &BTreeMap<usize, String>, hub: &[(&Value, bool)]) -> Result<Value> {
    let accepted_calls = hub
        .iter()
        .filter(|(_, accept)| *accept)
        .flat_map(|(item, _)| item["calls"].as_array().cloned().unwrap_or_default())
        .filter_map(|call| call.as_u64().map(|n| n as usize))
        .collect::<BTreeSet<usize>>();
    if accepted_calls.is_empty() {
        return Ok(json!({"state": "unchanged"}));
    }
    let state_file = ps::join(work, "state.json");
    let old = if ps::exists(&state_file) { canon::read_json(&state_file)? } else { json!({}) };
    let mut planned = plan["state"].clone();
    substitute(&mut planned, addresses);
    let mut merged = merge_state(&old, &planned);
    for (item, accept) in hub {
        if *accept {
            continue;
        }
        let tree = ps::s(&item["tree"]);
        if tree.is_empty() || tree == "field" {
            continue;
        }
        let previous = old["trees"].as_object().and_then(|trees| trees.get(&tree)).cloned().unwrap_or(Value::Null);
        if let Some(entry) = merged["trees"].as_object_mut().and_then(|trees| trees.get_mut(&tree)).and_then(Value::as_object_mut) {
            match previous.get("hash") {
                Some(hash) => {
                    entry.insert("hash".into(), hash.clone());
                }
                None => {
                    entry.remove("hash");
                }
            }
        }
    }
    canon::save_json(&state_file, &merged)?;
    Ok(json!({"state": "written"}))
}

/// Every `@@N` string in the planned state replaced by the address its publication returned.
fn substitute(value: &mut Value, addresses: &BTreeMap<usize, String>) {
    match value {
        Value::String(text) => {
            if let Some(captures) = RESULT_REF.captures(text) {
                let n = captures[1].parse::<usize>().unwrap_or(usize::MAX);
                if let Some(address) = addresses.get(&n) {
                    *text = address.clone();
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| substitute(item, addresses)),
        Value::Object(map) => map.values_mut().for_each(|item| substitute(item, addresses)),
        _ => {}
    }
}

/// The planned state over the old one: a planned value that is still an unpublised `@@N` keeps the
/// old state's value at its place, so a declined creation records nothing.
fn merge_state(old: &Value, planned: &Value) -> Value {
    match planned {
        Value::String(text) if RESULT_REF.is_match(text) => old.clone(),
        Value::Object(map) => {
            // The planned state names the parts this plan touches; every other tree, field, job and
            // counter the hub stage recorded stays as it is (an overlay, never a replacement).
            let mut out = old.as_object().cloned().unwrap_or_default();
            for (key, value) in map {
                let previous = old.get(key).cloned().unwrap_or(Value::Null);
                out.insert(key.clone(), merge_state(&previous, value));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(|item| merge_state(&Value::Null, item)).collect()),
        other => other.clone(),
    }
}

/// Publish the accepted register rows: re-stage the plan's accepted rows against the current
/// register body and publish the parts first and the root last.
fn publish_register(archive: &mut Archive, review: &Value, selection: &Selection, outcomes: &mut Vec<Value>, expected_rest: &str) -> Result<Value> {
    let accepted: BTreeSet<String> = selection
        .decisions
        .iter()
        .filter(|decision| decision.accept && decision.id.starts_with("term:"))
        .map(|decision| decision.id.trim_start_matches("term:").to_string())
        .collect();
    let declined: BTreeSet<String> = selection
        .decisions
        .iter()
        .filter(|decision| !decision.accept && decision.id.starts_with("term:"))
        .map(|decision| decision.id.trim_start_matches("term:").to_string())
        .collect();
    if accepted.is_empty() {
        for term in &declined {
            outcomes.push(outcome(&term_id(term), false, "declined", "the selection does not accept this register row", Value::Null));
        }
        return Ok(json!({"applied": false, "reason": "no accepted register term", "entries": 0}));
    }
    let by_term: BTreeMap<String, Value> = ps::pipe(&review["register"]["terms"])
        .into_iter()
        .map(|row| (ps::s(&row["term"]), row.clone()))
        .collect();
    // A declined row the register already serves cannot stay where it is: the register is a total
    // rebuild, so a partial publication would move or drop it. That is a conflict, not a write.
    let already = declined
        .iter()
        .filter(|term| by_term.get(*term).is_some_and(|row| row["published"].is_object()))
        .cloned()
        .collect::<Vec<_>>();
    if !already.is_empty() {
        for term in accepted.iter().chain(declined.iter()) {
            let status = if accepted.contains(term) { "conflict" } else { "declined" };
            outcomes.push(outcome(
                &term_id(term),
                accepted.contains(term),
                status,
                "declining a row the register already serves would move or drop it; accept every published row or stage a plan without it",
                Value::Null,
            ));
        }
        return Ok(json!({"applied": false, "reason": "a declined row is already published", "entries": 0}));
    }
    // The register's own frame must still be the reviewed one: a rebuild that met a changed frame
    // would publish an unreviewed group set.
    let view = archive.zt.read_current(true)?;
    let address = ps::s(&view["address"]);
    let card = archive.card(&address)?;
    let frame = body_rest(&ps::s(&card["body"]));
    if frame != expected_rest {
        let detail = format!(
            "the register's own frame changed since the review: stage the register plan again ({})",
            frame_diff(expected_rest, &frame)
        );
        for term in &accepted {
            outcomes.push(outcome(&term_id(term), true, "conflict", &detail, Value::Null));
        }
        return Ok(json!({"applied": false, "reason": detail, "entries": 0}));
    }
    // The review binds one exact register plan. A plan re-staged since then is not what the
    // reviewer read, even when the accepted row happens to keep its targets: publishing it would
    // write rows nobody reviewed.
    let plan_path = ps::s(&review["register"]["path"]);
    let plan = match canon::read_json(&plan_path) {
        Ok(plan) => plan,
        Err(error) => {
            let detail = format!("the staged register plan `{plan_path}` cannot be read: {error:#}; run `zt-digest register -Step plan` and `zt-digest publish -Step plan` again");
            return Ok(register_unbound(outcomes, &accepted, &declined, &detail));
        }
    };
    if ps::s(&plan["plan_id"]) != ps::s(&review["register"]["plan_id"]) {
        let detail = format!(
            "the staged register plan `{plan_path}` is no longer the reviewed plan (`{}`): run `zt-digest register -Step plan` and `zt-digest publish -Step plan` again",
            ps::s(&plan["plan_id"])
        );
        return Ok(register_unbound(outcomes, &accepted, &declined, &detail));
    }
    let rows = ps::pipe(&plan["terms"])
        .into_iter()
        .filter(|row| accepted.contains(ps::s(&row["term"]).as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if rows.len() != accepted.len() {
        for term in &accepted {
            if !rows.iter().any(|row| ps::s(&row["term"]) == *term) {
                outcomes.push(outcome(&term_id(term), true, "conflict", "the staged register plan no longer names this row: stage it again", Value::Null));
            }
        }
        return Ok(json!({"applied": false, "reason": "the staged plan no longer names a reviewed row", "entries": 0}));
    }
    let restaged = register::restage(&plan, &rows, &view)?;
    match archive.zt.publish_plan(&restaged, &format!("publish/{}", short_of(review))) {
        Ok(summary) => {
            for term in accepted.iter().chain(declined.iter()) {
                if accepted.contains(term) {
                    outcomes.push(outcome(
                        &term_id(term),
                        true,
                        "applied-and-verified",
                        "the accepted row is published in the verified register",
                        json!({
                            "targets": by_term.get(term).map(|row| ps::pipe(&row["targets"]).into_iter().map(|target| ps::s(&target["address"])).collect::<Vec<_>>()),
                            "register": summary["register"],
                        }),
                    ));
                } else {
                    outcomes.push(outcome(&term_id(term), false, "declined", "the selection does not accept this register row", Value::Null));
                }
            }
            Ok(json!({
                "applied": true,
                "plan_id": restaged["plan_id"],
                "reviewed_plan_id": review["register"]["plan_id"],
                "register": summary["register"],
                "wrote": summary["wrote"],
                "replayed": summary["replayed"],
                "entries": summary["entries"],
                "parts": summary["parts"],
                "complete": summary["complete"],
                "revision": summary["archive_revision"],
            }))
        }
        Err(error) => {
            for term in &accepted {
                outcomes.push(outcome(&term_id(term), true, "pending", &format!("{error:#}"), Value::Null));
            }
            Ok(json!({"applied": false, "reason": format!("{error:#}"), "entries": 0}))
        }
    }
}

/// The register items when the reviewed plan itself cannot be published: every accepted row is a
/// conflict with the reason, and every declined one stays declined. No Card is written (AC02, AC06).
fn register_unbound(outcomes: &mut Vec<Value>, accepted: &BTreeSet<String>, declined: &BTreeSet<String>, detail: &str) -> Value {
    for term in accepted {
        outcomes.push(outcome(&term_id(term), true, "conflict", detail, Value::Null));
    }
    for term in declined {
        outcomes.push(outcome(&term_id(term), false, "declined", "the selection does not accept this register row", Value::Null));
    }
    json!({"applied": false, "reason": detail, "entries": 0})
}

#[allow(clippy::too_many_arguments)]
fn result_record(
    review: &Value,
    selection: &Selection,
    selection_path: &str,
    before: u64,
    after: u64,
    outcomes: &[Value],
    register: &Value,
    hubs: &Value,
    protected: Value,
    requires_revision: bool,
    archive: &mut Archive,
) -> Value {
    let count = |status: &str| outcomes.iter().filter(|outcome| ps::s(&outcome["status"]) == status).count();
    let accepted = outcomes.iter().filter(|outcome| outcome["accepted"] == json!(true)).count();
    let declined = outcomes.iter().filter(|outcome| outcome["accepted"] == json!(false)).count();
    let scope = outcomes
        .iter()
        .filter(|outcome| outcome["accepted"] == json!(true))
        .map(|outcome| ps::s(&outcome["id"]))
        .collect::<Vec<_>>();
    json!({
        "schema_version": SCHEMA_VERSION,
        "contract": CONTRACT,
        "review_id": review["review_id"],
        "selection": {"path": selection_path, "note": selection.note, "readings_reviewed": selection.readings_reviewed, "backup": selection.backup},
        "archive": {"identity": archive.identity, "revision_before": before, "revision_after": after},
        "backup": selection.backup,
        "protected": protected,
        "register": register,
        "hubs": hubs,
        "outcomes": outcomes,
        "summary": {
            "accepted": accepted,
            "declined": declined,
            "applied": count("applied-and-verified"),
            "conflict": count("conflict"),
            "pending": count("pending"),
            "refused": count("refused"),
        },
        "requires_revision": requires_revision,
        "audit": {
            "scope": {"accepted": scope},
            "revision": after,
            "register_complete": register["complete"],
            "entries": register["entries"],
        },
        "created_at": canon::utc_now_o(),
    })
}

// --- check: the post-publication evidence ---------------------------------------------------------

fn check(work: &str, p: &Params) -> Result<i32> {
    let result_path = if p.file.trim().is_empty() {
        ps::join(work, RESULT_FILE)
    } else {
        full_path(&p.file)?
    };
    if !ps::exists(&result_path) {
        bail!("no publication result at `{result_path}`; run -Step apply first");
    }
    let result = canon::read_json(&result_path)?;
    if result["contract"] != json!(CONTRACT) {
        bail!("the result at `{result_path}` is not `{CONTRACT}`");
    }
    let mut archive = Archive::new()?;
    archive.read()?;
    if ps::s(&archive.identity["id"]) != ps::s(&result["archive"]["identity"]["id"]) {
        bail!("the result belongs to another archive");
    }
    let mut applied = Vec::new();
    let mut missing = Vec::new();
    let mut diverged = Vec::new();
    let (view, entries) = archive.zt.read_published()?;
    let published: BTreeMap<String, Vec<String>> = entries.into_iter().collect();
    for outcome in ps::pipe(&result["outcomes"]) {
        if outcome["accepted"] != json!(true) || ps::s(&outcome["status"]) != "applied-and-verified" {
            continue;
        }
        let id = ps::s(&outcome["id"]);
        applied.push(id.clone());
        if let Some(term) = id.strip_prefix("term:") {
            match published.get(term) {
                None => missing.push(json!({"item": id, "term": term})),
                Some(found) => {
                    let planned = outcome["verified"]["targets"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .iter()
                        .map(ps::s)
                        .collect::<Vec<_>>();
                    if *found != planned {
                        diverged.push(json!({"item": id, "planned": planned, "published": found}));
                    }
                }
            }
            continue;
        }
        let address = ps::s(&outcome["verified"]["address"]);
        if address.is_empty() {
            missing.push(json!({"item": id, "detail": "no verified address"}));
            continue;
        }
        if outcome["verified"]["superseded_by_register"] == json!(true) {
            // The accepted register publication rewrote this Card after the hub change; the
            // register's own entries and layout are the evidence that it is persisted.
            continue;
        }
        let card = archive.card(&address)?;
        if card.is_null() {
            missing.push(json!({"item": id, "address": address}));
        } else if ps::s(&outcome["verified"]["fingerprint"]) != calls::authored_fingerprint(&card) {
            diverged.push(json!({"item": id, "address": address, "published": calls::authored_fingerprint(&card)}));
        }
    }
    let protected_now = protected_scope(&mut archive)?;
    let faults = view["faults"].as_array().map_or(0, Vec::len);
    let complete = view["complete"] == json!(true);
    let wrote_terms = applied.iter().any(|id| id.starts_with("term:"));
    let ok = missing.is_empty() && diverged.is_empty() && (!wrote_terms || complete) && result["requires_revision"] == json!(false);
    util::print_last(&json!({
        "ok": ok,
        "step": "check",
        "review_id": result["review_id"],
        "applied": applied.len(),
        "missing": missing,
        "diverged": diverged,
        "complete": complete,
        "faults": faults,
        "entries": published.len(),
        "protected_digest": protected_now["digest"],
        "protected_cards": protected_now["cards"],
        "protected_unchanged": protected_now["digest"] == result["protected"]["after"],
        "revision": archive.revision,
        "published_revision": result["archive"]["revision_after"],
        "changed_since": archive.revision != result["archive"]["revision_after"].as_u64().unwrap_or_default(),
        "path": result_path,
    }))?;
    Ok(if ok { 0 } else { 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review() -> Value {
        json!({
            "schema_version": 1,
            "contract": CONTRACT,
            "review_id": "",
            "created_at": "2026-10-19T00:00:00Z",
            "archive": {"identity": {"id": "a".repeat(32).as_str()}, "revision": 4},
            "items": ["term:甲", "hub:000"],
            "register": {"plan_id": "rp-x", "address": "2/1", "body_rest": "范围：入口。"},
            "hubs": {"plan_id": "hp-x", "items": []},
            "protected": {"cards": 0, "addresses": [{"address": "1/1", "fingerprint": "f"}]},
        })
    }

    #[test]
    fn a_review_id_binds_its_content() {
        let mut value = review();
        value["review_id"] = json!(content_id(&value));
        assert!(is_review_id(&ps::s(&value["review_id"])));
        assert_eq!(content_id(&value), ps::s(&value["review_id"]));
        value["register"]["body_rest"] = json!("范围：别的入口。");
        assert_ne!(content_id(&value), ps::s(&value["review_id"]));
    }

    #[test]
    fn a_review_id_that_is_not_xp_hex_is_refused() {
        assert!(!is_review_id("r-0000000000000000000000000000000000000000000000000000000000000000"));
        assert!(!is_review_id("xp-123"));
        assert!(is_review_id(&format!("xp-{}", "0".repeat(64))));
    }

    #[test]
    fn links_are_addressed_and_a_placeholder_is_not_a_target() {
        assert_eq!(links_in("- [[1/2]] 与 [[@@0]] 和 [[3/4|a]]"), vec!["1/2", "3/4"]);
        assert!(links_in("范围：没有链接。").is_empty());
    }

    #[test]
    fn the_registers_own_frame_keeps_every_group_it_does_not_own() {
        let body = "范围：入口。\n\n## 领域\n- 学习：[[2/1|a]]\n\n## 关键词\n- 学习：[[0/1|a]]\n\n## 词条\n- 学习：[[0/1]]\n\n## 词条分册\n- [[2/1|b]] A–F";
        assert_eq!(body_rest(body), "范围：入口。\n\n## 领域\n- 学习：[[2/1|a]]");
    }

    #[test]
    fn a_frame_difference_names_the_first_line() {
        let diff = frame_diff("范围：入口。\n\n## 领域\n- 甲", "范围：入口。\n\n## 领域\n- 乙");
        assert_eq!(diff, "line 4: expected `- 甲`, read `- 乙`");
        assert_eq!(frame_diff("a", "a"), "the frames differ only in line endings");
    }

    #[test]
    fn a_staged_reference_is_resolved_from_the_published_address() {
        let call = json!({"tool": "edit_card", "arguments": {"at": "@@0", "body": "- [[@@0]]（枢纽）：甲。", "base_fingerprint": "f"}});
        let mut map = BTreeMap::new();
        map.insert(0usize, "3/2|a".to_string());
        let resolved = rewrite_refs(&call, &mut |n| map.get(&n).cloned());
        assert_eq!(ps::s(&resolved["arguments"]["at"]), "3/2|a");
        assert_eq!(ps::s(&resolved["arguments"]["body"]), "- [[3/2|a]]（枢纽）：甲。");
        let unresolved = rewrite_refs(&call, &mut |_| None);
        assert!(ps::s(&unresolved["arguments"]["body"]).contains("##UNRESOLVED##"));
    }

    #[test]
    fn staged_bodies_are_shown_with_the_cards_line_endings() {
        assert_eq!(normalize("甲\r\n乙\r\n"), "甲\n乙\n");
        assert_eq!(normalize("甲\n乙"), "甲\n乙");
    }

    #[test]
    fn a_declined_creation_keeps_the_old_state_value_at_its_place() {
        let old = json!({"trees": {"3": {"topic": "3/1", "lines": {"3/2": "3/2|a"}, "hash": "old"}}, "fields": {}});
        let mut planned = json!({"trees": {"3": {"topic": "3/1", "lines": {"3/2": "3/2|b", "3/4": "@@0"}, "hash": "new"}}, "fields": {}});
        let mut map = BTreeMap::new();
        map.insert(0usize, "3/4|a".to_string());
        substitute(&mut planned, &map);
        let merged = merge_state(&old, &planned);
        assert_eq!(ps::s(&merged["trees"]["3"]["lines"]["3/2"]), "3/2|b");
        assert_eq!(ps::s(&merged["trees"]["3"]["lines"]["3/4"]), "3/4|a");
        // An unpublished placeholder keeps what the old state held there, never the token itself.
        let mut unpublished = planned.clone();
        unpublished["trees"]["3"]["lines"]["3/5"] = json!("@@7");
        let merged = merge_state(&old, &unpublished);
        assert_eq!(merged["trees"]["3"]["lines"]["3/5"], Value::Null);
    }

    #[test]
    fn a_term_row_is_one_term_before_one_to_three_links() {
        assert_eq!(register_line("学习", &["0/1".to_string()]), "- 学习：[[0/1]]");
        assert_eq!(
            register_line("学习", &["0/1".to_string(), "pub2020/2".to_string()]),
            "- 学习：[[0/1]]、[[pub2020/2]]"
        );
    }

    #[test]
    fn the_pinyin_bands_carry_their_terms_and_readings() {
        let parts = vec![json!({"label": "A–F", "bucket": "latin", "address": "2/1|a", "terms": ["多元均衡"]})];
        let accepted = json!([{"term": "多元均衡", "reading": "duō yuán", "initial": "d"}]);
        let bands = band_evidence(&parts, &accepted);
        assert_eq!(ps::s(&bands[0]["label"]), "A–F");
        assert_eq!(ps::s(&bands[0]["terms"][0]["reading"]), "duō yuán");
        assert_eq!(ps::s(&bands[0]["terms"][0]["initial"]), "d");
    }

    /// A real, verifiable backup of a stand-in archive: the gate opens it and compares its
    /// archive id and Card count before it accepts any change.
    fn backup_record() -> (tempfile::TempDir, Value) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.sqlite3");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO metadata VALUES ('archive_id', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa');
             CREATE TABLE cards (address TEXT PRIMARY KEY);
             INSERT INTO cards VALUES ('1/1');",
        )
        .unwrap();
        drop(conn);
        let bytes = std::fs::metadata(&path).unwrap().len();
        let sha = file_sha256(&path.to_string_lossy()).unwrap();
        (
            dir,
            json!({"path": path.to_string_lossy(), "sha256": sha, "bytes": bytes}),
        )
    }

    fn selection(review_id: &str, accept: &[&str], decline: &[&str], note: &str) -> Value {
        json!({
            "schema_version": 1,
            "review_id": review_id,
            "archive_id": "a".repeat(32),
            "note": note,
            "readings_reviewed": true,
            "accept": accept,
            "decline": decline,
        })
    }

    #[test]
    fn silence_is_never_permission() {
        let mut value = review();
        value["review_id"] = json!(content_id(&value));
        let reviewed = ps::s(&value["review_id"]);
        let (_dir, backup) = backup_record();
        let mut asked = selection(&reviewed, &["term:甲"], &[], "accept one row");
        asked["backup"] = backup;
        let decided = validate_selection(&value, &asked, "selection.json").unwrap();
        let hub = decided.decisions.iter().find(|decision| decision.id == "hub:000").unwrap();
        assert!(!hub.accept);
        assert_eq!(hub.note, "not-selected");
        assert!(decided.decisions.iter().any(|decision| decision.id == "term:甲" && decision.accept));
    }

    #[test]
    fn a_selection_is_bound_to_its_review_archive_note_and_items() {
        let mut value = review();
        value["review_id"] = json!(content_id(&value));
        let reviewed = ps::s(&value["review_id"]);
        let wrong_review = validate_selection(&value, &selection("xp-00", &[], &[], "note"), "s").err().map(|error| error.to_string()).unwrap_or_default();
        assert!(wrong_review.contains("review"), "{wrong_review}");
        let unknown = validate_selection(&value, &selection(&reviewed, &["term:乙"], &[], "note"), "s").err().map(|error| error.to_string()).unwrap_or_default();
        assert!(unknown.contains("does not name"), "{unknown}");
        let twice = selection(&reviewed, &["term:甲"], &["term:甲"], "note");
        assert!(validate_selection(&value, &twice, "s").is_err());
        let silent = selection(&reviewed, &[], &[], "  ");
        assert!(validate_selection(&value, &silent, "s").err().map(|error| error.to_string()).unwrap_or_default().contains("note"));
        let mut other_archive = selection(&reviewed, &[], &[], "note");
        other_archive["archive_id"] = json!("b".repeat(32));
        assert!(validate_selection(&value, &other_archive, "s").err().map(|error| error.to_string()).unwrap_or_default().contains("archive"));
    }

    #[test]
    fn a_term_needs_the_pinyin_evidence_and_a_verified_backup() {
        let mut value = review();
        value["review_id"] = json!(content_id(&value));
        let reviewed = ps::s(&value["review_id"]);
        let mut asked = selection(&reviewed, &["term:甲"], &[], "accept one row");
        asked["readings_reviewed"] = json!(false);
        assert!(validate_selection(&value, &asked, "s").err().map(|error| error.to_string()).unwrap_or_default().contains("pinyin"));
        asked["readings_reviewed"] = json!(true);
        assert!(validate_selection(&value, &asked, "s").err().map(|error| error.to_string()).unwrap_or_default().contains("backup"));
        // A hub-only selection needs no reading review; it still needs a backup.
        let mut hub_only = selection(&reviewed, &["hub:000"], &[], "accept one hub change");
        hub_only["readings_reviewed"] = json!(false);
        assert!(validate_selection(&value, &hub_only, "s").err().map(|error| error.to_string()).unwrap_or_default().contains("backup"));
    }

    /// The hub stage's state covers the whole Workspace. Publishing one plan must keep every tree,
    /// field and counter the plan does not name, or the next stage forgets them (S27-AC05).
    #[test]
    fn a_published_plan_keeps_the_hub_state_it_does_not_name() {
        let old = json!({
            "trees": {"3": {"topic": "3/1", "hash": "h3"}, "4": {"topic": "4/1", "hash": "h4"}},
            "fields": {"甲": "3/1|a"},
            "register": "2/1",
            "seq": 7,
            "jobs": {"job-1": {"kind": "topic"}},
        });
        let planned = json!({"trees": {"3": {"topic": "3/1", "hash": "new"}}});
        let merged = merge_state(&old, &planned);
        assert_eq!(merged["trees"]["3"]["hash"], json!("new"));
        assert_eq!(merged["trees"]["4"]["hash"], json!("h4"), "a tree the plan does not name stays: {merged}");
        assert_eq!(merged["fields"]["甲"], json!("3/1|a"));
        assert_eq!(merged["register"], json!("2/1"));
        assert_eq!(merged["seq"], json!(7));
        assert_eq!(merged["jobs"]["job-1"]["kind"], json!("topic"));
    }
}
