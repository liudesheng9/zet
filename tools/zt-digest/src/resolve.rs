//! `zt-digest resolve`: the separately selected historical resolution batch
//! (S29, issue #122; design §15-phase-6, D377/D378).
//!
//! ```text
//! zt-digest resolve -Step plan  -Work <folder> [-File <batch-selection.json>]
//! zt-digest resolve -Step apply -Work <folder>
//! zt-digest resolve -Step check -Work <folder> [-File <resolution-batch-result.json>]
//! ```
//!
//! There is no archive-wide historical migration phase (D377). The reports already produced for an
//! archive — `topic-home`'s transfer review and `join`'s Topic-pair review — name the old
//! restatements, misfiled Cards and Topic pairs a reviewer may act on. This gate turns the exact
//! items a person selected out of those reports into one batch, executes only those items through
//! the already qualified paths ([`crate::topic_home::run_selected`],
//! [`crate::topic_join::run_selected`]), and verifies the batch afterwards.
//!
//! `plan` (S29-AC01, S29-AC02) writes `<Work>\resolution-batch.json` with an `rb-` content id, its
//! Markdown view and a retained body per read Card under `<Work>\resolve-endpoints\`. It changes no
//! Card. Every offered report item becomes one candidate, bound to its report id, finding id, item
//! or plan id, the exact texts or operations the qualified path will write, the current address and
//! authored fingerprint of every Card it touches, the archive identity and revision it read, and the
//! explicit treatment of an earlier carrier. An item the report itself did not offer (a blocked
//! Model Card, a refused plan) is listed as `ineligible` with its reason and can never be selected.
//! The batch is `selected` only when a selection file accepts items out of it; with no selection, or
//! with a selection that accepts nothing, the batch is `deferred` and every candidate keeps its
//! disposition (`awaiting-selection`, `not-selected` or `ineligible`), so an unselected or declined
//! batch stays visibly undone. A selected batch also binds the verified backup it was approved
//! against.
//!
//! `apply` (S29-AC03–S29-AC06) reads that batch, refuses a deferred one, verifies the archive
//! identity and the backup again, and re-checks the protected scope (every human Card and every
//! agent Model Card) before a single write. It then executes the accepted items one at a time
//! through the qualified paths: the nonmoving retained-entry transfers first, because they change
//! no address, and the reviewed `move_card` joins after them, so no selected move invalidates the
//! address a later item reads and every mapping a move returns is current for the items after it.
//! Each item keeps the qualified path's own evidence and ends `applied-and-verified`, `unchanged`,
//! `conflict`, `refused` or `pending`; the batch ends `complete` only when every item is verified or
//! explicitly unresolved, its mappings are current and no protected human body or finished model
//! changed, and its record says plainly that no other historical fault was rewritten.
//!
//! `check` (S29-AC05) reads the archive again and re-verifies what the batch claims: the applied
//! addresses, the statement fingerprints, the entry Links, the protected scope and the Broken-link
//! count, with the per-item status the result recorded. It never repairs anything.

use crate::args::Args;
use crate::canon;
use crate::calls;
use crate::organize::common::{self, Sb};
use crate::ps;
use crate::publish::{self, Archive};
use crate::topic_home;
use crate::topic_join;
use crate::util;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The batch's own contract.
pub const CONTRACT: &str = "resolve/1";
/// The immutable batch, written by `-Step plan`.
pub const BATCH_FILE: &str = "resolution-batch.json";
/// The batch's Markdown view.
pub const BATCH_MD: &str = "resolution-batch.md";
/// The selection a person writes to accept items out of the reports.
pub const SELECTION_FILE: &str = "resolution-batch-selection.json";
/// The batch's execution evidence, written by `-Step apply`.
pub const RESULT_FILE: &str = "resolution-batch-result.json";
/// The result's Markdown view.
pub const RESULT_MD: &str = "resolution-batch-result.md";
/// The retained bodies of the Cards a batch read.
const ENDPOINTS: &str = "resolve-endpoints";
const SCHEMA_VERSION: i64 = 1;
/// A batch id: `rb-` and the SHA-256 hex of its content.
const BATCH_PREFIX: &str = "rb-";
/// One batch is a concrete selection a person reviewed, never a migration: the most items it holds.
pub const MAX_ITEMS: usize = 100;
/// The transfer path's own entry-scan bound, passed through so a batch transfer reads what a
/// planned transfer reads.
const MAX_CARRIERS: usize = topic_home::MAX_CARRIERS as usize;
/// §4.5's accepted restatement entry heading: the entry a transfer writes is navigation.
const ENTRY_HEADING: &str = "同述入口";
/// The five per-item statuses the qualified paths report (D274).
pub const APPLIED: &str = "applied-and-verified";
pub const UNCHANGED: &str = "unchanged";
pub const CONFLICT: &str = "conflict";
pub const REFUSED: &str = "refused";
pub const PENDING: &str = "pending";

fn f(value: &Value, key: &str) -> String {
    ps::s(&value[key])
}

fn refuse(message: impl Into<String>) -> anyhow::Error {
    canon::refuse(message.into())
}

// --- the command ---------------------------------------------------------------------------------

struct Params {
    step: String,
    work: String,
    file: String,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Step", "Work", "File"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &[])?;
    let p = Params {
        step: a.choice("Step", &["plan", "apply", "check"])?,
        work: common::mandatory(&a, "Work")?,
        file: a.str("File"),
    };
    match p.step.as_str() {
        "plan" => plan(&p),
        "apply" => apply(&p),
        _ => check(&p),
    }
}

// --- the batch identity --------------------------------------------------------------------------

