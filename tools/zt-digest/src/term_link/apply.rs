//! `term-link -Step apply`: the guarded application of one frozen plan (design 13.6-13.7, S23 /
//! #163).
//!
//! Apply never edits from stale offsets. Before one Card is written it revalidates the frozen plan
//! against a fresh coherent read of the same archive:
//!
//! - the archive identity, the book, the Term list (address and body hash), the row→head→auto-link
//!   mapping, the whole vocabulary derivation, and the checker/rule/compiler contract must be the
//!   plan's (R0404). A change to any of them invalidates the whole plan: apply writes nothing and
//!   answers one typed `plan_stale` line, so the operator reviews a new preview (R0405).
//! - the frozen options (`-Occurrences`, `-Cards`) must be the ones the invocation names, and the
//!   scope must still be the same set of Cards with the same roles (R0405).
//! - every Card's authored/body hash must be the plan's. A changed body is explicitly replanned
//!   into the next plan revision, whose updated `preview.md` and per-Card diff the report carries
//!   (R0405). A Card already at its planned output was committed by an earlier run: its frozen
//!   edit is sent again, so the archive replays the accepted receipt (R0415).
//!
//! Every edit carries the archive's authored fingerprint as its `base_fingerprint` and is packed
//! into the existing identified-call chunks (at most 64 items and 1 MiB of authored bytes per call,
//! one final edit per changed Card, stable `op_key`s derived from the plan content and revision). A
//! change between the revalidating read and the commit is the archive's own `write_conflict`
//! refusal: the affected chunk stops, the earlier receipts stay, the later chunks stay pending, and
//! a new plan revision is required — the pass never retries until another writer stops (R0406).
//!
//! A `card_locked` answer stops immediately with the typed tree/Lock facts and exit 2; a
//! `card_check` answer returns its fault lines for correction and never rewrites prose to pass
//! (R0409). A plan with no edit sends no batch at all, so no receipt and no archive revision is
//! written (R0408). The report, `<Staging>/term-link/apply.json` and `apply.md`, carries the counts
//! and evidence the ledger's completion reads (R0410, S23-AC05): scanned/changed/refused Cards,
//! added/repaired/kept/removed links, retained foreign and other uses, committed/replayed/pending/
//! refused chunks, missing heads, avoid words with Card and line, the skip reasons, and the Cards
//! that end with no Term link as an observation only (R0418).
//!
//! The `-Token` claim is recorded in the report so the stage's ledger completion can bind its
//! evidence; the `termlink/book` unit's registration and completion belong to S24 (#167).

use super::{
    APPLY, APPLY_MD, Params, PLAN, PLAN_SCHEMA, PREVIEW, SYNTAX_VERSION, book_key, build_plan,
    counts_of, ensure_book_tree, preview_of, tree_addresses,
};
use crate::calls;
use crate::canon;
use crate::lockcheck;
use crate::organize::common::{self, pstr};
use crate::ps;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// The most items one identified call carries: the archive's own real-batch bound.
pub(crate) const CHUNK_ITEMS: usize = 64;
/// The most authored bytes one identified call carries: the archive's own real-batch bound.
pub(crate) const CHUNK_BYTES: usize = 1024 * 1024;
/// The exit code of a stop that is content or concurrency, not a Lock: the caller corrects the
/// source or rebuilds the preview and runs again.
pub(crate) const STOP_EXIT: i32 = 1;

/// One frozen plan read back from `<Staging>/term-link/plan.json`.
#[derive(Clone, Debug)]
pub(crate) struct Frozen {
    pub(crate) plan: Value,
    pub(crate) path: String,
}

/// The result of revalidating one frozen plan against a fresh read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Review {
    /// The frozen plan still describes the archive; its edits may run as frozen.
    Current,
    /// One or more Card bodies changed before apply. The plan is frozen again at the next revision
    /// with those Cards replanned (R0405); Cards already at their planned output are recorded as
    /// applied, because a new revision gives every remaining edit a new operation key.
    Revised {
        plan: Value,
        replanned: Vec<Value>,
        applied: Vec<Value>,
    },
    /// The plan's contract, mapping, options or scope changed: no edit may run from it (R0405).
    Stale { reasons: Vec<String> },
}

/// One identified chunk: the exact items, the Cards they edit, and their authored size.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Chunk {
    pub(crate) items: Vec<Value>,
    pub(crate) cards: Vec<String>,
    pub(crate) bytes: usize,
}

/// The answer to one chunk.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Reply {
    /// The chunk committed or replayed; its results in order.
    Done(Vec<Value>),
    /// The archive refused the whole chunk with this typed refusal.
    Refused(Value),
    /// The answer was lost: the chunk may or may not have committed.
    Lost(String),
}

/// What one chunk came to.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ChunkState {
    pub(crate) chunk: Chunk,
    /// `committed`, `replayed`, `refused`, `unknown` or `pending`.
    pub(crate) state: &'static str,
    pub(crate) replayed: usize,
    pub(crate) results: Vec<Value>,
    pub(crate) refusal: Option<Value>,
    pub(crate) message: String,
}

/// The stop that ended the run, or None when every chunk settled.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stop {
    /// `card_locked`, `card_check`, `stale_base`, `check_incomplete`, `interrupted`,
    /// `output_diverged` or `refused`.
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) detail: Value,
    pub(crate) exit: i32,
}

/// What the chunk execution came to.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Ran {
    pub(crate) chunks: Vec<ChunkState>,
    pub(crate) stop: Option<Stop>,
}

impl Ran {
    /// The chunks whose edits are committed (freshly or by replay).
    pub(crate) fn settled(&self) -> impl Iterator<Item = &ChunkState> {
        self.chunks
            .iter()
            .filter(|chunk| chunk.state == "committed" || chunk.state == "replayed")
    }

    /// Every committed or replayed receipt, in chunk order.
    pub(crate) fn receipts(&self) -> impl Iterator<Item = &Value> {
        self.settled().flat_map(|chunk| chunk.results.iter())
    }
}

// --- the command ---------------------------------------------------------------------------------

/// `-Step apply`: read the frozen plan, revalidate it, and apply its exact edits.
pub fn run(p: &Params) -> Result<i32> {
    let frozen = read_plan(p)?;
    let mut zt = crate::zt::start()?;
    let manifest = common::manifest(&p.staging)?;
    let key = book_key(&manifest, p)?;
    // The plan's own options are the ones the reviewed preview froze; the invocation must repeat
    // them (the revalidation below refuses a difference).
    let mut frozen_params = p.clone();
    frozen_params.occurrences = occurrences_of(&frozen.plan)?;
    frozen_params.cards = cards_of(&frozen.plan);
    let titles = zt.call("export_titles", json!({"zone": "literature"}))?;
    ensure_book_tree(&titles, &key)?;
    let addresses = tree_addresses(&titles, &key);
    let read = zt.cards(&addresses, "text")?;
    let terms = zt.call("get_terms", json!({"at": key}))?;
    let revision_before = read.archive_revision;
    let fresh = build_plan(
        &manifest,
        &titles,
        revision_before,
        &read.cards,
        &terms,
        &frozen_params,
    )?;
    let (plan, applied_plan, replanned, applied) =
        match review(&frozen.plan, &fresh.plan, p, &read.cards) {
            Review::Stale { reasons } => {
                return stop_stale(p, &frozen, &fresh, revision_before, reasons);
            }
            Review::Current => (frozen.plan.clone(), frozen.plan.clone(), Vec::new(), Vec::new()),
            Review::Revised {
                plan,
                replanned,
                applied,
            } => (frozen.plan.clone(), plan.clone(), replanned, applied),
        };
    // The replanned revision is the plan of record from here on: its preview and its exact bytes
    // replace the reviewed ones, and the report keeps the superseded hash and every diff.
    let revised = applied_plan["plan_hash"] != plan["plan_hash"];
    if revised {
        let work = p.work();
        util::write_atomic(&ps::join(&work, PLAN), &ps::json_pretty(&applied_plan))?;
        util::write_atomic(&ps::join(&work, PREVIEW), &preview_of(&applied_plan))?;
    }
    let plan_path = ps::join(&p.work(), PLAN);
    let chunks = chunks_of(&applied_plan)?;
    // A Lock governs every write; the preflight names it before the first chunk. The archive stays
    // authoritative: a Lock that appears later refuses its chunk with the same typed answer.
    if let Some(refusal) = lockcheck::preflight(&mut zt, "term-link", "apply", &key)? {
        let ran = Ran {
            chunks: chunks.iter().map(pending_state).collect(),
            stop: Some(Stop {
                code: "card_locked",
                message: refusal["error"]
                    .as_str()
                    .or(refusal["message"].as_str())
                    .unwrap_or("the destination tree is locked")
                    .to_string(),
                detail: refusal.clone(),
                exit: lockcheck::LOCKED_EXIT,
            }),
        };
        return finish(
            p,
            &frozen,
            &plan_path,
            &applied_plan,
            revised,
            &replanned,
            &applied,
            &read,
            revision_before,
            revision_before,
            ran,
        );
    }
    let ran = execute(&chunks, |items| send_batch(&mut zt, items))?;
    let (ran, settled_revision) = verify_outputs(&mut zt, &applied_plan, ran)?;
    // A stop after at least one attempted chunk may have left committed writes (and an unknown
    // chunk's commit), so the report names the archive's revision after that work rather than
    // pretending the archive did not move.
    let attempted = ran.chunks.iter().any(|chunk| chunk.state != "pending");
    let revision_after = match settled_revision {
        Some(revision) => revision,
        None if attempted => zt
            .archive_view()
            .ok()
            .and_then(|view| view["archive_revision"].as_u64())
            .unwrap_or(revision_before),
        None => revision_before,
    };
    finish(
        p,
        &frozen,
        &plan_path,
        &applied_plan,
        revised,
        &replanned,
        &applied,
        &read,
        revision_before,
        revision_after,
        ran,
    )
}

