//! `zt-digest topic-home`: the report-bound Topic-home transfer
//! (docs/luhmann-comparison-implementation-design.md §11.1, §11.3, §12.3; GitHub issue #113,
//! slice S21).
//!
//!   zt-digest topic-home -Step plan -Work <folder> [-Report <concept-review.json>]
//!                        [-MaxItems 50] [-MaxCarriers 60]
//!   zt-digest topic-home -Step apply -Work <folder> [-File <selection.json>]
//!                        -OperationPrefix <run/unit/positive-revision/step>
//!
//! `plan` consumes the reviewed concept report of a prepared run (`-Report`, default
//! `<Work>\concept-review.json`) and writes one immutable `topic-home` review:
//! `topic-home-review.json`, `topic-home-review.md` and one retained endpoint body per read Card
//! under `<Work>\endpoints\`. It changes no Card. For every judged `topic-home` finding it reads the
//! Topic's entry Cards and proposes, per book-bound entry, the concrete transfer §11.1 asks for: a
//! target Literature anchor with the plain Card draft that will hold the book-bound statement, the
//! edit of the old Card (an explicit `## 同述入口` entry, or an authored distinct interpretation),
//! and the source Links and statement/formula checks that prove the content was preserved
//! (D263–D265). The Topic-to-Literature `move` refusal is untouched: this path never moves a Card.
//!
//! Every item binds the concept report id and finding id, the archive identity, the reviewed
//! authored fingerprints, the exact proposed texts and the explicit treatment of an earlier carrier
//! — an Idea Card of the target Literature tree that already carries the statement, which is reused
//! instead of copied (D272). Agent writability and the finished-model boundary are decided before
//! any write: a Model Card or Model part is `blocked` on a named separate model decision, and no
//! Model part is created and no model membership or build order changes (D267, S21-AC04).
//!
//! `apply` consumes `-File`, a reviewed selection that binds the report id, one item per decision,
//! the remedy and a nonempty note, then executes each item as two identified writes: the Literature
//! statement first, read back and verified, then the old Card with its authored base guard. The two
//! writes are deliberately not one atomic move (D268): a crash after the creation leaves the created
//! statement and no old-Card edit, and the retry replays the creation receipt instead of creating
//! another copy. A changed old Card preserves the created statement and reports a revised review
//! item; it is never overwritten (D273). After the writes the persisted statements, source Links,
//! actual Relation kinds, target validity, inbound Links, Broken links, one substantive home and the
//! affected model reports are verified, and each item ends `applied-and-verified`, `unchanged`,
//! `conflict`, `refused` or `pending` (D274). A tool that returned without a process error is never
//! enough.
//!
//! Historical live execution is a separate gate (S29): this step executes only the exact items a
//! caller selected in a reviewed selection file, and its evidence scope is a scratch archive.

use crate::args::Args;
use crate::calls;
use crate::canon;
use crate::concepts::{self, sources};
use crate::organize::common::{self, pstr, Sb};
use crate::ps;
use crate::snapshot;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use zt_check::mcp::{reply_error, reply_text};

// --- the fixed bounds (§12.2; reported so a comparison is reproducible) ---------------------------

/// The judged findings one review turns into items.
pub const MAX_ITEMS: i64 = 50;
/// The entry Cards one Topic contributes: its Direct successors, read up to this bound.
pub const MAX_ENTRIES: i64 = 12;
/// The Idea Cards of one target Literature tree that the earlier-carrier scan reads.
pub const MAX_CARRIERS: i64 = 60;
/// The retained endpoints one review may name.
const MAX_ENDPOINTS: usize = 2_000;
/// A reviewed selection names at most this many items.
const MAX_DECISIONS: usize = 50;
/// The Card size rule one accepted body keeps (a Card fits one Card).
const MAX_TITLE: usize = 200;
const MAX_BODY: usize = 2_000;
const SCHEMA_VERSION: i64 = 1;
/// §11.1: the old-Card edit is an explicit entry, or an authored distinct interpretation.
pub const REMEDIES: [&str; 2] = ["entry", "distinct"];
/// §4.5's accepted restatement entry heading: the entry is navigation, not a second copy.
const ENTRY_HEADING: &str = "同述入口";

/// The per-item statuses §11.3 names (D274).
pub const APPLIED: &str = "applied-and-verified";
pub const UNCHANGED: &str = "unchanged";
pub const CONFLICT: &str = "conflict";
pub const REFUSED: &str = "refused";
pub const PENDING: &str = "pending";

fn refuse(message: impl Into<String>) -> anyhow::Error {
    canon::refuse(message.into())
}

fn f(value: &Value, key: &str) -> String {
    ps::s(&value[key])
}

// --- the command ---------------------------------------------------------------------------------

struct Params {
    step: String,
    work: String,
    report: String,
    file: String,
    prefix: String,
    max_items: usize,
    max_carriers: usize,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Step", "Work", "Report", "File", "OperationPrefix", "MaxItems", "MaxCarriers"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &[])?;
    let step = a.choice("Step", &["plan", "apply"])?;
    let p = Params {
        step,
        work: common::mandatory(&a, "Work")?,
        report: a.str("Report"),
        file: a.str("File"),
        prefix: a.str("OperationPrefix"),
        max_items: canon::integer_option(&a.str_or("MaxItems", &MAX_ITEMS.to_string()), "MaxItems", 1, MAX_ITEMS)? as usize,
        max_carriers: canon::integer_option(
            &a.str_or("MaxCarriers", &MAX_CARRIERS.to_string()),
            "MaxCarriers",
            1,
            MAX_CARRIERS,
        )? as usize,
    };
    match p.step.as_str() {
        "plan" => plan(&p),
        _ => apply(&p),
    }
}

// --- Location rules (the rules zt places Cards by) -----------------------------------------------

fn tree_of(at: &str) -> String {
    at.split_once('/').map(|(tree, _)| tree.to_string()).unwrap_or_else(|| at.to_string())
}

fn last_segment(at: &str) -> &str {
    at.rsplit_once('|').map(|(_, segment)| segment).or_else(|| at.rsplit_once('/').map(|(_, segment)| segment)).unwrap_or(at)
}

fn side_segment(segment: &str) -> bool {
    !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn side_number(label: &str) -> u64 {
    label.bytes().fold(0_u64, |acc, byte| acc * 26 + (byte - b'a' + 1) as u64)
}

fn side_label(value: u64) -> String {
    let mut value = value;
    let mut chars = Vec::new();
    while value > 0 {
        value -= 1;
        chars.push((b'a' + (value % 26) as u8) as char);
        value /= 26;
    }
    chars.iter().rev().collect()
}

fn replace_last(at: &str, replacement: &str) -> String {
    if let Some((prefix, _)) = at.rsplit_once('|') {
        format!("{prefix}|{replacement}")
    } else if let Some((prefix, _)) = at.rsplit_once('/') {
        format!("{prefix}/{replacement}")
    } else {
        replacement.to_string()
    }
}

fn in_tree(target: &str, tree: &str) -> bool {
    target == tree || target.starts_with(&format!("{tree}/"))
}

/// `direct_successor`: the one continuation slot after a Card. A Literature tree's root is its
/// Citation key, so its first continuation is `<key>/1`.
fn direct_successor(at: &str) -> String {
    if !at.contains('/') && !at.contains('|') {
        return format!("{at}/1");
    }
    match last_segment(at).parse::<u64>() {
        Ok(number) if !side_segment(last_segment(at)) => replace_last(at, &(number + 1).to_string()),
        _ => format!("{at}|1"),
    }
}

/// The next free Side successor of one Card: the plain Card §11.1 files beside the anchor.
fn next_side(sides: &[String]) -> String {
    let next = sides.iter().map(|label| side_number(label)).max().unwrap_or(0) + 1;
    side_label(next)
}

// --- the archive view ----------------------------------------------------------------------------

fn archive_identity(zt: &mut Zt) -> Result<(String, String, u64)> {
    let view = zt.archive_view()?;
    let id = view["archive_identity"]["id"].as_str().context("incomplete archive identity: no id")?.to_string();
    let path = view["archive_identity"]["path"].as_str().context("incomplete archive identity: no path")?.to_string();
    let revision = view["archive_revision"].as_u64().context("incomplete archive identity: no revision")?;
    Ok((id, path, revision))
}

/// One read view: the title catalogue (address, tree, Workspace, Card kind, edge, type, facet,
/// title and the derived restatement role) plus the zone of every tree.
#[derive(Default)]
struct View {
    rows: BTreeMap<String, Value>,
    zones: BTreeMap<String, String>,
}

impl View {
    fn read(zt: &mut Zt, revision: u64) -> Result<View> {
        let trees = snapshot::read_title_trees(zt)?;
        let mut view = View::default();
        for tree in &trees {
            if tree["archive_revision"].as_u64() != Some(revision) {
                return Err(concepts::changing_view());
            }
            let name = f(tree, "tree");
            for row in ps::each(&tree["cards"]) {
                let at = f(row, "address");
                if f(row, "edge") == "root" {
                    view.zones.insert(name.clone(), f(row, "kind"));
                }
                view.rows.insert(at, row.clone());
            }
        }
        Ok(view)
    }

    fn has(&self, at: &str) -> bool {
        self.rows.contains_key(at)
    }

    fn row(&self, at: &str) -> Option<&Value> {
        self.rows.get(at)
    }

    fn zone(&self, at: &str) -> String {
        self.zones.get(&tree_of(at)).cloned().unwrap_or_default()
    }

    /// A plain Regular Idea Card of the agent Workspace: a Card a new Side successor can start
    /// from. A tree root takes no Side successor, so a Literature root is never an anchor.
    fn plain_idea(&self, at: &str) -> bool {
        self.row(at).is_some_and(|row| {
            f(row, "type") == "idea"
                && row["facet"].is_null()
                && f(row, "workspace") == "agent"
                && f(row, "edge") != "root"
        })
    }

    /// The Topic's entry chain: its Direct successors `<tree>/1`, `<tree>/2`, … in order.
    fn chain(&self, root: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut at = direct_successor(root);
        while self.has(&at) && out.len() < 400 {
            out.push(at.clone());
            at = direct_successor(&at);
        }
        out
    }

    /// The side successors of one Card, in label order.
    fn sides(&self, at: &str) -> Vec<String> {
        let prefix = format!("{at}|");
        let mut out: Vec<(u64, String)> = self
            .rows
            .keys()
            .filter_map(|here| here.strip_prefix(&prefix))
            .filter_map(|rest| rest.split('|').next())
            .filter(|label| side_segment(label))
            .map(|label| (side_number(label), label.to_string()))
            .collect();
        out.sort_by_key(|(number, _)| *number);
        out.dedup_by_key(|(number, _)| *number);
        out.into_iter().map(|(_, label)| label).collect()
    }

    /// The Idea Cards of one Literature tree the earlier-carrier scan may read, and whether the
    /// bound left candidates unread.
    fn carriers(&self, tree: &str, max: usize) -> (Vec<String>, bool) {
        let mut idea: Vec<String> = self
            .rows
            .iter()
            .filter(|(at, row)| in_tree(at, tree) && f(row, "type") == "idea" && row["facet"].is_null())
            .map(|(at, _)| at.clone())
            .collect();
        idea.sort_by(|a, b| canon::ordinal_cmp(a, b));
        let bounded = idea.len() > max;
        idea.truncate(max);
        (idea, bounded)
    }

    fn books(&self) -> HashSet<String> {
        self.rows.values().filter(|row| f(row, "kind") == "literature").map(|row| f(row, "address")).collect()
    }
}

// --- retained endpoints --------------------------------------------------------------------------

/// The Cards the review read, with their retained bodies and authored fingerprints (§11.3, D272).
#[derive(Default)]
struct Endpoints {
    files: BTreeMap<String, Value>,
}

impl Endpoints {
    fn keep(&mut self, work: &str, card: &Value) -> Result<()> {
        let at = f(card, "address");
        if self.files.contains_key(&at) {
            return Ok(());
        }
        if self.files.len() >= MAX_ENDPOINTS {
            return Err(refuse(format!(
                "a Topic-home review keeps at most {MAX_ENDPOINTS} endpoints; this archive needs more"
            )));
        }
        let (path, sha256) = concepts::retain_endpoint(work, card)?;
        self.files.insert(
            at.clone(),
            json!({
                "address": at,
                "title": f(card, "title"),
                "type": f(card, "type"),
                "facet": card["facet"].clone(),
                "workspace": f(card, "workspace"),
                "fingerprint": calls::authored_fingerprint(card),
                "statement_fingerprint": f(card, "statement_fingerprint"),
                "path": path,
                "sha256": sha256,
            }),
        );
        Ok(())
    }

    fn fingerprint(&self, at: &str) -> String {
        self.files.get(at).and_then(|row| row["fingerprint"].as_str()).unwrap_or_default().to_string()
    }
}

// --- the book-bound statement --------------------------------------------------------------------

/// The Links a body's source lines name, split into this book's own targets and another book's.
/// An entry is book-bound when it names at least one Card of its own book and no Card of another
/// book — the evidence a `topic-home` finding rests on (§7.1, D079).
fn source_books(body: &str, book: &str, books: &HashSet<String>) -> (Vec<String>, Vec<String>) {
    let mut own: Vec<String> = Vec::new();
    let mut other: Vec<String> = Vec::new();
    for target in sources(body) {
        if in_tree(&target, book) {
            if !own.contains(&target) {
                own.push(target);
            }
        } else if books.iter().any(|tree| in_tree(&target, tree)) && !other.contains(&target) {
            other.push(target);
        }
    }
    (own, other)
}

/// The formula spans of a statement, in document order: what a transfer must not lose (§11.1).
fn formulas(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for found in concepts::FORMULA.find_iter(body) {
        let text = found.as_str().trim().to_string();
        if !text.is_empty() && !out.contains(&text) {
            out.push(text);
        }
    }
    out
}

/// The statement text a transfer moves: the authored body without its source lines. The source
/// lines travel beside the claim as its evidence, and they are not the claim.
fn statement_text(body: &str) -> String {
    let normalized = body.replace("\r\n", "\n").replace('\r', "\n");
    let mut kept: Vec<&str> = Vec::new();
    let mut fenced = false;
    for raw in normalized.split('\n') {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            kept.push(raw);
            continue;
        }
        if !fenced && concepts::SOURCE_LINE.is_match(trimmed) {
            continue;
        }
        kept.push(raw);
    }
    kept.join("\n").trim().to_string()
}