fn is_batch_id(value: &str) -> bool {
    value.len() == BATCH_PREFIX.len() + 64
        && value.starts_with(BATCH_PREFIX)
        && value[BATCH_PREFIX.len()..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// `rb-` and the SHA-256 of the batch's content without its own id and creation time: an edited
/// batch is a different batch, never trusted.
fn batch_id(value: &Value) -> String {
    format!("{BATCH_PREFIX}{}", canon::content_hash(value, &["batch_id", "created_at"]))
}

// --- the candidate items -------------------------------------------------------------------------

/// One address the batch's item touches, with what the report reviewed and what the archive holds
/// now. `role` is one of `entry`, `anchor`, `statement`, `moving`, `destination`, `joining`,
/// `keeper` or `other`.
#[derive(Clone, Debug)]
struct Addr {
    role: &'static str,
    address: String,
    /// The authored fingerprint the report reviewed, when it recorded one.
    reviewed: String,
    /// An address the item must create or occupy, so the archive is expected not to hold it.
    expected_free: bool,
}

/// One reported item the batch may select, before its disposition is decided.
#[derive(Clone, Debug)]
struct Candidate {
    kind: &'static str,
    report_id: String,
    finding: String,
    /// The `topic-home` item id (`t001`) or "" for a join finding.
    item: String,
    /// The join plan id (`t001/p001`) or "" for a nonmoving join resolution.
    plan: String,
    /// The join action (`keep_distinct`, `keep_both_with_crossing`, `move_subtree`) or "".
    action: String,
    /// Why the report did not offer this item; `Some` makes it `ineligible`.
    ineligible: Option<Value>,
    /// The exact texts or operations the qualified path will write (§11.3).
    operation: Value,
    /// The explicit treatment of an earlier carrier of the same statement (D272).
    earlier_carrier: Value,
    addresses: Vec<Addr>,
}

impl Candidate {
    /// The batch-internal id of this candidate's item: the one `disposition` names it by.
    fn key(&self) -> String {
        if self.kind == "transfer" {
            format!("{}/{}", self.finding, self.item)
        } else if self.plan.is_empty() {
            format!("{}/{}", self.finding, self.action)
        } else {
            format!("{}/{}", self.finding, self.plan)
        }
    }
}

/// The eligible candidates of one `topic-home` review, plus the items it did not offer.
fn topic_home_candidates(review: &Value) -> Vec<Candidate> {
    let report_id = f(review, "report_id");
    let mut out = Vec::new();
    for item in ps::pipe(&review["items"]) {
        let entry = f(item, "entry");
        let status = f(item, "status");
        let offered = status == "offered";
        let mut addresses: Vec<Addr> = vec![Addr {
            role: "entry",
            address: entry.clone(),
            reviewed: reviewed_fingerprint(&review["endpoints"], &entry),
            expected_free: false,
        }];
        let target = &item["target"];
        let mut earlier_carrier = json!({ "treatment": "none", "detail": "no earlier carrier was reviewed" });
        if offered {
            let anchor = f(target, "anchor");
            let statement = f(target, "statement_address");
            let slot = f(target, "to");
            addresses.push(Addr {
                role: "anchor",
                address: anchor.clone(),
                reviewed: f(target, "anchor_fingerprint"),
                expected_free: false,
            });
            let reuse = f(&item["carrier_scan"], "treatment") == "reuse";
            addresses.push(Addr {
                role: "statement",
                address: statement.clone(),
                reviewed: if reuse { reviewed_fingerprint(&review["endpoints"], &statement) } else { String::new() },
                expected_free: !reuse,
            });
            if !reuse && statement != slot {
                addresses.push(Addr {
                    role: "destination",
                    address: slot,
                    reviewed: String::new(),
                    expected_free: true,
                });
            }
            earlier_carrier = if reuse {
                json!({
                    "treatment": "reuse",
                    "address": statement,
                    "statement_fingerprint": f(&item["statement"], "statement_fingerprint"),
                    "detail": "the target Literature tree already carries this exact statement; the transfer reuses it and creates no second copy",
                })
            } else {
                json!({
                    "treatment": "create",
                    "address": statement,
                    "detail": "no Card of the target Literature tree carries this statement, so the transfer creates it beside the reviewed anchor",
                })
            };
        }
        out.push(Candidate {
            kind: "transfer",
            report_id: report_id.clone(),
            finding: f(item, "finding"),
            item: f(item, "id"),
            plan: String::new(),
            action: String::new(),
            ineligible: offered.then_some(()).is_none().then(|| {
                json!({
                    "code": f(&item["reason"], "code"),
                    "detail": f(&item["reason"], "detail"),
                    "status": status,
                })
            }),
            operation: json!({
                "book": f(item, "book"),
                "topic": f(item, "topic"),
                "entry": entry,
                "zone": f(item, "zone"),
                "statement": {
                    "address": f(target, "statement_address"),
                    "anchor": f(target, "anchor"),
                    "slot": f(target, "to"),
                    "title": f(&item["statement"], "title"),
                    "body": f(&item["statement"], "body"),
                    "statement_fingerprint": f(&item["statement"], "statement_fingerprint"),
                    "sources": item["statement"]["sources"],
                    "formulas": item["statement"]["formulas"],
                },
                "models": item["target"]["model_context"],
                "entry_remedy": {
                    "title": f(&item["remedies"]["entry"], "title"),
                    "body": f(&item["remedies"]["entry"], "body"),
                },
                "distinct_requires": item["remedies"]["distinct"]["requires"],
            }),
            earlier_carrier,
            addresses,
        });
    }
    out
}

/// The authored fingerprint a report reviewed for one address, from its retained endpoints.
fn reviewed_fingerprint(endpoints: &Value, address: &str) -> String {
    endpoints
        .as_object()
        .and_then(|map| map.get(address))
        .map(|row| f(row, "fingerprint"))
        .unwrap_or_default()
}

/// The authored fingerprint of each moved Card, from one move operation's own pairs.
fn move_fingerprints(operation: &Value) -> BTreeMap<String, String> {
    ps::ps_items(&operation["fingerprints"])
        .into_iter()
        .filter_map(|pair| {
            let list = pair.as_array()?;
            Some((ps::s(list.first()?), ps::s(list.get(1)?)))
        })
        .collect()
}

/// The eligible candidates of one `join` review: the nonmoving resolutions a reviewer may record,
/// and one candidate per offered move plan. A plan the report refused is listed, not selectable.
fn join_candidates(review: &Value) -> Vec<Candidate> {
    let report_id = f(review, "report_id");
    let mut out = Vec::new();
    for pair in ps::pipe(&review["pairs"]) {
        let finding = f(pair, "id");
        let keeper = f(pair, "keeper");
        let other = {
            let a = f(&pair["a"], "root");
            if keeper == a { f(&pair["b"], "root") } else { a }
        };
        let joining = f(&pair["joining"], "address");
        for alternative in ps::pipe(&pair["alternatives"]) {
            let action = f(alternative, "action");
            if action == "move_subtree" {
                for plan in ps::pipe(&alternative["plans"]) {
                    let status = f(plan, "status");
                    let ineligible = (status != "offered").then(|| {
                        json!({
                            "code": f(&plan["reason"], "code"),
                            "detail": f(&plan["reason"], "detail"),
                            "status": status,
                        })
                    });
                    let mut addresses: Vec<Addr> = Vec::new();
                    for operation in ps::pipe(&plan["operations"]) {
                        let reviewed = move_fingerprints(operation);
                        let from = f(operation, "from");
                        let mut seen: BTreeSet<String> = addresses
                            .iter()
                            .filter(|row| row.role == "moving")
                            .map(|row| row.address.clone())
                            .collect();
                        let mut moved = vec![from.clone()];
                        for mapped in ps::ps_items(&operation["mapping"]) {
                            if let Some(list) = mapped.as_array() {
                                moved.push(list.first().map(ps::s).unwrap_or_default());
                            }
                        }
                        for at in moved {
                            if at.is_empty() || !seen.insert(at.clone()) {
                                continue;
                            }
                            addresses.push(Addr {
                                role: "moving",
                                address: at.clone(),
                                reviewed: reviewed.get(&at).cloned().unwrap_or_default(),
                                expected_free: false,
                            });
                        }
                        // The landing slot must still be free. The other addresses a mapping names
                        // are the renumbered destinations of Cards that move in the same call, so
                        // they may hold a Card today: only the operation's own destination is a
                        // reason to call the reviewed plan stale.
                        let landing = f(operation, "to");
                        if !landing.is_empty() && !addresses.iter().any(|row| row.role == "landing" && row.address == landing) {
                            addresses.push(Addr {
                                role: "landing",
                                address: landing,
                                reviewed: String::new(),
                                expected_free: true,
                            });
                        }
                    }
                    if !joining.is_empty() {
                        addresses.push(Addr {
                            role: "joining",
                            address: joining.clone(),
                            reviewed: reviewed_fingerprint(&review["endpoints"], &joining),
                            expected_free: false,
                        });
                    }
                    out.push(Candidate {
                        kind: "join",
                        report_id: report_id.clone(),
                        finding: finding.clone(),
                        item: String::new(),
                        plan: f(plan, "id"),
                        action: action.clone(),
                        ineligible,
                        operation: json!({
                            "kind": f(plan, "kind"),
                            "description": f(plan, "description"),
                            "keeper": keeper,
                            "joining": joining,
                            "operations": plan["operations"],
                        }),
                        earlier_carrier: json!({ "treatment": "none", "detail": "a Topic move creates and rewrites no statement" }),
                        addresses,
                    });
                }
                continue;
            }
            // `keep_distinct` and `keep_both_with_crossing` write no Card: the resolution is the
            // recorded decision, and the report's own alternative is its evidence.
            let mut addresses = vec![Addr {
                role: "keeper",
                address: keeper.clone(),
                reviewed: reviewed_fingerprint(&review["endpoints"], &keeper),
                expected_free: false,
            }];
            if !other.is_empty() {
                addresses.push(Addr {
                    role: "other",
                    address: other.clone(),
                    reviewed: reviewed_fingerprint(&review["endpoints"], &other),
                    expected_free: false,
                });
            }
            out.push(Candidate {
                kind: "join",
                report_id: report_id.clone(),
                finding: finding.clone(),
                item: String::new(),
                plan: String::new(),
                action: action.clone(),
                ineligible: None,
                operation: json!({
                    "keeper": keeper,
                    "joining": joining,
                    "description": f(alternative, "description"),
                    "reason": f(alternative, "reason"),
                }),
                earlier_carrier: json!({ "treatment": "none", "detail": "a nonmoving resolution creates and rewrites no Card" }),
                addresses,
            });
        }
    }
    out
}

// --- the dispositions ----------------------------------------------------------------------------

/// One reported item with the disposition the selection gave it. `current` holds the Card each
/// reviewed address holds now, or `Value::Null` when it holds none.
#[derive(Debug)]
struct Item {
    id: String,
    candidate: Candidate,
    disposition: &'static str,
    reason: Value,
    remedy: String,
    note: String,
    draft: Value,
    current: BTreeMap<String, Value>,
}

impl Item {
    fn new(id: String, candidate: Candidate) -> Self {
        let (disposition, reason) = match &candidate.ineligible {
            Some(reason) => ("ineligible", reason.clone()),
            None => ("deferred", json!({ "code": "awaiting-selection", "detail": "no batch selection named this item" })),
        };
        Self {
            id,
            candidate,
            disposition,
            reason,
            remedy: String::new(),
            note: String::new(),
            draft: Value::Null,
            current: BTreeMap::new(),
        }
    }

    fn defer(&mut self, code: &str, detail: impl Into<String>) {
        if self.candidate.ineligible.is_some() {
            return;
        }
        self.disposition = "deferred";
        self.reason = json!({ "code": code, "detail": detail.into() });
    }

    fn select(&mut self, remedy: &str, note: &str, draft: Value) {
        self.disposition = "selected";
        self.reason = Value::Null;
        self.remedy = remedy.to_string();
        self.note = note.to_string();
        self.draft = draft;
    }

    fn selected(&self) -> bool {
        self.disposition == "selected"
    }

    /// The entry address a transfer item rewrites, or "" for a join item.
    fn entry(&self) -> String {
        f(&self.candidate.operation, "entry")
    }

    /// Whether the archive still holds what the report reviewed at every address of this item.
    fn stale(&self) -> Vec<Value> {
        let mut out = Vec::new();
        for address in &self.candidate.addresses {
            let now = self.current.get(&address.address);
            let exists = now.is_some_and(|card| !card.is_null());
            let current = now.map(now_fingerprint).unwrap_or_default();
            if address.expected_free && exists {
                out.push(json!({
                    "address": address.address,
                    "role": address.role,
                    "found": current,
                    "why": "the reviewed destination is no longer free",
                }));
            } else if !address.expected_free && !address.reviewed.is_empty() && exists && current != address.reviewed {
                out.push(json!({
                    "address": address.address,
                    "role": address.role,
                    "reviewed": address.reviewed,
                    "found": current,
                    "why": "the Card no longer carries the reviewed authored text",
                }));
            }
        }
        out
    }

    fn json(&self) -> Value {
        let mut addresses = Vec::new();
        for address in &self.candidate.addresses {
            let now = self.current.get(&address.address).filter(|card| !card.is_null());
            let mut current = String::new();
            let mut exists = false;
            let mut title = String::new();
            let mut kind = String::new();
            if let Some(card) = now {
                exists = true;
                current = now_fingerprint(card);
                title = f(card, "title");
                kind = f(card, "type");
            }
            addresses.push(json!({
                "role": address.role,
                "address": address.address,
                "expected_free": address.expected_free,
                "reviewed_fingerprint": address.reviewed.clone(),
                "current_fingerprint": current,
                "exists": exists,
                "title": title,
                "type": kind,
            }));
        }
        let stale = self.stale();
        json!({
            "id": self.id,
            "kind": self.candidate.kind,
            "report_id": self.candidate.report_id,
            "finding": self.candidate.finding,
            "item": self.candidate.item,
            "plan": self.candidate.plan,
            "action": self.candidate.action,
            "disposition": self.disposition,
            "reason": self.reason,
            "remedy": if self.remedy.is_empty() { Value::Null } else { json!(self.remedy) },
            "note": self.note,
            "draft": self.draft,
            "operation": self.candidate.operation,
            "earlier_carrier": self.candidate.earlier_carrier,
            "addresses": addresses,
            "stale": stale,
        })
    }
}

/// The authored fingerprint of one read Card, or "" when it cannot be read.
fn now_fingerprint(card: &Value) -> String {
    if card.is_null() { String::new() } else { calls::authored_fingerprint(card) }
}

/// The two produced reports a batch may select from.
struct Reports<'a> {
    topic_home: Option<&'a Value>,
    join: Option<&'a Value>,
}

/// S29-AC02: the disposition of every reported item, decided only by an explicit selection. Silence
/// is never permission: an item the selection does not accept stays `deferred`, and an item the
/// report itself did not offer stays `ineligible`.
fn settle(candidates: Vec<Candidate>, selection: Option<&Value>, reports: &Reports) -> Result<Vec<Item>> {
    let mut items: Vec<Item> = candidates
        .into_iter()
        .enumerate()
        .map(|(index, candidate)| Item::new(format!("c{:03}", index + 1), candidate))
        .collect();
    if items.len() > MAX_ITEMS {
        return Err(refuse(format!(
            "the reports offer {} items, above the {MAX_ITEMS} a batch may hold; plan a narrower run or select from fewer pairs",
            items.len()
        )));
    }
    let Some(asked) = selection else {
        for item in &mut items {
            item.defer(
                "awaiting-selection",
                "no batch selection was supplied, so no historical resolution is authorized; this gate stays deferred",
            );
        }
        return Ok(items);
    };
    let object = asked.as_object().ok_or_else(|| refuse("the batch selection must be an object"))?;
    for name in object.keys() {
        if !["schema_version", "note", "backup", "topic_home", "join"].contains(&name.as_str()) {
            return Err(refuse(format!("the batch selection has an unknown field `{name}`")));
        }
    }
    if asked["schema_version"].as_i64() != Some(SCHEMA_VERSION) {
        return Err(refuse("the batch selection needs schema_version 1"));
    }
    let note = f(asked, "note");
    if publish::blank(&note) {
        return Err(refuse("the batch selection needs a nonempty note: why this batch is accepted"));
    }
    let home = asked.get("topic_home").filter(|value| !value.is_null());
    let joined = asked.get("join").filter(|value| !value.is_null());
    if home.is_none() && joined.is_none() {
        for item in &mut items {
            item.defer(
                "declined",
                "the batch selection accepts no item, so the batch is declined and stays deferred",
            );
        }
        return Ok(items);
    }
    // The per-report blocks are the qualified steps' own selection shapes, so their own validators
    // decide them: a batch can never accept something `topic-home` or `join` would refuse.
    if let Some(block) = home {
        let review = reports.topic_home.ok_or_else(|| {
            refuse("the batch selection accepts topic-home items, but this work folder holds no topic-home review")
        })?;
        for decision in topic_home::validate_selection(review, block)? {
            let Some(item) = items.iter_mut().find(|item| {
                item.candidate.kind == "transfer"
                    && item.candidate.finding == decision.finding
                    && item.entry() == decision.entry
            }) else {
                return Err(refuse(format!(
                    "the batch selection accepts the transfer of `{}` in finding `{}`, which this batch does not hold",
                    decision.entry, decision.finding
                )));
            };
            if item.candidate.ineligible.is_some() {
                return Err(refuse(format!(
                    "the batch selection accepts the ineligible item `{}`",
                    item.candidate.key()
                )));
            }
            let draft = match decision.draft {
                Some((title, body)) => json!({ "title": title, "body": body }),
                None => Value::Null,
            };
            item.select(&decision.remedy, &decision.note, draft);
        }
    }
    if let Some(block) = joined {
        let review = reports
            .join
            .ok_or_else(|| refuse("the batch selection accepts join items, but this work folder holds no join review"))?;
        for decision in topic_join::validate_selection(review, block)? {
            let plan = decision.plan.clone().unwrap_or_default();
            let Some(item) = items.iter_mut().find(|item| {
                item.candidate.kind == "join" && item.candidate.finding == decision.finding && item.candidate.plan == plan
            }) else {
                return Err(refuse(format!(
                    "the batch selection accepts `{}` of finding `{}`, which this batch does not hold",
                    if plan.is_empty() { decision.action.clone() } else { plan.clone() },
                    decision.finding
                )));
            };
            if item.candidate.ineligible.is_some() {
                return Err(refuse(format!(
                    "the batch selection accepts the ineligible join plan `{}`",
                    item.candidate.key()
                )));
            }
            item.select(&decision.action, &decision.note, Value::Null);
        }
    }
    // Everything the selection passed over stays recorded as passed over.
    for item in &mut items {
        if item.disposition == "deferred" && item.candidate.ineligible.is_none() {
            item.defer("not-selected", "the batch selection did not accept this item; silence is never permission");
        }
    }
    Ok(items)
}

/// The current state of every reviewed address, read in one coherent revision.
fn fill_current(items: &mut [Item], cards: &BTreeMap<String, Value>) {
    for item in items.iter_mut() {
        for address in &item.candidate.addresses {
            let row = cards.get(&address.address).cloned().unwrap_or(Value::Null);
            item.current.insert(address.address.clone(), row);
        }
    }
}

// --- the batch document --------------------------------------------------------------------------

/// S29-AC01: the approved backup and rollback evidence a selected batch is bound to. A batch that
/// selects nothing needs none; a batch that accepts a change is refused without one, and the file
/// is verified here: this archive's own SQLite backup, holding Cards.
fn backup_evidence(selection: Option<&Value>, selected: usize, archive_id: &str) -> Result<Value> {
    if selected == 0 {
        return Ok(Value::Null);
    }
    let asked = selection.context("a selected batch needs the verified backup it was approved against")?;
    let backup = &asked["backup"];
    if !backup.is_object() {
        return Err(refuse(
            "the batch selection accepts changes without a verified backup (`backup`: path, sha256, bytes)",
        ));
    }
    let path = f(backup, "path");
    if publish::blank(&path) || !ps::exists(&path) {
        return Err(refuse(format!("the approved backup `{path}` does not exist")));
    }
    let bytes = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or_default();
    if let Some(recorded) = backup["bytes"].as_u64()
        && recorded != bytes
    {
        return Err(refuse(format!("the approved backup is {bytes} bytes, not the recorded {recorded}")));
    }
    let sha = publish::file_sha256(&path)?;
    if f(backup, "sha256") != sha {
        return Err(refuse(format!(
            "the approved backup's SHA-256 is {sha}, not the recorded `{}`",
            f(backup, "sha256")
        )));
    }
    publish::verify_backup_of(&path, archive_id)?;
    Ok(json!({
        "path": path,
        "sha256": sha,
        "bytes": bytes,
        "archive_id": archive_id,
        "verified": true,
        "detail": "a readable SQLite backup of this archive that holds Cards; it is the rollback evidence of this batch, not an automatic undo",
    }))
}

/// The batch document: the selected items, the reports they came from, the archive it binds, the
/// approved backup, the protected scope and the visible disposition of everything it did not select.
fn batch_document(
    archive: &Value,
    reports: &[Value],
    items: Vec<Item>,
    selection: Option<(&str, &Value)>,
    backup: Value,
    protected: Value,
) -> Value {
    let count = |disposition: &str| items.iter().filter(|item| item.disposition == disposition).count();
    let selected = count("selected");
    let rows: Vec<Value> = items.iter().map(Item::json).collect();
    let stale = rows.iter().filter(|row| row["stale"].as_array().is_some_and(|list| !list.is_empty())).count();
    let mut codes: BTreeSet<String> = BTreeSet::new();
    for row in &rows {
        if row["disposition"] == "deferred" {
            codes.insert(f(&row["reason"], "code"));
        }
    }
    let deferred = if selected > 0 {
        Value::Null
    } else if codes.contains("awaiting-selection") || selection.is_none() {
        json!({
            "code": "no-selection",
            "detail": "no batch selection accepted an item, so nothing is authorized; this optional historical gate stays deferred",
        })
    } else {
        json!({
            "code": "declined",
            "detail": "the batch selection accepted no item, so the batch is declined; every item keeps its visible disposition",
        })
    };
    json!({
        "schema_version": SCHEMA_VERSION,
        "contract": CONTRACT,
        "kind": "resolution-batch",
        "created_at": canon::utc_now_o(),
        "archive": archive.clone(),
        "reports": reports,
        "selection": match selection {
            Some((path, asked)) => json!({
                "present": true,
                "path": path,
                "note": f(asked, "note"),
                "topic_home": asked.get("topic_home").filter(|value| !value.is_null()).cloned().unwrap_or(Value::Null),
                "join": asked.get("join").filter(|value| !value.is_null()).cloned().unwrap_or(Value::Null),
            }),
            None => json!({ "present": false, "path": Value::Null, "note": Value::Null, "topic_home": Value::Null, "join": Value::Null }),
        },
        "backup": backup,
        "protected": protected,
        "items": rows,
        "counts": {
            "candidates": items.len(),
            "selected": selected,
            "deferred": count("deferred"),
            "ineligible": count("ineligible"),
            "stale": stale,
        },
        "status": if selected > 0 { "selected" } else { "deferred" },
        "deferred": deferred,
        "scope": "one explicitly selected batch of already produced reports; there is no automatic archive-wide migration, and nothing outside this batch is rewritten or claimed fixed",
        "bounds": {
            "items": MAX_ITEMS,
            "transfer_carriers": MAX_CARRIERS,
        },
    })
}

fn render_batch(batch: &Value) -> String {
    let mut sb = Sb::new();
    sb.line("# 历史决议批次复核").blank();
    sb.line(format!("Batch: `{}`", f(batch, "batch_id")));
    sb.line(format!("Contract: `{}`", f(batch, "contract")));
    sb.line(format!("Generated: {}", f(batch, "created_at")));
    sb.line(format!(
        "Archive: `{}` at revision {}.",
        f(&batch["archive"], "id"),
        f(&batch["archive"], "revision")
    ));
    sb.line(format!(
        "Status: **{}**; {} selected, {} deferred, {} ineligible.",
        f(batch, "status"),
        f(&batch["counts"], "selected"),
        f(&batch["counts"], "deferred"),
        f(&batch["counts"], "ineligible")
    ));
    sb.blank();
    if !batch["deferred"].is_null() {
        sb.line(format!(
            "> Deferred: `{}` — {}",
            f(&batch["deferred"], "code"),
            f(&batch["deferred"], "detail")
        ))
        .blank();
    }
    sb.line("## 报告").blank();
    for report in ps::pipe(&batch["reports"]) {
        sb.line(format!(
            "- `{}` ({}) — {}",
            f(report, "report_id"),
            f(report, "kind"),
            f(report, "path")
        ));
    }
    sb.blank();
    sb.line("## 项目").blank();
    for item in ps::pipe(&batch["items"]) {
        sb.line(format!(
            "- **{}** `{}` {} — {}",
            f(item, "id"),
            f(item, "kind"),
            f(item, "disposition"),
            f(&item["operation"], "entry")
        ));
        if !item["reason"].is_null() {
            sb.line(format!("  - 理由：`{}` — {}", f(&item["reason"], "code"), f(&item["reason"], "detail")));
        }
        for address in ps::pipe(&item["addresses"]) {
            sb.line(format!(
                "  - `{}` {} 现在 {}",
                f(address, "address"),
                f(address, "role"),
                if address["exists"].as_bool() == Some(true) { "存在" } else { "不存在" }
            ));
        }
    }
    sb.blank();
    sb.0
}

// --- the command steps ---------------------------------------------------------------------------

/// The work folder as an absolute path, so every recorded path names the same folder.
fn full_path(path: &str) -> Result<String> {
    let path = std::path::Path::new(path);
    let full = std::path::absolute(path).with_context(|| format!("resolve {}", path.display()))?;
    Ok(full.to_string_lossy().to_string())
}

/// The report paths the qualified steps write under one work folder.
const TOPIC_HOME_REVIEW: &str = "topic-home-review.json";
const JOIN_REVIEW: &str = "topic-join-review.json";

/// One produced report of this work folder, verified against its own content id.
fn report_row(kind: &str, review: &Value, path: &str, offered: usize, ineligible: usize, total: i64) -> Value {
    json!({
        "kind": kind,
        "report_id": f(review, "report_id"),
        "path": path,
        "created_at": f(review, "created_at"),
        "status": f(review, "status"),
        "archive_id": f(&review["archive"], "id"),
        "revision": review["archive"]["revision"],
        "reported": total,
        "offered": offered,
        "ineligible": ineligible,
    })
}

/// S29-AC01: retain the body of every Card the batch read, under the batch's own folder, so the
/// evidence a person reviewed is not a later archive.
fn retain_bodies(work: &str, cards: &BTreeMap<String, Value>) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    std::fs::create_dir_all(ps::join(work, ENDPOINTS))?;
    for (address, card) in cards {
        if card.is_null() {
            continue;
        }
        let name = canon::sha256_hex(address.as_bytes());
        let path = ps::join(&ps::join(work, ENDPOINTS), &format!("{}.md", &name[..16]));
        let title: String = f(card, "title").chars().take(200).collect();
        let body: String = f(card, "body").chars().take(20_000).collect();
        util::write_atomic(&path, &format!("# `{address}` {title}\n\n{body}\n"))?;
        rows.push(json!({ "address": address, "path": path, "fingerprint": now_fingerprint(card) }));
    }
    Ok(rows)
}

