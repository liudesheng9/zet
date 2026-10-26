//! The Term backfill preparation branch of `zt-digest prepare` (S25, #168; design 14.3 and 16).
//!
//!   zt-digest prepare -Out <prepared staging> -Validate [-Brief]
//!   zt-digest prepare -Out <prepared staging> -Sections [-Brief]
//!
//! A backfill adds Term Cards and Term links to one book that was digested before the Term work
//! existed. It must not digest that book again, so its preparation is a narrow branch of the one
//! existing `prepare` command, never a second pipeline (R0435):
//!
//! - `-Validate` inspects the already prepared run and its book: the Workspace, the one Term list
//!   (or the pre-Term `术语表` table), the existing descendants and their occupancy, the staged
//!   source slices, the frozen run contract and the owner's Lock. It fails before any provider work
//!   (S25-AC01) and names every conflict it finds (R0488).
//! - `-Sections` does the same and then reuses the staged source slices, or reconstructs them from
//!   the source. A reconstruction must reproduce the frozen run contract's own hashes byte for byte;
//!   a source that no longer does that is a conflict, never a silent staging rewrite. It writes no
//!   root, anchor, group, model or narrative Card (S25-AC02/R0487).
//!
//! Both branches bind the same fingerprints the Term-card plan reads: the source's SHA-256, the
//! section slice hashes, the whole-book hash and the Term list's authored title and body hash. They
//! write `<staging>/backfill.json` and the same facts under `backfill` in `manifest.json`; neither is
//! part of the immutable run contract, so the prepared run's binding is unchanged. They print one
//! compact JSON line: the stage's answer. A Lock stops them with the archive's typed `card_locked`
//! refusal and exit 2 (S25-AC05); a conflict ends them with `backfill_conflict` and exit 1, and no
//! Card and no Term plan is written.

use crate::args::Args;
use crate::figures::Figure;
use crate::ps::{self, CiMap};
use crate::term_cards::{self, ListRead, Row};
use crate::util;
use crate::zt::Zt;
use crate::{canon, lockcheck, prepare};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The record both branches write beside the manifest.
pub(crate) const RECORD: &str = "backfill.json";

/// One reason the selected book cannot take a Term backfill yet. A conflict blocks the branch before
/// any provider dispatch; it is shown, never smoothed over by a fresh digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Conflict {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

fn conflict(code: &'static str, message: impl Into<String>) -> Conflict {
    Conflict {
        code,
        message: message.into(),
    }
}

fn conflicts_value(conflicts: &[Conflict]) -> Value {
    Value::Array(
        conflicts
            .iter()
            .map(|conflict| json!({"code": conflict.code, "message": conflict.message}))
            .collect(),
    )
}

