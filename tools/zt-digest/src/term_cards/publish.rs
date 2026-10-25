//! `term-cards -Step apply|report`: publication of one book's Term Cards (design §12.5–§12.6,
//! S21 / #165).
//!
//! `apply` publishes only the longest **ready ordered row prefix**: for each ready row in table
//! order its head and its complete continuation chain, as Side children of the Term list and Direct
//! successors of the head. A row that is not ready stops the prefix; every later ready draft stays
//! staged, and no placeholder head, reserved address, insertion, move or deletion is ever written
//! (R0337, R0340, R0344–R0350).
//!
//! Before one Card is written the frozen plan is revalidated against a fresh coherent read: the
//! plan content hash (list rows/title, staged source, notation, limits, compiler/checker/rule and
//! instruction versions) must still describe the archive, or the step answers one typed
//! `plan_stale` line and writes nothing (R0339, R0342). The list's own *kind* is not list content:
//! a legacy `术语表` Idea Card is retyped to `term` metadata-only in the first batch, with its
//! authored title, body, Links, address and row identity kept byte for byte (R0336).
//!
//! The existing publication is then read, not assumed: the list's immediate Side children are the
//! slots of rows 1..n in address order. A slot whose Term Card carries exactly the plan's rendered
//! head — and whose Direct chain carries exactly its rendered parts — is a committed row; its
//! addresses are reused and nothing is written again, so a lost publication reply cannot create a
//! second head (R0341). A slot holding another Card, or one whose authored content differs from the
//! plan's rendering, is a conflict: the step stops instead of inserting, moving or editing an
//! accepted head (R0339, R0342).
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
//! revision is a new operation (R0341). Every committed Card is read back and compared with the
//! plan's rendering; a divergence is a typed stop, never a silent success.
//!
//! Both steps write `<Staging>/term-cards/report.json` and `report.md`, the publication coverage per
//! row (R0344–R0350): the source row and exact term, the math/concept classification and whether
//! promoted, the writer job, draft/check/review/fix facts, the row's own status (`committed`,
//! `ready_unpublished`, `pending`, `refused`, `conflicted`), the head/part addresses and receipt
//! keys, and why publication stopped plus which later ready drafts remain staged. `report` is the
//! same read without any write: the read-only audit an incomplete stage still runs (R0351). A book
//! unit is done only when every required row and part is verified; a partial prefix is useful
//! committed work that leaves the book incomplete (R0343).

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

/// `-Step apply`: revalidate the frozen plan, publish the ready row prefix, verify it and report.
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
    let mut ran = execute(&packing.batches, |items| send_batch(&mut zt, items))?;
    if ran.stop.is_none() {
        ran.stop = packing.stop;
    }
    let verified = verify_written(&mut zt, &compiled, &run, &ran)?;
    if ran.stop.is_none() {
        ran.stop = verified.stop.clone();
    }
    run.settle(&ran, &verified.diverged_rows);
    // The publication record is derived progress for the next run and the next plan revision
    // (R0341, R0349); it is never the authority for what the archive holds.
    save_published(&compiled, &run)?;
    let stop = final_stop(&run, &ran);
    let report = build_report(p, &compiled, &fresh, &run, &ran, &verified, stop.as_ref());
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
    let mut run = prepare(&compiled, &fresh, &mut zt)?;
    let ran = Ran::default();
    let verified = Verified::default();
    run.settle(&ran, &[]);
    let stop = final_stop(&run, &ran);
    let report = build_report(p, &compiled, &fresh, &run, &ran, &verified, stop.as_ref());
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
/// structure's own first unresolved row (a conflict, an unready draft, a divergence).
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
    /// Ready, and no Card exists at its slot yet: its draft stays staged for the next run.
    ReadyUnpublished,
    /// A batch that carried the row got no answer, so its commit cannot be claimed.
    Pending,
    /// The row's content is not ready (or the archive refused its write): it cannot be published.
    Refused,
    /// The slot holds a Card this plan did not publish, or its authored content changed.
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

/// What the archive holds at one row's structural slot.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Slot {
    /// No Card at this slot yet: publication has produced nothing beyond it.
    Absent,
    /// The head and its continuation parts, in address order.
    Committed { head: String, parts: Vec<String> },
}

