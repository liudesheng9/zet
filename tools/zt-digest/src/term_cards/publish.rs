//! `term-cards -Step apply|report`: publication of one book's Term Cards (design §12.5–§12.6,
//! S21 / #165, S33 / #182).
//!
//! `apply` publishes **every ready row**, in table order, as Side children of the Term list with
//! their complete continuation chains; a row whose content is not ready is left unpublished and
//! holds back no row after it. No placeholder head, no reserved address and no blind overwrite is
//! ever written (R0337, R0340, R0344–R0350).
//!
//! Before one Card is written the frozen plan is revalidated against a fresh coherent read: the
//! plan content hash (list rows/title, staged source, notation, limits, compiler/checker/rule and
//! instruction versions) must still describe the archive, or the step answers one typed
//! `plan_stale` line and writes nothing (R0339, R0342). The list's own *kind* is not list content:
//! a legacy `术语表` Idea Card is retyped to `term` metadata-only in the first batch, with its
//! authored title, body, Links, address and row identity kept byte for byte (R0336).
//!
//! The existing publication is then read by **identity**, never by position: a Side child of the
//! list is the committed row whose Term Card carries exactly the plan's rendered head — title and
//! body — and whose Direct chain carries exactly the rendered parts. Its addresses are reused and
//! nothing is written again, so a lost publication reply cannot create a second head (R0341). A
//! Side child that matches no planned row, a second head for one row, or a head filed outside the
//! list is a conflict: the step writes nothing instead of inserting, moving or editing an accepted
//! head (R0339, R0342).
//!
//! A row published later goes to **its table position**. When a ready row stands before rows that
//! are already published, its head is filed in the place of the next published row with one
//! identified `create_successor -Kind side -Before` item: that row and every later Side child of the
//! list move one label on inside the same archive transaction, so the list keeps its order (S33,
//! #182). Rows after the last published row are appended in table order. Each placement is its own
//! call, highest table position first, so the next placement names the head the previous one filed.
//!
//! Chains are packed into the archive's existing identified calls: at most 64 items and 1 MiB of
//! authored bytes per call, several complete chains in one call, a whole chain never split. A chain
//! that does not fit one existing call is refused for a bounded split/content fix instead of
//! opening a larger batch API (R0337). A part references its head through the same `@@` batch
//! reference the draft check bound (`@@term_head`), so no address is computed and written directly
//! (R0338).
//!
//! The operation keys are the stable `term-cards-<plan hash>/<book>/<plan revision>/apply/<row>`
//! identities, so a retry of the same plan replays the archive's own receipts and a new plan
//! revision is a new operation (R0341). The retype of a legacy list is its own `…/apply/list` call.
//! After every write the archive's publication is read once more and judged by the same identity
//! rule: the report carries the archive's current addresses (a placement moved the rows after it),
//! and a row this run wrote whose content is not the plan's rendering is a typed stop, never a
//! silent success.
//!
//! Both steps write `<Staging>/term-cards/report.json` and `report.md`, the publication coverage per
//! row (R0344–R0350): the source row and exact term, the math/concept classification and whether
//! promoted, the writer job, draft/check/review/fix facts, the row's own status (`committed`,
//! `ready_unpublished`, `pending`, `refused`, `conflicted`), the head/part addresses and receipt
//! keys, every structural conflict, and why publication stopped — naming each refused row — plus
//! which ready drafts remain staged. `report` is the same read without any write: the read-only
//! audit an incomplete stage still runs (R0351). A book unit is done only when every required row
//! and part is verified; committed work that leaves some row refused keeps the book incomplete
//! (R0343).

use super::*;
use crate::calls;
use crate::lockcheck;
use crate::zt::Zt;

/// The most items one identified call carries: the archive's own real-batch bound.
pub(crate) const CHUNK_ITEMS: usize = 64;
/// The most authored bytes one identified call carries: the archive's own real-batch bound.
pub(crate) const CHUNK_BYTES: usize = 1024 * 1024;
/// The exit code of a stop that is content, structure or concurrency, not a Lock.
const STOP_EXIT: i32 = 1;
/// The publication coverage report and its human-readable rendering.
const REPORT: &str = "report.json";
const REPORT_MD: &str = "report.md";
/// The chars a Term name cannot carry (`src/term.rs`): publication refuses the row for a bounded
/// list fix instead of leaving the archive's own write to refuse the whole batch.
const FORBIDDEN_NAME_CHARS: [char; 5] = ['[', ']', '@', '|', '`'];

// --- the two steps -------------------------------------------------------------------------------

/// `-Step apply`: revalidate the frozen plan, publish every ready row, verify the publication by
/// reading it back and report.
pub fn apply(p: &Params) -> Result<i32> {
    let compiled = Compiled::load(p)?;
    let manifest = book_manifest(p)?;
    let key = pstr(&manifest, "citation_key");
    let mut zt = crate::zt::start()?;
    // A Lock governs every write; the preflight names it before the first batch. The archive stays
    // authoritative: a Lock that appears later refuses its batch with the same typed answer.
    if let Some(refusal) = lockcheck::preflight(&mut zt, "term-cards", "apply", &key)? {
        return lockcheck::stop("term-cards", &refusal);
    }
    // The limits the plan froze are the ones the reviewed chain was rendered with; the invocation
    // repeats the other options, and the rebuilt plan must match the frozen one exactly.
    let frozen = frozen_params(p, &compiled);
    let p = &frozen;
    let fresh = match read_fresh(p, &manifest, &mut zt) {
        Ok(fresh) => fresh,
        Err(error) => return stop_unreadable(p, &compiled, error),
    };
    if let Some(code) = stale_stop(p, &compiled, &fresh)? {
        return Ok(code);
    }
    let mut run = prepare(&compiled, &fresh, &mut zt)?;
    let packing = pack(&compiled, &run);
    let mut ran = execute(&compiled, &packing, &run, |items| {
        send_batch(&mut zt, items)
    })?;
    // The archive's own publication decides what is committed and where it stands: a placement
    // moved the rows after it, and a receipt names the address its own call returned.
    let verdict = read_back(&compiled, &run, &mut zt, &ran)?;
    if ran.stop.is_none() {
        ran.stop = verdict.stop.clone();
    }
    run.settle(&ran, &verdict);
    // The publication record is derived progress for the next run and the next plan revision
    // (R0341, R0349); it is never the authority for what the archive holds.
    save_published(&compiled, &run)?;
    let stop = final_stop(&run, &ran);
    let report = build_report(p, &compiled, &fresh, &run, &ran, &verdict, stop.as_ref());
    let exit = stop.as_ref().map_or(0, |stop| stop.exit);
    finish(p, &compiled, &report, stop.as_ref(), exit)
}

/// `-Step report`: the read-only publication audit of the same rows. It writes the same two
/// artifacts and no Card, so an incomplete stage still reaches its audit (R0351).
pub fn report(p: &Params) -> Result<i32> {
    let compiled = Compiled::load(p)?;
    let manifest = book_manifest(p)?;
    let mut zt = crate::zt::start()?;
    let frozen = frozen_params(p, &compiled);
    let p = &frozen;
    let fresh = match read_fresh(p, &manifest, &mut zt) {
        Ok(fresh) => fresh,
        Err(error) => return stop_unreadable(p, &compiled, error),
    };
    if let Some(code) = stale_stop(p, &compiled, &fresh)? {
        return Ok(code);
    }
    let run = prepare(&compiled, &fresh, &mut zt)?;
    let ran = Ran::default();
    let verdict = Verdict {
        rows: run.rows.clone(),
        conflicts: run.conflicts.clone(),
        revision: run.revision_before,
        retyped: None,
        diverged: Vec::new(),
        stop: None,
    };
    let stop = final_stop(&run, &ran);
    let report = build_report(p, &compiled, &fresh, &run, &ran, &verdict, stop.as_ref());
    // The audit itself completes whenever it could read and write its report: an incomplete book is
    // what `complete` and the stop name, never a failed audit (R0351).
    finish(p, &compiled, &report, stop.as_ref(), 0)
}

/// Write both artifacts, print the compact answer and answer the given exit code.
fn finish(
    p: &Params,
    compiled: &Compiled,
    report: &Value,
    stop: Option<&Stop>,
    exit: i32,
) -> Result<i32> {
    let _ = compiled;
    let work = p.work();
    util::write_atomic(ps::join(&work, REPORT), &ps::json_pretty(report))?;
    util::write_atomic(ps::join(&work, REPORT_MD), &report_md(report))?;
    let mut summary = report["summary"].clone();
    summary["report"] = json!(ps::join(&work, REPORT));
    summary["report_md"] = json!(ps::join(&work, REPORT_MD));
    summary["work"] = json!(work);
    util::print_line(&canon::canonical_json(&summary))?;
    if let Some(stop) = stop {
        // The archive's typed refusal is the step's last line, so a caller reads the same JSON it
        // reads from every other step and never matches English text.
        if matches!(
            stop.code,
            "card_locked" | "card_check" | "stale_base" | "check_incomplete"
        ) {
            util::print_line(&canon::canonical_json(&stop.detail))?;
        }
        if exit != 0 {
            eprintln!("zt-digest term-cards: {}", stop.message);
        }
    }
    Ok(exit)
}

/// The limits the plan froze, in the invocation's own option shape. Both steps re-derive the plan
/// from a fresh read, and a rebuild that used other limits would answer `plan_stale` for a plan
/// that is current: the frozen limits are the ones the reviewed chain was rendered with (R0337).
fn frozen_params(p: &Params, compiled: &Compiled) -> Params {
    let limits = &compiled.plan["limits"];
    let number = |key: &str, default: i64| limits[key].as_i64().unwrap_or(default);
    let mut params = p.clone();
    params.per_job = number("per_job", DEFAULT_PER_JOB);
    params.math_per_job = number("math_per_job", DEFAULT_MATH_PER_JOB);
    params.max_steps = number("max_steps_per_card", DEFAULT_MAX_STEPS);
    params.max_prose = number("max_prose", DEFAULT_MAX_PROSE);
    params
}

/// The stop publication ended with: the batch or packing stop when one happened, else the
/// structure's own conflicts and unready rows.
fn final_stop(run: &Run, ran: &Ran) -> Option<Stop> {
    if let Some(stop) = &ran.stop {
        return Some(stop.clone());
    }
    run.structural_stop()
}

/// A plan that no longer describes the archive, or one the read cannot reproduce: one typed line,
/// nothing written (R0339, R0342).
fn stop_unreadable(p: &Params, compiled: &Compiled, error: anyhow::Error) -> Result<i32> {
    let value = json!({
        "schema_version": 1,
        "ok": false,
        "step": p.step,
        "code": "plan_stale",
        "book": compiled.book(),
        "plan_revision": compiled.revision,
        "plan_hash": compiled.plan_hash,
        "reasons": [format!("{error:#}")],
        "hint": "run -Step plan again and review the new plan revision; no Card was written",
    });
    util::print_line(&canon::canonical_json(&value))?;
    eprintln!("zt-digest term-cards: plan_stale: {error:#}");
    Ok(STOP_EXIT)
}

/// Compare the rebuilt plan with the frozen one. `None` means it still describes the archive;
/// `Some(exit)` means the typed stale answer was printed and nothing may be written.
fn stale_stop(p: &Params, compiled: &Compiled, fresh: &Fresh) -> Result<Option<i32>> {
    let rebuilt = &fresh.built.plan;
    if rebuilt["plan_hash"] == json!(compiled.plan_hash) {
        return Ok(None);
    }
    let value = json!({
        "schema_version": 1,
        "ok": false,
        "step": p.step,
        "code": "plan_stale",
        "book": compiled.book(),
        "plan_revision": compiled.revision,
        "plan_hash": compiled.plan_hash,
        "rebuilt_hash": rebuilt["plan_hash"],
        "reasons": stale_reasons(&compiled.plan, rebuilt),
        "hint": "run -Step plan again and review the new plan revision; no Card was written",
    });
    util::print_line(&canon::canonical_json(&value))?;
    eprintln!(
        "zt-digest term-cards: plan_stale: the frozen plan no longer describes this archive; run -Step plan again"
    );
    Ok(Some(STOP_EXIT))
}

/// Why the rebuilt plan differs from the frozen one, in the plan's own vocabulary.
fn stale_reasons(frozen: &Value, rebuilt: &Value) -> Vec<String> {
    let mut reasons = Vec::new();
    for (path, reason) in [
        (&["schema_version"][..], "the plan schema changed"),
        (&["kind"][..], "the artifact is not a term-cards plan"),
        (&["book"][..], "the plan belongs to another book or variant"),
        (&["list", "address"][..], "the book's Term list changed address"),
        (&["list", "body_sha256"][..], "the Term list's table changed since the plan"),
        (&["list", "title_sha256"][..], "the Term list's title changed since the plan"),
        (&["source", "fingerprint"][..], "the staged source sections changed since the plan"),
        (&["notation"][..], "the book's notation changed since the plan"),
        (&["planning_artifact"][..], "the Terms planning artifact changed since the plan"),
        (&["limits"][..], "the plan's limits changed"),
        (&["versions"][..], "the compiler/checker/rule or instruction contract changed"),
        (&["rows"][..], "the frozen rows or their source evidence changed"),
        (&["jobs"][..], "the job grouping changed"),
    ] {
        let of = |value: &Value| -> Value { path.iter().fold(value, |v, key| &v[*key]).clone() };
        if of(frozen) != of(rebuilt) {
            reasons.push(format!("{reason} (`{}`)", path.join(".")));
        }
    }
    if reasons.is_empty() {
        reasons.push("the frozen plan content changed".to_string());
    }
    reasons
}

// --- the publication input ----------------------------------------------------------------------

