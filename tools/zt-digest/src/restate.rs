//! S08 (#107): resolve new restatements without rewriting a finished model (design §4.5, §5.4;
//! D083-D087, D109-D116).
//!
//! A restatement is one statement written again in a new Card while another Card already holds it.
//! This compiler sends bounded comparison pairs to the Card cycle, validates the reviewer's
//! `same`/`different`/`uncertain` answer, and carries an accepted `same` decision through one
//! rewrite of the **new** Card only: a distinct implication, or an explicit ordinary Idea
//! navigation Card whose `## 同述入口` section names the existing terminal substantive Card. The
//! existing statement keeps its home; a resolution that would need the existing Card changed stays
//! a pending resolution and writes nothing. Every write goes through the archive's identified
//! receipt path, is read back, and is checked, so a recorded verdict is never counted as an
//! applied merge.
//!
//! Steps:
//!
//! - `prepare -Cards <file>`: read the run's new Topic Idea Cards and the bounded comparison
//!   population (the book's anchors and Model Cards/parts, and earlier Topic Idea Cards), short at
//!   most ten pairs per new Card by the current relatedness score, normalized title similarity and
//!   exact shared-formula evidence, and freeze `evidence.json`, the retained endpoint bodies and the
//!   judge job. Read-only.
//! - `judge -File <answer>`: validate one reviewer answer against the frozen evidence and freeze
//!   `judged.json`. A refused answer writes nothing and resolves nothing.
//! - `apply -OperationPrefix <run/unit/rev/step>`: write the accepted new-Card rewrites through the
//!   identified path with the authored base, read every output back, check it, and record one
//!   explicit status per finding (`recommended`, `resolved-by-rewrite`,
//!   `pending-existing-card-change`). Only the listed new Cards are writable.
//! - `report`: render `restate-report.json`/`.md` from the frozen files, with examples of a verified
//!   resolution and of a pending one. It starts no client.
//!
//! The derived navigation role (`restatement-entry`) and the exact statement fingerprint live in the
//! archive's own Card read; this compiler never invents another classifier.

use crate::args::Args;
use crate::calls;
use crate::canon;
use crate::organize::common::{self, pstr};
use crate::ps;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// The evidence, judged, resolution and report schema.
const SCHEMA: i64 = 1;
/// The most comparison pairs one new Card gets (§5.4, D109).
const CANDIDATES: i64 = 10;
/// The most new Cards one review takes.
const MAX_NEW: usize = 200;
/// The most excluded Cards one review's population evidence lists by name; the rest are counted.
const MAX_SKIPPED: usize = 50;
/// The most Cards one book's Literature tree may hold before the bound refuses the review.
const MAX_BOOK_CARDS: usize = 4_000;
/// The most earlier Topic Cards read for one review (the title prefilter's union).
const MAX_EARLIER: usize = 400;
/// Earlier Topic Cards one new Card prefilters on its title before their bodies are read.
const PREFILTER: usize = 40;
/// The most judgments one answer file may hold.
const MAX_JUDGMENTS: usize = 2_000;
/// The near-title threshold the archive's I6 and the gate use.
const TITLE_SIMILARITY: f64 = 0.8;
/// Weighted terms kept per document, as the other relatedness callers do.
const KEEP: usize = 150;
/// The shortest sentence that states a Link's reason (`zt-check` L2).
const MIN_REASON: usize = 8;
/// The verified verdicts the reviewer may return (D110-D113).
const VERDICTS: [&str; 3] = ["same", "different", "uncertain"];
/// The resolutions an accepted `same` may name (D114, D115).
const REMEDIES: [&str; 3] = ["distinct-implication", "entry", "existing-card-change"];
/// The status of one finding (D115): a verdict is not an applied merge.
const RECOMMENDED: &str = "recommended";
const RESOLVED: &str = "resolved-by-rewrite";
const PENDING: &str = "pending-existing-card-change";
const UNVERIFIED: &str = "unverified";

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]]+)\]\]").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static LIST_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(?:[-*+]|\d+[.)])\s+").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{2,3}\s+(.*)$").unwrap());

fn f(value: &Value, key: &str) -> String {
    ps::s(common::prop(value, key))
}

fn refuse(message: impl Into<String>) -> anyhow::Error {
    canon::refuse(message.into())
}

// --- the command ----------------------------------------------------------------------------------

struct Params {
    step: String,
    staging: String,
    work: String,
    cards: String,
    file: String,
    prefix: String,
    merges: String,
    candidates: i64,
}

fn work_dir(staging: &str) -> PathBuf {
    Path::new(staging).join("restate")
}

/// The one copy the Card cycle's report reads, beside the staging folder's own files.
fn resolution_copy(staging: &str) -> PathBuf {
    Path::new(staging).join("restate-resolution.json")
}

fn evidence_path(staging: &str) -> PathBuf {
    work_dir(staging).join("evidence.json")
}

fn judged_path(staging: &str) -> PathBuf {
    work_dir(staging).join("judged.json")
}

fn resolution_path(staging: &str) -> PathBuf {
    work_dir(staging).join("resolution.json")
}

fn report_path(staging: &str) -> PathBuf {
    work_dir(staging).join("restate-report.json")
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = [
        "Step",
        "Staging",
        "Work",
        "Cards",
        "File",
        "OperationPrefix",
        "Merges",
        "MaxCandidates",
    ];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &[])?;
    let step = a.choice("Step", &["prepare", "judge", "apply", "report"])?;
    let staging = common::mandatory(&a, "Staging")?;
    let work = {
        let given = a.str("Work");
        if given.is_empty() {
            ps::join(&staging, "restate")
        } else {
            given
        }
    };
    let p = Params {
        step,
        staging,
        work,
        cards: a.str("Cards"),
        file: a.str("File"),
        prefix: a.str("OperationPrefix"),
        merges: a.str("Merges"),
        candidates: canon::integer_option(
            &a.str_or("MaxCandidates", &CANDIDATES.to_string()),
            "MaxCandidates",
            1,
            CANDIDATES,
        )?,
    };
    match p.step.as_str() {
        "prepare" => prepare(&p),
        "judge" => judge(&p),
        "apply" => apply(&p),
        _ => report(&p),
    }
}

// --- pure helpers ---------------------------------------------------------------------------------

/// One line: white-space runs made one space, cut to `max` UTF-16 units with `…`.
fn line(text: &str, max: usize) -> String {
    crate::concepts::line(text, max)
}

/// The Links of one body's `## 同述入口` section, in body order: the terminal reference an accepted
/// entry writes (§4.5, D084).
fn entry_links(body: &str) -> Vec<String> {
    let mut inside = false;
    let mut fenced = false;
    let mut links = Vec::new();
    for raw in body.lines() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if let Some(heading) = HEADING.captures(trimmed) {
            inside = heading[1].trim() == "同述入口";
            continue;
        }
        if !inside {
            continue;
        }
        if !LIST_ITEM.is_match(trimmed) {
            continue;
        }
        for found in LINK.captures_iter(trimmed) {
            let target = found[1].trim().to_string();
            if !target.is_empty() && !links.contains(&target) {
                links.push(target);
            }
        }
    }
    links
}

/// Whether one body has the entry section at all: the derived navigation role (D086).
fn is_entry_body(body: &str) -> bool {
    zt_check::graph::is_restatement_entry(body)
}

/// The source Links of one body's `来源：` lines.
fn sources(body: &str) -> Vec<String> {
    crate::concepts::sources(body)
}

fn title_similarity(a: &str, b: &str) -> f64 {
    crate::concepts::similarity(
        &crate::concepts::bigrams(a),
        &crate::concepts::bigrams(b),
    )
}

fn score_text(score: f64) -> String {
    crate::concepts::similarity_text(score)
}

fn pair_key(card: &str, candidate: &str) -> String {
    format!("{card}\u{1}{candidate}")
}

/// Whether a tree id is a Topic number (numeric) rather than a Literature Citation key.
fn topic_tree(tree: &str) -> bool {
    !tree.is_empty() && tree.chars().all(|c| c.is_ascii_digit())
}

/// One new Card of the review: the run's own work, and the only Card kind this slice may rewrite.
fn new_card_refusal(card: &Value) -> Option<String> {
    if f(card, "workspace") != "agent" {
        return Some("it is not an agent Card; this review writes only the agent Workspace".into());
    }
    if !topic_tree(&f(card, "tree")) {
        return Some("it is in a Literature tree, not a Topic tree".into());
    }
    if f(card, "type") != "idea" {
        return Some(format!(
            "its Card type is `{}`, not an `idea` claim",
            f(card, "type")
        ));
    }
    if f(card, "address") == format!("{}/0", f(card, "tree")) {
        return Some("it is a tree root, not a claim".into());
    }
    if card["restatement"] == json!(true) {
        return Some("it is already a restatement entry, not a claim".into());
    }
    None
}

// --- prepare --------------------------------------------------------------------------------------

/// One candidate population row: the catalogue entry plus what the review compared.
struct Candidate {
    address: String,
    title: String,
    kind: String,
    /// The Card that holds the statement when this candidate is itself an entry (D085).
    terminal: String,
    body: String,
    /// The archive's exact statement fingerprint of the Card (D087).
    fingerprint: String,
    /// The archive's authored fingerprint, which an identified edit uses as its base.
    authored: String,
}

/// The run's new Cards: the file `-Cards` names, or — when it names none — the run's own staged
/// `owned-gNN.txt` manifests, the same receipts-backed lists the Card cycle enumerates (S23 / #115).
/// A digest stage therefore needs no separate collector: the run's own ownership evidence is the
/// only population this review may rewrite. A caller's own list is bounded at `MAX_NEW`; the run's
/// own population can be larger, so its reviewable claims are selected from the title catalogue and
/// bounded there, with the omitted count recorded in the evidence (§12.2).
fn read_cards_file(path: &str, staging: &str) -> Result<Vec<String>> {
    let mut text = String::new();
    let source = if path.trim().is_empty() {
        for name in ps::child_files(staging, "owned-g", ".txt") {
            text.push_str(&util::read_text(&ps::join(staging, &name))?);
            text.push('\n');
        }
        format!("the staged owned-g*.txt manifests in `{staging}`")
    } else {
        text = util::read_text(path)?;
        format!("`-Cards {path}`")
    };
    let mut addresses: Vec<String> = Vec::new();
    for raw in text.lines() {
        let at = raw.trim();
        if at.is_empty() || at.starts_with('#') {
            continue;
        }
        let at = at.split_whitespace().next().unwrap_or_default().to_string();
        if !addresses.contains(&at) {
            addresses.push(at);
        }
    }
    if addresses.is_empty() {
        bail!("{source} names no new Card");
    }
    if !path.trim().is_empty() && addresses.len() > MAX_NEW {
        bail!(
            "a restatement review takes at most {MAX_NEW} new Cards, not {}",
            addresses.len()
        );
    }
    Ok(addresses)
}

