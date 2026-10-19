//! `zt-digest concepts`: one immutable concept, restatement and filing review of a prepared
//! digest run (docs/luhmann-comparison-implementation-design.md §4.2, §4.4, §7.1, §12.1–§12.3;
//! GitHub issue #97). Four explicit steps, never a query language:
//!
//!   zt-digest concepts -Step prepare|judge|report|decision -Staging <folder>
//!                      [-Work <folder>] [-File <json>] [-MaxTopics 10] [-MaxRestatements 10]
//!
//! -Staging is the prepared run's staging folder (`manifest.json`). The caller freezes the proposed
//! concepts in `<Work>\proposals.json`; every other file the steps read or write lives under
//! `<Work>` (default `<Staging>\concepts`):
//!
//!   prepare   reads the book's term table through zt-check's own parser (术语, 不用, 意思,
//!             首次定义), shorts at most ten Topic candidates per proposed concept (D287) and at
//!             most ten restatement candidates per proposed statement, deduplicated by unordered
//!             pair (D288), and freezes the evidence, one retained endpoint file per address, and
//!             the judge job: `evidence.json`, `endpoints\`, `cj-01.md`. It makes read calls only.
//!   judge     validates one canned judge answer file (`-File`) against the frozen evidence and
//!             writes `judged.json`: schema, duplicate keys, unknown verdicts, contradictory
//!             outcomes for one pair, missing evidence, unread endpoints. A refused or absent
//!             answer leaves those pairs unjudged; it never becomes a create or merge decision.
//!   report    renders `concept-review.json` and `concept-review.md` in the staging folder from
//!             the frozen files alone: it starts no `zt mcp`, so report generation cannot write a
//!             Card. The same evidence and answers give the same report id and change no file.
//!   decision  validates a reviewed gate decision (`-File`) against the report: the exact report
//!             id (recomputed), known finding ids, the finding's own verdict, one of the remedies
//!             the evidence supports, and the reviewed authored fingerprints. It writes
//!             `decision.json` and no Card.
//!
//! A report with unread endpoints or unjudged candidates is `partial` and names them; the candidate
//! limits and the unselected counts are in it, so a shortlist is never presented as an exhaustive
//! semantic comparison. Nothing here creates, merges, moves, or edits a Card: the applied new-run
//! resolution is another slice's job and would consume this report's id.

use crate::args::Args;
use crate::canon;
use crate::calls;
use crate::organize::common::{self, pstr, Sb};
use crate::ps::{self, NL};
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::LazyLock;

// --- the fixed bounds (§12.2, D287, D288) ---------------------------------------------------------

/// D287: at most ten Topic candidates per proposed concept.
pub const TOPIC_CANDIDATES: i64 = 10;
/// D288: at most ten restatement candidates per new Topic Card.
pub const RESTATEMENT_CANDIDATES: i64 = 10;
/// §12.2 asks the effective constants to be reported so a comparison is reproducible. They are
/// named here and never read from a settings file before pilot evidence asks for one.
const MAX_PROPOSALS: usize = 20;
const MAX_TOPICS: usize = 500;
const MAX_IDEAS: usize = 5_000;
const MAX_ENDPOINTS: usize = 4_000;
const MAX_JUDGMENTS: usize = 400;
const MAX_DECISIONS: usize = 200;
/// The Idea Cards whose bodies one proposal reads before it scores the full statements.
const PREFILTER: usize = 40;
/// A Topic holds one to three entry Cards; the shortlist reads at most three of them.
const ENTRIES_PER_TOPIC: usize = 3;
/// One line of a catalogue entry: a title or a question, in UTF-16 units.
const MAX_LINE: usize = 200;
/// One statement in the retained evidence, in UTF-16 units.
const MAX_STATEMENT: usize = 320;
const ENDPOINT_BODY: usize = 4_000;

const SCHEMA_VERSION: i64 = 1;
const VERDICTS: [&str; 3] = ["same", "different", "uncertain"];
const KINDS: [&str; 2] = ["topic-concept", "restatement"];

/// A line that states where a claim comes from (§4.3).
static SOURCE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(来源|出处|source)\s*[：:]").unwrap());
/// A label line that states an assumption; its text is part of the evidence (D078).
static ASSUMPTION_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(假设|假定|前提|条件|设定)\s*[：:]").unwrap());
static FORMULA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\$[^$]*\$\$|\$[^$\n]+\$").unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]*)?\]\]").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static TERM_TABLE_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^术语表").unwrap());
/// What is not a claim: a heading, a figure, a table, a quote, a code fence, a rule.
static NOT_A_STATEMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(#{1,6}\s|!\[|\||>|```|~~~|-{2,})").unwrap());

// --- refusals and faults --------------------------------------------------------------------------

/// D300: the archive moved between two reads of one set. `prepare` retries the whole set once and
/// then returns this as the changing-view fault; nothing is frozen.
#[derive(Debug)]
pub struct ChangingView;

impl std::fmt::Display for ChangingView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("incomplete concept evidence: changing read view across archive revisions")
    }
}

impl std::error::Error for ChangingView {}

fn changing_view() -> anyhow::Error {
    anyhow::Error::new(ChangingView)
}

fn is_changing_view(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ChangingView>().is_some()
}

fn refuse(message: impl Into<String>) -> anyhow::Error {
    canon::refuse(message.into())
}

const PROMPT: &str = "按以上说明判断这些提议的概念，写出 JSON 文件，运行 SELF-CHECK 改到没有错误。最后只回复一行 JSON。";

// --- the command ----------------------------------------------------------------------------------

struct Params {
    step: String,
    staging: String,
    work: String,
    file: String,
    max_topics: i64,
    max_restatements: i64,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Step", "Staging", "Work", "File", "MaxTopics", "MaxRestatements"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &[])?;
    let step = a.choice("Step", &["prepare", "judge", "report", "decision"])?;
    let staging = common::mandatory(&a, "Staging")?;
    let work = {
        let given = a.str("Work");
        if given.is_empty() { ps::join(&staging, "concepts") } else { given }
    };
    let p = Params {
        step,
        staging,
        work,
        file: a.str("File"),
        max_topics: canon::integer_option(
            &a.str_or("MaxTopics", &TOPIC_CANDIDATES.to_string()),
            "MaxTopics",
            1,
            TOPIC_CANDIDATES,
        )?,
        max_restatements: canon::integer_option(
            &a.str_or("MaxRestatements", &RESTATEMENT_CANDIDATES.to_string()),
            "MaxRestatements",
            1,
            RESTATEMENT_CANDIDATES,
        )?,
    };
    match p.step.as_str() {
        "prepare" => prepare(&p),
        "judge" => judge(&p),
        "report" => report(&p),
        _ => decision(&p),
    }
}

// --- pure text helpers ----------------------------------------------------------------------------

/// The text of one scalar field, as PowerShell's `[string]` reads it.
fn f(value: &Value, key: &str) -> String {
    ps::s(&value[key])
}

/// One line: white-space runs made one space, cut to `max` UTF-16 units with `…`.
fn line(text: &str, max: usize) -> String {
    let collapsed = SPACES.replace_all(text, " ");
    let trimmed = collapsed.trim();
    if ps::len16(trimmed) <= max {
        return trimmed.to_string();
    }
    format!("{}…", ps::cut16(trimmed, max))
}

fn cut16(text: &str, max: usize) -> String {
    if ps::len16(text) <= max { text.to_string() } else { format!("{}…", ps::cut16(text, max)) }
}

/// The character bigrams of a text without formulas and Links: the cheap candidate comparison of
/// §7.1. It decides what to read; it is never a semantic verdict (D081).
fn bigrams(text: &str) -> HashSet<String> {
    let without_formulas = FORMULA.replace_all(text, " ");
    let plain = LINK.replace_all(&without_formulas, " ");
    let stripped: String = plain.chars().filter(|c| c.is_alphanumeric()).collect();
    let chars: Vec<char> = stripped.chars().collect();
    chars.windows(2).map(|pair| pair.iter().collect()).collect()
}

/// Dice's coefficient of two bigram sets: 0 when either is empty, so a candidate with no shared
/// evidence is never selected.
fn similarity(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let common = a.iter().filter(|gram| b.contains(*gram)).count();
    2.0 * common as f64 / (a.len() + b.len()) as f64
}

/// The claim a Card makes: its first paragraph that is not a heading, figure, table, quote, code
/// fence, or label line. A label and the source line are evidence beside the claim, not the claim.
fn statement(body: &str) -> String {
    for paragraph in body.split("\n\n") {
        let t = paragraph.trim();
        if t.is_empty() || NOT_A_STATEMENT.is_match(t) || SOURCE_LINE.is_match(t) || ASSUMPTION_LINE.is_match(t) {
            continue;
        }
        let text = line(&LINK.replace_all(t, " "), MAX_STATEMENT);
        if !text.is_empty() {
            return text;
        }
    }
    String::new()
}

/// The assumption labels of one text (`假设：…`, `前提: …`), each once, in document order.
fn assumptions(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in text.split('\n') {
        let Some(m) = ASSUMPTION_LINE.find(l) else { continue };
        let value = line(&l[m.end()..], MAX_LINE);
        if !value.is_empty() && !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

/// The formulas of one text, as they were written (the dollars kept), each once, ordinal order.
fn formulas(text: &str) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for m in FORMULA.find_iter(text) {
        let formula = m.as_str().trim().to_string();
        if formula.len() > 2 {
            out.insert(formula);
        }
    }
    out.into_iter().collect()
}

/// The source evidence of one Card: every `[[…]]` on its `来源：` lines (§4.3).
fn sources(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in body.split('\n') {
        if !SOURCE_LINE.is_match(l) {
            continue;
        }
        for m in LINK.captures_iter(l) {
            let target = m[1].trim().to_string();
            if !target.is_empty() && !out.contains(&target) {
                out.push(target);
            }
        }
    }
    out
}

/// Whether a Link target lies inside one tree: the root itself or an address below it.
fn in_tree(target: &str, tree: &str) -> bool {
    target == tree || target.starts_with(&format!("{tree}/"))
}

/// The retained evidence file of one endpoint: its full body, so the report and a later decision
/// name bytes that are still on disk.
fn retain_endpoint(work: &str, card: &Value) -> Result<(String, String)> {
    let address = f(card, "address");
    let name = canon::sha256_hex(address.as_bytes());
    let path = ps::join(&ps::join(work, "endpoints"), &format!("{}.md", &name[..16]));
    let text = format!(
        "# `{address}` {}\n\n{}\n",
        line(&f(card, "title"), MAX_LINE),
        cut16(&f(card, "body"), ENDPOINT_BODY)
    );
    util::write_atomic(&path, &text)?;
    Ok((path, canon::sha256_hex(text.as_bytes())))
}

/// One term row as the evidence and the job carry it (D063).
fn term_json(row: &zt_check::check::TermRow) -> Value {
    json!({ "term": row.term, "avoid": row.avoid, "meaning": row.meaning, "defined": row.defined })
}

/// Whether a proposal that names no term row mentions one: its title or question holds the term or
/// a word the book rejects for it (D064: the book's naming evidence, not a synonym table).
fn term_mentions(proposal: &Value, row: &zt_check::check::TermRow) -> bool {
    let text = format!("{} {}", f(proposal, "title"), f(proposal, "question"));
    text.contains(row.term.as_str()) || row.avoid.iter().any(|word| text.contains(word.as_str()))
}

fn dedup_sorted(list: &mut Vec<String>) {
    list.sort_by(|a, b| canon::ordinal_cmp(a, b));
    list.dedup();
}

// --- the frozen proposals --------------------------------------------------------------------------

/// The proposed concepts the caller freezes in `<Work>\proposals.json`. This is the one input the
/// tool does not derive: a proposal is a new Topic Card's title, question and (when it has one)
/// statement, with the book's term rows it means. Everything else is read evidence.
fn read_proposals(work: &str, key: &str) -> Result<Vec<Value>> {
    let path = ps::join(work, "proposals.json");
    if !ps::exists(&path) {
        return Err(refuse(format!("concepts prepare needs the frozen proposals file `{path}`")));
    }
    let value = common::read_json(&path)?;
    let map = value.as_object().ok_or_else(|| refuse("proposals must be an object"))?;
    for name in map.keys() {
        if !["schema_version", "book", "proposals"].contains(&name.as_str()) {
            return Err(refuse(format!("proposals has an unknown field `{name}`")));
        }
    }
    if value["schema_version"].as_i64() != Some(SCHEMA_VERSION) {
        return Err(refuse("proposals requires schema_version 1"));
    }
    if !value["book"].is_null() && value["book"].as_str() != Some(key) {
        return Err(refuse(format!(
            "proposals belongs to book `{}`, not `{key}`",
            ps::s(&value["book"])
        )));
    }
    let rows = value["proposals"]
        .as_array()
        .ok_or_else(|| refuse("proposals requires an array of proposals"))?;
    if rows.is_empty() || rows.len() > MAX_PROPOSALS {
        return Err(refuse(format!(
            "proposals needs 1..{MAX_PROPOSALS} proposed concepts, not {}",
            rows.len()
        )));
    }
    let mut out: Vec<Value> = Vec::new();
    let mut titles: Vec<String> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let id = format!("p{:03}", index + 1);
        let object = row.as_object().ok_or_else(|| refuse(format!("{id} is not an object")))?;
        for name in object.keys() {
            if !["title", "question", "statement", "assumptions", "formulas", "sources", "terms"].contains(&name.as_str()) {
                return Err(refuse(format!("{id} has an unknown field `{name}`")));
            }
        }
        let title = line(row["title"].as_str().unwrap_or_default(), MAX_LINE);
        if title.is_empty() {
            return Err(refuse(format!("{id} requires a title")));
        }
        if titles.iter().any(|known| ps::eq(known, &title)) {
            return Err(refuse(format!("two proposals are titled `{title}`")));
        }
        titles.push(title.clone());
        let question = line(row["question"].as_str().unwrap_or_default(), MAX_LINE);
        if question.is_empty() {
            return Err(refuse(format!("{id} requires a question")));
        }
        let texts = |name: &str| -> Result<Vec<String>> {
            let mut list: Vec<String> = Vec::new();
            for item in ps::ps_items(&row[name]) {
                let text = line(&ps::s(&item), MAX_LINE);
                if text.is_empty() {
                    return Err(refuse(format!("{id} has an empty {name} entry")));
                }
                if !list.contains(&text) {
                    list.push(text);
                }
            }
            Ok(list)
        };
        let assumed = texts("assumptions")?;
        let declared = texts("formulas")?;
        let cited = texts("sources")?;
        let mut terms: Vec<String> = Vec::new();
        for item in ps::ps_items(&row["terms"]) {
            let name = ps::s(&item).trim().to_string();
            if name.is_empty() {
                return Err(refuse(format!("{id} names an empty term")));
            }
            if !terms.contains(&name) {
                terms.push(name);
            }
        }
        out.push(json!({
            "id": id,
            "title": title,
            "question": question,
            "statement": line(row["statement"].as_str().unwrap_or_default(), MAX_STATEMENT),
            "assumptions": assumed,
            "formulas": declared,
            "sources": cited,
            "term_names": terms,
        }));
    }
    Ok(out)
}