/// The frozen plan, read and checked against its own content hash.
pub(crate) fn read_plan(p: &Params) -> Result<Frozen> {
    let path = ps::join(&p.work(), PLAN);
    if !std::path::Path::new(&path).is_file() {
        bail!("term-link apply needs `{path}`; run -Step plan first and review its preview");
    }
    let plan = util::read_json(&path).with_context(|| format!("cannot read `{path}`"))?;
    let schema = plan["schema_version"].as_u64().unwrap_or(0);
    if plan["kind"] != "term_link_plan" || schema != PLAN_SCHEMA as u64 {
        bail!(
            "`{path}` is not a schema-{PLAN_SCHEMA} Term-link plan; run -Step plan again and review the new preview"
        );
    }
    if plan["syntax_version"].as_u64() != Some(SYNTAX_VERSION as u64) {
        bail!(
            "`{path}` was frozen by syntax version {}; this build plans syntax {SYNTAX_VERSION}; run -Step plan again",
            plan["syntax_version"]
        );
    }
    let recorded = plan["plan_hash"]
        .as_str()
        .context("the plan carries no content hash")?
        .to_string();
    let computed = hash_of(&plan);
    if recorded.len() != 64 || recorded != computed {
        bail!("`{path}` does not match its own content hash; run -Step plan again and review the new preview");
    }
    Ok(Frozen { plan, path })
}

/// The plan's content hash: every field but `plan_hash` itself.
pub(crate) fn hash_of(plan: &Value) -> String {
    let mut body = plan.clone();
    if let Some(object) = body.as_object_mut() {
        object.remove("plan_hash");
    }
    canon::hash_value(&body)
}

fn occurrences_of(plan: &Value) -> Result<super::matches::Policy> {
    match plan["options"]["occurrences"].as_str() {
        Some("first") => Ok(super::matches::Policy::First),
        Some("all") => Ok(super::matches::Policy::All),
        other => bail!("the plan names no valid occurrence policy: {other:?}"),
    }
}