/// One frozen row, its rendered chain and what the archive holds for it.
struct RowState {
    number: u64,
    term: String,
    job: String,
    planned_category: String,
    /// The S19 readiness entry, carried verbatim into the report (R0347).
    readiness: Value,
    /// The rendered chain when the row is ready; `None` when its content is not ready.
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
/// child's Direct chain of continuation parts, and the authored facts of every one of them.
#[derive(Default)]
struct Scan {
    /// The list's own text read, for the retype's base fingerprint.
    list: Value,
    side: Vec<String>,
    chain: BTreeMap<String, Vec<String>>,
    cards: BTreeMap<String, Value>,
}

/// Everything the two steps decide before a write: the rows, the archive's current publication,
/// the legacy list's retype item, and the archive revision the read saw.
struct Run {
    rows: Vec<RowState>,
    scan: Scan,
    retype: Option<Value>,
    revision_before: u64,
}

impl Run {
    /// The structure's own stop: a conflict, or the first row that is not committed. An unready
    /// draft is a degraded run (exit 0); every other stop needs the operator (exit 1).
    fn structural_stop(&self) -> Option<Stop> {
        if let Some(row) = self.rows.iter().find(|row| row.status == Status::Conflicted) {
            let reason = row.reason.clone().unwrap_or(Value::Null);
            return Some(Stop {
                code: "conflicted",
                message: pstr(&reason, "message"),
                detail: json!({"row": row.number, "term": row.term, "reason": reason}),
                exit: STOP_EXIT,
            });
        }
        let row = self.rows.iter().find(|row| row.status != Status::Committed)?;
        let reason = row.reason.clone().unwrap_or(Value::Null);
        // A prefix that simply has not published every row yet is degraded, not failed: the report
        // and the workflow read `complete`, and the stage continues to its audit (R0351).
        let (code, exit) = match pstr(&reason, "code").as_str() {
            "not_ready" => ("not_ready", 0),
            "ready_unpublished" => ("ready_unpublished", 0),
            _ => ("refused", STOP_EXIT),
        };
        Some(Stop {
            code,
            message: pstr(&reason, "message"),
            detail: json!({
                "row": row.number,
                "term": row.term,
                "status": row.status.id(),
                "reason": reason,
            }),
            exit,
        })
    }