// --- the concept report the transfer consumes ----------------------------------------------------

fn topic_home_findings(report: &Value) -> Vec<Value> {
    ps::ps_items(&report["findings"]).into_iter().filter(|row| f(row, "kind") == "topic-home").collect()
}

/// The reviewed concept report: immutable for one evidence set, so a file that no longer matches
/// its own id is stale text and never this transfer's parent.
fn read_concept_review(path: &str) -> Result<Value> {
    if !ps::exists(path) {
        return Err(refuse(format!(
            "topic-home plan needs the reviewed concept report `{path}`; run `zt-digest concepts -Step report` first"
        )));
    }
    let value = canon::read_json(path)?;
    let id = f(&value, "report_id");
    if !id.starts_with("r-") {
        return Err(refuse(format!("`{path}` is not a concept review: no report id")));
    }
    let recomputed = format!("r-{}", canon::content_hash(&value, &["report_id", "created_at"]));
    if recomputed != id {
        return Err(refuse(format!(
            "the concept report at `{path}` does not match its own report id; regenerate it before transferring"
        )));
    }
    Ok(value)
}

// --- the plan step -------------------------------------------------------------------------------

fn plan(p: &Params) -> Result<i32> {
    let report_path = if p.report.is_empty() { ps::join(&p.work, "concept-review.json") } else { p.report.clone() };
    let report = read_concept_review(&report_path)?;
    let book = pstr(&report["book"], "citation_key");
    if book.is_empty() {
        return Err(refuse("the concept review names no book, so no Literature tree can be the target"));
    }
    let mut zt = crate::zt::start()?;
    let (archive_id, archive_path, revision) = archive_identity(&mut zt)?;
    let view = View::read(&mut zt, revision)?;
    let entries = MAX_ENTRIES as usize;
    let mut planner = Planner {
        work: &p.work,
        book: book.clone(),
        view,
        endpoints: Endpoints::default(),
        carriers: HashMap::new(),
        unread: Vec::new(),
        bounded: false,
        max_carriers: p.max_carriers,
    };
    let mut items: Vec<Value> = Vec::new();
    let mut findings: Vec<Value> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut skipped = 0usize;
    for finding in topic_home_findings(&report) {
        let fid = f(&finding, "id");
        let root = f(&finding, "candidate");
        let verdict = f(&finding, "verdict");
        if verdict != "same" && verdict != "different" {
            pending.push(format!("{fid}: `{root}` 的 topic-home 判断没有评审，转移仍未决"));
            findings.push(json!({
                "id": fid, "candidate": root, "book": f(&finding, "book"), "verdict": verdict,
                "status": "pending", "items": [],
                "why": "the concept judgment is not reviewed, so no transfer is offered",
            }));
            continue;
        }
        if !planner.view.has(&root) {
            findings.push(json!({
                "id": fid, "candidate": root, "book": f(&finding, "book"), "verdict": verdict,
                "status": "no_item", "items": [],
                "why": format!("`{root}` is not in this archive; the finding's evidence is stale"),
            }));
            continue;
        }
        let chain = planner.view.chain(&root);
        let read: Vec<String> = chain.iter().take(entries).cloned().collect();
        let mut unread_here: Vec<(String, String)> = Vec::new();
        let rows = concepts::read_cards(&mut zt, &read, revision, &mut unread_here, "topic-home")?;
        for (at, why) in unread_here {
            planner.note_unread(&at, &why);
        }
        for row in &rows {
            planner.endpoints.keep(&p.work, row)?;
        }
        // The anchor and the book root are read once per review, for their reviewed fingerprints.
        let books = planner.view.books();
        let mut context_reads: Vec<String> = vec![book.clone()];
        for row in &rows {
            let (own, _) = source_books(&f(row, "body"), &book, &books);
            for target in own {
                if !context_reads.contains(&target) {
                    context_reads.push(target);
                }
            }
        }
        let existing: Vec<String> = context_reads.iter().filter(|at| planner.view.has(at)).cloned().collect();
        let mut unread_context: Vec<(String, String)> = Vec::new();
        for row in concepts::read_cards(&mut zt, &existing, revision, &mut unread_context, "topic-home")? {
            planner.endpoints.keep(&p.work, &row)?;
        }
        for (at, why) in unread_context {
            planner.note_unread(&at, &why);
        }
        let mut offered_here = 0usize;
        let mut here: Vec<String> = Vec::new();
        for row in &rows {
            if items.len() >= p.max_items {
                skipped += 1;
                continue;
            }
            let index = items.len() + 1;
            let zone = planner.view.zone(&f(row, "address"));
            let item = planner.item(&mut zt, revision, &finding, row, index, &zone)?;
            if f(&item, "status") == "offered" {
                offered_here += 1;
            }
            here.push(f(&item, "id"));
            items.push(item);
        }
        findings.push(json!({
            "id": fid, "candidate": root, "book": f(&finding, "book"), "verdict": verdict,
            "status": if offered_here > 0 { "offered" } else { "no_item" },
            "items": here,
            "reason": finding["reason"],
            "own_book": finding["evidence"],
            "why": if offered_here > 0 {
                "each book-bound entry has its own transfer item".to_string()
            } else {
                "no book-bound entry can be transferred as reviewed; the items name why".to_string()
            },
        }));
    }
    let closing = archive_identity(&mut zt)?;
    if closing.2 != revision {
        return Err(concepts::changing_view());
    }
    let mut reasons: Vec<String> = Vec::new();
    if !planner.unread.is_empty() {
        reasons.push("unread_endpoint".to_string());
    }
    if planner.bounded {
        reasons.push("bounded_carrier_scan".to_string());
    }
    let offered = items.iter().filter(|item| f(item, "status") == "offered").count();
    let refused = items.iter().filter(|item| f(item, "status") == "refused").count();
    let blocked = items.iter().filter(|item| f(item, "status") == "blocked").count();
    let mut payload = json!({
        "schema_version": SCHEMA_VERSION,
        "kind": "topic-home-review",
        "created_at": canon::utc_now_o(),
        "concept_report": {
            "path": report_path,
            "report_id": report["report_id"],
            "status": report["status"],
            "book": report["book"],
            "archive": report["archive"],
        },
        "archive": {"id": archive_id, "path": archive_path, "revision": revision},
        "bounds": {"items": p.max_items, "entries": entries, "carriers": p.max_carriers, "decisions": MAX_DECISIONS},
        "findings": findings,
        "items": items,
        "pending_findings": pending,
        "unread": planner.unread.iter().map(|(at, why)| json!({"address": at, "reason": why})).collect::<Vec<Value>>(),
        "endpoints": planner.endpoints.files,
        "completeness": {
            "complete": reasons.is_empty(),
            "items": offered + refused + blocked,
            "offered": offered,
            "refused": refused,
            "blocked": blocked,
            "skipped": skipped,
            "reasons": reasons,
        },
        "status": if reasons.is_empty() { "complete" } else { "partial" },
        "scope": "one reviewed concept report; the transfer writes only when a reviewed selection names it, and running it on real historical Cards is a separate gate",
    });
    payload["report_id"] = json!(format!("th-{}", canon::content_hash(&payload, &["report_id", "created_at"])));
    let json_path = ps::join(&p.work, "topic-home-review.json");
    let md_path = ps::join(&p.work, "topic-home-review.md");
    let mut written = false;
    let existing = if ps::exists(&json_path) { canon::read_json(&json_path).ok() } else { None };
    let intact = existing.filter(|old| {
        f(old, "report_id") == f(&payload, "report_id")
            && format!("th-{}", canon::content_hash(old, &["report_id", "created_at"])) == f(&payload, "report_id")
    });
    match intact {
        Some(published) => {
            let markdown = render_markdown(&published);
            if !ps::exists(&md_path) || util::read_text(&md_path)? != markdown {
                util::write_atomic(&md_path, &markdown)?;
                written = true;
            }
            payload = published;
        }
        None => {
            canon::save_json(&json_path, &payload)?;
            util::write_atomic(&md_path, &render_markdown(&payload))?;
            written = true;
        }
    }
    util::print_line(&ps::json_compact(&json!({
        "ok": true,
        "step": "plan",
        "report_id": payload["report_id"],
        "concept_report_id": payload["concept_report"]["report_id"],
        "status": payload["status"],
        "archive_id": payload["archive"]["id"],
        "archive_revision": payload["archive"]["revision"],
        "findings": ps::pipe(&payload["findings"]).len(),
        "items": ps::pipe(&payload["items"]).len(),
        "offered": payload["completeness"]["offered"],
        "refused": payload["completeness"]["refused"],
        "blocked": payload["completeness"]["blocked"],
        "pending_findings": ps::pipe(&payload["pending_findings"]).len(),
        "unread": ps::pipe(&payload["unread"]).len(),
        "written": written,
        "paths": [json_path, md_path],
    })))?;
    Ok(0)
}

/// What one plan pass accumulates: the view, the retained endpoints and the bounded carrier reads.
struct Planner<'a> {
    work: &'a str,
    book: String,
    view: View,
    endpoints: Endpoints,
    carriers: HashMap<String, (Vec<Value>, bool)>,
    unread: Vec<(String, String)>,
    bounded: bool,
    max_carriers: usize,
}

impl<'a> Planner<'a> {
    fn note_unread(&mut self, at: &str, why: &str) {
        if !self.unread.iter().any(|(known, _)| known == at) {
            self.unread.push((at.to_string(), why.to_string()));
            self.bounded = true;
        }
    }

