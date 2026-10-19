//! `zt-digest join`: the existing-Topic pair review (the P5 report of
//! docs/luhmann-comparison-implementation-design.md §7.3) and the report-bound application of one
//! approved join (§11.2, §11.3, §12.3; GitHub issue #109, slice S22).
//!
//!   zt-digest join -Step plan -Work <folder> [-Similar 0.5] [-MaxPairs 10] [-MaxOperations 4]
//!   zt-digest join -Step apply -Work <folder> [-File <selection.json>]
//!
//! The `plan` step reads the agent Workspace through one `zt mcp` and writes the review files
//! `topic-join-review.json`, `topic-join-review.md` and one retained endpoint body per read Card
//! under `<Work>\endpoints\`. It changes no Card: its only move calls are `move_card` previews,
//! which answer `applied: false`. For every near-duplicate Topic pair it lists the full meanings,
//! the proposed keeper, the best joining Card, the source mix and the concrete alternatives:
//! keep both, keep both with a concept crossing through an existing Hub level, or move one
//! identified agent subtree (D155–D159). A Topic root is never an operation: each movable entry
//! chain is enumerated with the `move_card` plan it would get today, every mapped address, the
//! Link rewrite count and the finished-model effect. An operation the service would refuse (a human
//! Link rewrite, a Zone or Workspace boundary) is recorded as refused beside its nonmoving
//! alternative; nothing works around the refusal (D305).
//!
//! The `apply` step consumes `-File` (default `<Work>\join-selection.json`), a reviewed selection
//! that binds the report id, the finding and plan ids, the reviewed authored fingerprints and a
//! nonempty note. It checks the archive identity (D272) and applies one selected plan's operations
//! one at a time. Before each move it re-reads the moving Cards, compares the fresh `move_card`
//! plan with the reviewed one (D269: the confirmation word is not a frozen plan id, so a fresh
//! plan that differs is a conflict, never a silent replan), checks the model boundary again and
//! stops when another writer changed the archive in the interval (D270: a quiet migration window
//! is requested, never weakened). After each move it reads the returned mapping back, verifies the
//! persisted addresses, the authored fingerprints, the Link rewrites, the Broken links and the
//! protected model reports, and refreshes the pending operations' addresses with that mapping
//! (D271, D274). Every operation and finding ends `applied-and-verified`, `unchanged`, `conflict`,
//! `refused` or `pending`; a tool that returned without a process error is never enough.
//!
//! Historical live execution is a separate gate: this step executes only the exact plans a caller
//! selected in a reviewed selection file, and its evidence scope is a scratch archive.

use crate::args::Args;
use crate::calls;
use crate::canon;
use crate::concepts::{self, bigrams, line, similarity, sources, statement};
use crate::organize::common::{self, Sb};
use crate::ps;
use crate::snapshot;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

// --- the fixed bounds (§12.2; reported so a comparison is reproducible) ---------------------------

/// A Topic pair is a candidate when the character-bigram similarity of its title and question
/// reaches this value (the cheap §7.1 comparison; never a semantic verdict).
pub const SIMILAR: f64 = 0.5;
/// At most ten existing Topic pairs per review.
pub const MAX_PAIRS: i64 = 10;
/// At most four entry chains enumerated per pair (each is a real `move_card` preview).
pub const MAX_OPERATIONS: i64 = 4;
/// The agent Topic catalogue one review may scan.
const MAX_TOPICS: usize = 500;
/// A Topic's meanings read its own numbered entries, at most three (the concept review's bound).
const ENTRIES_PER_TOPIC: usize = 3;
/// A plan that would move or renumber more Cards than this is reported as too large, not offered.
pub const MAX_PLAN_CARDS: usize = 120;
/// The retained endpoints one review may name.
const MAX_ENDPOINTS: usize = 2_000;
/// A reviewed selection names at most twenty findings.
const MAX_DECISIONS: usize = 20;
/// Two questions below this bigram similarity ask different questions, so the pair is kept apart
/// unless a reviewer decides otherwise.
const QUESTION_FLOOR: f64 = 0.34;
const SCHEMA_VERSION: i64 = 1;
/// The three concrete operations §7.3 offers for one existing Topic pair (D156–D158).
pub const ACTIONS: [&str; 3] = ["keep_distinct", "keep_both_with_crossing", "move_subtree"];


fn refuse(message: impl Into<String>) -> anyhow::Error {
    canon::refuse(message.into())
}

// --- the command ----------------------------------------------------------------------------------

struct Params {
    step: String,
    work: String,
    file: String,
    similar: f64,
    max_pairs: usize,
    max_operations: usize,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Step", "Work", "File", "Similar", "MaxPairs", "MaxOperations"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &[])?;
    let step = a.choice("Step", &["plan", "apply"])?;
    let p = Params {
        step,
        work: common::mandatory(&a, "Work")?,
        file: a.str("File"),
        similar: a.float_or("Similar", SIMILAR)?,
        max_pairs: canon::integer_option(&a.str_or("MaxPairs", &MAX_PAIRS.to_string()), "MaxPairs", 1, MAX_PAIRS)? as usize,
        max_operations: canon::integer_option(
            &a.str_or("MaxOperations", &MAX_OPERATIONS.to_string()),
            "MaxOperations",
            1,
            MAX_OPERATIONS,
        )? as usize,
    };
    if !(0.0..=1.0).contains(&p.similar) {
        return Err(refuse("Similar must be between 0 and 1"));
    }
    match p.step.as_str() {
        "plan" => plan(&p),
        _ => apply(&p),
    }
}

// --- Location rules (the same rules zt places Cards by) ------------------------------------------

fn f(value: &Value, key: &str) -> String {
    ps::s(&value[key])
}

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

/// Whether the address is a Topic root (`<n>/0`): a Topic root cannot be moved (D159).
fn topic_root(at: &str) -> bool {
    match at.split_once('/') {
        Some((tree, rest)) => rest == "0" && !tree.is_empty() && tree.bytes().all(|byte| byte.is_ascii_digit()),
        None => false,
    }
}

/// `parent_location`: the Card this one continues.
fn parent(at: &str) -> Option<String> {
    if topic_root(at) {
        return None;
    }
    let last = last_segment(at);
    if side_segment(last) {
        return at.rsplit_once('|').map(|(prefix, _)| prefix.to_string());
    }
    let number: u64 = last.parse().ok()?;
    if number > 1 {
        return Some(replace_last(at, &(number - 1).to_string()));
    }
    if let Some((prefix, _)) = at.rsplit_once('|') {
        Some(prefix.to_string())
    } else {
        Some(format!("{}/0", tree_of(at)))
    }
}

/// `direct_successor`: the one continuation slot after a Card.
fn direct_successor(at: &str) -> String {
    if topic_root(at) {
        return format!("{}/1", tree_of(at));
    }
    match last_segment(at).parse::<u64>() {
        Ok(number) => replace_last(at, &(number + 1).to_string()),
        Err(_) => format!("{at}|1"),
    }
}

fn in_tree(target: &str, tree: &str) -> bool {
    target == tree || target.starts_with(&format!("{tree}/"))
}

fn ancestors(at: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = parent(at);
    while let Some(here) = current {
        if out.contains(&here) {
            break;
        }
        out.push(here.clone());
        current = parent(&here);
    }
    out
}

/// Every Location filed under `root`, by parent Location, inside `titles`.
fn descendants(titles: &BTreeMap<String, Value>, root: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for at in titles.keys() {
        if at == root {
            continue;
        }
        if at.starts_with(&format!("{root}/")) || at.starts_with(&format!("{root}|")) || ancestors(at).iter().any(|here| here == root) {
            out.insert(at.clone());
        }
    }
    out
}

fn similarity_text(value: f64) -> String {
    format!("{value:.4}")
}

// --- the archive view -----------------------------------------------------------------------------

/// One read view of the agent Workspace: the archive identity, the title catalogue of every Card,
/// the agent Topic roots and the agent Literature trees.
struct Snapshot {
    id: String,
    path: String,
    revision: u64,
    titles: BTreeMap<String, Value>,
    topics: Vec<Value>,
    books: Vec<String>,
}

fn archive_identity(zt: &mut Zt) -> Result<(String, String, u64)> {
    let view = zt.archive_view()?;
    let id = view["archive_identity"]["id"].as_str().context("incomplete archive identity: no id")?.to_string();
    let path = view["archive_identity"]["path"].as_str().context("incomplete archive identity: no path")?.to_string();
    let revision = view["archive_revision"].as_u64().context("incomplete archive identity: no revision")?;
    Ok((id, path, revision))
}

fn read_snapshot(zt: &mut Zt, revision: u64) -> Result<Snapshot> {
    let trees = snapshot::read_title_trees(zt)?;
    let mut titles: BTreeMap<String, Value> = BTreeMap::new();
    let mut topics: Vec<Value> = Vec::new();
    let mut books: Vec<String> = Vec::new();
    for tree in &trees {
        if tree["archive_revision"].as_u64() != Some(revision) {
            return Err(concepts::changing_view());
        }
        if f(tree, "workspace") != "agent" {
            continue;
        }
        let kind = f(tree, "kind");
        if kind == "literature" {
            books.push(f(tree, "tree"));
        }
        for row in ps::each(&tree["cards"]) {
            let at = f(row, "address");
            titles.insert(at.clone(), row.clone());
            if kind == "topic" && f(row, "edge") == "root" {
                topics.push(row.clone());
            }
        }
    }
    if topics.len() > MAX_TOPICS {
        bail!("a Topic join review scans at most {MAX_TOPICS} agent Topics; this archive has {}", topics.len());
    }
    topics.sort_by(|a, b| canon::ordinal_cmp(&f(a, "address"), &f(b, "address")));
    books.sort_by(|a, b| canon::ordinal_cmp(a, b));
    Ok(Snapshot { id: String::new(), path: String::new(), revision, titles, topics, books })
}

impl Snapshot {
    fn title(&self, at: &str) -> String {
        self.titles.get(at).map(|row| f(row, "title")).unwrap_or_default()
    }

    fn has(&self, at: &str) -> bool {
        self.titles.contains_key(at)
    }

    /// The Topic's entry chain: its Direct successors, `<tree>/1`, `<tree>/2`, … in order.
    fn chain(&self, root: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut at = direct_successor(root);
        while self.titles.contains_key(&at) && out.len() < 400 {
            out.push(at.clone());
            at = direct_successor(&at);
        }
        out
    }