fn plan(p: &Params) -> Result<i32> {
    let work = full_path(&p.work)?;
    let mut archive = Archive::new()?;
    archive.read()?;
    let archive_row = json!({
        "id": f(&archive.identity, "id"),
        "path": f(&archive.identity, "path"),
        "revision": archive.revision,
    });
    // The reports already produced for this archive; only these may be resolved.
    let home_path = ps::join(&work, TOPIC_HOME_REVIEW);
    let join_path = ps::join(&work, JOIN_REVIEW);
    let home = if ps::exists(&home_path) { Some(topic_home::read_review(&work)?) } else { None };
    let joined = if ps::exists(&join_path) { Some(topic_join::read_review(&work)?) } else { None };
    for review in [home.as_ref(), joined.as_ref()].into_iter().flatten() {
        let report_archive = f(&review["archive"], "id");
        if report_archive != f(&archive_row, "id") {
            return Err(refuse(format!(
                "the report `{}` belongs to archive `{report_archive}`, but this service serves `{}`; regenerate the report on this archive",
                f(review, "report_id"),
                f(&archive_row, "id")
            )));
        }
    }
    let mut candidates = Vec::new();
    if let Some(review) = &home {
        candidates.extend(topic_home_candidates(review));
    }
    if let Some(review) = &joined {
        candidates.extend(join_candidates(review));
    }
    let selection_path = if p.file.trim().is_empty() { ps::join(&work, SELECTION_FILE) } else { full_path(&p.file)? };
    let asked = if ps::exists(&selection_path) { Some(canon::read_json(&selection_path)?) } else { None };
    let reports = Reports { topic_home: home.as_ref(), join: joined.as_ref() };
    let mut items = settle(candidates, asked.as_ref(), &reports)?;
    // The current address and authored fingerprint of every Card an item touches (S29-AC01). A
    // reviewed destination the archive does not hold yet is read as absent, not as a fault.
    let wanted: Vec<String> = items
        .iter()
        .flat_map(|item| item.candidate.addresses.iter().map(|row| row.address.clone()))
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();
    let cards = archive.excerpts(&wanted)?;
    fill_current(&mut items, &cards);
    let retained = retain_bodies(&work, &cards)?;
    let protected = publish::protected_scope(&mut archive)?;
    let mut report_rows = Vec::new();
    if let Some(review) = &home {
        let (offered, ineligible) = candidates_of(&items, "transfer");
        let reported = ps::pipe(&review["items"]).len() as i64;
        report_rows.push(report_row("topic-home", review, &home_path, offered, ineligible, reported));
    }
    if let Some(review) = &joined {
        let (offered, ineligible) = candidates_of(&items, "join");
        let reported = ps::pipe(&review["pairs"]).len() as i64;
        report_rows.push(report_row("join", review, &join_path, offered, ineligible, reported));
    }
    let selected = items.iter().filter(|item| item.selected()).count();
    let backup = backup_evidence(asked.as_ref(), selected, &f(&archive_row, "id"))?;
    let selection = asked.as_ref().map(|value| (selection_path.as_str(), value));
    let mut batch = batch_document(&archive_row, &report_rows, items, selection, backup, protected);
    batch["endpoints"] = json!(retained);
    batch["batch_id"] = json!(batch_id(&batch));
    canon::save_json(&ps::join(&work, BATCH_FILE), &batch)?;
    let md_path = ps::join(&work, BATCH_MD);
    util::write_atomic(&md_path, &render_batch(&batch))?;
    util::print_last(&json!({
        "ok": true,
        "step": "plan",
        "batch_id": batch["batch_id"],
        "status": batch["status"],
        "deferred": batch["deferred"]["code"],
        "archive_id": batch["archive"]["id"],
        "archive_revision": batch["archive"]["revision"],
        "reports": batch["reports"],
        "counts": batch["counts"],
        "backup": if batch["backup"].is_null() { Value::Null } else { json!(f(&batch["backup"], "sha256")) },
        "paths": [ps::join(&work, BATCH_FILE), md_path],
    }))?;
    Ok(0)
}