// --- reads ----------------------------------------------------------------------------------------

/// One endpoint collection: the Cards by address, the addresses that could not be read, and the
/// retained evidence files that carry the bytes a report names.
#[derive(Default)]
struct Endpoints {
    cards: BTreeMap<String, Value>,
    unread: Vec<(String, String)>,
}

impl Endpoints {
    fn get(&self, at: &str) -> Option<&Value> {
        self.cards.get(at)
    }

    fn is_unread(&self, at: &str) -> bool {
        self.unread.iter().any(|(known, _)| known == at)
    }

    fn why(&self, at: &str) -> String {
        self.unread
            .iter()
            .find(|(known, _)| known == at)
            .map(|(_, why)| why.clone())
            .unwrap_or_else(|| "the Card read did not return it".to_string())
    }
}

/// One bounded read of the frozen set. A missing address is an unread endpoint, never a silent
/// absence; the revision of every reply must be the one revision of the set (D300).
fn read_cards(zt: &mut Zt, addresses: &[String], revision: u64, unread: &mut Vec<(String, String)>) -> Result<Vec<Value>> {
    let mut cards: Vec<Value> = Vec::new();
    let mut offset = 0usize;
    let mut width = 200usize;
    while offset < addresses.len() {
        let chunk: Vec<String> = addresses[offset..(offset + width).min(addresses.len())].to_vec();
        let reply = zt.request("get_cards", json!({ "addresses": chunk, "view": "text" }))?;
        if let Some(message) = zt_check::mcp::reply_error(&reply) {
            let fault = reply["result"]["structuredContent"].clone();
            if fault["code"] == "read_limit" && chunk.len() > 1 {
                width = (chunk.len() / 2).max(1);
                continue;
            }
            bail!("incomplete concept evidence: {message}");
        }
        let value: Value = serde_json::from_str(&zt_check::mcp::reply_text(&reply))
            .context("incomplete concept evidence: the Card answer is not JSON")?;
        let rows: Vec<Value> = ps::ps_items(&value["cards"]);
        let current = value["archive_revision"]
            .as_u64()
            .context("incomplete concept evidence: missing archive revision")?;
        if current != revision {
            return Err(changing_view());
        }
        for row in &rows {
            let at = f(row, "address");
            if !chunk.contains(&at) {
                bail!("incomplete concept evidence: the answer names `{at}`, which was not asked for");
            }
            cards.push(row.clone());
        }
        let missing: Vec<String> = ps::ps_items(&value["missing"]).iter().map(ps::s).collect();
        for at in missing {
            unread.push((at, "the Card read did not return it".to_string()));
        }
        if value["complete"] != true {
            for at in &chunk {
                if !rows.iter().any(|row| &f(row, "address") == at) && !unread.iter().any(|(known, _)| known == at) {
                    unread.push((at.clone(), "the Card read did not return it".to_string()));
                }
            }
        }
        offset += chunk.len();
    }
    Ok(cards)
}

/// The catalogue of the whole agent Workspace: every Topic root, every Idea Card title, and every
/// agent Literature tree.
struct Catalogue {
    topics: Vec<Value>,
    ideas: Vec<Value>,
    books: Vec<String>,
}

fn catalogue(trees: &[Value], key: &str) -> Result<Catalogue> {
    let agent: Vec<&Value> = trees.iter().filter(|tree| f(tree, "workspace") == "agent").collect();
    let mut books: Vec<String> = agent
        .iter()
        .filter(|tree| f(tree, "kind") == "literature")
        .map(|tree| f(tree, "tree"))
        .collect();
    if !books.iter().any(|tree| tree == key) {
        bail!("no agent Literature tree `{key}`");
    }
    books.sort_by(|a, b| canon::ordinal_cmp(a, b));
    // zt-check's own classifier says what a Card is: a root, an anchor, a term table, a notation
    // table, a structure Card, a hub, a model or a part is not a statement, so only an Idea Card is
    // a restatement candidate (D078).
    let literature: HashSet<String> = books.iter().cloned().collect();
    let mut anchors: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for tree in &agent {
        if !literature.contains(&f(tree, "tree")) {
            continue;
        }
        let root = f(tree, "tree");
        let count = ps::each(&tree["cards"]).into_iter().filter(|row| is_anchor(&f(row, "address"), &root)).count();
        anchors.insert(root, count);
    }
    let mut topics: Vec<Value> = Vec::new();
    let mut ideas: Vec<Value> = Vec::new();
    for tree in &agent {
        let topic_tree = f(tree, "kind") == "topic";
        for row in ps::each(&tree["cards"]) {
            let root = f(row, "edge") == "root";
            if root {
                if topic_tree {
                    topics.push(row.clone());
                }
                continue;
            }
            if !f(row, "title").trim().is_empty() && zt_check::graph::category(row, &literature, &anchors) == "idea" {
                ideas.push(row.clone());
            }
        }
    }
    topics.sort_by(|a, b| canon::ordinal_cmp(&f(a, "address"), &f(b, "address")));
    ideas.sort_by(|a, b| canon::ordinal_cmp(&f(a, "address"), &f(b, "address")));
    Ok(Catalogue { topics, ideas, books })
}