/// The entry point the `prepare` command dispatches to when it was given `-Validate` or `-Sections`.
pub(crate) fn run(a: &Args) -> Result<i32> {
    let sections = a.flag("Sections");
    if sections && a.flag("Validate") {
        bail!("-Sections already validates the book; give -Validate or -Sections, not both");
    }
    let step = if sections { "sections" } else { "validate" };
    if a.has("Source") {
        bail!("-Source is not a backfill option: the source comes from the prepared run's manifest (restore that file, or prepare the book again)");
    }
    for refused in ["Apply", "Variant", "RunOptions", "MaxAttempts", "DshRunCap", "Book", "FileId", "Instructions", "NoEventCandidates", "NoFigures"] {
        if a.flag(refused) || a.has(refused) {
            bail!("-{refused} prepares a full digest; the backfill branch takes only -Out");
        }
    }
    let out = a.str("Out").trim_end_matches(['\\', '/']).to_string();
    if !Path::new(&out).is_dir() {
        bail!("the staging folder `{out}` does not exist; a backfill works on the prepared run's own staging folder");
    }
    let staging = ps::full_name(&out);
    let manifest_path = ps::join(&staging, "manifest.json");
    if !Path::new(&manifest_path).is_file() {
        bail!("`{manifest_path}` does not exist; a backfill needs the prepared run's staging folder");
    }
    let manifest = util::read_json(&manifest_path)?;
    let key = manifest["citation_key"].as_str().unwrap_or_default().trim().to_string();
    if key.is_empty() {
        bail!("manifest.json names no citation_key; a backfill needs its book");
    }
    let mut conflicts: Vec<Conflict> = Vec::new();
    let contract = manifest["run_contract"].clone();
    if !contract.is_object() {
        conflicts.push(conflict(
            "not_prepared",
            "the staging folder has no frozen run contract; prepare this book with mode full first, then backfill its Terms",
        ));
        return finish(&manifest, &manifest_path, &staging, step, &key, "", &contract, &Facts::default(), &conflicts);
    }
    if !Path::new(&ps::join(&staging, "ledger.sqlite3")).is_file() {
        conflicts.push(conflict(
            "no_ledger",
            "the staging folder has no run ledger; a backfill schedules its Term work in the prepared run's own ledger",
        ));
    }

    // The owner's Lock comes first: the branch can neither unlock nor route around it (S25-AC05).
    let mut zt = crate::zt::start()?;
    if let Some(lock) = lockcheck::lock_of(&mut zt, &key)? {
        return lockcheck::stop("prepare", &lockcheck::refusal("prepare", step, &key, &lock));
    }
    let tree = match zt.call("get_tree", json!({"tree": key})) {
        Ok(tree) => tree,
        Err(error) => {
            conflicts.push(conflict("no_tree", format!("the Literature tree {key} cannot be read: {error}")));
            return finish(&manifest, &manifest_path, &staging, step, &key, "", &contract, &Facts::default(), &conflicts);
        }
    };
    let archive_revision = tree["archive_revision"].as_u64().unwrap_or(0);
    let workspace = tree["workspace"].as_str().unwrap_or_default().to_string();
    if let Some(refused) = workspace_refusal(&workspace, &key) {
        conflicts.push(refused);
    }
    let cards: Vec<Value> = tree["cards"].as_array().cloned().unwrap_or_default();
    let mut facts = Facts {
        workspace,
        cards: cards.len(),
        archive_revision,
        ..Facts::default()
    };

    // --- the book's Term list, by role -----------------------------------------------------------
    let list = read_list(&mut zt, &cards, &key, &mut conflicts);
    if let Some(list) = &list {
        facts.list = Some(list_facts(list));
        let (head_count, part_count, occupying) = occupancy(&cards, list, &mut conflicts);
        facts.heads = head_count;
        facts.parts = part_count;
        facts.occupying = occupying;
    }

    // --- the source and its staged slices -------------------------------------------------------
    let source_facts = source_state(&staging, &manifest, &contract, sections, &mut conflicts);
    facts.source = source_facts;

    // Incomplete generation is debt to show, never a reason to rerun a fresh digest (S25-AC07).
    facts.drafts = drafts(&staging);

    finish(&manifest, &manifest_path, &staging, step, &key, &tree["tree"].as_str().unwrap_or_default().to_string(), &contract, &facts, &conflicts)
}

/// The book must live in the agent Workspace: a digest writes no human Card (Access, R0487). The
/// sentence names the Workspace the tree really has, so the owner reads what to fix.
fn workspace_refusal(workspace: &str, key: &str) -> Option<Conflict> {
    (workspace != "agent").then(|| {
        conflict(
            "human_workspace",
            format!("the Literature tree {key} is in the {workspace} Workspace; a digest writes the agent Workspace only"),
        )
    })
}

/// Everything both branches bind, kept together so the record and the answer are one shape.
#[derive(Clone, Debug, Default)]
struct Facts {
    workspace: String,
    cards: usize,
    heads: usize,
    parts: usize,
    occupying: Vec<String>,
    archive_revision: u64,
    list: Option<Value>,
    source: Value,
    drafts: Value,
}