/// How many of this batch's items of one kind the reports offered and did not offer.
fn candidates_of(items: &[Item], kind: &str) -> (usize, usize) {
    let rows: Vec<&Item> = items.iter().filter(|item| item.candidate.kind == kind).collect();
    let ineligible = rows.iter().filter(|item| item.candidate.ineligible.is_some()).count();
    (rows.len() - ineligible, ineligible)
}

/// The reviewed batch of this work folder, verified against its own content id: an edited batch is
/// a different batch and is never applied.
fn read_batch(work: &str) -> Result<Value> {
    let path = ps::join(work, BATCH_FILE);
    if !ps::exists(&path) {
        return Err(refuse(format!("no batch at `{path}`; run -Step plan first")));
    }
    let batch = canon::read_json(&path)?;
    if f(&batch, "contract") != CONTRACT {
        return Err(refuse(format!("the batch at `{path}` is not `{CONTRACT}`")));
    }
    let claimed = f(&batch, "batch_id");
    if !is_batch_id(&claimed) || batch_id(&batch) != claimed {
        return Err(refuse(format!(
            "the batch at `{path}` no longer matches its own batch id; run -Step plan again"
        )));
    }
    Ok(batch)
}

fn apply(p: &Params) -> Result<i32> {
    let work = full_path(&p.work)?;
    let batch = read_batch(&work)?;
    apply_batch(&work, batch)
}

fn check(p: &Params) -> Result<i32> {
    let work = full_path(&p.work)?;
    let batch = read_batch(&work)?;
    check_batch(&work, batch, &p.file)
}

// --- apply: the selected batch --------------------------------------------------------------------

/// One item's outcome: the qualified path's own evidence, plus the status the batch reports.
fn outcome(item: &Value, status: &str, reason: Value, evidence: Value) -> Value {
    json!({
        "id": f(item, "id"),
        "kind": f(item, "kind"),
        "report_id": f(item, "report_id"),
        "finding": f(item, "finding"),
        "item": f(item, "item"),
        "plan": f(item, "plan"),
        "action": f(item, "action"),
        "remedy": item["remedy"],
        "note": f(item, "note"),
        "status": status,
        "reason": reason,
        "evidence": evidence,
    })
}