    /// The earlier-carrier population of one tree, read once per review.
    fn carriers_of(&mut self, zt: &mut Zt, tree: &str, revision: u64) -> Result<(Vec<Value>, bool)> {
        if !self.carriers.contains_key(tree) {
            let (addresses, bounded) = self.view.carriers(tree, self.max_carriers);
            let mut unread: Vec<(String, String)> = Vec::new();
            let rows = concepts::read_cards(zt, &addresses, revision, &mut unread, "topic-home")?;
            for row in &rows {
                self.endpoints.keep(self.work, row)?;
            }
            for (at, why) in unread {
                self.note_unread(&at, &why);
            }
            self.carriers.insert(tree.to_string(), (rows, bounded));
        }
        Ok(self.carriers[tree].clone())
    }

    /// The target anchor and the model context: a plain agent Idea Card of the book's tree, named by
    /// the entry's own source Links first, else the book's first plain anchor. A Model Card is
    /// recorded as model context, never the create parent, and a book without a plain anchor is
    /// refused beside the alternative: the digest's own anchors (D267).
    fn anchor_for(&self, own: &[String]) -> std::result::Result<(String, Vec<Value>), (String, String)> {
        let mut context: Vec<Value> = Vec::new();
        let mut named: Vec<String> = Vec::new();
        for target in own {
            let Some(row) = self.view.row(target) else { continue };
            if f(row, "workspace") != "agent" {
                continue;
            }
            if f(row, "type") == "model" {
                context.push(json!({
                    "address": target,
                    "title": f(row, "title"),
                    "facet": row["facet"].clone(),
                    "kind": if row["facet"].is_null() { "model" } else { "part" },
                }));
                continue;
            }
            if !named.contains(target) {
                named.push(target.clone());
            }
        }
        let Some(book_row) = self.view.row(&self.book) else {
            return Err((
                "book_missing".to_string(),
                format!("the concept report's book `{}` is not in this archive", self.book),
            ));
        };
        if f(book_row, "workspace") != "agent" {
            return Err((
                "read_only_workspace".to_string(),
                format!("the book `{}` is not an agent tree; a transfer writes agent Cards only", self.book),
            ));
        }
        for at in &named {
            if self.view.plain_idea(at) {
                return Ok((at.clone(), context));
            }
        }
        if let Some(first) = self.view.chain(&self.book).into_iter().find(|at| self.view.plain_idea(at)) {
            return Ok((first, context));
        }
        let detail = if context.is_empty() {
            format!("the book's Literature tree `{}` holds no plain anchor Card to file a statement beside", self.book)
        } else {
            format!(
                "the book's Literature tree `{}` holds no plain anchor Card; the statement concerns the model(s) {}, so a separate explicit model design decision owns that placement (D267)",
                self.book,
                context.iter().map(|row| f(row, "address")).collect::<Vec<String>>().join(", ")
            )
        };
        Err(("no_anchor".to_string(), detail))
    }

    /// D264: the old Card becomes an explicit entry naming the new statement. The entry section is
    /// §4.5's accepted `## 同述入口` shape, its single Link names the terminal statement, and the
    /// source line keeps I3's appropriate source Link.
    fn entry_remedy(title: &str, target: &str, book: &str) -> Value {
        let body = format!(
            "本主题的这条陈述已转成文献陈述。\n\n## {ENTRY_HEADING}\n\n- [[{target}]] 按本书的原话。\n\n来源：[[{book}]]"
        );
        json!({"title": title, "body": body})
    }

    /// The authored alternative §11.1 allows: a distinct interpretation the reviewer writes. The
    /// review names what that draft must satisfy; the selection binds its exact text.
    fn distinct_requires(target: &str, book: &str) -> Vec<String> {
        vec![
            format!("正文用带关系词的句子链接 `[[{target}]]`，说出这个主题自己的解释。"),
            format!("正文保留来源行 `来源：[[{book}]]`。"),
            "正文的陈述与文献陈述不同：读出卡后比较两张卡的陈述指纹。".to_string(),
            format!("标题是一行非空主张，最多 {MAX_TITLE} 个字符。"),
        ]
    }

    /// One transfer item for one entry Card of the finding's Topic.
    fn item(&mut self, zt: &mut Zt, revision: u64, finding: &Value, entry: &Value, index: usize, zone: &str) -> Result<Value> {
        let at = f(entry, "address");
        let books = self.view.books();
        let (own, other) = source_books(&f(entry, "body"), &self.book, &books);
        let claim = statement_text(&f(entry, "body"));
        let mut item = json!({
            "id": format!("t{index:03}"),
            "finding": f(finding, "id"),
            "book": self.book,
            "topic": f(finding, "candidate"),
            "entry": at,
            "verdict": f(finding, "verdict"),
            "zone": zone,
            "status": "offered",
            "reason": Value::Null,
            "statement": {
                "title": f(entry, "title"),
                "body": f(entry, "body"),
                "statement": claim,
                "statement_fingerprint": f(entry, "statement_fingerprint"),
                "formulas": formulas(&claim),
                "sources": own,
                "other_books": other,
                "chars": ps::len16(&claim),
            },
            "target": Value::Null,
            "earlier_carrier": Value::Null,
            "carrier_scan": Value::Null,
            "remedies": Value::Null,
            "checks": {
                "agent_writable": ps::truthy(&entry["writable"]),
                "plain_idea": f(entry, "type") == "idea" && entry["facet"].is_null(),
                "zone": zone,
                "card_kind": f(entry, "kind"),
                "restatement": ps::truthy(&entry["restatement"]),
                "model_boundary": "plain_idea",
                "source_links": own,
                "formulas": formulas(&claim),
            },
        });
        fn stop(item: &mut Value, status: &str, code: &str, detail: String) {
            item["status"] = json!(status);
            item["reason"] = json!({"code": code, "detail": detail});
        }
        if f(entry, "kind") != "regular" || f(entry, "workspace") != "agent" || !ps::truthy(&entry["writable"]) {
            stop(
                &mut item,
                REFUSED,
                "read_only_workspace",
                format!("`{at}` is not an agent Regular Card; agents cannot transfer it and its body is untouched"),
            );
            return Ok(item);
        }
        if f(entry, "type") == "model" {
            item["checks"]["model_boundary"] = json!("model_card");
            stop(
                &mut item,
                "blocked",
                "model_boundary",
                format!(
                    "`{at}` is a Model Card or a Model part: transferring it would create a Model part or change a finished model, so a separate explicit model design decision is required (D267)"
                ),
            );
            return Ok(item);
        }
        if f(entry, "type") != "idea" || !entry["facet"].is_null() {
            stop(
                &mut item,
                REFUSED,
                "not_plain_idea",
                format!("`{at}` is not a plain Idea Card; a transfer creates one plain Literature Idea Card and moves no other Card"),
            );
            return Ok(item);
        }
        if ps::truthy(&entry["restatement"]) {
            stop(
                &mut item,
                REFUSED,
                "already_entry",
                format!("`{at}` is already a restatement entry: its statement has a home, so no transfer is offered"),
            );
            return Ok(item);
        }
        if own.is_empty() || !other.is_empty() {
            let detail = if own.is_empty() {
                format!("`{at}` names no source Link inside its own book `{}`, so no Literature statement can be proved", self.book)
            } else {
                format!("`{at}` names another book's Card ({}), so it is not a book-bound statement", other.join(", "))
            };
            stop(&mut item, REFUSED, "not_book_bound", detail);
            return Ok(item);
        }
        if claim.trim().is_empty() {
            stop(
                &mut item,
                REFUSED,
                "no_statement",
                format!("`{at}` states no statement: its body holds only labels or a source line"),
            );
            return Ok(item);
        }
        let (anchor, context) = match self.anchor_for(&own) {
            Ok(value) => value,
            Err((code, detail)) => {
                stop(&mut item, REFUSED, &code, detail);
                return Ok(item);
            }
        };
        let to = format!("{anchor}|{}", next_side(&self.view.sides(&anchor)));
        let anchor_title = self.view.row(&anchor).map(|row| f(row, "title")).unwrap_or_default();
        let fingerprint = f(entry, "statement_fingerprint");
        let (rows, bounded) = self.carriers_of(zt, &tree_of(&anchor), revision)?;
        if bounded {
            self.bounded = true;
        }
        let carrier = rows
            .iter()
            .find(|row| f(row, "statement_fingerprint") == fingerprint && f(row, "address") != at)
            .cloned();
        let target_address = carrier.as_ref().map(|row| f(row, "address")).unwrap_or_else(|| to.clone());
        let mut model_context = context;
        model_context.sort_by(|a, b| canon::ordinal_cmp(&f(a, "address"), &f(b, "address")));
        let title = f(entry, "title");
        item["target"] = json!({
            "anchor": anchor,
            "anchor_title": anchor_title,
            "anchor_fingerprint": self.endpoints.fingerprint(&anchor),
            "book_root": self.book,
            "book_fingerprint": self.endpoints.fingerprint(&self.book),
            "to": to,
            "statement_address": target_address,
            "model_context": model_context,
        });
        item["earlier_carrier"] = match &carrier {
            Some(row) => json!({
                "address": f(row, "address"),
                "title": f(row, "title"),
                "statement_fingerprint": f(row, "statement_fingerprint"),
                "treatment": "reuse",
                "detail": "the target Literature tree already carries this exact statement; the transfer reuses it and creates no second copy",
            }),
            None => Value::Null,
        };
        item["carrier_scan"] = json!({
            "tree": tree_of(&anchor),
            "scanned": rows.len(),
            "bounded": bounded,
            "treatment": if carrier.is_some() { "reuse" } else { "create" },
        });
        item["remedies"] = json!({
            "proposed": "entry",
            "entry": Self::entry_remedy(&title, &target_address, &self.book),
            "distinct": {"supplied": true, "requires": Self::distinct_requires(&target_address, &self.book)},
        });
        Ok(item)
    }
}

// --- the report renderers ------------------------------------------------------------------------