fn list_facts(list: &ListRead) -> Value {
    json!({
        "address": list.address,
        "type": list.card_type,
        "title": list.title,
        "legacy": list.legacy,
        "rows": list.rows.len(),
        "terms": list.rows.iter().map(|row| row.term.clone()).collect::<Vec<_>>(),
        "sha256": list_sha256(list),
    })
}

/// The exact authored identity of one Term list: its title and body, never its Card kind (the first
/// publication retypes a legacy table metadata only).
fn list_sha256(list: &ListRead) -> String {
    canon::hash_value(&json!({"address": list.address, "title": list.title, "body": list.body}))
}

/// One book's Term list, read by role: exactly one Term Card without a name, or exactly one legacy
/// `术语表` table. Duplicate rows and an ambiguous or absent list are conflicts, never guessed.
fn read_list(zt: &mut Zt, cards: &[Value], key: &str, conflicts: &mut Vec<Conflict>) -> Option<ListRead> {
    let (lists, legacy) = term_cards::list_candidates(cards);
    let (candidate, is_legacy) = match (lists.as_slice(), legacy.as_slice()) {
        ([one], []) => (one, false),
        ([], [one]) => (one, true),
        ([], []) => {
            conflicts.push(conflict(
                "no_list",
                format!("the Literature tree {key} has no Term list and no legacy 术语表 table; run the Terms stage for this book first"),
            ));
            return None;
        }
        _ => {
            let names: Vec<String> = lists
                .iter()
                .chain(legacy.iter())
                .map(|card| format!("{} ({})", pstr(card, "address"), pstr(card, "title")))
                .collect();
            conflicts.push(conflict(
                "ambiguous_list",
                format!(
                    "the Literature tree {key} holds {} Term lists or legacy tables ({}); name one list or repair the tree",
                    names.len(),
                    names.join("; ")
                ),
            ));
            return None;
        }
    };
    let address = pstr(candidate, "address");
    let card = match zt.call("get_card", json!({"at": address})) {
        Ok(card) => card,
        Err(error) => {
            conflicts.push(conflict("unreadable_list", format!("the Term list {address} cannot be read: {error}")));
            return None;
        }
    };
    let title = pstr(&card, "title");
    let body = pstr(&card, "body");
    let duplicates = zt_check::text::term_row_duplicates(&body);
    if !duplicates.is_empty() {
        let named: Vec<String> = duplicates
            .iter()
            .map(|group| {
                format!(
                    "{} (rows {})",
                    group.spellings.join(", "),
                    group.rows.iter().map(usize::to_string).collect::<Vec<_>>().join(", ")
                )
            })
            .collect();
        conflicts.push(conflict(
            "duplicate_rows",
            format!(
                "the Term list {address} repeats a term: {}; one row per term is required before a backfill",
                named.join("; ")
            ),
        ));
        return None;
    }
    let rows: Vec<Row> = match term_cards::parsed_rows(&body) {
        Ok(rows) => rows,
        Err(error) => {
            conflicts.push(conflict("invalid_rows", format!("the Term list {address} names no usable row: {error}")));
            return None;
        }
    };
    Some(ListRead {
        address,
        card_type: pstr(&card, "type"),
        title,
        body,
        legacy: is_legacy,
        rows,
    })
}