/// One rendered Card of a row's chain, exactly as the archive must hold it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Paper {
    /// `card` for the head, `part` for a continuation part.
    role: &'static str,
    title: String,
    /// The rendered body. A part still carries [`HEAD_REFERENCE`]; publication substitutes the
    /// head's batch reference or its address.
    body: String,
}

/// One row's publication state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    /// The row's head and parts carry exactly the plan's rendering in the archive.
    Committed,
    /// Ready, and the archive holds no head for it yet: its draft stays staged for the next run.
    ReadyUnpublished,
    /// A batch that carried the row got no answer, so its commit cannot be claimed.
    Pending,
    /// The row's content is not ready (or the archive refused its write): it cannot be published.
    Refused,
    /// A Card this plan did not publish holds the row's name, or an accepted head changed.
    Conflicted,
}

impl Status {
    fn id(self) -> &'static str {
        match self {
            Status::Committed => "committed",
            Status::ReadyUnpublished => "ready_unpublished",
            Status::Pending => "pending",
            Status::Refused => "refused",
            Status::Conflicted => "conflicted",
        }
    }
}

/// What the archive holds for one row.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Slot {
    /// No Card carries the row's name as a Side child of the list yet.
    Absent,
    /// The head and its continuation parts, in address order.
    Committed { head: String, parts: Vec<String> },
}

/// One frozen row, before the archive's publication is read.
#[derive(Clone)]
struct RowPlan {
    number: u64,
    term: String,
    job: String,
    planned_category: String,
    /// The S19 readiness entry, carried verbatim into the report (R0347).
    readiness: Value,
    /// The rendered chain when the row is ready; `None` when its content is not ready.
    chain: Option<Vec<Paper>>,
    /// Why the row's content is not ready (`not_ready`, or the bounded reason publication refuses
    /// it for): the reason a row keeps while the archive holds no head for it.
    blocked: Option<Value>,
}

/// One frozen row, its rendered chain and what the archive holds for it.
#[derive(Clone)]
struct RowState {
    number: u64,
    term: String,
    job: String,
    planned_category: String,
    readiness: Value,
    chain: Option<Vec<Paper>>,
    slot: Slot,
    status: Status,
    /// Why the row is not committed: the readiness status, a stale check, a conflict, a refusal.
    reason: Option<Value>,
    /// This run's receipts for the row, in chain order.
    receipts: Vec<Value>,
}

impl RowState {
    fn is_committed(&self) -> bool {
        matches!(self.slot, Slot::Committed { .. })
    }

    fn head(&self) -> Option<&String> {
        match &self.slot {
            Slot::Committed { head, .. } => Some(head),
            Slot::Absent => None,
        }
    }

    fn parts(&self) -> &[String] {
        match &self.slot {
            Slot::Committed { parts, .. } => parts,
            Slot::Absent => &[],
        }
    }
}

/// The list's existing publication, read once: its immediate Side children in address order, each
/// child's Direct chain of continuation parts, the authored facts of every one of them, and every
/// Term Card of the tree that the publication does not hold (a head filed elsewhere).
#[derive(Default)]
struct Scan {
    /// The list's own text read, for the retype's base fingerprint.
    list: Value,
    side: Vec<String>,
    chain: BTreeMap<String, Vec<String>>,
    cards: BTreeMap<String, Value>,
    /// Term Cards of the tree that are neither an immediate Side child of the list nor in one of
    /// their chains, with their authored facts: a head filed outside the list is a conflict.
    stray: Vec<Value>,
}

/// Everything the two steps decide before a write: the frozen plans, the rows, the structural
/// conflicts, the archive's current publication, the legacy list's retype item, and the archive
/// revision the read saw.
struct Run {
    plans: Vec<RowPlan>,
    rows: Vec<RowState>,
    /// Problems with no owning row: a Side child the plan did not publish, a second head for one
    /// row. Any of them stops publication before a write.
    conflicts: Vec<Value>,
    scan: Scan,
    /// The publication record this run started from, for a conflict's own vocabulary.
    published: Value,
    retype: Option<Value>,
    revision_before: u64,
}

impl Run {
    /// The structure's own stop: a conflict first, then the rows that are not committed. A row
    /// whose content is not ready is a degraded run (exit 0); a conflict, a refusal or a row that
    /// is ready but still unpublished needs the operator (exit 1). The line names every row that
    /// held publication back instead of the first one.
    fn structural_stop(&self) -> Option<Stop> {
        if let Some(row) = self.rows.iter().find(|row| row.status == Status::Conflicted) {
            let reason = row.reason.clone().unwrap_or(Value::Null);
            return Some(Stop {
                code: "conflicted",
                message: pstr(&reason, "message"),
                detail: json!({"row": row.number, "term": row.term, "reason": reason, "conflicts": self.conflicts}),
                exit: STOP_EXIT,
            });
        }
        if let Some(conflict) = self.conflicts.first() {
            return Some(Stop {
                code: "conflicted",
                message: pstr(conflict, "message"),
                detail: json!({"conflicts": self.conflicts}),
                exit: STOP_EXIT,
            });
        }
        let open: Vec<&RowState> = self
            .rows
            .iter()
            .filter(|row| row.status != Status::Committed)
            .collect();
        if open.is_empty() {
            return None;
        }
        let reason_of = |row: &RowState| row.reason.clone().unwrap_or(Value::Null);
        let failed = open
            .iter()
            .any(|row| !matches!(pstr(&reason_of(row), "code").as_str(), "not_ready" | "ready_unpublished"));
        let ready = open
            .iter()
            .any(|row| pstr(&reason_of(row), "code") == "ready_unpublished");
        let (code, exit) = if failed {
            ("refused", STOP_EXIT)
        } else if ready {
            ("ready_unpublished", 0)
        } else {
            ("not_ready", 0)
        };
        let rows: Vec<Value> = open
            .iter()
            .map(|row| {
                json!({
                    "row": row.number,
                    "term": row.term,
                    "status": row.status.id(),
                    "reason": reason_of(row),
                })
            })
            .collect();
        let named: Vec<String> = open
            .iter()
            .map(|row| format!("{} `{}`", row.number, row.term))
            .collect();
        let message = match code {
            "refused" => format!(
                "{} row(s) are refused ({}); every ready row is published and the book unit stays incomplete",
                open.len(),
                named.join(", ")
            ),
            "ready_unpublished" => format!(
                "{} ready row(s) are not published yet ({}); the book unit stays incomplete",
                open.len(),
                named.join(", ")
            ),
            _ => format!(
                "{} row(s) are not ready ({}); every ready row is published and the book unit stays incomplete",
                open.len(),
                named.join(", ")
            ),
        };
        Some(Stop {
            code,
            message,
            detail: json!({"rows": rows, "conflicts": self.conflicts}),
            exit,
        })
    }

    /// Fold the readback into the rows: the archive's own content and address decide the state,
    /// and the call that carried a row still contributes its receipts and the answer it gave.
    fn settle(&mut self, ran: &Ran, verdict: &Verdict) {
        for (row, observed) in self.rows.iter_mut().zip(verdict.rows.iter()) {
            row.slot = observed.slot.clone();
            row.status = observed.status;
            row.reason = observed.reason.clone();
            row.receipts = ran.receipts_of(row.number);
        }
        for row in &mut self.rows {
            let Some(batch) = ran.rows_of(row.number) else {
                continue;
            };
            match batch.state {
                "committed" | "replayed" => {
                    if verdict.diverged.contains(&row.number) {
                        row.status = Status::Conflicted;
                        row.reason = Some(json!({
                            "code": "output_diverged",
                            "message": "the committed Card no longer carries the plan's rendering",
                        }));
                    }
                }
                // The read proved the row's Cards exist; otherwise the lost answer stands.
                "unknown" => {
                    if row.status != Status::Committed {
                        row.status = Status::Pending;
                        row.reason = Some(json!({"code": "interrupted", "message": batch.message}));
                    }
                }
                "refused" => {
                    if row.status != Status::Committed {
                        row.status = Status::Refused;
                        let code = batch
                            .refusal
                            .as_ref()
                            .map(|refusal| pstr(refusal, "error"))
                            .unwrap_or_default();
                        row.reason = Some(json!({
                            "code": code,
                            "message": batch.message,
                            "refusal": batch.refusal,
                        }));
                    }
                }
                _ => {}
            }
        }
        self.conflicts = verdict.conflicts.clone();
    }
}

/// Read the plan's rows and the archive's publication; judge what is already committed.
fn prepare(compiled: &Compiled, fresh: &Fresh, zt: &mut Zt) -> Result<Run> {
    let state = local_state(compiled)?;
    let checks = current_checks(compiled)?;
    let readiness = readiness_of(compiled, &checks, &state, &[]);
    let plans = row_plans(compiled, &readiness, &checks)?;
    let list_address = fresh.list.address.clone();
    let scan = scan_list(zt, &fresh.tree, &list_address)?;
    let revision_before = fresh.tree["archive_revision"]
        .as_u64()
        .context("the tree read carries no archive revision")?;
    let published = state["published"].clone();
    let (rows, conflicts) = judge(&plans, &scan, &published, compiled.revision);
    let retype = retype_item(compiled, &fresh.list, &scan)?;
    Ok(Run {
        plans,
        rows,
        conflicts,
        scan,
        published,
        retype,
        revision_before,
    })
}

/// One row's exact publication input (S19-AC01, R0347): the readiness entry, and for a ready row
/// the rendered head and parts the check artifact verified. `apply` re-renders from the checked
/// draft and refuses any row whose rendering no longer reproduces the check's own hashes, so it
/// never writes content another build or another draft produced (R0341).
fn row_plans(
    compiled: &Compiled,
    readiness: &Value,
    checks: &BTreeMap<String, Value>,
) -> Result<Vec<RowPlan>> {
    let limits = compiled.checker_limits()?;
    let rows = compiled.row_terms();
    let book = compiled.book();
    let mut plans = Vec::new();
    for entry in readiness["terms"].as_array().into_iter().flatten() {
        let number = entry["row"].as_u64().unwrap_or(0);
        let term = pstr(entry, "term");
        let job = pstr(entry, "job");
        let status = pstr(entry, "status");
        let mut plan = RowPlan {
            number,
            term: term.clone(),
            job: job.clone(),
            planned_category: pstr(entry, "planned_category"),
            readiness: entry.clone(),
            chain: None,
            blocked: None,
        };
        if status != "ready" {
            plan.blocked = Some(json!({
                "code": "not_ready",
                "status": status,
                "message": format!("row {number} `{term}` is `{status}`"),
            }));
            plans.push(plan);
            continue;
        }
        match chain_of(compiled, &limits, &rows, checks, &job, number, &term, &book) {
            Ok(chain) => plan.chain = Some(chain),
            Err(reason) => plan.blocked = Some(reason),
        }
        plans.push(plan);
    }
    Ok(plans)
}