fn render_markdown(review: &Value) -> String {
    let book = pstr(&review["concept_report"]["book"], "citation_key");
    let mut sb = Sb::new();
    sb.line(format!("# 主题回家转移复核：`[[{book}]]`")).blank();
    sb.line(format!("Report: `{}`", f(review, "report_id")));
    sb.line(format!("Concept report: `{}`", f(&review["concept_report"], "report_id")));
    sb.line(format!("Generated: {}", f(review, "created_at")));
    sb.line(format!(
        "Archive: `{}` at revision {}. Status: **{}**.",
        f(&review["archive"], "id"),
        f(&review["archive"], "revision"),
        f(review, "status")
    ))
    .blank();
    sb.line("## 边界").blank();
    sb.line(format!(
        "- 每个主题最多读 {} 条入口卡；本复核列出 {} 项。",
        f(&review["bounds"], "entries"),
        ps::pipe(&review["items"]).len()
    ));
    sb.line(format!(
        "- 文献树的同述扫描最多读 {} 张观点卡；扫描到上限时复核标为 partial。",
        f(&review["bounds"], "carriers")
    ));
    sb.line("- 主题到文献树的 `move` 仍然被拒绝：本复核从不移动卡片，只创建一条文献陈述并改写旧卡。");
    sb.line("- 模型边界：不创建模型分卡，不改变模型成员和构建顺序；需要模型判断的项标为 blocked。").blank();
    sb.line("## 完整性").blank();
    sb.line(format!(
        "- 可审阅 {} 项，refused {} 项，blocked {} 项，未决 finding {} 条，未读端点 {} 个。",
        f(&review["completeness"], "offered"),
        f(&review["completeness"], "refused"),
        f(&review["completeness"], "blocked"),
        ps::pipe(&review["pending_findings"]).len(),
        ps::pipe(&review["unread"]).len()
    ));
    for reason in ps::pipe(&review["completeness"]["reasons"]).into_iter().map(ps::s) {
        sb.line(format!("- 证据缺口：`{reason}`"));
    }
    sb.blank();
    sb.line("## 转移项").blank();
    for item in ps::ps_items(&review["items"]) {
        let carrier = if item["earlier_carrier"].is_null() {
            "创建".to_string()
        } else {
            format!("复用 `{}`", f(&item["earlier_carrier"], "address"))
        };
        let target = if item["target"].is_null() { String::new() } else { f(&item["target"], "statement_address") };
        sb.line(format!(
            "### {} `{}` → `{target}`（{}；{}）",
            f(&item, "id"),
            f(&item["statement"], "title"),
            f(&item, "status"),
            carrier
        ))
        .blank();
        if !item["reason"].is_null() {
            sb.line(format!("- 未提议：`{}` {}", f(&item["reason"], "code"), f(&item["reason"], "detail")));
        }
        sb.line(format!(
            "- 入口卡：`{}`（{}，finding {}）",
            f(&item, "entry"),
            f(&item, "verdict"),
            f(&item, "finding")
        ));
        sb.line(format!("- 原标题：{}", f(&item["statement"], "title")));
        sb.line(format!(
            "- 陈述指纹：`{}`（{} 字符）",
            f(&item["statement"], "statement_fingerprint"),
            f(&item["statement"], "chars")
        ));
        let sources: Vec<String> = ps::pipe(&item["statement"]["sources"]).into_iter().map(ps::s).collect();
        sb.line(format!("- 来源 Link：{}", sources.join(" ")));
        let formulas: Vec<String> = ps::pipe(&item["statement"]["formulas"]).into_iter().map(ps::s).collect();
        if !formulas.is_empty() {
            sb.line(format!("- 公式：{}", formulas.join(" ")));
        }
        if !item["target"].is_null() {
            sb.line(format!(
                "- 目标：文献锚点 `{}`（{}），陈述位置 `{}`",
                f(&item["target"], "anchor"),
                f(&item["target"], "anchor_title"),
                f(&item["target"], "to")
            ));
            let models: Vec<String> = ps::ps_items(&item["target"]["model_context"])
                .iter()
                .map(|row| format!("`{}`", f(row, "address")))
                .collect();
            if !models.is_empty() {
                sb.line(format!("- 模型上下文（只读检查）：{}", models.join(" ")));
            }
            sb.line(format!(
                "- 同述扫描：`{}` 读了 {} 张卡{}",
                f(&item["carrier_scan"], "tree"),
                f(&item["carrier_scan"], "scanned"),
                if ps::truthy(&item["carrier_scan"]["bounded"]) {
                    "（到上限，未读的候选不算已排除）"
                } else {
                    ""
                }
            ));
        }
        if !item["remedies"].is_null() {
            sb.line("- 旧卡改写（entry）：").blank();
            sb.line("```text");
            sb.line(f(&item["remedies"]["entry"], "body"));
            sb.line("```");
            sb.line("- 备选（distinct，需要作者写正文）：");
            for need in ps::pipe(&item["remedies"]["distinct"]["requires"]).into_iter().map(ps::s) {
                sb.line(format!("  - {need}"));
            }
        }
        sb.blank();
    }
    sb.line("## 选择").blank();
    sb.line("转移只执行审阅过的选择：`-Step apply -File <selection.json> -OperationPrefix <run/unit/rev/step>`。");
    sb.line("选择绑定本复核的 `report_id`、`finding`、`entry`、`remedy` 与一条非空说明。");
    sb.0
}

fn render_result(result: &Value) -> String {
    let mut sb = Sb::new();
    sb.line(format!("# 主题回家转移结果：`{}`", f(result, "review_report_id"))).blank();
    sb.line(format!("Result: `{}`", f(result, "result_id")));
    sb.line(format!("Prefix: `{}`", f(result, "prefix")));
    sb.line(format!(
        "Status: **{}**. Complete: {}. Broken links {} → {}.",
        f(result, "status"),
        ps::truthy(&result["complete"]),
        f(&result["broken_links"], "before"),
        f(&result["broken_links"], "after")
    ))
    .blank();
    for item in ps::ps_items(&result["items"]) {
        sb.line(format!(
            "## {} `{}` → `{}`（{}）",
            f(&item, "id"),
            f(&item, "entry"),
            f(&item, "statement_address"),
            f(&item, "status")
        ))
        .blank();
        if !item["reason"].is_null() {
            sb.line(format!("- `{}` {}", f(&item["reason"], "code"), f(&item["reason"], "detail")));
        }
        if !item["created"].is_null() {
            sb.line(format!(
                "- 创建 `{}`：陈述保留 {}，来源 Link 保留 {}，公式保留 {}，读回复核 {}",
                f(&item["created"], "address"),
                ps::truthy(&item["created"]["statement_preserved"]),
                ps::truthy(&item["created"]["sources_preserved"]),
                ps::truthy(&item["created"]["formulas_preserved"]),
                ps::truthy(&item["created"]["verified"])
            ));
        }
        if !item["rewritten"].is_null() {
            sb.line(format!(
                "- 改写 `{}`：{}，链接目标 `{}`，入链 {} → {}，读回复核 {}",
                f(&item["rewritten"], "address"),
                f(&item["rewritten"], "remedy"),
                f(&item["rewritten"], "target"),
                f(&item["rewritten"], "inbound_before"),
                f(&item["rewritten"], "inbound_after"),
                ps::truthy(&item["rewritten"]["verified"])
            ));
        }
        if !item["one_home"].is_null() {
            sb.line(format!("- 唯一实质陈述：`{}`", f(&item["one_home"], "statement_address")));
        }
        for fault in ps::pipe(&item["faults"]).into_iter().map(ps::s) {
            sb.line(format!("- 故障：{fault}"));
        }
        sb.blank();
    }
    sb.0
}

// --- the reviewed selection ----------------------------------------------------------------------

#[derive(Clone)]
struct Decision {
    finding: String,
    entry: String,
    remedy: String,
    note: String,
    draft: Option<(String, String)>,
}

fn item_of<'a>(review: &'a Value, finding: &str, entry: &str) -> Option<&'a Value> {
    review["items"]
        .as_array()?
        .iter()
        .find(|item| f(item, "finding") == finding && f(item, "entry") == entry)
}

/// §11.3: a reviewed selection binds the report id, the exact item, the remedy, the exact proposed
/// text or operation and a note. Anything the review does not support is refused.
fn validate_selection(review: &Value, asked: &Value) -> Result<Vec<Decision>> {
    let map = asked.as_object().ok_or_else(|| refuse("the selection must be an object"))?;
    for name in map.keys() {
        if !["schema_version", "report_id", "decisions"].contains(&name.as_str()) {
            return Err(refuse(format!("the selection has an unknown field `{name}`")));
        }
    }
    if asked["schema_version"].as_i64() != Some(SCHEMA_VERSION) {
        return Err(refuse("the selection needs schema_version 1"));
    }
    if f(asked, "report_id") != f(review, "report_id") {
        return Err(refuse(format!(
            "the selection belongs to report `{}`, not this report `{}`",
            f(asked, "report_id"),
            f(review, "report_id")
        )));
    }
    let rows = asked["decisions"].as_array().ok_or_else(|| refuse("the selection needs a decisions list"))?;
    if rows.is_empty() || rows.len() > MAX_DECISIONS {
        return Err(refuse(format!("a selection names one to {MAX_DECISIONS} items")));
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut accepted: Vec<Decision> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let number = index + 1;
        let object = row.as_object().ok_or_else(|| refuse(format!("decision {number} must be an object")))?;
        for name in object.keys() {
            if !["finding", "entry", "remedy", "note", "draft"].contains(&name.as_str()) {
                return Err(refuse(format!("decision {number} has an unknown field `{name}`")));
            }
        }
        let finding = f(row, "finding");
        let entry = f(row, "entry");
        let Some(item) = item_of(review, &finding, &entry) else {
            return Err(refuse(format!(
                "decision {number} names `{entry}` of finding `{finding}`, which the review does not hold"
            )));
        };
        if !seen.insert(format!("{finding}\u{0}{entry}")) {
            return Err(refuse(format!("decision {number} reviews `{entry}` of `{finding}` twice")));
        }
        if f(item, "status") != "offered" {
            return Err(refuse(format!(
                "decision {number}: `{entry}` is `{}` — {}; it is not an offered transfer",
                f(item, "status"),
                f(&item["reason"], "detail")
            )));
        }
        let remedy = f(row, "remedy");
        if !REMEDIES.contains(&remedy.as_str()) {
            return Err(refuse(format!("decision {number}: `{remedy}` is not one of {}", REMEDIES.join(", "))));
        }
        let note = f(row, "note").trim().to_string();
        if note.is_empty() {
            return Err(refuse(format!("decision {number} needs a note that states why this transfer was chosen")));
        }
        let target = f(&item["target"], "statement_address");
        let book = f(item, "book");
        let draft = match row.get("draft") {
            None | Some(Value::Null) => {
                if remedy == "distinct" {
                    return Err(refuse(format!(
                        "decision {number}: the distinct remedy needs an authored draft {{title, body}}; the review binds exact text before any write"
                    )));
                }
                None
            }
            Some(value) => {
                if remedy != "distinct" {
                    return Err(refuse(format!(
                        "decision {number}: the entry remedy takes its exact text from the review and no draft"
                    )));
                }
                let fields = value.as_object().ok_or_else(|| refuse(format!("decision {number}: draft must be an object")))?;
                for name in fields.keys() {
                    if !["title", "body"].contains(&name.as_str()) {
                        return Err(refuse(format!("decision {number}: draft has an unknown field `{name}`")));
                    }
                }
                let title = f(value, "title").trim().to_string();
                let body = f(value, "body").trim().to_string();
                if title.is_empty() || ps::len16(&title) > MAX_TITLE {
                    return Err(refuse(format!(
                        "decision {number}: the drafted title must be one nonempty line of at most {MAX_TITLE} characters"
                    )));
                }
                if body.is_empty() || ps::len16(&body) > MAX_BODY {
                    return Err(refuse(format!(
                        "decision {number}: the drafted body must be 1 through {MAX_BODY} characters"
                    )));
                }
                check_distinct_draft(&title, &body, &target, &book)?;
                Some((title, body))
            }
        };
        accepted.push(Decision { finding, entry, remedy, note, draft });
    }
    Ok(accepted)
}

/// The distinct remedy's authored body names the statement, keeps the source evidence and states a
/// claim other than the transferred one.
fn check_distinct_draft(title: &str, body: &str, target: &str, book: &str) -> Result<()> {
    if !body.contains(&format!("[[{target}]]")) {
        return Err(refuse(format!("the drafted body does not link the Literature statement `[[{target}]]`")));
    }
    let mut sourced = false;
    for raw in body.lines() {
        if concepts::SOURCE_LINE.is_match(raw.trim()) && raw.contains("[[") {
            sourced = true;
        }
    }
    if !sourced {
        return Err(refuse(format!("the drafted body keeps no source line; write `来源：[[{book}]]`")));
    }
    if let Some(claim) = body.split("\n\n").map(str::trim).find(|para| {
        !para.is_empty() && !para.starts_with('#') && !para.starts_with('-') && !concepts::SOURCE_LINE.is_match(para)
    }) {
        if claim == title.trim() {
            return Err(refuse("the drafted body repeats its own title as the claim"));
        }
    }
    Ok(())
}

// --- the apply step ------------------------------------------------------------------------------

fn read_review(work: &str) -> Result<Value> {
    let path = ps::join(work, "topic-home-review.json");
    if !ps::exists(&path) {
        return Err(refuse(format!("topic-home apply needs the reviewed report `{path}`, written by -Step plan")));
    }
    let value = canon::read_json(&path)?;
    let id = f(&value, "report_id");
    if !id.starts_with("th-") {
        return Err(refuse(format!("`{path}` is not a Topic-home review: no report id")));
    }
    let recomputed = format!("th-{}", canon::content_hash(&value, &["report_id", "created_at"]));
    if recomputed != id {
        return Err(refuse(format!(
            "the report at `{path}` does not match its own report id; regenerate it with -Step plan"
        )));
    }
    Ok(value)
}