    /// The side successors of one Card, in label order.
    fn sides(&self, at: &str) -> Vec<String> {
        let prefix = format!("{at}|");
        let mut out: Vec<(u64, String)> = self
            .titles
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
}

// --- retained endpoints ---------------------------------------------------------------------------

/// The Cards the review read, their retained bodies and authored fingerprints (§11.3).
#[derive(Default)]
struct Endpoints {
    cards: BTreeMap<String, Value>,
    files: BTreeMap<String, Value>,
    unread: Vec<(String, String)>,
}

impl Endpoints {
    fn keep(&mut self, work: &str, card: &Value) -> Result<()> {
        let at = f(card, "address");
        if self.files.contains_key(&at) || self.unread.iter().any(|(known, _)| known == &at) {
            return Ok(());
        }
        if self.files.len() >= MAX_ENDPOINTS {
            return Err(refuse(format!(
                "a Topic join review keeps at most {MAX_ENDPOINTS} endpoints; this archive needs more"
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
                "path": path,
                "sha256": sha256,
            }),
        );
        self.cards.insert(at, card.clone());
        Ok(())
    }

    fn fingerprint(&self, at: &str) -> Option<String> {
        self.files.get(at).and_then(|row| row["fingerprint"].as_str()).map(str::to_string)
    }

    fn statement(&self, at: &str) -> String {
        self.cards.get(at).map(|card| statement(&f(card, "body"))).unwrap_or_default()
    }

    fn body(&self, at: &str) -> String {
        self.cards.get(at).map(|card| f(card, "body")).unwrap_or_default()
    }

    fn is_unread(&self, at: &str) -> bool {
        self.unread.iter().any(|(known, _)| known == at)
    }
}

// --- the plan step --------------------------------------------------------------------------------

/// One Topic pair candidate, before its entries are read.
struct Candidate {
    a: String,
    b: String,
    similarity: f64,
    inside: bool,
}

/// The cheap §7.1 comparison over every existing agent Topic pair: character bigrams of the title
/// and the question, and one title inside the other. It selects what to read; it never decides that
/// two Topics mean the same.
fn pair_candidates(topics: &[Value], questions: &HashMap<String, String>, floor: f64, max: usize) -> (Vec<Candidate>, usize) {
    let grams: HashMap<String, HashSet<String>> = topics
        .iter()
        .map(|topic| {
            let root = f(topic, "address");
            let text = format!("{} {}", f(topic, "title"), questions.get(&root).cloned().unwrap_or_default());
            (root, bigrams(&text))
        })
        .collect();
    let mut found: Vec<Candidate> = Vec::new();
    for (index, left) in topics.iter().enumerate() {
        let left_root = f(left, "address");
        for right in topics.iter().skip(index + 1) {
            let right_root = f(right, "address");
            let left_title = f(left, "title");
            let right_title = f(right, "title");
            let score = similarity(&grams[&left_root], &grams[&right_root]);
            let inside =
                !left_title.is_empty() && !right_title.is_empty() && (left_title.contains(right_title.as_str()) || right_title.contains(left_title.as_str()));
            if score >= floor || inside {
                found.push(Candidate { a: left_root.clone(), b: right_root, similarity: score, inside });
            }
        }
    }
    found.sort_by(|x, y| {
        y.similarity
            .partial_cmp(&x.similarity)
            .unwrap_or(Ordering::Equal)
            .then_with(|| canon::ordinal_cmp(&x.a, &y.a))
            .then_with(|| canon::ordinal_cmp(&x.b, &y.b))
    });
    let matched = found.len();
    found.truncate(max);
    (found, matched)
}

/// The books one Topic's read entries name on their `来源：` lines, and how many of those entries
/// carry a source line at all.
fn source_evidence(snap: &Snapshot, endpoints: &Endpoints, entries: &[String]) -> (Vec<String>, usize) {
    let mut books: BTreeSet<String> = BTreeSet::new();
    let mut sourced = 0usize;
    for at in entries {
        let declared = sources(&endpoints.body(at));
        if declared.is_empty() {
            continue;
        }
        sourced += 1;
        for target in declared {
            if let Some(book) = snap.books.iter().find(|book| in_tree(&target, book)) {
                books.insert(book.clone());
            }
        }
    }
    (books.into_iter().collect(), sourced)
}

/// D156/D159: the keeper is proposed from evidence, never from the smaller tree's Card count.
fn keeper_choice(a: &str, a_books: &[String], a_sourced: usize, b: &str, b_books: &[String], b_sourced: usize) -> (String, &'static str, String) {
    if a_books.len() != b_books.len() {
        let keep = if a_books.len() > b_books.len() { a } else { b };
        let drop = if keep == a { b } else { a };
        let (keep_books, drop_books) = if keep == a { (a_books.len(), b_books.len()) } else { (b_books.len(), a_books.len()) };
        return (
            keep.to_string(),
            "books",
            format!(
                "`{keep}` carries {keep_books} book(s) on its read entries; `{drop}` carries {drop_books}. The keeper is the home that already holds the wider source mix."
            ),
        );
    }
    if a_sourced != b_sourced {
        let keep = if a_sourced > b_sourced { a } else { b };
        let drop = if keep == a { b } else { a };
        let (keep_sourced, drop_sourced) = if keep == a { (a_sourced, b_sourced) } else { (b_sourced, a_sourced) };
        return (
            keep.to_string(),
            "sourced_entries",
            format!(
                "Both carry the same book set, so the keeper is the one whose read entries state their sources: `{keep}` has {keep_sourced}, `{drop}` has {drop_sourced}."
            ),
        );
    }
    (
        a.to_string(),
        "unresolved",
        format!(
            "`{a}` and `{b}` name the same number of books and sourced entries, so neither is the keeper on evidence; the Card count is never the reason (D159)."
        ),
    )
}

/// The best joining Card: the non-keeper entry closest in words to the keeper's question and its
/// entries; else the keeper's own first entry, named as an anchor rather than a match.
/// The best joining Card: the keeper's entry closest in words to the other Topic's question and its
/// entries, so the moved line attaches where the two lines already discuss the same thing; else the
/// keeper's first entry, named as an anchor rather than a match.
fn joining_choice(snap: &Snapshot, endpoints: &Endpoints, keeper: &str, other_root: &str, other_entries: &[String]) -> Value {
    let keeper_entries = snap.chain(keeper);
    let mut target = format!(
        "{} {} {}",
        snap.title(other_root),
        endpoints.statement(&format!("{other_root}/0")),
        snap.title(keeper)
    );
    for at in other_entries {
        target.push(' ');
        target.push_str(&endpoints.statement(at));
    }
    let query = bigrams(&target);
    let mut best: Option<(f64, String)> = None;
    for at in &keeper_entries {
        let score = similarity(&query, &bigrams(&format!("{} {}", snap.title(at), endpoints.statement(at))));
        if score > 0.0 && best.as_ref().is_none_or(|(known, _)| score > *known) {
            best = Some((score, at.clone()));
        }
    }
    match best {
        Some((score, at)) => json!({
            "address": at,
            "title": snap.title(&at),
            "similarity": similarity_text(score),
            "why": "this keeper entry is closest in words to the other Topic's question and entries; it holds the join",
        }),
        None => {
            let anchor = keeper_entries.first().cloned().unwrap_or_else(|| keeper.to_string());
            json!({
                "address": anchor,
                "title": snap.title(&anchor),
                "similarity": "0.0000",
                "why": "no keeper entry matches the other Topic in words; the keeper's first entry anchors the join",
            })
        }
    }
}

/// The concrete keep-both-with-crossing alternative: an existing Hub level, never a new type (§8.1).
fn crossing_alternative(snap: &Snapshot, keeper: &str, keeper_books: &[String], other_books: &[String], joining: &Value) -> Value {
    let keeper_tree = tree_of(keeper);
    let hub = snap
        .titles
        .iter()
        .filter(|(_, row)| f(row, "type") == "hub" && f(row, "tree") == keeper_tree)
        .map(|(at, _)| at.clone())
        .min_by(|a, b| canon::ordinal_cmp(a, b));
    let shared: Vec<&String> = keeper_books.iter().filter(|book| other_books.contains(book)).collect();
    if let Some(hub) = hub {
        return json!({
            "action": "keep_both_with_crossing",
            "level": "topic",
            "anchor": keeper,
            "hub": hub,
            "group": "邻近",
            "joining": joining["address"],
            "description": format!("Keep both Topics and write the other treatment into the existing hub Card `{hub}` of `{keeper}` under `邻近`, as a line that says what it adds. The hub stage owns that write; this review writes no Card."),
            "reason": "an existing hub Card already navigates this Topic",
        });
    }
    if !shared.is_empty() {
        return json!({
            "action": "keep_both_with_crossing",
            "level": "topic",
            "anchor": keeper,
            "hub": Value::Null,
            "group": "入口",
            "joining": joining["address"],
            "description": format!("Keep both Topics and file the other treatment as an entry of `{keeper}` in its `## 入口` section, with the reason this line adds. The hub stage owns that write; this review writes no Card."),
            "reason": format!("both Topics touch {} shared book(s), so one home with a crossing line is enough", shared.len()),
        });
    }
    json!({
        "action": "keep_both_with_crossing",
        "level": "field",
        "anchor": keeper,
        "hub": Value::Null,
        "group": "邻近",
        "joining": joining["address"],
        "description": format!("Keep both Topics and place a field hub under the register that lists both treatments of this concept, with the keeper `{keeper}` as the first line. The hub stage owns that write; this review writes no Card."),
        "reason": "the two Topics rest on different books, so the concept spans more than one home",
    })
}

/// A `move_card` preview parsed: what the service would change today.
struct Preview {
    mapping: BTreeMap<String, String>,
    links_rewritten: usize,
    lines: Vec<String>,
}

fn preview_move(zt: &mut Zt, at: &str, to: &str, alone: bool) -> Result<std::result::Result<Preview, String>> {
    let request = if alone { json!({ "at": at, "to": to, "alone": true }) } else { json!({ "at": at, "to": to }) };
    let reply = zt.request("move_card", request)?;
    if let Some(message) = zt_check::mcp::reply_error(&reply) {
        return Ok(Err(message));
    }
    let value: Value = serde_json::from_str(&zt_check::mcp::reply_text(&reply)).context("move_card preview is not JSON")?;
    if value["applied"].as_bool() == Some(true) {
        return Err(refuse("a move preview applied a Card; the plan step must never write"));
    }
    let lines: Vec<String> = ps::each(&value["plan"]).iter().map(|row| ps::s(row)).collect();
    let (mapping, links_rewritten) = parse_plan_lines(&lines)?;
    Ok(Ok(Preview { mapping, links_rewritten, lines }))
}

/// Parse `move verification:` lines: `old -> new` pairs, then `moved cards` and
/// `link macros rewritten`.
fn parse_plan_lines(lines: &[String]) -> Result<(BTreeMap<String, String>, usize)> {
    let mut mapping = BTreeMap::new();
    let mut rewritten = None;
    for text in lines {
        let trimmed = text.trim();
        if let Some(rest) = trimmed.strip_prefix("link macros rewritten: ") {
            rewritten = rest.trim().parse::<usize>().ok();
            continue;
        }
        if trimmed.starts_with("move verification:") || trimmed.starts_with("moved cards:") {
            continue;
        }
        if let Some((old, new)) = trimmed.split_once(" -> ") {
            let old = old.trim();
            let new = new.trim();
            if old.is_empty() || new.is_empty() {
                return Err(refuse(format!("a move plan line names no address: `{text}`")));
            }
            if mapping.insert(old.to_string(), new.to_string()).is_some() {
                return Err(refuse(format!("a move plan names `{old}` twice")));
            }
        }
    }
    let rewritten = rewritten.context("a move plan did not state its Link rewrites")?;
    Ok((mapping, rewritten))
}

/// The finished-model effect of one move: a Model part keeps its model when its home Model Card is
/// the same Card after the move. A change is reported, never repaired (D267's boundary).
fn model_effect(titles: &BTreeMap<String, Value>, mapping: &BTreeMap<String, String>) -> Value {
    let observed = Observed { titles: titles.clone() };
    let moved: BTreeSet<String> = mapping.keys().cloned().collect();
    let mut models: BTreeSet<String> = BTreeSet::new();
    let mut changes: Vec<Value> = Vec::new();
    let inverse: HashMap<&String, &String> = mapping.iter().map(|(before, after)| (after, before)).collect();
    for (old, new) in mapping {
        let head_here = observed.titles.get(old).is_some_and(|row| f(row, "type") == "model" && f(row, "facet").is_empty());
        if head_here {
            models.insert(old.clone());
            let left: Vec<String> = observed
                .parts_of(old)
                .into_iter()
                .filter(|part| part != old && !moved.contains(part))
                .collect();
            if !left.is_empty() {
                changes.push(json!({
                    "card": old,
                    "kind": "parts_left_behind",
                    "detail": format!("Model Card `{old}` moves but {} of its parts stay under its old range", left.len()),
                    "cards": left,
                }));
            }
            continue;
        }
        if observed.type_of(old) != "model" {
            continue;
        }
        let home_before = observed.home_model(old);
        let home_after = ancestors(new).into_iter().find_map(|ancestor| {
            let identity = inverse.get(&ancestor).copied().cloned().unwrap_or(ancestor);
            observed.head(identity)
        });
        if let Some(home) = &home_before {
            models.insert(home.clone());
        }
        if home_before != home_after {
            changes.push(json!({
                "card": old,
                "kind": "membership",
                "home_before": home_before,
                "home_after": home_after.unwrap_or_default(),
                "detail": format!("`{old}` would serve a different model after the move"),
            }));
        }
    }
    json!({
        "preserved": changes.is_empty(),
        "models": models.into_iter().collect::<Vec<String>>(),
        "changes": changes,
    })
}

/// A title catalogue with the two questions this step asks of it (membership and Model Cards).
struct Observed {
    titles: BTreeMap<String, Value>,
}

impl Observed {
    fn type_of(&self, at: &str) -> String {
        self.titles.get(at).map(|row| f(row, "type")).unwrap_or_default()
    }

    fn head(&self, at: String) -> Option<String> {
        (self.type_of(&at) == "model" && self.titles.get(&at).is_some_and(|row| f(row, "facet").is_empty())).then_some(at)
    }