/// One ready row's rendered chain, rechecked against the check artifact's exact hashes.
#[allow(clippy::too_many_arguments)]
fn chain_of(
    compiled: &Compiled,
    limits: &zt_check::core::Limits,
    rows: &[String],
    checks: &BTreeMap<String, Value>,
    job: &str,
    number: u64,
    term: &str,
    book: &str,
) -> std::result::Result<Vec<Paper>, Value> {
    let blocked =
        |code: &str, message: String| json!({"code": code, "status": "refused", "message": message});
    let Some(row) = compiled.row(number) else {
        return Err(blocked(
            "unpublishable",
            format!("row {number} is not in the frozen plan; run -Step plan again"),
        ));
    };
    let term_row = zt_check::text::TermRow {
        term: term.to_string(),
        avoid: row["avoid"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        meaning: pstr(row, "meaning"),
        defined: pstr(row, "defined"),
    };
    // A Term name the archive cannot store would refuse the whole batch: name it here, as a bounded
    // list fix, instead of writing nothing and blaming the batch.
    let name = term.trim();
    if name.is_empty()
        || name.chars().count() > zt_check::text::MAX_TERM_NAME
        || name.contains(FORBIDDEN_NAME_CHARS)
    {
        return Err(blocked(
            "unpublishable_name",
            format!(
                "row {number} names `{name}`, which cannot be a Term name (1 through {} characters, no `[`, `]`, `@`, `|` or backtick); repair the list before publication",
                zt_check::text::MAX_TERM_NAME
            ),
        ));
    }
    let Some(check) = checks.get(job) else {
        return Err(blocked(
            "stale_check",
            format!("job `{job}` has no current check artifact; run -Step check again"),
        ));
    };
    let Some(entry) = check["terms"]
        .as_array()
        .and_then(|terms| terms.iter().find(|entry| entry["row"].as_u64() == Some(number)))
    else {
        return Err(blocked(
            "stale_check",
            format!("the check artifact of `{job}` holds no entry for row {number}"),
        ));
    };
    let draft = match zt_check::term::parse_draft(&entry["draft"], rows) {
        Ok(draft) => draft,
        Err(_) => {
            return Err(blocked(
                "stale_check",
                format!("row {number} `{term}` has no parseable checked draft; run -Step check again"),
            ));
        }
    };
    let rendered = match zt_check::term::render(&draft, &term_row, book, HEAD_REFERENCE, limits) {
        Ok(rendered) => rendered,
        Err(faults) => {
            return Err(blocked(
                "render_failed",
                format!("row {number} `{term}` does not render: {faults}"),
            ));
        }
    };
    // The rendering, the check's rendering and every Card hash must be the same content: a check
    // artifact that describes other bytes is stale, never a licence to publish them.
    if render_sha256(&rendered) != pstr(entry, "render_sha256") {
        return Err(blocked(
            "stale_check",
            format!("row {number} `{term}` renders other content than its check artifact; run -Step check again"),
        ));
    }
    let recorded = entry["cards"].as_array().cloned().unwrap_or_default();
    if recorded.len() != rendered.cards().len() {
        return Err(blocked(
            "stale_check",
            format!("row {number} `{term}` renders another number of Cards than its check artifact"),
        ));
    }
    let mut chain = Vec::new();
    for (index, card) in rendered.cards().iter().enumerate() {
        let sha = canon::hash_value(&json!({"title": card.title, "body": card.body}));
        if recorded[index]["sha256"] != json!(sha) {
            return Err(blocked(
                "stale_check",
                format!(
                    "row {number} `{term}` Card {} renders other bytes than its check artifact",
                    index + 1
                ),
            ));
        }
        // `@@` is publication's own reference syntax; anything else the writer authored would be
        // resolved (or refused) by the batch instead of reaching the archive verbatim.
        let expected = if card.role == "part" { 1 } else { 0 };
        if card.body.matches("@@").count() != expected {
            return Err(blocked(
                "unpublishable_reference",
                format!(
                    "row {number} `{term}` Card {} carries `@@`, which publication reserves for its head reference; repair the draft",
                    index + 1
                ),
            ));
        }
        chain.push(Paper {
            role: if card.role == "part" { "part" } else { "card" },
            title: card.title.clone(),
            body: card.body.clone(),
        });
    }
    Ok(chain)
}

/// Read the list's immediate Side children and their Direct chains through `get_tree`, then read
/// the authored facts of exactly those Cards in bounded `get_cards` reads, together with every Term
/// Card of the tree that the publication does not hold (a head filed outside the list, S33 / #182).
fn scan_list(zt: &mut Zt, tree: &Value, list: &str) -> Result<Scan> {
    let cards = tree["cards"].as_array().cloned().unwrap_or_default();
    let mut by_parent: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for card in &cards {
        by_parent
            .entry(pstr(card, "parent"))
            .or_default()
            .push(card.clone());
    }
    let mut side: Vec<String> = by_parent
        .get(list)
        .map(|children| {
            children
                .iter()
                .filter(|card| pstr(card, "edge") == "side")
                .map(|card| pstr(card, "address"))
                .collect()
        })
        .unwrap_or_default();
    side.sort_by(|a, b| address_order(a, b));
    let mut chain: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut addresses = vec![list.to_string()];
    for head in &side {
        let mut parts = Vec::new();
        let mut current = head.clone();
        loop {
            let next: Option<String> = by_parent.get(&current).and_then(|children| {
                children
                    .iter()
                    .find(|card| pstr(card, "edge") == "direct")
                    .map(|card| pstr(card, "address"))
            });
            match next {
                Some(child) => {
                    parts.push(child.clone());
                    current = child;
                }
                None => break,
            }
        }
        addresses.extend(parts.iter().cloned());
        chain.insert(head.clone(), parts);
    }
    addresses.extend(side.iter().cloned());
    // A Term Card of the tree that neither stands beside the list nor continues one of its rows is
    // read too: publication reports it instead of writing a second head for its term.
    let published: BTreeSet<&String> = addresses.iter().collect();
    let stray: Vec<String> = cards
        .iter()
        .filter(|card| pstr(card, "type") == "term")
        .map(|card| pstr(card, "address"))
        .filter(|address| !address.is_empty() && !published.contains(address))
        .collect();
    addresses.extend(stray.iter().cloned());
    let read = zt.cards(&addresses, "text")?;
    let revision = tree["archive_revision"]
        .as_u64()
        .context("the tree read carries no archive revision")?;
    if read.archive_revision != revision {
        bail!("the archive changed while reading the publication; run the step again");
    }
    let mut facts = BTreeMap::new();
    for card in &read.cards {
        facts.insert(pstr(card, "address"), card.clone());
    }
    let list_card = facts.get(list).cloned().unwrap_or(Value::Null);
    let strays = stray
        .iter()
        .filter_map(|address| facts.get(address).cloned())
        .collect();
    Ok(Scan {
        list: list_card,
        side,
        chain,
        cards: facts,
        stray: strays,
    })
}

// --- the structural judgement -------------------------------------------------------------------

/// Judge every row against the archive by **identity**, never by position (R0339, R0340, R0341,
/// R0348; S33 / #182): a Side child of the list is the committed row whose Term Card carries
/// exactly the plan's rendered head and whose Direct chain carries exactly the rendered parts.
/// A Side child that matches no planned row, a second head for one row, or a row's head filed
/// outside the list is a conflict, and the run writes nothing.
fn judge(plans: &[RowPlan], scan: &Scan, published: &Value, revision: i64) -> (Vec<RowState>, Vec<Value>) {
    let mut heads: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for address in &scan.side {
        let card = scan.cards.get(address).cloned().unwrap_or(Value::Null);
        let term = pstr(&card, "term");
        if pstr(&card, "type") == "term" && !term.is_empty() {
            heads.entry(term).or_default().push(address.clone());
        }
    }
    let mut rows: Vec<RowState> = Vec::new();
    let mut conflicts: Vec<Value> = Vec::new();
    let mut claimed: BTreeSet<String> = BTreeSet::new();
    for plan in plans {
        let mut row = RowState {
            number: plan.number,
            term: plan.term.clone(),
            job: plan.job.clone(),
            planned_category: plan.planned_category.clone(),
            readiness: plan.readiness.clone(),
            chain: plan.chain.clone(),
            slot: Slot::Absent,
            status: Status::Refused,
            reason: plan.blocked.clone(),
            receipts: Vec::new(),
        };
        let found = heads.get(&plan.term).cloned().unwrap_or_default();
        if found.len() > 1 {
            // Two heads for one row: publication files one head per row and never picks between
            // them, and it never deletes the second (R0339, R0342).
            claimed.extend(found.iter().cloned());
            row.status = Status::Conflicted;
            row.reason = Some(json!({
                "code": "duplicate_head",
                "kind": "duplicate_head",
                "term": plan.term,
                "heads": found,
                "message": format!(
                    "{} Term Cards carry row {} `{}`; publication files one head per row and never chooses between them",
                    found.len(), plan.number, plan.term
                ),
            }));
            rows.push(row);
            continue;
        }
        if let Some(head) = found.first() {
            claimed.insert(head.clone());
            let parts = scan.chain.get(head).cloned().unwrap_or_default();
            match plan.chain.as_ref() {
                // The row's draft is not ready, but its earlier accepted head stays exactly as it
                // is and is never edited from a new draft.
                None => {
                    row.slot = Slot::Committed {
                        head: head.clone(),
                        parts: parts.clone(),
                    };
                    row.reason = Some(json!({
                        "code": "not_ready",
                        "status": pstr(&plan.readiness, "status"),
                        "address": head,
                        "parts": parts,
                        "message": format!(
                            "row {} `{}` is not ready; its earlier accepted Card `{head}` stays in place and is never edited from a new draft",
                            plan.number, plan.term
                        ),
                    }));
                }
                Some(chain) => {
                    let expected = expected_chain(chain, &plan.term, head);
                    let observed = chain_contents(scan, head, &parts);
                    if observed == expected {
                        row.slot = Slot::Committed {
                            head: head.clone(),
                            parts,
                        };
                        row.status = Status::Committed;
                        row.reason = Some(json!({"code": "committed"}));
                    } else if published["rows"][plan.number.to_string()]["head"]["address"].as_str() == Some(head.as_str())
                        && published["rows"][plan.number.to_string()]["plan_revision"].as_i64().is_some_and(|earlier| earlier <= revision)
                    {
                        // The publication record names this head: an earlier plan revision published it (a Term
                        // backfill redrafts every row), or this one did and it changed since (the run's own
                        // Term-link pass adds Term links to it). It stays exactly as it is, counts as published,
                        // and holds no other row back; a head the record does not name is still a conflict.
                        row.slot = Slot::Committed {
                            head: head.clone(),
                            parts,
                        };
                        row.status = Status::Committed;
                        row.reason = Some(json!({
                            "code": "kept_earlier",
                            "address": head,
                            "message": format!(
                                "row {} `{}` keeps the Card `{head}` an earlier revision published; it is never edited from a new draft",
                                plan.number, plan.term
                            ),
                        }));
                    } else {
                        let (kind, message) = conflict_kind(&row, head, &parts, chain, published);
                        row.status = Status::Conflicted;
                        row.reason = Some(json!({
                            "code": kind,
                            "kind": kind,
                            "address": head,
                            "expected": expected.iter().map(|(role, sha)| json!({"role": role, "sha256": sha})).collect::<Vec<_>>(),
                            "observed": observed.iter().map(|(role, sha)| json!({"role": role, "sha256": sha})).collect::<Vec<_>>(),
                            "message": message,
                        }));
                    }
                }
            }
            rows.push(row);
            continue;
        }
        // No Side child of the list carries this row's term. A Term Card elsewhere in the tree does:
        // the row's head was filed outside the list, and publication never writes a second head.
        if let Some(stray) = scan
            .stray
            .iter()
            .find(|card| pstr(card, "term") == plan.term)
        {
            let address = pstr(stray, "address");
            row.status = Status::Conflicted;
            row.reason = Some(json!({
                "code": "misplaced_head",
                "kind": "misplaced_head",
                "address": address,
                "message": format!(
                    "the Term Card `{address}` carries row {} `{}` but is not an immediate Side child of the list; publication writes no second head (move it under the list first)",
                    plan.number, plan.term
                ),
            }));
            rows.push(row);
            continue;
        }
        if plan.chain.is_some() {
            row.status = Status::ReadyUnpublished;
            row.reason = Some(json!({
                "code": "ready_unpublished",
                "message": format!("row {} `{}` is ready and not published yet", plan.number, plan.term),
            }));
        }
        rows.push(row);
    }
    for head in &scan.side {
        if claimed.contains(head) {
            continue;
        }
        let observed = scan.cards.get(head).cloned().unwrap_or(Value::Null);
        conflicts.push(json!({
            "code": "unrelated_side",
            "kind": "unrelated_side",
            "address": head,
            "observed": {
                "type": pstr(&observed, "type"),
                "term": observed["term"].clone(),
                "title": pstr(&observed, "title"),
            },
            "message": format!(
                "the list's Side child `{head}` is not a row of the frozen plan; publication does not insert, move or delete it"
            ),
        }));
    }
    (rows, conflicts)
}

/// What kind of conflict a mismatching slot is, and the sentence that names it. A Card this work
/// folder published before (its address is in the state's publication record) changed its authored
/// content; anything else is another Card. Both stop publication (R0339, R0342).
fn conflict_kind(
    row: &RowState,
    head: &str,
    parts: &[String],
    chain: &[Paper],
    published: &Value,
) -> (&'static str, String) {
    let known = published["rows"]
        .as_object()
        .into_iter()
        .flat_map(|rows| rows.values())
        .any(|entry| {
            pstr(&entry["head"], "address") == head
                || entry["parts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|part| pstr(part, "address") == head)
        });
    if known {
        return (
            "changed_head",
            format!(
                "the accepted Card `{head}` of row {} `{}` changed its authored content since it was published; publication never rewrites an accepted head from a new draft",
                row.number, row.term
            ),
        );
    }
    if parts.len() != chain.len().saturating_sub(1) {
        return (
            "changed_chain",
            format!(
                "row {} `{}` holds {} continuation part(s) under `{head}`; the plan renders {}",
                row.number,
                row.term,
                parts.len(),
                chain.len().saturating_sub(1)
            ),
        );
    }
    (
        "changed_content",
        format!(
            "the Card at row {} `{}`'s slot (`{head}`) carries other content than the plan renders; publication does not overwrite it",
            row.number, row.term
        ),
    )
}

/// The exact content of a row's planned chain against one head address: the head, then every part
/// with the head reference resolved to that address.
fn expected_chain(chain: &[Paper], term: &str, head: &str) -> Vec<(&'static str, String)> {
    chain
        .iter()
        .map(|paper| {
            (
                paper.role,
                content_hash(&paper.title, &published_body(paper, head), Some(term)),
            )
        })
        .collect()
}

/// The authored content of the Cards a slot holds, head first, then its parts in address order.
fn chain_contents(scan: &Scan, head: &str, parts: &[String]) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    if let Some(card) = scan.cards.get(head) {
        out.push(("card", observed_hash(card)));
    }
    for part in parts {
        if let Some(card) = scan.cards.get(part) {
            out.push(("part", observed_hash(card)));
        }
    }
    out
}

/// One archived Term Card's content hash, over the identity publication decides with.
fn observed_hash(card: &Value) -> String {
    let term = card["term"].as_str().unwrap_or_default();
    content_hash(
        card["title"].as_str().unwrap_or_default(),
        card["body"].as_str().unwrap_or_default(),
        (!term.is_empty()).then_some(term),
    )
}

/// The identity one published Card must carry: its exact title and body, the Term type, and its
/// stored Term name. Publication compares no address and no receipt: a Card is the same Card
/// wherever the archive files it.
fn content_hash(title: &str, body: &str, term: Option<&str>) -> String {
    let mut value = json!({"type": "term", "title": title, "body": normalized_body(body)});
    if let Some(term) = term {
        value["term"] = json!(term);
    }
    canon::hash_value(&value)
}

/// A body as the archive stores and returns it (`src/card.rs::parse`): lines joined by `\n`, with
/// the leading and trailing blank lines dropped. The renderer's body ends with a newline the stored
/// Card does not keep, so both sides of the comparison read the same normalization.
fn normalized_body(body: &str) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let start = lines.iter().position(|line| !line.trim().is_empty());
    let end = lines.iter().rposition(|line| !line.trim().is_empty());
    match (start, end) {
        (Some(start), Some(end)) => lines[start..=end].join("\n"),
        _ => String::new(),
    }
}