/// The list's existing publication: its immediate Side children are the row slots in address order,
/// and each head's own descendants are its continuation parts. A slot that holds another Card, a
/// Term of no row, a repeated Term or a row out of order is an occupation conflict (R0488).
fn occupancy(cards: &[Value], list: &ListRead, conflicts: &mut Vec<Conflict>) -> (usize, usize, Vec<String>) {
    let mut children: Vec<&Value> = cards
        .iter()
        .filter(|card| immediate_side_child(&pstr(card, "address"), &list.address))
        .collect();
    children.sort_by_key(|card| address_key(&pstr(card, "address")));
    let row_of: BTreeMap<&str, usize> = list.rows.iter().map(|row| (row.term.as_str(), row.row)).collect();
    let mut occupying: Vec<String> = Vec::new();
    let mut why: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut last_row = 0usize;
    for (index, child) in children.iter().enumerate() {
        let at = pstr(child, "address");
        let card_type = pstr(child, "type");
        let term = child.get("term").and_then(Value::as_str).unwrap_or_default().to_string();
        let slot = index + 1;
        if card_type != "term" || term.is_empty() {
            occupying.push(at.clone());
            why.push(format!("`{at}` is a {} Card, not a named Term Card of slot {slot}", if card_type.is_empty() { "plain" } else { card_type.as_str() }));
            continue;
        }
        if !seen.insert(term.clone()) {
            occupying.push(at.clone());
            why.push(format!("`{at}` repeats the Term `{term}`"));
            continue;
        }
        // #182: a refused row leaves no slot, so a published head is read by its Term and only has to keep
        // table order after the head before it.
        match row_of.get(term.as_str()) {
            Some(row) if *row > last_row => last_row = *row,
            Some(row) => {
                occupying.push(at.clone());
                why.push(format!("`{at}` holds row {row}'s Term `{term}` at slot {slot}; the rows publish in table order"));
            }
            None => {
                occupying.push(at.clone());
                why.push(format!("`{at}` names the Term `{term}`, which is not a row of the list"));
            }
        }
    }
    if !why.is_empty() {
        conflicts.push(conflict(
            "occupying_children",
            format!(
                "the Term list {} already holds {} conflicting Side child(ren): {}",
                list.address,
                why.len(),
                why.join("; ")
            ),
        ));
    }
    let heads: Vec<String> = children.iter().map(|card| pstr(card, "address")).collect();
    let parts = cards
        .iter()
        .filter(|card| {
            let at = pstr(card, "address");
            heads.iter().any(|head| below(&at, head))
        })
        .count();
    (heads.len(), parts, occupying)
}