/// The worst status of a list, in the order the batch reports it.
fn worst(statuses: &[String]) -> &'static str {
    for candidate in [CONFLICT, PENDING, REFUSED, APPLIED, UNCHANGED] {
        if statuses.iter().any(|status| status == candidate) {
            return candidate;
        }
    }
    UNCHANGED
}

fn apply_batch(work: &str, batch: Value) -> Result<i32> {
    let batch_id_value = f(&batch, "batch_id");
    let archive_id = f(&batch["archive"], "id");
    let selected: Vec<Value> = ps::pipe(&batch["items"])
        .into_iter()
        .filter(|row| f(row, "disposition") == "selected")
        .cloned()
        .collect();
    if selected.is_empty() {
        // S29-AC02: an unselected or declined batch authorizes nothing, and says so.
        return Err(refuse(format!(
            "the batch `{batch_id_value}` selected no item ({}); the optional historical gate stays deferred and no Card is written",
            f(&batch["deferred"], "code")
        )));
    }
    // S29-AC01: the approved backup is re-verified before any write, and it must be the same bytes.
    let backup = &batch["backup"];
    let backup_path = f(backup, "path");
    if !ps::exists(&backup_path) {
        return Err(refuse(format!("the approved backup `{backup_path}` is gone; a batch is not applied without its rollback evidence")));
    }
    let sha = publish::file_sha256(&backup_path)?;
    if sha != f(backup, "sha256") {
        return Err(refuse(format!(
            "the approved backup `{backup_path}` is no longer the recorded bytes (SHA-256 {sha}, recorded `{}`)",
            f(backup, "sha256")
        )));
    }
    publish::verify_backup_of(&backup_path, &archive_id)?;
    let mut archive = Archive::new()?;
    archive.read()?;
    if f(&archive.identity, "id") != archive_id {
        return Err(refuse(format!(
            "the batch belongs to archive `{archive_id}`, but this service serves `{}`",
            f(&archive.identity, "id")
        )));
    }
    let before = archive.revision;
    let protected_before = publish::protected_scope(&mut archive)?;
    let changed = publish::protected_diff(&batch, &protected_before);
    let broken_before = broken_links(&mut archive)?;
    if !changed.is_empty() {
        // AC06: a protected human body or finished model moved since the review, so nothing is
        // written and the batch waits for a quiet window.
        let outcomes: Vec<Value> = selected
            .iter()
            .map(|item| {
                outcome(
                    item,
                    PENDING,
                    json!({
                        "code": "quiet_window_required",
                        "detail": "the protected human/Model scope changed since the batch was selected, so no write is sent",
                    }),
                    Value::Null,
                )
            })
            .collect();
        return write_result(
            work,
            &batch,
            &archive,
            before,
            before,
            &protected_before,
            &changed,
            broken_before,
            broken_before,
            outcomes,
            Value::Null,
            Value::Null,
        );
    }
    let protected_before_digest = protected_before["digest"].clone();
    // The receipt prefix is the batch's own id, so a retry of the same batch replays the same
    // receipts: `run/unit/positive-plan-revision/step` as the archive's identified writes require.
    let prefix = format!("resolve/{}/1/transfer", &batch_id_value[BATCH_PREFIX.len()..BATCH_PREFIX.len() + 16]);
    // S29-AC03: the nonmoving retained-entry transfers first — they change no address — and the
    // reviewed moves after them, so no selected move invalidates an address a later item reads.
    let mut outcomes: Vec<Value> = Vec::new();
    let mut rollback = json!({ "transfers": [], "moves": [] });
    let mut applied_mappings: Vec<Value> = Vec::new();
    if selected.iter().any(|item| f(item, "kind") == "transfer") {
        let review = topic_home::read_review(work)?;
        let block = batch["selection"]["topic_home"].clone();
        if block.is_null() {
            return Err(refuse("the batch holds a selected transfer but no topic-home selection; plan the batch again"));
        }
        let decisions = topic_home::validate_selection(&review, &block)?;
        let payload = topic_home::run_selected(work, review, decisions, &prefix, MAX_CARRIERS)?;
        rollback["transfers"] = payload["rollback"].clone();
        for row in ps::pipe(&payload["items"]) {
            let Some(item) = selected.iter().find(|item| f(item, "kind") == "transfer" && f(item, "item") == f(row, "id")) else {
                continue;
            };
            outcomes.push(outcome(item, &f(row, "status"), row["reason"].clone(), row.clone()));
        }
    }
    if selected.iter().any(|item| f(item, "kind") == "join") {
        let review = topic_join::read_review(work)?;
        let block = batch["selection"]["join"].clone();
        if block.is_null() {
            return Err(refuse("the batch holds a selected join but no join selection; plan the batch again"));
        }
        let decisions = topic_join::validate_selection(&review, &block)?;
        let payload = topic_join::run_selected(review, decisions)?;
        rollback["moves"] = payload["rollback"].clone();
        for row in ps::pipe(&payload["items"]) {
            let Some(item) = selected.iter().find(|item| f(item, "kind") == "join" && f(item, "finding") == f(row, "finding")) else {
                continue;
            };
            outcomes.push(outcome(item, &f(row, "status"), row["reason"].clone(), row.clone()));
        }
        for pair in ps::ps_items(&payload["applied"]) {
            applied_mappings.push(pair);
        }
    }
    // An item the qualified path did not answer is a batch fault, never a silent success.
    for item in &selected {
        if outcomes.iter().any(|row| f(row, "id") == f(item, "id")) {
            continue;
        }
        outcomes.push(outcome(
            item,
            PENDING,
            json!({ "code": "no_outcome", "detail": "the qualified path returned no outcome for this item" }),
            Value::Null,
        ));
    }
    outcomes.sort_by(|a, b| canon::ordinal_cmp(&f(a, "id"), &f(b, "id")));
    let mut archive_after = Archive::new()?;
    archive_after.read()?;
    let protected_after = publish::protected_scope(&mut archive_after)?;
    let changed_after = publish::protected_diff(&batch, &protected_after);
    let broken_after = broken_links(&mut archive_after)?;
    write_result(
        work,
        &batch,
        &archive_after,
        before,
        archive_after.revision,
        &protected_after,
        &changed_after,
        broken_before,
        broken_after,
        outcomes,
        json!({ "digest": protected_before_digest }),
        json!({ "rollback": rollback, "applied": applied_mappings, "prefix": prefix }),
    )
}

#[allow(clippy::too_many_arguments)]
fn write_result(
    work: &str,
    batch: &Value,
    archive_after: &Archive,
    revision_before: u64,
    revision_after: u64,
    protected_after: &Value,
    protected_changed: &[Value],
    broken_before: usize,
    broken_after: usize,
    outcomes: Vec<Value>,
    protected_before: Value,
    execution: Value,
) -> Result<i32> {
    let statuses: Vec<String> = outcomes.iter().map(|row| f(row, "status")).collect();
    let count = |status: &str| statuses.iter().filter(|known| known.as_str() == status).count();
    let status = worst(&statuses);
    let protected_unchanged = protected_changed.is_empty();
    // D378: every item is verified or explicitly unresolved, and no protected human body or
    // finished model changed. `complete` is the stronger claim: nothing is left unresolved.
    let explicit = outcomes.iter().all(|row| {
        let status = f(row, "status");
        status == APPLIED || status == UNCHANGED || (!status.is_empty() && !row["reason"].is_null())
    });
    let complete = protected_unchanged && outcomes.iter().all(|row| {
        let status = f(row, "status");
        status == APPLIED || status == UNCHANGED
    });
    let unresolved: Vec<Value> = outcomes
        .iter()
        .filter(|row| {
            let status = f(row, "status");
            status != APPLIED && status != UNCHANGED
        })
        .map(|row| json!({ "id": f(row, "id"), "status": f(row, "status"), "reason": row["reason"] }))
        .collect();
    let before_digest = if protected_before.is_null() {
        protected_after["digest"].clone()
    } else {
        protected_before["digest"].clone()
    };
    let result_id = format!(
        "rr-{}",
        canon::content_hash(
            &json!({
                "batch_id": batch["batch_id"],
                "archive": archive_after.identity,
                "revision_before": revision_before,
                "revision_after": revision_after,
                "items": outcomes,
                "protected": protected_after["digest"],
                "broken": broken_after,
            }),
            &[],
        )
    );
    let mut result = json!({
        "schema_version": SCHEMA_VERSION,
        "contract": CONTRACT,
        "kind": "resolution-batch-result",
        "created_at": canon::utc_now_o(),
        "result_id": result_id,
        "batch_id": batch["batch_id"],
        "batch_created_at": batch["created_at"],
        "batch_revision": batch["archive"]["revision"],
        "selection": batch["selection"]["path"],
        "archive": {
            "id": f(&archive_after.identity, "id"),
            "path": f(&archive_after.identity, "path"),
            "revision_before": revision_before,
            "revision_after": revision_after,
        },
        "backup": batch["backup"],
        "protected": {
            "reviewed": batch["protected"]["digest"],
            "before": before_digest,
            "after": protected_after["digest"],
            "cards": protected_after["cards"],
            "unchanged": protected_unchanged,
            "changed": protected_changed,
        },
        "items": outcomes,
        "counts": {
            "items": outcomes.len(),
            "applied_and_verified": count(APPLIED),
            "unchanged": count(UNCHANGED),
            "conflict": count(CONFLICT),
            "refused": count(REFUSED),
            "pending": count(PENDING),
        },
        "status": status,
        "complete": complete,
        "done": protected_unchanged && explicit,
        "unresolved": unresolved,
        "broken_links": { "before": broken_before, "after": broken_after },
        "scope": "one explicitly selected batch of already produced reports; no other historical fault is rewritten, repaired or claimed fixed",
    });
    for (key, value) in execution.as_object().into_iter().flatten() {
        result[key] = value.clone();
    }
    let path = ps::join(work, RESULT_FILE);
    canon::save_json(&path, &result)?;
    let md_path = ps::join(work, RESULT_MD);
    util::write_atomic(&md_path, &render_result(&result))?;
    util::print_last(&json!({
        "ok": true,
        "step": "apply",
        "batch_id": result["batch_id"],
        "result_id": result["result_id"],
        "status": result["status"],
        "complete": result["complete"],
        "done": result["done"],
        "counts": result["counts"],
        "protected": { "unchanged": result["protected"]["unchanged"] },
        "broken_links": result["broken_links"],
        "unresolved": result["unresolved"],
        "paths": [path, md_path],
    }))?;
    Ok(0)
}