/// The run's own population: the listed Cards that are reviewable Topic Idea claims, decided from
/// the title catalogue so a large owned set costs no body read. Each excluded Card keeps its reason;
/// the list is capped so the review stays inside its own bounds (its omissions are reported, never
/// silently dropped). Attached to the evidence as `population`.
fn population(trees: &[Value], owned: &[String]) -> (Vec<String>, Value) {
    let mut rows: BTreeMap<String, Value> = BTreeMap::new();
    for tree in trees {
        for row in ps::each(&tree["cards"]) {
            rows.insert(f(row, "address"), row.clone());
        }
    }
    let mut selected: Vec<String> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();
    let mut skipped_total = 0usize;
    for at in owned {
        let why = match rows.get(at) {
            None => Some("the archive does not hold it".to_string()),
            Some(row) => new_card_refusal(row),
        };
        match why {
            Some(why) => {
                skipped_total += 1;
                if skipped.len() < MAX_SKIPPED {
                    skipped.push(json!({"address": at, "why": why}));
                }
            }
            None => selected.push(at.clone()),
        }
    }
    selected.sort_by(|a, b| canon::ordinal_cmp(a, b));
    let reviewable = selected.len();
    selected.truncate(MAX_NEW);
    let omitted = reviewable - selected.len();
    (
        selected.clone(),
        json!({
            "owned": owned.len(),
            "reviewable": reviewable,
            "selected": selected.len(),
            "omitted": omitted,
            "skipped": skipped,
            "skipped_total": skipped_total,
        }),
    )
}

fn prepare(p: &Params) -> Result<i32> {
    let manifest = common::manifest(&p.staging)?;
    let key = pstr(&manifest, "citation_key");
    if key.is_empty() {
        bail!("manifest.json names no citation_key; a restatement review needs its book");
    }
    let cards = read_cards_file(&p.cards, &p.staging)?;
    std::fs::create_dir_all(ps::join(&p.work, "endpoints"))?;
    // The frozen evidence, the judged verdicts and the resolution live in the staging folder's own
    // `restate` folder whatever -Work names (the review module passes `<out>/review/restate`): create it too
    // (grossman2002integration trial 7: "系统找不到指定的路径" on evidence.json).
    std::fs::create_dir_all(work_dir(&p.staging))?;
    let mut zt = crate::zt::start()?;
    let mut attempt = 0;
    let (evidence, job) = loop {
        attempt += 1;
        match observe(&mut zt, p, &key, &cards) {
            Ok(frozen) => break frozen,
            Err(error) if crate::concepts::is_changing_view(&error) && attempt == 1 => continue,
            Err(error) => return Err(error),
        }
    };
    canon::save_json(&evidence_path(&p.staging).to_string_lossy(), &evidence)?;
    let job_path = ps::join(&p.work, "rj-01.md");
    util::write_atomic(&job_path, &job)?;
    let doc = util::read_text(ps::join(
        &common::scripts_dir(),
        "dsh-restate-instructions.zh.md",
    ))?;
    let doc_copy = ps::join(&p.work, "dsh-restate-instructions.zh.md");
    util::write_atomic(&doc_copy, &doc)?;
    let new_cards = ps::ps_items(&evidence["new_cards"]);
    let pairs: usize = new_cards
        .iter()
        .map(|card| ps::ps_items(&card["candidates"]).len())
        .sum();
    let summary = json!({
        "ok": true,
        "step": "prepare",
        "evidence_id": evidence["evidence_id"],
        "archive_id": evidence["archive"]["id"],
        "archive_revision": evidence["archive"]["revision"],
        "cards": cards.len(),
        "reviewable": new_cards.len(),
        "skipped": evidence["skipped"],
        "population": evidence.get("population").cloned().unwrap_or(Value::Null),
        "candidates": pairs,
        "unread": ps::ps_items(&evidence["unread"]).len(),
        "attempts": attempt,
        "evidence": evidence_path(&p.staging).to_string_lossy(),
        "jobs": [{ "id": "rj-01", "files": [doc_copy, job_path], "prompt": PROMPT, "answer": ps::join(&ps::join(&p.work, "out"), "rj-01.json") }],
    });
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

const PROMPT: &str = "按以上说明判断这些重述对，写出 JSON 文件，运行 SELF-CHECK 改到没有错误。最后只回复一行 JSON。";

/// One bounded read set of the review: the archive view, the title catalogue, the book's own Cards,
/// the earlier Topic Cards, and the new Cards it reviews.
fn observe(zt: &mut Zt, p: &Params, key: &str, cards: &[String]) -> Result<(Value, String)> {
    let view = zt.archive_view()?;
    let revision = view["archive_revision"]
        .as_u64()
        .context("incomplete restatement evidence: no archive revision")?;
    let trees = crate::snapshot::read_title_trees(zt)?;
    if trees
        .iter()
        .any(|tree| tree["archive_revision"].as_u64() != Some(revision))
    {
        return Err(crate::concepts::changing_view());
    }
    // The population: a caller's own list is read as given (a missing Card is a refusal, never an
    // absence); the run's own staged manifests are first narrowed to its reviewable claims and
    // bounded, so a large digest run still reviews a bounded set and reports its omissions.
    let population_mode = p.cards.trim().is_empty();
    let (selected, population) = if population_mode {
        population(&trees, cards)
    } else {
        (cards.to_vec(), Value::Null)
    };
    if population_mode && selected.is_empty() {
        bail!(
            "no new Card of the run's staged manifests ({}) is a Topic Idea Card this review can compare",
            cards.len()
        );
    }
    let read = zt.cards(&selected, "text")?;
    if read.archive_revision != revision {
        return Err(crate::concepts::changing_view());
    }
    let mut skipped: Vec<Value> = Vec::new();
    let mut new_cards: Vec<Value> = Vec::new();
    for card in &read.cards {
        let address = f(card, "address");
        match new_card_refusal(card) {
            Some(why) => skipped.push(json!({"address": address, "why": why})),
            None => new_cards.push(card.clone()),
        }
    }
    if new_cards.is_empty() {
        bail!(
            "no new Card of `{}` is a Topic Idea Card this review can compare",
            if population_mode { "the run's staged manifests" } else { p.cards.as_str() }
        );
    }
    let new_addresses: BTreeSet<String> = new_cards.iter().map(|card| f(card, "address")).collect();
    // The book's Literature tree: its anchors and its Model Cards and parts (D109).
    let mut book_rows: Vec<Value> = Vec::new();
    for tree in trees
        .iter()
        .filter(|tree| f(tree, "workspace") == "agent" && f(tree, "tree") == key)
    {
        for row in ps::each(&tree["cards"]) {
            if f(row, "edge") == "root" {
                continue;
            }
            let anchor = f(row, "address")
                .strip_prefix(&format!("{key}/"))
                .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()));
            if anchor || f(row, "type") == "model" {
                book_rows.push(row.clone());
            }
        }
    }
    if book_rows.len() > MAX_BOOK_CARDS {
        bail!(
            "a restatement review needs a bounded book: `{key}` holds {} claim Cards (limit {MAX_BOOK_CARDS})",
            book_rows.len()
        );
    }
    // Earlier Topic Idea Cards: title prefilter first, then bounded reads (the cheap comparison of
    // §7.1; a similarity is what to read next, never a verdict).
    let earlier_rows: Vec<Value> = trees
        .iter()
        .filter(|tree| f(tree, "workspace") == "agent" && topic_tree(&f(tree, "tree")))
        .flat_map(|tree| ps::each(&tree["cards"]))
        .filter(|row| {
            f(row, "edge") != "root"
                && f(row, "type") == "idea"
                && row["restatement"] != json!(true)
                && !new_addresses.contains(&f(row, "address"))
                && !f(row, "title").trim().is_empty()
        })
        .cloned()
        .collect();
    let mut earlier_reads: Vec<String> = Vec::new();
    for card in &new_cards {
        let title = f(card, "title");
        let mut scored: Vec<(f64, String)> = earlier_rows
            .iter()
            .map(|row| (title_similarity(&title, &f(row, "title")), f(row, "address")))
            .filter(|(score, _)| *score > 0.0)
            .collect();
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| canon::ordinal_cmp(&a.1, &b.1))
        });
        for (_, at) in scored.into_iter().take(PREFILTER) {
            if earlier_reads.len() >= MAX_EARLIER && !earlier_reads.contains(&at) {
                continue;
            }
            if !earlier_reads.contains(&at) {
                earlier_reads.push(at);
            }
        }
    }
    // Every body the comparison needs, in bounded batches at one revision.
    let mut wanted: Vec<String> = book_rows.iter().map(|row| f(row, "address")).collect();
    wanted.extend(earlier_reads.iter().cloned());
    dedup_sorted(&mut wanted);
    let mut unread: Vec<(String, String)> = Vec::new();
    let mut bodies: BTreeMap<String, Value> = BTreeMap::new();
    for card in crate::concepts::read_cards(zt, &wanted, revision, &mut unread, "restatement")? {
        bodies.insert(f(&card, "address"), card);
    }
    // The terminal of a candidate that is itself an entry: the substantive Card it names (D085).
    let terminals = restatement_terminals(zt)?;
    let mut candidates: Vec<Candidate> = Vec::new();
    for row in &book_rows {
        let address = f(row, "address");
        let Some(card) = bodies.get(&address) else {
            continue;
        };
        let anchor = f(row, "address")
            .strip_prefix(&format!("{key}/"))
            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()));
        candidates.push(Candidate {
            address: address.clone(),
            title: f(card, "title"),
            kind: if anchor {
                "anchor".into()
            } else if f(card, "facet").is_empty() {
                "model".into()
            } else {
                "part".into()
            },
            // An anchor is navigation: it may be compared, but it can never be the substantive
            // terminal an entry names (§4.5, D085).
            terminal: if anchor { String::new() } else { address.clone() },
            body: f(card, "body"),
            fingerprint: f(card, "statement_fingerprint"),
            authored: calls::authored_fingerprint(card),
        });
    }
    for address in &earlier_reads {
        let Some(card) = bodies.get(address) else {
            continue;
        };
        candidates.push(Candidate {
            address: address.clone(),
            title: f(card, "title"),
            kind: "topic-idea".into(),
            terminal: terminals.get(address).cloned().unwrap_or_else(|| address.clone()),
            body: f(card, "body"),
            fingerprint: f(card, "statement_fingerprint"),
            authored: calls::authored_fingerprint(card),
        });
    }
    // The current relatedness over the read corpus: the TF-IDF order of §5.4 (D109).
    let documents: Vec<(String, String)> = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.address.clone(),
                format!("{}\n{}", candidate.title, crate::concepts::statement(&candidate.body)),
            )
        })
        .collect();
    let relatedness = crate::card::relatedness::Relatedness::new(&documents, KEEP);
    let mut frozen_cards: Vec<Value> = Vec::new();
    let mut endpoints_wanted: BTreeSet<String> = new_addresses.clone();
    for card in &new_cards {
        let address = f(card, "address");
        let statement = crate::concepts::statement(&f(card, "body"));
        let title = f(card, "title");
        let query = format!("{title}\n{statement}");
        let scores: BTreeMap<String, f64> = relatedness
            .rank_text_scored(&query, candidates.len(), |_| false)
            .into_iter()
            .collect();
        let formulas = crate::concepts::formulas(&f(card, "body"));
        let mut scored: Vec<(bool, f64, f64, String, &Candidate, f64)> = Vec::new();
        for candidate in &candidates {
            let shared = formulas
                .iter()
                .any(|formula| candidate.body.contains(formula.as_str()));
            let tfidf = scores.get(&candidate.address).copied().unwrap_or(0.0);
            let similarity = title_similarity(&title, &candidate.title);
            if tfidf <= 0.0 && similarity <= 0.15 && !shared {
                continue;
            }
            scored.push((shared, tfidf, similarity, candidate.address.clone(), candidate, tfidf));
        }
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| canon::ordinal_cmp(&a.3, &b.3))
        });
        let selected: Vec<Value> = scored
            .into_iter()
            .take(p.candidates as usize)
            .map(|(shared, tfidf, similarity, address, candidate, _)| {
                let fingerprint = candidate.fingerprint.clone();
                endpoints_wanted.insert(candidate.address.clone());
                json!({
                    "address": address,
                    "title": candidate.title,
                    "kind": candidate.kind,
                    "terminal": candidate.terminal,
                    "statement": crate::concepts::statement(&candidate.body),
                    "assumptions": crate::concepts::assumptions(&candidate.body),
                    "formulas": crate::concepts::formulas(&candidate.body),
                    "sources": sources(&candidate.body),
                    "fingerprint": candidate.authored.clone(),
                    "statement_fingerprint": fingerprint,
                    "relatedness": score_text(tfidf),
                    "title_similarity": score_text(similarity),
                    "formula_match": shared,
                })
            })
            .collect();
        frozen_cards.push(json!({
            "address": address,
            "title": title,
            "statement": statement,
            "assumptions": crate::concepts::assumptions(&f(card, "body")),
            "formulas": formulas,
            "sources": sources(&f(card, "body")),
            "fingerprint": calls::authored_fingerprint(card),
            "statement_fingerprint": f(card, "statement_fingerprint"),
            "candidates": selected,
        }));
    }
    // The retained endpoint bodies: every new Card and every shortlisted candidate.
    let mut endpoints: Map<String, Value> = Map::new();
    for address in &endpoints_wanted {
        let Some(card) = bodies.get(address).or_else(|| read.cards.iter().find(|card| &f(card, "address") == address)) else {
            continue;
        };
        let (path, sha256) = crate::concepts::retain_endpoint(&p.work, card)?;
        endpoints.insert(
            address.clone(),
            json!({"title": f(card, "title"), "path": path, "sha256": sha256}),
        );
    }
    let unread_rows: Vec<Value> = unread
        .iter()
        .map(|(address, why)| json!({"address": address, "why": why}))
        .collect();
    // The closing probe: one revision, or this is not one view (D300).
    let closed = zt.archive_view()?;
    if closed["archive_revision"].as_u64() != Some(revision) {
        return Err(crate::concepts::changing_view());
    }
    let pairs: usize = frozen_cards
        .iter()
        .map(|card| ps::ps_items(&card["candidates"]).len())
        .sum();
    let anchors = candidates.iter().filter(|row| row.kind == "anchor").count();
    let models = candidates.iter().filter(|row| row.kind == "model").count();
    let parts = candidates.iter().filter(|row| row.kind == "part").count();
    let earlier = candidates.iter().filter(|row| row.kind == "topic-idea").count();
    let mut evidence = json!({
        "schema_version": SCHEMA,
        "created_at": canon::utc_now_o(),
        "archive": {"id": view["archive_identity"]["id"], "path": view["archive_identity"]["path"], "revision": revision},
        "book": key,
        "bounds": {"cards": MAX_NEW, "candidates": CANDIDATES, "prefilter": PREFILTER, "earlier": MAX_EARLIER, "book_cards": MAX_BOOK_CARDS, "judgments": MAX_JUDGMENTS, "title_similarity": TITLE_SIMILARITY, "related_keep": KEEP},
        "context": {"anchors": anchors, "models": models, "parts": parts, "earlier_topic_ideas": earlier, "read": wanted.len()},
        "new_cards": frozen_cards,
        "skipped": skipped,
        "unread": unread_rows,
        "endpoints": Value::Object(endpoints),
        "coverage": {"cards": new_cards.len(), "pairs": pairs},
        "evidence_id": Value::Null,
    });
    // The run's own population is a new, additive evidence field: `-Cards` mode keeps exactly the
    // payload an earlier run froze, so a resumed caller's evidence id does not change.
    if !population.is_null() {
        evidence["population"] = population;
    }
    let payload = evidence.clone();
    let id = format!(
        "e-{}",
        canon::content_hash(&payload, &["evidence_id", "created_at"])
    );
    evidence["evidence_id"] = json!(id);
    let job = job_text(&evidence, &p.work, &p.staging);
    Ok((evidence, job))
}