/// The staged source state: the source's availability and hash, and its section slices. `-Sections`
/// reuses matching slices or reconstructs them from the source under the frozen contract; `-Validate`
/// only reports what is staged.
fn source_state(
    staging: &str,
    manifest: &Value,
    contract: &Value,
    sections: bool,
    conflicts: &mut Vec<Conflict>,
) -> Value {
    let mut facts = json!({
        "path": pstr(manifest, "source"),
        "sha256": pstr(manifest, "source_sha256"),
        "sections": 0,
        "book_sha256": "",
        "reused": false,
        "rebuilt": [],
        "bound": false,
    });
    let path = pstr(manifest, "source");
    if path.trim().is_empty() || !Path::new(&path).is_file() {
        conflicts.push(conflict(
            "missing_source",
            format!("the prepared run's source `{path}` is missing; restore it or prepare the book again"),
        ));
        return facts;
    }
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            conflicts.push(conflict("missing_source", format!("the prepared run's source `{path}` cannot be read: {error}")));
            return facts;
        }
    };
    let sha = canon::sha256_hex(&bytes);
    let recorded = pstr(manifest, "source_sha256");
    facts["sha256"] = json!(sha);
    if !recorded.is_empty() && recorded != sha {
        conflicts.push(conflict(
            "source_changed",
            format!(
                "the source `{path}` is not the prepared run's source (prepared {}, now {}); restore it or prepare the book again",
                &recorded[..recorded.len().min(16)],
                &sha[..sha.len().min(16)]
            ),
        ));
        return facts;
    }
    let raw = String::from_utf8_lossy(&bytes).into_owned();
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw).replace("\r\n", "\n");
    let options = &contract["options"];
    let start = pstr(options, "start_heading");
    let end = pstr(options, "end_heading");
    let skip: Vec<String> = pstr(options, "skip_headings")
        .split(',')
        .map(|heading| heading.trim().to_string())
        .filter(|heading| !heading.is_empty())
        .collect();
    let min_unit_chars = options["min_unit_chars"].as_i64().unwrap_or(1000);

    let sections_dir = ps::join(staging, "sections");
    let names = staged_section_names(&sections_dir);
    let book_path = ps::join(staging, "book.zh.md");
    let book_text = if Path::new(&book_path).is_file() {
        util::read_text(&book_path).ok()
    } else {
        None
    };
    facts["sections"] = json!(names.len());

    if !sections {
        if names.is_empty() || book_text.is_none() {
            conflicts.push(conflict(
                "missing_sections",
                format!("the staged source slices under `{sections_dir}` are incomplete; run prepare -Sections to reconstruct them"),
            ));
            return facts;
        }
        let book_sha = canon::sha256_hex(book_text.as_deref().unwrap_or_default().as_bytes());
        facts["book_sha256"] = json!(book_sha);
        facts["reused"] = json!(true);
        facts["bound"] = json!(contract["artifacts"]["book"]["sha256"] == json!(book_sha));
        return facts;
    }

    // A reconstruction reads the source under the frozen options and the staged figure map, so the
    // artifact lines it reproduces are the ones the original preparation wrote.
    let figures = staged_figures(staging);
    let kept = prepare::chinese_blocks(&raw, &figures);
    let empty = format!("the source text has no section between `{start}` and `{end}`; the frozen options no longer select text");
    let rebuilt = match prepare::staged_sections(&kept, &start, &end, &skip, min_unit_chars, &empty) {
        Ok(staged) => staged,
        Err(error) => {
            conflicts.push(conflict("staged_source_mismatch", format!("the staged source cannot be reconstructed: {error}")));
            return facts;
        }
    };
    let expected = frozen_slices(contract);
    let matches_frozen = expected.iter().all(|(name, sha)| {
        rebuilt
            .files
            .iter()
            .find(|(file, _)| file == name)
            .is_some_and(|(_, text)| canon::sha256_hex(text.as_bytes()) == *sha)
    }) && expected.len() == rebuilt.files.len()
        && contract["artifacts"]["book"]["sha256"]
            .as_str()
            .is_some_and(|sha| canon::sha256_hex(rebuilt.book.as_bytes()) == sha);
    let already = names.len() == rebuilt.files.len()
        && book_text.as_deref() == Some(rebuilt.book.as_str())
        && rebuilt.files.iter().all(|(name, text)| {
            std::fs::read(ps::join(&sections_dir, name))
                .ok()
                .is_some_and(|bytes| bytes == text.as_bytes())
        });
    if already {
        facts["book_sha256"] = json!(canon::sha256_hex(rebuilt.book.as_bytes()));
        facts["reused"] = json!(true);
        facts["bound"] = json!(matches_frozen);
        return facts;
    }
    if !matches_frozen {
        conflicts.push(conflict(
            "staged_source_mismatch",
            "the reconstructed source slices do not reproduce the prepared run's frozen contract; the staged source belongs to another preparation",
        ));
        return facts;
    }
    let mut written: Vec<String> = Vec::new();
    for (name, text) in &rebuilt.files {
        let path = ps::join(&sections_dir, name);
        if std::fs::read(&path).ok().as_deref() != Some(text.as_bytes()) {
            let written_file = std::fs::create_dir_all(&sections_dir).map_err(anyhow::Error::from).and_then(|_| util::write_atomic(&path, text));
            if let Err(error) = written_file {
                conflicts.push(conflict("missing_sections", format!("the section `{path}` cannot be written: {error}")));
                return facts;
            }
            written.push(name.clone());
        }
    }
    if book_text.as_deref() != Some(rebuilt.book.as_str()) {
        if let Err(error) = util::write_atomic(&book_path, &rebuilt.book) {
            conflicts.push(conflict("missing_sections", format!("the book text `{book_path}` cannot be written: {error}")));
            return facts;
        }
        written.push("book.zh.md".to_string());
    }
    facts["sections"] = json!(rebuilt.files.len());
    facts["book_sha256"] = json!(canon::sha256_hex(rebuilt.book.as_bytes()));
    facts["reused"] = json!(written.is_empty());
    facts["rebuilt"] = json!(written);
    facts["bound"] = json!(true);
    facts
}