/// The archive's Broken-link count for the agent Workspace: the same seam both qualified paths use.
fn broken_links(archive: &mut Archive) -> Result<usize> {
    let value = archive.call("list_broken_links", json!({ "workspace": "agent" }))?;
    Ok(ps::ps_items(&value["links"]).len())
}

// --- check: the current readback ----------------------------------------------------------------

/// The addresses one applied item must still hold, with what must be there: the statement's
/// fingerprint, or the authored text of a moved Card.
fn verified_state(batch_item: &Value, evidence: &Value) -> (Vec<Value>, Option<String>, Vec<String>) {
    let mut checks: Vec<Value> = Vec::new();
    let mut statement = None;
    let mut models: Vec<String> = Vec::new();
    if f(batch_item, "kind") == "transfer" {
        let address = f(&batch_item["operation"]["statement"], "address");
        let fingerprint = f(&batch_item["operation"]["statement"], "statement_fingerprint");
        models = ps::ps_items(&batch_item["operation"]["models"])
            .into_iter()
            .map(|row| f(&row, "address"))
            .collect();
        if !address.is_empty() {
            checks.push(json!({ "address": address, "role": "statement", "fingerprint": fingerprint.clone() }));
            statement = Some(fingerprint);
        }
        let entry = f(&batch_item["operation"], "entry");
        if !entry.is_empty() {
            checks.push(json!({ "address": entry, "role": "entry", "links": address }));
        }
        // A statement the item reused has no created Card, so its address is the one read above.
        if let Some(created) = evidence.get("created").filter(|row| !row.is_null()) {
            let at = f(created, "address");
            let after = f(created, "after_fingerprint");
            if !at.is_empty() && !checks.iter().any(|row| f(row, "address") == at) {
                checks.push(json!({ "address": at, "role": "statement", "fingerprint": after }));
            }
            if !after.is_empty() {
                statement = Some(after);
            }
        }
        return (checks, statement, models);
    }
    for operation in ps::ps_items(&batch_item["operation"]["operations"]) {
        for pair in ps::ps_items(&operation["mapping"]) {
            let Some(list) = pair.as_array() else { continue };
            let before = list.first().map(ps::s).unwrap_or_default();
            let after = list.get(1).map(ps::s).unwrap_or_default();
            checks.push(json!({ "address": after, "role": "moved", "reviewed": before, "exists": true }));
        }
    }
    (checks, None, models)
}

fn check_batch(work: &str, batch: Value, file: &str) -> Result<i32> {
    let batch_id_value = f(&batch, "batch_id");
    if f(&batch, "status") != "selected" {
        // S29-AC02: the gate is deferred, and this is what says so. Nothing was applied.
        util::print_last(&json!({
            "ok": true,
            "step": "check",
            "batch_id": batch_id_value,
            "status": "deferred",
            "deferred": batch["deferred"]["code"],
            "complete": false,
            "done": false,
            "claim": "no historical batch was selected, so no old Card was corrected; the optional gate is deferred, and this check asserts nothing about the archive's other historical faults",
            "path": ps::join(work, BATCH_FILE),
        }))?;
        return Ok(0);
    }
    let result_path = if file.trim().is_empty() { ps::join(work, RESULT_FILE) } else { full_path(file)? };
    if !ps::exists(&result_path) {
        return Err(refuse(format!("no batch result at `{result_path}`; run -Step apply first")));
    }
    let result = canon::read_json(&result_path)?;
    if f(&result, "batch_id") != batch_id_value {
        return Err(refuse(format!(
            "the result at `{result_path}` belongs to batch `{}`, not `{batch_id_value}`",
            f(&result, "batch_id")
        )));
    }
    let mut archive = Archive::new()?;
    archive.read()?;
    let protected_now = publish::protected_scope(&mut archive)?;
    let changed = publish::protected_diff(&batch, &protected_now);
    let broken_now = broken_links(&mut archive)?;
    let mut rows: Vec<Value> = Vec::new();
    let mut faults: Vec<String> = Vec::new();
    for outcome in ps::pipe(&result["items"]) {
        let id = f(outcome, "id");
        let status = f(outcome, "status");
        let Some(batch_item) = ps::pipe(&batch["items"]).into_iter().find(|item| f(item, "id") == id) else {
            faults.push(format!("`{id}` is in the result but not in the batch"));
            continue;
        };
        let mut row = json!({ "id": id, "kind": f(outcome, "kind"), "status": status, "ok": true, "now": [] });
        if status != APPLIED && status != UNCHANGED {
            // An unresolved item is reported as it was recorded; check repairs nothing.
            rows.push(row);
            continue;
        }
        let (checks, statement, models) =
            verified_state(batch_item, if outcome["evidence"].is_null() { &Value::Null } else { &outcome["evidence"] });
        let addresses: Vec<String> = checks.iter().map(|row| f(row, "address")).collect();
        let cards = archive.excerpts(&addresses)?;
        let mut now: Vec<Value> = Vec::new();
        for check in &checks {
            let at = f(check, "address");
            let card = cards.get(&at).cloned().unwrap_or(Value::Null);
            let exists = !card.is_null();
            let fingerprint = now_fingerprint(&card);
            let mut ok = exists;
            if exists && f(check, "role") == "moved" {
                // The moved Card must still carry the authored text the reviewed `from` held.
                let reviewed = f(check, "reviewed");
                if let Some(want) = reviewed_fingerprint_in(&batch_item["addresses"], &reviewed) {
                    ok = fingerprint == want;
                }
            }
            if exists && f(check, "role") == "statement" && !f(check, "fingerprint").is_empty() {
                ok = f(&card, "statement_fingerprint") == f(check, "fingerprint");
            }
            if exists && f(check, "role") == "entry" {
                let body = f(&card, "body");
                let target = f(check, "links");
                ok = body.contains(ENTRY_HEADING) && (target.is_empty() || body.contains(&format!("[[{target}]]")));
            }
            if !ok {
                faults.push(format!("`{at}` ({}) no longer holds what the batch verified", f(check, "role")));
            }
            now.push(json!({
                "address": at,
                "role": f(check, "role"),
                "exists": exists,
                "fingerprint": fingerprint,
                "ok": ok,
            }));
        }
        let mut model_rows: Vec<Value> = Vec::new();
        for model in &models {
            match archive.call("get_model", json!({ "at": model })) {
                Ok(report) => model_rows.push(json!({ "address": model, "ok": true, "parts": ps::each(&report["parts"]).len() })),
                Err(error) => {
                    faults.push(format!("the model report of `{model}` cannot be read: {error:#}"));
                    model_rows.push(json!({ "address": model, "ok": false, "detail": format!("{error:#}") }));
                }
            }
        }
        row["now"] = json!(now);
        row["statement"] = json!(statement);
        row["models"] = json!(model_rows);
        row["ok"] = json!(now.iter().all(|entry| entry["ok"].as_bool() == Some(true))
            && model_rows.iter().all(|entry| entry["ok"].as_bool() == Some(true)));
        rows.push(row);
    }
    let result_broken = result["broken_links"]["after"].as_u64().unwrap_or_default() as usize;
    if broken_now != result_broken {
        faults.push(format!(
            "the archive now holds {broken_now} Broken links, not the {result_broken} the batch recorded"
        ));
    }
    if !changed.is_empty() {
        faults.push("a protected human or Model Card changed since the batch was selected".to_string());
    }
    let ok = faults.is_empty() && rows.iter().all(|row| row["ok"].as_bool() == Some(true));
    util::print_last(&json!({
        "ok": ok,
        "step": "check",
        "batch_id": batch_id_value,
        "result_id": result["result_id"],
        "status": result["status"],
        "complete": result["complete"],
        "done": result["done"],
        "items": rows,
        "faults": faults,
        "protected": {
            "reviewed": batch["protected"]["digest"],
            "now": protected_now["digest"],
            "unchanged": changed.is_empty(),
            "changed": changed,
        },
        "broken_links": { "recorded": result_broken, "now": broken_now },
        "archive": {
            "id": f(&archive.identity, "id"),
            "revision_recorded": result["archive"]["revision_after"],
            "revision_now": archive.revision,
        },
        "claim": "the applied items of this one selected batch are checked against the archive as it stands; no other historical fault is inspected, repaired or claimed fixed",
        "path": result_path,
    }))?;
    Ok(if ok { 0 } else { 1 })
}

/// The reviewed fingerprint of a moving address, from the batch item's own address rows.
fn reviewed_fingerprint_in(addresses: &Value, reviewed: &str) -> Option<String> {
    ps::ps_items(addresses)
        .into_iter()
        .find(|row| f(row, "address") == reviewed && f(row, "role") == "moving")
        .map(|row| f(&row, "reviewed_fingerprint"))
        .filter(|value| !value.is_empty())
}