/// Whether an address is an anchor of `tree`: `<tree>/<n>` with no further segment.
fn is_anchor(address: &str, tree: &str) -> bool {
    address
        .strip_prefix(&format!("{tree}/"))
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

/// The term-table Card of the run's book, as the title catalogue names it.
fn term_table_address(trees: &[Value], key: &str) -> Option<String> {
    let mut found: Option<String> = None;
    for tree in trees.iter().filter(|tree| f(tree, "workspace") == "agent" && f(tree, "tree") == key) {
        for row in ps::each(&tree["cards"]) {
            if TERM_TABLE_TITLE.is_match(&f(row, "title").trim()) {
                let at = f(row, "address");
                if found.as_ref().is_none_or(|known| canon::ordinal_cmp(&at, known) == std::cmp::Ordering::Less) {
                    found = Some(at);
                }
            }
        }
    }
    found
}

/// The representative Cards of one Topic from the title catalogue, in address order: the Topic's own
/// numbered Cards (`<tree>/1`, `<tree>/2`…), not its subtree. A Topic holds one to three entries, so
/// the shortlist reads at most three (§7.1).
fn topic_entries(trees: &[Value], root: &str) -> Vec<Value> {
    let tree = root.rsplit_once('/').map(|(head, _)| head.to_string()).unwrap_or_else(|| root.to_string());
    let prefix = format!("{tree}/");
    let mut rows: Vec<Value> = Vec::new();
    for branch in trees {
        for row in ps::each(&branch["cards"]) {
            let at = f(row, "address");
            let Some(rest) = at.strip_prefix(&prefix) else { continue };
            if rest.is_empty() || rest == "0" || !rest.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            rows.push(row.clone());
        }
    }
    rows.sort_by(|a, b| canon::ordinal_cmp(&f(a, "address"), &f(b, "address")));
    rows.truncate(ENTRIES_PER_TOPIC);
    rows
}

// --- prepare --------------------------------------------------------------------------------------

fn prepare(p: &Params) -> Result<i32> {
    let manifest = common::manifest(&p.staging)?;
    let key = pstr(&manifest, "citation_key");
    if key.is_empty() {
        bail!("manifest.json names no citation_key; a concept review needs its book");
    }
    let book_title = pstr(&manifest, "book_title");
    let proposals = read_proposals(&p.work, &key)?;
    std::fs::create_dir_all(ps::join(&p.work, "endpoints"))?;
    let mut zt = crate::zt::start()?;
    let mut attempt = 0;
    let (evidence, job) = loop {
        attempt += 1;
        match observe(&mut zt, p, &key, &book_title, &proposals) {
            Ok(frozen) => break frozen,
            Err(error) if is_changing_view(&error) && attempt == 1 => continue,
            Err(error) => return Err(error),
        }
    };
    let evidence_path = ps::join(&p.work, "evidence.json");
    canon::save_json(&evidence_path, &evidence)?;
    let job_path = ps::join(&p.work, "cj-01.md");
    util::write_atomic(&job_path, &job)?;
    let doc = util::read_text(ps::join(&common::scripts_dir(), "dsh-concept-instructions.zh.md"))?;
    let doc_copy = ps::join(&p.work, "dsh-concept-instructions.zh.md");
    util::write_atomic(&doc_copy, &doc)?;
    let coverage = &evidence["coverage"];
    let unread = evidence["unread"].as_array().map_or(0, |list| list.len());
    let summary = json!({
        "ok": true,
        "step": "prepare",
        "evidence_id": evidence["evidence_id"],
        "archive_id": evidence["archive"]["id"],
        "archive_revision": evidence["archive"]["revision"],
        "proposals": proposals.len(),
        "term_rows": ps::pipe(&evidence["term_table"]["rows"]).len(),
        "topic_candidates": coverage["topic_candidates"],
        "topic_selected": coverage["topic_selected"],
        "topic_unselected": coverage["topic_unselected"],
        "restatement_candidates": coverage["restatement_candidates"],
        "restatement_selected": coverage["restatement_selected"],
        "restatement_unselected": coverage["restatement_unselected"],
        "unread": unread,
        "attempts": attempt,
        "evidence": evidence_path,
        "jobs": [{ "id": "cj-01", "files": [doc_copy, job_path], "prompt": PROMPT, "answer": ps::join(&ps::join(&p.work, "out"), "cj-01.json") }],
    });
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

/// One attempt at the frozen evidence: the archive view, the title catalogue, the bodies the
/// shortlists name, and one closing revision probe.
fn observe(zt: &mut Zt, p: &Params, key: &str, book_title: &str, proposals: &[Value]) -> Result<(Value, String)> {
    let view = zt.archive_view()?;
    let revision = view["archive_revision"]
        .as_u64()
        .context("incomplete concept evidence: no archive revision")?;
    let trees = crate::snapshot::read_title_trees(zt)?;
    for tree in &trees {
        if tree["archive_revision"].as_u64() != Some(revision) {
            return Err(changing_view());
        }
    }
    let catalogue = catalogue(&trees, key)?;
    if catalogue.topics.len() > MAX_TOPICS {
        bail!(
            "a concept review needs a bounded Topic catalogue: this archive has {} agent Topics (limit {MAX_TOPICS})",
            catalogue.topics.len()
        );
    }
    if catalogue.ideas.len() > MAX_IDEAS {
        bail!(
            "a concept review needs a bounded statement catalogue: this archive has {} Idea Cards (limit {MAX_IDEAS})",
            catalogue.ideas.len()
        );
    }
    let table_at = term_table_address(&trees, key);
    let mut unread: Vec<(String, String)> = Vec::new();
    let mut endpoints = Endpoints::default();
    // The first read: the term table and every Topic root.
    let mut wanted: Vec<String> = Vec::new();
    if let Some(at) = &table_at {
        wanted.push(at.clone());
    }
    for topic in &catalogue.topics {
        wanted.push(f(topic, "address"));
    }
    dedup_sorted(&mut wanted);
    for card in read_cards(zt, &wanted, revision, &mut unread)? {
        endpoints.cards.insert(f(&card, "address"), card);
    }
    let topics: Vec<Value> = catalogue
        .topics
        .iter()
        .filter_map(|topic| endpoints.get(&f(topic, "address")))
        .map(|card| json!({ "address": f(card, "address"), "title": f(card, "title"), "question": line(&f(card, "body"), MAX_LINE) }))
        .collect();
    // The book's term rows, through zt-check's own parser (D063).
    let table_rows: Vec<zt_check::check::TermRow> = table_at
        .as_ref()
        .and_then(|at| endpoints.get(at))
        .map(|card| zt_check::check::term_table(&f(card, "body")))
        .unwrap_or_default();
    let mut matched: Vec<Vec<zt_check::check::TermRow>> = Vec::new();
    for proposal in proposals {
        let named: Vec<String> = ps::ps_items(&proposal["term_names"]).iter().map(ps::s).collect();
        for name in &named {
            if !table_rows.iter().any(|row| ps::eq(&row.term, name)) {
                return Err(refuse(format!(
                    "proposal `{}` names the term `{name}`, which the book's term table does not define",
                    f(proposal, "title")
                )));
            }
        }
        let rows: Vec<zt_check::check::TermRow> = table_rows
            .iter()
            .filter(|row| {
                if named.is_empty() {
                    term_mentions(proposal, row)
                } else {
                    named.iter().any(|name| ps::eq(name, &row.term))
                }
            })
            .cloned()
            .collect();
        matched.push(rows);
    }
    // The Topic shortlist (D287) and the representative Cards it needs.
    let mut topic_lists: Vec<(Vec<Value>, usize)> = Vec::new();
    let mut topic_reads: Vec<String> = Vec::new();
    for (proposal, rows) in proposals.iter().zip(matched.iter()) {
        let mut with_terms = proposal.clone();
        with_terms["terms"] = Value::Array(rows.iter().map(term_json).collect());
        let (chosen, count) = topic_candidates(&with_terms, &topics, p.max_topics as usize);
        for entry in &chosen {
            let root = f(&entry["topic"], "address");
            topic_reads.push(root.clone());
            for row in topic_entries(&trees, &root) {
                topic_reads.push(f(&row, "address"));
            }
        }
        topic_lists.push((chosen, count));
    }
    // The statement prefilter (§7.1: cheap comparisons first, then the full statements).
    let mut prefiltered: Vec<Vec<Value>> = Vec::new();
    let mut idea_reads: Vec<String> = Vec::new();
    for (proposal, rows) in proposals.iter().zip(matched.iter()) {
        let mut with_terms = proposal.clone();
        with_terms["terms"] = Value::Array(rows.iter().map(term_json).collect());
        let query = bigrams(&format!("{} {}", f(&with_terms, "statement"), f(&with_terms, "title")));
        let mut scored: Vec<(f64, String)> = Vec::new();
        for idea in &catalogue.ideas {
            let score = similarity(&query, &bigrams(&f(idea, "title")));
            if score > 0.0 {
                scored.push((score, f(idea, "address")));
            }
        }
        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| canon::ordinal_cmp(&a.1, &b.1))
        });
        let seen: Vec<Value> = scored
            .into_iter()
            .take(PREFILTER)
            .enumerate()
            .map(|(rank, (score, at))| json!({ "address": at, "similarity": similarity_text(score), "rank": rank + 1 }))
            .collect();
        for row in &seen {
            idea_reads.push(f(row, "address"));
        }
        prefiltered.push(seen);
    }
    let mut rest: Vec<String> = topic_reads.clone();
    rest.extend(idea_reads.clone());
    dedup_sorted(&mut rest);
    for card in read_cards(zt, &rest, revision, &mut unread)? {
        endpoints.cards.insert(f(&card, "address"), card);
    }
    endpoints.unread = unread;
    // The closing probe: still one revision, or the set is not one view (D300).
    let closed = zt.archive_view()?;
    if closed["archive_revision"].as_u64() != Some(revision) {
        return Err(changing_view());
    }
    // Freeze the selected evidence, one pair at a time (D288).
    let mut frozen: Vec<Value> = Vec::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut topic_selected = 0usize;
    let mut topic_matched = 0usize;
    let mut restatement_selected = 0usize;
    let mut restatement_matched = 0usize;
    let mut prefilter_total = 0usize;
    for (index, ((proposal, rows), (chosen, count))) in proposals.iter().zip(matched.iter()).zip(topic_lists.iter()).enumerate() {
        let id = f(proposal, "id");
        let mut topics_out: Vec<Value> = Vec::new();
        for entry in chosen {
            let topic = &entry["topic"];
            let root = f(topic, "address");
            let mut entries: Vec<Value> = Vec::new();
            let mut own_book: Vec<String> = Vec::new();
            let mut other_books: Vec<String> = Vec::new();
            let mut unread_here: Vec<String> = Vec::new();
            for row in topic_entries(&trees, &root) {
                let at = f(&row, "address");
                let Some(card) = endpoints.get(&at) else {
                    unread_here.push(at.clone());
                    continue;
                };
                let src = sources(&f(card, "body"));
                for target in &src {
                    if in_tree(target, key) {
                        if !own_book.contains(target) {
                            own_book.push(target.clone());
                        }
                    } else if catalogue.books.iter().any(|tree| in_tree(target, tree)) && !other_books.contains(target) {
                        other_books.push(target.clone());
                    }
                }
                entries.push(json!({
                    "address": at,
                    "title": f(card, "title"),
                    "statement": statement(&f(card, "body")),
                    "sources": src,
                }));
            }
            if endpoints.is_unread(&root) && !unread_here.contains(&root) {
                unread_here.push(root.clone());
            }
            topic_selected += 1;
            topics_out.push(json!({
                "address": root,
                "title": f(topic, "title"),
                "question": f(topic, "question"),
                "similarity": entry["similarity"],
            "rank": entry["rank"],
                "entries": entries,
                "own_book": own_book,
                "other_books": other_books,
                "unread": unread_here,
            }));
        }
        topic_matched += count;
        let pre = &prefiltered[index];
        prefilter_total += pre.len();
        let proposed_statement = f(proposal, "statement");
        let mut proposed_formulas = formulas(&proposed_statement);
        for formula in ps::ps_items(&proposal["formulas"]).iter().map(ps::s) {
            if !proposed_formulas.contains(&formula) {
                proposed_formulas.push(formula);
            }
        }
        let query = bigrams(&format!("{proposed_statement} {}", f(proposal, "title")));
        let mut rows_out: Vec<(bool, f64, String, Value)> = Vec::new();
        let mut scored_total = 0usize;
        for seen in pre {
            let at = f(seen, "address");
            let Some(card) = endpoints.get(&at) else { continue };
            let body = f(card, "body");
            let claim = statement(&body);
            let shared_formula = proposed_formulas.iter().any(|formula| body.contains(formula.as_str()));
            let score = similarity(&query, &bigrams(&format!("{claim} {}", f(card, "title"))));
            if score <= 0.0 && !shared_formula {
                continue;
            }
            scored_total += 1;
            rows_out.push((
                shared_formula,
                score,
                at.clone(),
                json!({
                    "address": at,
                    "title": f(card, "title"),
                    "statement": claim,
                    "assumptions": assumptions(&body),
                    "formulas": formulas(&body),
                    "sources": sources(&body),
                    "formula_match": shared_formula,
                    "unread": Vec::<String>::new(),
                }),
            ));
        }
        // A shared formula first, then the closer statement, then the archive's own order.
        rows_out.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| canon::ordinal_cmp(&a.2, &b.2))
        });
        let chosen_rows: Vec<Value> = rows_out
            .into_iter()
            .take(p.max_restatements as usize)
            .enumerate()
            .map(|(rank, (_, score, _, mut row))| {
                row["similarity"] = json!(similarity_text(score));
                row["rank"] = json!(rank + 1);
                row
            })
            .collect();
        restatement_selected += chosen_rows.len();
        restatement_matched += scored_total;
        for row in topics_out.iter().chain(chosen_rows.iter()) {
            push_pair(&mut pairs, &id, &f(row, "address"));
        }
        frozen.push(json!({
            "id": id,
            "title": f(proposal, "title"),
            "question": f(proposal, "question"),
            "statement": proposed_statement,
            "assumptions": proposal["assumptions"],
            "formulas": proposed_formulas,
            "sources": proposal["sources"],
            "terms": rows.iter().map(term_json).collect::<Vec<_>>(),
            "term": rows.first().map(term_json).unwrap_or(Value::Null),
            "topic_candidates": topics_out,
            "restatement_candidates": chosen_rows,
            "topic_matched": count,
            "restatement_matched": scored_total,
            "prefiltered": pre.len(),
        }));
    }
    // Every endpoint a finding may name, with its retained evidence file.
    let mut named: BTreeSet<String> = BTreeSet::new();
    if let Some(at) = &table_at {
        named.insert(at.clone());
    }
    for proposal in &frozen {
        for row in ps::ps_items(&proposal["topic_candidates"]).iter().chain(ps::ps_items(&proposal["restatement_candidates"]).iter()) {
            named.insert(f(row, "address"));
            for entry in ps::ps_items(&row["entries"]) {
                named.insert(f(&entry, "address"));
            }
        }
        for target in ps::ps_items(&proposal["sources"]).iter().map(ps::s) {
            if endpoints.get(&target).is_some() {
                named.insert(target);
            }
        }
    }
    if named.len() > MAX_ENDPOINTS {
        bail!("a concept review retains at most {MAX_ENDPOINTS} endpoint snapshots, not {}", named.len());
    }
    let mut endpoints_out = Map::new();
    let mut unread_out: Vec<Value> = Vec::new();
    for at in &named {
        match endpoints.get(at) {
            Some(card) => {
                let (path, sha256) = retain_endpoint(&p.work, card)?;
                endpoints_out.insert(
                    at.clone(),
                    json!({
                        "address": at,
                        "title": f(card, "title"),
                        "tree": f(card, "tree"),
                        "workspace": f(card, "workspace"),
                        "type": f(card, "type"),
                        "fingerprint": calls::authored_fingerprint(card),
                        "path": path,
                        "sha256": sha256,
                    }),
                );
            }
            None => unread_out.push(json!({ "address": at, "why": endpoints.why(at) })),
        }
    }
    for (at, why) in &endpoints.unread {
        if !unread_out.iter().any(|row| f(row, "address") == *at) {
            unread_out.push(json!({ "address": at, "why": why }));
        }
    }
    unread_out.sort_by(|a, b| canon::ordinal_cmp(&f(a, "address"), &f(b, "address")));
    let coverage = json!({
        "topics_catalogued": catalogue.topics.len(),
        "ideas_catalogued": catalogue.ideas.len(),
        "prefiltered": prefilter_total,
        "topic_candidates": topic_matched,
        "topic_selected": topic_selected,
        "topic_unselected": topic_matched - topic_selected,
        "restatement_candidates": restatement_matched,
        "restatement_selected": restatement_selected,
        "restatement_unselected": restatement_matched - restatement_selected,
        "pairs": pairs.len(),
    });
    let mut payload = json!({
        "schema_version": SCHEMA_VERSION,
        "created_at": canon::utc_now_o(),
        "book": { "citation_key": key, "title": book_title },
        "archive": {
            "id": view["archive_identity"]["id"],
            "path": view["archive_identity"]["path"],
            "revision": revision,
        },
        "bounds": {
            "topic_candidates": p.max_topics,
            "restatement_candidates": p.max_restatements,
            "proposals": MAX_PROPOSALS,
            "topics_catalogued": MAX_TOPICS,
            "ideas_catalogued": MAX_IDEAS,
            "endpoints": MAX_ENDPOINTS,
            "prefilter": PREFILTER,
            "entries_per_topic": ENTRIES_PER_TOPIC,
        },
        "term_table": {
            "address": table_at.clone().map(Value::String).unwrap_or(Value::Null),
            "read": table_at.is_some(),
            "rows": table_rows.iter().map(term_json).collect::<Vec<_>>(),
        },
        "catalogue": { "topics": catalogue.topics.len(), "ideas": catalogue.ideas.len(), "books": catalogue.books },
        "coverage": coverage,
        "endpoints": Value::Object(endpoints_out),
        "unread": unread_out,
        "proposals": frozen,
    });
    let id = format!("e-{}", canon::content_hash(&payload, &["evidence_id", "created_at"]));

    payload["evidence_id"] = json!(id);
    let job = job_text(&payload, &p.staging, &p.work);
    Ok((payload, job))
}