/// The staged section file names `sNN.md` (the index is no section), in culture order, which is the
/// order the preparation assigned them.
fn staged_section_names(dir: &str) -> Vec<String> {
    let mut names: Vec<String> = ps::child_files(dir, "s", ".md")
        .into_iter()
        .filter(|name| !name.eq_ignore_ascii_case("index.md"))
        .collect();
    ps::ps_sort(&mut names, |a, b| ps::culture_cmp(a, b));
    names
}

/// The frozen contract's section hashes by file name, and the whole-book hash under `book.zh.md`.
fn frozen_slices(contract: &Value) -> Vec<(String, String)> {
    let mut expected: Vec<(String, String)> = Vec::new();
    for group in contract["groups"].as_array().into_iter().flatten() {
        for section in group["sections"].as_array().into_iter().flatten() {
            let path = pstr(section, "path");
            let name = path.rsplit(['\\', '/']).next().unwrap_or_default().to_string();
            if let Some(sha) = section["sha256"].as_str() {
                expected.push((name, sha.to_string()));
            }
        }
    }
    expected
}

/// The staged figure map: the preparation's own `figures.json`, so a reconstruction reproduces its
/// `artifact:` lines without asking the Scholar Archive again.
fn staged_figures(staging: &str) -> CiMap<Figure> {
    let mut figures: CiMap<Figure> = CiMap::new();
    let path = ps::join(staging, "figures.json");
    let Ok(record) = util::read_json(&path) else {
        return figures;
    };
    for figure in ps::each(&record["figures"]) {
        let name = pstr(figure, "name");
        if name.is_empty() {
            continue;
        }
        figures.insert(
            &name,
            Figure {
                sha256: figure["sha256"].clone(),
                path: figure["path"].clone(),
                labels: figure["labels"]
                    .as_array()
                    .map(|labels| labels.iter().filter_map(Value::as_str).map(str::to_string).collect())
                    .unwrap_or_default(),
                alt: pstr(figure, "alt"),
                line: pstr(figure, "line"),
                position: 0,
            },
        );
    }
    figures
}

/// The staged generation's own progress: a plan revision and its readiness counts when they exist.
/// This is debt to report, never a conflict: a ready prefix is useful committed work.
fn drafts(staging: &str) -> Value {
    let work = ps::join(staging, "term-cards");
    let plan_path = ps::join(&work, "plan.json");
    let readiness_path = ps::join(&work, "check").to_string();
    let readiness_path = ps::join(&readiness_path, "readiness.json");
    let plan = util::read_json(&plan_path).ok();
    let readiness = util::read_json(&readiness_path).ok();
    if plan.is_none() && readiness.is_none() {
        return Value::Null;
    }
    json!({
        "plan_revision": plan.as_ref().and_then(|plan| plan["revision"].as_u64()),
        "plan_hash": plan.as_ref().and_then(|plan| plan["plan_hash"].as_str()),
        "counts": readiness.as_ref().map(|readiness| readiness["counts"].clone()),
        "complete": readiness.as_ref().and_then(|readiness| readiness["complete"].as_bool()).unwrap_or(false),
    })
}