/// A rendered body with the head reference resolved: `@@term_head` becomes the head's address.
fn published_body(paper: &Paper, head: &str) -> String {
    paper.body.replace(HEAD_REFERENCE, head)
}

// --- the legacy list's metadata-only retype -----------------------------------------------------

/// The first publication's metadata-only retype of a legacy `术语表` Idea Card (R0336, S21-AC01):
/// one `edit_card` that changes the Card type and nothing else. The authored title, body, Links,
/// address and row identity stay byte for byte, and the gate checks the retyped Card as a list.
fn retype_item(compiled: &Compiled, list: &ListRead, scan: &Scan) -> Result<Option<Value>> {
    if list.card_type == "term" {
        return Ok(None);
    }
    let address = list.address.clone();
    let card = scan.cards.get(&address).cloned().unwrap_or(Value::Null);
    let base = calls::authored_fingerprint(&card);
    if base.len() != 64 {
        bail!("the Term list `{address}` cannot be read for its retype; run -Step apply again");
    }
    let hash = &compiled.plan_hash;
    Ok(Some(json!({
        "op_key": format!(
            "term-cards-{}/{}/{}/apply/list",
            &hash[..16.min(hash.len())],
            compiled.book(),
            compiled.revision
        ),
        "client_ref": "list",
        "tool": "edit_card",
        "arguments": {"at": address, "type": "term", "base_fingerprint": base},
    })))
}

// --- bounded identified batches -----------------------------------------------------------------

/// One identified call: the exact items, the rows whose chains it publishes, and their size.
#[derive(Clone, Debug, PartialEq)]
struct Batch {
    items: Vec<Value>,
    rows: Vec<u64>,
    bytes: usize,
    /// `before` for a placement in the place of a published row, `append` for a chain after the last
    /// published row, `list` for the legacy list's retype.
    placement: &'static str,
}

/// One ready row whose table position is before rows that are already published: it is filed in the
/// place of the next published row, and that call's answer names the address the row took.
struct Hole {
    /// The row's index in the frozen table, for the placement order.
    index: usize,
    row: u64,
    term: String,
    chain: Vec<Paper>,
    /// Which address the head is filed before: a published head read at the start of the run, or the
    /// head the previous placement of this run created.
    before: Before,
}

/// The address a hole's head is filed before.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Before {
    /// The head of the next published row, as the run's first read found it.
    Published(String),
    /// The head the placement sent immediately before this one created.
    Placed,
}

/// The packed publication: the repaired rows' placements, one call each, and the chains appended
/// after the last published row, packed into bounded calls.
struct Packing {
    /// The placements in sending order: the highest table position first, so the next placement
    /// names the head the previous one filed.
    holes: Vec<Hole>,
    batches: Vec<Batch>,
    stop: Option<Stop>,
}

/// One row that cannot be published as planned, with the bounded fix its message names.
#[derive(Debug)]
struct Blocked {
    code: &'static str,
    message: String,
    row: u64,
}

impl Blocked {
    fn stop(&self) -> Stop {
        Stop {
            code: self.code,
            message: self.message.clone(),
            detail: json!({"row": self.row, "code": self.code}),
            exit: STOP_EXIT,
        }
    }
}

/// Pack every ready row that is not published yet. A row whose table position is before published
/// rows becomes one placement of its own, highest table position first; the rows after the last
/// published row are packed into identified calls, several complete chains in one call, at most 64
/// items and 1 MiB each, a whole chain never split (R0337, R0338; S33 / #182). A conflict stops
/// everything before the first call.
fn pack(compiled: &Compiled, run: &Run) -> Packing {
    let mut packing = Packing {
        holes: Vec::new(),
        batches: Vec::new(),
        stop: None,
    };
    if run
        .rows
        .iter()
        .any(|row| row.status == Status::Conflicted)
        || !run.conflicts.is_empty()
    {
        return packing;
    }
    let published: Vec<bool> = run.rows.iter().map(|row| row.is_committed()).collect();
    let last_published = published.iter().rposition(|committed| *committed);
    // The nearest published row after each table position, so a repaired row knows the head it
    // must stand before.
    let next_published: Vec<Option<usize>> = (0..run.rows.len())
        .map(|index| {
            (index + 1..run.rows.len())
                .find(|later| published[*later])
        })
        .collect();
    let mut ready: Vec<usize> = Vec::new();
    for (index, row) in run.rows.iter().enumerate() {
        if row.is_committed() || row.status != Status::ReadyUnpublished {
            continue;
        }
        let Some(chain) = row.chain.as_ref() else {
            continue;
        };
        let before = match next_published[index] {
            Some(next) => match run.rows[next].head() {
                Some(head) => Before::Published(head.clone()),
                None => continue,
            },
            // After every published row: the chain is appended in table order below.
            None => {
                ready.push(index);
                continue;
            }
        };
        packing.holes.push(Hole {
            index,
            row: row.number,
            term: row.term.clone(),
            chain: chain.clone(),
            before,
        });
    }
    // A placement names a head, so the highest table position is filed first; each later placement
    // before the head the previous one filed, unless its own next published row stands in between.
    let mut previous: Option<usize> = None;
    for hole in packing.holes.iter_mut().rev() {
        let next = next_published[hole.index].expect("a placement has a published row after it");
        hole.before = match previous {
            Some(sent) if sent < next => Before::Placed,
            _ => Before::Published(
                run.rows[next]
                    .head()
                    .cloned()
                    .expect("a committed row has a head"),
            ),
        };
        previous = Some(hole.index);
    }
    packing.holes.reverse();
    // The rows after the last published row append in table order; `at` names the list itself.
    let mut items: Vec<Value> = Vec::new();
    let mut rows: Vec<u64> = Vec::new();
    let mut bytes = 0usize;
    let after = last_published.map_or(0, |index| index + 1);
    for index in ready.into_iter().filter(|index| *index >= after) {
        let row = &run.rows[index];
        let Some(chain) = row.chain.as_ref() else {
            continue;
        };
        let chain_items = match chain_items(compiled, row.number, &row.term, chain, None) {
            Ok(chain_items) => chain_items,
            Err(blocked) => {
                packing.stop = Some(blocked.stop());
                break;
            }
        };
        let chain_bytes: usize = chain_items
            .iter()
            .map(|item| authored_bytes(&item["arguments"]))
            .sum();
        if chain_items.len() > CHUNK_ITEMS || chain_bytes > CHUNK_BYTES {
            packing.batches.push(Batch {
                items: std::mem::take(&mut items),
                rows: std::mem::take(&mut rows),
                bytes,
                placement: "append",
            });
            packing.stop = Some(too_large(row.number, &row.term, chain_items.len(), chain_bytes));
            break;
        }
        if !items.is_empty() && items.len() + chain_items.len() > CHUNK_ITEMS
            || !items.is_empty() && bytes + chain_bytes > CHUNK_BYTES
        {
            packing.batches.push(Batch {
                items: std::mem::take(&mut items),
                rows: std::mem::take(&mut rows),
                bytes,
                placement: "append",
            });
            bytes = 0;
        }
        items.extend(chain_items);
        bytes += chain_bytes;
        rows.push(row.number);
    }
    if !items.is_empty() {
        packing.batches.push(Batch {
            items,
            rows,
            bytes,
            placement: "append",
        });
    }
    packing
        .batches
        .retain(|batch| !batch.items.is_empty());
    packing
}

/// One row's chain that does not fit a single identified call: a bounded split/content fix, never a
/// new larger batch API (R0337).
fn too_large(row: u64, term: &str, cards: usize, bytes: usize) -> Stop {
    Blocked {
        code: "chain_too_large",
        message: format!(
            "row {row} `{term}` renders {cards} Card(s) and {bytes} authored bytes; one identified call carries at most {CHUNK_ITEMS} Cards and {CHUNK_BYTES} bytes, and a chain is never split: split the content and run the check again"
        ),
        row,
    }
    .stop()
}

/// One row's items: its head as a Side child of the list, then each part as the next Direct
/// successor of the previous Card, so a part references an earlier item instead of an address
/// (R0338). `before` files the head in the place of that published Side child. The keys are stable
/// per run, book, plan revision, step and row (R0341).
fn chain_items(
    compiled: &Compiled,
    number: u64,
    term: &str,
    chain: &[Paper],
    before: Option<&str>,
) -> std::result::Result<Vec<Value>, Blocked> {
    let blocked = |message: String| Blocked {
        code: "unpublishable",
        message,
        row: number,
    };
    let list = compiled.plan["list"]["address"]
        .as_str()
        .ok_or_else(|| blocked("the plan names no Term list address".to_string()))?;
    let hash = &compiled.plan_hash;
    let run = format!("term-cards-{}", &hash[..16.min(hash.len())]);
    let unit = compiled.book();
    let revision = compiled.revision;
    let mut items = Vec::new();
    for (index, paper) in chain.iter().enumerate() {
        let head_ref = format!("r{number:03}");
        let (client_ref, at, kind, key) = if index == 0 {
            (head_ref.clone(), list.to_string(), "side", head_ref.clone())
        } else {
            let previous = if index == 1 {
                head_ref.clone()
            } else {
                format!("{head_ref}-p{:02}", index - 1)
            };
            (
                format!("{head_ref}-p{index:02}"),
                format!("@@{previous}"),
                "direct",
                format!("{head_ref}-p{index:02}"),
            )
        };
        let body = paper.body.replace(HEAD_REFERENCE, &format!("@@{head_ref}"));
        let mut arguments = json!({
            "at": at,
            "kind": kind,
            "type": "term",
            "term": term,
            "title": paper.title,
            "body": body,
        });
        if index == 0
            && let Some(before) = before
        {
            arguments["before"] = json!(before);
        }
        items.push(json!({
            "op_key": format!("{run}/{unit}/{revision}/apply/{key}"),
            "client_ref": client_ref,
            "tool": "create_successor",
            "arguments": arguments,
        }));
    }
    Ok(items)
}

/// The authored bytes one item carries, as the archive's own batch bound counts them.
fn authored_bytes(arguments: &Value) -> usize {
    ["title", "body", "text", "bibtex", "base"]
        .iter()
        .map(|name| arguments[*name].as_str().map(str::len).unwrap_or(0))
        .sum()
}

/// The row and role an item's client reference names: `r001` is row 1's head, `r001-p02` its second
/// continuation part.
fn ref_role(client_ref: &str) -> Option<(u64, &'static str, usize)> {
    let (row, rest) = match client_ref.split_once('-') {
        Some((row, rest)) => (row, Some(rest)),
        None => (client_ref, None),
    };
    let number = row.strip_prefix('r')?.parse::<u64>().ok()?;
    match rest.and_then(|rest| rest.strip_prefix('p')) {
        None => Some((number, "card", 0)),
        Some(index) => Some((number, "part", index.parse::<usize>().ok()?)),
    }
}

// --- sending and stopping -----------------------------------------------------------------------

/// The answer to one batch.
#[derive(Clone, Debug, PartialEq)]
enum Reply {
    /// The batch committed or replayed; its results in order.
    Done(Vec<Value>),
    /// The archive refused the whole batch with this typed refusal.
    Refused(Value),
    /// The answer was lost: the batch may or may not have committed.
    Lost(String),
}

/// What one batch came to.
#[derive(Clone, Debug, PartialEq)]
struct BatchState {
    batch: Batch,
    /// `committed`, `replayed`, `refused`, `unknown` or `pending`.
    state: &'static str,
    replayed: usize,
    results: Vec<Value>,
    /// The `moved` maps this call reported: every Card a placement shifted, old address to new.
    moved: Vec<BTreeMap<String, String>>,
    refusal: Option<Value>,
    message: String,
}

/// The stop that ended the run.
#[derive(Clone, Debug, PartialEq)]
struct Stop {
    code: &'static str,
    message: String,
    detail: Value,
    exit: i32,
}

/// What the batch execution came to.
#[derive(Clone, Debug, Default, PartialEq)]
struct Ran {
    batches: Vec<BatchState>,
    stop: Option<Stop>,
}

impl Ran {
    /// The batch that carried one row, when a batch carried it.
    fn rows_of(&self, row: u64) -> Option<&BatchState> {
        self.batches
            .iter()
            .find(|batch| batch.batch.rows.contains(&row))
    }

    /// One row's receipts in chain order, each address as it stands after every *later* placement:
    /// a call's own `moved` map shifts the rows after it, never the chain it just filed.
    fn receipts_of(&self, row: u64) -> Vec<Value> {
        let Some(index) = self
            .batches
            .iter()
            .position(|batch| batch.batch.rows.contains(&row))
        else {
            return Vec::new();
        };
        let batch = &self.batches[index];
        if !matches!(batch.state, "committed" | "replayed") {
            return Vec::new();
        }
        let later: Vec<&BTreeMap<String, String>> = self
            .batches
            .iter()
            .skip(index + 1)
            .flat_map(|batch| batch.moved.iter())
            .collect();
        let mut receipts = Vec::new();
        for (item, result) in batch.batch.items.iter().zip(&batch.results) {
            let Some((number, role, _)) = ref_role(&pstr(item, "client_ref")) else {
                continue;
            };
            if number != row {
                continue;
            }
            receipts.push(json!({
                "op_key": item["op_key"],
                "address": through(&later, &pstr(&result["result"], "address")),
                "role": if role == "card" { "card" } else { "part" },
                "replayed": result["replayed"],
            }));
        }
        receipts
    }
}

/// An address after a run of sequential renames: each call's `moved` map applies in call order.
fn through(moves: &[&BTreeMap<String, String>], address: &str) -> String {
    let mut current = address.to_string();
    for map in moves {
        if let Some(next) = map.get(&current) {
            current = next.clone();
        }
    }
    current
}