/// The Topic candidates of one proposal: every Topic whose title or question shares evidence with
/// the proposal, at most `max` of them, most similar first (D287).
fn topic_candidates(proposal: &Value, topics: &[Value], max: usize) -> (Vec<Value>, usize) {
    let terms: Vec<String> = ps::ps_items(&proposal["terms"])
        .iter()
        .map(|term| {
            let avoid: Vec<String> = ps::ps_items(&term["avoid"]).iter().map(ps::s).collect();
            format!("{} {}", f(term, "term"), avoid.join(" "))
        })
        .collect();
    let query = bigrams(&format!("{} {} {}", f(proposal, "title"), f(proposal, "question"), terms.join(" ")));
    let mut scored: Vec<(f64, Value)> = Vec::new();
    for topic in topics {
        let score = similarity(&query, &bigrams(&format!("{} {}", f(topic, "title"), f(topic, "question"))));
        if score > 0.0 {
            scored.push((score, topic.clone()));
        }
    }
    // A fixed bound: the score first, then the archive's own address order.
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| canon::ordinal_cmp(&f(&a.1, "address"), &f(&b.1, "address")))
    });
    let matched = scored.len();
    let chosen = scored
        .into_iter()
        .take(max)
        .enumerate()
        .map(|(rank, (score, topic))| json!({ "similarity": similarity_text(score), "rank": rank + 1, "topic": topic }))
        .collect();
    (chosen, matched)
}

/// A candidate similarity as the frozen evidence spells it. The evidence carries no binary float:
/// another JSON reader's parse of a float can land on the neighbouring value, and then the
/// recomputed evidence id would differ from the one prepare wrote. The order the shortlist was
/// made in is materialized as `rank`, so this number is only a readable comparison.
fn similarity_text(score: f64) -> String {
    format!("{score:.4}")
}

/// The unordered pair a candidate is: `(proposal, Card)`. A pair appears once however many signals
/// point at it (D288).
fn push_pair(pairs: &mut Vec<(String, String)>, proposal: &str, address: &str) {
    if !pairs.iter().any(|(known, at)| known == proposal && at == address) {
        pairs.push((proposal.to_string(), address.to_string()));
    }
}

/// The job file: the four term columns and the full candidate evidence together (D064).
fn job_text(evidence: &Value, staging: &str, work: &str) -> String {
    let key = pstr(&evidence["book"], "citation_key");
    let title = pstr(&evidence["book"], "title");
    let answer = ps::join(&ps::join(work, "out"), "cj-01.json");
    let mut sb = Sb::new();
    sb.line(format!("# 概念复核任务 cj-01：《{title}》（`[[{key}]]`）")).blank();
    sb.line(format!("把结果写入 `{answer}`。")).blank();
    sb.line(format!("SELF-CHECK: zt-digest concepts -Step judge -Staging \"{staging}\" -File \"{answer}\"")).blank();
    sb.line(format!("冻结的证据集：`{}`。答案的 `evidence_id` 必须写它。", f(evidence, "evidence_id"))).blank();
    if !ps::truthy(&evidence["term_table"]["read"]) {
        sb.line("本书没有术语表卡：术语行缺失。意思不清时判 uncertain 并写出要读的卡。").blank();
    }
    for proposal in ps::ps_items(&evidence["proposals"]) {
        sb.line(format!("## 提议 {}：{}", f(&proposal, "id"), f(&proposal, "title"))).blank();
        sb.line(format!("问题：{}", f(&proposal, "question")));
        if !f(&proposal, "statement").is_empty() {
            sb.line(format!("陈述：{}", f(&proposal, "statement")));
        }
        let assumed: Vec<String> = ps::ps_items(&proposal["assumptions"]).iter().map(ps::s).collect();
        if !assumed.is_empty() {
            sb.line(format!("假设：{}", assumed.join("、")));
        }
        for formula in ps::ps_items(&proposal["formulas"]).iter().map(ps::s) {
            sb.line(format!("公式：{formula}"));
        }
        let cited: Vec<String> = ps::ps_items(&proposal["sources"]).iter().map(ps::s).collect();
        if !cited.is_empty() {
            sb.line(format!("来源：{}", cited.join("、")));
        }
        sb.blank();
        let rows = ps::ps_items(&proposal["terms"]);
        if rows.is_empty() {
            sb.line("术语：本书的术语表没有给这个提议的行。").blank();
        } else {
            sb.line(format!("术语（本书《{title}》的术语表）：")).blank();
            for row in &rows {
                let avoid: Vec<String> = ps::ps_items(&row["avoid"]).iter().map(ps::s).collect();
                sb.line(format!("- 术语：`{}`", f(row, "term")));
                sb.line(format!("  不用：{}", if avoid.is_empty() { "（没有）".to_string() } else { avoid.join("、") }));
                sb.line(format!("  意思：{}", f(row, "meaning")));
                sb.line(format!("  首次定义：{}", f(row, "defined")));
            }
            sb.blank();
        }
        let topics = ps::ps_items(&proposal["topic_candidates"]);
        sb.line(format!("候选主题（本次只读了 {} 个，不是全部主题）：", topics.len())).blank();
        if topics.is_empty() {
            sb.line("（没有）").blank();
        }
        for (index, topic) in topics.iter().enumerate() {
            sb.line(format!("### {}. `{}` {}", index + 1, f(topic, "address"), f(topic, "title")));
            sb.line(format!("问题：{}", f(topic, "question")));
            let entries = ps::ps_items(&topic["entries"]);
            if entries.is_empty() {
                sb.line("代表卡片：（没有读到）");
            } else {
                sb.line("代表卡片：");
                for entry in &entries {
                    sb.line(format!("- `{}` {}", f(entry, "address"), f(entry, "title")));
                    if !f(entry, "statement").is_empty() {
                        sb.line(format!("  陈述：{}", f(entry, "statement")));
                    }
                    let src: Vec<String> = ps::ps_items(&entry["sources"]).iter().map(ps::s).collect();
                    if !src.is_empty() {
                        sb.line(format!("  来源：{}", src.join("、")));
                    }
                    sb.line(format!("  全文：{}", endpoint_path(evidence, &f(entry, "address"))));
                }
            }
            for at in ps::ps_items(&topic["unread"]).iter().map(ps::s) {
                sb.line(format!("（`{at}` 没有读到：不得对这个候选判 same 或 different。）"));
            }
            sb.blank();
        }
        let restatements = ps::ps_items(&proposal["restatement_candidates"]);
        sb.line(format!("候选陈述（本次只读了 {} 个，不是全部陈述）：", restatements.len())).blank();
        if restatements.is_empty() {
            sb.line("（没有）").blank();
        }
        for (index, row) in restatements.iter().enumerate() {
            sb.line(format!(
                "### {}. `{}` {}{}",
                index + 1,
                f(row, "address"),
                f(row, "title"),
                if ps::truthy(&row["formula_match"]) { "（公式相同）" } else { "" }
            ));
            sb.line(format!("陈述：{}", f(row, "statement")));
            let assumed: Vec<String> = ps::ps_items(&row["assumptions"]).iter().map(ps::s).collect();
            sb.line(format!("假设：{}", if assumed.is_empty() { "（没有写）".to_string() } else { assumed.join("、") }));
            let forms: Vec<String> = ps::ps_items(&row["formulas"]).iter().map(ps::s).collect();
            if !forms.is_empty() {
                sb.line(format!("公式：{}", forms.join("、")));
            }
            let src: Vec<String> = ps::ps_items(&row["sources"]).iter().map(ps::s).collect();
            sb.line(format!("来源：{}", if src.is_empty() { "（没有来源行）".to_string() } else { src.join("、") }));
            sb.line(format!("全文：{}", endpoint_path(evidence, &f(row, "address"))));
            sb.blank();
        }
    }
    sb.0.trim_end().to_string() + NL
}

fn endpoint_path(evidence: &Value, at: &str) -> String {
    evidence["endpoints"]
        .as_object()
        .and_then(|map| map.get(at))
        .map(|endpoint| f(endpoint, "path"))
        .unwrap_or_default()
}

// --- judge ----------------------------------------------------------------------------------------

fn judge(p: &Params) -> Result<i32> {
    if p.file.is_empty() {
        return Err(refuse("concepts judge needs -File, the judge answer file"));
    }
    let evidence = frozen_evidence(&p.work)?;
    let answer = canon::read_json(&p.file)?;
    let judged = validate_answer(&evidence, &answer)?;
    let path = ps::join(&p.work, "judged.json");
    canon::save_json(&path, &judged)?;
    let total = pairs(&evidence).len();
    let count = judged["judgments"].as_array().map_or(0, |list| list.len());
    let summary = json!({
        "ok": true,
        "step": "judge",
        "evidence_id": evidence["evidence_id"],
        "judged_id": judged["judged_id"],
        "judged": count,
        "pairs": total,
        "unjudged": total.saturating_sub(count),
        "file": path,
    });
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

/// The frozen evidence, verified against its own canonical id: a tampered or edited set is never
/// silently reported (D080).
fn frozen_evidence(work: &str) -> Result<Value> {
    let path = ps::join(work, "evidence.json");
    if !ps::exists(&path) {
        return Err(refuse(format!(
            "no frozen concept evidence at `{path}`; run -Step prepare first"
        )));
    }
    let evidence = common::read_json(&path)?;
    let id = format!("e-{}", canon::content_hash(&evidence, &["evidence_id", "created_at"]));
    if evidence["evidence_id"].as_str() != Some(id.as_str()) {
        return Err(refuse(format!(
            "the frozen evidence at `{path}` does not match its evidence id; run -Step prepare again"
        )));
    }
    if evidence["schema_version"].as_i64() != Some(SCHEMA_VERSION) {
        return Err(refuse("the frozen concept evidence requires schema_version 1"));
    }
    Ok(evidence)
}

/// Every pair the review prepared: one per Topic candidate and one per restatement candidate,
/// deduplicated by unordered pair (D288).
fn pairs(evidence: &Value) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    for proposal in ps::ps_items(&evidence["proposals"]) {
        let id = f(&proposal, "id");
        for kind in KINDS {
            let key = if kind == "topic-concept" { "topic_candidates" } else { "restatement_candidates" };
            for row in ps::ps_items(&proposal[key]) {
                let pair = (id.clone(), kind.to_string(), f(&row, "address"));
                if !out.contains(&pair) {
                    out.push(pair);
                }
            }
        }
    }
    out
}

fn prepared_pair(proposal: &Value, kind: &str, address: &str) -> Option<Value> {
    let key = if kind == "topic-concept" { "topic_candidates" } else { "restatement_candidates" };
    ps::ps_items(&proposal[key]).into_iter().find(|row| f(row, "address") == address)
}

/// The addresses a judgment on one pair may cite: the candidate, the representative Cards the
/// review read for it, its source evidence that was prepared as an endpoint, and the book's term
/// table. Nothing else was read, so nothing else is evidence.
fn pair_endpoints(evidence: &Value, proposal: &Value, kind: &str, address: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![address.to_string()];
    if let Some(row) = prepared_pair(proposal, kind, address) {
        for entry in ps::ps_items(&row["entries"]) {
            let at = f(&entry, "address");
            if !out.contains(&at) {
                out.push(at);
            }
        }
        for target in ps::ps_items(&row["sources"]).iter().map(ps::s) {
            if evidence["endpoints"].get(&target).is_some() && !out.contains(&target) {
                out.push(target);
            }
        }
    }
    for target in ps::ps_items(&proposal["sources"]).iter().map(ps::s) {
        if evidence["endpoints"].get(&target).is_some() && !out.contains(&target) {
            out.push(target);
        }
    }
    let table = f(&evidence["term_table"], "address");
    if !table.is_empty() && !out.contains(&table) {
        out.push(table);
    }
    out
}

/// The unread endpoints of one pair: a judgment that claims a verdict over one of them is refused.
fn pair_unread(evidence: &Value, proposal: &Value, kind: &str, address: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let globally = ps::ps_items(&evidence["unread"]);
    if globally.iter().any(|row| f(row, "address") == address) {
        out.push(address.to_string());
    }
    if let Some(row) = prepared_pair(proposal, kind, address) {
        for at in ps::ps_items(&row["unread"]).iter().map(ps::s) {
            if !out.contains(&at) {
                out.push(at);
            }
        }
    }
    out
}