/// Write the record, the manifest's `backfill` facts and the answer line, then answer the exit code:
/// 0 when the book can take the backfill, 1 when a conflict stops it.
fn finish(
    manifest: &Value,
    manifest_path: &str,
    staging: &str,
    step: &str,
    key: &str,
    tree: &str,
    contract: &Value,
    facts: &Facts,
    conflicts: &[Conflict],
) -> Result<i32> {
    let complete = conflicts.is_empty();
    let list = facts.list.clone();
    let list_sha256 = list
        .as_ref()
        .and_then(|list| list["sha256"].as_str())
        .unwrap_or_default()
        .to_string();
    let record = json!({
        "schema_version": 1,
        "kind": "backfill_preparation",
        "step": step,
        "book": key,
        "tree_address": tree,
        "workspace": facts.workspace,
        "tree": {
            "cards": facts.cards,
            "heads": facts.heads,
            "parts": facts.parts,
            "occupying": facts.occupying,
        },
        "list": list,
        "list_sha256": list_sha256,
        "source": facts.source,
        "drafts": facts.drafts,
        "contract": format!(
            "{}/{}",
            pstr(&contract["contracts"], "base_version"),
            pstr(&contract["contracts"], "variant_version")
        ),
        "archive_revision": facts.archive_revision,
        "conflicts": conflicts_value(conflicts),
        "complete": complete,
        "created_at": canon::utc_now_o(),
    });
    let record_path = ps::join(staging, RECORD);
    let mut manifest_out = manifest.clone();
    manifest_out["backfill"] = record.clone();
    util::write_atomic(&record_path, &ps::json_pretty(&record))?;
    util::write_atomic(manifest_path, &ps::json_pretty(&manifest_out))?;
    // The answer line is one compact JSON object a relay carries whole; the record keeps every term.
    let mut answer = record.clone();
    if let Some(list) = answer.get_mut("list")
        && let Some(terms) = list.get_mut("terms")
        && let Some(array) = terms.as_array_mut()
        && array.len() > 32
    {
        let omitted = array.len() - 32;
        array.truncate(32);
        list["terms_omitted"] = json!(omitted);
    }
    answer["record"] = json!(record_path);
    if complete {
        answer["ok"] = json!(true);
        util::print_line(&ps::json_compact(&answer))?;
        Ok(0)
    } else {
        answer["ok"] = json!(false);
        answer["code"] = json!("backfill_conflict");
        answer["error"] = json!(conflicts.first().map(|conflict| conflict.message.clone()).unwrap_or_default());
        util::print_line(&ps::json_compact(&answer))?;
        Ok(1)
    }
}

// --- address helpers -----------------------------------------------------------------------------