    fn home_model(&self, at: &str) -> Option<String> {
        if self.head(at.to_string()).is_some() {
            return Some(at.to_string());
        }
        ancestors(at).into_iter().find_map(|here| self.head(here))
    }

    fn parts_of(&self, head: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::from([head.to_string()]);
        out.extend(descendants(&self.titles, head));
        out
    }
}

/// One enumerated entry chain as an offered operation, with the `move_card` plan it gets today.
struct Operation {
    id: String,
    from: String,
    alone: bool,
    to: String,
    preview: Preview,
    model_effect: Value,
}

fn operation_json(op: &Operation, endpoints: &Endpoints) -> Value {
    json!({
        "id": op.id,
        "from": op.from,
        "with_successors": !op.alone,
        "to": op.to,
        "moved": op.preview.mapping.keys().cloned().collect::<Vec<String>>(),
        "mapping": op.preview.mapping.iter().map(|(before, after)| json!([before, after])).collect::<Vec<Value>>(),
        "links_rewritten": op.preview.links_rewritten,
        "model_effect": op.model_effect,
        "plan_lines": op.preview.lines,
        "fingerprints": op.preview.mapping.keys().filter_map(|at| endpoints.fingerprint(at).map(|value| json!([at, value]))).collect::<Vec<Value>>(),
    })
}

/// Read every Card a plan's mapping names, retain its authored fingerprint, and refuse a plan that
/// would move or renumber more Cards than the bound allows.
fn retain_plan_cards(zt: &mut Zt, revision: u64, work: &str, endpoints: &mut Endpoints, preview: &Preview) -> Result<std::result::Result<(), String>> {
    let wanted: Vec<String> = preview.mapping.keys().cloned().collect();
    if wanted.len() > MAX_PLAN_CARDS {
        return Ok(Err(format!("the plan would move or renumber {} Cards, above the {MAX_PLAN_CARDS} bound", wanted.len())));
    }
    let cards = concepts::read_cards(zt, &wanted, revision, &mut endpoints.unread, "topic join")?;
    for card in &cards {
        endpoints.keep(work, card)?;
    }
    let missing: Vec<String> = wanted.iter().filter(|at| !endpoints.files.contains_key(*at)).cloned().collect();
    if !missing.is_empty() {
        return Ok(Err(format!("the plan names Cards the archive did not return: {}", missing.join(", "))));
    }
    Ok(Ok(()))
}

/// The plans one pair offers. Every plan is one operation (one `move_card` call), so its destination
/// is the next free Side successor of the joining Card and its mapping is the service's own plan for
/// today's archive. The plans enumerate the movable entry chains (D159): the whole chain from each
/// entry, and the first entry alone.
struct PlanBuilder<'a> {
    snapshot: &'a Snapshot,
    endpoints: &'a mut Endpoints,
    pair: &'a str,
    anchor: String,
    plans: Vec<Value>,
}

impl<'a> PlanBuilder<'a> {
    fn new(snapshot: &'a Snapshot, endpoints: &'a mut Endpoints, pair: &'a str, anchor: &str) -> Self {
        Self { snapshot, endpoints, pair, anchor: anchor.to_string(), plans: Vec::new() }
    }

    fn preview(&mut self, zt: &mut Zt, work: &str, from: &str, alone: bool) -> Result<std::result::Result<Operation, (String, String)>> {
        let to = next_side(&self.snapshot.titles, &self.anchor);
        let id = format!("{}/p{:03}/o001", self.pair, self.plans.len() + 1);
        match preview_move(zt, from, &to, alone)? {
            Ok(preview) => {
                let effect = model_effect(&self.snapshot.titles, &preview.mapping);
                match retain_plan_cards(zt, self.snapshot.revision, work, self.endpoints, &preview)? {
                    Ok(()) => Ok(Ok(Operation { id, from: from.to_string(), alone, to, preview, model_effect: effect })),
                    Err(reason) => Ok(Err(("too_large".to_string(), reason))),
                }
            }
            Err(message) => Ok(Err((classify_refusal(&message).to_string(), message))),
        }
    }

    fn push(&mut self, kind: &str, description: String, operations: Vec<Value>, refused: Option<(String, String)>) {
        let id = format!("{}/p{:03}", self.pair, self.plans.len() + 1);
        let model_refused = operations
            .iter()
            .find(|operation| operation["model_effect"]["preserved"].as_bool() != Some(true))
            .map(|operation| {
                (
                    "model_boundary".to_string(),
                    format!("the move `{}` would change a finished model's membership", f(operation, "from")),
                )
            });
        let (status, reason) = match refused.or(model_refused) {
            Some((code, detail)) => ("refused", json!({ "code": code, "detail": detail })),
            None => ("offered", Value::Null),
        };
        self.plans.push(json!({
            "id": id,
            "kind": kind,
            "description": description,
            "status": status,
            "reason": reason,
            "operations": operations,
        }));
    }
}

/// The step's classification of a service refusal: a recorded outcome, never a process error.
fn classify_refusal(message: &str) -> &'static str {
    let text = message.to_lowercase();
    if text.contains("read-only for ai agents") {
        "human_link"
    } else if text.contains("workspace") || text.contains("literature tree") {
        "zone_boundary"
    } else if text.contains("side successors") {
        "side_successors"
    } else if text.contains("next available label") || text.contains("destination locations already exist") {
        "destination_taken"
    } else if text.contains("does not exist") {
        "missing_card"
    } else {
        "plan_refused"
    }
}

#[allow(clippy::too_many_arguments)]
fn move_plans(
    zt: &mut Zt,
    snapshot: &Snapshot,
    work: &str,
    endpoints: &mut Endpoints,
    pair: &str,
    other_root: &str,
    joining: &Value,
    bound: usize,
) -> Result<Value> {
    let chain = snapshot.chain(other_root);
    if chain.is_empty() {
        return Ok(json!({ "plans": [], "reason": "the non-keeper Topic has no entry chain to move" }));
    }
    let anchor = f(joining, "address");
    if anchor.is_empty() || !snapshot.has(&anchor) {
        return Ok(json!({ "plans": [], "reason": "no existing Card can anchor the join" }));
    }
    if tree_of(other_root) == tree_of(&anchor) {
        return Ok(json!({ "plans": [], "reason": "the two Topics are the same tree" }));
    }
    let mut builder = PlanBuilder::new(snapshot, endpoints, pair, &anchor);
    // Each movable entry chain: the whole chain from entry k to the Topic's last entry, with its
    // Side subtrees. A chain is one `move_card` operation, so each plan is reviewed and applied
    // against the archive of its own moment.
    for (index, from) in chain.iter().take(bound).enumerate() {
        let mut operations = Vec::new();
        let mut refused = None;
        match builder.preview(zt, work, from, false)? {
            Ok(op) => {
                let row = operation_json(&op, builder.endpoints);
                operations.push(row);
            }
            Err(reason) => refused = Some(reason),
        }
        let kind = if index == 0 { "whole_chain" } else { "entry_chain" };
        builder.push(
            kind,
            format!("move the entry chain from `{from}` to `{other_root}`'s last entry beside `{anchor}`"),
            operations,
            refused,
        );
    }
    // The first entry alone, when it has no Side successors: its Direct-successor chain closes up,
    // so every later reviewed address moves with it (D271's refresh).
    let mut operations = Vec::new();
    let mut refused = None;
    let first = chain[0].clone();
    if snapshot.sides(&first).is_empty() {
        match builder.preview(zt, work, &first, true)? {
            Ok(op) => {
                let row = operation_json(&op, builder.endpoints);
                operations.push(row);
            }
            Err(reason) => refused = Some(reason),
        }
    } else {
        refused = Some((
            "side_successors".to_string(),
            format!("`{first}` has Side successors, so it can only move with its whole chain (the whole-chain plan)"),
        ));
    }
    builder.push(
        "lone_entry",
        format!("move `{first}` alone beside `{anchor}`, its Direct-successor chain closing up"),
        operations,
        refused,
    );
    Ok(json!({ "anchor": anchor, "plans": builder.plans }))
}

/// One Topic's read meanings.
struct TopicEvidence {
    root: String,
    title: String,
    question: String,
    entries: Vec<Value>,
    books: Vec<String>,
    sourced: usize,
}

fn topic_evidence(snapshot: &Snapshot, endpoints: &Endpoints, root: &str, read_entries: &[String]) -> TopicEvidence {
    let mut entries = Vec::new();
    for at in read_entries {
        entries.push(json!({
            "address": at,
            "title": snapshot.title(at),
            "statement": endpoints.statement(at),
            "sources": sources(&endpoints.body(at)),
        }));
    }
    let (books, sourced) = source_evidence(snapshot, endpoints, read_entries);
    TopicEvidence {
        root: root.to_string(),
        title: snapshot.title(root),
        question: line(&endpoints.body(root), 200),
        entries,
        books,
        sourced,
    }
}

fn plan(p: &Params) -> Result<i32> {
    std::fs::create_dir_all(ps::join(&p.work, "endpoints"))?;
    let mut zt = crate::zt::start()?;
    let mut attempt = 0;
    let (snapshot, endpoints, pairs, matched) = loop {
        attempt += 1;
        match observe(&mut zt, p) {
            Ok(frozen) => break frozen,
            Err(error) if concepts::is_changing_view(&error) && attempt == 1 => continue,
            Err(error) => return Err(error),
        }
    };
    let mut payload = json!({
        "schema_version": SCHEMA_VERSION,
        "kind": "topic-join-review",
        "created_at": canon::utc_now_o(),
        "archive": { "id": snapshot.id, "path": snapshot.path, "revision": snapshot.revision },
        "bounds": {
            "pairs": p.max_pairs,
            "operations": p.max_operations,
            "similarity": similarity_text(p.similar),
            "question_floor": similarity_text(QUESTION_FLOOR),
            "entries_per_topic": ENTRIES_PER_TOPIC,
            "plan_cards": MAX_PLAN_CARDS,
            "endpoints": MAX_ENDPOINTS,
        },
        "coverage": {
            "agent_topics": snapshot.topics.len(),
            "agent_books": snapshot.books.len(),
            "pairs_matched": matched,
            "pairs_selected": pairs.len(),
            "pairs_unselected": matched.saturating_sub(pairs.len()),
            "endpoints": endpoints.files.len(),
            "unread": endpoints.unread.len(),
        },
        "endpoints": endpoints.files,
        "unread": endpoints.unread.iter().map(|(address, why)| json!({ "address": address, "why": why })).collect::<Vec<Value>>(),
        "pairs": pairs,
        "status": if endpoints.unread.is_empty() { "complete" } else { "partial" },
    });
    let id = format!("tj-{}", canon::content_hash(&payload, &["report_id", "created_at"]));
    payload["report_id"] = json!(id);
    let json_path = ps::join(&p.work, "topic-join-review.json");
    let md_path = ps::join(&p.work, "topic-join-review.md");
    let mut written = false;
    let existing = if ps::exists(&json_path) { canon::read_json(&json_path).ok() } else { None };
    let intact = existing.filter(|old| {
        old["report_id"].as_str() == payload["report_id"].as_str()
            && format!("tj-{}", canon::content_hash(old, &["report_id", "created_at"])) == payload["report_id"]
    });
    if let Some(published) = intact {
        let markdown = render_markdown(&published);
        if !ps::exists(&md_path) || util::read_text(&md_path)? != markdown {
            util::write_atomic(&md_path, &markdown)?;
            written = true;
        }
    } else {
        canon::save_json(&json_path, &payload)?;
        util::write_atomic(&md_path, &render_markdown(&payload))?;
        written = true;
    }
    let moves_offered = payload["pairs"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter(|pair| {
                    pair["alternatives"].as_array().is_some_and(|list| {
                        list.iter().any(|item| {
                            item["action"] == "move_subtree"
                                && item["plans"].as_array().is_some_and(|plans| plans.iter().any(|plan| plan["status"] == "offered"))
                        })
                    })
                })
                .count()
        })
        .unwrap_or(0);
    let summary = json!({
        "ok": true,
        "step": "plan",
        "report_id": payload["report_id"],
        "status": payload["status"],
        "archive_id": payload["archive"]["id"],
        "archive_revision": payload["archive"]["revision"],
        "agent_topics": payload["coverage"]["agent_topics"],
        "pairs_matched": payload["coverage"]["pairs_matched"],
        "pairs_selected": payload["coverage"]["pairs_selected"],
        "pairs_unselected": payload["coverage"]["pairs_unselected"],
        "moves_offered": moves_offered,
        "unread": payload["coverage"]["unread"],
        "attempts": attempt,
        "written": written,
        "paths": [json_path, md_path],
    });
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