fn apply(p: &Params) -> Result<i32> {
    if p.prefix.trim().is_empty() {
        return Err(refuse(
            "apply needs -OperationPrefix <run/unit/positive-plan-revision/step>; a retry replays the same receipt keys",
        ));
    }
    let review = read_review(&p.work)?;
    let selection_path = if p.file.is_empty() { ps::join(&p.work, "topic-home-selection.json") } else { p.file.clone() };
    if !ps::exists(&selection_path) {
        return Err(refuse(format!("topic-home apply needs the reviewed selection `{selection_path}`")));
    }
    let asked = canon::read_json(&selection_path)?;
    let decisions = validate_selection(&review, &asked)?;
    let mut zt = crate::zt::start()?;
    let (archive_id, _, revision) = archive_identity(&mut zt)?;
    if archive_id != f(&review["archive"], "id") {
        return Err(refuse(format!(
            "the review belongs to archive `{}`, but this service serves `{archive_id}`; regenerate the review on this archive",
            f(&review["archive"], "id")
        )));
    }
    let result_path = ps::join(&p.work, "topic-home-result.json");
    let hash = canon::hash_value(&asked);
    if let Ok(previous) = util::read_json(&result_path)
        && previous["selection_id"].as_str() == Some(hash.as_str())
        && previous["complete"].as_bool() == Some(true)
    {
        util::print_line(&ps::json_compact(&json!({
            "ok": true,
            "step": "apply",
            "replayed": true,
            "report_id": previous["review_report_id"],
            "result_id": previous["result_id"],
            "status": previous["status"],
            "counts": previous["counts"],
            "paths": [result_path],
        })))?;
        return Ok(0);
    }
    let broken_before = broken_links(&mut zt)?;
    let mut run = Run {
        zt,
        review,
        decisions,
        work: p.work.clone(),
        prefix: p.prefix.clone(),
        revision,
        archive_id,
        broken_before,
        max_carriers: p.max_carriers,
        created: BTreeMap::new(),
    };
    let mut payload = run.execute()?;
    payload["selection_id"] = json!(hash);
    payload["selection"] = json!(selection_path);
    payload["created_at"] = json!(canon::utc_now_o());
    payload["result_id"] = json!(format!("tr-{}", canon::content_hash(&payload, &["result_id", "created_at"])));
    canon::save_json(&result_path, &payload)?;
    let md_path = ps::join(&p.work, "topic-home-result.md");
    util::write_atomic(&md_path, &render_result(&payload))?;
    util::print_line(&ps::json_compact(&json!({
        "ok": true,
        "step": "apply",
        "replayed": false,
        "report_id": payload["review_report_id"],
        "result_id": payload["result_id"],
        "status": payload["status"],
        "complete": payload["complete"],
        "counts": payload["counts"],
        "broken_links": payload["broken_links"],
        "paths": [result_path, md_path],
    })))?;
    Ok(0)
}

fn broken_links(zt: &mut Zt) -> Result<usize> {
    let value = zt.call("list_broken_links", json!({ "workspace": "agent" }))?;
    Ok(ps::ps_items(&value["links"]).len())
}

struct Run {
    zt: Zt,
    review: Value,
    decisions: Vec<Decision>,
    work: String,
    prefix: String,
    revision: u64,
    archive_id: String,
    broken_before: usize,
    max_carriers: usize,
    /// The Side successors this run created per anchor: a slot the selection itself spent is not
    /// another writer taking the reviewed destination.
    created: BTreeMap<String, BTreeSet<String>>,
}