fn pstr(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// The sort key of one address, so `|a`, `|aa` and `|b` order as the archive orders them.
fn address_key(address: &str) -> (String, String) {
    match address.rsplit_once('|') {
        Some((parent, label)) => (parent.to_string(), label.to_string()),
        None => (address.to_string(), String::new()),
    }
}

/// Whether `address` is an immediate Side child of `parent`: `parent|label`, all lowercase letters.
fn immediate_side_child(address: &str, parent: &str) -> bool {
    let Some(rest) = address.strip_prefix(&format!("{parent}|")) else {
        return false;
    };
    !rest.is_empty() && !rest.contains(['/', '|']) && rest.chars().all(|ch| ch.is_ascii_lowercase())
}

/// Whether `address` lies below `parent` (`parent/1`, `parent|a`, `parent|a/1`).
fn below(address: &str, parent: &str) -> bool {
    !parent.is_empty()
        && address != parent
        && (address.starts_with(&format!("{parent}/")) || address.starts_with(&format!("{parent}|")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list_read(rows: &[&str]) -> ListRead {
        ListRead {
            address: "Key/1|a".into(),
            card_type: "term".into(),
            title: "术语表".into(),
            body: String::new(),
            legacy: false,
            rows: rows
                .iter()
                .enumerate()
                .map(|(index, term)| Row {
                    row: index + 1,
                    term: term.to_string(),
                    avoid: Vec::new(),
                    meaning: String::new(),
                    defined: String::new(),
                })
                .collect(),
        }
    }

    fn card(address: &str, card_type: &str, term: Option<&str>) -> Value {
        let mut value = json!({"address": address, "type": card_type, "title": "t"});
        if let Some(term) = term {
            value["term"] = json!(term);
        }
        value
    }

    #[test]
    fn a_human_workspace_book_is_never_a_backfill_target() {
        assert_eq!(workspace_refusal("agent", "Key"), None);
        let refused = workspace_refusal("human", "Key").expect("human is refused");
        assert_eq!(refused.code, "human_workspace");
        assert!(refused.message.contains("the human Workspace"));
        assert!(refused.message.contains("agent Workspace only"));
    }

    #[test]
    fn an_empty_slot_list_has_no_occupation_conflict() {
        let mut conflicts = Vec::new();
        let cards = vec![json!({"address": "Key/1"})];
        let (heads, parts, occupying) = occupancy(&cards, &list_read(&["均衡", "剩余控制权"]), &mut conflicts);
        assert_eq!((heads, parts, occupying.len()), (0, 0, 0));
        assert!(conflicts.is_empty());
    }

    #[test]
    fn published_heads_in_table_order_occupy_their_own_slots() {
        let mut conflicts = Vec::new();
        let cards = vec![
            card("Key/1|a|b", "term", Some("均衡")),
            card("Key/1|a|b/1", "term", Some("均衡")),
            card("Key/1|a|c", "term", Some("剩余控制权")),
        ];
        let (heads, parts, occupying) = occupancy(&cards, &list_read(&["均衡", "剩余控制权"]), &mut conflicts);
        assert_eq!((heads, parts), (2, 1));
        assert!(occupying.is_empty(), "{occupying:?}");
        assert!(conflicts.is_empty(), "{conflicts:?}");
    }

    #[test]
    fn another_card_or_an_unknown_or_repeated_term_is_an_occupation_conflict() {
        let mut conflicts = Vec::new();
        let cards = vec![
            card("Key/1|a|b", "idea", None),
            card("Key/1|a|c", "term", Some("另一个术语")),
            card("Key/1|a|d", "term", Some("剩余控制权")),
            card("Key/1|a|e", "term", Some("均衡")),
            card("Key/1|a|f", "term", Some("均衡")),
        ];
        let (heads, _parts, occupying) = occupancy(&cards, &list_read(&["均衡", "剩余控制权"]), &mut conflicts);
        assert_eq!(heads, 5);
        assert_eq!(occupying, vec!["Key/1|a|b", "Key/1|a|c", "Key/1|a|e", "Key/1|a|f"]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].code, "occupying_children");
        let message = &conflicts[0].message;
        assert!(message.contains("not a named Term Card"), "{message}");
        assert!(message.contains("not a row of the list"), "{message}");
        assert!(message.contains("repeats the Term `均衡`"), "{message}");
        assert!(message.contains("publish in table order"), "{message}");
    }

    #[test]
    fn a_refused_row_leaves_no_slot_and_the_later_rows_keep_table_order() {
        // #182 publishes every ready row past a refused one (live run 2026-10-26, staging8: row 2 was refused,
        // row 3's head took the second slot); a backfill reads that publication by identity, in table order.
        let mut conflicts = Vec::new();
        let cards = vec![
            card("Key/1|a|b", "term", Some("均衡")),
            card("Key/1|a|c", "term", Some("外包")),
        ];
        let (heads, _parts, occupying) = occupancy(&cards, &list_read(&["均衡", "剩余控制权", "外包"]), &mut conflicts);
        assert_eq!(heads, 2);
        assert!(occupying.is_empty(), "{occupying:?}");
        assert!(conflicts.is_empty(), "{conflicts:?}");
    }

    #[test]
    fn a_side_child_of_the_list_is_not_a_published_head() {
        let list = list_read(&["均衡"]);
        assert!(immediate_side_child("Key/1|a|b", &list.address));
        assert!(!immediate_side_child("Key/1|a|b/1", &list.address), "a part is below its head");
        assert!(!immediate_side_child("Key/1|a|b|c", &list.address));
        assert!(!immediate_side_child("Key/1|a/1", &list.address));
        assert!(!immediate_side_child("Key/1|ab", &list.address));
        assert!(below("Key/1|a|b/1", "Key/1|a|b"));
        assert!(!below("Key/1|a|b", "Key/1|a|b"));
        assert!(!below("Key/1|a|bb", "Key/1|a|b"));
    }
}