fn observe(zt: &mut Zt, p: &Params) -> Result<(Snapshot, Endpoints, Vec<Value>, usize)> {
    let (id, path, revision) = archive_identity(zt)?;
    let mut snapshot = read_snapshot(zt, revision)?;
    snapshot.id = id;
    snapshot.path = path;
    let mut endpoints = Endpoints::default();
    let roots: Vec<String> = snapshot.topics.iter().map(|topic| f(topic, "address")).collect();
    for card in concepts::read_cards(zt, &roots, revision, &mut endpoints.unread, "topic join")? {
        endpoints.keep(&p.work, &card)?;
    }
    let questions: HashMap<String, String> = roots.iter().map(|root| (root.clone(), line(&endpoints.body(root), 200))).collect();
    let (candidates, matched) = pair_candidates(&snapshot.topics, &questions, p.similar, p.max_pairs);
    // The entries the meanings read, for every selected pair, in one bounded batch.
    let mut entry_reads: Vec<String> = Vec::new();
    for candidate in &candidates {
        for root in [&candidate.a, &candidate.b] {
            for at in snapshot.chain(root).into_iter().take(ENTRIES_PER_TOPIC) {
                if !entry_reads.contains(&at) {
                    entry_reads.push(at);
                }
            }
        }
    }
    for card in concepts::read_cards(zt, &entry_reads, revision, &mut endpoints.unread, "topic join")? {
        endpoints.keep(&p.work, &card)?;
    }
    let mut pairs: Vec<Value> = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let pair_id = format!("t{:03}", index + 1);
        let mut topics: Vec<(TopicEvidence, Vec<String>)> = Vec::new();
        for root in [&candidate.a, &candidate.b] {
            let entries: Vec<String> = snapshot.chain(root).into_iter().take(ENTRIES_PER_TOPIC).collect();
            topics.push((topic_evidence(&snapshot, &endpoints, root, &entries), entries));
        }
        let (a, _) = &topics[0];
        let (b, _) = &topics[1];
        let (keeper_root, keeper_reason, keeper_note) = keeper_choice(&a.root, &a.books, a.sourced, &b.root, &b.books, b.sourced);
        let (keeper, other) = if keeper_root == a.root { (a, b) } else { (b, a) };
        let question_similarity =
            similarity(&bigrams(&format!("{} {}", a.title, a.question)), &bigrams(&format!("{} {}", b.title, b.question)));
        let other_entries: Vec<String> = other.entries.iter().map(|row| f(row, "address")).collect();
        let joining = joining_choice(&snapshot, &endpoints, &keeper_root, &other.root, &other_entries);
        let crossing = crossing_alternative(&snapshot, &keeper_root, &keeper.books, &other.books, &joining);
        let mut alternatives = vec![
            json!({
                "action": "keep_distinct",
                "description": format!("Keep `{}` and `{}` apart and make each Topic's question state the sense it owns.", a.root, b.root),
                "reason": if question_similarity < QUESTION_FLOOR { "the two questions differ in words, so the Topics may hold different senses" } else { "the meanings are close but not proven the same" },
            }),
            crossing,
        ];
        let moving = if keeper_reason == "unresolved" {
            json!({ "plans": [], "reason": "no keeper is proposed on evidence, so no move is offered" })
        } else {
            move_plans(zt, &snapshot, &p.work, &mut endpoints, &pair_id, &other.root, &joining, p.max_operations)?
        };
        let move_offered = moving["plans"].as_array().is_some_and(|plans| plans.iter().any(|plan| plan["status"] == "offered"));
        alternatives.push(json!({
            "action": "move_subtree",
            "keeper": keeper_root,
            "joining": joining["address"],
            "other": other.root,
            "plans": moving["plans"],
            "reason": moving["reason"],
        }));
        let recommendation = if question_similarity < QUESTION_FLOOR {
            "keep_distinct"
        } else if keeper_reason == "unresolved" {
            "keep_both_with_crossing"
        } else if move_offered {
            "move_subtree"
        } else {
            "keep_both_with_crossing"
        };
        let mut reasons: Vec<String> = Vec::new();
        for evidence in [a, b] {
            for row in &evidence.entries {
                let address = f(row, "address");
                if endpoints.is_unread(&address) {
                    reasons.push(format!("`{address}` could not be read"));
                }
            }
            if endpoints.is_unread(&evidence.root) {
                reasons.push(format!("`{}` could not be read", evidence.root));
            }
        }
        pairs.push(json!({
            "id": pair_id,
            "a": topic_json(a),
            "b": topic_json(b),
            "similarity": similarity_text(candidate.similarity),
            "inside": candidate.inside,
            "question_similarity": similarity_text(question_similarity),
            "keeper": keeper_root,
            "keeper_reason": keeper_reason,
            "keeper_note": keeper_note,
            "joining": joining,
            "source_mix": {
                "a": a.books,
                "b": b.books,
                "shared": a.books.iter().filter(|book| b.books.contains(book)).cloned().collect::<Vec<String>>(),
                "only_a": a.books.iter().filter(|book| !b.books.contains(book)).cloned().collect::<Vec<String>>(),
                "only_b": b.books.iter().filter(|book| !a.books.contains(book)).cloned().collect::<Vec<String>>(),
            },
            "alternatives": alternatives,
            "recommendation": recommendation,
            "status": if reasons.is_empty() { "complete" } else { "partial" },
            "reasons": reasons,
        }));
    }
    let closed = zt.archive_view()?;
    if closed["archive_revision"].as_u64() != Some(revision) || closed["archive_identity"]["id"].as_str() != Some(snapshot.id.as_str()) {
        return Err(concepts::changing_view());
    }
    Ok((snapshot, endpoints, pairs, matched))
}

fn topic_json(evidence: &TopicEvidence) -> Value {
    json!({
        "root": evidence.root,
        "tree": tree_of(&evidence.root),
        "title": evidence.title,
        "question": evidence.question,
        "entries": evidence.entries,
        "books": evidence.books,
        "sourced_entries": evidence.sourced,
    })
}

// --- rendering ------------------------------------------------------------------------------------

fn render_markdown(review: &Value) -> String {
    let bounds = &review["bounds"];
    let mut sb = Sb::new();
    sb.line(format!("# Existing Topic pairs: archive `{}`", f(&review["archive"], "id"))).blank();
    sb.line(format!("Report: `{}`", f(review, "report_id")));
    sb.line(format!("Generated: {}", f(review, "created_at")));
    sb.line(format!(
        "Archive revision {}. Status: **{}**. {} agent Topic(s), {} agent book(s); {} candidate pair(s), {} reviewed, {} unselected.",
        f(&review["archive"], "revision"),
        f(review, "status"),
        f(&review["coverage"], "agent_topics"),
        f(&review["coverage"], "agent_books"),
        f(&review["coverage"], "pairs_matched"),
        f(&review["coverage"], "pairs_selected"),
        f(&review["coverage"], "pairs_unselected"),
    ))
    .blank();
    sb.line(format!(
        "Bounds: at most {} pair(s), {} entry chain(s) per pair; title-question similarity ≥ {}; a plan may move or renumber at most {} Cards. A shortlist is a bound, never an exhaustive semantic comparison.",
        f(bounds, "pairs"),
        f(bounds, "operations"),
        f(bounds, "similarity"),
        f(bounds, "plan_cards")
    ))
    .blank();
    for pair in ps::ps_items(&review["pairs"]) {
        sb.line(format!(
            "## `{}` {} (`{}`) and {} (`{}`)",
            f(&pair, "id"),
            cell(&f(&pair["a"], "title")),
            f(&pair["a"], "root"),
            cell(&f(&pair["b"], "title")),
            f(&pair["b"], "root")
        ))
        .blank();
        sb.line(format!(
            "- similarity {}; question similarity {}; one title inside the other: {}.",
            f(&pair, "similarity"),
            f(&pair, "question_similarity"),
            pair["inside"].as_bool().unwrap_or(false)
        ));
        sb.line(format!("- keeper `{}` ({}): {}", f(&pair, "keeper"), f(&pair, "keeper_reason"), cell(&f(&pair, "keeper_note"))));
        let joining = &pair["joining"];
        sb.line(format!(
            "- best joining Card `{}` {}: {}",
            f(joining, "address"),
            cell(&f(joining, "title")),
            cell(&f(joining, "why"))
        ));
        let mix = &pair["source_mix"];
        sb.line(format!(
            "- source mix: `{}` {}; `{}` {}; shared {}; only `{}` {}; only `{}` {}.",
            f(&pair["a"], "root"),
            list(&mix["a"]),
            f(&pair["b"], "root"),
            list(&mix["b"]),
            list(&mix["shared"]),
            f(&pair["a"], "root"),
            list(&mix["only_a"]),
            f(&pair["b"], "root"),
            list(&mix["only_b"])
        ));
        sb.blank().line("Questions:").blank();
        sb.line(format!("- `{}`: {}", f(&pair["a"], "root"), cell(&f(&pair["a"], "question"))));
        sb.line(format!("- `{}`: {}", f(&pair["b"], "root"), cell(&f(&pair["b"], "question"))));
        sb.blank().line("Alternatives:").blank();
        for alternative in ps::ps_items(&pair["alternatives"]) {
            let action = f(&alternative, "action");
            if action != "move_subtree" {
                sb.line(format!("- **{action}** — {}", cell(&f(&alternative, "description"))));
                continue;
            }
            sb.line(format!(
                "- **move_subtree** — keeper `{}`, joining `{}`.",
                f(&alternative, "keeper"),
                f(&alternative, "joining")
            ));
            if !f(&alternative, "reason").is_empty() {
                sb.line(format!("  {}", cell(&f(&alternative, "reason"))));
            }
            for plan in ps::ps_items(&alternative["plans"]) {
                sb.line(format!("  - `{}` {} — **{}**", f(&plan, "id"), f(&plan, "kind"), f(&plan, "status")));
                sb.line(format!("    {}", cell(&f(&plan, "description"))));
                if plan["status"] == "refused" {
                    sb.line(format!("    refused: {} — {}", f(&plan["reason"], "code"), cell(&f(&plan["reason"], "detail"))));
                    sb.line("    the nonmoving alternative is the crossing above; no workaround is offered (D305).");
                }
                for operation in ps::ps_items(&plan["operations"]) {
                    sb.line(format!(
                        "    - `{}` {} `{}` → `{}`: {} Link rewrite(s); model preserved: {}.",
                        f(&operation, "id"),
                        if operation["with_successors"].as_bool() == Some(true) { "with its successors" } else { "alone" },
                        f(&operation, "from"),
                        f(&operation, "to"),
                        f(&operation, "links_rewritten"),
                        operation["model_effect"]["preserved"].as_bool().unwrap_or(false)
                    ));
                    for row in ps::ps_items(&operation["mapping"]) {
                        if let Some(list) = row.as_array() {
                            sb.line(format!(
                                "      - `{}` → `{}`",
                                list.first().map(ps::s).unwrap_or_default(),
                                list.get(1).map(ps::s).unwrap_or_default()
                            ));
                        }
                    }
                    for change in ps::ps_items(&operation["model_effect"]["changes"]) {
                        sb.line(format!("      - model change: {}", cell(&f(&change, "detail"))));
                    }
                }
            }
        }
        sb.blank().line(format!("Recommendation: **{}**.", f(&pair, "recommendation")));
        if !ps::ps_items(&pair["reasons"]).is_empty() {
            sb.blank().line("Incomplete reads:").blank();
            for reason in ps::ps_items(&pair["reasons"]) {
                sb.line(format!("- {}", ps::s(&reason)));
            }
        }
        sb.blank();
    }
    sb.0
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

fn list(value: &Value) -> String {
    let items: Vec<String> = ps::ps_items(value).iter().map(|item| format!("`{}`", ps::s(item))).collect();
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

// --- the apply step -------------------------------------------------------------------------------

/// One accepted decision: the finding it names and, for a move, the reviewed plan and operations.
#[derive(Clone, Debug)]
struct Decision {
    finding: String,
    action: String,
    note: String,
    plan: Option<String>,
    operations: Vec<String>,
}

fn read_review(work: &str) -> Result<Value> {
    let path = ps::join(work, "topic-join-review.json");
    if !ps::exists(&path) {
        return Err(refuse(format!("join apply needs the reviewed report `{path}`, written by -Step plan")));
    }
    let value = canon::read_json(&path)?;
    let id = f(&value, "report_id");
    if !id.starts_with("tj-") {
        return Err(refuse(format!("`{path}` is not a Topic join review: no report id")));
    }
    let recomputed = format!("tj-{}", canon::content_hash(&value, &["report_id", "created_at"]));
    if recomputed != id {
        return Err(refuse(format!("the report at `{path}` does not match its own report id; regenerate it with -Step plan")));
    }
    Ok(value)
}

fn finding_of<'a>(review: &'a Value, id: &str) -> Option<&'a Value> {
    review["pairs"].as_array()?.iter().find(|pair| f(pair, "id") == id)
}