/// D063/D064/D081/D300/D302: one judge answer, validated. Malformed answers are refused by name; a
/// refusal writes nothing, so a failed or timed-out judge never becomes a create or merge decision.
fn validate_answer(evidence: &Value, answer: &Value) -> Result<Value> {
    let map = answer.as_object().ok_or_else(|| refuse("the judge answer must be an object"))?;
    for name in map.keys() {
        if !["schema_version", "evidence_id", "judgments"].contains(&name.as_str()) {
            return Err(refuse(format!("the judge answer has an unknown field `{name}`")));
        }
    }
    if answer["schema_version"].as_i64() != Some(SCHEMA_VERSION) {
        return Err(refuse("the judge answer requires schema_version 1"));
    }
    if answer["evidence_id"].as_str() != evidence["evidence_id"].as_str() {
        return Err(refuse(format!(
            "the judge answer belongs to evidence `{}`, not `{}`",
            ps::s(&answer["evidence_id"]),
            ps::s(&evidence["evidence_id"])
        )));
    }
    let rows = answer["judgments"]
        .as_array()
        .ok_or_else(|| refuse("the judge answer requires a judgments array"))?;
    if rows.len() > MAX_JUDGMENTS {
        return Err(refuse(format!("the judge answer carries {} judgments (limit {MAX_JUDGMENTS})", rows.len())));
    }
    let mut accepted: Vec<Value> = Vec::new();
    let mut seen: Vec<(String, String, String)> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let number = index + 1;
        let object = row.as_object().ok_or_else(|| refuse(format!("judgment {number} is not an object")))?;
        for name in object.keys() {
            if !["kind", "proposal", "candidate", "verdict", "reason", "evidence", "scope", "assumptions", "needs", "entry"].contains(&name.as_str()) {
                return Err(refuse(format!("judgment {number} has an unknown field `{name}`")));
            }
        }
        let kind = f(row, "kind");
        if !KINDS.contains(&kind.as_str()) {
            return Err(refuse(format!(
                "judgment {number} names the kind `{kind}`; it must be one of {}",
                KINDS.join(", ")
            )));
        }
        let proposal_id = f(row, "proposal");
        let Some(proposal) = ps::ps_items(&evidence["proposals"]).into_iter().find(|p| f(p, "id") == proposal_id) else {
            return Err(refuse(format!(
                "judgment {number} names proposal `{proposal_id}`, which the evidence does not hold"
            )));
        };
        let address = f(row, "candidate");
        let Some(candidate) = prepared_pair(&proposal, &kind, &address) else {
            return Err(refuse(format!(
                "judgment {number} names candidate `{address}`, which {proposal_id} did not prepare as a {kind}"
            )));
        };
        let verdict = f(row, "verdict");
        if !VERDICTS.contains(&verdict.as_str()) {
            return Err(refuse(format!(
                "judgment {number} names the verdict `{verdict}`; it must be one of {}",
                VERDICTS.join(", ")
            )));
        }
        let reason = line(&f(row, "reason"), MAX_LINE);
        if reason.is_empty() {
            return Err(refuse(format!("judgment {number} has no reason")));
        }
        let unread = pair_unread(evidence, &proposal, &kind, &address);
        if !unread.is_empty() {
            return Err(refuse(format!(
                "judgment {number} judges `{address}`, whose endpoint `{}` the review did not read",
                unread.join("`, `")
            )));
        }
        let allowed = pair_endpoints(evidence, &proposal, &kind, &address);
        let cited: Vec<String> = ps::ps_items(&row["evidence"])
            .iter()
            .map(ps::s)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        if cited.is_empty() {
            return Err(refuse(format!("judgment {number} has no evidence")));
        }
        for at in &cited {
            if !allowed.contains(at) {
                return Err(refuse(format!(
                    "judgment {number} cites `{at}`, which the review did not read for this pair"
                )));
            }
        }
        let scope = line(&f(row, "scope"), MAX_LINE);
        if verdict == "same" && scope.is_empty() {
            return Err(refuse(format!("judgment {number} says same without the scope that claims it")));
        }
        let needs: Vec<String> = ps::ps_items(&row["needs"])
            .iter()
            .map(|value| line(&ps::s(value), MAX_LINE))
            .filter(|value| !value.is_empty())
            .collect();
        if verdict == "uncertain" && needs.is_empty() {
            return Err(refuse(format!("judgment {number} says uncertain without naming what must be read")));
        }
        let entry = f(row, "entry");
        if !entry.is_empty() {
            let entries: Vec<String> = ps::ps_items(&candidate["entries"]).iter().map(|e| f(e, "address")).collect();
            if !entries.contains(&entry) {
                return Err(refuse(format!(
                    "judgment {number} names the entry `{entry}`, which is not a representative Card of `{address}`"
                )));
            }
        }
        let assumed: Vec<String> = ps::ps_items(&row["assumptions"])
            .iter()
            .map(|value| line(&ps::s(value), MAX_LINE))
            .filter(|value| !value.is_empty())
            .collect();
        let normalized = json!({
            "kind": kind,
            "proposal": proposal_id,
            "candidate": address,
            "verdict": verdict,
            "reason": reason,
            "scope": if scope.is_empty() { Value::Null } else { json!(scope) },
            "assumptions": assumed,
            "needs": needs,
            "entry": if entry.is_empty() { Value::Null } else { json!(entry) },
            "evidence": cited,
        });
        let pair = (proposal_id.clone(), kind.clone(), address.clone());
        if let Some(first) = seen.iter().position(|known| *known == pair) {
            if canon::canonical_json(&accepted[first]) != canon::canonical_json(&normalized) {
                return Err(refuse(format!(
                    "judgment {number} contradicts the earlier judgment {} for `{address}`",
                    first + 1
                )));
            }
            continue;
        }
        seen.push(pair);
        accepted.push(normalized);
    }
    let mut judged = json!({
        "schema_version": SCHEMA_VERSION,
        "created_at": canon::utc_now_o(),
        "evidence_id": evidence["evidence_id"],
        "judgments": accepted,
    });
    let id = format!("j-{}", canon::content_hash(&judged, &["judged_id", "created_at"]));
    judged["judged_id"] = json!(id);
    Ok(judged)
}

// --- report ---------------------------------------------------------------------------------------

/// The remedies the evidence supports, per kind and verdict (D077, D078, D079). A finding the report
/// leaves `uncertain` supports only `defer`: the evidence is not there yet.
fn remedies(verdict: &str, kind: &str, candidate: &str, entry: &str, book: &str) -> Vec<Value> {
    if verdict == "uncertain" {
        return vec![json!({ "action": "defer", "address": Value::Null, "description": "先读缺失的证据，再判断。" })];
    }
    match kind {
        "topic-concept" if verdict == "same" => vec![
            json!({ "action": "reuse_topic", "address": candidate, "entry": entry, "description": format!("把提议的概念并入 `{candidate}`，入口用 `{entry}`。") }),
            json!({ "action": "join_hub", "address": candidate, "entry": entry, "description": format!("两个入口都保留，用既有的 Hub 层把 `{candidate}` 连到本概念。") }),
            json!({ "action": "keep_distinct", "address": candidate, "entry": Value::Null, "description": format!("保留 `{candidate}` 与本概念，各自写清问题的范围。") }),
        ],
        "topic-concept" => vec![json!({
            "action": "keep_distinct",
            "address": candidate,
            "entry": Value::Null,
            "description": format!("`{candidate}` 与本概念的问题不同，各自保留。"),
        })],
        "restatement" if verdict == "same" => vec![
            json!({ "action": "keep_one_statement", "address": candidate, "entry": Value::Null, "description": format!("只保留一处陈述，另一处改成指向 `{candidate}` 的入口。") }),
            json!({ "action": "rewrite_as_interpretation", "address": candidate, "entry": Value::Null, "description": format!("把 `{candidate}` 改写成对同一陈述的解释。") }),
            json!({ "action": "retain_both", "address": candidate, "entry": Value::Null, "description": "两处都保留，各自写清范围与假设。" }),
        ],
        "restatement" => vec![json!({
            "action": "retain_both",
            "address": candidate,
            "entry": Value::Null,
            "description": format!("两处陈述的假设不同，保留两处并写清区别（`{candidate}`）。"),
        })],
        _ => vec![
            json!({ "action": "transfer_to_literature", "address": book, "entry": candidate, "description": format!("把 `{candidate}` 的陈述转成 `[[{book}]]` 下的文献陈述，原地址改成入口。") }),
            json!({ "action": "rewrite_standalone", "address": candidate, "entry": Value::Null, "description": format!("把 `{candidate}` 改写成不依赖《{book}》的独立陈述。") }),
        ],
    }
}

/// One finding, with its report-local id filled in after the whole list is built (D081).
#[allow(clippy::too_many_arguments)]
fn finding(
    kind: &str,
    proposal: &str,
    book: &str,
    candidate: &str,
    entry: &str,
    verdict: &str,
    reason: &str,
    scope: &Value,
    assumed: &Value,
    needs: &Value,
    cited: &Value,
    addresses: Vec<String>,
    formula: bool,
    unjudged: bool,
) -> Value {
    json!({
        "id": Value::Null,
        "kind": kind,
        "proposal": proposal,
        "candidate": candidate,
        "entry": if entry.is_empty() { Value::Null } else { json!(entry) },
        "verdict": verdict,
        "reason": reason,
        "scope": if scope.is_null() { Value::Null } else { scope.clone() },
        "assumptions": assumed.clone(),
        "needs": needs.clone(),
        "evidence": cited.clone(),
        "addresses": addresses,
        "formula_match": formula,
        "unjudged": unjudged,
        "remedies": remedies(verdict, kind, candidate, entry, book),
    })
}