fn pending_state(batch: &Batch) -> BatchState {
    BatchState {
        batch: batch.clone(),
        state: "pending",
        replayed: 0,
        results: Vec::new(),
        moved: Vec::new(),
        refusal: None,
        message: String::new(),
    }
}

/// Send the publication in its order: the legacy list's retype first, then each placement of a
/// repaired row — the highest table position first, so the next one names the head the previous
/// one filed — then the chains after the last published row. A committed call's receipts stay; a
/// refused call stops the run with the archive's typed answer; a lost answer stops it with that
/// call unknown, because its commit cannot be claimed (R0341).
fn execute<F>(
    compiled: &Compiled,
    packing: &Packing,
    run: &Run,
    mut send: F,
) -> Result<Ran>
where
    F: FnMut(&[Value]) -> Result<Reply>,
{
    let mut ran = Ran::default();
    let mut stop: Option<Stop> = None;
    // A legacy list is retyped only when a head is filed under it: the retype exists to make the
    // heads legal, so a run with nothing to publish writes no Card at all.
    let publishing = !packing.holes.is_empty() || !packing.batches.is_empty();
    if let Some(retype) = run.retype.as_ref().filter(|_| publishing) {
        let batch = Batch {
            items: vec![retype.clone()],
            rows: Vec::new(),
            bytes: authored_bytes(&retype["arguments"]),
            placement: "list",
        };
        stop = send_call(&batch, &mut send, &mut ran)?;
    }
    let mut placed: Option<String> = None;
    for hole in &packing.holes {
        if stop.is_some() {
            break;
        }
        let before = match &hole.before {
            Before::Published(head) => head.clone(),
            Before::Placed => placed.clone().unwrap_or_default(),
        };
        if before.is_empty() {
            stop = Some(Stop {
                code: "conflicted",
                message: format!(
                    "row {} `{}` must stand before the head the previous placement filed, and this run has no such answer; run -Step apply again",
                    hole.row, hole.term
                ),
                detail: json!({"row": hole.row, "term": hole.term}),
                exit: STOP_EXIT,
            });
            break;
        }
        let items = match chain_items(compiled, hole.row, &hole.term, &hole.chain, Some(&before)) {
            Ok(items) => items,
            Err(blocked) => {
                stop = Some(blocked.stop());
                break;
            }
        };
        let bytes: usize = items
            .iter()
            .map(|item| authored_bytes(&item["arguments"]))
            .sum();
        if items.len() > CHUNK_ITEMS || bytes > CHUNK_BYTES {
            stop = Some(too_large(hole.row, &hole.term, items.len(), bytes));
            break;
        }
        let batch = Batch {
            items,
            rows: vec![hole.row],
            bytes,
            placement: "before",
        };
        stop = send_call(&batch, &mut send, &mut ran)?;
        if stop.is_none() {
            placed = ran.batches.iter().rev().find_map(|state| {
                state
                    .batch
                    .items
                    .iter()
                    .zip(&state.results)
                    .find_map(|(item, result)| {
                        match ref_role(&pstr(item, "client_ref")) {
                            Some((number, "card", _)) if number == hole.row => {
                                Some(pstr(&result["result"], "address"))
                            }
                            _ => None,
                        }
                    })
            });
        }
    }
    for batch in &packing.batches {
        if stop.is_some() {
            break;
        }
        stop = send_call(batch, &mut send, &mut ran)?;
    }
    ran.stop = stop.or_else(|| packing.stop.clone());
    Ok(ran)
}

/// Send one identified call and keep what it answered.
fn send_call<F>(batch: &Batch, send: &mut F, ran: &mut Ran) -> Result<Option<Stop>>
where
    F: FnMut(&[Value]) -> Result<Reply>,
{
    match send(&batch.items)? {
        Reply::Done(results) => {
            let mut state = done_state(batch, results)?;
            for result in &state.results {
                if let Some(moved) = result["result"]["moved"].as_object() {
                    state.moved.push(
                        moved
                            .iter()
                            .map(|(from, to)| {
                                (from.clone(), to.as_str().unwrap_or_default().to_string())
                            })
                            .collect(),
                    );
                }
            }
            ran.batches.push(state);
            Ok(None)
        }
        Reply::Refused(refusal) => {
            let code = pstr(&refusal, "error");
            let (stop_code, exit) = stop_of(&code);
            let message = refusal["message"]
                .as_str()
                .filter(|message| !message.trim().is_empty())
                .or_else(|| refusal["error"].as_str())
                .unwrap_or("the archive refused the batch")
                .to_string();
            let mut state = pending_state(batch);
            state.state = "refused";
            state.refusal = Some(refusal.clone());
            state.message = message.clone();
            ran.batches.push(state);
            Ok(Some(Stop {
                code: stop_code,
                message,
                detail: refusal,
                exit,
            }))
        }
        Reply::Lost(message) => {
            let mut state = pending_state(batch);
            state.state = "unknown";
            state.message = message.clone();
            ran.batches.push(state);
            Ok(Some(Stop {
                code: "interrupted",
                message: format!(
                    "the call for row(s) {} got no answer ({message}); it may or may not have committed: run -Step apply again to verify its Cards before publishing more",
                    batch.rows.iter().map(u64::to_string).collect::<Vec<_>>().join(", ")
                ),
                detail: json!({"rows": batch.rows, "reason": message}),
                exit: STOP_EXIT,
            }))
        }
    }
}

/// One answered batch: verify every result's identity and order before its commit is believed.
fn done_state(batch: &Batch, results: Vec<Value>) -> Result<BatchState> {
    if results.len() != batch.items.len() {
        bail!(
            "incomplete identified batch: {} of {} results",
            results.len(),
            batch.items.len()
        );
    }
    let mut replayed = 0usize;
    for (item, result) in batch.items.iter().zip(&results) {
        if result["op_key"] != item["op_key"]
            || result["client_ref"] != item["client_ref"]
            || !result["replayed"].is_boolean()
            || pstr(&result["result"], "address").is_empty()
        {
            bail!(
                "incomplete identified batch: result identity/order mismatch at {}",
                item["op_key"].as_str().unwrap_or_default()
            );
        }
        if result["replayed"] == json!(true) {
            replayed += 1;
        }
    }
    let state = if replayed == results.len() {
        "replayed"
    } else {
        "committed"
    };
    Ok(BatchState {
        batch: batch.clone(),
        state,
        replayed,
        results,
        moved: Vec::new(),
        refusal: None,
        message: String::new(),
    })
}

/// The typed stop a refusal code is: a Lock keeps exit 2, a blocking Card check and a stale base
/// are content/concurrency stops with exit 1.
fn stop_of(code: &str) -> (&'static str, i32) {
    match code {
        "card_locked" => ("card_locked", lockcheck::LOCKED_EXIT),
        "card_check" => ("card_check", STOP_EXIT),
        "write_conflict" => ("stale_base", STOP_EXIT),
        "check_incomplete" => ("check_incomplete", STOP_EXIT),
        _ => ("refused", STOP_EXIT),
    }
}

/// Send one real identified batch through the archive.
fn send_batch(zt: &mut Zt, items: &[Value]) -> Result<Reply> {
    let reply = match zt.request("apply_batch", json!({ "items": items })) {
        Ok(reply) => reply,
        Err(error) => return Ok(Reply::Lost(format!("{error:#}"))),
    };
    if let Some(message) = zt_check::mcp::reply_error(&reply) {
        return Ok(Reply::Refused(calls::refusal_of(&reply).unwrap_or_else(
            || json!({"error": "refused", "message": message}),
        )));
    }
    let text = zt_check::mcp::reply_text(&reply);
    let value: Value =
        serde_json::from_str(&text).context("incomplete identified batch: the answer is not JSON")?;
    let results = value["results"]
        .as_array()
        .context("incomplete identified batch: missing results")?
        .clone();
    Ok(Reply::Done(results))
}

// --- readback -----------------------------------------------------------------------------------

/// What the readback observed: the rows judged from the archive's own content, its structural
/// conflicts, the archive revision after the writes, the retype's effect, the written rows whose
/// content diverged from the plan, and the stop that divergence is.
#[derive(Default)]
struct Verdict {
    rows: Vec<RowState>,
    conflicts: Vec<Value>,
    revision: u64,
    retyped: Option<Value>,
    diverged: Vec<u64>,
    stop: Option<Stop>,
}

/// Read the archive once more and judge the publication by identity, not by receipt: the report's
/// addresses are the archive's current ones (a placement moved the rows after it), and a row this
/// run wrote that the archive does not hold as the plan rendered it is a typed stop, never a silent
/// success (S21-AC01, S21-AC07).
fn read_back(compiled: &Compiled, run: &Run, zt: &mut Zt, ran: &Ran) -> Result<Verdict> {
    let list = pstr(&run.scan.list, "address");
    let tree = zt.call("get_tree", json!({"tree": compiled.book()}))?;
    let revision = tree["archive_revision"]
        .as_u64()
        .context("the tree read carries no archive revision")?;
    let scan = scan_list(zt, &tree, &list)?;
    let (rows, conflicts) = judge(&run.plans, &scan, &run.published, compiled.revision);
    let diverged: Vec<u64> = ran
        .batches
        .iter()
        .filter(|batch| matches!(batch.state, "committed" | "replayed"))
        .flat_map(|batch| batch.batch.rows.iter().copied())
        .filter(|row| {
            rows.iter()
                .find(|state| state.number == *row)
                .is_none_or(|state| state.status != Status::Committed)
        })
        .collect();
    let mut detail = json!({"rows": diverged});
    let mut retyped = None;
    // The retype is verified only when this run sent it: a run with nothing to publish writes no
    // Card, so the list's kind is the read's business alone.
    let sent_retype = ran
        .batches
        .iter()
        .any(|batch| batch.batch.placement == "list");
    if run.retype.is_some() && sent_retype {
        // A retyped list keeps its authored title and body byte for byte and becomes a Term Card.
        let planned = &compiled.plan["list"];
        let card = scan.cards.get(&list).cloned().unwrap_or(Value::Null);
        let title_sha = canon::sha256_hex(pstr(&card, "title").as_bytes());
        let body_sha = canon::sha256_hex(pstr(&card, "body").as_bytes());
        let observed = json!({
            "address": list,
            "from": planned["type"],
            "to": pstr(&card, "type"),
            "title_sha256": title_sha,
            "body_sha256": body_sha,
        });
        if title_sha != pstr(planned, "title_sha256")
            || body_sha != pstr(planned, "body_sha256")
            || pstr(&card, "type") != "term"
        {
            detail["list"] = observed.clone();
        }
        retyped = Some(observed);
    }
    let stop = divergence_stop(&detail);
    Ok(Verdict {
        rows,
        conflicts,
        revision,
        retyped,
        diverged,
        stop,
    })
}

/// The typed stop a divergence is, or `None` when the archive holds every written Card.
fn divergence_stop(detail: &Value) -> Option<Stop> {
    let rows = detail["rows"].as_array().map_or(0, Vec::len);
    let list = !detail["list"].is_null();
    if rows == 0 && !list {
        return None;
    }
    let mut what = format!("{rows} committed row(s)");
    if list {
        what.push_str(" and the retyped list");
    }
    Some(Stop {
        code: "output_diverged",
        message: format!("{what} do not carry the plan's rendering; keep them and replan"),
        detail: detail.clone(),
        exit: STOP_EXIT,
    })
}

// --- the publication record ---------------------------------------------------------------------

/// Record the publication this run verified in the compiler's derived state (R0349): each committed
/// row's head and part addresses with the operation keys that committed them. A row committed by an
/// earlier revision keeps its recorded entry, because this run has no receipt of its own for it.
fn save_published(compiled: &Compiled, run: &Run) -> Result<()> {
    let mut state = local_state(compiled)?;
    if !state["published"].is_object() {
        state["published"] = json!({"rows": {}});
    }
    let Some(rows) = state["published"]["rows"].as_object_mut() else {
        bail!("the term-cards state's publication record is not an object; remove state.json and re-run -Step plan");
    };
    for row in &run.rows {
        let (Some(head), false) = (row.head(), row.status == Status::Conflicted) else {
            continue;
        };
        if row.status != Status::Committed && row.status != Status::Refused {
            continue;
        }
        let receipts = &row.receipts;
        if receipts.is_empty() && rows.contains_key(&row.number.to_string()) {
            continue;
        }
        let key = |role: &str, address: &str| -> Value {
            receipts
                .iter()
                .find(|receipt| receipt["role"] == json!(role) && receipt["address"] == json!(address))
                .map(|receipt| receipt["op_key"].clone())
                .unwrap_or(Value::Null)
        };
        let parts: Vec<Value> = row
            .parts()
            .iter()
            .map(|part| json!({"address": part, "op_key": key("part", part)}))
            .collect();
        rows.insert(
            row.number.to_string(),
            json!({
                "row": row.number,
                "term": row.term,
                "plan_revision": compiled.revision,
                "head": {"address": head, "op_key": key("card", head)},
                "parts": parts,
                "at": canon::utc_now_o(),
            }),
        );
    }
    util::write_atomic(compiled.state_path(), &ps::json_pretty(&state))
}

// --- the report ---------------------------------------------------------------------------------