fn plan_of<'a>(finding: &'a Value, id: &str) -> Option<&'a Value> {
    let alternative = finding["alternatives"].as_array()?.iter().find(|item| f(item, "action") == "move_subtree")?;
    alternative["plans"].as_array()?.iter().find(|plan| f(plan, "id") == id)
}

/// §11.3: a reviewed selection binds the report id, the finding and plan ids, the reviewed
/// authored fingerprints and a note. Anything the report does not support is refused.
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
        return Err(refuse(format!("a selection names one to {MAX_DECISIONS} findings")));
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut accepted: Vec<Decision> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let number = index + 1;
        let object = row.as_object().ok_or_else(|| refuse(format!("decision {number} must be an object")))?;
        for name in object.keys() {
            if !["finding", "action", "plan", "note", "fingerprints"].contains(&name.as_str()) {
                return Err(refuse(format!("decision {number} has an unknown field `{name}`")));
            }
        }
        let finding_id = f(row, "finding");
        let Some(finding) = finding_of(review, &finding_id) else {
            return Err(refuse(format!("decision {number} names the finding `{finding_id}`, which the report does not hold")));
        };
        if !seen.insert(finding_id.clone()) {
            return Err(refuse(format!("decision {number} reviews the finding `{finding_id}` twice")));
        }
        let action = f(row, "action");
        if !ACTIONS.contains(&action.as_str()) {
            return Err(refuse(format!("decision {number}: `{action}` is not one of {}", ACTIONS.join(", "))));
        }
        if !finding["alternatives"].as_array().is_some_and(|list| list.iter().any(|item| f(item, "action") == action)) {
            return Err(refuse(format!("decision {number}: the finding does not offer `{action}`")));
        }
        let note = f(row, "note").trim().to_string();
        if note.is_empty() {
            return Err(refuse(format!("decision {number} needs a note that states why this resolution was chosen")));
        }
        let mut plan = None;
        let mut operations = Vec::new();
        if action == "move_subtree" {
            let plan_id = f(row, "plan");
            let Some(reviewed) = plan_of(finding, &plan_id) else {
                return Err(refuse(format!("decision {number} names the plan `{plan_id}`, which the finding does not hold")));
            };
            if reviewed["status"] != "offered" {
                return Err(refuse(format!(
                    "decision {number}: plan `{plan_id}` is `{}` — {}; select the nonmoving alternative",
                    f(reviewed, "status"),
                    f(&reviewed["reason"], "detail")
                )));
            }
            for operation in ps::ps_items(&reviewed["operations"]) {
                let at = f(&operation, "from");
                if topic_root(&at) {
                    return Err(refuse(format!("decision {number}: a Topic root cannot be moved as an ordinary subtree (D159)")));
                }
                operations.push(f(&operation, "id"));
            }
            if operations.is_empty() {
                return Err(refuse(format!("decision {number}: plan `{plan_id}` holds no operation")));
            }
            plan = Some(plan_id);
        } else if row.get("plan").is_some() {
            return Err(refuse(format!("decision {number}: `{action}` takes no plan")));
        }
        if let Some(map) = row["fingerprints"].as_object() {
            for (at, value) in map {
                let Some(known) = review["endpoints"].as_object().and_then(|endpoints| endpoints.get(at)) else {
                    return Err(refuse(format!("decision {number} names the fingerprint of `{at}`, which the review did not read")));
                };
                if ps::s(value) != f(known, "fingerprint") {
                    return Err(refuse(format!(
                        "decision {number} names fingerprint `{}` for `{at}`, but the reviewed endpoint is `{}`",
                        ps::s(value),
                        f(known, "fingerprint")
                    )));
                }
            }
        }
        accepted.push(Decision { finding: finding_id, action, note, plan, operations });
    }
    Ok(accepted)
}

fn apply(p: &Params) -> Result<i32> {
    let review = read_review(&p.work)?;
    let selection_path = if p.file.is_empty() { ps::join(&p.work, "join-selection.json") } else { p.file.clone() };
    if !ps::exists(&selection_path) {
        return Err(refuse(format!("join apply needs the reviewed selection `{selection_path}`")));
    }
    let asked = canon::read_json(&selection_path)?;
    let decisions = validate_selection(&review, &asked)?;
    let mut zt = crate::zt::start()?;
    let (id, _, revision) = archive_identity(&mut zt)?;
    if id != f(&review["archive"], "id") {
        return Err(refuse(format!(
            "the report belongs to archive `{}`, but this service serves `{id}`; regenerate the review on this archive",
            f(&review["archive"], "id")
        )));
    }
    let result_path = ps::join(&p.work, "join-result.json");
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
    let mut run = Run { zt, review, decisions, revision, archive_id: id, broken_before, quiet: None };
    let mut payload = run.execute()?;
    payload["selection_id"] = json!(hash);
    payload["selection"] = json!(selection_path);
    payload["created_at"] = json!(canon::utc_now_o());
    payload["result_id"] = json!(format!("jr-{}", canon::content_hash(&payload, &["result_id", "created_at"])));
    canon::save_json(&result_path, &payload)?;
    let md_path = ps::join(&p.work, "join-result.md");
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
    revision: u64,
    archive_id: String,
    broken_before: usize,
    /// D270: another writer changed an affected location, so no further automated move is sent.
    quiet: Option<String>,
}

impl Run {
    fn execute(&mut self) -> Result<Value> {
        let mut items: Vec<Value> = Vec::new();
        let mut all_applied: BTreeMap<String, String> = BTreeMap::new();
        let mut pending_refreshed: Vec<Value> = Vec::new();
        let decisions = self.decisions.clone();
        for decision in &decisions {
            let finding = finding_of(&self.review, &decision.finding).cloned().context("finding vanished")?;
            let item = if let Some(quiet) = self.quiet.clone() {
                // D270: the archive is not quiet, so no further automated move is sent; every
                // remaining item is recorded as pending with its addresses refreshed by what has
                // been applied (D271).
                self.pending_finding(&finding, decision, &all_applied, &mut pending_refreshed, &quiet)
            } else {
                match decision.action.as_str() {
                    "move_subtree" => self.apply_finding(&finding, decision, &mut all_applied, &mut pending_refreshed)?,
                    _ => json!({
                        "finding": decision.finding,
                        "action": decision.action,
                        "note": decision.note,
                        "status": "unchanged",
                        "detail": "the reviewed nonmoving resolution writes no Card; the report's alternative is retained",
                        "operations": [],
                    }),
                }
            };
            items.push(item);
        }
        let (after_id, after_path, after_revision) = archive_identity(&mut self.zt)?;
        let broken_after = broken_links(&mut self.zt)?;
        let statuses: Vec<String> = items.iter().map(|item| f(item, "status")).collect();
        let count = |status: &str| statuses.iter().filter(|known| known.as_str() == status).count();
        let counts = json!({
            "findings": items.len(),
            "applied_and_verified": count("applied-and-verified"),
            "unchanged": count("unchanged"),
            "conflict": count("conflict"),
            "refused": count("refused"),
            "pending": count("pending"),
        });
        let status = if count("conflict") > 0 {
            "conflict"
        } else if count("pending") > 0 {
            "pending"
        } else if count("refused") > 0 {
            "refused"
        } else if count("applied-and-verified") > 0 {
            "applied"
        } else {
            "unchanged"
        };
        let complete = statuses.iter().all(|known| known != "conflict" && known != "refused" && known != "pending");
        Ok(json!({
            "schema_version": SCHEMA_VERSION,
            "kind": "topic-join-result",
            "review_report_id": self.review["report_id"],
            "review_created_at": self.review["created_at"],
            "archive": {
                "id": after_id,
                "path": after_path,
                "revision_before": self.revision,
                "revision_after": after_revision,
                "review_revision": self.review["archive"]["revision"],
            },
            "items": items,
            "applied": all_applied.iter().map(|(before, after)| json!([before, after])).collect::<Vec<Value>>(),
            "rollback": all_applied.iter().map(|(before, after)| json!([after, before])).collect::<Vec<Value>>(),
            "pending_operations": pending_refreshed,
            "broken_links": { "before": self.broken_before, "after": broken_after },
            "counts": counts,
            "status": status,
            "complete": complete,
            "scope": "one explicitly selected review; historical live execution is a separate gate",
        }))
    }

    fn apply_finding(
        &mut self,
        finding: &Value,
        decision: &Decision,
        all_applied: &mut BTreeMap<String, String>,
        pending_refreshed: &mut Vec<Value>,
    ) -> Result<Value> {
        let mut applied_here: BTreeMap<String, String> = BTreeMap::new();
        let mut operations: Vec<Value> = Vec::new();
        let plan = decision.plan.clone().unwrap_or_default();
        let mut stopped: Option<String> = None;
        for (index, op_id) in decision.operations.iter().enumerate() {
            let Some(reviewed) = plan_of(finding, &plan).and_then(|plan| plan["operations"].as_array()).and_then(|list| list.iter().find(|operation| f(operation, "id") == *op_id)).cloned() else {
                return Err(refuse(format!("the reviewed operation `{op_id}` is not in plan `{plan}`")));
            };
            if let Some(reason) = &stopped {
                // D271: the pending items keep refreshed addresses, so a later run (or a reviewer)
                // reads current ones rather than the reviewed ones.
                let refreshed: BTreeMap<String, String> = refresh_all(all_applied, &applied_here, &reviewed);
                pending_refreshed.push(json!({
                    "finding": decision.finding,
                    "plan": plan,
                    "operation": op_id,
                    "status": "pending",
                    "reason": reason,
                    "addresses": refreshed.iter().map(|(before, after)| json!([before, after])).collect::<Vec<Value>>(),
                }));
                continue;
            }
            let mut combined = all_applied.clone();
            combined.extend(applied_here.clone());
            let mut outcome = self.apply_operation(finding, &reviewed, &combined, &mut applied_here)?;
            outcome["finding"] = json!(decision.finding);
            outcome["operation"] = json!(op_id);
            outcome["index"] = json!(index + 1);
            // D271: `from`/`to` are the addresses this run used after refreshing the reviewed ones;
            // `reviewed` keeps what the report said, so a reviewer sees exactly what moved.
            outcome["reviewed"] = json!({ "from": f(&reviewed, "from"), "to": f(&reviewed, "to") });
            let status = f(&outcome, "status");
            if status == "pending" {
                // D271: the pending item keeps its addresses refreshed by everything applied so far.
                let refreshed = refresh_all(all_applied, &applied_here, &reviewed);
                pending_refreshed.push(json!({
                    "finding": decision.finding,
                    "plan": plan,
                    "operation": op_id,
                    "status": "pending",
                    "reason": f(&outcome["reason"], "detail"),
                    "addresses": refreshed.iter().map(|(before, after)| json!([before, after])).collect::<Vec<Value>>(),
                }));
            }
            if status == "conflict" || status == "refused" || status == "pending" {
                stopped = Some(f(&outcome["reason"], "detail"));
            }
            if status == "pending" && f(&outcome["reason"], "code") == "quiet_window_required" {
                self.quiet = Some(f(&outcome["reason"], "detail"));
            }
            operations.push(outcome);
        }
        for (before, after) in applied_here.clone() {
            all_applied.insert(before, after);
        }
        let statuses: Vec<String> = operations.iter().map(|item| f(item, "status")).collect();
        let status = worst(&statuses);
        Ok(json!({
            "finding": decision.finding,
            "action": decision.action,
            "plan": decision.plan,
            "note": decision.note,
            "status": status,
            "operation_statuses": statuses,
            "operations": operations,
        }))
    }