fn render_result(result: &Value) -> String {
    let mut sb = Sb::new();
    sb.line("# 历史决议批次结果").blank();
    sb.line(format!("Batch: `{}`", f(result, "batch_id")));
    sb.line(format!("Result: `{}`", f(result, "result_id")));
    sb.line(format!("Generated: {}", f(result, "created_at")));
    sb.line(format!(
        "Archive: `{}` at revision {} → {}.",
        f(&result["archive"], "id"),
        f(&result["archive"], "revision_before"),
        f(&result["archive"], "revision_after")
    ));
    sb.line(format!(
        "Status: **{}**; complete: {}; done: {}.",
        f(result, "status"),
        ps::truthy(&result["complete"]),
        ps::truthy(&result["done"])
    ));
    sb.blank();
    sb.line("## 项目").blank();
    for item in ps::pipe(&result["items"]) {
        sb.line(format!("- **{}** `{}` — {}", f(item, "id"), f(item, "kind"), f(item, "status")));
        if !item["reason"].is_null() {
            sb.line(format!("  - 理由：`{}` — {}", f(&item["reason"], "code"), f(&item["reason"], "detail")));
        }
    }
    sb.blank();
    sb.line("## 保护范围").blank();
    sb.line(format!(
        "- 复核时 {} 张卡，摘要 `{}`",
        f(&result["protected"], "reviewed"),
        f(&result["protected"], "reviewed")
    ));
    sb.line(format!("- 现在摘要 `{}`，未变：{}", f(&result["protected"], "after"), ps::truthy(&result["protected"]["unchanged"])));
    sb.blank();
    sb.line(format!(
        "> {}",
        f(result, "scope")
    ))
    .blank();
    sb.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_id_binds_its_own_content() {
        let batch = json!({ "contract": CONTRACT, "batch_id": "", "created_at": "t", "items": [] });
        let id = batch_id(&batch);
        assert!(is_batch_id(&id), "{id}");
        let mut edited = batch.clone();
        edited["items"] = json!([{ "id": "c001" }]);
        assert_ne!(batch_id(&edited), id);
    }

    #[test]
    fn a_blocked_transfer_item_is_listed_but_never_selectable() {
        let review = json!({
            "report_id": "th-abc",
            "items": [
                { "id": "t001", "finding": "c001", "entry": "12/1", "status": "offered",
                  "statement": { "title": "T", "body": "B", "statement_fingerprint": "fp", "sources": ["book/1"], "formulas": [] },
                  "target": { "anchor": "book/1", "anchor_fingerprint": "afp", "to": "book/1|a", "statement_address": "book/1|a", "model_context": [] },
                  "carrier_scan": { "treatment": "create" },
                  "earlier_carrier": null,
                  "remedies": { "entry": { "title": "T", "body": "## 同述入口\n\n- [[book/1|a]]\n\n来源：[[book]]" }, "distinct": { "requires": ["…"] } },
                  "book": "book" },
                { "id": "t002", "finding": "c001", "entry": "12/2", "status": "blocked",
                  "reason": { "code": "model_boundary", "detail": "a Model Card needs a separate model decision" },
                  "statement": { "title": "M", "body": "MB", "statement_fingerprint": "mfp", "sources": [], "formulas": [] },
                  "target": null, "carrier_scan": null, "earlier_carrier": null, "remedies": null, "book": "book" }
            ],
            "endpoints": { "12/1": { "fingerprint": "e1" }, "book/1": { "fingerprint": "a1" } }
        });
        let candidates = topic_home_candidates(&review);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].key(), "c001/t001");
        assert!(candidates[0].ineligible.is_none());
        assert_eq!(candidates[0].operation["statement"]["statement_fingerprint"], "fp");
        assert_eq!(candidates[0].earlier_carrier["treatment"], "create");
        // The entry, the anchor and the statement slot with the reviewed fingerprints.
        let roles: Vec<&str> = candidates[0].addresses.iter().map(|a| a.role).collect();
        assert_eq!(roles, vec!["entry", "anchor", "statement"]);
        assert_eq!(candidates[0].addresses[0].reviewed, "e1");
        assert!(candidates[0].addresses[2].expected_free);
        // The blocked Model Card is listed with its own reason, and cannot be selected.
        assert_eq!(candidates[1].key(), "c001/t002");
        assert_eq!(candidates[1].ineligible.as_ref().unwrap()["code"], "model_boundary");
    }

    /// One join review: two Topics, the nonmoving alternatives and one offered move beside one the
    /// report refused.
    fn join_review() -> Value {
        json!({
            "report_id": "tj-abc",
            "pairs": [{
                "id": "t001",
                "keeper": "108/0",
                "joining": { "address": "108/1" },
                "a": { "root": "108/0" }, "b": { "root": "78/0" },
                "alternatives": [
                    { "action": "keep_distinct", "description": "分开放", "reason": "问题不同" },
                    { "action": "keep_both_with_crossing", "description": "保留并相连", "reason": "各有价值" },
                    { "action": "move_subtree", "joining": "108/1", "other": "78/0", "reason": "", "plans": [
                        { "id": "t001/p001", "kind": "whole_chain", "description": "整条入口链", "status": "offered", "reason": null,
                          "operations": [{
                              "id": "t001/p001/o001", "from": "78/1", "to": "108/1|a", "with_successors": true,
                              "mapping": [["78/1", "108/1|a"], ["78/2", "108/1|a|1"]],
                              "links_rewritten": 2, "model_effect": { "preserved": true },
                              "fingerprints": [["78/1", "fp81"], ["78/2", "fp82"]]
                          }] },
                        { "id": "t001/p002", "kind": "first_entry", "description": "只有第一条", "status": "refused",
                          "reason": { "code": "human_link", "detail": "a human Card links the moving chain" },
                          "operations": [{ "id": "t001/p002/o001", "from": "78/1", "to": "108/1|a", "mapping": [["78/1", "108/1|a"]], "links_rewritten": 1, "model_effect": { "preserved": true }, "fingerprints": [] }] }
                    ] }
                ]
            }],
            "endpoints": { "108/0": { "fingerprint": "k0" }, "78/0": { "fingerprint": "o0" }, "108/1": { "fingerprint": "j1" } }
        })
    }

    #[test]
    fn a_join_finding_offers_its_two_nonmoving_resolutions_and_its_offered_plans_only() {
        let candidates = join_candidates(&join_review());
        let keys: Vec<String> = candidates.iter().map(Candidate::key).collect();
        assert_eq!(keys, vec!["t001/keep_distinct", "t001/keep_both_with_crossing", "t001/t001/p001", "t001/t001/p002"]);
        // A nonmoving resolution writes no Card and keeps both reviewed Topic roots as its evidence.
        assert!(candidates[0].ineligible.is_none() && candidates[1].ineligible.is_none());
        let roles: Vec<&str> = candidates[0].addresses.iter().map(|a| a.role).collect();
        assert_eq!(roles, vec!["keeper", "other"]);
        assert_eq!(candidates[0].addresses[1].reviewed, "o0");
        // The offered plan carries the reviewed mapping and the authored fingerprints it moves.
        let offered = &candidates[2];
        assert!(offered.ineligible.is_none());
        assert_eq!(offered.operation["operations"][0]["links_rewritten"], 2);
        let moving: BTreeMap<String, String> = offered
            .addresses
            .iter()
            .filter(|a| a.role == "moving")
            .map(|a| (a.address.clone(), a.reviewed.clone()))
            .collect();
        assert_eq!(moving["78/1"], "fp81");
        assert_eq!(moving["78/2"], "fp82");
        let destinations: Vec<&Addr> = offered.addresses.iter().filter(|a| a.role == "landing").collect();
        assert_eq!(destinations.len(), 1);
        assert!(destinations.iter().all(|a| a.expected_free));
        assert_eq!(destinations[0].address, "108/1|a");
        // Every moved Card is a `moving` address with the fingerprint the report reviewed for it.
        let moving_roles = offered.addresses.iter().filter(|a| a.role == "moving").count();
        assert_eq!(moving_roles, 2);
        assert_eq!(offered.addresses.iter().filter(|a| a.role == "joining").count(), 1);
        // A refused plan is listed with its refusal and can never be selected (D305).
        assert_eq!(candidates[3].ineligible.as_ref().unwrap()["code"], "human_link");
    }

    fn transfer_review() -> Value {
        json!({
            "report_id": "th-abc",
            "items": [
                { "id": "t001", "finding": "c001", "entry": "12/1", "status": "offered",
                  "statement": { "title": "T", "body": "B", "statement_fingerprint": "fp", "sources": ["book/1"], "formulas": [] },
                  "target": { "anchor": "book/1", "anchor_fingerprint": "afp", "to": "book/1|a", "statement_address": "book/1|a", "model_context": [] },
                  "carrier_scan": { "treatment": "create" }, "earlier_carrier": null,
                  "remedies": { "entry": { "title": "T", "body": "## 同述入口\n\n- [[book/1|a]]\n\n来源：[[book]]" }, "distinct": { "requires": ["…"] } },
                  "book": "book" },
                { "id": "t002", "finding": "c001", "entry": "12/2", "status": "offered",
                  "statement": { "title": "U", "body": "UB", "statement_fingerprint": "ufp", "sources": ["book/2"], "formulas": [] },
                  "target": { "anchor": "book/2", "anchor_fingerprint": "bfp", "to": "book/2|a", "statement_address": "book/2|a", "model_context": [] },
                  "carrier_scan": { "treatment": "create" }, "earlier_carrier": null,
                  "remedies": { "entry": { "title": "U", "body": "## 同述入口\n\n- [[book/2|a]]\n\n来源：[[book]]" }, "distinct": { "requires": ["…"] } },
                  "book": "book" }
            ],
            "endpoints": { "12/1": { "fingerprint": "e1" }, "12/2": { "fingerprint": "e2" }, "book/1": { "fingerprint": "a1" }, "book/2": { "fingerprint": "a2" } }
        })
    }

    #[test]
    fn no_selection_defers_every_item_and_declining_defers_it_visibly() {
        let review = transfer_review();
        let reports = Reports { topic_home: Some(&review), join: None };
        let items = settle(topic_home_candidates(&review), None, &reports).unwrap();
        assert_eq!(items.len(), 2);
        for item in &items {
            assert!(!item.selected());
            let row = item.json();
            assert_eq!(row["disposition"], "deferred");
            assert_eq!(row["reason"]["code"], "awaiting-selection");
        }
        // A selection that exists but accepts nothing is a decline, and it is recorded as one.
        let declined = json!({ "schema_version": 1, "note": "这批判定不做。" });
        let items = settle(topic_home_candidates(&review), Some(&declined), &reports).unwrap();
        assert!(items.iter().all(|item| !item.selected()));
        assert_eq!(items[0].json()["reason"]["code"], "declined");
    }

    #[test]
    fn a_selection_accepts_exactly_what_it_names_and_the_rest_stays_deferred() {
        let review = transfer_review();
        let reports = Reports { topic_home: Some(&review), join: None };
        let asked = json!({
            "schema_version": 1,
            "note": "只接受第一条。",
            "topic_home": {
                "schema_version": 1,
                "report_id": "th-abc",
                "decisions": [{ "finding": "c001", "entry": "12/1", "remedy": "entry", "note": "把本书的陈述转成文献陈述。" }]
            }
        });
        let items = settle(topic_home_candidates(&review), Some(&asked), &reports).unwrap();
        assert_eq!(items[0].json()["disposition"], "selected");
        assert_eq!(items[0].json()["remedy"], "entry");
        assert_eq!(items[0].json()["note"], "把本书的陈述转成文献陈述。");
        assert_eq!(items[1].json()["disposition"], "deferred");
        assert_eq!(items[1].json()["reason"]["code"], "not-selected");
        // A block for a report the work folder does not hold is refused, never guessed.
        let no_report = Reports { topic_home: None, join: None };
        let missing = settle(topic_home_candidates(&review), Some(&asked), &no_report).unwrap_err().to_string();
        assert!(missing.contains("no topic-home review"), "{missing}");
    }

    #[test]
    fn a_batch_selection_is_refused_for_an_unknown_field_a_note_or_an_unoffered_item() {
        let review = transfer_review();
        let reports = Reports { topic_home: Some(&review), join: None };
        let accept = json!({
            "schema_version": 1, "report_id": "th-abc",
            "decisions": [{ "finding": "c001", "entry": "12/1", "remedy": "entry", "note": "note" }]
        });
        let with = |extra: Value| {
            let mut asked = json!({ "schema_version": 1, "note": "note", "topic_home": accept });
            for (key, value) in extra.as_object().unwrap() {
                asked[key] = value.clone();
            }
            asked
        };
        let candidates = || topic_home_candidates(&review);
        let unknown = settle(candidates(), Some(&with(json!({ "everything": true }))), &reports).unwrap_err().to_string();
        assert!(unknown.contains("unknown field"), "{unknown}");
        let silent = settle(candidates(), Some(&with(json!({ "note": " " }))), &reports).unwrap_err().to_string();
        assert!(silent.contains("note"), "{silent}");
        let mut wrong_schema = with(json!({}));
        wrong_schema["schema_version"] = json!(2);
        let schema = settle(candidates(), Some(&wrong_schema), &reports).unwrap_err().to_string();
        assert!(schema.contains("schema_version"), "{schema}");
        // The qualified step's own refusal decides the item: a blocked item is never accepted.
        let blocked = json!({
            "schema_version": 1, "note": "note",
            "topic_home": { "schema_version": 1, "report_id": "th-abc",
                "decisions": [{ "finding": "c001", "entry": "12/2", "remedy": "entry", "note": "note" }] }
        });
        let mut blocked_review = review.clone();
        blocked_review["items"][1]["status"] = json!("blocked");
        blocked_review["items"][1]["reason"] = json!({ "code": "model_boundary", "detail": "a Model Card needs a separate decision" });
        let blocked_reports = Reports { topic_home: Some(&blocked_review), join: None };
        let refused = settle(topic_home_candidates(&blocked_review), Some(&blocked), &blocked_reports)
            .unwrap_err()
            .to_string();
        assert!(refused.contains("not an offered transfer"), "{refused}");
    }

    #[test]
    fn an_item_becomes_stale_when_an_address_no_longer_carries_what_the_report_reviewed() {
        // The report reviewed the Cards as they were; one of them changed afterwards.
        let reviewed = json!({ "address": "12/1", "title": "原样", "type": "idea", "body": "本来的陈述" });
        let anchor = json!({ "address": "book/1", "title": "锚", "type": "idea", "body": "锚" });
        let mut review = transfer_review();
        review["endpoints"]["12/1"]["fingerprint"] = json!(calls::authored_fingerprint(&reviewed));
        review["items"][0]["target"]["anchor_fingerprint"] = json!(calls::authored_fingerprint(&anchor));
        let reports = Reports { topic_home: Some(&review), join: None };
        let mut items = settle(topic_home_candidates(&review), None, &reports).unwrap();
        let mut cards: BTreeMap<String, Value> = BTreeMap::new();
        cards.insert("12/1".to_string(), json!({ "address": "12/1", "title": "改了", "type": "idea", "body": "另一位写作者改过" }));
        cards.insert("book/1".to_string(), anchor.clone());
        fill_current(&mut items, &cards);
        let row = items[0].json();
        assert_eq!(row["addresses"][0]["exists"], true);
        // The reviewed statement slot is still free; the entry is no longer the reviewed text.
        assert_eq!(row["addresses"][2]["exists"], false);
        let stale = row["stale"].as_array().unwrap();
        assert_eq!(stale.len(), 1, "{stale:?}");
        assert_eq!(stale[0]["address"], "12/1");
        // The unchanged item's entry was not read at all: an absent Card is not a stale one.
        assert_eq!(items[1].json()["addresses"][0]["exists"], false);
        assert!(items[1].json()["stale"].as_array().unwrap().is_empty());
    }

    /// A real, verifiable backup of a stand-in archive: this archive's id in its metadata and one
    /// Card, so the gate can check it is the right archive and not an empty file.
    fn backup_record(archive_id: &str) -> (tempfile::TempDir, Value) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.sqlite3");
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(&format!(
            "CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO metadata VALUES ('archive_id', '{archive_id}');
             CREATE TABLE cards (address TEXT PRIMARY KEY);
             INSERT INTO cards VALUES ('1/1');"
        ))
        .unwrap();
        drop(conn);
        let bytes = std::fs::metadata(&path).unwrap().len();
        let sha = publish::file_sha256(&path.to_string_lossy()).unwrap();
        (dir, json!({ "path": path.to_string_lossy(), "sha256": sha, "bytes": bytes }))
    }

    #[test]
    fn a_selected_batch_is_bound_to_a_verified_backup_or_not_selected_at_all() {
        let archive_id = "a".repeat(32);
        let (_dir, backup) = backup_record(&archive_id);
        // Nothing selected: no backup is asked for.
        assert!(backup_evidence(None, 0, &archive_id).unwrap().is_null());
        // One selected item without a backup is refused, and so is another archive's backup.
        assert!(backup_evidence(Some(&json!({})), 1, &archive_id).is_err());
        let other = backup_evidence(Some(&json!({ "backup": backup })), 1, &"b".repeat(32)).unwrap_err().to_string();
        assert!(other.contains("another archive"), "{other}");
        // A recorded file that changed is refused, so the evidence names the bytes it protects.
        let mut edited = backup.clone();
        edited["sha256"] = json!("0".repeat(64));
        let changed = backup_evidence(Some(&json!({ "backup": edited })), 1, &archive_id).unwrap_err().to_string();
        assert!(changed.contains("SHA-256"), "{changed}");
        let evidence = backup_evidence(Some(&json!({ "backup": backup.clone() })), 1, &archive_id).unwrap();
        assert_eq!(evidence["verified"], true);
        assert_eq!(evidence["archive_id"], json!(archive_id));
        assert_eq!(evidence["bytes"], backup["bytes"]);
    }

    #[test]
    fn the_batch_document_counts_every_disposition_and_says_what_it_did_not_do() {
        let review = transfer_review();
        let mut review_with_blocked = review.clone();
        review_with_blocked["items"][1]["status"] = json!("refused");
        review_with_blocked["items"][1]["reason"] = json!({ "code": "not_book_bound", "detail": "no own-book source" });
        let reports_value = Reports { topic_home: Some(&review_with_blocked), join: None };
        let items = settle(topic_home_candidates(&review_with_blocked), None, &reports_value).unwrap();
        let archive = json!({ "id": "a".repeat(32), "path": "D:\\a", "revision": 7 });
        let protected = json!({ "cards": 1, "digest": "d", "addresses": [] });
        let batch = batch_document(&archive, &[], items, None, Value::Null, protected.clone());
        assert_eq!(batch["status"], "deferred");
        assert_eq!(batch["deferred"]["code"], "no-selection");
        assert_eq!(batch["counts"]["candidates"], 2);
        assert_eq!(batch["counts"]["deferred"], 1);
        assert_eq!(batch["counts"]["ineligible"], 1);
        assert_eq!(batch["counts"]["selected"], 0);
        assert!(batch["scope"].as_str().unwrap().contains("nothing outside this batch"));
        let id = batch_id(&batch);
        assert!(is_batch_id(&id));
        // A batch that accepted an item is selected, and one that accepted none is declined.
        let asked = json!({
            "schema_version": 1, "note": "接受第一条。",
            "topic_home": { "schema_version": 1, "report_id": "th-abc",
                "decisions": [{ "finding": "c001", "entry": "12/1", "remedy": "entry", "note": "转成文献陈述。" }] }
        });
        let items = settle(topic_home_candidates(&review), Some(&asked), &reports_value).unwrap();
        let batch = batch_document(&archive, &[], items, Some(("s.json", &asked)), Value::Null, protected.clone());
        assert_eq!(batch["status"], "selected");
        assert!(batch["deferred"].is_null());
        assert_eq!(batch["counts"]["selected"], 1);
        assert_eq!(batch["selection"]["present"], true);
        let declined = json!({ "schema_version": 1, "note": "不做。" });
        let items = settle(topic_home_candidates(&review), Some(&declined), &reports_value).unwrap();
        let batch = batch_document(&archive, &[], items, Some(("s.json", &declined)), Value::Null, protected.clone());
        assert_eq!(batch["status"], "deferred");
        assert_eq!(batch["deferred"]["code"], "declined");
    }
}