/// The exact `-Cards` subset the plan froze; empty when it planned the whole eligible tree.
pub(crate) fn cards_of(plan: &Value) -> Vec<String> {
    plan["options"]["cards"]
        .as_array()
        .map(|cards| {
            cards
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

// --- revalidation and explicit replanning --------------------------------------------------------

/// Revalidate one frozen plan against the fresh plan of the same read. `cards` are the fresh read's
/// Card values, used only to publish the current prose of a replanned Card.
pub(crate) fn review(frozen: &Value, fresh: &Value, p: &Params, cards: &[Value]) -> Review {
    let mut reasons: Vec<String> = Vec::new();
    for (path, reason) in [
        (&["schema_version"][..], "the plan schema changed"),
        (&["syntax_version"][..], "the planned syntax changed"),
        (&["versions"][..], "the checker/rule/compiler contract changed since the plan"),
        (&["archive", "id"][..], "the plan belongs to another archive"),
        (&["archive", "path"][..], "the plan belongs to another archive"),
        (&["book", "citation_key"][..], "the plan belongs to another book"),
        (&["list", "address"][..], "the book's Term list changed"),
        (&["list", "body_sha256"][..], "the Term list changed since the plan"),
        (&["fingerprints", "mapping"][..], "the row→head mapping changed since the plan"),
        (&["fingerprints", "autolink"][..], "a Term head's auto-link choice changed since the plan"),
        (&["fingerprints", "vocabulary"][..], "the book's Term vocabulary changed since the plan"),
    ] {
        if field_of(frozen, path) != field_of(fresh, path) {
            reasons.push(reason.to_string());
        }
    }
    // The invocation must name the reviewed options, not a different pass.
    if frozen["options"]["occurrences"].as_str() != Some(p.occurrences.id()) {
        reasons.push(format!(
            "the plan freezes -Occurrences {}; repeat it, or plan a new preview",
            frozen["options"]["occurrences"].as_str().unwrap_or_default()
        ));
    }
    let wanted: Value = if p.cards.is_empty() {
        Value::Null
    } else {
        json!(p.cards)
    };
    if frozen["options"]["cards"] != wanted {
        reasons.push("the plan freezes another -Cards subset; repeat it, or plan a new preview".to_string());
    }
    compare_scope(frozen, fresh, &mut reasons);
    if !reasons.is_empty() {
        return Review::Stale { reasons };
    }
    let fresh_cards: BTreeMap<String, &Value> = fresh["cards"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|card| (pstr(card, "address"), card))
        .collect();
    let current: BTreeMap<String, &Value> = cards
        .iter()
        .map(|card| (pstr(card, "address"), card))
        .collect();
    let mut merged: Vec<Value> = Vec::new();
    let mut replanned: Vec<Value> = Vec::new();
    let mut applied_cards: Vec<Value> = Vec::new();
    for entry in frozen["cards"].as_array().into_iter().flatten() {
        let at = pstr(entry, "address");
        let Some(fresh_entry) = fresh_cards.get(&at) else {
            // Unreachable after the scope check, but a plan is never edited from a partial match.
            return Review::Stale {
                reasons: vec![format!("`{at}` is no longer in the plan's scope")],
            };
        };
        let unchanged = entry["authored_fingerprint"] == fresh_entry["authored_fingerprint"]
            && entry["body_sha256"] == fresh_entry["body_sha256"];
        if unchanged {
            merged.push(entry.clone());
            continue;
        }
        // Already committed by an earlier run: the current authored read is exactly the planned
        // output. The frozen edit is kept, so a retry of the same revision replays its accepted
        // receipt (R0415); if another Card forces a new revision, this one is marked applied and is
        // never resent under a new operation key with a base the archive has already moved past.
        let applied = entry["changed"] == true
            && entry["new_body_sha256"] == fresh_entry["body_sha256"]
            && entry["new_fingerprint"] == fresh_entry["authored_fingerprint"];
        if applied {
            let mut kept = entry.clone();
            kept["applied"] = json!(true);
            applied_cards.push(json!({
                "address": at,
                "title": entry["title"],
                "fingerprint": fresh_entry["authored_fingerprint"],
            }));
            merged.push(kept);
            continue;
        }
        let body_changed = entry["body_sha256"] != fresh_entry["body_sha256"];
        replanned.push(json!({
            "address": at,
            "title": entry["title"],
            "reason": if body_changed { "body_changed" } else { "authored_changed" },
            "previous_authored_fingerprint": entry["authored_fingerprint"],
            "authored_fingerprint": fresh_entry["authored_fingerprint"],
            "previous_body_sha256": entry["body_sha256"],
            "body_sha256": fresh_entry["body_sha256"],
            "plan_body": entry["body"],
            "current_body": current
                .get(&at)
                .map(|card| json!(pstr(card, "body")))
                .unwrap_or(Value::Null),
            "previous_edits": entry["edits"],
            "edits": fresh_entry["edits"],
            "previous_new_body": entry["new_body"],
            "new_body": fresh_entry["new_body"],
            "previous_changed": entry["changed"],
            "changed": fresh_entry["changed"],
        }));
        merged.push((*fresh_entry).clone());
    }
    // A revision is needed only for a replanned Card. Without one, the frozen plan is current and
    // its cards already at their planned output are sent again so the archive replays their
    // receipts; with one, those cards are recorded as applied and are not resent.
    if replanned.is_empty() {
        return Review::Current;
    }
    let mut plan = frozen.clone();
    plan["cards"] = json!(merged);
    plan["revision"] = json!(frozen["revision"].as_u64().unwrap_or(1) + 1);
    plan["replaces"] = frozen["plan_hash"].clone();
    plan["replanned"] = json!(replanned
        .iter()
        .map(|card| json!({
            "address": card["address"],
            "title": card["title"],
            "reason": card["reason"],
        }))
        .collect::<Vec<_>>());
    plan["applied_cards"] = json!(applied_cards);
    plan["archive"]["revision"] = fresh["archive"]["revision"].clone();
    plan["fingerprints"]["cards"] = fresh["fingerprints"]["cards"].clone();
    plan["counts"] = counts_of(&merged);
    plan["scope"]["changed"] = plan["counts"]["cards_changed"].clone();
    plan["scope"]["refused"] = plan["counts"]["cards_refused"].clone();
    plan["plan_hash"] = json!(hash_of(&plan));
    Review::Revised {
        plan,
        replanned,
        applied: applied_cards,
    }
}

/// One nested field of a plan.
fn field_of<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter()
        .fold(value, |value, key| &value[*key])
}

/// The plan's scope must still be the same Cards with the same roles. A Card added, removed or
/// excluded differently changes the source list the preview reviewed (R0405).
fn compare_scope(frozen: &Value, fresh: &Value, reasons: &mut Vec<String>) {
    for key in ["tree", "workspace", "zone"] {
        if frozen["scope"][key] != fresh["scope"][key] {
            reasons.push("the planned tree or Workspace changed".to_string());
            return;
        }
    }
    let included = |plan: &Value| -> Vec<String> {
        plan["cards"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|card| pstr(card, "address"))
            .collect()
    };
    let (frozen_included, fresh_included) = (included(frozen), included(fresh));
    let frozen_set: BTreeSet<&String> = frozen_included.iter().collect();
    let fresh_set: BTreeSet<&String> = fresh_included.iter().collect();
    let added: Vec<&String> = fresh_set.difference(&frozen_set).copied().collect();
    let removed: Vec<&String> = frozen_set.difference(&fresh_set).copied().collect();
    if !added.is_empty() || !removed.is_empty() {
        reasons.push(format!(
            "the plan's scope changed (added: {}; removed: {})",
            names(&added),
            names(&removed)
        ));
    }
    let excluded = |plan: &Value| -> Vec<(String, String)> {
        plan["scope"]["excluded"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|card| (pstr(card, "address"), pstr(card, "reason")))
            .collect()
    };
    if excluded(frozen) != excluded(fresh) {
        reasons.push("a Card's role or the excluded set changed since the plan".to_string());
    }
}

fn names(items: &[&String]) -> String {
    if items.is_empty() {
        return "none".to_string();
    }
    items
        .iter()
        .map(|item| format!("`{item}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

// --- bounded identified chunks -------------------------------------------------------------------

/// Pack the plan's one-final-edit-per-Card into identified chunks: at most 64 items and 1 MiB of
/// authored bytes each, in natural address order (R0407). The `op_key` of an item is derived from
/// the plan content and its revision only, so a retry of the same plan replays the accepted
/// receipts and a new revision is a new operation.
pub(crate) fn chunks_of(plan: &Value) -> Result<Vec<Chunk>> {
    let hash = plan["plan_hash"]
        .as_str()
        .context("the plan carries no content hash")?;
    let run = format!("term-link-{}", &hash[..16.min(hash.len())]);
    let unit = pstr(&plan["book"], "citation_key");
    if unit.is_empty() {
        bail!("the plan names no book; run -Step plan again");
    }
    let revision = plan["revision"]
        .as_u64()
        .filter(|revision| *revision > 0)
        .context("the plan carries no positive revision")?;
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut index = 0usize;
    for card in plan["cards"].as_array().into_iter().flatten() {
        if card["changed"] != true || card["fault"].is_object() || card["applied"] == true {
            continue;
        }
        let at = pstr(card, "address");
        let body = pstr(card, "body");
        let new_body = card["new_body"]
            .as_str()
            .with_context(|| format!("`{at}` is changed but carries no planned body"))?;
        let base = card["authored_fingerprint"]
            .as_str()
            .filter(|base| base.len() == 64 && base.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .with_context(|| format!("`{at}` carries no authored fingerprint"))?;
        recheck(&at, &body, new_body, card)?;
        let size = new_body.len();
        if size > CHUNK_BYTES {
            bail!(
                "`{at}` plans {size} authored bytes; one identified call carries at most {CHUNK_BYTES}"
            );
        }
        let item = json!({
            "op_key": format!("{run}/{unit}/{revision}/apply/item-{index:06}"),
            "client_ref": index.to_string(),
            "tool": "edit_card",
            "arguments": {"at": at, "body": new_body, "base_fingerprint": base},
        });
        index += 1;
        match chunks.last_mut() {
            Some(chunk) if chunk.items.len() < CHUNK_ITEMS && chunk.bytes + size <= CHUNK_BYTES => {
                chunk.items.push(item);
                chunk.cards.push(at);
                chunk.bytes += size;
            }
            _ => chunks.push(Chunk {
                items: vec![item],
                cards: vec![at],
                bytes: size,
            }),
        }
    }
    Ok(chunks)
}

/// Recheck the preservation equation and the exact byte edits of one Card before its write
/// (R0404, R0413). A plan that no longer rebuilds its output is never sent.
fn recheck(at: &str, body: &str, new_body: &str, card: &Value) -> Result<()> {
    if zt_check::text::strip_term_links(new_body) != zt_check::text::strip_term_links(body) {
        bail!("`{at}` would not keep the same visible prose; the plan is refused and must be replanned");
    }
    let mut rebuilt = String::with_capacity(new_body.len());
    let mut offset = 0usize;
    for edit in card["edits"].as_array().into_iter().flatten() {
        let start = edit["start"].as_u64().unwrap_or(u64::MAX) as usize;
        let end = edit["end"].as_u64().unwrap_or(u64::MAX) as usize;
        let before = edit["before"].as_str().unwrap_or_default();
        let after = edit["after"].as_str().unwrap_or_default();
        if start < offset || end < start || end > body.len() || body.get(start..end) != Some(before) {
            bail!("`{at}` carries an edit that does not match its frozen body; replan it");
        }
        rebuilt.push_str(&body[offset..start]);
        rebuilt.push_str(after);
        offset = end;
    }
    rebuilt.push_str(&body[offset..]);
    if rebuilt != new_body {
        bail!("`{at}`'s frozen edits do not rebuild its planned output; replan it");
    }
    Ok(())
}

fn pending_state(chunk: &Chunk) -> ChunkState {
    ChunkState {
        chunk: chunk.clone(),
        state: "pending",
        replayed: 0,
        results: Vec::new(),
        refusal: None,
        message: String::new(),
    }
}

/// Send every chunk in order. A committed chunk's results stay; a refused chunk stops the run with
/// the archive's typed answer; a lost answer stops it with the chunk's state unknown, because its
/// commit cannot be claimed. Later chunks stay pending and are named in the report (R0406, R0415).
pub(crate) fn execute<F>(chunks: &[Chunk], mut send: F) -> Result<Ran>
where
    F: FnMut(&[Value]) -> Result<Reply>,
{
    let mut states: Vec<ChunkState> = Vec::new();
    let mut stop: Option<Stop> = None;
    for (index, chunk) in chunks.iter().enumerate() {
        if stop.is_some() {
            states.push(pending_state(chunk));
            continue;
        }
        let reply = send(&chunk.items)?;
        match reply {
            Reply::Done(answers) => {
                states.push(done_state(chunk, answers)?);
            }
            Reply::Refused(refusal) => {
                let code = refusal["error"].as_str().unwrap_or("refused");
                let (stop_code, exit) = stop_of(code);
                let message = refusal["message"]
                    .as_str()
                    .or(refusal["error"].as_str())
                    .unwrap_or("the archive refused the chunk")
                    .to_string();
                let mut state = pending_state(chunk);
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
                let mut state = pending_state(chunk);
                state.state = "unknown";
                state.message = message.clone();
                states.push(state);
                stop = Some(Stop {
                    code: "interrupted",
                    message: format!(
                        "chunk {} got no answer ({}); it may or may not have committed: run -Step apply again to replay its receipts",
                        index + 1,
                        chunk
                            .items
                            .first()
                            .and_then(|item| item["op_key"].as_str())
                            .unwrap_or_default()
                    ),
                    detail: json!({"chunk": index, "cards": chunk.cards, "reason": message}),
                    exit: STOP_EXIT,
                });
            }
        }
    }
    Ok(Ran {
        chunks: states,
        stop,
    })
}

/// One answered chunk: verify every result's identity and order before its commit is believed.
fn done_state(chunk: &Chunk, results: Vec<Value>) -> Result<ChunkState> {
    if results.len() != chunk.items.len() {
        bail!(
            "incomplete identified batch: {} of {} results",
            results.len(),
            chunk.items.len()
        );
    }
    let mut replayed = 0usize;
    for (item, result) in chunk.items.iter().zip(&results) {
        if result["op_key"] != item["op_key"]
            || result["client_ref"] != item["client_ref"]
            || !result["replayed"].is_boolean()
            || result["result"]["address"].as_str().is_none_or(str::is_empty)
            || result["result"]["after_fingerprint"]
                .as_str()
                .is_none_or(|fingerprint| fingerprint.len() != 64)
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
    Ok(ChunkState {
        chunk: chunk.clone(),
        state,
        replayed,
        results,
        refusal: None,
        message: String::new(),
    })
}

/// The typed stop a refusal code is: a Lock keeps exit 2, a blocking Card check and a stale base
/// are content/concurrency stops with exit 1.
pub(crate) fn stop_of(code: &str) -> (&'static str, i32) {
    match code {
        "card_locked" => ("card_locked", lockcheck::LOCKED_EXIT),
        "card_check" => ("card_check", STOP_EXIT),
        "write_conflict" => ("stale_base", STOP_EXIT),
        "check_incomplete" => ("check_incomplete", STOP_EXIT),
        _ => ("refused", STOP_EXIT),
    }
}

/// Send one real identified batch through the archive.
pub(crate) fn send_batch(zt: &mut Zt, items: &[Value]) -> Result<Reply> {
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

// --- output verification -------------------------------------------------------------------------

/// Read every committed output once and compare it with the plan's planned fingerprint. A Card
/// that changed again is a divergence, never a silent success. Answers the read revision when a
/// readback happened.
pub(crate) fn verify_outputs(
    zt: &mut Zt,
    plan: &Value,
    mut ran: Ran,
) -> Result<(Ran, Option<u64>)> {
    let expected: BTreeMap<String, String> = plan["cards"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|card| card["changed"] == true)
        .filter_map(|card| {
            Some((
                pstr(card, "address"),
                card["new_fingerprint"].as_str()?.to_string(),
            ))
        })
        .collect();
    let mut addresses: Vec<String> = Vec::new();
    let mut receipt_diverge: Option<Stop> = None;
    for chunk in ran.settled() {
        for result in &chunk.results {
            let (Some(at), Some(fingerprint)) = (
                result["result"]["address"].as_str(),
                result["result"]["after_fingerprint"].as_str(),
            ) else {
                continue;
            };
            if expected.contains_key(at) && !addresses.iter().any(|seen| seen == at) {
                addresses.push(at.to_string());
            }
            // The receipt's own claim must be this plan's planned output.
            if expected
                .get(at)
                .is_some_and(|planned| planned != fingerprint)
                && receipt_diverge.is_none()
            {
                receipt_diverge = Some(Stop {
                    code: "output_diverged",
                    message: format!(
                        "the archived receipt of `{at}` is not this plan's planned output"
                    ),
                    detail: json!({"address": at, "receipt": fingerprint, "planned": expected.get(at)}),
                    exit: STOP_EXIT,
                });
            }
        }
    }
    if receipt_diverge.is_some() {
        ran.stop = receipt_diverge;
    }
    if ran.stop.is_some() || addresses.is_empty() {
        return Ok((ran, None));
    }
    let observed = zt.cards(&addresses, "text")?;
    let revision = observed.archive_revision;
    let mut by_address: BTreeMap<String, &Value> = BTreeMap::new();
    for card in &observed.cards {
        by_address.insert(pstr(card, "address"), card);
    }
    let mut diverged: Vec<Value> = Vec::new();
    for at in &addresses {
        let planned = expected.get(at).cloned().unwrap_or_default();
        match by_address.get(at) {
            Some(card) if calls::authored_fingerprint(card) == planned => {}
            Some(card) => diverged.push(json!({
                "address": at,
                "observed": calls::authored_fingerprint(card),
                "planned": planned,
            })),
            None => diverged.push(json!({
                "address": at,
                "observed": Value::Null,
                "planned": planned,
            })),
        }
    }
    if !diverged.is_empty() {
        ran.stop = Some(Stop {
            code: "output_diverged",
            message: format!(
                "{} committed Card(s) no longer match their planned output; keep them and plan a new revision",
                diverged.len()
            ),
            detail: json!({"diverged": diverged}),
            exit: STOP_EXIT,
        });
    }
    Ok((ran, Some(revision)))
}

// --- the report ----------------------------------------------------------------------------------

/// Write both report artifacts, print the compact answer, and answer the step's exit code.
#[allow(clippy::too_many_arguments)]
fn finish(
    p: &Params,
    frozen: &Frozen,
    plan_path: &str,
    plan: &Value,
    revised: bool,
    replanned: &[Value],
    applied: &[Value],
    read: &zt_check::mcp::CardRead,
    revision_before: u64,
    revision_after: u64,
    ran: Ran,
) -> Result<i32> {
    let bodies: BTreeMap<String, String> = read
        .cards
        .iter()
        .map(|card| (pstr(card, "address"), pstr(card, "body")))
        .collect();
    let report = build_report(
        p,
        frozen,
        plan_path,
        plan,
        revised,
        replanned,
        applied,
        &bodies,
        revision_before,
        revision_after,
        &ran,
    );
    let work = p.work();
    util::write_atomic(&ps::join(&work, APPLY), &ps::json_pretty(&report))?;
    util::write_atomic(&ps::join(&work, APPLY_MD), &report_md(&report))?;
    let mut summary = report["summary"].clone();
    summary["report"] = json!(ps::join(&work, APPLY));
    summary["report_md"] = json!(ps::join(&work, APPLY_MD));
    summary["work"] = json!(work);
    util::print_line(&canon::canonical_json(&summary))?;
    match &ran.stop {
        Some(stop) => {
            // The archive's typed refusal is the step's last line, so a caller reads the same
            // JSON it reads from every other step and never matches English text (R0409).
            if stop.code == "card_locked"
                || stop.code == "card_check"
                || stop.code == "stale_base"
                || stop.code == "check_incomplete"
            {
                util::print_line(&canon::canonical_json(&stop.detail))?;
            }
            eprintln!("zt-digest term-link: {}", stop.message);
            Ok(stop.exit)
        }
        None => Ok(0),
    }
}

/// The report of one apply run: the counts, the chunk states with their receipts, the replanned
/// diffs, the refused Cards and the observations (R0410).
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_report(
    p: &Params,
    frozen: &Frozen,
    plan_path: &str,
    plan: &Value,
    revised: bool,
    replanned: &[Value],
    applied: &[Value],
    bodies: &BTreeMap<String, String>,
    revision_before: u64,
    revision_after: u64,
    ran: &Ran,
) -> Value {
    let counts = &plan["counts"];
    let mut chunk_states = Vec::new();
    for (index, state) in ran.chunks.iter().enumerate() {
        chunk_states.push(json!({
            "chunk": index + 1,
            "cards": state.chunk.cards,
            "items": state.chunk.items.len(),
            "bytes": state.chunk.bytes,
            "state": state.state,
            "replayed": state.replayed,
            "op_keys": state.chunk.items.iter().filter_map(|item| item["op_key"].as_str()).collect::<Vec<_>>(),
            "refusal": state.refusal,
            "message": state.message,
            "receipts": state.results.iter().map(|result| json!({
                "address": result["result"]["address"],
                "op_key": result["op_key"],
                "after_fingerprint": result["result"]["after_fingerprint"],
                "replayed": result["replayed"],
            })).collect::<Vec<_>>(),
        }));
    }
    let by_state = |state: &str| -> usize {
        ran.chunks.iter().filter(|chunk| chunk.state == state).count()
    };
    let cards_in = |state: &str| -> usize {
        ran.chunks
            .iter()
            .filter(|chunk| chunk.state == state)
            .map(|chunk| chunk.chunk.cards.len())
            .sum()
    };
    let replayed_results = ran
        .receipts()
        .filter(|result| result["replayed"] == json!(true))
        .count();
    let written_results = ran.receipts().count() - replayed_results;
    // Which Cards' edits are settled. The plan's skip offsets are byte offsets into the *input*
    // body, so an avoid word's line is read from that body; a Card's resulting body (its planned
    // output once settled) is used only for the no-Term-link observation.
    let settled: BTreeSet<String> = ran
        .settled()
        .flat_map(|chunk| chunk.chunk.cards.iter().cloned())
        .collect();
    let input_body = |card: &Value| -> String {
        let at = pstr(card, "address");
        card["body"]
            .as_str()
            .map(str::to_string)
            .or_else(|| bodies.get(&at).cloned())
            .unwrap_or_default()
    };
    let mut avoid_words = Vec::new();
    let mut missing_heads = Vec::new();
    let mut refused = Vec::new();
    for card in plan["cards"].as_array().into_iter().flatten() {
        let at = pstr(card, "address");
        if card["fault"].is_object() {
            refused.push(card["fault"].clone());
        }
        let body = input_body(card);
        for skip in card["skips"].as_array().into_iter().flatten() {
            if skip["reason"] != "avoid_word" {
                continue;
            }
            let start = skip["start"].as_u64().unwrap_or(0) as usize;
            let (line, line_text) = super::card::line_of(&body, start);
            avoid_words.push(json!({
                "address": at, "line": line, "text": skip["text"], "line_text": line_text,
            }));
        }
    }
    for head in plan["heads"].as_array().into_iter().flatten() {
        if head["head"].is_null() {
            missing_heads.push(json!({"row": head["row"], "term": head["term"]}));
        }
    }
    let mut without_term = Vec::new();
    for card in plan["cards"].as_array().into_iter().flatten() {
        if card["fault"].is_object() {
            continue;
        }
        let at = pstr(card, "address");
        let body = if settled.contains(&at) {
            card["new_body"].as_str().map(str::to_string)
        } else {
            card["body"].as_str().map(str::to_string)
        }
        .or_else(|| bodies.get(&at).cloned());
        let Some(body) = body else { continue };
        let has_term = zt_check::text::parse_links_at(&body)
            .map(|links| {
                links
                    .iter()
                    .any(|link| link.form == zt_check::text::LinkForm::Term)
            })
            .unwrap_or(false);
        if !has_term {
            without_term.push(at);
        }
    }
    let no_op = ran.chunks.is_empty();
    let pending = cards_in("pending") + cards_in("unknown");
    let all_settled = ran
        .chunks
        .iter()
        .all(|chunk| chunk.state == "committed" || chunk.state == "replayed");
    let complete = ran.stop.is_none()
        && counts["cards_refused"].as_u64().unwrap_or(0) == 0
        && all_settled;
    let stop = ran.stop.as_ref().map(|stop| {
        json!({
            "code": stop.code,
            "message": stop.message,
            "exit": stop.exit,
            "detail": stop.detail,
        })
    });
    let summary = json!({
        "schema_version": 1,
        "ok": stop.is_none(),
        "step": "apply",
        "book": plan["book"]["citation_key"],
        "complete": complete,
        "no_op": no_op,
        "code": stop.as_ref().map(|stop| stop["code"].clone()),
        "claim_token": p.token,
        "pass": if p.round.is_some() { "final" } else { "book" },
        "round": p.round,
        "plan_revision": plan["revision"],
        "plan_hash": plan["plan_hash"],
        "plan_replaces": plan["replaces"],
        "revised": revised,
        "replanned": replanned.len(),
        "cards_already_applied": applied.len(),
        "archive_revision_before": revision_before,
        "archive_revision_after": revision_after,
        "cards_scanned": counts["cards_scanned"],
        "cards_changed": counts["cards_changed"],
        "cards_refused": counts["cards_refused"],
        "cards_written": written_results,
        "cards_replayed": replayed_results,
        "cards_pending": pending,
        "chunks": ran.chunks.len(),
        "chunks_committed": by_state("committed"),
        "chunks_replayed": by_state("replayed"),
        "chunks_pending": by_state("pending"),
        "chunks_unknown": by_state("unknown"),
        "chunks_refused": by_state("refused"),
        "links_added": counts["links_added"],
        "links_repaired": counts["links_repaired"],
        "links_kept": counts["links_kept"],
        "links_removed": counts["links_removed"],
        "retained_foreign": counts["retained_foreign"],
        "retained_other": counts["retained_other"],
        "skipped": counts["skipped"],
        "cards_without_term_use": without_term.len(),
        "missing_heads": missing_heads.len(),
        "avoid_words": avoid_words.len(),
    });
    json!({
        "schema_version": 1,
        "kind": "term_link_apply",
        "ok": stop.is_none(),
        "complete": complete,
        "no_op": no_op,
        "code": stop.as_ref().map(|stop| stop["code"].clone()),
        "claim": p.token.map(|token| json!({"token": token})),
        "pass": if p.round.is_some() { "final" } else { "book" },
        "round": p.round,
        "book": plan["book"],
        "plan": {
            "path": plan_path,
            "revision": plan["revision"],
            "hash": plan["plan_hash"],
            "replaces": plan["replaces"],
            "superseded": revised.then(|| frozen.plan["plan_hash"].clone()),
            "revised": revised,
            "replanned": replanned,
            "already_applied": applied,
        },
        "archive": {
            "id": plan["archive"]["id"],
            "path": plan["archive"]["path"],
            "revision_before": revision_before,
            "revision_after": revision_after,
            "plan_revision": plan["archive"]["revision"],
        },
        "versions": plan["versions"],
        "options": plan["options"],
        "scope": plan["scope"],
        "counts": counts,
        "chunks": chunk_states,
        "refused": refused,
        "observations": {
            "cards_without_term_use": without_term.len(),
            "cards_without_term_use_addresses": without_term,
            "missing_heads": missing_heads.len(),
            "missing_heads_detail": missing_heads,
            "avoid_words": avoid_words.len(),
            "avoid_word_detail": avoid_words,
        },
        "stop": stop,
        "summary": summary,
    })
}

/// The human-readable apply report.
pub(crate) fn report_md(report: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Term-link apply\n\n");
    out.push_str(&format!(
        "Book: {} — {}\n\n",
        report["book"]["citation_key"].as_str().unwrap_or_default(),
        report["book"]["title"].as_str().unwrap_or_default()
    ));
    let plan = &report["plan"];
    out.push_str(&format!(
        "Plan revision {} · {} · superseded {}\n\n",
        plan["revision"],
        plan["hash"].as_str().unwrap_or_default(),
        plan["superseded"].as_str().unwrap_or_default()
    ));
    let counts = &report["counts"];
    out.push_str(&format!(
        "{} Cards scanned, {} changed, {} refused. {} link(s) added, {} repaired, {} kept, {} removed.\n\n",
        counts["cards_scanned"],
        counts["cards_changed"],
        counts["cards_refused"],
        counts["links_added"],
        counts["links_repaired"],
        counts["links_kept"],
        counts["links_removed"]
    ));
    let summary = &report["summary"];
    out.push_str(&format!(
        "Archive revision {} → {}. Wrote {} Card(s) in {} committed chunk(s); replayed {} Card(s) in {} chunk(s); {} pending, {} unknown, {} refused.\n\n",
        summary["archive_revision_before"],
        summary["archive_revision_after"],
        summary["cards_written"],
        summary["chunks_committed"],
        summary["cards_replayed"],
        summary["chunks_replayed"],
        summary["cards_pending"],
        summary["chunks_unknown"],
        summary["chunks_refused"],
    ));
    if report["no_op"] == true {
        out.push_str("No edit: the plan has nothing to write, so no receipt and no archive revision.\n\n");
    }
    if let Some(stop) = report["stop"].as_object().filter(|stop| !stop.is_empty()) {
        out.push_str(&format!(
            "## Stopped: {} (exit {})\n\n{}\n\n",
            stop["code"].as_str().unwrap_or_default(),
            stop["exit"],
            stop["message"].as_str().unwrap_or_default()
        ));
    }
    for reason in report["reasons"].as_array().into_iter().flatten() {
        out.push_str(&format!("- {}\n", reason.as_str().unwrap_or_default()));
    }
    if report["reasons"].is_array() {
        out.push_str("\nThe plan is invalidated: no Card was written and a new preview is required.\n\n");
        return out;
    }
    let replanned = report["plan"]["replanned"].as_array().cloned().unwrap_or_default();
    if !replanned.is_empty() {
        out.push_str("## Replanned Cards\n\n");
        for card in &replanned {
            out.push_str(&format!(
                "### {} {} ({})\n\n",
                card["address"].as_str().unwrap_or_default(),
                card["title"].as_str().unwrap_or_default(),
                card["reason"].as_str().unwrap_or_default()
            ));
            diff_section(&mut out, card);
        }
    }
    let refused = report["refused"].as_array().cloned().unwrap_or_default();
    if !refused.is_empty() {
        out.push_str("## Refused Cards\n\n");
        for fault in &refused {
            out.push_str(&format!(
                "- `{}` {} line {}: {} ({})\n",
                fault["address"].as_str().unwrap_or_default(),
                fault["title"].as_str().unwrap_or_default(),
                fault["line"],
                fault["message"].as_str().unwrap_or_default(),
                fault["code"].as_str().unwrap_or_default()
            ));
        }
        out.push('\n');
    }
    let chunks = report["chunks"].as_array().cloned().unwrap_or_default();
    out.push_str("## Chunks\n\n");
    if chunks.is_empty() {
        out.push_str("No edits: no identified call was sent.\n\n");
    }
    for chunk in &chunks {
        out.push_str(&format!(
            "- chunk {}: {} · {} Card(s) · {} bytes\n",
            chunk["chunk"],
            chunk["state"].as_str().unwrap_or_default(),
            chunk["items"],
            chunk["bytes"]
        ));
        for card in chunk["cards"].as_array().into_iter().flatten() {
            out.push_str(&format!("  - `{}`\n", card.as_str().unwrap_or_default()));
        }
    }
    out.push('\n');
    let observations = &report["observations"];
    out.push_str(&format!(
        "## Observations\n\n{} Card(s) with no Term link (an observation, not a defect). {} missing head(s). {} avoid word(s).\n\n",
        observations["cards_without_term_use"],
        observations["missing_heads"],
        observations["avoid_words"]
    ));
    for head in observations["missing_heads_detail"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "- row {} `{}` has no single valid head\n",
            head["row"],
            head["term"].as_str().unwrap_or_default()
        ));
    }
    for word in observations["avoid_word_detail"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "- `{}` line {}: `{}` is an avoid word\n",
            word["address"].as_str().unwrap_or_default(),
            word["line"],
            word["text"].as_str().unwrap_or_default()
        ));
    }
    out.push('\n');
    out
}

/// One replanned Card's updated diff: the body the frozen plan planned from, the current body and
/// the new planned output (R0405).
fn diff_section(out: &mut String, card: &Value) {
    let plan_body = card["plan_body"].as_str();
    let current_body = card["current_body"].as_str();
    let new_body = card["new_body"].as_str();
    if let (Some(plan_body), Some(current_body)) = (plan_body, current_body)
        && plan_body != current_body
    {
        out.push_str("Body changed since the plan:\n\n");
        for (index, (old, new)) in plan_body.lines().zip(current_body.lines()).enumerate() {
            if old != new {
                out.push_str(&format!("line {}:\n  - {old}\n  + {new}\n\n", index + 1));
            }
        }
    }
    match new_body {
        Some(new_body) => {
            out.push_str("Updated plan:\n\n");
            for edit in card["edits"].as_array().into_iter().flatten() {
                out.push_str(&format!(
                    "- {} `{}` → `{}` (bytes {}–{})\n",
                    edit["kind"].as_str().unwrap_or_default(),
                    edit["before"].as_str().unwrap_or_default(),
                    edit["after"].as_str().unwrap_or_default(),
                    edit["start"],
                    edit["end"]
                ));
            }
            out.push('\n');
            if let Some(current_body) = current_body {
                for (index, (old, new)) in current_body.lines().zip(new_body.lines()).enumerate() {
                    if old != new {
                        out.push_str(&format!("line {}:\n  - {old}\n  + {new}\n\n", index + 1));
                    }
                }
            }
        }
        None => out.push_str("The new revision plans no edit for this Card.\n\n"),
    }
}

/// One typed `plan_stale` refusal: the whole plan is invalidated, nothing is written, and the
/// operator reviews a new preview.
fn stop_stale(
    p: &Params,
    frozen: &Frozen,
    fresh: &super::Built,
    revision: u64,
    reasons: Vec<String>,
) -> Result<i32> {
    let message = format!(
        "the Term-link plan no longer matches the archive: {}; run -Step plan again and review the new preview",
        reasons.join("; ")
    );
    let summary = json!({
        "schema_version": 1,
        "ok": false,
        "step": "apply",
        "book": fresh.plan["book"]["citation_key"],
        "complete": false,
        "no_op": false,
        "code": "plan_stale",
        "claim_token": p.token,
        "plan_revision": frozen.plan["revision"],
        "plan_hash": frozen.plan["plan_hash"],
        "archive_revision_before": revision,
        "archive_revision_after": revision,
        "cards_scanned": fresh.plan["counts"]["cards_scanned"],
        "cards_changed": fresh.plan["counts"]["cards_changed"],
        "message": message,
    });
    let report = json!({
        "schema_version": 1,
        "kind": "term_link_apply",
        "ok": false,
        "complete": false,
        "no_op": false,
        "code": "plan_stale",
        "claim": p.token.map(|token| json!({"token": token})),
        "pass": if p.round.is_some() { "final" } else { "book" },
        "round": p.round,
        "book": fresh.plan["book"],
        "plan": {
            "path": frozen.path,
            "revision": frozen.plan["revision"],
            "hash": frozen.plan["plan_hash"],
            "replaces": frozen.plan["replaces"],
            "superseded": Value::Null,
            "revised": false,
            "replanned": [],
        },
        "archive": {
            "id": fresh.plan["archive"]["id"],
            "path": fresh.plan["archive"]["path"],
            "revision_before": revision,
            "revision_after": revision,
            "plan_revision": fresh.plan["archive"]["revision"],
        },
        "versions": frozen.plan["versions"],
        "options": frozen.plan["options"],
        "scope": fresh.plan["scope"],
        "counts": fresh.plan["counts"],
        "chunks": [],
        "refused": [],
        "observations": {
            "cards_without_term_use": 0,
            "cards_without_term_use_addresses": [],
            "missing_heads": 0,
            "missing_heads_detail": [],
            "avoid_words": 0,
            "avoid_word_detail": [],
        },
        "reasons": reasons,
        "message": message,
        "stop": {"code": "plan_stale", "message": message, "exit": STOP_EXIT, "detail": {"reasons": reasons}},
        "summary": summary,
    });
    let work = p.work();
    util::write_atomic(&ps::join(&work, APPLY), &ps::json_pretty(&report))?;
    util::write_atomic(&ps::join(&work, APPLY_MD), &report_md(&report))?;
    util::print_line(&canon::canonical_json(&report["summary"]))?;
    eprintln!("zt-digest term-link: {message}");
    Ok(STOP_EXIT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canon::{hash_value, sha256_hex};
    use serde_json::json;

    fn params() -> Params {
        Params {
            step: "apply".to_string(),
            staging: "D:\\staging".to_string(),
            citation_key: None,
            occurrences: super::super::matches::Policy::First,
            cards: Vec::new(),
            round: None,
            token: None,
            apply: true,
        }
    }

    fn digest(text: &str) -> String {
        sha256_hex(text.as_bytes())
    }

    /// One Card entry as the plan freezes it: an unchanged Card carries only its guards.
    fn card(at: &str, body: &str, changed: bool) -> Value {
        let base = digest(&format!("{at}:/{body}"));
        let mut entry = json!({
            "address": at,
            "title": format!("title {at}"),
            "type": "idea",
            "term_role": null,
            "authored_fingerprint": base,
            "body_sha256": digest(body),
            
            "new_fingerprint": Value::Null,
            "changed": false,
            "edits": [],
            "kept": [],
            "retained": [],
            "skips": [],
            "fault": null,
        });
        if changed {
            let new_body = body.replace("生产函数", "[[生产函数@K/4]]");
            let start = body.find("生产函数").unwrap();
            entry["changed"] = json!(true);
            entry["body"] = json!(body);
            entry["new_body"] = json!(new_body);
            entry["new_body_sha256"] = json!(digest(&new_body));
            entry["new_fingerprint"] = json!(digest(&format!("{at}:/{new_body}")));
            entry["edits"] = json!([{
                "kind": "link", "start": start, "end": start + 12,
                "term": "生产函数", "target": "K/4", "label": "生产函数",
                "before": "生产函数", "after": "[[生产函数@K/4]]",
            }]);
        }
        entry
    }

    fn plan_of(revision: u64, cards: Vec<Value>) -> Value {
        let counts = counts_of(&cards);
        let mut plan = json!({
            "schema_version": PLAN_SCHEMA,
            "kind": "term_link_plan",
            "syntax_version": SYNTAX_VERSION,
            "revision": revision,
            "replaces": Value::Null,
            "replanned": [],
            "book": {"citation_key": "K", "title": "书", "variant": ""},
            "archive": {"id": "a".repeat(32), "path": "\\\\?\\D:\\zt", "revision": 5},
            "versions": {"compiler": env!("CARGO_PKG_VERSION"), "checker": zt_check::VERSION, "rule": zt_check::contract::RULE_VERSION},
            "options": {"occurrences": "first", "cards": Value::Null},
            "scope": {
                "tree": "K", "workspace": "agent", "zone": "literature",
                "cards": cards.len() + 2, "eligible": cards.len(), "included": cards.len(), "unselected": 0,
                "changed": counts["cards_changed"], "refused": counts["cards_refused"],
                "excluded": [{"address": "K", "title": "书", "reason": "root"}, {"address": "K/3", "title": "术语表", "reason": "term_list"}],
            },
            "list": {"address": "K/3", "title": "术语表", "body_sha256": digest("表"), "row_count": 1, "rows": []},
            "heads": [{"row": 1, "term": "生产函数", "head": "K/4", "heads": ["K/4"], "auto_link": true, "published": true}],
            "fingerprints": {"list": digest("表"), "mapping": digest("m"), "autolink": digest("a"), "vocabulary": digest("v"), "cards": hash_value(&json!(cards))},
            "term_structure": {"faults": [], "row_duplicates": [], "coverage": {}},
            "cards": cards,
            "counts": counts,
        });
        plan["plan_hash"] = json!(hash_of(&plan));
        plan
    }

    fn fresh_from(frozen: &Value, cards: Vec<Value>) -> Value {
        let mut fresh = plan_of(1, cards);
        fresh["archive"] = frozen["archive"].clone();
        fresh
    }

    fn review_of(frozen: &Value, fresh: &Value, p: &Params, read: &[Value]) -> Review {
        review(frozen, fresh, p, read)
    }

    #[test]
    fn an_equal_read_revalidates_to_the_same_frozen_plan() {
        let frozen = plan_of(
            1,
            vec![card("K/1", "生产函数决定产出。", true), card("K/2", "均衡。", false)],
        );
        let fresh = fresh_from(&frozen, frozen["cards"].as_array().unwrap().clone());
        assert_eq!(review_of(&frozen, &fresh, &params(), &[]), Review::Current);
    }

    #[test]
    fn a_changed_body_is_replanned_into_the_next_revision_with_its_diff() {
        let frozen = plan_of(
            1,
            vec![card("K/1", "生产函数决定产出。", true), card("K/2", "均衡。", false)],
        );
        // The operator changed the Card before apply: a new sentence, with the term at a new offset.
        let changed = "第一段。生产函数决定产出。";
        let next = card("K/1", changed, true);
        let fresh = fresh_from(&frozen, vec![next, card("K/2", "均衡。", false)]);
        let read = vec![json!({"address": "K/1", "body": changed, "title": "title K/1", "type": "idea"})];
        let Review::Revised { plan, replanned, .. } = review_of(&frozen, &fresh, &params(), &read) else {
            panic!("a changed body must be replanned: {fresh}");
        };
        assert_eq!(plan["revision"], 2);
        assert_eq!(plan["replaces"], frozen["plan_hash"]);
        assert_eq!(plan["replanned"][0]["address"], "K/1");
        assert_eq!(plan["replanned"][0]["reason"], "body_changed");
        assert_eq!(replanned.len(), 1);
        let diff = &replanned[0];
        assert_eq!(diff["address"], "K/1");
        assert_eq!(diff["plan_body"], "生产函数决定产出。");
        assert_eq!(diff["current_body"], changed);
        assert_eq!(diff["edits"][0]["start"], 12, "the byte offset after 第一段。");
        assert_eq!(diff["previous_new_body"], "[[生产函数@K/4]]决定产出。");
        assert_eq!(diff["new_body"], "第一段。[[生产函数@K/4]]决定产出。");
        // The revision's own hash moves, and a second review of the same inputs is byte-identical.
        assert_ne!(plan["plan_hash"], frozen["plan_hash"]);
        assert_eq!(hash_of(&plan), plan["plan_hash"].as_str().unwrap());
        let Review::Revised { plan: again, .. } = review_of(&frozen, &fresh, &params(), &read) else {
            panic!("not revised");
        };
        assert_eq!(canon::canonical_json(&plan), canon::canonical_json(&again));
        // The unchanged Card keeps its frozen entry and its guard.
        let kept = plan["cards"]
            .as_array()
            .unwrap()
            .iter()
            .find(|card| card["address"] == "K/2")
            .unwrap();
        assert_eq!(kept["changed"], false);
        assert_eq!(plan["counts"]["cards_changed"], 1);
        assert_eq!(plan["counts"]["cards_scanned"], 2);
    }

    #[test]
    fn a_body_already_at_its_planned_output_replays_instead_of_replanning() {
        let frozen = plan_of(1, vec![card("K/1", "生产函数决定产出。", true)]);
        // A committed chunk whose answer was lost: the Card is already the planned output.
        let output = "[[生产函数@K/4]]决定产出。";
        let mut now = card("K/1", output, false);
        now["authored_fingerprint"] = frozen["cards"][0]["new_fingerprint"].clone();
        let fresh = fresh_from(&frozen, vec![now]);
        assert_eq!(review_of(&frozen, &fresh, &params(), &[]), Review::Current);
    }

    #[test]
    fn a_replan_marks_an_already_committed_card_applied_and_never_resends_it() {
        // Two changed Cards; the first was committed by an earlier run, the second then changed.
        let first = card("K/1", "生产函数决定产出。", true);
        let frozen = plan_of(1, vec![first.clone(), card("K/2", "生产函数影响均衡。", true)]);
        let mut committed = card("K/1", "[[生产函数@K/4]]决定产出。", false);
        committed["authored_fingerprint"] = first["new_fingerprint"].clone();
        let changed = "第一段。生产函数影响均衡。";
        let second = card("K/2", changed, true);
        let fresh = fresh_from(&frozen, vec![committed, second]);
        let read = vec![json!({"address": "K/2", "body": changed, "title": "title K/2", "type": "idea"})];
        let Review::Revised { plan, replanned, applied } =
            review_of(&frozen, &fresh, &params(), &read)
        else {
            panic!("the changed Card must replan");
        };
        assert_eq!(replanned.len(), 1);
        assert_eq!(replanned[0]["address"], "K/2");
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0]["address"], "K/1");
        assert_eq!(applied[0]["fingerprint"], first["new_fingerprint"]);
        assert_eq!(plan["applied_cards"], json!(applied));
        assert_eq!(plan["counts"]["cards_applied"], 1);
        // The committed Card is recorded as applied, and its edit is not owed again under a new key.
        let kept = plan["cards"]
            .as_array()
            .unwrap()
            .iter()
            .find(|card| card["address"] == "K/1")
            .unwrap();
        assert_eq!(kept["applied"], true);
        assert_eq!(kept["changed"], true, "the record of the edit stays");
        let chunks = chunks_of(&plan).unwrap();
        assert_eq!(chunks.iter().map(|chunk| chunk.items.len()).sum::<usize>(), 1);
        assert_eq!(chunks[0].items[0]["arguments"]["at"], "K/2");
    }

    #[test]
    fn a_changed_contract_mapping_list_options_or_scope_invalidates_the_whole_plan() {
        let frozen = plan_of(1, vec![card("K/1", "生产函数决定产出。", true)]);
        let fresh = fresh_from(&frozen, frozen["cards"].as_array().unwrap().clone());
        let stale = |frozen: &Value, fresh: &Value, p: &Params| match review_of(frozen, fresh, p, &[]) {
            Review::Stale { reasons } => reasons,
            other => panic!("not stale: {other:?}"),
        };
        // It is current first, so each mutation below is the only reason.
        assert_eq!(review_of(&frozen, &fresh, &params(), &[]), Review::Current);
        let mut other = fresh.clone();
        other["versions"]["rule"] = json!("0.13.0");
        assert!(stale(&frozen, &other, &params())[0].contains("contract"));
        let mut other = fresh.clone();
        other["syntax_version"] = json!(2);
        assert!(stale(&frozen, &other, &params())[0].contains("syntax"));
        let mut other = fresh.clone();
        other["archive"]["id"] = json!("b".repeat(32));
        assert!(stale(&frozen, &other, &params())[0].contains("archive"));
        let mut other = fresh.clone();
        other["list"]["body_sha256"] = json!(digest("另一张表"));
        assert!(stale(&frozen, &other, &params())[0].contains("Term list"));
        for key in ["mapping", "autolink", "vocabulary"] {
            let mut other = fresh.clone();
            other["fingerprints"][key] = json!(digest(key));
            assert!(stale(&frozen, &other, &params())[0].contains("changed"), "{key}");
        }
        let mut other = fresh.clone();
        other["scope"]["excluded"][1]["reason"] = json!("structure");
        assert!(stale(&frozen, &other, &params())[0].contains("role"));
        // A Card added to the tree, or removed from it, changes the source list.
        let mut other = plan_of(
            1,
            vec![card("K/1", "生产函数决定产出。", true), card("K/2", "均衡。", false)],
        );
        other["archive"] = frozen["archive"].clone();
        assert!(stale(&frozen, &other, &params())[0].contains("scope changed"));
        let mut other = plan_of(1, vec![]);
        other["archive"] = frozen["archive"].clone();
        assert!(stale(&frozen, &other, &params())[0].contains("scope changed"));
        // The invocation must repeat the plan's options.
        let mut all = params();
        all.occurrences = super::super::matches::Policy::All;
        assert!(stale(&frozen, &fresh, &all)[0].contains("-Occurrences"));
        let mut subset = params();
        subset.cards = vec!["K/1".to_string()];
        assert!(stale(&frozen, &fresh, &subset)[0].contains("-Cards"));
    }

    #[test]
    fn the_plan_and_its_revision_never_claim_a_modified_or_missing_plan() {
        let dir = tempfile::tempdir().unwrap();
        let p = Params { staging: dir.path().to_string_lossy().into_owned(), ..params() };
        let missing = read_plan(&p).unwrap_err().to_string();
        assert!(missing.contains("run -Step plan first"), "{missing}");
        std::fs::create_dir_all(dir.path().join("term-link")).unwrap();
        let mut value = plan_of(1, vec![card("K/1", "生产函数决定产出。", true)]);
        std::fs::write(
            dir.path().join("term-link").join("plan.json"),
            serde_json::to_string(&value).unwrap(),
        )
        .unwrap();
        assert!(read_plan(&p).is_ok());
        // A changed byte is refused instead of replayed.
        value["counts"]["links_added"] = json!(99);
        std::fs::write(
            dir.path().join("term-link").join("plan.json"),
            serde_json::to_string(&value).unwrap(),
        )
        .unwrap();
        let tampered = read_plan(&p).unwrap_err().to_string();
        assert!(tampered.contains("content hash"), "{tampered}");
        // An older schema or syntax is refused with the preview instruction.
        let mut old = plan_of(1, vec![]);
        old["schema_version"] = json!(1);
        old["plan_hash"] = json!(hash_of(&old));
        std::fs::write(
            dir.path().join("term-link").join("plan.json"),
            serde_json::to_string(&old).unwrap(),
        )
        .unwrap();
        assert!(read_plan(&p).unwrap_err().to_string().contains("run -Step plan again"));
        old["schema_version"] = json!(PLAN_SCHEMA);
        old["syntax_version"] = json!(SYNTAX_VERSION + 1);
        old["plan_hash"] = json!(hash_of(&old));
        std::fs::write(
            dir.path().join("term-link").join("plan.json"),
            serde_json::to_string(&old).unwrap(),
        )
        .unwrap();
        assert!(read_plan(&p).unwrap_err().to_string().contains("syntax version"));
    }

    fn changed_cards(count: usize) -> Vec<Value> {
        (0..count)
            .map(|index| card(&format!("K/{}", index + 10), "生产函数决定产出。", true))
            .collect()
    }

    #[test]
    fn the_chunks_are_bounded_stable_and_hold_one_final_edit_per_card() {
        let plan = plan_of(1, changed_cards(300));
        let chunks = chunks_of(&plan).unwrap();
        // 300 edits at 64 items per identified call: five calls, never one unbounded batch.
        assert_eq!(chunks.len(), 5);
        assert_eq!(chunks.iter().map(|chunk| chunk.items.len()).sum::<usize>(), 300);
        assert!(chunks.iter().all(|chunk| chunk.items.len() <= CHUNK_ITEMS));
        assert_eq!(chunks[0].items.len(), 64);
        assert_eq!(chunks[4].items.len(), 44);
        let mut cards: Vec<String> = chunks.iter().flat_map(|chunk| chunk.cards.clone()).collect();
        cards.sort();
        cards.dedup();
        assert_eq!(cards.len(), 300, "one final edit per changed Card");
        let hash = plan["plan_hash"].as_str().unwrap();
        let first = &chunks[0].items[0];
        assert_eq!(first["tool"], "edit_card");
        assert_eq!(
            first["op_key"],
            format!("term-link-{}/K/1/apply/item-000000", &hash[..16])
        );
        assert_eq!(first["client_ref"], "0");
        assert_eq!(first["arguments"]["at"], "K/10");
        assert_eq!(first["arguments"]["body"], "[[生产函数@K/4]]决定产出。");
        assert_eq!(
            first["arguments"]["base_fingerprint"],
            card("K/10", "生产函数决定产出。", true)["authored_fingerprint"]
        );
        // Equal plans produce equal chunks; a new revision is a new operation.
        let again = chunks_of(&plan).unwrap();
        assert_eq!(chunks, again);
        let mut next = plan.clone();
        next["revision"] = json!(2);
        next["plan_hash"] = json!(hash_of(&next));
        let moved = chunks_of(&next).unwrap();
        assert_ne!(moved[0].items[0]["op_key"], first["op_key"]);
    }

    #[test]
    fn a_large_body_splits_the_chunk_before_the_byte_bound() {
        let body = format!("生产函数{}。", "很".repeat(CHUNK_BYTES / 4));
        let cards = vec![
            card("K/1", &body, true),
            card("K/2", &body, true),
            card("K/3", "生产函数决定产出。", true),
        ];
        let plan = plan_of(1, cards);
        let chunks = chunks_of(&plan).unwrap();
        assert!(chunks.iter().all(|chunk| chunk.bytes <= CHUNK_BYTES));
        assert_eq!(chunks.iter().map(|chunk| chunk.items.len()).sum::<usize>(), 3);
        // Each large body takes its own chunk; the small body joins the second.
        assert_eq!(
            chunks.len(),
            2,
            "{:?}",
            chunks.iter().map(|chunk| chunk.bytes).collect::<Vec<_>>()
        );
        assert_eq!(chunks[1].items.len(), 2);
    }

    #[test]
    fn a_plan_that_breaks_the_preservation_equation_or_its_edits_is_never_sent() {
        // The planned output changes the visible prose: the equation refuses it.
        let mut broken = card("K/1", "生产函数决定产出。", true);
        broken["new_body"] = json!("替代弹性决定产出。");
        broken["edits"] = json!([{
            "kind": "link", "start": 0, "end": 12, "before": "生产函数", "after": "替代弹性",
        }]);
        let plan = plan_of(1, vec![broken]);
        let error = chunks_of(&plan).unwrap_err().to_string();
        assert!(error.contains("visible prose"), "{error}");
        // The edits no longer rebuild the planned output.
        let mut broken = card("K/1", "生产函数决定产出。", true);
        broken["new_body"] = json!("[[生产函数@K/4]]影响产出。");
        let plan = plan_of(1, vec![broken]);
        let error = chunks_of(&plan).unwrap_err().to_string();
        assert!(error.contains("replan"), "{error}");
        // A faulted Card produces no edit at all, and a clean one still does.
        let mut faulted = card("K/2", "生产函数决定产出。", false);
        faulted["fault"] = json!({"address": "K/2", "title": "t", "code": "malformed_term_markup", "at": 0, "line": 1, "line_text": "", "message": "x"});
        let plan = plan_of(1, vec![card("K/1", "生产函数决定产出。", true), faulted]);
        let chunks = chunks_of(&plan).unwrap();
        assert_eq!(chunks.iter().map(|chunk| chunk.items.len()).sum::<usize>(), 1);
    }

    fn answer(items: &[Value]) -> Reply {
        Reply::Done(
            items
                .iter()
                .map(|item| {
                    json!({
                        "op_key": item["op_key"], "client_ref": item["client_ref"], "replayed": false,
                        "result": {"address": item["arguments"]["at"], "after_fingerprint": "f".repeat(64)},
                    })
                })
                .collect(),
        )
    }

    #[test]
    fn every_chunk_commits_and_a_later_failure_leaves_the_rest_pending() {
        let plan = plan_of(1, changed_cards(130));
        let chunks = chunks_of(&plan).unwrap();
        assert_eq!(chunks.len(), 3);
        let mut sent = 0;
        let ran = execute(&chunks, |items| {
            sent += 1;
            Ok(answer(items))
        })
        .unwrap();
        assert_eq!(sent, 3);
        assert!(ran.stop.is_none());
        assert!(ran.chunks.iter().all(|chunk| chunk.state == "committed"));
        assert_eq!(ran.receipts().count(), 130);
        // A refusal in the second chunk keeps the first committed chunk and leaves the third
        // pending; the typed refusal and its exit travel with the stop.
        let refusal = json!({"error": "write_conflict", "code": "write_conflict", "message": "authored base changed at `K/20`"});
        let mut seen = 0;
        let ran = execute(&chunks, |items| {
            seen += 1;
            if seen == 2 {
                Ok(Reply::Refused(refusal.clone()))
            } else {
                Ok(answer(items))
            }
        })
        .unwrap();
        let states: Vec<&str> = ran.chunks.iter().map(|chunk| chunk.state).collect();
        assert_eq!(states, ["committed", "refused", "pending"]);
        assert_eq!(
            ran.receipts().count(),
            64,
            "only the committed chunk's receipts stay"
        );
        let stop = ran.stop.unwrap();
        assert_eq!(stop.code, "stale_base");
        assert_eq!(stop.exit, STOP_EXIT);
        assert_eq!(stop.detail, refusal);
    }

    #[test]
    fn a_lost_answer_leaves_its_chunk_unknown_and_never_claims_completion() {
        let plan = plan_of(1, changed_cards(70));
        let chunks = chunks_of(&plan).unwrap();
        let ran = execute(&chunks, |_| {
            Ok(Reply::Lost("the service closed the pipe".to_string()))
        })
        .unwrap();
        let states: Vec<&str> = ran.chunks.iter().map(|chunk| chunk.state).collect();
        assert_eq!(states, ["unknown", "pending"]);
        assert_eq!(ran.receipts().count(), 0);
        let stop = ran.stop.unwrap();
        assert_eq!(stop.code, "interrupted");
        assert_eq!(stop.exit, STOP_EXIT);
        assert!(stop.message.contains("run -Step apply again"), "{}", stop.message);
        // A typed Lock keeps exit 2 and a blocking Card check returns its fault lines.
        let locked = jobs_locked();
        let ran = execute(&chunks, |_| Ok(Reply::Refused(locked.clone()))).unwrap();
        let stop = ran.stop.unwrap();
        assert_eq!(stop.code, "card_locked");
        assert_eq!(stop.exit, lockcheck::LOCKED_EXIT);
        let check = json!({"error": "card_check", "blocking": 1, "faults": ["K/10 R3 (advisory) 甲；乙。"]});
        let ran = execute(&chunks, |_| Ok(Reply::Refused(check.clone()))).unwrap();
        let stop = ran.stop.unwrap();
        assert_eq!(stop.code, "card_check");
        assert_eq!(stop.exit, STOP_EXIT);
        assert_eq!(stop.detail["faults"][0], "K/10 R3 (advisory) 甲；乙。");
    }

    fn jobs_locked() -> Value {
        json!({"error": "card_locked", "message": "card `K/10` is inside locked tree `K`",
            "tree": "K", "reason": "edit", "hint": "unlock the tree in the GUI first",
            "locks": [{"lock_id": 2, "tree": "K"}]})
    }

    #[test]
    fn an_answered_chunk_with_mismatched_identities_is_an_incomplete_batch() {
        let plan = plan_of(1, changed_cards(2));
        let chunks = chunks_of(&plan).unwrap();
        let error = execute(&chunks, |items| {
            let mut answers = match answer(items) {
                Reply::Done(answers) => answers,
                other => panic!("{other:?}"),
            };
            answers.reverse();
            Ok(Reply::Done(answers))
        })
        .unwrap_err()
        .to_string();
        assert!(error.contains("identity/order mismatch"), "{error}");
        let error = execute(&chunks, |_| Ok(Reply::Done(vec![])))
            .unwrap_err()
            .to_string();
        assert!(error.contains("0 of 2"), "{error}");
    }

    #[test]
    fn a_plan_with_no_edit_sends_no_batch_and_reports_no_op() {
        let plan = plan_of(1, vec![card("K/1", "[[生产函数@K/4]]决定产出。", false)]);
        let chunks = chunks_of(&plan).unwrap();
        assert!(chunks.is_empty());
        let ran = execute(&chunks, |_| panic!("a no-op plan must send no batch")).unwrap();
        let report = build_report(
            &params(),
            &Frozen { plan: plan.clone(), path: "plan.json".into() },
            "plan.json",
            &plan,
            false,
            &[],
            &[],
            &BTreeMap::new(),
            5,
            5,
            &ran,
        );
        assert_eq!(report["no_op"], true);
        assert_eq!(report["complete"], true);
        assert_eq!(report["summary"]["cards_written"], 0);
        assert_eq!(report["summary"]["chunks"], 0);
        assert_eq!(report["summary"]["archive_revision_before"], 5);
        assert_eq!(report["summary"]["archive_revision_after"], 5);
        let md = report_md(&report);
        assert!(md.contains("No edit: the plan has nothing to write"), "{md}");
    }

    #[test]
    fn the_report_counts_every_phase_and_names_the_observations() {
        let mut body_card = card("K/1", "生产函数决定产出。替代弹性系数不变。", true);
        let avoid_at = body_card["body"]
            .as_str()
            .unwrap()
            .find("替代弹性系数")
            .unwrap();
        body_card["skips"] = json!([
            {"start": avoid_at, "end": avoid_at + 18, "reason": "avoid_word", "text": "替代弹性系数"},
            {"start": avoid_at, "end": avoid_at + 18, "reason": "own_term", "text": "替代弹性系数"},
        ]);
        let refused = {
            let mut card = card("K/2", "坏宏。", false);
            card["fault"] = json!({"address": "K/2", "title": "t", "code": "malformed_term_markup", "at": 0, "line": 1, "line_text": "坏宏。", "message": "invalid"});
            card
        };
        let plain = card("K/3", "没有术语的一段话。", false);
        let plan = plan_of(1, vec![body_card, refused, plain]);
        let chunks = chunks_of(&plan).unwrap();
        assert_eq!(chunks.len(), 1);
        let answers = match answer(&chunks[0].items) {
            Reply::Done(answers) => answers,
            other => panic!("{other:?}"),
        };
        let state = done_state(&chunks[0], answers).unwrap();
        let ran = Ran { chunks: vec![state], stop: None };
        let report = build_report(
            &params(),
            &Frozen { plan: plan.clone(), path: "plan.json".into() },
            "plan.json",
            &plan,
            false,
            &[],
            &[],
            &BTreeMap::new(),
            5,
            6,
            &ran,
        );
        assert_eq!(report["summary"]["cards_written"], 1);
        assert_eq!(report["summary"]["cards_refused"], 1);
        assert_eq!(report["summary"]["complete"], false);
        assert_eq!(report["observations"]["avoid_words"], 1);
        assert_eq!(report["observations"]["avoid_word_detail"][0]["address"], "K/1");
        assert_eq!(report["observations"]["avoid_word_detail"][0]["line"], 1);
        assert_eq!(report["observations"]["avoid_word_detail"][0]["text"], "替代弹性系数");
        assert_eq!(report["summary"]["missing_heads"], 0);
        let md = report_md(&report);
        assert!(md.contains("avoid word"), "{md}");
        assert!(md.contains("Refused Cards"), "{md}");
        assert!(md.contains("chunk 1: committed"), "{md}");
    }

    #[test]
    fn the_human_report_names_the_stop_and_the_replanned_diff() {
        let frozen = plan_of(1, vec![card("K/1", "生产函数决定产出。", true)]);
        let changed = "第一段。生产函数决定产出。";
        let next = card("K/1", changed, true);
        let fresh = fresh_from(&frozen, vec![next]);
        let read = vec![json!({"address": "K/1", "body": changed, "title": "title K/1", "type": "idea"})];
        let Review::Revised { plan, replanned, .. } = review_of(&frozen, &fresh, &params(), &read) else {
            panic!("not revised");
        };
        let ran = Ran {
            chunks: Vec::new(),
            stop: Some(Stop {
                code: "card_check",
                message: "card_check: 1 blocking fault".to_string(),
                detail: json!({"error": "card_check", "faults": ["K/1 R3 (advisory) 甲；乙。"]}),
                exit: STOP_EXIT,
            }),
        };
        let report = build_report(
            &params(),
            &Frozen { plan: frozen.clone(), path: "plan.json".into() },
            "plan.json",
            &plan,
            true,
            &replanned,
            &[],
            &BTreeMap::new(),
            5,
            6,
            &ran,
        );
        assert_eq!(report["ok"], false);
        assert_eq!(report["code"], "card_check");
        assert_eq!(report["plan"]["revised"], true);
        assert_eq!(report["plan"]["superseded"], frozen["plan_hash"]);
        let md = report_md(&report);
        assert!(md.contains("Stopped: card_check (exit 1)"), "{md}");
        assert!(md.contains("Replanned Cards"), "{md}");
        assert!(md.contains("Body changed since the plan:"), "{md}");
        assert!(md.contains("Updated plan:"), "{md}");
    }
}