fn dedup_sorted(list: &mut Vec<String>) {
    list.sort_by(|a, b| canon::ordinal_cmp(a, b));
    list.dedup();
}

/// The terminal substantive Card of every confirmed restatement entry of the agent Workspace: the
/// register view's own resolution, never a second reading of the sections (D085).
fn restatement_terminals(zt: &mut Zt) -> Result<BTreeMap<String, String>> {
    let mut terminals = BTreeMap::new();
    let view = zt.call(
        "get_register",
        json!({"workspace": "agent", "entry_offset": 0, "entry_limit": 1}),
    )?;
    for row in ps::each(&view["restatements"]) {
        let entry = f(row, "entry");
        let terminal = f(row, "terminal");
        if !entry.is_empty() && !terminal.is_empty() {
            terminals.insert(entry, terminal);
        }
    }
    Ok(terminals)
}

/// The reviewer's job: both complete statements of every pair, the evidence that put the pair on the
/// shortlist, and the one answer shape this compiler validates.
fn job_text(evidence: &Value, work: &str, staging: &str) -> String {
    let mut sb = common::Sb::new();
    sb.line("按 `dsh-restate-instructions.zh.md` 判断下面的每一对卡片。");
    sb.line("");
    sb.line(format!("证据：`{}`", ps::join(work, "evidence.json")));
    sb.line(format!("答案写到：`{}`", ps::join(&ps::join(work, "out"), "rj-01.json")));
    sb.line("");
    for card in ps::each(&evidence["new_cards"]) {
        sb.line(format!(
            "## 新卡 `{}` {}",
            f(card, "address"),
            line(&f(card, "title"), 80)
        ));
        sb.line("");
        sb.line(format!("陈述：{}", f(card, "statement")));
        if !ps::ps_items(&card["assumptions"]).is_empty() {
            sb.line(format!(
                "假设：{}",
                ps::ps_items(&card["assumptions"])
                    .iter()
                    .map(ps::s)
                    .collect::<Vec<_>>()
                    .join("；")
            ));
        }
        if !ps::ps_items(&card["formulas"]).is_empty() {
            sb.line(format!(
                "公式：{}",
                ps::ps_items(&card["formulas"])
                    .iter()
                    .map(ps::s)
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        sb.line(format!(
            "来源：{}",
            ps::ps_items(&card["sources"])
                .iter()
                .map(ps::s)
                .collect::<Vec<_>>()
                .join(" ")
        ));
        sb.line("");
        for candidate in ps::each(&card["candidates"]) {
            sb.line(format!(
                "### 候选 `{}`（{}） {}",
                f(candidate, "address"),
                f(candidate, "kind"),
                line(&f(candidate, "title"), 80)
            ));
            let terminal = f(candidate, "terminal");
            if !terminal.is_empty() && terminal != f(candidate, "address") {
                sb.line(format!("终述卡：`{terminal}`"));
            }
            sb.line(format!("陈述：{}", f(candidate, "statement")));
            if !ps::ps_items(&candidate["assumptions"]).is_empty() {
                sb.line(format!(
                    "假设：{}",
                    ps::ps_items(&candidate["assumptions"])
                        .iter()
                        .map(ps::s)
                        .collect::<Vec<_>>()
                        .join("；")
                ));
            }
            if !ps::ps_items(&candidate["formulas"]).is_empty() {
                sb.line(format!(
                    "公式：{}",
                    ps::ps_items(&candidate["formulas"])
                        .iter()
                        .map(ps::s)
                        .collect::<Vec<_>>()
                        .join(" ")
                ));
            }
            sb.line(format!(
                "信号：relatedness {}，标题相似度 {}，共享公式 {}，同述指纹 {}",
                f(candidate, "relatedness"),
                f(candidate, "title_similarity"),
                if candidate["formula_match"] == json!(true) { "是" } else { "否" },
                if f(candidate, "statement_fingerprint") == f(card, "statement_fingerprint") { "是" } else { "否" },
            ));
            sb.line("");
        }
    }
    sb.line("## 答案");
    sb.line("");
    sb.line("```json");
    sb.line("{\"schema_version\":1,\"evidence_id\":\"<证据里的 evidence_id>\",\"judgments\":[");
    sb.line("  {\"card\":\"<新卡地址>\",\"candidate\":\"<候选地址>\",\"verdict\":\"same|different|uncertain\",\"canonical\":\"<same 时：终述卡地址>\",\"scope\":\"<same 时：假设与范围为何相同>\",\"reason\":\"<理由>\",\"evidence\":[\"<读过的地址>\"],\"remedy\":\"<same 时：distinct-implication|entry|existing-card-change>\",\"title\":\"<改写后的标题>\",\"body\":\"<改写后的正文>\",\"needs\":[\"<uncertain 时：缺什么>\"]}");
    sb.line("]}");
    sb.line("```");
    sb.line("");
    sb.line("`same` 必须给出 `scope` 与 `remedy`。`remedy` 为 `distinct-implication` 或 `entry` 时，`title` 与 `body` 必填：只改写新卡。`entry` 的正文要有 `## 同述入口` 一节，其中恰好一个 Link，指向终述卡。`remedy` 为 `existing-card-change` 时不改写任何卡。`different` 用 `reason` 说出区别；`uncertain` 用 `needs` 说出缺的证据。");
    sb.line("");
    sb.line(format!(
        "SELF-CHECK: zt-digest restate -Step judge -Staging \"{staging}\" -File \"{}\"",
        ps::join(&ps::join(work, "out"), "rj-01.json")
    ));
    sb.0
}

// --- judge ----------------------------------------------------------------------------------------

/// The prepared pairs: `(card, candidate)` in evidence order.
fn pairs(evidence: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for card in ps::each(&evidence["new_cards"]) {
        for candidate in ps::each(&card["candidates"]) {
            out.push((f(card, "address"), f(candidate, "address")));
        }
    }
    out
}

fn prepared_pair<'a>(evidence: &'a Value, card: &str, candidate: &str) -> Option<(&'a Value, &'a Value)> {
    // Addresses are exact: a Citation key is case-significant, so a case-folded match would pair
    // two different Cards.
    let cards = evidence["new_cards"].as_array()?;
    let new_card = cards.iter().find(|row| f(row, "address") == *card)?;
    let candidates = new_card["candidates"].as_array()?;
    let found = candidates.iter().find(|row| f(row, "address") == *candidate)?;
    Some((new_card, found))
}

/// The addresses one pair may cite: the two statements and the terminal they name.
fn pair_endpoints(card: &Value, candidate: &Value) -> Vec<String> {
    let mut out = vec![f(card, "address"), f(candidate, "address")];
    let terminal = f(candidate, "terminal");
    if !terminal.is_empty() && !out.contains(&terminal) {
        out.push(terminal);
    }
    out
}

/// The accepted source Links of one new Card must survive its rewrite: the entry keeps the source
/// evidence it was written from. A rewrite that drops a source is refused, not silently checked.
fn keeps_sources(accepted: &str, wanted: &[String]) -> bool {
    if wanted.is_empty() {
        return true;
    }
    let kept = sources(accepted);
    wanted.iter().all(|source| kept.iter().any(|seen| ps::eq(seen, source)))
}

/// One Link's sentence in an accepted body states a reason (`zt-check` L2's rule).
fn linked_with_reason(body: &str, target: &str) -> bool {
    for line in body.lines() {
        if !line.contains("[[") {
            continue;
        }
        for sentence in line.split(['。', '！', '？', '!', '?', '.']) {
            if !sentence.contains(&format!("[[{target}]]")) {
                continue;
            }
            let plain = LINK.replace_all(sentence, "");
            let plain: String = plain
                .chars()
                .filter(|c| !matches!(c, '$' | '*'))
                .collect();
            if SPACES.replace_all(plain.trim(), " ").trim().chars().count() >= MIN_REASON {
                return true;
            }
        }
    }
    false
}

fn validate_answer(evidence: &Value, answer: &Value) -> Result<Value> {
    let schema = answer["schema_version"].as_i64().unwrap_or_default();
    if schema != SCHEMA {
        return Err(refuse(format!(
            "the answer's schema_version is `{schema}`; this review writes {SCHEMA}"
        )));
    }
    if f(answer, "evidence_id") != f(evidence, "evidence_id") {
        return Err(refuse(format!(
            "the answer judges evidence `{}`, not `{}`",
            f(answer, "evidence_id"),
            f(evidence, "evidence_id")
        )));
    }
    let allowed: HashSet<&str> = [
        "card",
        "candidate",
        "verdict",
        "canonical",
        "scope",
        "reason",
        "evidence",
        "remedy",
        "title",
        "body",
        "needs",
    ]
    .into_iter()
    .collect();
    let judgments = ps::ps_items(&answer["judgments"]);
    if judgments.is_empty() {
        return Err(refuse("the answer holds no judgment; an empty answer resolves nothing"));
    }
    if judgments.len() > MAX_JUDGMENTS {
        return Err(refuse(format!(
            "the answer holds {} judgments; at most {MAX_JUDGMENTS} are accepted",
            judgments.len()
        )));
    }
    let prepared = pairs(evidence);
    let mut seen: BTreeMap<String, Value> = BTreeMap::new();
    let mut rewritten: BTreeMap<String, String> = BTreeMap::new();
    let mut accepted: Vec<Value> = Vec::new();
    for (index, judgment) in judgments.iter().enumerate() {
        let Some(map) = judgment.as_object() else {
            return Err(refuse(format!("judgment {index} is not an object")));
        };
        for key in map.keys() {
            if !allowed.contains(key.as_str()) {
                return Err(refuse(format!("judgment {index} names the unknown field `{key}`")));
            }
        }
        let card = f(judgment, "card");
        let candidate = f(judgment, "candidate");
        if !prepared
            .iter()
            .any(|(known_card, known_candidate)| *known_card == card && *known_candidate == candidate)
        {
            return Err(refuse(format!(
                "judgment {index} names the pair `{card}` / `{candidate}`, which the evidence did not prepare"
            )));
        }
        let Some((new_card, prepared_candidate)) = prepared_pair(evidence, &card, &candidate) else {
            return Err(refuse(format!("judgment {index} names an unprepared pair")));
        };
        let key = pair_key(&ps::key(&card), &ps::key(&candidate));
        if let Some(earlier) = seen.get(&key) {
            if earlier == judgment {
                continue;
            }
            return Err(refuse(format!(
                "the pair `{card}` / `{candidate}` is judged twice with different outcomes; one pair has one verdict"
            )));
        }
        seen.insert(key, judgment.clone());
        let verdict = f(judgment, "verdict");
        if !VERDICTS.iter().any(|known| ps::eq(known, &verdict)) {
            return Err(refuse(format!(
                "judgment {index} names the unknown verdict `{verdict}`"
            )));
        }
        let endpoints = pair_endpoints(new_card, prepared_candidate);
        for cited in ps::ps_items(&judgment["evidence"]).iter().map(ps::s) {
            if !endpoints.iter().any(|known| ps::eq(known, &cited)) {
                return Err(refuse(format!(
                    "judgment {index} cites `{cited}`, which this pair did not read"
                )));
            }
        }
        let reason = f(judgment, "reason");
        if reason.trim().chars().count() < MIN_REASON {
            return Err(refuse(format!(
                "judgment {index} gives no reason for its verdict"
            )));
        }
        let canonical = f(judgment, "canonical");
        let remedy = f(judgment, "remedy");
        let title = f(judgment, "title");
        let body = f(judgment, "body");
        let scope = f(judgment, "scope");
        let needs: Vec<String> = ps::ps_items(&judgment["needs"]).iter().map(ps::s).collect();
        match verdict.as_str() {
            "same" => {
                if canonical.is_empty() || !ps::eq(&canonical, &card) && !ps::eq(&canonical, &candidate) {
                    return Err(refuse(format!(
                        "judgment {index}: a `same` verdict names the canonical statement, one of the two Cards"
                    )));
                }
                if scope.trim().is_empty() {
                    return Err(refuse(format!(
                        "judgment {index}: a `same` verdict states the scope and assumptions that coincide"
                    )));
                }
                if !REMEDIES.iter().any(|known| ps::eq(known, &remedy)) {
                    return Err(refuse(format!(
                        "judgment {index}: a `same` verdict names a remedy (`{}`); `{remedy}` is none",
                        REMEDIES.join("`, `")
                    )));
                }
                if !needs.is_empty() {
                    return Err(refuse(format!(
                        "judgment {index}: a `same` verdict names no `needs`"
                    )));
                }
                if ps::eq(&canonical, &card) && !ps::eq(&remedy, "existing-card-change") {
                    return Err(refuse(format!(
                        "judgment {index}: the new Card is the canonical statement, so the other Card would change; name `existing-card-change`"
                    )));
                }
                if ps::eq(&remedy, "existing-card-change") {
                    if !title.is_empty() || !body.is_empty() {
                        return Err(refuse(format!(
                            "judgment {index}: an `existing-card-change` resolution rewrites no Card, so it carries no text"
                        )));
                    }
                    accepted.push(json!({
                        "card": card, "candidate": candidate, "verdict": verdict,
                        "canonical": canonical, "terminal": terminal_of(evidence, &canonical, &candidate),
                        "scope": scope, "reason": reason, "remedy": remedy,
                        "evidence": judgment["evidence"].clone(), "status": PENDING,
                        "why": "the accepted resolution changes the existing Card; a new pilot may not rewrite it",
                    }));
                    continue;
                }
                if title.trim().is_empty() {
                    return Err(refuse(format!(
                        "judgment {index}: the accepted rewrite names no title"
                    )));
                }
                if body.trim().is_empty() {
                    return Err(refuse(format!(
                        "judgment {index}: the accepted rewrite states no text"
                    )));
                }
                // The Card heading already carries the title; a body that repeats it is refused by
                // the archive's own write path, so the reviewer must not write it.
                let first = body.lines().find(|line| !line.trim().is_empty()).unwrap_or_default();
                let first = first.trim().trim_start_matches('#').trim();
                if !title.trim().is_empty() && ps::eq(first, title.trim()) {
                    return Err(refuse(format!(
                        "judgment {index}: the body repeats its title; keep the title only in the Card heading"
                    )));
                }
                let terminal = terminal_of(evidence, &canonical, &candidate);
                if ps::eq(&remedy, "entry") {
                    if terminal.is_empty() {
                        return Err(refuse(format!(
                            "judgment {index}: `{canonical}` is an anchor, which is navigation; an entry names the substantive Card that holds the statement, so name a distinct implication instead"
                        )));
                    }
                    let links = entry_links(&body);
                    if links.len() != 1 {
                        return Err(refuse(format!(
                            "judgment {index}: the entry's 同述入口 section names {} Cards; it names exactly one terminal substantive Card",
                            links.len()
                        )));
                    }
                    if !ps::eq(&links[0], &terminal) {
                        return Err(refuse(format!(
                            "judgment {index}: the entry names `{}`, not the terminal substantive Card `{terminal}`",
                            links[0]
                        )));
                    }
                } else {
                    if is_entry_body(&body) {
                        return Err(refuse(format!(
                            "judgment {index}: a distinct implication carries no 同述入口 section; that section is the entry resolution"
                        )));
                    }
                    // The implication links the statement's home: the terminal of an entry, or the
                    // anchor itself when it is the candidate the reviewer read.
                    let home = if terminal.is_empty() { canonical.clone() } else { terminal.clone() };
                    if !linked_with_reason(&body, &home) {
                        return Err(refuse(format!(
                            "judgment {index}: a distinct implication links `{home}` with a reason in the same sentence"
                        )));
                    }
                }
                let wanted_sources: Vec<String> = ps::ps_items(&new_card["sources"])
                    .iter()
                    .map(ps::s)
                    .collect();
                if !keeps_sources(&body, &wanted_sources) {
                    return Err(refuse(format!(
                        "judgment {index}: the accepted rewrite drops the new Card's source evidence ({})",
                        wanted_sources.join(", ")
                    )));
                }
                if let Some(other) = rewritten.get(&card) {
                    return Err(refuse(format!(
                        "judgment {index}: `{card}` is already resolved by another pair (`{other}`); one new Card has one rewrite"
                    )));
                }
                rewritten.insert(card.clone(), candidate.clone());
                accepted.push(json!({
                    "card": card, "candidate": candidate, "verdict": verdict,
                    "canonical": canonical, "terminal": terminal,
                    "scope": scope, "reason": reason, "remedy": remedy,
                    "title": title, "body": body,
                    "evidence": judgment["evidence"].clone(), "status": RECOMMENDED,
                }));
                continue;
            }
            "different" => {
                if !canonical.is_empty() || !remedy.is_empty() || !title.is_empty() || !body.is_empty() {
                    return Err(refuse(format!(
                        "judgment {index}: a `different` verdict states the distinction and changes no Card"
                    )));
                }
                accepted.push(json!({
                    "card": card, "candidate": candidate, "verdict": verdict,
                    "reason": reason, "evidence": judgment["evidence"].clone(), "status": "different",
                }));
            }
            _ => {
                if !canonical.is_empty() || !remedy.is_empty() || !title.is_empty() || !body.is_empty() {
                    return Err(refuse(format!(
                        "judgment {index}: an `uncertain` verdict names the missing evidence and changes no Card"
                    )));
                }
                if needs.is_empty() {
                    return Err(refuse(format!(
                        "judgment {index}: an `uncertain` verdict names the missing source or premise in `needs`"
                    )));
                }
                accepted.push(json!({
                    "card": card, "candidate": candidate, "verdict": verdict,
                    "reason": reason, "needs": needs, "evidence": judgment["evidence"].clone(), "status": "unresolved",
                }));
            }
        }
    }
    let judged = accepted.len();
    let unjudged: Vec<Value> = prepared
        .iter()
        .filter(|(card, candidate)| {
            !seen.contains_key(&pair_key(&ps::key(card), &ps::key(candidate)))
        })
        .map(|(card, candidate)| json!({"card": card, "candidate": candidate}))
        .collect();
    let mut value = json!({
        "schema_version": SCHEMA,
        "evidence_id": evidence["evidence_id"],
        "judged": judged,
        "unjudged": unjudged.len(),
        "unjudged_pairs": unjudged,
        "judgments": accepted,
        "judged_id": Value::Null,
    });
    let payload = value.clone();
    value["judged_id"] = json!(format!(
        "j-{}",
        canon::content_hash(&payload, &["judged_id"])
    ));
    Ok(value)
}

/// The terminal substantive Card an accepted canonical names: the canonical itself, or the terminal
/// the prepared evidence recorded for an existing entry (D085).
fn terminal_of(evidence: &Value, canonical: &str, candidate: &str) -> String {
    for card in ps::each(&evidence["new_cards"]) {
        for row in ps::each(&card["candidates"]) {
            if f(row, "address") == *candidate {
                if canonical == candidate || canonical == f(card, "address") {
                    return f(row, "terminal");
                }
            }
        }
    }
    canonical.to_string()
}

fn judge(p: &Params) -> Result<i32> {
    let evidence = read_json(&evidence_path(&p.staging))?;
    verify_evidence(&evidence)?;
    if p.file.trim().is_empty() {
        bail!("judge needs -File: the reviewer's answer file");
    }
    let answer = canon::read_json(&p.file)?;
    let judged = validate_answer(&evidence, &answer)?;
    canon::save_json(&judged_path(&p.staging).to_string_lossy(), &judged)?;
    let summary = json!({
        "ok": true,
        "step": "judge",
        "evidence_id": evidence["evidence_id"],
        "judged_id": judged["judged_id"],
        "judged": judged["judged"],
        "unjudged": judged["unjudged"],
        "file": p.file,
    });
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

// --- apply ----------------------------------------------------------------------------------------

fn apply(p: &Params) -> Result<i32> {
    if p.prefix.trim().is_empty() {
        bail!("apply needs -OperationPrefix <run/unit/positive-revision/step>");
    }
    let evidence = read_json(&evidence_path(&p.staging))?;
    verify_evidence(&evidence)?;
    let judged = read_json(&judged_path(&p.staging))?;
    if f(&judged, "evidence_id") != f(&evidence, "evidence_id") {
        return Err(refuse("the judged answers belong to another evidence set"));
    }
    let key = f(&evidence, "book");
    let mut zt = crate::zt::start()?;
    let revision = zt.cards(&[], "text")?.archive_revision;
    let new_addresses: BTreeSet<String> = ps::ps_items(&evidence["new_cards"])
        .iter()
        .map(|card| f(card, "address"))
        .collect();
    // The accepted rewrites: `edit_card` on the new Card only, with the authored base the evidence
    // froze, so an independent edit between prepare and apply conflicts instead of being overwritten.
    let mut calls: Vec<Value> = Vec::new();
    let mut planned: Vec<Value> = Vec::new();
    for judgment in ps::each(&judged["judgments"]) {
        if f(judgment, "status") != RECOMMENDED {
            continue;
        }
        let card = f(judgment, "card");
        let Some(new_card) = ps::ps_items(&evidence["new_cards"])
            .into_iter()
            .find(|row| ps::eq(&f(row, "address"), &card))
        else {
            return Err(refuse(format!("the accepted rewrite names `{card}`, which the evidence did not review")));
        };
        calls.push(json!({
            "tool": "edit_card",
            "arguments": {
                "at": card,
                "title": f(judgment, "title"),
                "body": f(judgment, "body"),
                "base_fingerprint": f(&new_card, "fingerprint"),
            }
        }));
        planned.push(judgment.clone());
    }
    let watched_pre = {
        let mut watched: Vec<String> = Vec::new();
        for judgment in &planned {
            for address in [f(judgment, "canonical"), f(judgment, "terminal")] {
                if !address.is_empty() && !watched.contains(&address) {
                    watched.push(address);
                }
            }
        }
        watched
            .into_iter()
            .filter(|address| !new_addresses.contains(address))
            .collect::<Vec<_>>()
    };
    let before = if watched_pre.is_empty() {
        Vec::new()
    } else {
        zt.cards(&watched_pre, "text")?.cards
    };
    let receipts = if calls.is_empty() {
        Vec::new()
    } else {
        let allow: HashSet<String> = new_addresses.iter().cloned().collect();
        calls::invoke_identified(&mut zt, &calls, &p.prefix, Some(&allow))?
    };
    // Read every output back and check it: the receipt is not the evidence (D115).
    let after = if new_addresses.is_empty() {
        Vec::new()
    } else {
        zt.cards(&planned.iter().map(|row| f(row, "card")).collect::<Vec<_>>(), "text")?
            .cards
    };
    let mut findings: Vec<Value> = Vec::new();
    let mut resolved: BTreeMap<String, String> = BTreeMap::new();
    for judgment in &planned {
        let card = f(judgment, "card");
        let Some(read) = after.iter().find(|row| f(row, "address") == card) else {
            return Err(refuse(format!(
                "the rewrite of `{card}` returned no Card to read back"
            )));
        };
        let receipt = receipts
            .iter()
            .find(|receipt| f(&receipt["result"], "address") == card);
        let expected = receipt
            .map(|receipt| f(&receipt["result"], "after_fingerprint"))
            .unwrap_or_default();
        let fingerprint = calls::authored_fingerprint(read);
        let mut faults: Vec<String> = Vec::new();
        if f(read, "title") != f(judgment, "title") || f(read, "body") != f(judgment, "body") {
            faults.push("the read-back Card does not carry the accepted text".to_string());
        }
        if !expected.is_empty() && expected != fingerprint {
            faults.push("the read-back Card does not match its receipt".to_string());
        }
        if ps::eq(&f(judgment, "remedy"), "entry") && read["restatement"] != json!(true) {
            faults.push("the read-back entry has no 同述入口 section".to_string());
        }
        // The archive's own checker has the last word on the rewritten Card.
        let mut check = crate::check::Check::default();
        check.cards = card.clone();
        check.book = key.clone();
        match check.run(&mut zt) {
            Ok(lines) => {
                for fault in lines.iter().filter(|line| {
                    !line.contains("(advisory)") && !line.starts_with("faults: ")
                }) {
                    faults.push(fault.clone());
                }
            }
            Err(error) => faults.push(format!("the check could not read the Card: {error:#}")),
        }
        let status = if faults.is_empty() { RESOLVED } else { UNVERIFIED };
        if status == RESOLVED {
            resolved.insert(card.clone(), f(judgment, "candidate"));
        }
        findings.push(json!({
            "card": card,
            "candidate": f(judgment, "candidate"),
            "canonical": f(judgment, "canonical"),
            "terminal": f(judgment, "terminal"),
            "verdict": "same",
            "remedy": f(judgment, "remedy"),
            "reason": f(judgment, "reason"),
            "scope": f(judgment, "scope"),
            "evidence": judgment["evidence"],
            "status": status,
            "receipt": receipt.map(|receipt| receipt["op_key"].clone()).unwrap_or(Value::Null),
            "after_fingerprint": if expected.is_empty() { Value::Null } else { json!(expected) },
            "read_back_fingerprint": json!(fingerprint),
            "checked": faults.is_empty(),
            "faults": faults,
        }));
    }
    // The existing statements keep their home: no watched Card changed.
    for card in &before {
        let address = f(card, "address");
        let Some(read) = zt.cards(&[address.clone()], "text")?.cards.first().cloned() else {
            return Err(refuse(format!(
                "the existing statement `{address}` disappeared during the rewrite"
            )));
        };
        if calls::authored_fingerprint(&read) != calls::authored_fingerprint(card) {
            return Err(refuse(format!(
                "the existing statement `{address}` changed; a restatement resolution rewrites only the new Card"
            )));
        }
    }
    // Every other accepted finding: its own status; a merge verdict stays a pending resolution.
    let mut findings_all: Vec<Value> = findings;
    let mut solved_pairs: BTreeMap<String, String> = BTreeMap::new();
    for (card, candidate) in &resolved {
        solved_pairs.insert(card.clone(), candidate.clone());
    }
    for judgment in ps::each(&judged["judgments"]) {
        let status = f(judgment, "status");
        if status == RECOMMENDED {
            continue;
        }
        findings_all.push(json!({
            "card": f(judgment, "card"),
            "candidate": f(judgment, "candidate"),
            "canonical": f(judgment, "canonical"),
            "terminal": f(judgment, "terminal"),
            "verdict": f(judgment, "verdict"),
            "remedy": f(judgment, "remedy"),
            "reason": f(judgment, "reason"),
            "scope": f(judgment, "scope"),
            "needs": judgment["needs"],
            "evidence": judgment["evidence"],
            "status": status,
            "why": judgment["why"],
            "checked": Value::Null,
            "faults": [],
        }));
    }
    // The recorded merge verdicts of the Card cycle: pending until a rewrite covers them, and never
    // counted as an applied merge (D115, D116).
    let mut merges: Vec<Value> = Vec::new();
    if !p.merges.trim().is_empty() {
        let decisions = read_json(Path::new(&p.merges))?;
        for merge in ps::each(&decisions["merges"]) {
            let address = f(merge, "address");
            let target = f(merge, "merge_with");
            let covered = solved_pairs
                .iter()
                .any(|(card, canonical)| *card == address && *canonical == target)
                || findings_all.iter().any(|row| {
                    ps::eq(&f(row, "status"), RESOLVED)
                        && f(row, "card") == address
                        && (f(row, "candidate") == target || f(row, "terminal") == target)
                });
            merges.push(json!({
                "address": address,
                "merge_with": target,
                "note": f(merge, "note"),
                "status": if covered { RESOLVED } else { PENDING },
                "why": if covered {
                    "the new Card's rewrite carries this pair, so the verdict is resolved by the rewrite and is not an applied merge"
                } else {
                    "the recorded merge verdict names an existing Card change; no rewrite covers it, so it stays pending"
                },
            }));
        }
    }
    let final_revision = zt.cards(&[], "text")?.archive_revision;
    let mut counts: BTreeMap<&str, i64> = BTreeMap::new();
    for row in &findings_all {
        *counts.entry(status_of(row)).or_default() += 1;
    }
    for row in &merges {
        // A covered merge verdict is the same pair the rewrite already resolved: it is recorded
        // with that status but not counted a second time.
        if status_of(row) == RESOLVED {
            continue;
        }
        *counts.entry(status_of(row)).or_default() += 1;
    }
    let resolution = json!({
        "schema_version": SCHEMA,
        "evidence_id": evidence["evidence_id"],
        "judged_id": judged["judged_id"],
        "archive": {"id": evidence["archive"]["id"], "revision_before": revision, "revision_after": final_revision},
        "prefix": p.prefix,
        "findings": findings_all,
        "merges": merges,
        "counts": {
            "recommended": counts.get(RECOMMENDED).copied().unwrap_or(0),
            "resolved_by_rewrite": counts.get(RESOLVED).copied().unwrap_or(0),
            "pending_existing_card_change": counts.get(PENDING).copied().unwrap_or(0),
            "unverified": counts.get(UNVERIFIED).copied().unwrap_or(0),
            "different": counts.get("different").copied().unwrap_or(0),
            "unresolved": counts.get("unresolved").copied().unwrap_or(0),
        },
        "complete": counts.get(UNVERIFIED).copied().unwrap_or(0) == 0,
    });
    canon::save_json(&resolution_path(&p.staging).to_string_lossy(), &resolution)?;
    canon::save_json(&resolution_copy(&p.staging).to_string_lossy(), &resolution)?;
    let summary = json!({
        "ok": true,
        "step": "apply",
        "evidence_id": evidence["evidence_id"],
        "judged_id": judged["judged_id"],
        "prefix": p.prefix,
        "written": calls.len(),
        "counts": resolution["counts"],
        "complete": resolution["complete"],
        "resolution": resolution_path(&p.staging).to_string_lossy(),
    });
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

fn status_of(row: &Value) -> &str {
    row["status"].as_str().unwrap_or_default()
}

// --- report ---------------------------------------------------------------------------------------

fn report(p: &Params) -> Result<i32> {
    let evidence = read_json(&evidence_path(&p.staging))?;
    verify_evidence(&evidence)?;
    let judged = if ps::exists(&judged_path(&p.staging).to_string_lossy()) {
        Some(read_json(&judged_path(&p.staging))?)
    } else {
        None
    };
    let resolution = if ps::exists(&resolution_path(&p.staging).to_string_lossy()) {
        Some(read_json(&resolution_path(&p.staging))?)
    } else {
        None
    };
    let payload = report_payload(&evidence, judged.as_ref(), resolution.as_ref());
    let mut report = payload.clone();
    report["report_id"] = json!(format!(
        "rr-{}",
        canon::content_hash(&payload, &["report_id", "created_at"])
    ));
    canon::save_json(&report_path(&p.staging).to_string_lossy(), &report)?;
    let markdown = ps::join(&p.work, "restate-report.md");
    util::write_atomic(&markdown, &render_markdown(&report))?;
    let summary = json!({
        "ok": true,
        "step": "report",
        "report_id": report["report_id"],
        "status": report["status"],
        "complete": report["complete"],
        "counts": report["counts"],
        "written": [report_path(&p.staging).to_string_lossy(), markdown],
    });
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

/// Every prepared pair with the status the frozen files support: nothing is recommended over an
/// unjudged pair, and a `same` verdict without a carried rewrite stays `recommended` (D115).
fn report_payload(evidence: &Value, judged: Option<&Value>, resolution: Option<&Value>) -> Value {
    let mut findings: Vec<Value> = Vec::new();
    let mut resolved: BTreeMap<String, Value> = BTreeMap::new();
    if let Some(resolution) = resolution {
        for row in ps::each(&resolution["findings"]) {
            resolved.insert(
                pair_key(&ps::key(&f(row, "card")), &ps::key(&f(row, "candidate"))),
                row.clone(),
            );
        }
        for row in ps::each(&resolution["merges"]) {
            resolved.insert(format!("merge\u{1}{}", ps::key(&f(row, "address"))), row.clone());
        }
    }
    let judgments: Vec<Value> = judged
        .map(|judged| ps::ps_items(&judged["judgments"]))
        .unwrap_or_default();
    for card in ps::each(&evidence["new_cards"]) {
        for candidate in ps::each(&card["candidates"]) {
            let key = pair_key(&ps::key(&f(card, "address")), &ps::key(&f(candidate, "address")));
            let judgment = judgments.iter().find(|judgment| {
                ps::eq(&f(judgment, "card"), &f(card, "address"))
                    && ps::eq(&f(judgment, "candidate"), &f(candidate, "address"))
            });
            let row = resolved.get(&key);
            let (verdict, status, reason, needs, evidence_cited, scope, remedy, typed) = match judgment {
                None => (
                    String::new(),
                    "unjudged".to_string(),
                    "the reviewer judged no pair".to_string(),
                    Vec::new(),
                    Vec::new(),
                    String::new(),
                    String::new(),
                    None,
                ),
                Some(judgment) => {
                    let verdict = f(judgment, "verdict");
                    let status = match row {
                        Some(row) => f(row, "status"),
                        None => match verdict.as_str() {
                            "same" => RECOMMENDED.to_string(),
                            "different" => "different".to_string(),
                            _ => "unresolved".to_string(),
                        },
                    };
                    (
                        verdict,
                        status,
                        f(judgment, "reason"),
                        ps::ps_items(&judgment["needs"]),
                        ps::ps_items(&judgment["evidence"]),
                        f(judgment, "scope"),
                        f(judgment, "remedy"),
                        row.cloned(),
                    )
                }
            };
            findings.push(json!({
                "card": f(card, "address"),
                "card_title": f(card, "title"),
                "candidate": f(candidate, "address"),
                "candidate_title": f(candidate, "title"),
                "candidate_kind": f(candidate, "kind"),
                "terminal": f(candidate, "terminal"),
                "verdict": verdict,
                "status": status,
                "reason": reason,
                "scope": scope,
                "remedy": remedy,
                "needs": needs,
                "evidence": evidence_cited,
                "formula_match": candidate["formula_match"],
                "title_similarity": candidate["title_similarity"],
                "relatedness": candidate["relatedness"],
                "exact_statement": f(candidate, "statement_fingerprint") == f(card, "statement_fingerprint"),
                "checked": typed.as_ref().map(|row| row["checked"].clone()).unwrap_or(Value::Null),
                "faults": typed.as_ref().map(|row| row["faults"].clone()).unwrap_or(json!([])),
            }));
        }
    }
    // A recorded merge verdict is part of this report even when no rewrite was applied. A verdict
    // a rewrite already covers is the same pair the findings name, so it is not listed twice.
    if let Some(resolution) = resolution {
        for row in ps::each(&resolution["merges"]) {
            let covered = ps::eq(&f(row, "status"), RESOLVED)
                && findings.iter().any(|finding| {
                    ps::eq(&f(finding, "card"), &f(row, "address"))
                        && ps::eq(&f(finding, "candidate"), &f(row, "merge_with"))
                });
            if covered {
                continue;
            }
            findings.push(json!({
                "card": f(&row, "address"),
                "card_title": Value::Null,
                "candidate": f(&row, "merge_with"),
                "candidate_title": Value::Null,
                "candidate_kind": Value::Null,
                "terminal": Value::Null,
                "verdict": "merge",
                "status": f(&row, "status"),
                "reason": f(&row, "why"),
                "scope": Value::Null,
                "remedy": "existing-card-change",
                "needs": [],
                "evidence": [],
                "formula_match": Value::Null,
                "title_similarity": Value::Null,
                "relatedness": Value::Null,
                "exact_statement": Value::Null,
                "checked": Value::Null,
                "faults": [],
            }));
        }
    }
    findings.sort_by(|a, b| {
        canon::ordinal_cmp(&f(a, "card"), &f(b, "card"))
            .then_with(|| canon::ordinal_cmp(&f(a, "candidate"), &f(b, "candidate")))
    });
    let count = |status: &str| -> i64 {
        findings
            .iter()
            .filter(|row| ps::eq(&f(row, "status"), status))
            .count() as i64
    };
    let pairs = findings.len() as i64;
    let pending = count(PENDING);
    let recommended = count(RECOMMENDED);
    let complete = pairs > 0 && count("unjudged") == 0 && count(UNVERIFIED) == 0;
    let status = if !complete {
        "partial"
    } else if pending > 0 {
        "resolved-with-pending"
    } else if recommended > 0 {
        "recommended"
    } else {
        "resolved"
    };
    json!({
        "schema_version": SCHEMA,
        "created_at": canon::utc_now_o(),
        "archive": evidence["archive"],
        "book": evidence["book"],
        "bounds": evidence["bounds"],
        "coverage": evidence["coverage"],
        "findings": findings,
        "counts": {
            "pairs": pairs,
            "recommended": recommended,
            "resolved_by_rewrite": count(RESOLVED),
            "pending_existing_card_change": pending,
            "unverified": count(UNVERIFIED),
            "different": count("different"),
            "unresolved": count("unresolved"),
            "unjudged": count("unjudged"),
        },
        "complete": complete,
        "status": status,
        "guarantee": "A recorded verdict is not an applied merge: only a rewrite that was written through the receipt path, read back and checked is `resolved-by-rewrite`; a resolution that would need the existing Card changed stays `pending-existing-card-change`, and existing duplicates remain report-only.",
        "report_id": Value::Null,
    })
}

fn render_markdown(report: &Value) -> String {
    let mut sb = common::Sb::new();
    sb.line(format!(
        "# 重述复核 {}",
        report["report_id"].as_str().unwrap_or_default()
    ));
    sb.line("");
    sb.line(format!(
        "- 书：`{}`；归档修订：{}；状态：{}",
        f(report, "book"),
        report["archive"]["revision"],
        f(report, "status")
    ));
    let counts = &report["counts"];
    sb.line(format!(
        "- 对数 {}：已由改写解决 {}，待改已有卡 {}，仅为建议 {}，已确认不同 {}，不确定 {}，未判 {}，未通过检查 {}。",
        counts["pairs"],
        counts["resolved_by_rewrite"],
        counts["pending_existing_card_change"],
        counts["recommended"],
        counts["different"],
        counts["unresolved"],
        counts["unjudged"],
        counts["unverified"]
    ));
    sb.line(format!("- 保证：{}", f(report, "guarantee")));
    sb.line("");
    sb.line("## 一个待处理的解决");
    sb.line("");
    match report["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|row| ps::eq(&f(row, "status"), PENDING) || ps::eq(&f(row, "status"), RECOMMENDED))
    {
        Some(row) => {
            sb.line(format!(
                "- `{}` / `{}`：{}（`{}`）",
                f(row, "card"),
                f(row, "candidate"),
                f(row, "reason"),
                f(row, "status")
            ));
        }
        None => {
            sb.line("- 无：没有待处理的解决。");
        }
    }
    sb.line("");
    sb.line("## 一个已核实的解决");
    sb.line("");
    match report["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|row| ps::eq(&f(row, "status"), RESOLVED))
    {
        Some(row) => {
            sb.line(format!(
                "- `{}` → `{}`：{}（已回读并检查）",
                f(row, "card"),
                f(row, "candidate"),
                f(row, "reason")
            ));
        }
        None => {
            sb.line("- 无：还没有通过回读与检查的改写。");
        }
    }
    sb.line("");
    sb.line("## 每一对");
    sb.line("");
    for row in ps::each(&report["findings"]) {
        sb.line(format!(
            "- `{}` / `{}`：{}，{}，{}",
            f(&row, "card"),
            f(&row, "candidate"),
            f(&row, "verdict"),
            f(&row, "status"),
            f(&row, "reason")
        ));
    }
    sb.line("");
    sb.0
}

// --- small file helpers ---------------------------------------------------------------------------

/// The frozen evidence is immutable: its content must still hash to its own `evidence_id`, so no
/// step reads an edited set as the reviewed one.
fn verify_evidence(evidence: &Value) -> Result<()> {
    let id = f(evidence, "evidence_id");
    let expected = format!(
        "e-{}",
        canon::content_hash(evidence, &["evidence_id", "created_at"])
    );
    if id != expected {
        return Err(refuse(format!(
            "the evidence file content does not match its evidence id (`{id}`); no step reviews an edited set"
        )));
    }
    Ok(())
}

fn read_json(path: &Path) -> Result<Value> {
    if !path.is_file() {
        bail!("Cannot find path '{}' because it does not exist.", path.display());
    }
    Ok(ps::parse_json(&util::read_text(path)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// S23 / #115: the run's own population is decided from the title catalogue, bounded, and every
    /// exclusion keeps its reason. A Card the archive does not hold, a Literature-tree Card, a Model
    /// part and a restatement entry are never new claims; an over-long population reports its
    /// omissions instead of silently shrinking.
    #[test]
    fn the_run_population_is_narrowed_bounded_and_explained() {
        let row = |address: &str, tree: &str, workspace: &str, card_type: &str, extra: Value| {
            let mut value = json!({ "address": address, "tree": tree, "workspace": workspace, "type": card_type, "title": "t" });
            if let (Some(map), Some(add)) = (value.as_object_mut(), extra.as_object()) {
                for (key, item) in add {
                    map.insert(key.clone(), item.clone());
                }
            }
            value
        };
        let trees = vec![
            json!({ "tree": "12", "workspace": "agent", "cards": [
                row("12/0", "12", "agent", "idea", json!({ "edge": "root" })),
                row("12/1", "12", "agent", "idea", json!({})),
                row("12/2", "12", "agent", "idea", json!({ "restatement": true })),
                row("12/3", "12", "agent", "model", json!({})),
                row("12/4", "12", "human", "idea", json!({})),
            ]}),
            json!({ "tree": "K", "workspace": "agent", "cards": [
                row("K/1", "K", "agent", "idea", json!({})),
            ]}),
        ];
        let owned = vec![
            "12/0".to_string(), "12/1".to_string(), "12/2".to_string(), "12/3".to_string(),
            "12/4".to_string(), "K/1".to_string(), "12/9".to_string(),
        ];
        let (selected, facts) = population(&trees, &owned);
        assert_eq!(selected, vec!["12/1".to_string()]);
        assert_eq!(facts["owned"], 7);
        assert_eq!(facts["reviewable"], 1);
        assert_eq!(facts["omitted"], 0);
        assert_eq!(facts["skipped_total"], 6);
        let reasons = facts["skipped"].as_array().unwrap();
        assert_eq!(reasons.len(), 6);
        assert!(reasons.iter().any(|row| row["address"] == "12/0" && ps::s(&row["why"]).contains("tree root")));
        assert!(reasons.iter().any(|row| row["address"] == "K/1" && ps::s(&row["why"]).contains("Literature tree")));
        assert!(reasons.iter().any(|row| row["address"] == "12/9" && ps::s(&row["why"]).contains("does not hold")));
        // Omission is reported, never a silent truncation.
        let many: Vec<String> = (1..=(MAX_NEW + 5)).map(|i| format!("12/{}", i)).collect();
        let mut wide = vec![json!({ "tree": "12", "workspace": "agent", "cards": [] })];
        for at in &many {
            wide[0]["cards"].as_array_mut().unwrap().push(row(at, "12", "agent", "idea", json!({})));
        }
        let (selected, facts) = population(&wide, &many);
        assert_eq!(selected.len(), MAX_NEW);
        assert_eq!(facts["reviewable"], MAX_NEW + 5);
        assert_eq!(facts["omitted"], 5);
    }

    fn evidence(card: &str, candidate: &str, terminal: &str) -> Value {
        json!({
            "schema_version": SCHEMA,
            "archive": {"id": "a".repeat(32), "revision": 7},
            "book": "K",
            "evidence_id": "e-".to_string() + &"f".repeat(64),
            "new_cards": [{
                "address": card,
                "title": "斯卢茨基分解的多重性",
                "statement": "斯卢茨基分解把两种效应分开。",
                "assumptions": ["价格变化很小"],
                "formulas": ["$dx=\\partial x_s+\\partial x_i$"],
                "sources": ["K"],
                "fingerprint": "c".repeat(64),
                "candidates": [{
                    "address": candidate, "title": "斯卢茨基分解", "kind": "part",
                    "terminal": terminal, "statement": "斯卢茨基分解把两种效应分开。",
                    "assumptions": ["价格变化很小"], "formulas": ["$dx=\\partial x_s+\\partial x_i$"],
                    "sources": ["K"], "fingerprint": "d".repeat(64),
                    "relatedness": "0.4321", "title_similarity": "0.7100", "formula_match": true,
                }],
            }],
            "skipped": [],
            "unread": [],
            "endpoints": {card: {"title": "新卡"}, candidate: {"title": "候选"}},
            "coverage": {"cards": 1, "pairs": 1},
        })
    }

    fn answer(card: &str, candidate: &str, verdict: &str) -> Value {
        json!({
            "schema_version": SCHEMA,
            "evidence_id": "e-".to_string() + &"f".repeat(64),
            "judgments": [{
                "card": card, "candidate": candidate, "verdict": verdict,
                "reason": "两张卡的前提与范围相同。",
                "evidence": [card, candidate],
            }],
        })
    }

    fn same(card: &str, candidate: &str, remedy: &str, title: &str, body: &str) -> Value {
        let mut value = answer(card, candidate, "same");
        value["judgments"][0]["canonical"] = json!(candidate);
        value["judgments"][0]["scope"] = json!("价格变化很小的局部比较静态");
        value["judgments"][0]["remedy"] = json!(remedy);
        if !title.is_empty() {
            value["judgments"][0]["title"] = json!(title);
        }
        if !body.is_empty() {
            value["judgments"][0]["body"] = json!(body);
        }
        value
    }

    /// A `same` verdict carries the accepted rewrite of the new Card only: an entry names its
    /// terminal, and a distinct implication links it with a reason.
    #[test]
    fn a_same_decision_carries_one_new_card_rewrite() {
        let evidence = evidence("13/2", "K/4", "K/4");
        let entry_body = "## 同述入口\n\n- [[K/4]] 给出补偿需求项与收入效应项的完整定义。\n\n来源：[[K]] §2.1";
        let accepted = validate_answer(
            &evidence,
            &same("13/2", "K/4", "entry", "斯卢茨基分解的陈述入口", entry_body),
        )
        .expect("the entry resolution is accepted");
        assert_eq!(accepted["judgments"][0]["status"], json!(RECOMMENDED));
        assert_eq!(accepted["judgments"][0]["terminal"], json!("K/4"));
        let distinct = "本卡给出 [[K/4]] 的一个推论：当价格不变时，两项效应都为零。\n\n来源：[[K]] §2.1";
        validate_answer(
            &evidence,
            &same("13/2", "K/4", "distinct-implication", "斯卢茨基分解的恒等推论", distinct),
        )
        .expect("the distinct implication is accepted");
    }

    /// A formula match never decides: a `different` verdict states the distinction, and a rewrite
    /// that would change the existing Card stays pending.
    #[test]
    fn different_and_existing_card_changes_are_not_rewrites() {
        let evidence = evidence("13/2", "K/4", "K/4");
        let different = validate_answer(&evidence, &answer("13/2", "K/4", "different"))
            .expect("a different verdict is accepted");
        assert_eq!(different["judgments"][0]["status"], json!("different"));
        assert!(different["judgments"][0].get("body").is_none());
        let pending = validate_answer(
            &evidence,
            &same("13/2", "K/4", "existing-card-change", "", ""),
        )
        .expect("the pending resolution is accepted");
        assert_eq!(pending["judgments"][0]["status"], json!(PENDING));
        // The new Card as canonical would require the existing Card to change.
        let mut wrong = same("13/2", "K/4", "entry", "入口", "## 同述入口\n\n- [[K/4]] 给出完整定义。");
        wrong["judgments"][0]["canonical"] = json!("13/2");
        let error = validate_answer(&evidence, &wrong).expect_err("a rewrite of the canonical is refused");
        assert!(format!("{error:#}").contains("existing-card-change"), "{error:#}");
    }

    #[test]
    fn the_answer_is_bounded_to_prepared_pairs_and_read_endpoints() {
        let evidence = evidence("13/2", "K/4", "K/4");
        let mut unknown = answer("13/2", "K/9", "different");
        let error = validate_answer(&evidence, &unknown).expect_err("an unprepared pair is refused");
        assert!(format!("{error:#}").contains("did not prepare"), "{error:#}");
        unknown = answer("13/2", "K/4", "maybe");
        let error = validate_answer(&evidence, &unknown).expect_err("an unknown verdict is refused");
        assert!(format!("{error:#}").contains("unknown verdict"), "{error:#}");
        let mut unread = answer("13/2", "K/4", "different");
        unread["judgments"][0]["evidence"] = json!(["9/9"]);
        let error = validate_answer(&evidence, &unread).expect_err("an unread citation is refused");
        assert!(format!("{error:#}").contains("did not read"), "{error:#}");
        let mut twice = answer("13/2", "K/4", "different");
        twice["judgments"]
            .as_array_mut()
            .unwrap()
            .push(json!({"card": "13/2", "candidate": "K/4", "verdict": "same", "reason": "重复的判断。", "evidence": ["13/2"]}));
        let error = validate_answer(&evidence, &twice).expect_err("two outcomes for one pair are refused");
        assert!(format!("{error:#}").contains("twice"), "{error:#}");
        let error = validate_answer(&evidence, &json!({"schema_version": 1, "evidence_id": "e-x", "judgments": []}))
            .expect_err("another archive's evidence is refused");
        assert!(format!("{error:#}").contains("judges evidence"), "{error:#}");
    }

    /// An anchor is a comparison candidate, never the substantive terminal an entry names (§4.5,
    /// D085): the anchor keeps its place as the statement's home, and the new Card becomes a
    /// distinct implication that links it.
    #[test]
    fn an_anchor_is_no_entry_terminal_but_a_distinct_implication_links_it() {
        let mut evidence = evidence("13/2", "K/4", "K/4");
        evidence["new_cards"][0]["candidates"][0]["kind"] = json!("anchor");
        evidence["new_cards"][0]["candidates"][0]["terminal"] = json!("");
        let error = validate_answer(
            &evidence,
            &same(
                "13/2",
                "K/4",
                "entry",
                "入口",
                "## 同述入口\n\n- [[K/4]] 给出补偿需求项与收入效应项的完整定义。\n\n来源：[[K]] §2.1",
            ),
        )
        .expect_err("an entry cannot name an anchor");
        assert!(format!("{error:#}").contains("anchor"), "{error:#}");
        let accepted = validate_answer(
            &evidence,
            &same(
                "13/2",
                "K/4",
                "distinct-implication",
                "斯卢茨基分解在价格不变时的后果",
                "本卡依赖 [[K/4]] 的定义：价格不变时两项效应都为零。这条推论只用于局部比较。\n\n来源：[[K]] §2.1",
            ),
        )
        .expect("a distinct implication links the anchor");
        assert_eq!(accepted["judgments"][0]["terminal"], json!(""));
    }
    #[test]
    fn a_rewrite_keeps_the_source_evidence_and_the_terminal_reference() {
        let evidence = evidence("13/2", "K/4", "K/4");
        // A rewrite without the source line is refused.
        let error = validate_answer(
            &evidence,
            &same(
                "13/2",
                "K/4",
                "entry",
                "入口",
                "## 同述入口\n\n- [[K/4]] 给出补偿需求项与收入效应项的完整定义。\n",
            ),
        )
        .expect_err("a dropped source is refused");
        assert!(format!("{error:#}").contains("source evidence"), "{error:#}");
        // A chain is never written: the entry names the terminal, not the candidate's entry address.
        let error = validate_answer(
            &evidence,
            &same(
                "13/2",
                "K/4",
                "entry",
                "入口",
                "## 同述入口\n\n- [[K/9]] 给出补偿需求项与收入效应项的完整定义。\n\n来源：[[K]] §2.1",
            ),
        )
        .expect_err("a non-terminal entry target is refused");
        assert!(format!("{error:#}").contains("terminal"), "{error:#}");
        // A distinct implication must state a reason in the Link's sentence.
        let error = validate_answer(
            &evidence,
            &same("13/2", "K/4", "distinct-implication", "推论", "见 [[K/4]]。\n\n来源：[[K]] §2.1"),
        )
        .expect_err("a bare link is refused");
        assert!(format!(r#"{error:#}"#).contains("reason"), "{error:#}");
    }

    #[test]
    fn a_body_that_repeats_its_title_is_refused() {
        let evidence = evidence("13/2", "K/4", "K/4");
        let error = validate_answer(
            &evidence,
            &same(
                "13/2",
                "K/4",
                "entry",
                "入口",
                "入口\n\n## 同述入口\n\n- [[K/4]] 给出补偿需求项与收入效应项的完整定义。\n\n来源：[[K]] §2.1",
            ),
        )
        .expect_err("a repeated title is refused");
        assert!(format!("{error:#}").contains("repeats its title"), "{error:#}");
    }

    #[test]
    fn only_one_rewrite_per_new_card_is_accepted() {
        let mut evidence = evidence("13/2", "K/4", "K/4");
        let second = evidence["new_cards"][0]["candidates"][0].clone();
        let mut other = second;
        other["address"] = json!("K/7");
        other["terminal"] = json!("K/7");
        evidence["new_cards"][0]["candidates"]
            .as_array_mut()
            .unwrap()
            .push(other);
        let mut value = same(
            "13/2",
            "K/4",
            "entry",
            "入口",
            "## 同述入口\n\n- [[K/4]] 给出补偿需求项与收入效应项的完整定义。\n\n来源：[[K]] §2.1",
        );
        let mut second_judgment = value["judgments"][0].clone();
        second_judgment["candidate"] = json!("K/7");
        second_judgment["canonical"] = json!("K/7");
        second_judgment["body"] = json!("## 同述入口\n\n- [[K/7]] 给出补偿需求项与收入效应项的完整定义。\n\n来源：[[K]] §2.1");
        second_judgment["evidence"] = json!(["13/2", "K/7"]);
        value["judgments"]
            .as_array_mut()
            .unwrap()
            .push(second_judgment);
        // Both candidates are the same statement: the first accepted rewrite stands, the second is
        // refused, so one new Card never carries two contradictory resolutions.
        let error = validate_answer(&evidence, &value).expect_err("a second rewrite is refused");
        assert!(format!("{error:#}").contains("already resolved"), "{error:#}");
    }

    #[test]
    fn the_report_keeps_recommendations_pending_and_verification_apart() {
        let evidence = evidence("13/2", "K/4", "K/4");
        let judged = validate_answer(
            &evidence,
            &same(
                "13/2",
                "K/4",
                "entry",
                "入口",
                "## 同述入口\n\n- [[K/4]] 给出补偿需求项与收入效应项的完整定义。\n\n来源：[[K]] §2.1",
            ),
        )
        .unwrap();
        let recommended = report_payload(&evidence, Some(&judged), None);
        assert_eq!(recommended["counts"]["recommended"], json!(1));
        assert_eq!(recommended["counts"]["resolved_by_rewrite"], json!(0));
        assert_eq!(recommended["counts"]["pending_existing_card_change"], json!(0));
        assert_eq!(recommended["status"], json!("recommended"));
        // The applied resolution: one verified rewrite, and a merge verdict that stays pending.
        let resolution = json!({
            "findings": [{
                "card": "13/2", "candidate": "K/4", "verdict": "same", "status": RESOLVED,
                "reason": "同一陈述。", "checked": true, "faults": [],
            }],
            "merges": [{
                "address": "13/9", "merge_with": "13/8", "status": PENDING,
                "why": "no rewrite covers it",
            }],
        });
        let applied = report_payload(&evidence, Some(&judged), Some(&resolution));
        assert_eq!(applied["counts"]["resolved_by_rewrite"], json!(1));
        assert_eq!(applied["counts"]["pending_existing_card_change"], json!(1));
        assert_eq!(applied["counts"]["recommended"], json!(0));
        assert_eq!(applied["status"], json!("resolved-with-pending"));
        let markdown = render_markdown(&applied);
        assert!(markdown.contains("待处理的解决"), "{markdown}");
        assert!(markdown.contains("已核实的解决"), "{markdown}");
        assert!(markdown.contains("仅仅是建议") || markdown.contains("仅为建议"), "{markdown}");
        // An unjudged pair is never reported as resolved.
        let none = report_payload(&evidence, None, None);
        assert_eq!(none["counts"]["unjudged"], json!(1));
        assert_eq!(none["status"], json!("partial"));
        assert_eq!(none["complete"], json!(false));
    }

    #[test]
    fn entry_links_read_only_the_section_and_the_list_items() {
        assert_eq!(
            entry_links("陈述。\n\n## 同述入口\n\n- [[88/2|a]] 给出完整定义。\n\n来源：[[K]]\n"),
            vec!["88/2|a"]
        );
        assert!(entry_links("陈述 [[88/2]]。").is_empty());
        assert!(entry_links("```\n## 同述入口\n- [[88/2]]\n```").is_empty());
        assert_eq!(entry_links("## 同述入口\n- [[88/2]] 一处\n- [[88/3]] 两处\n").len(), 2);
        assert!(is_entry_body("## 同述入口\n- [[88/2]]\n"));
        assert!(!is_entry_body("这张卡是 [[88/2]] 的同述入口。"));
        // A source line is evidence beside the claim, not the claim.
        assert_eq!(sources("陈述。\n\n来源：[[K]] §2.1\n"), vec!["K"]);
        assert!(keeps_sources("陈述。\n\n来源：[[K]] §2.1\n", &["K".to_string()]));
        assert!(!keeps_sources("陈述。\n", &["K".to_string()]));
        assert!(linked_with_reason("本卡给出 [[K/4]] 的一个推论：价格不变时两项都为零。", "K/4"));
        assert!(!linked_with_reason("见 [[K/4]]。", "K/4"));
    }

    #[test]
    fn a_new_card_must_be_an_agent_topic_idea_claim() {
        let claim = json!({"address": "5/2", "tree": "5", "workspace": "agent", "type": "idea", "title": "t", "restatement": false});
        assert!(new_card_refusal(&claim).is_none());
        let human = json!({"address": "5/2", "tree": "5", "workspace": "human", "type": "idea", "restatement": false});
        assert!(new_card_refusal(&human).is_some());
        let literature = json!({"address": "K/3", "tree": "K", "workspace": "agent", "type": "idea", "restatement": false});
        assert!(new_card_refusal(&literature).is_some());
        let part = json!({"address": "5/2", "tree": "5", "workspace": "agent", "type": "model", "facet": "设定", "restatement": false});
        assert!(new_card_refusal(&part).is_some());
        let root = json!({"address": "5/0", "tree": "5", "workspace": "agent", "type": "idea", "restatement": false});
        assert!(new_card_refusal(&root).is_some());
        let entry = json!({"address": "5/3", "tree": "5", "workspace": "agent", "type": "idea", "restatement": true});
        assert!(new_card_refusal(&entry).is_some());
    }
}