fn report(p: &Params) -> Result<i32> {
    let evidence = frozen_evidence(&p.work)?;
    let judged = read_judged(&p.work, &evidence)?;
    let payload = report_payload(&evidence, judged.as_ref())?;
    let json_path = ps::join(&p.staging, "concept-review.json");
    let md_path = ps::join(&p.staging, "concept-review.md");
    let mut written = false;
    let existing = if ps::exists(&json_path) { common::read_json(&json_path).ok() } else { None };
    // D080: the report is immutable for one frozen evidence set and one set of answers. A second
    // report over them publishes the same id and changes no byte, and the Markdown is rendered from
    // the published report, not from a new run's clock. A file that no longer matches its own id is
    // republished instead of trusted.
    let intact = existing.filter(|old| {
        old["report_id"].as_str() == payload["report_id"].as_str()
            && format!("r-{}", canon::content_hash(old, &["report_id", "created_at"])) == payload["report_id"]
    });
    match intact {
        Some(published) => {
            let markdown = render_markdown(&published);
            if !ps::exists(&md_path) || util::read_text(&md_path)? != markdown {
                util::write_atomic(&md_path, &markdown)?;
                written = true;
            }
        }
        None => {
            canon::save_json(&json_path, &payload)?;
            util::write_atomic(&md_path, &render_markdown(&payload))?;
            written = true;
        }
    }
    let summary = json!({
        "ok": true,
        "step": "report",
        "report_id": payload["report_id"],
        "status": payload["status"],
        "complete": payload["completeness"]["complete"],
        "findings": ps::pipe(&payload["findings"]).len(),
        "judged": payload["completeness"]["judged"],
        "unjudged": payload["completeness"]["unjudged"],
        "unread": payload["completeness"]["unread"],
        "written": written,
        "paths": [json_path, md_path],
    });
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

/// The accepted judge answers, when `judge` wrote them. They are verified against the evidence:
/// answers for another evidence set are a stale file, not this report's judgments.
fn read_judged(work: &str, evidence: &Value) -> Result<Option<Value>> {
    let path = ps::join(work, "judged.json");
    if !ps::exists(&path) {
        return Ok(None);
    }
    let judged = common::read_json(&path)?;
    let id = format!("j-{}", canon::content_hash(&judged, &["judged_id", "created_at"]));
    if judged["judged_id"].as_str() != Some(id.as_str()) {
        return Err(refuse(format!(
            "the accepted judge answers at `{path}` do not match their id; run -Step judge again"
        )));
    }
    if judged["evidence_id"].as_str() != evidence["evidence_id"].as_str() {
        return Err(refuse(
            "the accepted judge answers belong to another frozen evidence set; run -Step judge again",
        ));
    }
    Ok(Some(judged))
}

/// D076–D082: one report over the frozen evidence and the accepted judgments. Nothing here reads
/// the archive, so report generation cannot call a Card write tool.
fn report_payload(evidence: &Value, judged: Option<&Value>) -> Result<Value> {
    let book = pstr(&evidence["book"], "citation_key");
    let judged_rows: Vec<Value> = judged.map(|value| ps::ps_items(&value["judgments"])).unwrap_or_default();
    let mut findings: Vec<Value> = Vec::new();
    let mut unjudged = 0usize;
    // D082: every read the frozen evidence missed is named here, whether or not the missing Card
    // became a candidate. A read that failed before the shortlist existed is still a missing read.
    let mut unread: Vec<String> = ps::ps_items(&evidence["unread"])
        .iter()
        .map(|row| f(row, "address"))
        .filter(|at| !at.is_empty())
        .collect();
    let mut unresolved: Vec<String> = Vec::new();
    let mut summaries: Vec<Value> = Vec::new();
    for proposal in ps::ps_items(&evidence["proposals"]) {
        let id = f(&proposal, "id");
        let mut same = 0usize;
        let mut different = 0usize;
        let mut uncertain = 0usize;
        let mut judged_here = 0usize;
        let mut topic_all_different = true;
        for candidate in ps::ps_items(&proposal["topic_candidates"]) {
            let address = f(&candidate, "address");
            let judgment = judged_rows
                .iter()
                .find(|row| f(row, "proposal") == id && f(row, "kind") == "topic-concept" && f(row, "candidate") == address);
            for at in pair_unread(evidence, &proposal, "topic-concept", &address) {
                if !unread.contains(&at) {
                    unread.push(at);
                }
            }
            let mut addresses: Vec<String> = vec![address.clone()];
            for entry in ps::ps_items(&candidate["entries"]) {
                addresses.push(f(&entry, "address"));
            }
            match judgment {
                Some(row) => {
                    judged_here += 1;
                    let verdict = f(row, "verdict");
                    match verdict.as_str() {
                        "same" => same += 1,
                        "different" => different += 1,
                        _ => uncertain += 1,
                    }
                    if verdict != "different" {
                        topic_all_different = false;
                    }
                    findings.push(finding(
                        "topic-concept",
                        &id,
                        &book,
                        &address,
                        &f(row, "entry"),
                        &verdict,
                        &f(row, "reason"),
                        &row["scope"],
                        &row["assumptions"],
                        &row["needs"],
                        &row["evidence"],
                        addresses.clone(),
                        false,
                        false,
                    ));
                }
                None => {
                    unjudged += 1;
                    topic_all_different = false;
                    findings.push(finding(
                        "topic-concept",
                        &id,
                        &book,
                        &address,
                        "",
                        "uncertain",
                        "没有评审：判据缺失，这一对仍是未决问题。",
                        &Value::Null,
                        &Value::Array(Vec::new()),
                        &json!([format!("`{address}` 的概念判断")]),
                        &json!([address]),
                        addresses.clone(),
                        false,
                        true,
                    ));
                    unresolved.push(format!("{id}: `{address}` 的 topic-concept 候选没有评审"));
                }
            }
            // D079: a Topic whose entries rest only on their own book is a filing finding.
            if !ps::truthy(&candidate["own_book"]) || ps::truthy(&candidate["other_books"]) {
                continue;
            }
            let verdict = judgment.map(|row| f(row, "verdict")).unwrap_or_else(|| "uncertain".to_string());
            let (reason, needs) = match verdict.as_str() {
                "same" => (
                    format!("`{address}` 的陈述只依赖本书《{book}》，这个主题的范围就是本书的模型。"),
                    Vec::<String>::new(),
                ),
                "different" => (
                    format!("`{address}` 的陈述只依赖本书《{book}》，它与本书的模型不同，但离开本书就没有前提。"),
                    Vec::<String>::new(),
                ),
                _ => (
                    "没有评审：这个主题的陈述只依赖本书，转成文献陈述还是改写成独立陈述仍未决。".to_string(),
                    vec![format!("`{address}` 的陈述与《{book}》的依赖")],
                ),
            };
            let uncertain_here = verdict != "same" && verdict != "different";
            findings.push(finding(
                "topic-home",
                &id,
                &book,
                &address,
                "",
                &verdict,
                &reason,
                &Value::Null,
                &Value::Array(Vec::new()),
                &json!(needs),
                &candidate["own_book"],
                addresses.clone(),
                false,
                uncertain_here,
            ));
            if uncertain_here {
                unresolved.push(format!("{id}: `{address}` 的 topic-home 判断没有评审"));
            }
        }
        for row in ps::ps_items(&proposal["restatement_candidates"]) {
            let address = f(&row, "address");
            let judgment = judged_rows
                .iter()
                .find(|found| f(found, "proposal") == id && f(found, "kind") == "restatement" && f(found, "candidate") == address);
            for at in pair_unread(evidence, &proposal, "restatement", &address) {
                if !unread.contains(&at) {
                    unread.push(at);
                }
            }
            let mut addresses = vec![address.clone()];
            for target in ps::ps_items(&row["sources"]).iter().map(ps::s) {
                if evidence["endpoints"].get(&target).is_some() && !addresses.contains(&target) {
                    addresses.push(target);
                }
            }
            match judgment {
                Some(found) => {
                    judged_here += 1;
                    let verdict = f(found, "verdict");
                    match verdict.as_str() {
                        "same" => same += 1,
                        "different" => different += 1,
                        _ => uncertain += 1,
                    }
                    let assumed = if ps::ps_items(&found["assumptions"]).is_empty() {
                        row["assumptions"].clone()
                    } else {
                        found["assumptions"].clone()
                    };
                    findings.push(finding(
                        "restatement",
                        &id,
                        &book,
                        &address,
                        "",
                        &verdict,
                        &f(found, "reason"),
                        &found["scope"],
                        &assumed,
                        &found["needs"],
                        &found["evidence"],
                        addresses,
                        ps::truthy(&row["formula_match"]),
                        false,
                    ));
                }
                None => {
                    unjudged += 1;
                    findings.push(finding(
                        "restatement",
                        &id,
                        &book,
                        &address,
                        "",
                        "uncertain",
                        "没有评审：判据缺失，这一对陈述仍是未决问题。",
                        &Value::Null,
                        &row["assumptions"],
                        &json!([format!("`{address}` 的陈述与假设")]),
                        &json!([address]),
                        addresses,
                        ps::truthy(&row["formula_match"]),
                        true,
                    ));
                    unresolved.push(format!("{id}: `{address}` 的 restatement 候选没有评审"));
                }
            }
        }
        let topics = ps::ps_items(&proposal["topic_candidates"]).len();
        summaries.push(json!({
            "id": id,
            "title": f(&proposal, "title"),
            "topic_candidates": topics,
            "restatement_candidates": ps::ps_items(&proposal["restatement_candidates"]).len(),
            "judged": judged_here,
            "same": same,
            "different": different,
            "uncertain": uncertain,
            "clears_shortlist": if topics == 0 || judged_here == 0 { Value::Null } else { json!(topic_all_different) },
        }));
    }
    // Report-local ids: the findings grouped by kind in D076's order, `c001` upward (D081).
    let rank = |kind: &str| match kind {
        "topic-concept" => 0,
        "restatement" => 1,
        _ => 2,
    };
    findings.sort_by_key(|row| rank(&f(row, "kind")));
    for (index, row) in findings.iter_mut().enumerate() {
        row["id"] = json!(format!("c{:03}", index + 1));
    }
    unread.sort_by(|a, b| canon::ordinal_cmp(a, b));
    unread.dedup();
    for at in &unread {
        unresolved.push(format!("`{at}` 没有读到：这一对端点不能进入干净结论"));
    }
    let table_read = ps::truthy(&evidence["term_table"]["read"]);
    let mut reasons: Vec<String> = Vec::new();
    if !table_read {
        reasons.push("term_table_missing".to_string());
        unresolved.push(format!("本书《{book}》没有术语表卡：命名证据缺失"));
    }
    if unjudged > 0 {
        reasons.push("unjudged_candidates".to_string());
    }
    if !unread.is_empty() {
        reasons.push("unread_endpoint".to_string());
    }
    let complete = reasons.is_empty();
    let mut payload = json!({
        "schema_version": SCHEMA_VERSION,
        "created_at": canon::utc_now_o(),
        "book": evidence["book"],
        "archive": evidence["archive"],
        "bounds": evidence["bounds"],
        "coverage": {
            "proposals": ps::ps_items(&evidence["proposals"]).len(),
            "pairs": evidence["coverage"]["pairs"],
            "topics_catalogued": evidence["coverage"]["topics_catalogued"],
            "ideas_catalogued": evidence["coverage"]["ideas_catalogued"],
            "prefiltered": evidence["coverage"]["prefiltered"],
            "topic_candidates": evidence["coverage"]["topic_candidates"],
            "topic_selected": evidence["coverage"]["topic_selected"],
            "topic_unselected": evidence["coverage"]["topic_unselected"],
            "restatement_candidates": evidence["coverage"]["restatement_candidates"],
            "restatement_selected": evidence["coverage"]["restatement_selected"],
            "restatement_unselected": evidence["coverage"]["restatement_unselected"],
        },
        "completeness": {
            "complete": complete,
            "judged": judged_rows.len(),
            "unjudged": unjudged,
            "unread": unread,
            "reasons": reasons,
        },
        "endpoints": evidence["endpoints"],
        "evidence_path": evidence["evidence_id"],
        "term_table": {
            "address": evidence["term_table"]["address"],
            "rows": ps::pipe(&evidence["term_table"]["rows"]).len(),
        },
        "proposals": summaries,
        "findings": findings,
        "unresolved": unresolved,
        "status": if complete { "complete" } else { "partial" },
    });
    // The judgments are part of what the report id binds: new answers are a new review.
    payload["judged_id"] = judged.map(|value| value["judged_id"].clone()).unwrap_or(Value::Null);
    payload["judgments"] = judged.map(|value| value["judgments"].clone()).unwrap_or(json!([]));
    let id = format!("r-{}", canon::content_hash(&payload, &["report_id", "created_at"]));
    payload["report_id"] = json!(id);
    Ok(payload)
}

/// `concept-review.md`: the report as prose. It repeats the ids, the bounds, and the completeness so
/// a reader can see that a shortlist is a bound, not an exhaustive comparison.
fn render_markdown(report: &Value) -> String {
    let book = pstr(&report["book"], "citation_key");
    let title = pstr(&report["book"], "title");
    let mut sb = Sb::new();
    sb.line(format!("# 概念复核报告：{title}（`[[{book}]]`）")).blank();
    sb.line(format!("Report: `{}`", f(report, "report_id")));
    sb.line(format!("Generated: {}", f(report, "created_at")));
    sb.line(format!(
        "Archive: `{}` at revision {}. Status: **{}**.",
        f(&report["archive"], "id"),
        f(&report["archive"], "revision"),
        f(report, "status")
    ))
    .blank();
    sb.line("## Bounds").blank();
    sb.line(format!(
        "- Topic candidates per proposal: at most {}; {} selected of {} candidates, {} unselected.",
        f(&report["bounds"], "topic_candidates"),
        f(&report["coverage"], "topic_selected"),
        f(&report["coverage"], "topic_candidates"),
        f(&report["coverage"], "topic_unselected")
    ));
    sb.line(format!(
        "- Restatement candidates per proposal: at most {}; {} selected of {} candidates, {} unselected.",
        f(&report["bounds"], "restatement_candidates"),
        f(&report["coverage"], "restatement_selected"),
        f(&report["coverage"], "restatement_candidates"),
        f(&report["coverage"], "restatement_unselected")
    ));
    sb.line(format!(
        "- Catalogued {} agent Topics and {} Idea Cards; at most {} ideas read in full per proposal. A shortlist is not exhaustive semantic uniqueness.",
        f(&report["coverage"], "topics_catalogued"),
        f(&report["coverage"], "ideas_catalogued"),
        f(&report["bounds"], "prefilter")
    ))
    .blank();
    sb.line("## Completeness").blank();
    sb.line(format!(
        "- Judged: {}. Unjudged candidates: {}. Unread endpoints: {}.",
        f(&report["completeness"], "judged"),
        f(&report["completeness"], "unjudged"),
        ps::pipe(&report["completeness"]["unread"]).len()
    ));
    let reasons: Vec<String> = ps::ps_items(&report["completeness"]["reasons"]).iter().map(ps::s).collect();
    sb.line(format!(
        "- {}",
        if reasons.is_empty() {
            "No missing read and no unjudged candidate.".to_string()
        } else {
            format!("Missing: {}.", reasons.join(", "))
        }
    ))
    .blank();
    let unresolved: Vec<String> = ps::ps_items(&report["unresolved"]).iter().map(ps::s).collect();
    if !unresolved.is_empty() {
        sb.line("## Unresolved").blank();
        for item in &unresolved {
            sb.line(format!("- {item}"));
        }
        sb.blank();
    }
    for (kind, heading) in [("topic-concept", "Topic concepts"), ("restatement", "Restatements"), ("topic-home", "Topic homes")] {
        let rows: Vec<Value> = ps::ps_items(&report["findings"]).into_iter().filter(|row| f(row, "kind") == kind).collect();
        sb.line(format!("## {heading} ({})", rows.len())).blank();
        if rows.is_empty() {
            sb.line("None.").blank();
            continue;
        }
        sb.line("| finding | proposal | verdict | candidate | addresses | reason |");
        sb.line("| --- | --- | --- | --- | --- | --- |");
        for row in &rows {
            let addresses: Vec<String> = ps::ps_items(&row["addresses"]).iter().map(ps::s).collect();
            sb.line(format!(
                "| `{}` | {} | {} | `{}` | {} | {} |",
                f(row, "id"),
                f(row, "proposal"),
                f(row, "verdict"),
                f(row, "candidate"),
                cell(&addresses.join(", ")),
                cell(&f(row, "reason"))
            ));
        }
        sb.blank();
    }
    sb.line("## Findings").blank();
    for row in ps::ps_items(&report["findings"]) {
        sb.line(format!("### `{}` {} — {}", f(&row, "id"), f(&row, "kind"), f(&row, "verdict")));
        sb.line(format!("- Proposal: {}. Candidate: `{}`.", f(&row, "proposal"), f(&row, "candidate")));
        let addresses: Vec<String> = ps::ps_items(&row["addresses"]).iter().map(|a| format!("`{}`", ps::s(a))).collect();
        sb.line(format!("- Affected addresses: {}.", addresses.join(", ")));
        if let Some(scope) = row["scope"].as_str() {
            sb.line(format!("- Scope: {scope}."));
        }
        let assumed: Vec<String> = ps::ps_items(&row["assumptions"]).iter().map(ps::s).collect();
        if !assumed.is_empty() {
            sb.line(format!("- Assumptions: {}.", assumed.join("; ")));
        }
        if ps::truthy(&row["formula_match"]) {
            sb.line("- Candidate signal: a shared formula. That is not a verdict.");
        }
        let needs: Vec<String> = ps::ps_items(&row["needs"]).iter().map(ps::s).collect();
        if !needs.is_empty() {
            sb.line(format!("- Must be read: {}.", needs.join("; ")));
        }
        sb.line(format!("- Reason: {}", f(&row, "reason")));
        for remedy in ps::ps_items(&row["remedies"]) {
            sb.line(format!("- Remedy `{}`: {}", f(&remedy, "action"), f(&remedy, "description")));
        }
        sb.blank();
    }
    sb.0.trim_end().to_string() + NL
}

fn cell(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('|', "\\|").replace(['\r', '\n'], " ")
}

// --- decision -------------------------------------------------------------------------------------

fn decision(p: &Params) -> Result<i32> {
    if p.file.is_empty() {
        return Err(refuse("concepts decision needs -File, the reviewed decision file"));
    }
    let report_path = ps::join(&p.staging, "concept-review.json");
    if !ps::exists(&report_path) {
        return Err(refuse(format!("no concept review at `{report_path}`; run -Step report first")));
    }
    let report = common::read_json(&report_path)?;
    let id = format!("r-{}", canon::content_hash(&report, &["report_id", "created_at"]));
    if report["report_id"].as_str() != Some(id.as_str()) {
        return Err(refuse(format!(
            "the report at `{report_path}` does not match its report id; regenerate it"
        )));
    }
    let asked = canon::read_json(&p.file)?;
    let accepted = validate_decision(&report, &asked)?;
    let path = ps::join(&p.work, "decision.json");
    canon::save_json(&path, &accepted)?;
    let summary = json!({
        "ok": true,
        "step": "decision",
        "report_id": report["report_id"],
        "decision_id": accepted["decision_id"],
        "accepted": ps::pipe(&accepted["decisions"]).len(),
        "file": path,
    });
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

/// §11.3: an accepted decision binds the exact report id, the finding ids it names, and the authored
/// fingerprints it reviewed. A decision that names another report, an unknown finding, a verdict or
/// remedy the report does not support, or a fingerprint the review did not see is refused; nothing
/// is written and no Card is touched.
fn validate_decision(report: &Value, asked: &Value) -> Result<Value> {
    let map = asked.as_object().ok_or_else(|| refuse("the decision must be an object"))?;
    for name in map.keys() {
        if !["schema_version", "report_id", "decisions"].contains(&name.as_str()) {
            return Err(refuse(format!("the decision has an unknown field `{name}`")));
        }
    }
    if asked["schema_version"].as_i64() != Some(SCHEMA_VERSION) {
        return Err(refuse("the decision requires schema_version 1"));
    }
    if asked["report_id"].as_str().is_none_or(str::is_empty) {
        return Err(refuse("the decision names no report id"));
    }
    if asked["report_id"].as_str() != report["report_id"].as_str() {
        return Err(refuse(format!(
            "the decision belongs to report `{}`, not `{}`; review the committed report",
            ps::s(&asked["report_id"]),
            ps::s(&report["report_id"])
        )));
    }
    let rows = asked["decisions"]
        .as_array()
        .ok_or_else(|| refuse("the decision requires a decisions array"))?;
    if rows.is_empty() || rows.len() > MAX_DECISIONS {
        return Err(refuse(format!("the decision needs 1..{MAX_DECISIONS} decisions, not {}", rows.len())));
    }
    let findings = ps::ps_items(&report["findings"]);
    let mut accepted: Vec<Value> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let number = index + 1;
        let object = row.as_object().ok_or_else(|| refuse(format!("decision {number} is not an object")))?;
        for name in object.keys() {
            if !["finding", "action", "verdict", "note", "fingerprints"].contains(&name.as_str()) {
                return Err(refuse(format!("decision {number} has an unknown field `{name}`")));
            }
        }
        let finding_id = f(row, "finding");
        let Some(found) = findings.iter().find(|known| f(known, "id") == finding_id) else {
            return Err(refuse(format!(
                "decision {number} names the finding `{finding_id}`, which the report does not hold"
            )));
        };
        if seen.contains(&finding_id) {
            return Err(refuse(format!("decision {number} names the finding `{finding_id}` twice")));
        }
        seen.push(finding_id.clone());
        let verdict = f(row, "verdict");
        if verdict != f(found, "verdict") {
            return Err(refuse(format!(
                "the report does not support verdict `{verdict}` for `{finding_id}`; it says `{}`",
                f(found, "verdict")
            )));
        }
        let action = f(row, "action");
        let Some(remedy) = ps::ps_items(&found["remedies"]).into_iter().find(|remedy| f(remedy, "action") == action) else {
            return Err(refuse(format!("finding `{finding_id}` does not offer the action `{action}`")));
        };
        let note = line(&f(row, "note"), MAX_LINE);
        if note.is_empty() {
            return Err(refuse(format!("decision {number} has no note")));
        }
        let mut fingerprints = Map::new();
        if let Some(map) = row["fingerprints"].as_object() {
            for (at, value) in map {
                let wanted = ps::s(value);
                let known = report["endpoints"].as_object().and_then(|endpoints| endpoints.get(at));
                let Some(known) = known else {
                    return Err(refuse(format!(
                        "decision {number} names the fingerprint of `{at}`, which the review did not read"
                    )));
                };
                if f(known, "fingerprint") != wanted {
                    return Err(refuse(format!(
                        "decision {number} names fingerprint `{wanted}` for `{at}`, but the reviewed endpoint is `{}`",
                        f(known, "fingerprint")
                    )));
                }
                fingerprints.insert(at.clone(), json!(wanted));
            }
        }
        accepted.push(json!({
            "finding": finding_id,
            "kind": found["kind"],
            "proposal": found["proposal"],
            "candidate": found["candidate"],
            "verdict": verdict,
            "action": action,
            "address": remedy["address"],
            "addresses": found["addresses"],
            "note": note,
            "fingerprints": Value::Object(fingerprints),
        }));
    }
    let mut payload = json!({
        "schema_version": SCHEMA_VERSION,
        "created_at": canon::utc_now_o(),
        "report_id": report["report_id"],
        "evidence_id": report["evidence_path"],
        "book": report["book"],
        "archive": report["archive"],
        "decisions": accepted,
    });
    let id = format!("d-{}", canon::content_hash(&payload, &["decision_id", "created_at"]));
    payload["decision_id"] = json!(id);
    Ok(payload)
}

// --- tests ----------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence() -> Value {
        json!({
            "schema_version": 1,
            "evidence_id": "e-1",
            "book": { "citation_key": "K", "title": "书" },
            "archive": { "id": "0123456789abcdef0123456789abcdef", "path": "D:\\a", "revision": 7 },
            "bounds": { "topic_candidates": 10, "restatement_candidates": 10, "prefilter": 40 },
            "term_table": { "address": "K/1|a", "read": true, "rows": [] },
            "coverage": { "topics_catalogued": 2, "ideas_catalogued": 1, "prefiltered": 1, "topic_candidates": 2, "topic_selected": 2, "topic_unselected": 0, "restatement_candidates": 1, "restatement_selected": 1, "restatement_unselected": 0, "pairs": 3 },
            "endpoints": {
                "T/0": { "address": "T/0", "title": "T", "fingerprint": "aa", "path": "p", "sha256": "s" },
                "T/1": { "address": "T/1", "title": "e", "fingerprint": "bb", "path": "p", "sha256": "s" },
                "K/1|a": { "address": "K/1|a", "title": "术语表", "fingerprint": "dd", "path": "p", "sha256": "s" },
                "K/4": { "address": "K/4", "title": "s", "fingerprint": "ee", "path": "p", "sha256": "s" }
            },
            "unread": [],
            "proposals": [{
                "id": "p001", "title": "均衡的多重性", "question": "什么条件下有多个均衡？",
                "statement": "策略互补时可能有多个均衡。", "assumptions": ["策略互补"], "formulas": [], "sources": [],
                "terms": [], "term": Value::Null,
                "topic_candidates": [{ "address": "T/0", "title": "T", "question": "q", "similarity": "0.5000", "rank": 1,
                    "entries": [{ "address": "T/1", "title": "e", "statement": "s", "sources": ["K"] }],
                    "own_book": ["K"], "other_books": [], "unread": [] }],
                "restatement_candidates": [{ "address": "K/4", "title": "s", "statement": "s", "assumptions": ["x"], "formulas": [], "sources": [], "formula_match": false, "similarity": "0.4000", "rank": 1, "unread": [] }]
            }]
        })
    }

    fn answer(evidence: &Value, judgments: Value) -> Value {
        json!({ "schema_version": 1, "evidence_id": evidence["evidence_id"], "judgments": judgments })
    }

    #[test]
    fn bigrams_ignore_formulas_and_links_and_dice_is_symmetric() {
        let a = bigrams("策略互补时可能有多个均衡 $x=1$ [[K/1]]");
        let b = bigrams("策略互补时可能有多个均衡");
        assert_eq!(a, b);
        assert_eq!(similarity(&a, &b), 1.0);
        assert_eq!(similarity(&a, &bigrams("债务契约")), 0.0);
        assert_eq!(similarity(&bigrams(""), &b), 0.0);
    }

    #[test]
    fn a_long_endpoint_body_is_retained_within_its_bound() {
        let work = std::env::temp_dir().join(format!("zt-concepts-retained-{}", std::process::id()));
        std::fs::create_dir_all(work.join("endpoints")).unwrap();
        let body = "均衡的陈述。".repeat(1_200);
        assert!(ps::len16(&body) > ENDPOINT_BODY);
        let card = json!({ "address": "K/4", "title": "长卡", "body": body });
        let (path, sha256) = retain_endpoint(&work.to_string_lossy(), &card).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(canon::sha256_hex(text.as_bytes()), sha256);
        let retained = text.split("\n\n").nth(1).unwrap().trim_end_matches('\n');
        assert!(ps::len16(retained) <= ENDPOINT_BODY + 1, "the retained body is cut to its bound: {}", ps::len16(retained));
        assert!(retained.ends_with('…'), "a cut body is marked as cut");
        assert!(retained.starts_with("均衡的陈述。"));
        std::fs::remove_dir_all(&work).unwrap();
    }

    #[test]
    fn a_statement_keeps_the_claim_not_the_labels() {
        let body = "对称参与者的重复博弈有两个均衡。\n\n假设：参与者对称。\n\n来源：[[K]] §2.1\n\n第二段不是陈述。";
        assert_eq!(statement(body), "对称参与者的重复博弈有两个均衡。");
        assert_eq!(assumptions(body), vec!["参与者对称。"]);
        assert_eq!(sources(body), vec!["K".to_string()]);
        assert_eq!(sources("没有来源行 [[K/1]]"), Vec::<String>::new());
        assert_eq!(formulas("$a=b$ 与 $c=d$ 和 $a=b$"), vec!["$a=b$".to_string(), "$c=d$".to_string()]);
        assert_eq!(statement("| 表 |\n\n> 引文\n\n# 标题"), "");
        assert!(in_tree("K/2", "K") && in_tree("K", "K") && !in_tree("K2/1", "K"));
        let mut pairs: Vec<(String, String)> = Vec::new();
        push_pair(&mut pairs, "p001", "K/4");
        push_pair(&mut pairs, "p001", "K/4");
        push_pair(&mut pairs, "p002", "K/4");
        assert_eq!(pairs, vec![("p001".to_string(), "K/4".to_string()), ("p002".to_string(), "K/4".to_string())]);
    }

    #[test]
    fn a_valid_answer_is_accepted_and_every_malformed_one_is_refused() {
        let evidence = evidence();
        let ok = validate_answer(
            &evidence,
            &answer(&evidence, json!([
                { "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "same", "scope": "策略互补时", "reason": "同一个问题。", "evidence": ["T/0", "T/1"], "entry": "T/1" },
                { "kind": "restatement", "proposal": "p001", "candidate": "K/4", "verdict": "different", "reason": "假设不同。", "evidence": ["K/4"] }
            ])),
        )
        .unwrap();
        assert_eq!(ok["judgments"].as_array().unwrap().len(), 2);
        assert!(ok["judged_id"].as_str().unwrap().starts_with("j-"));
        for (fault, judgments) in [
            ("names the kind", json!([{ "kind": "topic-home", "proposal": "p001", "candidate": "T/0", "verdict": "same", "scope": "x", "reason": "r", "evidence": ["T/0"] }])),
            ("names the verdict", json!([{ "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "equivalent", "reason": "r", "evidence": ["T/0"] }])),
            ("does not hold", json!([{ "kind": "topic-concept", "proposal": "p009", "candidate": "T/0", "verdict": "same", "scope": "x", "reason": "r", "evidence": ["T/0"] }])),
            ("did not prepare", json!([{ "kind": "topic-concept", "proposal": "p001", "candidate": "K/4", "verdict": "same", "scope": "x", "reason": "r", "evidence": ["K/4"] }])),
            ("no evidence", json!([{ "kind": "restatement", "proposal": "p001", "candidate": "K/4", "verdict": "different", "reason": "r", "evidence": [] }])),
            ("did not read for this pair", json!([{ "kind": "restatement", "proposal": "p001", "candidate": "K/4", "verdict": "different", "reason": "r", "evidence": ["K/9"] }])),
            ("without the scope", json!([{ "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "same", "reason": "r", "evidence": ["T/0"] }])),
            ("must be read", json!([{ "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "uncertain", "reason": "r", "evidence": ["T/0"] }])),
            ("is not a representative Card", json!([{ "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "same", "scope": "x", "reason": "r", "evidence": ["T/0"], "entry": "K/1|a" }])),
            ("contradicts", json!([
                { "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "same", "scope": "x", "reason": "r", "evidence": ["T/0"] },
                { "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "different", "reason": "r", "evidence": ["T/0"] }
            ])),
            ("unknown field", json!([{ "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "same", "scope": "x", "reason": "r", "evidence": ["T/0"], "extra": 1 }])),
        ] {
            let error = validate_answer(&evidence, &answer(&evidence, judgments)).unwrap_err().to_string();
            assert!(error.contains(fault), "{fault}: {error}");
        }
        let wrong = json!({ "schema_version": 1, "evidence_id": "e-2", "judgments": [] });
        assert!(validate_answer(&evidence, &wrong).unwrap_err().to_string().contains("belongs to evidence"));
        // An identical duplicate is one judgment, never two.
        let once = json!([{ "kind": "restatement", "proposal": "p001", "candidate": "K/4", "verdict": "different", "reason": "r", "evidence": ["K/4"] }]);
        let twice = json!([once[0].clone(), once[0].clone()]);
        assert_eq!(validate_answer(&evidence, &answer(&evidence, twice)).unwrap()["judgments"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn an_unread_endpoint_cannot_be_judged() {
        let mut evidence = evidence();
        evidence["unread"] = json!([{ "address": "T/1", "why": "the Card read did not return it" }]);
        evidence["proposals"][0]["topic_candidates"][0]["unread"] = json!(["T/1"]);
        let error = validate_answer(
            &evidence,
            &answer(&evidence, json!([{ "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "same", "scope": "x", "reason": "r", "evidence": ["T/0"] }])),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("did not read"), "{error}");
    }

    #[test]
    fn a_complete_report_ids_its_findings_and_keeps_the_kinds_apart() {
        let evidence = evidence();
        let judged = validate_answer(
            &evidence,
            &answer(&evidence, json!([
                { "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "same", "scope": "策略互补时", "reason": "同一个问题。", "evidence": ["T/0", "T/1"], "entry": "T/1" },
                { "kind": "restatement", "proposal": "p001", "candidate": "K/4", "verdict": "different", "reason": "假设不同。", "evidence": ["K/4"] }
            ])),
        )
        .unwrap();
        let report = report_payload(&evidence, Some(&judged)).unwrap();
        assert_eq!(report["status"], "complete");
        assert_eq!(report["completeness"]["complete"], true);
        assert_eq!(report["completeness"]["unjudged"], 0);
        assert!(report["report_id"].as_str().unwrap().starts_with("r-"));
        let findings = ps::ps_items(&report["findings"]);
        let ids: Vec<String> = findings.iter().map(|row| f(row, "id")).collect();
        assert_eq!(ids, ["c001", "c002", "c003"]);
        assert_eq!(f(&findings[0], "kind"), "topic-concept");
        assert_eq!(f(&findings[1], "kind"), "restatement");
        assert_eq!(f(&findings[2], "kind"), "topic-home");
        assert_eq!(findings[0]["scope"], "策略互补时");
        assert!(ps::ps_items(&findings[0]["remedies"]).iter().any(|row| f(row, "action") == "reuse_topic"));
        assert!(ps::ps_items(&findings[1]["remedies"]).iter().all(|row| f(row, "action") == "retain_both"));
        // The same evidence and answers give the same id, without the created stamp.
        let again = report_payload(&evidence, Some(&judged)).unwrap();
        assert_eq!(report["report_id"], again["report_id"]);
        assert!(render_markdown(&report).contains("`c001`"));
    }

    #[test]
    fn a_report_without_its_judgments_is_partial_and_says_why() {
        let evidence = evidence();
        let report = report_payload(&evidence, None).unwrap();
        assert_eq!(report["status"], "partial");
        assert_eq!(report["completeness"]["complete"], false);
        assert_eq!(report["completeness"]["unjudged"], 2);
        assert!(ps::ps_items(&report["completeness"]["reasons"]).iter().any(|row| ps::s(row) == "unjudged_candidates"));
        assert!(!ps::ps_items(&report["unresolved"]).is_empty());
        for row in ps::ps_items(&report["findings"]) {
            assert_eq!(f(&row, "verdict"), "uncertain");
            assert_eq!(row["unjudged"], true);
            assert!(!ps::ps_items(&row["needs"]).is_empty());
            assert!(ps::ps_items(&row["remedies"]).iter().all(|remedy| f(remedy, "action") == "defer"));
        }
        // A missing term table is named, never silently zero.
        let mut without = evidence.clone();
        without["term_table"]["read"] = json!(false);
        without["term_table"]["address"] = Value::Null;
        let report = report_payload(&without, None).unwrap();
        assert!(ps::ps_items(&report["completeness"]["reasons"]).iter().any(|row| ps::s(row) == "term_table_missing"));
        assert_eq!(report["status"], "partial");
    }

    #[test]
    fn a_card_the_review_could_not_read_keeps_the_report_partial() {
        let mut evidence = evidence();
        // A Card the catalogue named but the read did not return: it never became a candidate, so
        // no pair names it, yet the frozen evidence says the read is missing (D082).
        evidence["unread"] = json!([{ "address": "K/9", "why": "the Card read did not return it" }]);
        let judged = validate_answer(
            &evidence,
            &answer(&evidence, json!([
                { "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "same", "scope": "s", "reason": "r", "evidence": ["T/0"], "entry": "T/1" },
                { "kind": "restatement", "proposal": "p001", "candidate": "K/4", "verdict": "different", "reason": "r", "evidence": ["K/4"] }
            ])),
        )
        .unwrap();
        let report = report_payload(&evidence, Some(&judged)).unwrap();
        let unread: Vec<String> = ps::ps_items(&report["completeness"]["unread"]).iter().map(ps::s).collect();
        assert_eq!(unread, ["K/9"], "the report names every read the frozen evidence missed");
        assert_eq!(report["completeness"]["complete"], false);
        assert_eq!(report["status"], "partial");
        assert!(ps::ps_items(&report["completeness"]["reasons"]).iter().any(|row| ps::s(row) == "unread_endpoint"));
    }

    #[test]
    fn a_decision_binds_the_report_id_and_the_remedies_it_names() {
        let evidence = evidence();
        let judged = validate_answer(
            &evidence,
            &answer(&evidence, json!([
                { "kind": "topic-concept", "proposal": "p001", "candidate": "T/0", "verdict": "same", "scope": "s", "reason": "r", "evidence": ["T/0"], "entry": "T/1" },
                { "kind": "restatement", "proposal": "p001", "candidate": "K/4", "verdict": "different", "reason": "r", "evidence": ["K/4"] }
            ])),
        )
        .unwrap();
        let report = report_payload(&evidence, Some(&judged)).unwrap();
        let id = report["report_id"].as_str().unwrap();
        let asked = json!({ "schema_version": 1, "report_id": id, "decisions": [
            { "finding": "c001", "action": "reuse_topic", "verdict": "same", "note": "复用。", "fingerprints": { "T/0": "aa" } }
        ] });
        let accepted = validate_decision(&report, &asked).unwrap();
        assert!(accepted["decision_id"].as_str().unwrap().starts_with("d-"));
        assert_eq!(accepted["report_id"], id);
        for (fault, wrong) in [
            ("belongs to report", json!({ "schema_version": 1, "report_id": "r-other", "decisions": [{ "finding": "c001", "action": "reuse_topic", "verdict": "same", "note": "x" }] })),
            ("does not hold", json!({ "schema_version": 1, "report_id": id, "decisions": [{ "finding": "c999", "action": "reuse_topic", "verdict": "same", "note": "x" }] })),
            ("does not support verdict", json!({ "schema_version": 1, "report_id": id, "decisions": [{ "finding": "c001", "action": "keep_distinct", "verdict": "different", "note": "x" }] })),
            ("does not offer the action", json!({ "schema_version": 1, "report_id": id, "decisions": [{ "finding": "c002", "action": "reuse_topic", "verdict": "different", "note": "x" }] })),
            ("names fingerprint", json!({ "schema_version": 1, "report_id": id, "decisions": [{ "finding": "c001", "action": "reuse_topic", "verdict": "same", "note": "x", "fingerprints": { "T/0": "zz" } }] })),
            ("which the review did not read", json!({ "schema_version": 1, "report_id": id, "decisions": [{ "finding": "c001", "action": "reuse_topic", "verdict": "same", "note": "x", "fingerprints": { "T/9": "aa" } }] })),
            ("twice", json!({ "schema_version": 1, "report_id": id, "decisions": [
                { "finding": "c001", "action": "reuse_topic", "verdict": "same", "note": "x" },
                { "finding": "c001", "action": "keep_distinct", "verdict": "same", "note": "y" }
            ] })),
            ("no note", json!({ "schema_version": 1, "report_id": id, "decisions": [{ "finding": "c001", "action": "reuse_topic", "verdict": "same", "note": "" }] })),
        ] {
            let error = validate_decision(&report, &wrong).unwrap_err().to_string();
            assert!(error.contains(fault), "{fault}: {error}");
        }
        // An uncertain finding supports only the deferral.
        let partial = report_payload(&evidence, None).unwrap();
        let partial_id = partial["report_id"].as_str().unwrap();
        let reuse = json!({ "schema_version": 1, "report_id": partial_id, "decisions": [{ "finding": "c001", "action": "reuse_topic", "verdict": "uncertain", "note": "x" }] });
        assert!(validate_decision(&partial, &reuse).is_err());
        let defer = json!({ "schema_version": 1, "report_id": partial_id, "decisions": [{ "finding": "c001", "action": "defer", "verdict": "uncertain", "note": "先读。" }] });
        assert!(validate_decision(&partial, &defer).is_ok());
    }

    #[test]
    fn the_bounds_are_the_named_constants() {
        assert_eq!(TOPIC_CANDIDATES, 10);
        assert_eq!(RESTATEMENT_CANDIDATES, 10);
        assert!(canon::integer_option("10", "MaxTopics", 1, TOPIC_CANDIDATES).is_ok());
        assert!(canon::integer_option("11", "MaxTopics", 1, TOPIC_CANDIDATES).is_err());
    }
}