impl Run {
    fn execute(&mut self) -> Result<Value> {
        let mut items: Vec<Value> = Vec::new();
        for decision in self.decisions.clone() {
            let reviewed = match item_of(&self.review, &decision.finding, &decision.entry) {
                Some(item) => item.clone(),
                None => return Err(refuse(format!("the reviewed item `{}` vanished", decision.entry))),
            };
            let item = match self.one(&reviewed, &decision) {
                Ok(item) => item,
                // D270: another writer changed the archive between two reads of one item, so no
                // automated write is sent for it and the item stays pending.
                Err(error) if concepts::is_changing_view(&error) => json!({
                    "id": f(&reviewed, "id"),
                    "finding": f(&reviewed, "finding"),
                    "entry": f(&reviewed, "entry"),
                    "book": f(&reviewed, "book"),
                    "remedy": decision.remedy,
                    "note": decision.note,
                    "target": f(&reviewed["target"], "statement_address"),
                    "statement_address": f(&reviewed["target"], "statement_address"),
                    "statement_fingerprint": f(&reviewed["statement"], "statement_fingerprint"),
                    "status": PENDING,
                    "reason": {
                        "code": "quiet_window_required",
                        "detail": "the archive changed between two reads of this item, so no write was sent; read it again and retry with the same prefix",
                    },
                    "created": Value::Null,
                    "rewritten": Value::Null,
                    "one_home": Value::Null,
                    "model_reports": [],
                    "faults": [],
                }),
                Err(error) => return Err(error),
            };
            items.push(item);
        }
        let (after_id, after_path, after_revision) = archive_identity(&mut self.zt)?;
        let broken_after = broken_links(&mut self.zt)?;
        let statuses: Vec<String> = items.iter().map(|item| f(item, "status")).collect();
        let count = |status: &str| statuses.iter().filter(|known| known.as_str() == status).count();
        let counts = json!({
            "items": items.len(),
            "applied_and_verified": count(APPLIED),
            "unchanged": count(UNCHANGED),
            "conflict": count(CONFLICT),
            "refused": count(REFUSED),
            "pending": count(PENDING),
        });
        let status = if count(CONFLICT) > 0 {
            "conflict"
        } else if count(PENDING) > 0 {
            "pending"
        } else if count(REFUSED) > 0 {
            "refused"
        } else if count(APPLIED) > 0 {
            "applied"
        } else {
            "unchanged"
        };
        let complete = statuses.iter().all(|known| known != CONFLICT && known != REFUSED && known != PENDING);
        let mut applied: Vec<Value> = Vec::new();
        let mut rollback: Vec<Value> = Vec::new();
        for item in &items {
            if f(item, "status") != APPLIED && f(item, "status") != UNCHANGED {
                continue;
            }
            if !item["created"].is_null() && ps::truthy(&item["created"]["written"]) {
                applied.push(json!([f(item, "entry"), f(&item["created"], "address")]));
                rollback.push(json!({
                    "kind": "created",
                    "address": f(&item["created"], "address"),
                    "supports": "delete the created statement; nothing else refers to it yet",
                    "fingerprint": f(&item["created"], "after_fingerprint"),
                }));
            }
            if !item["rewritten"].is_null() && ps::truthy(&item["rewritten"]["written"]) {
                rollback.push(json!({
                    "kind": "rewritten",
                    "address": f(&item["rewritten"], "address"),
                    "supports": "restore the reviewed authored text of the old Card",
                    "before_fingerprint": f(&item["rewritten"], "before_fingerprint"),
                    "after_fingerprint": f(&item["rewritten"], "after_fingerprint"),
                    "before_title": f(&item["rewritten"], "before_title"),
                    "before_body": f(&item["rewritten"], "before_body"),
                    "retained": f(&item["rewritten"], "retained"),
                }));
            }
        }
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "kind": "topic-home-result",
            "review_report_id": self.review["report_id"],
            "review_created_at": self.review["created_at"],
            "concept_report_id": self.review["concept_report"]["report_id"],
            "archive": {
                "id": after_id,
                "path": after_path,
                "revision_before": self.revision,
                "revision_after": after_revision,
                "review_revision": self.review["archive"]["revision"],
            },
            "prefix": self.prefix,
            "items": items,
            "applied": applied,
            "rollback": rollback,
            "broken_links": {"before": self.broken_before, "after": broken_after},
            "counts": counts,
            "status": status,
            "complete": complete,
            "scope": "one explicitly selected review; historical live execution is a separate gate",
        }))
    }

    /// One selected item: the Literature statement first, then the guarded old-Card edit, then the
    /// read-back verification of both (D266–D274).
    fn one(&mut self, reviewed: &Value, decision: &Decision) -> Result<Value> {
        let entry = f(reviewed, "entry");
        let book = f(reviewed, "book");
        let anchor = f(&reviewed["target"], "anchor");
        let reviewed_to = f(&reviewed["target"], "to");
        let statement_fingerprint = f(&reviewed["statement"], "statement_fingerprint");
        let statement_title = f(&reviewed["statement"], "title");
        let statement_body = f(&reviewed["statement"], "body");
        let reviewed_carrier = if reviewed["earlier_carrier"].is_null() {
            None
        } else {
            Some(f(&reviewed["earlier_carrier"], "address"))
        };
        let reviewed_source = reviewed_source_fingerprint(&self.review, &entry);
        let (accepted_title, mut accepted_body) = match decision.remedy.as_str() {
            "entry" => (f(&reviewed["remedies"]["entry"], "title"), f(&reviewed["remedies"]["entry"], "body")),
            _ => decision.draft.clone().unwrap_or_default(),
        };
        let mut item = json!({
            "id": f(reviewed, "id"),
            "finding": f(reviewed, "finding"),
            "entry": entry,
            "book": book,
            "remedy": decision.remedy,
            "note": decision.note,
            "target": f(&reviewed["target"], "statement_address"),
            "statement_address": f(&reviewed["target"], "statement_address"),
            "statement_fingerprint": statement_fingerprint,
            "status": APPLIED,
            "reason": Value::Null,
            "created": Value::Null,
            "rewritten": Value::Null,
            "one_home": Value::Null,
            "model_reports": [],
            "faults": [],
        });
        fn stop(item: &mut Value, status: &str, code: &str, detail: String) {
            item["status"] = json!(status);
            item["reason"] = json!({"code": code, "detail": detail});
        }
        // 1. One view; the archive must still be this review's archive.
        let (id, _, revision) = archive_identity(&mut self.zt)?;
        if id != self.archive_id || f(&self.review["archive"], "id") != id {
            stop(
                &mut item,
                CONFLICT,
                "archive_changed",
                "the service now serves another archive than the reviewed one".to_string(),
            );
            return Ok(item);
        }
        // 2. The reviewed Cards, as they are now.
        let mut wanted: Vec<String> = vec![entry.clone(), anchor.clone(), book.clone()];
        wanted.sort_by(|a, b| canon::ordinal_cmp(a, b));
        wanted.dedup();
        let mut unread: Vec<(String, String)> = Vec::new();
        let cards = concepts::read_cards(&mut self.zt, &wanted, revision, &mut unread, "topic-home")?;
        let found: BTreeMap<String, Value> = cards.iter().map(|card| (f(card, "address"), card.clone())).collect();
        // 3. A resumed run: both writes may already be in place.
        if let Some(source) = found.get(&entry) {
            if f(source, "title") == accepted_title && f(source, "body") == accepted_body {
                let target = f(&reviewed["target"], "statement_address");
                match self.carries(&target, &statement_fingerprint) {
                    Ok(true) => {
                        item["rewritten"] = json!({
                            "address": entry, "remedy": decision.remedy, "target": target,
                            "written": false, "verified": true, "already_applied": true,
                            "inbound_before": Value::Null, "inbound_after": Value::Null,
                        });
                        stop(
                            &mut item,
                            UNCHANGED,
                            "already_applied",
                            "the old Card already carries the reviewed edit and the statement has its home; nothing was written".to_string(),
                        );
                        return Ok(item);
                    }
                    Ok(false) => {}
                    Err(error) => {
                        stop(&mut item, CONFLICT, "statement_missing", format!("{error:#}"));
                        return Ok(item);
                    }
                }
            }
        }
        let Some(source) = found.get(&entry).cloned() else {
            let why = unread.iter().find(|(at, _)| at == &entry).map(|(_, why)| why.clone()).unwrap_or_default();
            stop(
                &mut item,
                CONFLICT,
                "source_missing",
                format!("the reviewed entry `{entry}` could not be read: {why}"),
            );
            return Ok(item);
        };
        // 4. Agent writability and the model boundary are decided before any write (D273, D267).
        if !ps::truthy(&source["writable"]) || f(&source, "workspace") != "agent" {
            stop(
                &mut item,
                REFUSED,
                "read_only_workspace",
                format!("`{entry}` is not an agent Card now; its body is untouched"),
            );
            return Ok(item);
        }
        if f(&source, "type") == "model" {
            stop(
                &mut item,
                REFUSED,
                "model_boundary",
                format!(
                    "`{entry}` is a Model Card or Model part now; a separate explicit model design decision is required (D267)"
                ),
            );
            return Ok(item);
        }
        if ps::truthy(&source["restatement"]) {
            stop(
                &mut item,
                REFUSED,
                "already_entry",
                format!("`{entry}` is already a restatement entry; nothing is transferred"),
            );
            return Ok(item);
        }
        // 5. The reviewed authored fingerprints: a changed endpoint is a conflict, never a replan.
        let view = View::read(&mut self.zt, revision)?;
        let (own, other) = source_books(&f(&source, "body"), &book, &view.books());
        if own.is_empty() || !other.is_empty() {
            stop(
                &mut item,
                CONFLICT,
                "finding_changed",
                format!("`{entry}` no longer rests only on its own book; regenerate the review"),
            );
            return Ok(item);
        }
        let current = calls::authored_fingerprint(&source);
        if !reviewed_source.is_empty() && reviewed_source != current {
            stop(
                &mut item,
                CONFLICT,
                "endpoint_changed",
                format!("`{entry}` no longer matches the reviewed authored fingerprint"),
            );
            return Ok(item);
        }
        for at in [&anchor, &book] {
            let Some(row) = found.get(at) else {
                stop(&mut item, CONFLICT, "endpoint_missing", format!("`{at}` is no longer in this archive"));
                return Ok(item);
            };
            let want = reviewed_endpoint_fingerprint(&self.review, at);
            if !want.is_empty() && calls::authored_fingerprint(row) != want {
                stop(
                    &mut item,
                    CONFLICT,
                    "endpoint_changed",
                    format!("`{at}` no longer matches the reviewed authored fingerprint"),
                );
                return Ok(item);
            }
        }
        // 6. The earlier carrier: the reviewed treatment must still hold.
        let tree = tree_of(&anchor);
        // The reviewed destination is where this item's own creation lands: it is not an earlier
        // carrier, so a resumed run reads its own statement back instead of calling it a conflict.
        let carrier_now = self.carrier(revision, &tree, &statement_fingerprint, &entry, &reviewed_to)?;
        if carrier_now.as_deref() != reviewed_carrier.as_deref() {
            let detail = match (&reviewed_carrier, &carrier_now) {
                (Some(known), _) => format!("the reviewed earlier carrier `{known}` no longer carries this statement"),
                (None, Some(found)) => format!("`{found}` now carries this exact statement, so reuse needs a new review"),
                _ => "the earlier-carrier evidence changed".to_string(),
            };
            stop(&mut item, CONFLICT, "carrier_changed", detail);
            return Ok(item);
        }
        // 7. The affected model reports, read before the writes.
        let model_addresses: Vec<String> = ps::ps_items(&reviewed["target"]["model_context"])
            .iter()
            .map(|row| f(row, "address"))
            .collect();
        let model_before = self.model_parts(&model_addresses)?;
        // 8. The Literature statement: reuse the carrier, or create and verify one plain Card.
        let mut replanned = false;
        let statement_address = if let Some(known) = &reviewed_carrier {
            known.clone()
        } else if self.carries(&reviewed_to, &statement_fingerprint)? {
            // D268: the creation receipt's Card is already here (a lost reply, an interrupted run),
            // so the retry finishes the old-Card edit instead of creating another copy.
            item["created"] = json!({
                "address": reviewed_to,
                "replayed": true,
                "written": false,
                "verified": false,
                "after_fingerprint": "",
                "receipt": format!("{}/create-{}", self.prefix, f(reviewed, "id")),
            });
            reviewed_to.clone()
        } else {
            let fresh = format!("{anchor}|{}", next_side(&view.sides(&anchor)));
            if fresh != reviewed_to {
                // The reviewed slot is spent. A slot this run created for another item of the
                // same anchor is the selection's own doing, so the statement takes the next one;
                // another writer's Card beside the anchor is a conflict.
                if !self.spent(&anchor, &reviewed_to) {
                    stop(
                        &mut item,
                        CONFLICT,
                        "destination_taken",
                        format!("`{reviewed_to}` is no longer the next free slot of `{anchor}`; it is `{fresh}`"),
                    );
                    return Ok(item);
                }
                if decision.remedy != "entry" {
                    stop(
                        &mut item,
                        CONFLICT,
                        "destination_taken",
                        format!(
                            "the authored distinct draft names `{reviewed_to}`, but the next free slot is `{fresh}`; write the draft for the current slot and review it again"
                        ),
                    );
                    return Ok(item);
                }
                replanned = true;
            }
            let call = json!({"tool": "create_successor", "arguments": json!({
                "at": anchor,
                "kind": "side",
                "type": "idea",
                "title": statement_title,
                "body": statement_body,
            })});
            let key = format!("{}/create-{}", self.prefix, f(reviewed, "id"));
            match self.write(&key, "0", &call)? {
                Write::Done { address, replayed, after } => {
                    if address != reviewed_to {
                        if !replanned {
                            item["created"] = json!({
                                "address": address, "replayed": replayed, "written": !replayed,
                                "verified": false, "after_fingerprint": after, "receipt": key,
                            });
                            stop(
                                &mut item,
                                CONFLICT,
                                "destination_taken",
                                format!(
                                    "the created statement landed on `{address}`, not the reviewed `{reviewed_to}`; the created Card is preserved and a new review is required"
                                ),
                            );
                            return Ok(item);
                        }
                        // The entry names the created address: it is re-derived for it.
                        accepted_body = f(&Planner::entry_remedy(&accepted_title, &address, &book), "body");
                    }
                    self.created.entry(anchor.clone()).or_default().insert(address.clone());
                    item["created"] = json!({
                        "address": address, "replayed": replayed, "written": !replayed,
                        "verified": false, "after_fingerprint": after, "receipt": key,
                        "replanned": replanned,
                    });
                    address
                }
                Write::Conflicted { address, detail, observed } => {
                    stop(
                        &mut item,
                        CONFLICT,
                        "create_conflict",
                        format!("`{address}` conflicts: {detail} (observed `{observed}`)"),
                    );
                    return Ok(item);
                }
                Write::Refused(detail) => {
                    stop(&mut item, REFUSED, "create_refused", detail);
                    return Ok(item);
                }
                Write::Failed(detail) => {
                    // D301: a lost write response is verified by reading the anchor back.
                    match self.statement_beside(&anchor, &statement_fingerprint) {
                        Ok(Some(found)) => {
                            self.created.entry(anchor.clone()).or_default().insert(found.clone());
                            item["created"] = json!({
                                "address": found, "replayed": true, "written": true,
                                "verified": false, "after_fingerprint": "",
                                "receipt": key, "replanned": replanned, "recovered_by_readback": true,
                            });
                            found
                        }
                        _ => {
                            stop(
                                &mut item,
                                PENDING,
                                "create_failed",
                                format!(
                                    "the creation did not answer: {detail}; read the archive back, then retry with the same prefix"
                                ),
                            );
                            return Ok(item);
                        }
                    }
                }
            }
        };
        item["statement_address"] = json!(statement_address);
        item["target"] = json!(statement_address);
        // 9. Verify the substantive statement before the old Card is touched (D266).
        match self.verify_statement(&statement_address, &statement_fingerprint, reviewed) {
            Ok(verified) => {
                if !item["created"].is_null() {
                    item["created"]["verified"] = json!(true);
                    item["created"]["statement_preserved"] = verified["statement_preserved"].clone();
                    item["created"]["sources_preserved"] = verified["sources_preserved"].clone();
                    item["created"]["formulas_preserved"] = verified["formulas_preserved"].clone();
                }
            }
            Err(error) => {
                if !item["created"].is_null() {
                    item["created"]["verified"] = json!(false);
                }
                item["faults"] = json!([format!("{error:#}")]);
                stop(&mut item, CONFLICT, "statement_unverified", format!("{error:#}"));
                return Ok(item);
            }
        }
        // 10. The guarded old-Card edit: the exact accepted text, the reviewed authored base.
        let before_title = f(&source, "title");
        let before_body = f(&source, "body");
        let inbound_before = self.inbound(&entry)?;
        let edit = json!({"tool": "edit_card", "arguments": json!({
            "at": entry,
            "title": accepted_title,
            "body": accepted_body,
            "base_fingerprint": reviewed_source,
        })});
        let key = format!("{}/rewrite-{}", self.prefix, f(reviewed, "id"));
        match self.write(&key, "1", &edit)? {
            Write::Done { address, replayed, after } => {
                item["rewritten"] = json!({
                    "address": address,
                    "remedy": decision.remedy,
                    "target": statement_address,
                    "written": !replayed,
                    "replayed": replayed,
                    "receipt": key,
                    "before_title": before_title,
                    "before_body": before_body,
                    "before_fingerprint": current,
                    "after_fingerprint": after,
                    "retained": self.retain_rewrite(&entry, &before_title, &before_body, &current)?,
                    "verified": false,
                });
            }
            Write::Conflicted { address, detail, observed } => {
                item["rewritten"] = json!({
                    "address": address, "remedy": decision.remedy, "target": statement_address,
                    "written": false, "verified": false,
                    "inbound_before": inbound_before, "inbound_after": inbound_before,
                });
                item["revised_review_item"] = json!({
                    "reason": "the old Card changed after the review",
                    "entry": entry,
                    "current_fingerprint": observed,
                    "detail": detail,
                    "required": "review the old Card's current text again before any rewrite; the created statement is preserved",
                    "statement_address": statement_address,
                });
                stop(
                    &mut item,
                    CONFLICT,
                    "source_changed",
                    format!(
                        "`{entry}` changed after the review ({detail}); the created statement `{statement_address}` is preserved and the edit is not applied"
                    ),
                );
                return Ok(item);
            }
            Write::Refused(detail) => {
                stop(
                    &mut item,
                    REFUSED,
                    "rewrite_refused",
                    format!("the old-Card edit was refused: {detail}; the created statement `{statement_address}` is preserved"),
                );
                return Ok(item);
            }
            Write::Failed(detail) => {
                stop(
                    &mut item,
                    PENDING,
                    "rewrite_pending",
                    format!(
                        "the old-Card edit did not answer: {detail}; retry with the same prefix — the creation receipt is replayed, not repeated"
                    ),
                );
                return Ok(item);
            }
        }
        // 11. Read the persisted result back: the text, the Link and its kind, inbound Links, Broken
        // links, one substantive home and the model reports (D274).
        match self.verify_rewrite(&entry, &accepted_title, &accepted_body, &statement_address, &statement_fingerprint, inbound_before) {
            Ok(value) => {
                item["rewritten"]["verified"] = json!(true);
                item["rewritten"]["inbound_before"] = value["inbound_before"].clone();
                item["rewritten"]["inbound_after"] = value["inbound_after"].clone();
                item["rewritten"]["statement_fingerprint"] = value["statement_fingerprint"].clone();
                item["rewritten"]["links"] = value["links"].clone();
                item["rewritten"]["kinds"] = value["kinds"].clone();
                item["one_home"] = value["one_home"].clone();
                item["model_reports"] = json!(self.model_reports(&model_before)?);
                stop(&mut item, APPLIED, "verified", f(&value, "detail"));
            }
            Err(error) => {
                item["rewritten"]["verified"] = json!(false);
                item["faults"] = json!([format!("{error:#}")]);
                stop(&mut item, CONFLICT, "verification_failed", format!("{error:#}"));
            }
        }
        Ok(item)
    }

    /// One identified write through the archive's receipt path. A committed key replays its stored
    /// result, so a retry never creates a second copy (D268).
    fn write(&mut self, key: &str, client_ref: &str, call: &Value) -> Result<Write> {
        let item = json!({
            "op_key": key,
            "client_ref": client_ref,
            "tool": call["tool"],
            "arguments": call["arguments"],
        });
        let reply = match self.zt.request("apply_batch", json!({"items": [item]})) {
            Ok(reply) => reply,
            Err(error) => return Ok(Write::Failed(format!("{error:#}"))),
        };
        if let Some(message) = reply_error(&reply) {
            let fault = reply["result"]["structuredContent"].clone();
            if f(&fault, "code") == "write_conflict" {
                return Ok(Write::Conflicted {
                    address: f(&fault, "address"),
                    detail: message,
                    observed: f(&fault, "observed_fingerprint"),
                });
            }
            return Ok(Write::Refused(message));
        }
        let value: Value = match serde_json::from_str(&reply_text(&reply)) {
            Ok(value) => value,
            Err(error) => return Ok(Write::Failed(format!("the answer is not JSON: {error}"))),
        };
        let Some(result) = value["results"].as_array().and_then(|rows| rows.first()) else {
            return Ok(Write::Failed("no receipt in the answer".to_string()));
        };
        if f(result, "op_key") != key {
            return Ok(Write::Failed(format!("the receipt key `{}` is not `{key}`", f(result, "op_key"))));
        }
        let address = f(&result["result"], "address");
        if address.is_empty() {
            return Ok(Write::Failed("the receipt carries no address".to_string()));
        }
        Ok(Write::Done {
            address,
            replayed: ps::truthy(&result["replayed"]),
            after: f(&result["result"], "after_fingerprint"),
        })
    }

    /// Whether a Card carries the reviewed statement, read back rather than assumed. A Card that is
    /// not there carries nothing: an unfinished run is not an error.
    fn carries(&mut self, at: &str, fingerprint: &str) -> Result<bool> {
        let (_, _, revision) = archive_identity(&mut self.zt)?;
        let mut unread: Vec<(String, String)> = Vec::new();
        let cards = concepts::read_cards(&mut self.zt, &[at.to_string()], revision, &mut unread, "topic-home")?;
        Ok(cards.first().is_some_and(|card| f(card, "statement_fingerprint") == fingerprint))
    }

    /// D274: the statement is the reviewed statement. The archive's own statement fingerprint proves
    /// the claim survived, and the reviewed source Links, formulas and Broken links are checked
    /// beside it.
    fn verify_statement(&mut self, at: &str, fingerprint: &str, reviewed: &Value) -> Result<Value> {
        let (_, _, revision) = archive_identity(&mut self.zt)?;
        let mut unread: Vec<(String, String)> = Vec::new();
        let cards = concepts::read_cards(&mut self.zt, &[at.to_string()], revision, &mut unread, "topic-home")?;
        if let Some((known, why)) = unread.first() {
            return Err(refuse(format!("`{known}` could not be read back: {why}")));
        }
        let card = cards.first().context("the statement Card does not exist")?;
        if f(card, "statement_fingerprint") != fingerprint {
            return Err(refuse(format!(
                "`{at}` does not carry the reviewed statement: its statement fingerprint is `{}`",
                f(card, "statement_fingerprint")
            )));
        }
        let body = f(card, "body");
        for target in ps::pipe(&reviewed["statement"]["sources"]).into_iter().map(ps::s) {
            if !body.contains(&format!("[[{target}]]")) {
                return Err(refuse(format!("`{at}` lost the reviewed source Link `[[{target}]]`")));
            }
        }
        let now = formulas(&body);
        for formula in ps::pipe(&reviewed["statement"]["formulas"]).into_iter().map(ps::s) {
            if !now.contains(&formula) {
                return Err(refuse(format!("`{at}` lost the reviewed formula `{formula}`")));
            }
        }
        let broken = broken_links(&mut self.zt)?;
        if broken != self.broken_before {
            return Err(refuse(format!(
                "the transfer changed the Broken link count from {} to {broken}",
                self.broken_before
            )));
        }
        Ok(json!({
            "statement_preserved": true,
            "sources_preserved": true,
            "formulas_preserved": true,
        }))
    }

    /// D274: the old Card holds the accepted text at its own address, links the statement with a
    /// real Relation kind, keeps its inbound Links, adds no Broken link, leaves one substantive home
    /// and changes no model membership.
    fn verify_rewrite(
        &mut self,
        entry: &str,
        title: &str,
        body: &str,
        statement_address: &str,
        statement_fingerprint: &str,
        inbound_before: usize,
    ) -> Result<Value> {
        let (_, _, revision) = archive_identity(&mut self.zt)?;
        let mut unread: Vec<(String, String)> = Vec::new();
        let cards = concepts::read_cards(&mut self.zt, &[entry.to_string()], revision, &mut unread, "topic-home")?;
        if let Some((known, why)) = unread.first() {
            return Err(refuse(format!("`{known}` could not be read back: {why}")));
        }
        let card = cards.first().context("the rewritten Card does not exist")?;
        if f(card, "address") != entry {
            return Err(refuse(format!("the rewritten Card is not at `{entry}` any more")));
        }
        if f(card, "title") != title || f(card, "body") != body {
            return Err(refuse("the read-back Card does not carry the accepted text".to_string()));
        }
        if !f(card, "body").contains(&format!("[[{statement_address}]]")) {
            return Err(refuse(format!("the rewritten Card does not link the statement `{statement_address}`")));
        }
        let fingerprint = f(card, "statement_fingerprint");
        if fingerprint == statement_fingerprint {
            return Err(refuse(
                "the rewritten Card repeats the substantive statement; the entry is not a second copy (D266)".to_string(),
            ));
        }
        let inbound_after = self.inbound(entry)?;
        if inbound_after != inbound_before {
            return Err(refuse(format!(
                "the inbound Link count of `{entry}` changed from {inbound_before} to {inbound_after}; existing Links must still reach the old address"
            )));
        }
        let broken = broken_links(&mut self.zt)?;
        if broken != self.broken_before {
            return Err(refuse(format!(
                "the transfer changed the Broken link count from {} to {broken}",
                self.broken_before
            )));
        }
        // The old Card's Links and their actual Relation kinds, after the write.
        let full = self.zt.call("get_card", json!({ "at": entry }))?;
        let links: Vec<String> = ps::pipe(&full["outbound"]).into_iter().map(|row| f(row, "address")).collect();
        let kinds: BTreeMap<String, String> = ps::pipe(&full["outbound"])
            .into_iter()
            .map(|row| (f(row, "address"), f(row, "kind")))
            .collect();
        if !links.iter().any(|link| link == statement_address) {
            return Err(refuse(format!("the rewritten Card's Links do not reach `{statement_address}`")));
        }
        // One substantive home: the statement lives in exactly one Card of its tree.
        let view = View::read(&mut self.zt, revision)?;
        let tree = tree_of(statement_address);
        let (addresses, bounded) = view.carriers(&tree, self.max_carriers);
        let mut unread_scan: Vec<(String, String)> = Vec::new();
        let rows = concepts::read_cards(&mut self.zt, &addresses, revision, &mut unread_scan, "topic-home")?;
        let carrying: Vec<String> = rows
            .iter()
            .filter(|row| f(row, "statement_fingerprint") == statement_fingerprint)
            .map(|row| f(row, "address"))
            .collect();
        if carrying != vec![statement_address.to_string()] {
            return Err(refuse(format!(
                "the substantive statement is carried by {carrying:?}, not only by `{statement_address}`"
            )));
        }
        Ok(json!({
            "detail": "the persisted statement, source Links, Link targets and kinds, inbound Links, Broken links, one substantive home and the model reports match the reviewed item",
            "statement_fingerprint": fingerprint,
            "links": links,
            "kinds": kinds,
            "inbound_before": inbound_before,
            "inbound_after": inbound_after,
            "one_home": {
                "statement_fingerprint": statement_fingerprint,
                "statement_address": statement_address,
                "cards": carrying,
                "bounded": bounded,
                "detail": "one Card carries the substantive statement; the old Card is its entry",
            },
        }))
    }

    /// The statement this anchor now holds beside it, read back: a lost creation reply is recovered
    /// from the archive, never repeated (D301).
    fn statement_beside(&mut self, anchor: &str, fingerprint: &str) -> Result<Option<String>> {
        let card = self.zt.call("get_card", json!({ "at": anchor }))?;
        let sides: Vec<String> = ps::pipe(&card["sides"]).into_iter().map(|row| f(row, "address")).collect();
        if sides.is_empty() {
            return Ok(None);
        }
        let (_, _, revision) = archive_identity(&mut self.zt)?;
        let mut unread: Vec<(String, String)> = Vec::new();
        let rows = concepts::read_cards(&mut self.zt, &sides, revision, &mut unread, "topic-home")?;
        Ok(rows
            .iter()
            .find(|row| f(row, "statement_fingerprint") == fingerprint)
            .map(|row| f(row, "address")))
    }

    fn spent(&self, anchor: &str, at: &str) -> bool {
        self.created.get(anchor).is_some_and(|set| set.contains(at))
    }

    fn inbound(&mut self, at: &str) -> Result<usize> {
        let card = self.zt.call("get_card", json!({ "at": at }))?;
        Ok(ps::pipe(&card["inbound"]).len())
    }

    /// The model reports before the writes: the parts each named model holds.
    fn model_parts(&mut self, addresses: &[String]) -> Result<BTreeMap<String, Vec<String>>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for at in addresses {
            let report = self.zt.call("get_model", json!({ "at": at }))?;
            let parts: Vec<String> = ps::each(&report["parts"]).iter().map(|part| f(part, "address")).collect();
            out.insert(at.clone(), parts);
        }
        Ok(out)
    }

    /// The model reports after the writes: no membership and no build order moved (D267, S21-AC04).
    fn model_reports(&mut self, before: &BTreeMap<String, Vec<String>>) -> Result<Vec<Value>> {
        let mut out: Vec<Value> = Vec::new();
        for (at, was) in before {
            let report = self.zt.call("get_model", json!({ "at": at }))?;
            let parts: Vec<String> = ps::each(&report["parts"]).iter().map(|part| f(part, "address")).collect();
            if &parts != was {
                return Err(refuse(format!(
                    "the model `{at}` changed its membership: {was:?} → {parts:?}"
                )));
            }
            out.push(json!({"model": at, "parts_before": was, "parts_after": parts, "preserved": true}));
        }
        Ok(out)
    }

    /// The earlier carrier of one statement in one Literature tree, read fresh and bounded: a run
    /// that created one statement must see it before the next item decides reuse.
    fn carrier(&mut self, revision: u64, tree: &str, fingerprint: &str, entry: &str, own: &str) -> Result<Option<String>> {
        let view = View::read(&mut self.zt, revision)?;
        let (addresses, _) = view.carriers(tree, self.max_carriers);
        let mut unread: Vec<(String, String)> = Vec::new();
        let rows = concepts::read_cards(&mut self.zt, &addresses, revision, &mut unread, "topic-home")?;
        Ok(rows
            .iter()
            .find(|row| {
                let at = f(row, "address");
                f(row, "statement_fingerprint") == fingerprint && at != entry && at != own
            })
            .map(|row| f(row, "address")))
    }

    /// D272: the accepted edit keeps its retained evidence and its rollback facts.
    fn retain_rewrite(&self, entry: &str, title: &str, body: &str, fingerprint: &str) -> Result<String> {
        let card = json!({
            "address": entry,
            "title": title,
            "body": body,
            "type": "idea",
            "facet": Value::Null,
            "workspace": "agent",
            "fingerprint": fingerprint,
        });
        let (path, _) = concepts::retain_endpoint(&self.work, &card)?;
        Ok(path)
    }
}