    /// A finding that is never attempted because an earlier move found the archive not quiet. Its
    /// reviewed addresses are kept refreshed by every applied mapping (D271).
    fn pending_finding(
        &self,
        finding: &Value,
        decision: &Decision,
        all_applied: &BTreeMap<String, String>,
        pending_refreshed: &mut Vec<Value>,
        quiet: &str,
    ) -> Value {
        let plan = decision.plan.clone().unwrap_or_default();
        let mut operations = Vec::new();
        for op_id in &decision.operations {
            let Some(reviewed) =
                plan_of(finding, &plan).and_then(|plan| plan["operations"].as_array()).and_then(|list| list.iter().find(|operation| f(operation, "id") == *op_id)).cloned()
            else {
                continue;
            };
            let refreshed = refresh_all(all_applied, &BTreeMap::new(), &reviewed);
            pending_refreshed.push(json!({
                "finding": decision.finding,
                "plan": plan,
                "operation": op_id,
                "status": "pending",
                "reason": quiet,
                "addresses": refreshed.iter().map(|(before, after)| json!([before, after])).collect::<Vec<Value>>(),
            }));
            operations.push(operation_result(&f(&reviewed, "id"), &refresh(all_applied, &f(&reviewed, "from")), &refresh(all_applied, &f(&reviewed, "to")), "pending", json!({"code": "quiet_window_required", "detail": quiet}), None));
        }
        json!({
            "finding": decision.finding,
            "action": decision.action,
            "plan": decision.plan,
            "note": decision.note,
            "status": "pending",
            "operation_statuses": ["pending"],
            "operations": operations,
        })
    }

    fn apply_operation(
        &mut self,
        finding: &Value,
        operation: &Value,
        applied: &BTreeMap<String, String>,
        applied_here: &mut BTreeMap<String, String>,
    ) -> Result<Value> {
        let from = refresh(applied, &f(operation, "from"));
        let to_reviewed = refresh(applied, &f(operation, "to"));
        let alone = operation["with_successors"].as_bool() != Some(true);
        let mut expected: BTreeMap<String, String> = BTreeMap::new();
        for pair in ps::ps_items(&operation["mapping"]) {
            let list = pair.as_array().cloned().unwrap_or_default();
            let before = list.first().map(ps::s).unwrap_or_default();
            let after = list.get(1).map(ps::s).unwrap_or_default();
            expected.insert(refresh(applied, &before), refresh(applied, &after));
        }
        let fingerprints: BTreeMap<String, String> = ps::ps_items(&operation["fingerprints"])
            .iter()
            .filter_map(|pair| {
                let list = pair.as_array()?;
                Some((list.first()?.as_str()?.to_string(), list.get(1)?.as_str()?.to_string()))
            })
            .collect();
        // 1. One view before anything is planned; the archive must still be this review's archive.
        let (id, _, revision) = archive_identity(&mut self.zt)?;
        if id != self.archive_id || self.review["archive"]["id"].as_str() != Some(id.as_str()) {
            return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, "conflict", json!({"code": "archive_changed", "detail": "the service now serves another archive than the reviewed one"}), None));
        }
        // 2. The moving Cards, as they are now; a changed authored fingerprint is a conflict.
        let moved_now: Vec<String> = expected.keys().cloned().collect();
        let mut unread: Vec<(String, String)> = Vec::new();
        let cards = concepts::read_cards(&mut self.zt, &moved_now, revision, &mut unread, "topic join")?;
        let found: BTreeMap<String, Value> = cards.iter().map(|card| (f(card, "address"), card.clone())).collect();
        // D273: Agent writability is checked before any plan is used.
        if let Some(card) = cards.iter().find(|card| f(card, "workspace") != "agent") {
            return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, "refused", json!({"code": "read_only_workspace", "detail": format!("`{}` is not an agent Card; agents cannot move it", f(card, "address"))}), None));
        }
        if let Some((at, why)) = unread.first() {
            if already_applied(&mut self.zt, &expected, &fingerprints, applied)? {
                applied_here.extend(expected.clone());
                return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, "unchanged", json!({"code": "already_applied", "detail": "the reviewed destination already holds the reviewed Cards"}), None));
            }
            return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, "conflict", json!({"code": "source_missing", "detail": format!("`{at}` could not be read: {why}")}), None));
        }
        for at in &moved_now {
            let Some(want) = fingerprint_at(&fingerprints, at, applied) else { continue };
            let now = calls::authored_fingerprint(&found[at]);
            if now != want {
                return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, "conflict", json!({"code": "endpoint_changed", "detail": format!("`{at}` no longer matches the reviewed authored fingerprint"), "reviewed": want, "current": now}), None));
            }
        }
        // 3. The destination: the next free Side successor of the joining Card.
        let joining = refresh(applied, &f(&finding["joining"], "address"));
        let titles = tree_titles(&mut self.zt, &[tree_of(&joining), tree_of(&from)])?;
        let destination = next_side(&titles, &joining);
        if destination != to_reviewed {
            return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, "conflict", json!({"code": "destination_taken", "detail": format!("`{to_reviewed}` is no longer the next free slot of `{joining}`; it is `{destination}`"), "next": destination}), None));
        }
        // 4. The fresh plan, compared with the reviewed one (D269).
        let fresh = match preview_move(&mut self.zt, &from, &destination, alone)? {
            Ok(preview) => preview,
            Err(message) => {
                let code = classify_refusal(&message);
                let status = if code == "human_link" || code == "zone_boundary" { "refused" } else { "conflict" };
                return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, status, json!({"code": code, "detail": message, "alternative": "keep_both_with_crossing"}), None));
            }
        };
        if fresh.mapping != expected || fresh.links_rewritten != operation["links_rewritten"].as_u64().unwrap_or(0) as usize {
            return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, "conflict", json!({"code": "plan_changed", "detail": "the fresh move plan differs from the reviewed plan; review the item again", "reviewed": mapping_json(&expected), "fresh": mapping_json(&fresh.mapping), "reviewed_links": operation["links_rewritten"], "fresh_links": fresh.links_rewritten}), None));
        }
        // 5. The finished-model boundary, checked on the plan that would be applied (D273).
        let effect = model_effect(&titles, &fresh.mapping);
        if !effect["preserved"].as_bool().unwrap_or(false) {
            return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, "refused", json!({"code": "model_boundary", "detail": "the move would change a finished model's membership", "changes": effect["changes"]}), Some(effect)));
        }
        // 6. The quiet window: another writer between the plan and the confirmation stops the move.
        let (_, _, before_confirm) = archive_identity(&mut self.zt)?;
        if before_confirm != revision {
            return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, "pending", json!({"code": "quiet_window_required", "detail": format!("the archive changed from revision {revision} to {before_confirm} between the plan and the confirmation; no automated move is sent")}), None));
        }
        // 7. Confirm, then read the returned mapping back.
        let done = match self.zt.call("move_card", json!({ "at": from, "to": destination, "alone": alone, "confirm": "move" })) {
            Ok(value) => value,
            Err(error) => {
                return Ok(operation_result(&f(operation, "id"), &from, &to_reviewed, "pending", json!({"code": "lost_response", "detail": format!("the confirmation did not answer: {error:#}; read the archive back before retrying")}), None));
            }
        };
        let returned: BTreeMap<String, String> =
            done["moved"].as_object().map(|map| map.iter().map(|(before, after)| (before.clone(), ps::s(after))).collect()).unwrap_or_default();
        if returned != expected {
            return Ok(operation_result(&f(operation, "id"), &from, &destination, "conflict", json!({"code": "mapping_mismatch", "detail": "the applied mapping differs from the reviewed plan", "applied": mapping_json(&returned)}), None));
        }
        let verified = self.verify_operation(operation, &from, &expected, &fingerprints, applied, &joining)?;
        let status = if verified["ok"].as_bool().unwrap_or(false) { "applied-and-verified" } else { "conflict" };
        applied_here.extend(expected.clone());
        Ok(operation_result(
            &f(operation, "id"),
            &from,
            &destination,
            status,
            json!({
                "code": if status == "applied-and-verified" { "verified" } else { "verification_failed" },
                "detail": verified["detail"],
            }),
            Some(verified),
        ))
    }

    /// D274: persisted addresses, authored fingerprints, Link rewrites, Broken links and the
    /// protected model reports are verified after the write; the mapping is consumed, not assumed.
    fn verify_operation(
        &mut self,
        operation: &Value,
        from: &str,
        expected: &BTreeMap<String, String>,
        fingerprints: &BTreeMap<String, String>,
        applied: &BTreeMap<String, String>,
        joining: &str,
    ) -> Result<Value> {
        let (_, _, revision) = archive_identity(&mut self.zt)?;
        let targets: Vec<String> = expected.values().cloned().collect();
        let mut unread = Vec::new();
        let cards = concepts::read_cards(&mut self.zt, &targets, revision, &mut unread, "topic join")?;
        if !unread.is_empty() {
            return Ok(json!({ "ok": false, "detail": format!("the persisted Cards could not be read: {}", unread.iter().map(|(at, _)| at.clone()).collect::<Vec<String>>().join(", ")) }));
        }
        let now: BTreeMap<String, Value> = cards.iter().map(|card| (f(card, "address"), card.clone())).collect();
        for (before, after) in expected {
            let Some(card) = now.get(after) else {
                return Ok(json!({ "ok": false, "detail": format!("`{after}` does not exist after the move") }));
            };
            if let Some(want) = fingerprint_at(fingerprints, before, applied) {
                let actual = calls::authored_fingerprint(card);
                if actual != want {
                    return Ok(json!({ "ok": false, "detail": format!("`{after}` holds different authored text than the reviewed `{before}`") }));
                }
            }
        }
        let broken = broken_links(&mut self.zt)?;
        if broken != self.broken_before {
            return Ok(json!({ "ok": false, "detail": format!("the move changed the Broken link count from {} to {broken}", self.broken_before) }));
        }
        // The moved Card really is filed under the joining Card (a joining Card inside the moving
        // subtree is allowed; then the whole chain moved with it).
        let landed = expected.get(from).cloned().unwrap_or_default();
        let joining_now = expected.get(joining).cloned().unwrap_or_else(|| joining.to_string());
        let under = landed == joining_now || ancestors(&landed).iter().any(|at| at == &joining_now);
        if !under {
            return Ok(json!({ "ok": false, "detail": format!("the moved Card `{landed}` is not filed under the joining Card `{joining_now}`") }));
        }
        // The protected model reports: each affected model still holds the same Cards.
        let mut models: Vec<Value> = Vec::new();
        for head in ps::ps_items(&operation["model_effect"]["models"]) {
            let at = refresh(applied, &ps::s(&head));
            let at = expected.get(&at).cloned().unwrap_or(at);
            match self.zt.call("get_model", json!({ "at": at })) {
                Ok(report) => {
                    let parts: Vec<String> = ps::each(&report["parts"]).iter().map(|part| f(part, "address")).collect();
                    models.push(json!({ "model": at, "parts": parts.len(), "ok": true }));
                }
                Err(error) => models.push(json!({ "model": at, "ok": false, "detail": format!("{error:#}") })),
            }
        }
        if models.iter().any(|row| row["ok"].as_bool() != Some(true)) {
            return Ok(json!({ "ok": false, "detail": "a protected model report could not be verified", "models": models }));
        }
        Ok(json!({
            "ok": true,
            "detail": "the persisted addresses, authored fingerprints, Link rewrites, Broken links and model reports match the reviewed plan",
            "cards": targets.len(),
            "models": models,
        }))
    }
}

fn fingerprint_at(fingerprints: &BTreeMap<String, String>, before: &str, applied: &BTreeMap<String, String>) -> Option<String> {
    // The reviewed fingerprint belongs to the reviewed address. A pending operation's address may
    // already have moved through an earlier applied operation, so find the reviewed address whose
    // applied image is the current one.
    if let Some(found) = fingerprints.iter().find(|(reviewed, _)| refresh(applied, reviewed) == before) {
        return Some(found.1.clone());
    }
    fingerprints.get(before).cloned()
}