    /// Fold the batch results into the rows: a settled batch's receipts prove the row's Cards
    /// exist; an unknown batch leaves it pending; a refused one names the archive's answer.
    fn settle(&mut self, ran: &Ran, diverged: &[u64]) {
        for row in &mut self.rows {
            let Some(batch) = ran.rows_of(row.number) else {
                continue;
            };
            match batch.state {
                "committed" | "replayed" => {
                    let mut receipts = Vec::new();
                    let mut head = None;
                    let mut parts = Vec::new();
                    for (item, result) in batch.batch.items.iter().zip(&batch.results) {
                        let Some((number, role, _)) = ref_role(&pstr(item, "client_ref")) else {
                            continue;
                        };
                        if number != row.number {
                            continue;
                        }
                        let address = pstr(&result["result"], "address");
                        receipts.push(json!({
                            "op_key": item["op_key"],
                            "address": address,
                            "role": if role == "card" { "card" } else { "part" },
                            "replayed": result["replayed"],
                        }));
                        if role == "card" {
                            head = Some(address);
                        } else {
                            parts.push(address);
                        }
                    }
                    if let Some(head) = head {
                        row.slot = Slot::Committed { head, parts };
                    }
                    row.receipts = receipts;
                    if diverged.contains(&row.number) {
                        row.status = Status::Conflicted;
                        row.reason = Some(json!({
                            "code": "output_diverged",
                            "message": "the committed Card no longer carries the plan's rendering",
                        }));
                    } else {
                        row.status = Status::Committed;
                        row.reason = Some(json!({"code": "committed"}));
                    }
                }
                "unknown" => {
                    row.status = Status::Pending;
                    row.reason = Some(json!({"code": "interrupted", "message": batch.message}));
                }
                "refused" => {
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
                _ => {}
            }
        }
    }
}

/// Read the plan's rows and the archive's publication; judge what is already committed.
fn prepare(compiled: &Compiled, fresh: &Fresh, zt: &mut Zt) -> Result<Run> {
    let state = local_state(compiled)?;
    let checks = current_checks(compiled)?;
    let readiness = readiness_of(compiled, &checks, &state, &[]);
    let rows = row_plans(compiled, &readiness, &checks)?;
    let list_address = fresh.list.address.clone();
    let scan = scan_list(zt, &fresh.tree, &list_address)?;
    let revision_before = fresh.tree["archive_revision"]
        .as_u64()
        .context("the tree read carries no archive revision")?;
    let rows = judge(rows, &scan, &state["published"]);
    let retype = retype_item(compiled, &fresh.list, &scan)?;
    Ok(Run {
        rows,
        scan,
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
) -> Result<Vec<RowState>> {
    let limits = compiled.checker_limits()?;
    let rows = compiled.row_terms();
    let book = compiled.book();
    let mut plans = Vec::new();
    for entry in readiness["terms"].as_array().into_iter().flatten() {
        let number = entry["row"].as_u64().unwrap_or(0);
        let term = pstr(entry, "term");
        let job = pstr(entry, "job");
        let status = pstr(entry, "status");
        let mut row = RowState {
            number,
            term: term.clone(),
            job: job.clone(),
            planned_category: pstr(entry, "planned_category"),
            readiness: entry.clone(),
            chain: None,
            slot: Slot::Absent,
            status: Status::Refused,
            reason: None,
            receipts: Vec::new(),
        };
        if status != "ready" {
            row.reason = Some(json!({
                "code": "not_ready",
                "status": status,
                "message": format!(
                    "row {number} `{term}` is `{status}`; its later ready drafts stay staged"
                ),
            }));
            plans.push(row);
            continue;
        }
        match chain_of(compiled, &limits, &rows, checks, &job, number, &term, &book) {
            Ok(chain) => {
                row.chain = Some(chain);
                row.status = Status::ReadyUnpublished;
                row.reason = Some(json!({
                    "code": "ready_unpublished",
                    "message": format!("row {number} `{term}` is ready and not published yet"),
                }));
            }
            Err(reason) => row.reason = Some(reason),
        }
        plans.push(row);
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
/// the authored facts of exactly those Cards in bounded `get_cards` reads.
fn scan_list(zt: &mut Zt, tree: &Value, list: &str) -> Result<Scan> {
    let cards = tree["cards"].as_array().cloned().unwrap_or_default();
    let mut by_parent: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for card in cards {
        by_parent.entry(pstr(&card, "parent")).or_default().push(card);
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
    let read = zt.cards(&addresses, "text")?;
    let mut facts = BTreeMap::new();
    for card in &read.cards {
        facts.insert(pstr(card, "address"), card.clone());
    }
    let list_card = facts.get(list).cloned().unwrap_or(Value::Null);
    Ok(Scan {
        list: list_card,
        side,
        chain,
        cards: facts,
    })
}

// --- the structural judgement -------------------------------------------------------------------

/// Align every row to the list's Side children in row order and decide what is already committed
/// (R0339, R0340, R0341, R0348). A slot that does not hold the expected head stops the alignment:
/// publication never inserts before an existing accepted head, so every later row keeps its draft.
fn judge(rows: Vec<RowState>, scan: &Scan, published: &Value) -> Vec<RowState> {
    let mut out: Vec<RowState> = Vec::new();
    let mut slot = 0usize;
    let mut broken = false;
    for mut row in rows {
        if broken {
            row.reason = Some(json!({
                "code": "unaligned",
                "message": format!(
                    "row {} `{}` follows a publication conflict; its draft stays staged",
                    row.number, row.term
                ),
            }));
            out.push(row);
            continue;
        }
        let Some(head) = scan.side.get(slot).cloned() else {
            // No Card at this slot: the publication prefix ends here. Later rows cannot have a slot
            // either, because nothing is ever published after a gap.
            out.push(row);
            continue;
        };
        let observed = scan.cards.get(&head).cloned().unwrap_or(Value::Null);
        let same_name = pstr(&observed, "type") == "term" && pstr(&observed, "term") == row.term;
        if !same_name {
            row.status = Status::Conflicted;
            row.reason = Some(json!({
                "code": "unrelated_side",
                "kind": "unrelated_side",
                "address": head,
                "observed": {
                    "type": pstr(&observed, "type"),
                    "term": observed["term"].clone(),
                    "title": pstr(&observed, "title"),
                },
                "message": format!(
                    "the list's Side child `{head}` is not row {} `{}`; publication does not insert, move or delete it",
                    row.number, row.term
                ),
            }));
            broken = true;
            out.push(row);
            continue;
        }
        let parts = scan.chain.get(&head).cloned().unwrap_or_default();
        let Some(chain) = row.chain.as_ref() else {
            // The row's draft is not ready, but its earlier accepted head stays exactly as it is.
            row.slot = Slot::Committed {
                head: head.clone(),
                parts: parts.clone(),
            };
            row.reason = Some(json!({
                "code": "not_ready",
                "status": pstr(&row.readiness, "status"),
                "address": head,
                "parts": parts,
                "message": format!(
                    "row {} `{}` is not ready; its earlier accepted Card `{head}` stays in place and is never edited from a new draft",
                    row.number, row.term
                ),
            }));
            slot += 1;
            out.push(row);
            continue;
        };
        let expected = expected_chain(chain, &row.term, &head);
        let found = chain_contents(scan, &head, &parts);
        if found != expected {
            let (kind, message) = conflict_kind(&row, &head, &parts, chain, published);
            row.status = Status::Conflicted;
            row.reason = Some(json!({
                "code": kind,
                "kind": kind,
                "address": head,
                "expected": expected.iter().map(|(role, sha)| json!({"role": role, "sha256": sha})).collect::<Vec<_>>(),
                "observed": found.iter().map(|(role, sha)| json!({"role": role, "sha256": sha})).collect::<Vec<_>>(),
                "message": message,
            }));
            broken = true;
            out.push(row);
            continue;
        }
        row.slot = Slot::Committed {
            head: head.clone(),
            parts,
        };
        row.status = Status::Committed;
        row.reason = Some(json!({"code": "committed"}));
        slot += 1;
        out.push(row);
    }
    out
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
}

/// The packed publication, and the row that stopped further packing.
struct Packing {
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

/// Pack the ready chains after the committed prefix into identified batches: several complete
/// chains in one call, at most 64 items and 1 MiB each, a whole chain never split. The first batch
/// may also carry the list's retype (R0337, R0338).
fn pack(compiled: &Compiled, run: &Run) -> Packing {
    let mut batches: Vec<Batch> = Vec::new();
    let mut items: Vec<Value> = Vec::new();
    let mut rows: Vec<u64> = Vec::new();
    let mut bytes = 0usize;
    let mut reserved_items = run.retype.is_some() as usize;
    let mut reserved_bytes = run
        .retype
        .as_ref()
        .map(|item| authored_bytes(&item["arguments"]))
        .unwrap_or(0);
    // Only the rows at and after the first empty slot are published, and only while they are ready:
    // the first row whose draft is not ready stops the prefix, and every later draft stays staged
    // (R0340). A conflict stops everything before the first batch.
    if run.rows.iter().any(|row| row.status == Status::Conflicted) {
        return Packing {
            batches,
            stop: None,
        };
    }
    let Some(start) = run.rows.iter().position(|row| !row.is_committed()) else {
        return Packing {
            batches,
            stop: None,
        };
    };
    for row in &run.rows[start..] {
        if row.is_committed() || row.status != Status::ReadyUnpublished {
            break;
        }
        let Some(chain) = row.chain.as_ref() else {
            break;
        };
        let chain_items = match chain_items(compiled, row, chain) {
            Ok(chain_items) => chain_items,
            Err(blocked) => {
                if !items.is_empty() {
                    batches.push(Batch {
                        items: std::mem::take(&mut items),
                        rows: std::mem::take(&mut rows),
                        bytes,
                    });
                }
                return Packing {
                    batches,
                    stop: Some(blocked.stop()),
                };
            }
        };
        let chain_bytes: usize = chain_items
            .iter()
            .map(|item| authored_bytes(&item["arguments"]))
            .sum();
        if chain_items.len() > CHUNK_ITEMS || chain_bytes > CHUNK_BYTES {
            if !items.is_empty() {
                batches.push(Batch {
                    items: std::mem::take(&mut items),
                    rows: std::mem::take(&mut rows),
                    bytes,
                });
            }
            return Packing {
                batches,
                stop: Some(
                    Blocked {
                        code: "chain_too_large",
                        message: format!(
                            "row {} `{}` renders {} Card(s) and {chain_bytes} authored bytes; one identified call carries at most {CHUNK_ITEMS} Cards and {CHUNK_BYTES} bytes, and a chain is never split: split the content and run the check again",
                            row.number,
                            row.term,
                            chain_items.len()
                        ),
                        row: row.number,
                    }
                    .stop(),
                ),
            };
        }
        if !items.is_empty()
            && (items.len() + chain_items.len() + reserved_items > CHUNK_ITEMS
                || bytes + chain_bytes + reserved_bytes > CHUNK_BYTES)
        {
            batches.push(Batch {
                items: std::mem::take(&mut items),
                rows: std::mem::take(&mut rows),
                bytes,
            });
            bytes = 0;
            reserved_items = 0;
            reserved_bytes = 0;
        }
        items.extend(chain_items);
        bytes += chain_bytes;
        rows.push(row.number);
    }
    if !items.is_empty() {
        batches.push(Batch {
            items,
            rows,
            bytes,
        });
    }
    // The retype belongs to the first batch, before the heads it makes legal.
    if let Some(retype) = run.retype.as_ref()
        && let Some(first) = batches.first_mut()
    {
        first.items.insert(0, retype.clone());
        first.bytes += authored_bytes(&retype["arguments"]);
    }
    Packing {
        batches,
        stop: None,
    }
}

/// One row's items: its head as a Side child of the list, then each part as the next Direct
/// successor of the previous Card, so a part references an earlier item instead of an address
/// (R0338). The keys are stable per run, book, plan revision, step and row (R0341).
fn chain_items(
    compiled: &Compiled,
    row: &RowState,
    chain: &[Paper],
) -> std::result::Result<Vec<Value>, Blocked> {
    let blocked = |message: String| Blocked {
        code: "unpublishable",
        message,
        row: row.number,
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
        let head_ref = format!("r{:03}", row.number);
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
        items.push(json!({
            "op_key": format!("{run}/{unit}/{revision}/apply/{key}"),
            "client_ref": client_ref,
            "tool": "create_successor",
            "arguments": {
                "at": at,
                "kind": kind,
                "type": "term",
                "term": row.term,
                "title": paper.title,
                "body": body,
            },
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
    fn settled(&self) -> impl Iterator<Item = &BatchState> {
        self.batches
            .iter()
            .filter(|batch| batch.state == "committed" || batch.state == "replayed")
    }

    /// The batch that carried one row, when a batch carried it.
    fn rows_of(&self, row: u64) -> Option<&BatchState> {
        self.batches
            .iter()
            .find(|batch| batch.batch.rows.contains(&row))
    }

    /// The receipt of one row's head, from the batch that carried the row.
    fn head_receipt(&self, row: u64) -> Option<String> {
        let batch = self.rows_of(row)?;
        for (item, result) in batch.batch.items.iter().zip(&batch.results) {
            if let Some((number, "card", _)) = ref_role(&pstr(item, "client_ref"))
                && number == row
            {
                return Some(pstr(&result["result"], "address"));
            }
        }
        None
    }
}

fn pending_state(batch: &Batch) -> BatchState {
    BatchState {
        batch: batch.clone(),
        state: "pending",
        replayed: 0,
        results: Vec::new(),
        refusal: None,
        message: String::new(),
    }
}

/// Send every batch in row order. A committed batch's receipts stay; a refused batch stops the run
/// with the archive's typed answer; a lost answer stops it with the batch unknown, because its
/// commit cannot be claimed. Later batches stay pending (R0341).
fn execute<F>(batches: &[Batch], mut send: F) -> Result<Ran>
where
    F: FnMut(&[Value]) -> Result<Reply>,
{
    let mut states = Vec::new();
    let mut stop: Option<Stop> = None;
    for (index, batch) in batches.iter().enumerate() {
        if stop.is_some() {
            states.push(pending_state(batch));
            continue;
        }
        match send(&batch.items)? {
            Reply::Done(results) => states.push(done_state(batch, results)?),
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
                states.push(state);
                stop = Some(Stop {
                    code: stop_code,
                    message,
                    detail: refusal,
                    exit,
                });
            }
            Reply::Lost(message) => {
                let mut state = pending_state(batch);
                state.state = "unknown";
                state.message = message.clone();
                states.push(state);
                stop = Some(Stop {
                    code: "interrupted",
                    message: format!(
                        "batch {} got no answer ({message}); it may or may not have committed: run -Step apply again to verify its Cards before publishing more",
                        index + 1
                    ),
                    detail: json!({"batch": index, "rows": batch.rows, "reason": message}),
                    exit: STOP_EXIT,
                });
            }
        }
    }
    Ok(Ran {
        batches: states,
        stop,
    })
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

/// What the readback observed: the archive revision after the writes, the retype's effect, the
/// rows whose committed content diverged from the plan, and the stop that divergence is.
#[derive(Default)]
struct Verified {
    revision: Option<u64>,
    retyped: Option<Value>,
    diverged_rows: Vec<u64>,
    stop: Option<Stop>,
}

/// Read every Card this run committed and compare it with the plan's rendering in the row's own
/// head address; read a retyped list and prove its authored title and body did not change. A
/// divergence is a typed stop, never a silent success (S21-AC01, S21-AC07).
fn verify_written(zt: &mut Zt, compiled: &Compiled, run: &Run, ran: &Ran) -> Result<Verified> {
    let mut verified = Verified::default();
    // A stop does not exempt the batches that did settle: their Cards are committed work, and the
    // report must say whether the archive carries the plan's rendering or the step's end.
    let settled: Vec<&BatchState> = ran.settled().collect();
    if settled.is_empty() {
        return Ok(verified);
    }
    let mut addresses: Vec<String> = Vec::new();
    let mut retyped_in: Option<&str> = None;
    for batch in &settled {
        for (item, result) in batch.batch.items.iter().zip(&batch.results) {
            if ref_role(&pstr(item, "client_ref")).is_none() {
                retyped_in = item["op_key"].as_str();
                continue;
            }
            let address = pstr(&result["result"], "address");
            if !addresses.contains(&address) {
                addresses.push(address);
            }
        }
    }
    let list = pstr(&run.scan.list, "address");
    if !list.is_empty() && !addresses.contains(&list) {
        addresses.push(list.clone());
    }
    let observed = zt.cards(&addresses, "text")?;
    verified.revision = Some(observed.archive_revision);
    let mut by_address: BTreeMap<String, &Value> = BTreeMap::new();
    for card in &observed.cards {
        by_address.insert(pstr(card, "address"), card);
    }
    let mut diverged: Vec<Value> = Vec::new();
    for batch in &settled {
        for (item, result) in batch.batch.items.iter().zip(&batch.results) {
            let Some((number, role, index)) = ref_role(&pstr(item, "client_ref")) else {
                continue;
            };
            let address = pstr(&result["result"], "address");
            let Some(row) = run.rows.iter().find(|row| row.number == number) else {
                continue;
            };
            let Some(chain) = row.chain.as_ref() else {
                continue;
            };
            let Some(paper) = chain.get(index).filter(|paper| paper.role == role) else {
                continue;
            };
            // The head's own address; a part resolves its `@@term_head` to that same address.
            let head = if role == "card" {
                address.clone()
            } else {
                ran.head_receipt(number).unwrap_or_default()
            };
            let expected = content_hash(&paper.title, &published_body(paper, &head), Some(&row.term));
            match by_address.get(&address) {
                Some(card) if observed_hash(card) == expected => {}
                Some(card) => {
                    diverged.push(json!({
                        "address": address,
                        "role": role,
                        "observed": observed_hash(card),
                        "planned": expected,
                    }));
                    if !verified.diverged_rows.contains(&number) {
                        verified.diverged_rows.push(number);
                    }
                }
                None => {
                    diverged.push(json!({"address": address, "role": role, "observed": Value::Null, "planned": expected}));
                    if !verified.diverged_rows.contains(&number) {
                        verified.diverged_rows.push(number);
                    }
                }
            }
        }
    }
    // A retyped list keeps its authored title and body byte for byte and becomes a Term Card.
    if retyped_in.is_some()
        && let Some(card) = by_address.get(&list)
    {
        let planned = &compiled.plan["list"];
        let title_sha = canon::sha256_hex(pstr(card, "title").as_bytes());
        let body_sha = canon::sha256_hex(pstr(card, "body").as_bytes());
        let retyped = json!({
            "address": list,
            "from": planned["type"],
            "to": pstr(card, "type"),
            "title_sha256": title_sha,
            "body_sha256": body_sha,
        });
        if title_sha != pstr(planned, "title_sha256") || body_sha != pstr(planned, "body_sha256") {
            diverged.push(json!({
                "address": list,
                "role": "list",
                "observed": {"title_sha256": title_sha, "body_sha256": body_sha},
                "planned": {"title_sha256": planned["title_sha256"], "body_sha256": planned["body_sha256"]},
                "message": "the retype changed the list's authored title or body",
            }));
        }
        if pstr(card, "type") != "term" {
            diverged.push(json!({
                "address": list,
                "role": "list",
                "observed": pstr(card, "type"),
                "planned": "term",
                "message": "the legacy list was not retyped",
            }));
        }
        verified.retyped = Some(retyped);
    }
    if !diverged.is_empty() {
        verified.stop = Some(Stop {
            code: "output_diverged",
            message: format!(
                "{} committed Card(s) do not carry the plan's rendering; keep them and replan",
                diverged.len()
            ),
            detail: json!({"diverged": diverged}),
            exit: STOP_EXIT,
        });
    }
    Ok(verified)
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
    verified: &Verified,
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
    let ready_prefix = run
        .rows
        .iter()
        .take_while(|row| row.chain.is_some())
        .count();
    let committed = run
        .rows
        .iter()
        .filter(|row| row.status == Status::Committed)
        .count();
    let complete = !run.rows.is_empty()
        && committed == run.rows.len()
        && stop.is_none();
    let archives = json!({
        "revision_before": run.revision_before,
        "revision_after": verified.revision.unwrap_or(run.revision_before),
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
    let summary = json!({
        "schema_version": 1,
        "ok": stop.is_none_or(|stop| stop.exit == 0),
        "step": p.step,
        "book": compiled.book(),
        "plan_revision": compiled.revision,
        "plan_hash": compiled.plan_hash,
        "list": fresh.list.address,
        "list_type": fresh.list.card_type,
        "legacy": fresh.list.legacy,
        "retyped": run.retype.is_some(),
        "rows": rows.len(),
        "ready": ready,
        "ready_prefix": ready_prefix,
        "committed": committed,
        "ready_unpublished": counts.get("ready_unpublished").copied().unwrap_or(0),
        "pending": counts.get("pending").copied().unwrap_or(0),
        "refused": counts.get("refused").copied().unwrap_or(0),
        "conflicted": counts.get("conflicted").copied().unwrap_or(0),
        "cards_written": written,
        "cards_replayed": replayed,
        "batches": batches.len(),
        "complete": complete,
        "code": stop_value.as_ref().map(|stop| stop["code"].clone()),
        "stopped": stop_value.as_ref().map(|stop| stop["detail"]["row"].clone()),
    });
    json!({
        "schema_version": 1,
        "kind": "term_cards_report",
        "step": p.step,
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
            "observed": verified.retyped,
        },
        "source": compiled.plan["source"],
        "limits": compiled.plan["limits"],
        "versions": compiled.plan["versions"],
        "archive": archives,
        "complete": complete,
        "counts": {
            "rows": rows.len(),
            "ready": ready,
            "ready_prefix": ready_prefix,
            "committed": committed,
            "ready_unpublished": counts.get("ready_unpublished").copied().unwrap_or(0),
            "pending": counts.get("pending").copied().unwrap_or(0),
            "refused": counts.get("refused").copied().unwrap_or(0),
            "conflicted": counts.get("conflicted").copied().unwrap_or(0),
            "cards_written": written,
            "cards_replayed": replayed,
            "batches": batches.len(),
        },
        "stop": stop_value,
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
        "{} row(s): {} ready (prefix {}) · {} committed · {} ready-unpublished · {} pending · {} refused · {} conflicted.\n\n",
        counts["rows"],
        counts["ready"],
        counts["ready_prefix"],
        counts["committed"],
        counts["ready_unpublished"],
        counts["pending"],
        counts["refused"],
        counts["conflicted"]
    ));
    out.push_str(&format!(
        "Archive revision {} → {}. {} Card(s) written, {} replayed, in {} batch(es).\n\n",
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
            "Not complete: the committed prefix is useful work, and the book unit stays incomplete.\n\n",
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
    out.push_str("## Batches\n\n");
    if batches.is_empty() {
        out.push_str("No identified call was sent.\n\n");
    }
    for batch in &batches {
        out.push_str(&format!(
            "- batch {}: {} · {} item(s) · {} bytes\n",
            batch["batch"],
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

    fn row_state(number: u64, chain: Option<Vec<Paper>>) -> RowState {
        let ready = chain.is_some();
        RowState {
            number,
            term: format!("词{number}"),
            job: format!("t{number:03}"),
            planned_category: "concept".to_string(),
            readiness: json!({"row": number, "status": if ready { "ready" } else { "needs_review" }}),
            chain,
            slot: Slot::Absent,
            status: if ready {
                Status::ReadyUnpublished
            } else {
                Status::Refused
            },
            reason: None,
            receipts: Vec::new(),
        }
    }

    fn run_of(rows: Vec<RowState>) -> Run {
        Run {
            rows,
            scan: Scan::default(),
            retype: None,
            revision_before: 7,
        }
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
        let row = row_state(
            1,
            Some(vec![
                paper("词1", "术语：词1\n来源：[[K]] §1", "card"),
                paper("推导（续 2/3）：第一步", "续：[[@@term_head]] 本卡沿用该卡的符号表。", "part"),
                paper("推导（续 3/3）：第二步", "续：[[@@term_head]] 本卡沿用该卡的符号表。", "part"),
            ]),
        );
        let chain = row.chain.clone().unwrap();
        let items = chain_items(&compiled, &row, &chain).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["client_ref"], "r001");
        assert_eq!(items[0]["arguments"]["at"], "K/1|a");
        assert_eq!(items[0]["arguments"]["kind"], "side");
        assert_eq!(items[0]["arguments"]["type"], "term");
        assert_eq!(items[0]["arguments"]["term"], "词1");
        assert_eq!(items[1]["arguments"]["at"], "@@r001");
        assert_eq!(items[1]["arguments"]["kind"], "direct");
        assert_eq!(items[2]["arguments"]["at"], "@@r001-p01");
        assert_eq!(
            items[1]["arguments"]["body"],
            "续：[[@@r001]] 本卡沿用该卡的符号表。"
        );
        assert_eq!(items[0]["op_key"], format!("term-cards-{}/K/2/apply/r001", "a".repeat(16)));
        assert_eq!(
            items[2]["op_key"],
            format!("term-cards-{}/K/2/apply/r001-p02", "a".repeat(16))
        );
    }

    #[test]
    fn chains_are_packed_into_calls_of_at_most_sixty_four_items() {
        let compiled = compiled(200);
        let rows: Vec<RowState> = (1..=200).map(|number| row_state(number, Some(vec![paper(&format!("词{number}"), "来源：[[K]] §1", "card")]))).collect();
        let run = run_of(rows);
        let packing = pack(&compiled, &run);
        assert!(packing.stop.is_none());
        assert_eq!(packing.batches.len(), 4);
        assert!(packing.batches.iter().all(|batch| batch.items.len() <= CHUNK_ITEMS));
        assert_eq!(
            packing.batches.iter().map(|batch| batch.items.len()).sum::<usize>(),
            200
        );
        assert_eq!(packing.batches[0].rows, (1..=64).collect::<Vec<u64>>());
    }

    #[test]
    fn a_chain_that_does_not_fit_one_call_is_refused_and_the_prefix_stays_packed() {
        let compiled = compiled(2);
        let mut huge = vec![paper("词2", "来源：[[K]] §1", "card")];
        for index in 1..70 {
            huge.push(paper(&format!("续 {index}"), "续：[[@@term_head]] 沿用。", "part"));
        }
        let rows = vec![
            row_state(1, Some(vec![paper("词1", "来源：[[K]] §1", "card")])),
            row_state(2, Some(huge)),
        ];
        let packing = pack(&compiled, &run_of(rows));
        let stop = packing.stop.expect("a 70-Card chain is refused");
        assert_eq!(stop.code, "chain_too_large");
        assert_eq!(stop.exit, STOP_EXIT);
        assert_eq!(packing.batches.len(), 1, "the prefix before the refusal stays packed");
        assert_eq!(packing.batches[0].rows, vec![1]);
    }

    #[test]
    fn the_prefix_ends_at_the_first_unready_row_and_later_drafts_stay_staged() {
        let compiled = compiled(3);
        let rows = vec![
            row_state(1, Some(vec![paper("词1", "来源：[[K]] §1", "card")])),
            row_state(2, None),
            row_state(3, Some(vec![paper("词3", "来源：[[K]] §1", "card")])),
        ];
        let packing = pack(&compiled, &run_of(rows));
        assert!(packing.stop.is_none());
        assert_eq!(packing.batches.len(), 1);
        assert_eq!(packing.batches[0].rows, vec![1], "row 3 waits behind row 2");
    }

    #[test]
    fn the_retype_rides_in_the_first_batch_before_its_heads() {
        let compiled = compiled(1);
        let mut run = run_of(vec![row_state(
            1,
            Some(vec![paper("词1", "来源：[[K]] §1", "card")]),
        )]);
        run.retype = Some(json!({
            "op_key": "term-cards-x/K/2/apply/list",
            "client_ref": "list",
            "tool": "edit_card",
            "arguments": {"at": "K/1|a", "type": "term", "base_fingerprint": "b".repeat(64)},
        }));
        let packing = pack(&compiled, &run);
        assert_eq!(packing.batches.len(), 1);
        assert_eq!(packing.batches[0].items[0]["client_ref"], "list");
        assert_eq!(packing.batches[0].items[1]["client_ref"], "r001");
    }

    fn scan_of(side: Vec<&str>, chains: &[(&str, &[&str])], cards: Vec<Value>) -> Scan {
        let mut scan = Scan {
            list: json!({"address": "K/1|a", "type": "term"}),
            side: side.into_iter().map(str::to_string).collect(),
            ..Scan::default()
        };
        for (head, parts) in chains {
            scan.chain.insert(
                head.to_string(),
                parts.iter().map(|part| part.to_string()).collect(),
            );
        }
        for card in cards {
            scan.cards.insert(pstr(&card, "address"), card);
        }
        scan
    }

    fn term_card(address: &str, term: &str, title: &str, body: &str) -> Value {
        json!({"address": address, "type": "term", "term": term, "title": title, "body": body})
    }

    #[test]
    fn a_slot_that_carries_the_plans_rendering_is_committed_and_never_sent_again() {
        let compiled = compiled(1);
        let head = paper("词1", "术语：词1\n来源：[[K]] §1", "card");
        let scan = scan_of(
            vec!["K/1|a|a"],
            &[("K/1|a|a", &[])],
            vec![term_card(
                "K/1|a|a",
                "词1",
                "词1",
                "术语：词1\n来源：[[K]] §1",
            )],
        );
        let rows = judge(
            vec![row_state(1, Some(vec![head]))],
            &scan,
            &Value::Null,
        );
        assert_eq!(rows[0].status, Status::Committed);
        assert_eq!(rows[0].head(), Some(&"K/1|a|a".to_string()));
        assert!(pack(&compiled, &run_of(rows)).batches.is_empty());
    }

    #[test]
    fn a_slot_that_holds_another_card_is_a_conflict_and_stops_the_alignment() {        let rows = vec![
            row_state(1, Some(vec![paper("词1", "来源：[[K]] §1", "card")])),
            row_state(2, Some(vec![paper("词2", "来源：[[K]] §1", "card")])),
        ];
        let scan = scan_of(
            vec!["K/1|a|a"],
            &[("K/1|a|a", &[])],
            vec![term_card("K/1|a|a", "别的词", "别的词", "x")],
        );
        let rows = judge(rows, &scan, &Value::Null);
        assert_eq!(rows[0].status, Status::Conflicted);
        assert_eq!(pstr(&rows[0].reason.clone().unwrap(), "code"), "unrelated_side");
        assert_eq!(rows[1].status, Status::ReadyUnpublished);
        assert_eq!(pstr(&rows[1].reason.clone().unwrap(), "code"), "unaligned");
        let run = run_of(rows);
        let stop = run.structural_stop().unwrap();
        assert_eq!(stop.code, "conflicted");
        assert_eq!(stop.exit, STOP_EXIT);
    }

    #[test]
    fn a_changed_accepted_head_is_never_overwritten() {
        let head = paper("词1", "来源：[[K]] §1", "card");
        let scan = scan_of(
            vec!["K/1|a|a"],
            &[("K/1|a|a", &[])],
            vec![term_card("K/1|a|a", "词1", "词1", "来源：[[K]] §2")],
        );
        let published = json!({"rows": {"1": {"plan_revision": 1, "head": {"address": "K/1|a|a"}, "parts": []}}});
        let rows = judge(vec![row_state(1, Some(vec![head]))], &scan, &published);
        assert_eq!(rows[0].status, Status::Conflicted);
        assert_eq!(pstr(&rows[0].reason.clone().unwrap(), "code"), "changed_head");
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
    fn a_lost_answer_leaves_its_batch_unknown_and_never_claims_completion() {
        let compiled = compiled(3);
        let rows: Vec<RowState> = (1..=3)
            .map(|number| row_state(number, Some(vec![paper(&format!("词{number}"), "来源：[[K]] §1", "card")])))
            .collect();
        let packing = pack(&compiled, &run_of(rows));
        assert_eq!(packing.batches.len(), 1);
        let ran = execute(&packing.batches, |_| {
            Ok(Reply::Lost("the service closed the pipe".to_string()))
        })
        .unwrap();
        assert_eq!(ran.batches[0].state, "unknown");
        let stop = ran.stop.unwrap();
        assert_eq!(stop.code, "interrupted");
        assert_eq!(stop.exit, STOP_EXIT);
        assert!(stop.message.contains("run -Step apply again"), "{}", stop.message);
        // A Lock keeps exit 2; a blocking Card check and a stale base are content stops.
        let locked = json!({"error": "card_locked", "message": "card `K/1|a` is inside locked tree `K`"});
        let ran = execute(&packing.batches, |_| Ok(Reply::Refused(locked.clone()))).unwrap();
        let stop = ran.stop.unwrap();
        assert_eq!(stop.code, "card_locked");
        assert_eq!(stop.exit, lockcheck::LOCKED_EXIT);
        let check = json!({"error": "card_check", "blocking": 1, "faults": ["K/1 R3 甲"]});
        let ran = execute(&packing.batches, |_| Ok(Reply::Refused(check.clone()))).unwrap();
        let stop = ran.stop.unwrap();
        assert_eq!(stop.code, "card_check");
        assert_eq!(stop.exit, STOP_EXIT);
    }

    #[test]
    fn an_answered_batch_must_name_every_item_in_order() {
        let compiled = compiled(2);
        let rows: Vec<RowState> = (1..=2)
            .map(|number| row_state(number, Some(vec![paper(&format!("词{number}"), "来源：[[K]] §1", "card")])))
            .collect();
        let packing = pack(&compiled, &run_of(rows));
        let error = execute(&packing.batches, |_| Ok(Reply::Done(vec![])))
            .unwrap_err()
            .to_string();
        assert!(error.contains("0 of 2"), "{error}");
        let error = execute(&packing.batches, |items| {
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
    fn a_committed_row_whose_readback_differs_is_a_conflict_not_a_success() {
        let compiled = compiled(1);
        let rows = vec![row_state(
            1,
            Some(vec![paper("词1", "术语：词1\n来源：[[K]] §1", "card")]),
        )];
        let packing = pack(&compiled, &run_of(rows));
        let ran = execute(&packing.batches, |items| {
            let answers: Vec<Value> = items
                .iter()
                .map(|item| {
                    json!({
                        "op_key": item["op_key"],
                        "client_ref": item["client_ref"],
                        "replayed": false,
                        "result": {"address": "K/1|a|a", "after_fingerprint": "c".repeat(64)},
                    })
                })
                .collect();
            Ok(Reply::Done(answers))
        })
        .unwrap();
        let mut run = run_of(vec![row_state(
            1,
            Some(vec![paper("词1", "术语：词1\n来源：[[K]] §1", "card")]),
        )]);
        run.settle(&ran, &[1]);
        assert_eq!(run.rows[0].status, Status::Conflicted);
        assert_eq!(
            pstr(&run.rows[0].reason.clone().unwrap(), "code"),
            "output_diverged"
        );
        assert_eq!(run.rows[0].head(), Some(&"K/1|a|a".to_string()));
        assert_eq!(run.structural_stop().unwrap().code, "conflicted");
    }
}