/// The publication coverage of one run (R0344–R0350).
fn build_report(
    p: &Params,
    compiled: &Compiled,
    fresh: &Fresh,
    run: &Run,
    ran: &Ran,
    verdict: &Verdict,
    stop: Option<&Stop>,
) -> Value {
    let mut rows = Vec::new();
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut staged = Vec::new();
    let mut written = 0usize;
    let mut replayed = 0usize;
    for row in &run.rows {
        let batch = ran.rows_of(row.number).map(|batch| batch.state);
        for receipt in &row.receipts {
            if receipt["replayed"] == json!(false) {
                written += 1;
            } else if receipt["replayed"] == json!(true) {
                replayed += 1;
            }
        }
        *counts.entry(row.status.id()).or_insert(0) += 1;
        if row.status == Status::ReadyUnpublished || row.status == Status::Pending {
            staged.push(json!({
                "row": row.number,
                "term": row.term,
                "job": row.job,
                "status": row.status.id(),
                "draft_sha256": row.readiness["draft_sha256"],
            }));
        }
        let receipts = if row.receipts.is_empty() {
            carried_receipts(compiled, row)
        } else {
            row.receipts.clone()
        };
        rows.push(json!({
            "row": row.number,
            "term": row.term,
            "job": row.job,
            "planned_category": row.planned_category,
            "category": row.readiness["category"],
            "promoted": row.readiness["promoted"],
            "readiness": row.readiness,
            "publication": {
                "status": row.status.id(),
                "batch": batch,
                "head": row.head(),
                "parts": row.parts(),
                "receipts": receipts,
                "reason": row.reason,
            },
        }));
    }
    let ready = run.rows.iter().filter(|row| row.chain.is_some()).count();
    let committed = run
        .rows
        .iter()
        .filter(|row| row.status == Status::Committed)
        .count();
    let complete = !run.rows.is_empty()
        && committed == run.rows.len()
        && stop.is_none()
        && run.conflicts.is_empty();
    let conflicted = counts.get("conflicted").copied().unwrap_or(0) + run.conflicts.len();
    let archives = json!({
        "revision_before": run.revision_before,
        "revision_after": verdict.revision,
    });
    let stop_value = stop.map(|stop| {
        json!({
            "code": stop.code,
            "message": stop.message,
            "exit": stop.exit,
            "detail": stop.detail,
        })
    });
    let batches: Vec<Value> = ran
        .batches
        .iter()
        .enumerate()
        .map(|(index, state)| {
            json!({
                "batch": index + 1,
                "placement": state.batch.placement,
                "state": state.state,
                "items": state.batch.items.len(),
                "rows": state.batch.rows,
                "bytes": state.batch.bytes,
                "op_keys": state.batch.items.iter().map(|item| item["op_key"].clone()).collect::<Vec<_>>(),
                "replayed": state.replayed,
                "refusal": state.refusal,
                "message": state.message,
                "receipts": state.results.iter().map(|result| json!({
                    "address": result["result"]["address"],
                    "op_key": result["op_key"],
                    "role": match ref_role(&pstr(result, "client_ref")) {
                        Some((_, "card", _)) => "card",
                        Some(_) => "part",
                        None => "list",
                    },
                    "replayed": result["replayed"],
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    // The first row the stop names, for a caller that reads one address instead of the whole list.
    let stopped = match stop_value.as_ref() {
        Some(stop) => stop["detail"]["rows"]
            .as_array()
            .and_then(|rows| rows.first())
            .map(|row| row["row"].clone())
            .unwrap_or_else(|| stop["detail"]["row"].clone()),
        None => Value::Null,
    };
    let summary = json!({
        "schema_version": 1,
        "ok": stop.is_none_or(|stop| stop.exit == 0),
        "step": p.step,
        "claim_token": p.token,
        "book": compiled.book(),
        "plan_revision": compiled.revision,
        "plan_hash": compiled.plan_hash,
        "list": fresh.list.address,
        "list_type": fresh.list.card_type,
        "legacy": fresh.list.legacy,
        "retyped": run.retype.is_some(),
        "rows": rows.len(),
        "ready": ready,
        "committed": committed,
        "ready_unpublished": counts.get("ready_unpublished").copied().unwrap_or(0),
        "pending": counts.get("pending").copied().unwrap_or(0),
        "refused": counts.get("refused").copied().unwrap_or(0),
        "conflicted": conflicted,
        "conflicts": run.conflicts.len(),
        "cards_written": written,
        "cards_replayed": replayed,
        "batches": batches.len(),
        "complete": complete,
        "code": stop_value.as_ref().map(|stop| stop["code"].clone()),
        "stopped": stopped,
    });
    json!({
        "schema_version": 1,
        "kind": "term_cards_report",
        "step": p.step,
        "claim_token": p.token,
        "at": canon::utc_now_o(),
        "book": compiled.plan["book"],
        "plan": {
            "path": ps::join(&p.work(), "plan.json"),
            "revision": compiled.revision,
            "hash": compiled.plan_hash,
            "list": compiled.plan["list"],
        },
        "list": {
            "address": fresh.list.address,
            "type": fresh.list.card_type,
            "legacy": fresh.list.legacy,
            "title": fresh.list.title,
            "body_sha256": canon::sha256_hex(fresh.list.body.as_bytes()),
            "rows": fresh.list.rows.len(),
            "retyped": run.retype.is_some(),
            "retype": run.retype.as_ref().map(|item| json!({
                "op_key": item["op_key"],
                "arguments": item["arguments"],
            })),
            "observed": verdict.retyped,
        },
        "source": compiled.plan["source"],
        "limits": compiled.plan["limits"],
        "versions": compiled.plan["versions"],
        "archive": archives,
        "complete": complete,
        "counts": {
            "rows": rows.len(),
            "ready": ready,
            "committed": committed,
            "ready_unpublished": counts.get("ready_unpublished").copied().unwrap_or(0),
            "pending": counts.get("pending").copied().unwrap_or(0),
            "refused": counts.get("refused").copied().unwrap_or(0),
            "conflicted": conflicted,
            "conflicts": run.conflicts.len(),
            "cards_written": written,
            "cards_replayed": replayed,
            "batches": batches.len(),
        },
        "stop": stop_value,
        "conflicts": run.conflicts,
        "staged": staged,
        "batches": batches,
        "rows": rows,
        "summary": summary,
    })
}

/// The receipts an earlier plan revision recorded for one committed row, when the work folder still
/// holds them; the state is progress, never the authority.
fn carried_receipts(compiled: &Compiled, row: &RowState) -> Vec<Value> {
    if !row.is_committed() {
        return Vec::new();
    }
    let Ok(state) = local_state(compiled) else {
        return Vec::new();
    };
    let Some(entry) = state["published"]["rows"]
        .as_object()
        .and_then(|rows| rows.get(&row.number.to_string()))
        .cloned()
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(head) = entry.get("head").filter(|head| !head.is_null()) {
        out.push(json!({
            "op_key": head["op_key"],
            "address": head["address"],
            "role": "card",
            "replayed": Value::Null,
            "plan_revision": entry["plan_revision"],
        }));
    }
    for part in entry["parts"].as_array().into_iter().flatten() {
        out.push(json!({
            "op_key": part["op_key"],
            "address": part["address"],
            "role": "part",
            "replayed": Value::Null,
            "plan_revision": entry["plan_revision"],
        }));
    }
    out
}

/// The human-readable publication report.
fn report_md(report: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Term-card publication\n\n");
    out.push_str(&format!(
        "Book: {} — {}\n\n",
        report["book"]["citation_key"].as_str().unwrap_or_default(),
        report["book"]["title"].as_str().unwrap_or_default()
    ));
    out.push_str(&format!(
        "Plan revision {} · `{}` · list `{}` ({})\n\n",
        report["plan"]["revision"],
        report["plan"]["hash"].as_str().unwrap_or_default(),
        report["list"]["address"].as_str().unwrap_or_default(),
        if report["list"]["legacy"] == true {
            "legacy table, retyped metadata-only"
        } else {
            "term list"
        }
    ));
    let counts = &report["counts"];
    out.push_str(&format!(
        "{} row(s): {} ready · {} committed · {} ready-unpublished · {} pending · {} refused · {} conflicted.\n\n",
        counts["rows"],
        counts["ready"],
        counts["committed"],
        counts["ready_unpublished"],
        counts["pending"],
        counts["refused"],
        counts["conflicted"]
    ));
    out.push_str(&format!(
        "Archive revision {} → {}. {} Card(s) written, {} replayed, in {} call(s).\n\n",
        report["archive"]["revision_before"],
        report["archive"]["revision_after"],
        counts["cards_written"],
        counts["cards_replayed"],
        counts["batches"]
    ));
    if report["complete"] == true {
        out.push_str("Every row and part is committed and read back: the book unit may complete.\n\n");
    } else {
        out.push_str(
            "Not complete: the committed rows are useful work, and the book unit stays incomplete.\n\n",
        );
    }
    if let Some(stop) = report["stop"].as_object() {
        out.push_str(&format!(
            "## Stopped: {} (exit {})\n\n{}\n\n",
            stop["code"].as_str().unwrap_or_default(),
            stop["exit"],
            stop["message"].as_str().unwrap_or_default()
        ));
    }
    let conflicts = report["conflicts"].as_array().cloned().unwrap_or_default();
    if !conflicts.is_empty() {
        out.push_str("## Conflicts\n\n");
        for conflict in &conflicts {
            out.push_str(&format!(
                "- `{}` ({}): {}\n",
                conflict["address"].as_str().unwrap_or_default(),
                conflict["code"].as_str().unwrap_or_default(),
                conflict["message"].as_str().unwrap_or_default()
            ));
        }
        out.push('\n');
    }
    out.push_str("## Rows\n\n");
    out.push_str("| Row | Term | Job | Category | Status | Head / parts |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for row in report["rows"].as_array().into_iter().flatten() {
        let publication = &row["publication"];
        let mut cards: Vec<String> = Vec::new();
        if let Some(head) = publication["head"].as_str().filter(|head| !head.is_empty()) {
            cards.push(format!("`{head}`"));
        }
        for part in publication["parts"].as_array().into_iter().flatten() {
            cards.push(format!("`{}`", part.as_str().unwrap_or_default()));
        }
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            row["row"],
            row["term"].as_str().unwrap_or_default(),
            row["job"].as_str().unwrap_or_default(),
            row["category"]
                .as_str()
                .unwrap_or_else(|| row["planned_category"].as_str().unwrap_or_default()),
            publication["status"].as_str().unwrap_or_default(),
            if cards.is_empty() {
                "—".to_string()
            } else {
                cards.join(" + ")
            }
        ));
        if let Some(message) = publication["reason"]["message"].as_str() {
            out.push_str(&format!("| | | | | ↳ {message} | |\n"));
        }
    }
    out.push('\n');
    let staged = report["staged"].as_array().cloned().unwrap_or_default();
    out.push_str("## Staged, ready, not published\n\n");
    if staged.is_empty() {
        out.push_str("None.\n\n");
    } else {
        for row in &staged {
            out.push_str(&format!(
                "- row {} `{}` (job {}, {})\n",
                row["row"],
                row["term"].as_str().unwrap_or_default(),
                row["job"].as_str().unwrap_or_default(),
                row["status"].as_str().unwrap_or_default()
            ));
        }
        out.push('\n');
    }
    let batches = report["batches"].as_array().cloned().unwrap_or_default();
    out.push_str("## Calls\n\n");
    if batches.is_empty() {
        out.push_str("No identified call was sent.\n\n");
    }
    for batch in &batches {
        out.push_str(&format!(
            "- call {} ({}): {} · {} item(s) · {} bytes\n",
            batch["batch"],
            batch["placement"].as_str().unwrap_or_default(),
            batch["state"].as_str().unwrap_or_default(),
            batch["items"],
            batch["bytes"]
        ));
    }
    out.push('\n');
    out
}

// --- the archive's address order ----------------------------------------------------------------

/// The archive's own address order (`src/model.rs::address_order`), so the list's Side children are
/// aligned in the order the archive itself reads them: numbers before Side labels, and Side labels
/// in their base-26 numbering (`a` < `b` < … < `z` < `aa`).
fn address_order(a: &str, b: &str) -> std::cmp::Ordering {
    fn segments(address: &str) -> Vec<(u8, u64, &str)> {
        address
            .split(['/', '|'])
            .map(|segment| match segment.parse::<u64>() {
                Ok(number) => (0, number, ""),
                Err(_) => (1, segment.len() as u64, segment),
            })
            .collect()
    }
    segments(a).cmp(&segments(b)).then_with(|| a.cmp(b))
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn compiled(rows: usize) -> Compiled {
        let plan = json!({
            "schema_version": 1,
            "kind": "term_cards_plan",
            "book": {"citation_key": "K", "title": "The Book"},
            "list": {"address": "K/1|a", "type": "term"},
            "revision": 2,
            "plan_hash": "a".repeat(64),
            "rows": (1..=rows).map(|number| json!({"row": number, "term": format!("词{number}")})).collect::<Vec<_>>(),
        });
        Compiled {
            plan,
            work: "D:\\work\\term-cards".to_string(),
            plan_hash: "a".repeat(64),
            revision: 2,
        }
    }

    fn paper(title: &str, body: &str, role: &'static str) -> Paper {
        Paper {
            role,
            title: title.to_string(),
            body: body.to_string(),
        }
    }

    /// One row's rendered head.
    fn head(number: u64) -> Paper {
        paper(
            &format!("词{number}"),
            &format!("术语：词{number}\n来源：[[K]] §1"),
            "card",
        )
    }

    /// One row's chain: the head, then one continuation part that names it.
    fn chain_with_part(number: u64) -> Vec<Paper> {
        vec![
            head(number),
            paper(
                &format!("推导（续 2/2）：词{number}"),
                "续：[[@@term_head]] 本卡沿用该卡的符号表。",
                "part",
            ),
        ]
    }

    fn plan(number: u64, chain: Option<Vec<Paper>>) -> RowPlan {
        let ready = chain.is_some();
        RowPlan {
            number,
            term: format!("词{number}"),
            job: format!("t{number:03}"),
            planned_category: "concept".to_string(),
            readiness: json!({"row": number, "status": if ready { "ready" } else { "needs_review" }}),
            chain,
            blocked: (!ready).then(|| {
                json!({
                    "code": "not_ready",
                    "status": "needs_review",
                    "message": format!("row {number} `词{number}` is `needs_review`"),
                })
            }),
        }
    }

    /// The frozen plans of `count` rows, ready exactly where `ready` says so.
    fn plans(count: usize, ready: impl Fn(u64) -> bool) -> Vec<RowPlan> {
        (1..=count as u64)
            .map(|number| plan(number, ready(number).then(|| vec![head(number)])))
            .collect()
    }

    fn empty_scan() -> Scan {
        Scan {
            list: json!({"address": "K/1|a", "type": "term"}),
            ..Scan::default()
        }
    }

    /// The exact rendering of one row's head, as the archive stores it at `address`.
    fn add_head(scan: &mut Scan, address: &str, number: u64) {
        scan.side.push(address.to_string());
        scan.cards.insert(
            address.to_string(),
            json!({
                "address": address,
                "type": "term",
                "term": format!("词{number}"),
                "title": format!("词{number}"),
                "body": format!("术语：词{number}\n来源：[[K]] §1"),
            }),
        );
    }

    /// The exact rendering of one row's continuation part, filed under `head`.
    fn add_part(scan: &mut Scan, head: &str, address: &str, number: u64) {
        scan.chain
            .entry(head.to_string())
            .or_default()
            .push(address.to_string());
        scan.cards.insert(
            address.to_string(),
            json!({
                "address": address,
                "type": "term",
                "term": format!("词{number}"),
                "title": format!("推导（续 2/2）：词{number}"),
                "body": format!("续：[[{head}]] 本卡沿用该卡的符号表。"),
            }),
        );
    }

    fn run_of(plans: Vec<RowPlan>, scan: Scan) -> Run {
        let (rows, conflicts) = judge(&plans, &scan, &Value::Null, 2);
        Run {
            plans,
            rows,
            conflicts,
            scan,
            published: Value::Null,
            retype: None,
            revision_before: 7,
        }
    }

    /// The answer to one sent call, as `apply_batch` returns it: the head at `address`, its parts
    /// under it, and the `moved` map the placement reported.
    fn answers(items: &[Value], address: &str, moved: Value) -> Vec<Value> {
        items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let mut result = json!({
                    "address": if index == 0 {
                        address.to_string()
                    } else {
                        format!("{address}|{index}")
                    },
                });
                if index == 0 && !moved.is_null() {
                    result["moved"] = moved.clone();
                }
                json!({
                    "op_key": item["op_key"],
                    "client_ref": item["client_ref"],
                    "replayed": false,
                    "result": result,
                })
            })
            .collect()
    }

    #[test]
    fn the_archive_address_order_aligns_side_labels_by_number() {
        let mut addresses = vec!["K/1|aa", "K/1|b", "K/1|a", "K/1|z", "K/1"];
        addresses.sort_by(|a, b| address_order(a, b));
        assert_eq!(addresses, ["K/1", "K/1|a", "K/1|b", "K/1|z", "K/1|aa"]);
        assert_eq!(ref_role("r004"), Some((4, "card", 0)));
        assert_eq!(ref_role("r004-p02"), Some((4, "part", 2)));
        assert_eq!(ref_role("list"), None);
    }

    #[test]
    fn the_chain_is_the_head_then_direct_parts_that_reference_it() {
        let compiled = compiled(1);
        let chain = vec![
            paper("词1", "术语：词1\n来源：[[K]] §1", "card"),
            paper("推导（续 2/3）：第一步", "续：[[@@term_head]] 本卡沿用该卡的符号表。", "part"),
            paper("推导（续 3/3）：第二步", "续：[[@@term_head]] 本卡沿用该卡的符号表。", "part"),
        ];
        let items = chain_items(&compiled, 1, "词1", &chain, None).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["client_ref"], "r001");
        assert_eq!(items[0]["arguments"]["at"], "K/1|a");
        assert_eq!(items[0]["arguments"]["kind"], "side");
        assert_eq!(items[0]["arguments"]["type"], "term");
        assert_eq!(items[0]["arguments"]["term"], "词1");
        assert!(items[0]["arguments"].get("before").is_none());
        assert_eq!(items[1]["arguments"]["at"], "@@r001");
        assert_eq!(items[1]["arguments"]["kind"], "direct");
        assert_eq!(items[2]["arguments"]["at"], "@@r001-p01");
        assert_eq!(
            items[1]["arguments"]["body"],
            "续：[[@@r001]] 本卡沿用该卡的符号表。"
        );
        assert_eq!(
            items[0]["op_key"],
            format!("term-cards-{}/K/2/apply/r001", "a".repeat(16))
        );
        assert_eq!(
            items[2]["op_key"],
            format!("term-cards-{}/K/2/apply/r001-p02", "a".repeat(16))
        );
        // A placement files the head in a published sibling's place; only the head carries `before`.
        let placed = chain_items(&compiled, 1, "词1", &chain, Some("K/1|a|b")).unwrap();
        assert_eq!(placed[0]["arguments"]["at"], "K/1|a");
        assert_eq!(placed[0]["arguments"]["before"], "K/1|a|b");
        assert!(placed[1]["arguments"].get("before").is_none());
    }

    /// S33 (#182) acceptance (a): rows 1, 2, 4 and 5 are ready and row 3 is refused, so every ready
    /// row is published in table order and only row 3 keeps the book incomplete.
    #[test]
    fn every_ready_row_past_a_refused_one_is_published_in_table_order() {
        let compiled = compiled(5);
        let run = run_of(plans(5, |number| number != 3), empty_scan());
        assert_eq!(run.rows[2].status, Status::Refused);
        assert_eq!(run.rows[3].status, Status::ReadyUnpublished);
        assert!(run.conflicts.is_empty());

        let packing = pack(&compiled, &run);
        assert!(packing.stop.is_none());
        assert!(packing.holes.is_empty());
        assert_eq!(packing.batches.len(), 1, "one appended call carries every ready row");
        assert_eq!(packing.batches[0].rows, vec![1, 2, 4, 5]);
        assert_eq!(packing.batches[0].placement, "append");
        let refs: Vec<String> = packing.batches[0]
            .items
            .iter()
            .map(|item| pstr(item, "client_ref"))
            .collect();
        assert_eq!(refs, ["r001", "r002", "r004", "r005"]);

        // A committed call leaves every ready row committed and row 3 refused: the book unit stays
        // incomplete because a run's own publication does not complete a refused row.
        let mut committed_scan = empty_scan();
        for (address, number) in [("K/1|a|a", 1), ("K/1|a|b", 2), ("K/1|a|c", 4), ("K/1|a|d", 5)] {
            add_head(&mut committed_scan, address, number);
        }
        let (rows, conflicts) = judge(&run.plans, &committed_scan, &Value::Null, 2);
        assert!(conflicts.is_empty());
        let after = Run {
            plans: run.plans.clone(),
            rows,
            conflicts,
            scan: committed_scan,
            published: Value::Null,
            retype: None,
            revision_before: 7,
        };
        let packing = pack(&compiled, &after);
        assert!(packing.batches.is_empty() && packing.holes.is_empty());
        assert_eq!(
            after.rows.iter().filter(|row| row.status == Status::Committed).count(),
            4
        );
        assert_eq!(after.rows[2].status, Status::Refused);
        let stop = after
            .structural_stop()
            .expect("the refused row keeps the book open");
        assert_eq!(stop.code, "not_ready");
        assert_eq!(stop.exit, 0, "a refused draft is a degraded run");
        assert!(stop.message.contains("3 `词3`"), "{}", stop.message);
        assert!(!stop.message.contains("row 3"), "the line names the row, not a sentence");
    }

    /// S33 (#182) acceptance (b): a repaired row is filed between its neighbours — before the next
    /// published head, never at the end — and the rows already published are never sent again.
    #[test]
    fn a_repaired_row_is_filed_between_its_neighbours_and_the_published_rows_replay() {
        let compiled = compiled(5);
        let mut published = empty_scan();
        for (address, number) in [("K/1|a|a", 1), ("K/1|a|b", 2), ("K/1|a|c", 4), ("K/1|a|d", 5)] {
            add_head(&mut published, address, number);
        }
        let run = run_of(plans(5, |_| true), published);
        assert_eq!(run.rows[0].status, Status::Committed);
        assert_eq!(run.rows[2].status, Status::ReadyUnpublished);
        assert_eq!(run.rows[3].status, Status::Committed);
        assert_eq!(run.rows[3].head(), Some(&"K/1|a|c".to_string()));

        let packing = pack(&compiled, &run);
        assert!(packing.batches.is_empty(), "the published rows are never sent again");
        assert_eq!(packing.holes.len(), 1);
        assert_eq!(packing.holes[0].row, 3);
        assert_eq!(
            packing.holes[0].before,
            Before::Published("K/1|a|c".to_string()),
            "row 3 stands before row 4's head, never after the last row"
        );

        // The archive files row 3 in that place: it takes |c, and rows 4 and 5 move one label on.
        let first = RefCell::new(Vec::<Value>::new());
        let ran = execute(&compiled, &packing, &run, |items| {
            first.borrow_mut().push(items[0].clone());
            Ok(Reply::Done(answers(
                items,
                "K/1|a|c",
                json!({"K/1|a|c": "K/1|a|d", "K/1|a|d": "K/1|a|e"}),
            )))
        })
        .unwrap();
        assert!(ran.stop.is_none(), "{:?}", ran.stop);
        assert_eq!(first.borrow().len(), 1, "one call files the one repaired row");
        assert_eq!(first.borrow()[0]["arguments"]["before"], "K/1|a|c");
        let receipts = ran.receipts_of(3);
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0]["address"], "K/1|a|c");

        // Reading the placement back finds every row where it now stands; nothing is written again.
        let mut placed = empty_scan();
        for (address, number) in [
            ("K/1|a|a", 1),
            ("K/1|a|b", 2),
            ("K/1|a|c", 3),
            ("K/1|a|d", 4),
            ("K/1|a|e", 5),
        ] {
            add_head(&mut placed, address, number);
        }
        let (rows, conflicts) = judge(&run.plans, &placed, &Value::Null, 2);
        assert!(conflicts.is_empty());
        let after = Run {
            plans: run.plans.clone(),
            rows,
            conflicts,
            scan: placed,
            published: Value::Null,
            retype: None,
            revision_before: 7,
        };
        assert!(after.rows.iter().all(|row| row.status == Status::Committed));
        assert_eq!(after.rows[3].head(), Some(&"K/1|a|d".to_string()));
        assert_eq!(after.rows[4].head(), Some(&"K/1|a|e".to_string()));
        let packing = pack(&compiled, &after);
        assert!(
            packing.batches.is_empty() && packing.holes.is_empty(),
            "the repaired row and every shifted row replay with no new write"
        );
    }

    /// S33 (#182): two repaired rows are filed highest table position first, and the next placement
    /// names the head the previous one filed.
    #[test]
    fn two_repaired_rows_are_filed_highest_first_and_the_next_names_the_head_just_filed() {
        let compiled = compiled(5);
        let mut published = empty_scan();
        for (address, number) in [("K/1|a|a", 1), ("K/1|a|b", 2), ("K/1|a|c", 5)] {
            add_head(&mut published, address, number);
        }
        let run = run_of(plans(5, |number| number == 3 || number == 4), published);
        let packing = pack(&compiled, &run);
        let rows: Vec<u64> = packing.holes.iter().map(|hole| hole.row).collect();
        assert_eq!(rows, vec![4, 3], "the highest table position is sent first");
        assert_eq!(
            packing.holes[0].before,
            Before::Published("K/1|a|c".to_string())
        );
        assert_eq!(packing.holes[1].before, Before::Placed);

        let sent = RefCell::new(Vec::<Value>::new());
        let ran = execute(&compiled, &packing, &run, |items| {
            sent.borrow_mut().push(items[0].clone());
            Ok(Reply::Done(answers(items, "K/1|a|c", Value::Null)))
        })
        .unwrap();
        assert!(ran.stop.is_none(), "{:?}", ran.stop);
        assert_eq!(sent.borrow().len(), 2);
        assert_eq!(sent.borrow()[0]["arguments"]["before"], "K/1|a|c");
        assert_eq!(
            sent.borrow()[1]["arguments"]["before"], "K/1|a|c",
            "row 3 is filed before the head row 4 just took"
        );
    }

    /// S33 (#182) acceptance (c): a Side child that matches no planned row is a conflict, and the
    /// step writes nothing.
    #[test]
    fn a_side_child_that_matches_no_planned_row_is_a_conflict() {
        let compiled = compiled(2);
        let mut scan = empty_scan();
        scan.side.push("K/1|a|a".to_string());
        scan.cards.insert(
            "K/1|a|a".to_string(),
            json!({"address": "K/1|a|a", "type": "idea", "title": "一条笔记", "body": "这不是术语卡。"}),
        );
        let run = run_of(plans(2, |_| true), scan);
        assert_eq!(run.conflicts.len(), 1);
        assert_eq!(pstr(&run.conflicts[0], "code"), "unrelated_side");
        assert_eq!(pstr(&run.conflicts[0], "address"), "K/1|a|a");
        assert_eq!(run.rows[0].status, Status::ReadyUnpublished);
        let packing = pack(&compiled, &run);
        assert!(
            packing.batches.is_empty() && packing.holes.is_empty(),
            "a conflict writes nothing"
        );
        let stop = run.structural_stop().expect("a conflict stops publication");
        assert_eq!(stop.code, "conflicted");
        assert_eq!(stop.exit, STOP_EXIT);
    }

    /// S33 (#182) acceptance (d): a lost publication reply replays from the identity read and never
    /// creates a second head.
    #[test]
    fn a_lost_answer_replays_from_the_identity_read_without_a_second_head() {
        let compiled = compiled(1);
        let run = run_of(plans(1, |_| true), empty_scan());
        let packing = pack(&compiled, &run);
        assert_eq!(packing.batches.len(), 1);
        let ran = execute(&compiled, &packing, &run, |_| {
            Ok(Reply::Lost("the service closed the pipe".to_string()))
        })
        .unwrap();
        assert_eq!(ran.batches[0].state, "unknown");
        let stop = ran.stop.expect("a lost answer stops the run");
        assert_eq!(stop.code, "interrupted");
        assert_eq!(stop.exit, STOP_EXIT);
        assert!(stop.message.contains("run -Step apply again"), "{}", stop.message);

        // The archive did commit the head before the answer was lost; the next read finds it by its
        // exact rendering, so nothing is written again.
        let mut committed = empty_scan();
        add_head(&mut committed, "K/1|a|a", 1);
        let (rows, conflicts) = judge(&run.plans, &committed, &Value::Null, 2);
        assert!(conflicts.is_empty());
        let next = Run {
            plans: run.plans.clone(),
            rows,
            conflicts,
            scan: committed,
            published: Value::Null,
            retype: None,
            revision_before: 7,
        };
        assert_eq!(next.rows[0].status, Status::Committed);
        assert_eq!(next.rows[0].head(), Some(&"K/1|a|a".to_string()));
        let again = pack(&compiled, &next);
        assert!(
            again.batches.is_empty() && again.holes.is_empty(),
            "no second head"
        );
    }

    #[test]
    fn a_head_this_plan_published_that_changed_since_is_kept_and_never_overwritten() {
        // Live box 2026-10-26 (acemoglu2025nobel): the run's own Term-link pass added Term links to the heads it had
        // just published; the next run called all 57 conflicted and held row 15 back. A head the publication record
        // names stays exactly as it is, counts as published and holds no other row back.
        let mut scan = empty_scan();
        scan.side.push("K/1|a|a".to_string());
        scan.cards.insert(
            "K/1|a|a".to_string(),
            json!({"address": "K/1|a|a", "type": "term", "term": "词1", "title": "词1", "body": "来源：[[K]] §2"}),
        );
        let published = json!({"rows": {"1": {"plan_revision": 2, "head": {"address": "K/1|a|a"}, "parts": []}}});
        let (rows, conflicts) = judge(&plans(2, |_| true), &scan, &published, 2);
        assert!(conflicts.is_empty());
        assert_eq!(rows[0].status, Status::Committed);
        assert_eq!(pstr(&rows[0].reason.clone().unwrap(), "code"), "kept_earlier");
        assert_eq!(rows[1].status, Status::ReadyUnpublished);
        let run = Run {
            plans: plans(2, |_| true),
            rows,
            conflicts,
            scan,
            published,
            retype: None,
            revision_before: 7,
        };
        assert!(run.structural_stop().is_none_or(|stop| stop.code != "conflicted"));
        let packing = pack(&compiled(2), &run);
        assert_eq!(packing.batches.iter().flat_map(|batch| batch.rows.iter().copied()).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn a_head_accepted_in_an_earlier_revision_is_kept_and_holds_nothing_back() {
        // Live run 2026-10-26 (staging8): a Term backfill made plan revision 2 and redrafted the 44 rows revision 1
        // had published; every one was a conflict and the 16 new ready rows were held back. A head an earlier
        // revision published stays exactly as it is and counts as published; the other rows are written.
        let mut scan = empty_scan();
        scan.side.push("K/1|a|a".to_string());
        scan.cards.insert(
            "K/1|a|a".to_string(),
            json!({"address": "K/1|a|a", "type": "term", "term": "词1", "title": "词1", "body": "来源：[[K]] §2"}),
        );
        let published = json!({"rows": {"1": {"plan_revision": 1, "head": {"address": "K/1|a|a"}, "parts": []}}});
        let (rows, conflicts) = judge(&plans(2, |_| true), &scan, &published, 2);
        assert!(conflicts.is_empty());
        assert_eq!(rows[0].status, Status::Committed);
        assert_eq!(rows[0].head().map(String::as_str), Some("K/1|a|a"));
        assert_eq!(pstr(&rows[0].reason.clone().unwrap(), "code"), "kept_earlier");
        assert_eq!(rows[1].status, Status::ReadyUnpublished);
        let run = Run {
            plans: plans(2, |_| true),
            rows,
            conflicts,
            scan,
            published,
            retype: None,
            revision_before: 7,
        };
        assert!(run.structural_stop().is_none_or(|stop| stop.code != "conflicted"));
        let packing = pack(&compiled(2), &run);
        assert_eq!(packing.batches.iter().flat_map(|batch| batch.rows.iter().copied()).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn a_second_head_for_one_row_and_a_head_outside_the_list_are_conflicts() {
        // Two Side children carry row 1's term: publication never chooses between them.
        let mut scan = empty_scan();
        add_head(&mut scan, "K/1|a|a", 1);
        add_head(&mut scan, "K/1|a|b", 1);
        let (rows, conflicts) = judge(&plans(1, |_| true), &scan, &Value::Null, 2);
        assert!(conflicts.is_empty(), "the conflict belongs to the row");
        assert_eq!(rows[0].status, Status::Conflicted);
        assert_eq!(pstr(&rows[0].reason.clone().unwrap(), "code"), "duplicate_head");

        // The row's head exists in the tree, but not as an immediate Side child of the list.
        let mut misplaced = empty_scan();
        let mut head_only = empty_scan();
        add_head(&mut head_only, "K/2|a", 1);
        let stray = head_only.cards["K/2|a"].clone();
        misplaced.cards.insert("K/2|a".to_string(), stray.clone());
        misplaced.stray.push(stray);
        let (rows, conflicts) = judge(&plans(1, |_| true), &misplaced, &Value::Null, 2);
        assert!(conflicts.is_empty());
        assert_eq!(rows[0].status, Status::Conflicted);
        let reason = rows[0].reason.clone().unwrap();
        assert_eq!(pstr(&reason, "code"), "misplaced_head");
        assert_eq!(pstr(&reason, "address"), "K/2|a");
    }

    #[test]
    fn a_committed_row_with_its_part_is_identified_by_its_content() {
        let compiled = compiled(1);
        let plans = vec![plan(1, Some(chain_with_part(1)))];
        let mut scan = empty_scan();
        add_head(&mut scan, "K/1|a|a", 1);
        add_part(&mut scan, "K/1|a|a", "K/1|a|a|1", 1);
        let run = run_of(plans, scan);
        assert_eq!(run.rows[0].status, Status::Committed);
        assert_eq!(run.rows[0].head(), Some(&"K/1|a|a".to_string()));
        assert_eq!(run.rows[0].parts(), ["K/1|a|a|1"]);
        assert!(pack(&compiled, &run).batches.is_empty());
    }

    #[test]
    fn the_typed_statuses_are_the_reports_vocabulary() {
        let ids: Vec<&str> = [
            Status::Committed,
            Status::ReadyUnpublished,
            Status::Pending,
            Status::Refused,
            Status::Conflicted,
        ]
        .iter()
        .map(|status| status.id())
        .collect();
        assert_eq!(
            ids,
            ["committed", "ready_unpublished", "pending", "refused", "conflicted"]
        );
    }

    #[test]
    fn chains_are_packed_into_calls_of_at_most_sixty_four_items() {
        let compiled = compiled(200);
        let run = run_of(plans(200, |_| true), empty_scan());
        let packing = pack(&compiled, &run);
        assert!(packing.stop.is_none());
        assert_eq!(packing.batches.len(), 4);
        assert!(
            packing
                .batches
                .iter()
                .all(|batch| batch.items.len() <= CHUNK_ITEMS)
        );
        assert_eq!(
            packing
                .batches
                .iter()
                .map(|batch| batch.items.len())
                .sum::<usize>(),
            200
        );
        assert_eq!(packing.batches[0].rows, (1..=64).collect::<Vec<u64>>());
    }

    #[test]
    fn a_chain_that_does_not_fit_one_call_is_refused_and_the_earlier_rows_stay_packed() {
        let compiled = compiled(2);
        let mut huge = vec![head(2)];
        for index in 1..70 {
            huge.push(paper(
                &format!("续 {index}"),
                "续：[[@@term_head]] 沿用。",
                "part",
            ));
        }
        let plans = vec![plan(1, Some(vec![head(1)])), plan(2, Some(huge))];
        let run = run_of(plans, empty_scan());
        let packing = pack(&compiled, &run);
        let stop = packing.stop.expect("a 70-Card chain is refused");
        assert_eq!(stop.code, "chain_too_large");
        assert_eq!(stop.exit, STOP_EXIT);
        assert_eq!(packing.batches.len(), 1, "the rows before it stay packed");
        assert_eq!(packing.batches[0].rows, vec![1]);
    }

    #[test]
    fn the_retype_is_sent_as_its_own_call_before_the_heads_it_makes_legal() {
        let compiled = compiled(1);
        let mut run = run_of(plans(1, |_| true), empty_scan());
        run.retype = Some(json!({
            "op_key": "term-cards-x/K/2/apply/list",
            "client_ref": "list",
            "tool": "edit_card",
            "arguments": {"at": "K/1|a", "type": "term", "base_fingerprint": "b".repeat(64)},
        }));
        let packing = pack(&compiled, &run);
        assert_eq!(packing.batches.len(), 1);
        assert!(
            packing.batches[0]
                .items
                .iter()
                .all(|item| pstr(item, "client_ref") != "list"),
            "the retype is no longer prepended to a head call"
        );
        let calls = RefCell::new(Vec::<Vec<Value>>::new());
        let ran = execute(&compiled, &packing, &run, |items| {
            calls.borrow_mut().push(items.to_vec());
            Ok(Reply::Done(answers(items, "K/1|a|a", Value::Null)))
        })
        .unwrap();
        assert_eq!(calls.borrow().len(), 2);
        assert_eq!(calls.borrow()[0][0]["client_ref"], "list");
        assert_eq!(calls.borrow()[1][0]["client_ref"], "r001");
        assert_eq!(ran.batches[0].batch.placement, "list");
        assert_eq!(ran.batches[1].batch.placement, "append");
    }

    #[test]
    fn a_refused_call_stops_with_the_archive_answer_and_keeps_earlier_receipts() {
        let compiled = compiled(2);
        let run = run_of(plans(2, |_| true), empty_scan());
        let packing = pack(&compiled, &run);
        assert_eq!(packing.batches.len(), 1);
        // A Lock keeps exit 2; a blocking Card check and a stale base are content stops.
        let locked = json!({"error": "card_locked", "message": "card `K/1|a` is inside locked tree `K`"});
        let ran = execute(&compiled, &packing, &run, |_| {
            Ok(Reply::Refused(locked.clone()))
        })
        .unwrap();
        let stop = ran.stop.unwrap();
        assert_eq!(stop.code, "card_locked");
        assert_eq!(stop.exit, lockcheck::LOCKED_EXIT);
        assert_eq!(ran.batches[0].state, "refused");
        let check = json!({"error": "card_check", "blocking": 1, "faults": ["K/1 R3 甲"]});
        let ran = execute(&compiled, &packing, &run, |_| {
            Ok(Reply::Refused(check.clone()))
        })
        .unwrap();
        let stop = ran.stop.unwrap();
        assert_eq!(stop.code, "card_check");
        assert_eq!(stop.exit, STOP_EXIT);
    }

    #[test]
    fn an_answered_call_must_name_every_item_in_order() {
        let compiled = compiled(2);
        let run = run_of(plans(2, |_| true), empty_scan());
        let packing = pack(&compiled, &run);
        let error = execute(&compiled, &packing, &run, |_| Ok(Reply::Done(vec![])))
            .unwrap_err()
            .to_string();
        assert!(error.contains("0 of 2"), "{error}");
        let error = execute(&compiled, &packing, &run, |items| {
            let answers: Vec<Value> = items
                .iter()
                .rev()
                .map(|item| json!({"op_key": item["op_key"], "client_ref": item["client_ref"], "replayed": false, "result": {"address": "K/1|a|a"}}))
                .collect();
            Ok(Reply::Done(answers))
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("identity/order mismatch"), "{error}");
    }

    #[test]
    fn a_written_row_the_archive_does_not_hold_is_a_divergence_not_a_success() {
        let compiled = compiled(1);
        let run = run_of(plans(1, |_| true), empty_scan());
        let packing = pack(&compiled, &run);
        let ran = execute(&compiled, &packing, &run, |items| {
            Ok(Reply::Done(answers(items, "K/1|a|a", Value::Null)))
        })
        .unwrap();
        // The readback does not find the row: the archive holds something else at the head.
        let mut scan = empty_scan();
        scan.side.push("K/1|a|a".to_string());
        scan.cards.insert(
            "K/1|a|a".to_string(),
            json!({"address": "K/1|a|a", "type": "term", "term": "词1", "title": "词1", "body": "别的内容"}),
        );
        let verdict = Verdict {
            rows: {
                let (rows, _) = judge(&run.plans, &scan, &Value::Null, 2);
                rows
            },
            conflicts: Vec::new(),
            revision: 8,
            retyped: None,
            diverged: vec![1],
            stop: divergence_stop(&json!({"rows": [1]})),
        };
        let mut run = run;
        run.settle(&ran, &verdict);
        assert_eq!(run.rows[0].status, Status::Conflicted);
        assert_eq!(
            pstr(&run.rows[0].reason.clone().unwrap(), "code"),
            "output_diverged"
        );
        let stop = verdict.stop.expect("a divergence stops the run");
        assert_eq!(stop.code, "output_diverged");
        assert_eq!(stop.exit, STOP_EXIT);
        assert!(stop.message.contains("1 committed row(s)"), "{}", stop.message);
    }
}