/// Whether the reviewed destination already holds every reviewed Card: a lost response or a
/// resumed run is detected by readback instead of repeating the move.
fn already_applied(
    zt: &mut Zt,
    expected: &BTreeMap<String, String>,
    fingerprints: &BTreeMap<String, String>,
    applied: &BTreeMap<String, String>,
) -> Result<bool> {
    let wanted: Vec<String> = expected.values().cloned().collect();
    if wanted.is_empty() {
        return Ok(false);
    }
    let (_, _, revision) = archive_identity(zt)?;
    let mut unread = Vec::new();
    let cards = concepts::read_cards(zt, &wanted, revision, &mut unread, "topic join")?;
    if !unread.is_empty() || cards.len() != wanted.len() {
        return Ok(false);
    }
    for card in &cards {
        let at = f(card, "address");
        let Some(before) = expected.iter().find(|(_, after)| **after == at).map(|(before, _)| before.clone()) else {
            return Ok(false);
        };
        if let Some(want) = fingerprint_at(fingerprints, &before, applied)
            && calls::authored_fingerprint(card) != want
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn refresh(applied: &BTreeMap<String, String>, at: &str) -> String {
    applied.get(at).cloned().unwrap_or_else(|| at.to_string())
}

fn refresh_all(all: &BTreeMap<String, String>, here: &BTreeMap<String, String>, operation: &Value) -> BTreeMap<String, String> {
    let mut combined = all.clone();
    combined.extend(here.clone());
    let mut out = BTreeMap::new();
    for pair in ps::ps_items(&operation["mapping"]) {
        let list = pair.as_array().cloned().unwrap_or_default();
        let before = list.first().map(ps::s).unwrap_or_default();
        let after = list.get(1).map(ps::s).unwrap_or_default();
        out.insert(before.clone(), refresh(&combined, &after));
    }
    out
}

fn mapping_json(mapping: &BTreeMap<String, String>) -> Vec<Value> {
    mapping.iter().map(|(before, after)| json!([before, after])).collect()
}

fn next_side(titles: &BTreeMap<String, Value>, at: &str) -> String {
    let prefix = format!("{at}|");
    let next = titles
        .keys()
        .filter_map(|here| here.strip_prefix(&prefix))
        .filter_map(|rest| rest.split('|').next())
        .filter(|label| side_segment(label))
        .map(side_number)
        .max()
        .unwrap_or(0)
        + 1;
    format!("{at}|{}", side_label(next))
}

fn tree_titles(zt: &mut Zt, trees: &[String]) -> Result<BTreeMap<String, Value>> {
    let mut out: BTreeMap<String, Value> = BTreeMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for tree in trees {
        if tree.is_empty() || !seen.insert(tree.clone()) {
            continue;
        }
        let value = zt.call("get_tree", json!({ "tree": tree, "workspace": "agent" }))?;
        for row in ps::each(&value["cards"]) {
            out.insert(f(row, "address"), row.clone());
        }
    }
    Ok(out)
}

fn worst(statuses: &[String]) -> String {
    for status in ["conflict", "refused", "pending", "applied-and-verified", "unchanged"] {
        if statuses.iter().any(|known| known == status) {
            return status.to_string();
        }
    }
    "unchanged".to_string()
}

fn operation_result(id: &str, from: &str, to: &str, status: &str, reason: Value, verified: Option<Value>) -> Value {
    json!({
        "id": id,
        "from": from,
        "to": to,
        "status": status,
        "reason": reason,
        "verification": verified,
    })
}

fn render_result(result: &Value) -> String {
    let mut sb = Sb::new();
    sb.line(format!("# Topic join result: `{}`", f(result, "review_report_id"))).blank();
    sb.line(format!("Result: `{}`", f(result, "result_id")));
    sb.line(format!(
        "Archive `{}` revision {} → {}. Status: **{}**; complete: {}.",
        f(&result["archive"], "id"),
        f(&result["archive"], "revision_before"),
        f(&result["archive"], "revision_after"),
        f(result, "status"),
        result["complete"].as_bool().unwrap_or(false)
    ))
    .blank();
    let counts = &result["counts"];
    sb.line(format!(
        "Findings: {} · applied-and-verified {} · unchanged {} · conflict {} · refused {} · pending {}.",
        f(counts, "findings"),
        f(counts, "applied_and_verified"),
        f(counts, "unchanged"),
        f(counts, "conflict"),
        f(counts, "refused"),
        f(counts, "pending")
    ))
    .blank();
    sb.line("An operation is `applied-and-verified` only after its persisted addresses, authored fingerprints, Link rewrites, Broken links and model reports were read back; a tool that returned without an error is not enough (D274).").blank();
    for item in ps::ps_items(&result["items"]) {
        sb.line(format!("## `{}` — {}", f(&item, "finding"), f(&item, "action")));
        sb.line(format!("Status: **{}**. Note: {}", f(&item, "status"), cell(&f(&item, "note")))).blank();
        for operation in ps::ps_items(&item["operations"]) {
            sb.line(format!(
                "- `{}` `{}` → `{}`: **{}** — {}",
                f(&operation, "id"),
                f(&operation, "from"),
                f(&operation, "to"),
                f(&operation, "status"),
                cell(&f(&operation["reason"], "detail"))
            ));
        }
        sb.blank();
    }
    sb.line("Rollback evidence: the inverse of every applied mapping is in `rollback`.").blank();
    sb.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(address: &str, title: &str, card_type: &str, facet: &str) -> Value {
        json!({ "address": address, "title": title, "tree": tree_of(address), "workspace": "agent", "type": card_type, "facet": facet, "edge": if topic_root(address) { "root" } else { "direct" } })
    }

    fn snapshot(rows: Vec<Value>, books: &[&str]) -> Snapshot {
        let mut titles = BTreeMap::new();
        let mut topics = Vec::new();
        for item in rows {
            if f(&item, "edge") == "root" {
                topics.push(item.clone());
            }
            titles.insert(f(&item, "address"), item);
        }
        Snapshot {
            id: "0123456789abcdef0123456789abcdef".to_string(),
            path: "D:\\a".to_string(),
            revision: 7,
            titles,
            topics,
            books: books.iter().map(|book| book.to_string()).collect(),
        }
    }

    fn endpoint(address: &str, body: &str) -> Value {
        json!({ "address": address, "title": address, "body": body })
    }

    fn review() -> Value {
        let operations = json!([
            { "id": "t001/p001/o001", "from": "161/1", "to": "254/1|a",
              "with_successors": true, "mapping": [["161/1", "254/1|a"]], "links_rewritten": 0,
              "model_effect": { "preserved": true, "models": [] }, "fingerprints": [["161/1", "bb"]] }
        ]);
        let plans = json!([
            { "id": "t001/p001", "kind": "whole_chain", "status": "offered", "description": "move all entry chains", "operations": operations },
            { "id": "t001/p002", "kind": "lone_entries", "status": "refused", "description": "move entries alone",
              "reason": { "code": "human_link", "detail": "a human Card links `161/1`" }, "operations": [] }
        ]);
        let alternatives = json!([
            { "action": "keep_distinct", "description": "keep both" },
            { "action": "keep_both_with_crossing", "level": "topic", "description": "crossing through the topic hub" },
            { "action": "move_subtree", "keeper": "254/0", "joining": "254/1", "plans": plans }
        ]);
        let pairs = json!([{
            "id": "t001",
            "a": { "root": "161/0", "title": "均衡的多重性", "question": "什么时候有多个均衡？" },
            "b": { "root": "254/0", "title": "多重均衡", "question": "什么时候有多个均衡？" },
            "similarity": "0.5400",
            "question_similarity": "0.8000",
            "inside": true,
            "keeper": "254/0",
            "keeper_reason": "books",
            "keeper_note": "`254/0` carries 2 book(s); the Card count is never the reason (D159).",
            "joining": { "address": "254/1", "title": "预期渠道", "why": "closest in words" },
            "source_mix": { "a": ["K1"], "b": ["K1", "K2"], "shared": ["K1"], "only_a": [], "only_b": ["K2"] },
            "recommendation": "move_subtree",
            "reasons": [],
            "alternatives": alternatives
        }]);
        json!({
            "schema_version": 1,
            "kind": "topic-join-review",
            "created_at": "2026-10-19T00:00:00.0000000Z",
            "archive": { "id": "0123456789abcdef0123456789abcdef", "path": "D:\\a", "revision": 7 },
            "endpoints": {
                "161/0": { "address": "161/0", "fingerprint": "aa", "title": "均衡的多重性" },
                "161/1": { "address": "161/1", "fingerprint": "bb", "title": "两侧轨迹" },
                "254/1": { "address": "254/1", "fingerprint": "cc", "title": "预期渠道" },
            },
            "pairs": pairs,
        })
    }

    fn selection(finding: &str, action: &str, plan: &str) -> Value {
        json!({ "schema_version": 1, "report_id": format!("tj-{}", canon::content_hash(&review(), &["report_id", "created_at"])),
            "decisions": [{ "finding": finding, "action": action, "plan": plan, "note": "复核过。" }] })
    }

    fn reviewed_report() -> Value {
        let mut report = review();
        report["report_id"] = json!(format!("tj-{}", canon::content_hash(&report, &["report_id", "created_at"])));
        report
    }

    #[test]
    fn locations_follow_the_zt_rules() {
        assert_eq!(parent("3/2").as_deref(), Some("3/1"));
        assert_eq!(parent("3/1").as_deref(), Some("3/0"));
        assert_eq!(parent("3/0"), None);
        assert_eq!(parent("3/1|a").as_deref(), Some("3/1"));
        assert_eq!(parent("3/1|a|1").as_deref(), Some("3/1|a"));
        assert_eq!(direct_successor("3/0"), "3/1");
        assert_eq!(direct_successor("3/2"), "3/3");
        assert_eq!(direct_successor("3/1|a"), "3/1|a|1");
        assert!(topic_root("161/0") && !topic_root("161/1") && !topic_root("stiglitz2014learning"));
        assert_eq!(side_label(1), "a");
        assert_eq!(side_label(26), "z");
        assert_eq!(side_label(27), "aa");
        assert_eq!(side_number("aa"), 27);
        assert_eq!(last_segment("3/1|a|2"), "2");
    }

    #[test]
    fn the_snapshot_lists_a_topics_entry_chain_and_side_slots() {
        let snap = snapshot(
            vec![
                row("161/0", "A", "idea", ""),
                row("161/1", "A1", "idea", ""),
                row("161/2", "A2", "idea", ""),
                row("161/1|a", "A1a", "idea", ""),
                row("161/1|b", "A1b", "idea", ""),
            ],
            &[],
        );
        assert_eq!(snap.chain("161/0"), vec!["161/1".to_string(), "161/2".to_string()]);
        assert_eq!(snap.sides("161/1"), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(next_side(&snap.titles, "161/1"), "161/1|c");
        assert_eq!(descendants(&snap.titles, "161/1"), BTreeSet::from(["161/2".to_string(), "161/1|a".to_string(), "161/1|b".to_string()]));
    }

    #[test]
    fn the_pair_scan_keeps_close_titles_and_an_inside_title() {
        let topics = vec![
            row("161/0", "均衡的多重性", "idea", ""),
            row("254/0", "多重均衡", "idea", ""),
            row("160/0", "公理化讨价还价", "idea", ""),
        ];
        let mut questions = HashMap::new();
        questions.insert("161/0".to_string(), "什么时候有多个均衡？".to_string());
        questions.insert("254/0".to_string(), "什么时候有多个均衡？".to_string());
        questions.insert("160/0".to_string(), "讨价还价的解由什么公理决定？".to_string());
        let (found, matched) = pair_candidates(&topics, &questions, 0.5, 10);
        assert_eq!(matched, 1);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].a.as_str(), found[0].b.as_str()), ("161/0", "254/0"));
        assert!(found[0].similarity >= 0.5);
        // A bound is reported, never hidden.
        let (bounded, matched) = pair_candidates(&topics, &questions, 0.0, 1);
        assert_eq!(matched, 3);
        assert_eq!(bounded.len(), 1);
        // A title that contains the other is a candidate even when the questions differ.
        let only_titles = vec![row("1/0", "学习", "idea", ""), row("2/0", "学习与增长", "idea", "")];
        let (found, _) = pair_candidates(&only_titles, &HashMap::new(), 0.5, 10);
        assert_eq!(found.len(), 1);
        assert!(found[0].inside);
    }

    #[test]
    fn the_keeper_is_proposed_from_books_or_sourced_entries_never_a_count() {
        let (keeper, reason, note) = keeper_choice("161/0", &["K1".to_string(), "K2".to_string()], 1, "254/0", &["K1".to_string()], 2);
        assert_eq!(keeper, "161/0");
        assert_eq!(reason, "books");
        assert!(note.contains("2 book(s)"), "{note}");
        let (keeper, reason, _) = keeper_choice("161/0", &["K1".to_string()], 1, "254/0", &["K1".to_string()], 2);
        assert_eq!(keeper, "254/0");
        assert_eq!(reason, "sourced_entries");
        let (keeper, reason, note) = keeper_choice("161/0", &["K1".to_string()], 2, "254/0", &["K1".to_string()], 2);
        assert_eq!(keeper, "161/0");
        assert_eq!(reason, "unresolved");
        assert!(note.contains("Card count is never the reason"), "{note}");
    }

    #[test]
    fn the_joining_card_is_the_closest_keeper_entry_or_the_anchor() {
        let snap = snapshot(
            vec![
                row("254/0", "多重均衡", "idea", ""),
                row("254/1", "预期渠道", "idea", ""),
                row("254/2", "学习渠道", "idea", ""),
                row("161/0", "学习", "idea", ""),
                row("161/1", "学习使成本下降", "idea", ""),
                row("160/0", "债务契约", "idea", ""),
            ],
            &[],
        );
        let mut endpoints = Endpoints::default();
        for (address, body) in [
            ("254/0", "什么时候有多个均衡？"),
            ("254/1", "预期越高，本期生产越多。"),
            ("254/2", "学习使第二期产出更高，成本更低。"),
            ("161/0", "学习怎样改变成本？"),
            ("161/1", "累计产量提高使单位成本下降。"),
            ("160/0", "违约时债权人取得什么权利？"),
        ] {
            let card = endpoint(address, body);
            endpoints.cards.insert(address.to_string(), card);
        }
        let joining = joining_choice(&snap, &endpoints, "254/0", "161/0", &["161/1".to_string()]);
        assert_eq!(f(&joining, "address"), "254/2");
        assert!(joining["similarity"].as_str().is_some_and(|value| value != "0.0000"));
        let none = joining_choice(&snap, &endpoints, "254/0", "160/0", &[]);
        assert_eq!(f(&none, "address"), "254/1", "with no match the keeper's first entry anchors the join");
    }

    #[test]
    fn the_crossing_uses_an_existing_hub_level_and_no_new_type() {
        let snap = snapshot(
            vec![
                row("254/0", "多重均衡", "idea", ""),
                row("254/1", "预期渠道", "idea", ""),
                row("254/1|a", "主题枢纽", "hub", ""),
            ],
            &[],
        );
        let joining = json!({ "address": "254/1" });
        let crossing = crossing_alternative(&snap, "254/0", &["K1".to_string()], &["K1".to_string()], &joining);
        assert_eq!(f(&crossing, "level"), "topic");
        assert_eq!(f(&crossing, "hub"), "254/1|a");
        assert_eq!(f(&crossing, "group"), "邻近");
        let snap = snapshot(vec![row("254/0", "多重均衡", "idea", ""), row("254/1", "预期渠道", "idea", "")], &[]);
        let crossing = crossing_alternative(&snap, "254/0", &["K1".to_string()], &["K1".to_string()], &joining);
        assert_eq!(f(&crossing, "level"), "topic");
        assert_eq!(f(&crossing, "hub"), "" , "no hub is invented");
        assert_eq!(f(&crossing, "group"), "入口");
        let crossing = crossing_alternative(&snap, "254/0", &["K1".to_string()], &["K2".to_string()], &joining);
        assert_eq!(f(&crossing, "level"), "field");
        assert!(f(&crossing, "description").contains("field hub"));
    }

    #[test]
    fn a_move_plan_is_parsed_and_a_malformed_one_is_refused() {
        let lines: Vec<String> = ["move verification:", "161/1 -> 254/1|a", "161/2 -> 254/1|a|1", "moved cards: 2", "link macros rewritten: 3"]
            .iter()
            .map(|line| line.to_string())
            .collect();
        let (mapping, rewritten) = parse_plan_lines(&lines).unwrap();
        assert_eq!(mapping.get("161/1").map(String::as_str), Some("254/1|a"));
        assert_eq!(mapping.get("161/2").map(String::as_str), Some("254/1|a|1"));
        assert_eq!(rewritten, 3);
        let twice: Vec<String> = ["161/1 -> 254/1|a", "161/1 -> 254/1|b", "link macros rewritten: 0"].iter().map(|line| line.to_string()).collect();
        assert!(parse_plan_lines(&twice).unwrap_err().to_string().contains("twice"));
        let silent: Vec<String> = ["161/1 -> 254/1|a", "moved cards: 1"].iter().map(|line| line.to_string()).collect();
        assert!(parse_plan_lines(&silent).unwrap_err().to_string().contains("Link rewrites"));
    }

    #[test]
    fn moving_a_part_out_of_its_model_is_reported_as_a_model_change() {
        let titles = BTreeMap::from([
            ("161/1".to_string(), row("161/1", "模型卡", "model", "")),
            ("161/1|a".to_string(), row("161/1|a", "模型分卡", "model", "设定")),
            ("254/1".to_string(), row("254/1", "别的卡", "idea", "")),
        ]);
        // The part leaves its model: a change (refused by the plan).
        let effect = model_effect(&titles, &BTreeMap::from([("161/1|a".to_string(), "254/1|a".to_string())]));
        assert_eq!(effect["preserved"], false);
        assert_eq!(f(&effect["changes"][0], "kind"), "membership");
        // A whole model moves together: its membership is preserved.
        let effect = model_effect(
            &titles,
            &BTreeMap::from([
                ("161/1".to_string(), "254/1|b".to_string()),
                ("161/1|a".to_string(), "254/1|b|a".to_string()),
            ]),
        );
        assert_eq!(effect["preserved"], true);
        assert!(ps::ps_items(&effect["models"]).iter().any(|model| ps::s(model) == "161/1"));
        // A Model Card that leaves its part behind is a change too.
        let effect = model_effect(&titles, &BTreeMap::from([("161/1".to_string(), "254/1|b".to_string())]));
        assert_eq!(effect["preserved"], false);
        assert_eq!(f(&effect["changes"][0], "kind"), "parts_left_behind");
    }

    #[test]
    fn a_pending_address_is_refreshed_through_the_returned_mapping() {
        let applied = BTreeMap::from([
            ("161/1".to_string(), "254/1|a".to_string()),
            ("161/2".to_string(), "161/1".to_string()),
            ("161/3".to_string(), "161/2".to_string()),
        ]);
        assert_eq!(refresh(&applied, "161/2"), "161/1");
        assert_eq!(refresh(&applied, "161/9"), "161/9");
        // A reviewed fingerprint is found through the address it moved from.
        let fingerprints = BTreeMap::from([("161/2".to_string(), "bb".to_string())]);
        assert_eq!(fingerprint_at(&fingerprints, "161/1", &applied).as_deref(), Some("bb"));
        assert_eq!(fingerprint_at(&fingerprints, "161/2", &BTreeMap::new()).as_deref(), Some("bb"));
    }

    #[test]
    fn a_reviewed_selection_binds_the_report_the_finding_and_the_plan() {
        let report = reviewed_report();
        let ok = selection_checked(&report, "move_subtree", "t001/p001");
        let accepted = validate_selection(&report, &ok).unwrap();
        assert_eq!(accepted.len(), 1);
        assert_eq!(accepted[0].action, "move_subtree");
        assert_eq!(accepted[0].operations, vec!["t001/p001/o001".to_string()]);
        let distinct = selection_checked(&report, "keep_distinct", "");
        let accepted = validate_selection(&report, &distinct).unwrap();
        assert!(accepted[0].plan.is_none());
        let refused_plan = selection_checked(&report, "move_subtree", "t001/p002");
        assert!(validate_selection(&report, &refused_plan).unwrap_err().to_string().contains("select the nonmoving alternative"));
        let mut wrong = ok.clone();
        wrong["report_id"] = json!("tj-other");
        assert!(validate_selection(&report, &wrong).unwrap_err().to_string().contains("belongs to report"));
        let mut unknown = ok.clone();
        unknown["decisions"][0]["finding"] = json!("t999");
        assert!(validate_selection(&report, &unknown).unwrap_err().to_string().contains("does not hold"));
        let mut action = ok.clone();
        action["decisions"][0]["action"] = json!("rewrite_as_interpretation");
        assert!(validate_selection(&report, &action).unwrap_err().to_string().contains("not one of"));
        let mut note = ok.clone();
        note["decisions"][0]["note"] = json!("");
        assert!(validate_selection(&report, &note).unwrap_err().to_string().contains("needs a note"));
        let mut field = ok.clone();
        field["decisions"][0]["extra"] = json!(1);
        assert!(validate_selection(&report, &field).unwrap_err().to_string().contains("unknown field"));
        let mut hash = ok.clone();
        hash["decisions"][0]["fingerprints"] = json!({ "161/1": "zz" });
        assert!(validate_selection(&report, &hash).unwrap_err().to_string().contains("reviewed endpoint"));
        let mut twice = ok.clone();
        twice["decisions"] = json!([ok["decisions"][0].clone(), ok["decisions"][0].clone()]);
        assert!(validate_selection(&report, &twice).unwrap_err().to_string().contains("twice"));
    }

    fn selection_checked(report: &Value, action: &str, plan: &str) -> Value {
        let mut decision = json!({ "finding": "t001", "action": action, "note": "复核过。" });
        if !plan.is_empty() {
            decision["plan"] = json!(plan);
        }
        json!({ "schema_version": 1, "report_id": f(report, "report_id"), "decisions": [decision] })
    }

    #[test]
    fn a_topic_root_is_never_a_move_operation() {
        let mut report = reviewed_report();
        report["pairs"][0]["alternatives"][2]["plans"][0]["operations"][0]["from"] = json!("161/0");
        let asked = selection_checked(&report, "move_subtree", "t001/p001");
        let error = validate_selection(&report, &asked).unwrap_err().to_string();
        assert!(error.contains("Topic root cannot be moved"), "{error}");
    }

    #[test]
    fn the_worst_operation_status_orders_the_finding() {
        assert_eq!(worst(&["unchanged".to_string(), "applied-and-verified".to_string()]), "applied-and-verified");
        assert_eq!(worst(&["applied-and-verified".to_string(), "pending".to_string()]), "pending");
        assert_eq!(worst(&["pending".to_string(), "conflict".to_string()]), "conflict");
        assert_eq!(worst(&["refused".to_string(), "unchanged".to_string()]), "refused");
        assert_eq!(worst(&[]), "unchanged");
    }

    #[test]
    fn the_rendered_review_names_every_alternative_and_the_mapping() {
        let text = render_markdown(&review());
        for expected in ["keep_distinct", "keep_both_with_crossing", "move_subtree", "t001/p001/o001", "161/1` → `254/1|a", "Card count"] {
            assert!(text.contains(expected), "the review names {expected}:\n{text}");
        }
        let result = json!({
            "review_report_id": "tj-x", "result_id": "jr-x",
            "archive": { "id": "0123456789abcdef0123456789abcdef", "revision_before": 7, "revision_after": 8 },
            "status": "applied", "complete": true,
            "counts": { "findings": 1, "applied_and_verified": 1, "unchanged": 0, "conflict": 0, "refused": 0, "pending": 0 },
            "items": [{ "finding": "t001", "action": "move_subtree", "note": "复核过。", "status": "applied-and-verified",
                "operations": [{ "id": "t001/p001/o001", "from": "161/1", "to": "254/1|a", "status": "applied-and-verified",
                    "reason": { "detail": "verified" } }] }],
        });
        let text = render_result(&result);
        assert!(text.contains("applied-and-verified") && text.contains("not enough"));
    }

    #[test]
    fn the_bounds_are_the_named_constants() {
        assert_eq!(SIMILAR, 0.5);
        assert_eq!(MAX_PAIRS, 10);
        assert_eq!(MAX_OPERATIONS, 4);
        assert_eq!(MAX_PLAN_CARDS, 120);
        assert!(canon::integer_option("4", "MaxOperations", 1, MAX_OPERATIONS).is_ok());
        assert!(canon::integer_option("5", "MaxOperations", 1, MAX_OPERATIONS).is_err());
        assert_eq!(ACTIONS.len(), 3);
    }
}