enum Write {
    Done { address: String, replayed: bool, after: String },
    Conflicted { address: String, detail: String, observed: String },
    Refused(String),
    Failed(String),
}

/// The reviewed authored fingerprint of one endpoint Card (§11.3, D272).
fn reviewed_endpoint_fingerprint(review: &Value, at: &str) -> String {
    review["endpoints"]
        .as_object()
        .and_then(|endpoints| endpoints.get(at))
        .map(|row| f(row, "fingerprint"))
        .unwrap_or_default()
}

fn reviewed_source_fingerprint(review: &Value, entry: &str) -> String {
    reviewed_endpoint_fingerprint(review, entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(address: &str, kind: &str, card_type: &str, facet: Option<&str>, title: &str) -> Value {
        json!({
            "address": address,
            "tree": tree_of(address),
            "workspace": "agent",
            "kind": kind,
            "type": card_type,
            "facet": facet,
            "title": title,
            "restatement": false,
            "edge": if kind == "regular" { "direct" } else { "root" },
        })
    }

    fn view(rows: Vec<Value>) -> View {
        let mut view = View::default();
        for row in rows {
            let at = f(&row, "address");
            if f(&row, "edge") == "root" {
                view.zones.insert(tree_of(&at), f(&row, "kind"));
            }
            view.rows.insert(at, row);
        }
        view
    }

    fn books(list: &[&str]) -> HashSet<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_entry_chain_and_the_side_slot_follow_the_zt_location_rules() {
        let view = view(vec![
            row("alpha", "literature", "idea", None, "书"),
            row("alpha/1", "regular", "idea", None, "第 1 组"),
            row("0/0", "topic", "idea", None, "主题"),
            row("0/1", "regular", "idea", None, "入口一"),
            row("0/2", "regular", "idea", None, "入口二"),
        ]);
        assert_eq!(view.chain("0/0"), vec!["0/1".to_string(), "0/2".to_string()]);
        assert_eq!(view.chain("alpha"), vec!["alpha/1".to_string()]);
        assert_eq!(next_side(&[]), "a");
        assert_eq!(next_side(&["a".to_string()]), "b");
        assert_eq!(next_side(&["a".to_string(), "c".to_string()]), "d");
        assert_eq!(side_label(side_number("ab")), "ab");
        assert_eq!(direct_successor("0/1"), "0/2");
        assert_eq!(direct_successor("alpha/1|a"), "alpha/1|a|1");
        assert_eq!(view.zone("0/1"), "topic");
        assert_eq!(view.zone("alpha/1"), "literature");
    }

    #[test]
    fn a_book_bound_entry_names_its_own_book_and_no_other() {
        let books = books(&["alpha", "beta"]);
        let body = "陈述一句。\n\n来源：[[alpha]] §2";
        let (own, other) = source_books(body, "alpha", &books);
        assert_eq!(own, vec!["alpha".to_string()]);
        assert!(other.is_empty());
        let mixed = "陈述一句。\n\n来源：[[alpha]] §2 与 [[beta]] §4";
        let (own, other) = source_books(mixed, "alpha", &books);
        assert_eq!(own, vec!["alpha".to_string()]);
        assert_eq!(other, vec!["beta".to_string()]);
        let foreign = "陈述一句。\n\n来源：[[beta]] §4";
        assert!(source_books(foreign, "alpha", &books).0.is_empty());
        let (own, other) = source_books("来源：[[alpha/3]] §1", "alpha", &books);
        assert_eq!(own, vec!["alpha/3".to_string()]);
        assert!(other.is_empty());
    }

    #[test]
    fn the_statement_text_drops_source_lines_and_keeps_formulas() {
        let body = "当 $x=1$ 时均衡唯一。\n\n假设：同一产业。\n\n来源：[[alpha]] §2\n";
        assert_eq!(statement_text(body), "当 $x=1$ 时均衡唯一。\n\n假设：同一产业。");
        assert_eq!(formulas(body), vec!["$x=1$".to_string()]);
        let fenced = "```\n来源：[[alpha]]\n```";
        assert_eq!(statement_text(fenced), fenced);
    }

    #[test]
    fn the_entry_remedy_is_navigation_with_one_terminal_link_and_a_source_line() {
        let remedy = Planner::entry_remedy("对称参与者有两个均衡", "alpha/1|a", "alpha");
        let body = f(&remedy, "body");
        assert_eq!(f(&remedy, "title"), "对称参与者有两个均衡");
        assert!(body.contains(&format!("## {ENTRY_HEADING}")), "{body}");
        assert_eq!(body.matches("[[alpha/1|a]]").count(), 1, "{body}");
        assert!(body.contains("来源：[[alpha]]"), "{body}");
        let (inside, entry) = body.split_once(&format!("## {ENTRY_HEADING}")).unwrap();
        let after = entry.split_once("来源：").map(|(a, _)| a).unwrap_or(entry);
        assert_eq!(after.matches("[[").count(), 1, "{body}");
        assert!(!inside.contains("[[alpha/1|a]]"), "the statement is not repeated above the entry");
        // The entry section's list item carries a reason (L2) and the source line is separate (L3).
        assert!(after.contains("按本书的原话"), "{body}");
    }

    #[test]
    fn the_anchor_is_a_plain_card_of_the_book_and_a_model_is_context_only() {
        let planner = Planner {
            work: "",
            book: "alpha".to_string(),
            view: view(vec![
                row("alpha", "literature", "idea", None, "书"),
                row("alpha/1", "regular", "idea", None, "第 1 组"),
                row("alpha/1|a", "regular", "idea", None, "入口"),
                row("alpha/2", "regular", "model", None, "模型"),
                row("alpha/2|a", "regular", "model", Some("设定"), "设定"),
            ]),
            endpoints: Endpoints::default(),
            carriers: HashMap::new(),
            unread: Vec::new(),
            bounded: false,
            max_carriers: 60,
        };
        let (anchor, context) = planner.anchor_for(&["alpha/1|a".to_string()]).unwrap();
        assert_eq!(anchor, "alpha/1|a");
        assert!(context.is_empty());
        let (anchor, context) = planner.anchor_for(&["alpha/2".to_string()]).unwrap();
        assert_eq!(anchor, "alpha/1");
        assert_eq!(context.len(), 1);
        assert_eq!(f(&context[0], "address"), "alpha/2");
        assert_eq!(f(&context[0], "kind"), "model");
        let planner = Planner {
            work: "",
            book: "alpha".to_string(),
            view: view(vec![
                row("alpha", "literature", "idea", None, "书"),
                row("alpha/2", "regular", "model", None, "模型"),
                row("alpha/2|a", "regular", "model", Some("设定"), "设定"),
            ]),
            endpoints: Endpoints::default(),
            carriers: HashMap::new(),
            unread: Vec::new(),
            bounded: false,
            max_carriers: 60,
        };
        let refused = planner.anchor_for(&["alpha/2".to_string()]).unwrap_err();
        assert_eq!(refused.0, "no_anchor");
        assert!(refused.1.contains("model design decision"), "{}", refused.1);
    }

    #[test]
    fn the_carrier_scan_is_bounded_and_says_so() {
        let mut rows = vec![row("alpha", "literature", "idea", None, "书")];
        for n in 1..=3 {
            rows.push(row(&format!("alpha/1|{}", side_label(n)), "regular", "idea", None, "陈述"));
        }
        rows.push(row("alpha/2", "regular", "model", Some("设定"), "设定"));
        let view = view(rows);
        let (addresses, bounded) = view.carriers("alpha", 60);
        assert!(!bounded);
        assert_eq!(addresses.len(), 4, "the Idea Cards of the tree, the model part left out");
        let (addresses, bounded) = view.carriers("alpha", 2);
        assert!(bounded);
        assert_eq!(addresses.len(), 2);
    }

    fn review_for_selection() -> Value {
        json!({
            "schema_version": 1,
            "report_id": "th-abc",
            "items": [
                {"id": "t001", "finding": "c002", "entry": "0/2", "book": "alpha", "status": "offered",
                 "target": {"statement_address": "alpha/1|a", "to": "alpha/1|a"},
                 "remedies": {"proposed": "entry", "entry": {"title": "T", "body": "B"}}},
                {"id": "t002", "finding": "c003", "entry": "0/3", "book": "alpha", "status": "blocked",
                 "reason": {"code": "model_boundary", "detail": "needs a model decision"},
                 "target": {"statement_address": "alpha/1|b", "to": "alpha/1|b"}}
            ]
        })
    }

    #[test]
    fn a_selection_binds_the_review_the_item_the_remedy_and_the_note() {
        let review = review_for_selection();
        let good = json!({"schema_version": 1, "report_id": "th-abc", "decisions": [
            {"finding": "c002", "entry": "0/2", "remedy": "entry", "note": "把本书的陈述转成文献陈述。"}
        ]});
        let decisions = validate_selection(&review, &good).unwrap();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].remedy, "entry");
        assert!(decisions[0].draft.is_none());
        for (label, asked) in [
            ("another report", json!({"schema_version":1,"report_id":"th-other","decisions":[{"finding":"c002","entry":"0/2","remedy":"entry","note":"x"}]})),
            ("an unknown item", json!({"schema_version":1,"report_id":"th-abc","decisions":[{"finding":"c002","entry":"0/9","remedy":"entry","note":"x"}]})),
            ("a blocked item", json!({"schema_version":1,"report_id":"th-abc","decisions":[{"finding":"c003","entry":"0/3","remedy":"entry","note":"x"}]})),
            ("an unknown remedy", json!({"schema_version":1,"report_id":"th-abc","decisions":[{"finding":"c002","entry":"0/2","remedy":"move","note":"x"}]})),
            ("no note", json!({"schema_version":1,"report_id":"th-abc","decisions":[{"finding":"c002","entry":"0/2","remedy":"entry","note":"  "}]})),
            ("a distinct item without a draft", json!({"schema_version":1,"report_id":"th-abc","decisions":[{"finding":"c002","entry":"0/2","remedy":"distinct","note":"x"}]})),
            ("a draft on an entry item", json!({"schema_version":1,"report_id":"th-abc","decisions":[{"finding":"c002","entry":"0/2","remedy":"entry","note":"x","draft":{"title":"T","body":"B"}}]})),
            ("a draft that does not link the statement", json!({"schema_version":1,"report_id":"th-abc","decisions":[{"finding":"c002","entry":"0/2","remedy":"distinct","note":"x","draft":{"title":"T","body":"这句解释没有链接。\n\n来源：[[alpha]]"}}]})),
            ("a draft without a source line", json!({"schema_version":1,"report_id":"th-abc","decisions":[{"finding":"c002","entry":"0/2","remedy":"distinct","note":"x","draft":{"title":"T","body":"本主题解释 [[alpha/1|a]] 的对照。"}}]})),
            ("a draft that repeats its title", json!({"schema_version":1,"report_id":"th-abc","decisions":[{"finding":"c002","entry":"0/2","remedy":"distinct","note":"x","draft":{"title":"同一句","body":"同一句\n\n来源：[[alpha]]"}}]})),
            ("a duplicate item", json!({"schema_version":1,"report_id":"th-abc","decisions":[
                {"finding":"c002","entry":"0/2","remedy":"entry","note":"x"},
                {"finding":"c002","entry":"0/2","remedy":"entry","note":"y"}]})),
        ] {
            assert!(validate_selection(&review, &asked).is_err(), "{label} must be refused");
        }
        let distinct = json!({"schema_version": 1, "report_id": "th-abc", "decisions": [
            {"finding": "c002", "entry": "0/2", "remedy": "distinct", "note": "写出这个主题自己的解释。",
             "draft": {"title": "本主题的解释与本书不同", "body": "本主题把均衡看作习惯的结果。\n\n本卡对照 [[alpha/1|a]] 的模型。\n\n来源：[[alpha]] §2"}}
        ]});
        let decisions = validate_selection(&review, &distinct).unwrap();
        assert_eq!(decisions[0].remedy, "distinct");
        assert!(decisions[0].draft.is_some());
    }

    #[test]
    fn the_review_id_is_a_content_hash_and_the_same_archive_gives_the_same_id() {
        let mut payload = json!({"schema_version": 1, "kind": "topic-home-review", "items": [], "created_at": "t"});
        let id = format!("th-{}", canon::content_hash(&payload, &["report_id", "created_at"]));
        payload["report_id"] = json!(id.clone());
        assert!(id.starts_with("th-") && id.len() == 67);
        payload["created_at"] = json!("later");
        assert_eq!(
            format!("th-{}", canon::content_hash(&payload, &["report_id", "created_at"])),
            id,
            "a new run over the same evidence keeps the same review id"
        );
        payload["items"] = json!([{"id": "t001"}]);
        assert_ne!(format!("th-{}", canon::content_hash(&payload, &["report_id", "created_at"])), id);
    }

    #[test]
    fn the_bounds_are_the_named_constants() {
        assert_eq!(MAX_ITEMS, 50);
        assert_eq!(MAX_ENTRIES, 12);
        assert_eq!(MAX_CARRIERS, 60);
        assert_eq!(MAX_DECISIONS, 50);
        assert_eq!(REMEDIES, ["entry", "distinct"]);
    }
}
