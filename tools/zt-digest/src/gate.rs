//! S07 (#100): the concept-meaning Topic gate (design §7.1, §7.2).
//!
//! The existing exact/near-title refusal stays the fast first check. Beyond it, this gate compares a
//! proposed Topic's **question** with the questions and titles of every agent Topic, the accepted
//! register terms and the book's term rows, shortlists at most ten plausible Topics, and reads
//! representative Cards for that shortlist. An empty shortlist with complete required inputs clears
//! creation deterministically. A non-empty shortlist needs a judge decision (`same`, `different`,
//! `uncertain`); creation then revalidates that decision under the run's own Topic gate file
//! and writes through the receipt-owned identified path.
//!
//! The lock is never held while judging: `prepare` and `decide` run without it, and only `create`
//! takes it, re-reads the concept inputs and releases it after the receipt is known. The guarantee is
//! bounded: the lock coordinates managed digest processes, and a raw Owner or GUI Topic creation is
//! still possible — a competing Topic it made is reported.

use crate::calls::{self, Gate};
use crate::card::relatedness::Relatedness;
use crate::canon;
use crate::ps::s;
use crate::util;
use crate::zt::Zt;
use crate::{args::Args, snapshot};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The shortlist never exceeds this many Topics (D140).
pub(crate) const SHORTLIST: usize = 10;
/// Representative Regular Cards read in full per shortlisted Topic.
const REPRESENTATIVE: usize = 3;
/// A Topic question joins the comparison up to this many characters.
const QUESTION_CHARS: usize = 400;
/// The least cosine score a Topic needs to count as a plausible match.
const MIN_MATCH: f64 = 0.15;
/// A candidate must also stay within this share of the best match, so a large box keeps its head.
const MATCH_FLOOR: f64 = 0.35;
/// The near-title threshold the helper already used (`calls::Options::new`).
const TITLE_SIMILARITY: f64 = 0.8;
/// The evidence, decision and report schema.
const SCHEMA: i64 = 1;
/// Weighted terms kept per document, as the other relatedness callers do.
const KEEP: usize = 150;
/// The verified verdicts the judge may return (D143).
pub(crate) const VERDICTS: [&str; 3] = ["same", "different", "uncertain"];
/// The lock file name inside the gate folder.
const DEFAULT_LOCK: &str = "topic-gate.lock";

fn gate_dir(staging: &str) -> PathBuf {
    Path::new(staging).join("gate")
}

/// The run's own Topic gate file: `prepare` writes it into every group's write command
/// (`-TopicGate <staging>\topic-gate.lock`) and `ledger` group-apply guards with it, so the managed
/// gate and the run's managed writers wait on one file (D148, D154). It is not a second lock beside
/// the run's own: two files would let a managed writer and this gate create the same Topic twice.
fn run_gate(staging: &str) -> PathBuf {
    Path::new(staging).join(DEFAULT_LOCK)
}

pub(crate) fn evidence_path(staging: &str) -> PathBuf {
    gate_dir(staging).join("evidence.json")
}

pub(crate) fn decision_path(staging: &str) -> PathBuf {
    gate_dir(staging).join("decision.json")
}

fn prepare_path(staging: &str) -> PathBuf {
    gate_dir(staging).join("prepare.json")
}

fn result_path(staging: &str) -> PathBuf {
    gate_dir(staging).join("create.json")
}

fn report_path(staging: &str) -> PathBuf {
    gate_dir(staging).join("gate-report.json")
}

fn report_markdown_path(staging: &str) -> PathBuf {
    gate_dir(staging).join("gate-report.md")
}

/// One term row of the book term table: its accepted word, the words not to use, and its meaning.
#[derive(Clone, Debug)]
struct TermRow {
    term: String,
    avoid: Vec<String>,
    meaning: String,
}

/// The concept inputs a decision binds: the agent Topic titles and questions, the accepted register
/// terms, the book's term rows, and the proposed title and question. Nothing else participates, so
/// an unrelated Card edit does not stale a valid decision (D150).
struct Inputs {
    /// Every agent Topic with its question.
    topics: Vec<Value>,
    /// The complete title catalogue, for the Regular Cards of a candidate's tree.
    trees: Vec<Value>,
    register: Value,
    terms: Value,
    rows: Vec<TermRow>,
}

/// The book's term table at `<staging>/terms.md`: one `|`-separated row per term. A run with the
/// terms layer disabled has none, and the gate reports that rather than inventing meanings.
fn read_terms(staging: &str) -> (Value, Vec<TermRow>) {
    let path = Path::new(staging).join("terms.md");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return (
            json!({"status":"absent","path":path.to_string_lossy()}),
            Vec::new(),
        );
    };
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with('|') || line.starts_with("|-") || line.starts_with("| -") {
            continue;
        }
        let cells: Vec<String> = line
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect();
        if cells.len() < 3 || cells[0].is_empty() || cells[0] == "术语" {
            continue;
        }
        let avoid = cells[1]
            .split(['、', ',', '，', ' '])
            .map(str::trim)
            .filter(|word| !word.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        rows.push(TermRow {
            term: cells[0].clone(),
            avoid,
            meaning: cells[2].clone(),
        });
    }
    (
        json!({"status":"read","path":path.to_string_lossy(),"rows":rows.len()}),
        rows,
    )
}

/// The accepted register entries of the agent Workspace, as term labels. A missing or unreadable
/// register is reported, never substituted (D089).
fn read_register(zt: &mut Zt) -> Value {
    let answer = match zt.call("get_register", json!({"workspace":"agent"})) {
        Ok(answer) => answer,
        Err(error) => return json!({"status":"failed","fault":format!("{error:#}")}),
    };
    if answer["register_missing"] == json!(true) {
        return json!({"status":"absent"});
    }
    if let Some(many) = answer["duplicate_registers"].as_array()
        && !many.is_empty()
    {
        return json!({"status":"ambiguous","registers":many});
    }
    let mut terms = Vec::new();
    for entry in answer["entries"].as_array().into_iter().flatten() {
        if entry["complete"] == json!(true) {
            let term = s(&entry["term"]);
            if !term.is_empty() && !terms.contains(&term) {
                terms.push(term);
            }
        }
    }
    json!({"status":"read","card":answer["register"],"terms":terms})
}

/// One Topic's question: its body as one line, cut to the comparison bound.
pub(crate) fn question_of(body: &str) -> String {
    let mut line = String::new();
    for word in body.split_whitespace() {
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    clip(&line, QUESTION_CHARS)
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

fn read_inputs(zt: &mut Zt, staging: &str) -> Result<Inputs> {
    let trees = snapshot::read_title_trees(zt)?;
    let mut topics = Vec::new();
    let mut addresses = Vec::new();
    for tree in &trees {
        for card in tree["cards"].as_array().into_iter().flatten() {
            if s(&card["kind"]) != "topic" || s(&card["workspace"]) != "agent" {
                continue;
            }
            let address = s(&card["address"]);
            addresses.push(address.clone());
            topics.push(json!({
                "address": address,
                "title": s(&card["title"]),
                "tree": s(&card["tree"]),
            }));
        }
    }
    if !addresses.is_empty() {
        let read = zt.cards(&addresses, "text")?;
        let mut questions: BTreeMap<String, String> = BTreeMap::new();
        for card in read.cards {
            questions.insert(s(&card["address"]), question_of(&s(&card["body"])));
        }
        for topic in &mut topics {
            let address = s(&topic["address"]);
            topic["question"] = json!(questions.remove(&address).unwrap_or_default());
        }
    }
    let register = read_register(zt);
    let (terms, rows) = read_terms(staging);
    Ok(Inputs {
        topics,
        trees,
        register,
        terms,
        rows,
    })
}

/// The fingerprint a decision binds: the proposed title and question, every agent Topic's title and
/// question, the accepted register term labels, and the book's term rows.
fn fingerprint(proposed: &Value, topics: &[Value], register: &Value, rows: &[TermRow]) -> String {
    let topics: Vec<Value> = topics
        .iter()
        .map(|topic| {
            json!({
                "address": s(&topic["address"]),
                "title": s(&topic["title"]),
                "question": s(&topic["question"]),
            })
        })
        .collect();
    let mut terms: Vec<String> = register["terms"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|term| s(term))
        .collect();
    terms.sort();
    let rows: Vec<Value> = rows
        .iter()
        .map(|row| json!({"term":row.term,"avoid":row.avoid,"meaning":row.meaning}))
        .collect();
    canon::hash_value(&json!({
        "schema_version": SCHEMA,
        "proposed": proposed,
        "topics": topics,
        "register": {"status": s(&register["status"]), "terms": terms},
        "terms": rows,
    }))
}

/// The terms a Topic names through its title or question.
fn matched_terms(topic: &Value, rows: &[TermRow], register: &[String]) -> Vec<(String, &'static str)> {
    let text = format!("{} {}", s(&topic["title"]), s(&topic["question"]));
    let mut matched: Vec<(String, &'static str)> = Vec::new();
    for row in rows {
        let names = std::iter::once(&row.term).chain(row.avoid.iter());
        for name in names {
            if !name.is_empty() && text.contains(name.as_str()) {
                matched.push((row.term.clone(), "term-table"));
                break;
            }
        }
    }
    for term in register {
        if !term.is_empty()
            && text.contains(term.as_str())
            && !matched.iter().any(|(seen, source)| seen == term && *source == "register")
        {
            matched.push((term.clone(), "register"));
        }
    }
    matched
}

/// The shortlist: exact and near titles first (the fast check), then the TF-IDF order, then the
/// Topics that name a book or register term the proposed Topic also uses. At most [`SHORTLIST`].
fn shortlist(proposed: &Value, inputs: &Inputs, fast: &Value) -> Vec<Value> {
    let title = s(&proposed["title"]);
    let question = s(&proposed["question"]);
    let query = format!("{title}\n{question}");
    let register: Vec<String> = inputs
        .register["terms"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|term| s(term))
        .collect();
    let mut order: Vec<String> = Vec::new();
    let push = |order: &mut Vec<String>, address: String| {
        if !address.is_empty() && !order.contains(&address) {
            order.push(address);
        }
    };
    for address in fast["exact"].as_array().into_iter().flatten() {
        push(&mut order, s(address));
    }
    for candidate in fast["near"].as_array().into_iter().flatten() {
        push(&mut order, s(&candidate["address"]));
    }
    let documents: Vec<(String, String)> = inputs
        .topics
        .iter()
        .map(|topic| {
            let address = s(&topic["address"]);
            let text = format!(
                "{} {} {} {}",
                s(&topic["title"]),
                s(&topic["question"]),
                s(&topic["title"]),
                s(&topic["question"])
            );
            (address, text)
        })
        .collect();
    let mut unranked = false;
    if !documents.is_empty() {
        let model = Relatedness::new(&documents, KEEP);
        let mut scored = model.rank_text_scored(&query, SHORTLIST, |_| false);
        if scored.is_empty() {
            for address in model.overlap(&query, SHORTLIST, |_| false) {
                scored.push((address, MIN_MATCH));
            }
            unranked = true;
        }
        let best = scored.first().map(|(_, score)| *score).unwrap_or(0.0);
        for (address, score) in scored {
            // A card that shares one common bigram is not a plausible earlier Topic; a real match
            // scores far above the floor, and the relative cut drops the weak tail of a large box.
            if score >= MIN_MATCH && score >= best * MATCH_FLOOR {
                push(&mut order, address);
            }
        }
    }
    let proposal_terms: std::collections::BTreeSet<String> =
        matched_terms(&proposed, &inputs.rows, &register).into_iter().map(|(term, _)| term).collect();
    if !proposal_terms.is_empty() {
        for topic in &inputs.topics {
            if matched_terms(topic, &inputs.rows, &register)
                .iter()
                .any(|(term, _)| proposal_terms.contains(term))
            {
                push(&mut order, s(&topic["address"]));
            }
        }
    }
    order.truncate(SHORTLIST);
    let exact: BTreeSet<String> = fast["exact"].as_array().into_iter().flatten().map(|at| s(at)).collect();
    let near: BTreeSet<String> = fast["near"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|candidate| s(&candidate["address"]))
        .collect();
    let mut candidates = Vec::new();
    for address in &order {
        let Some(topic) = inputs.topics.iter().find(|topic| s(&topic["address"]) == *address) else {
            continue;
        };
        let matched = matched_terms(topic, &inputs.rows, &register);
        candidates.push(json!({
            "address": s(&topic["address"]),
            "title": s(&topic["title"]),
            "tree": s(&topic["tree"]),
            "question": s(&topic["question"]),
            "exact": exact.contains(address),
            "near": near.contains(address),
            "ranked": if unranked { "overlap" } else { "tfidf" },
            "matched_terms": matched.iter().map(|(term, source)| json!({"term":term,"source":source})).collect::<Vec<_>>(),
        }));
    }
    candidates
}

/// The fast first check: normalised exact and near title matches (D139).
fn fast_check(proposed: &Value, inputs: &Inputs) -> Value {
    let key = calls::title_key(&s(&proposed["title"]));
    let mut exact = Vec::new();
    let mut near = Vec::new();
    for topic in &inputs.topics {
        let other = calls::title_key(&s(&topic["title"]));
        if other == key {
            exact.push(s(&topic["address"]));
        } else {
            let score = calls::similarity(&other, &key);
            if score >= TITLE_SIMILARITY {
                near.push(json!({
                    "address": s(&topic["address"]),
                    "title": s(&topic["title"]),
                    "similarity": score,
                }));
            }
        }
    }
    json!({
        "title_key": key,
        "exact": exact,
        "near": near,
        "similarity": TITLE_SIMILARITY,
    })
}

/// The Regular Cards of one Topic tree, in Folgezettel order, for the representative read.
fn tree_members(inputs: &Inputs, tree: &str) -> Vec<String> {
    let mut members: Vec<String> = Vec::new();
    for catalogue in &inputs.trees {
        for card in catalogue["cards"].as_array().into_iter().flatten() {
            let address = s(&card["address"]);
            if s(&card["tree"]) == tree
                && s(&card["kind"]) == "regular"
                && address.starts_with(&format!("{tree}/"))
                && !members.contains(&address)
            {
                members.push(address);
            }
        }
    }
    members.sort_by(|a, b| canon::ordinal_cmp(a, b));
    members
}

/// `prepare`: build and freeze the shortlist evidence. No lock is taken here (D146).
fn prepare(zt: &mut Zt, staging: &str, title: &str, question: &str) -> Result<Value> {
    let proposed = json!({"title": title, "question": question});
    let inputs = read_inputs(zt, staging)?;
    let fast = fast_check(&proposed, &inputs);
    let mut candidates = shortlist(&proposed, &inputs, &fast);
    // Read every candidate's Topic Card and up to three Regular Cards of its tree, so the judge sees
    // definitions rather than names alone (D140).
    let mut wanted: Vec<String> = Vec::new();
    for candidate in &candidates {
        let tree = s(&candidate["tree"]);
        let ready = wanted.clone();
        if !ready.contains(&s(&candidate["address"])) {
            wanted.push(s(&candidate["address"]));
        }
        for member in tree_members(&inputs, &tree).into_iter().take(REPRESENTATIVE + 1) {
            if !wanted.contains(&member) {
                wanted.push(member);
            }
        }
    }
    wanted.truncate(SHORTLIST * (REPRESENTATIVE + 1) + SHORTLIST);
    let mut endpoints: BTreeMap<String, Value> = BTreeMap::new();
    if !wanted.is_empty() {
        let read = zt.cards(&wanted, "text")?;
        for card in read.cards {
            let address = s(&card["address"]);
            endpoints.insert(
                address.clone(),
                json!({
                    "address": address,
                    "title": s(&card["title"]),
                    "type": s(&card["type"]),
                    "excerpt": clip(&s(&card["body"]), 400),
                    "source_evidence": source_evidence(&card),
                }),
            );
        }
    }
    for candidate in &mut candidates {
        let address = s(&candidate["address"]);
        let tree = s(&candidate["tree"]);
        let mut representative = Vec::new();
        if let Some(own) = endpoints.get(&address) {
            representative.push(own.clone());
        }
        for member in tree_members(&inputs, &tree) {
            if let Some(card) = endpoints.get(&member) {
                representative.push(card.clone());
            }
        }
        representative.truncate(REPRESENTATIVE + 1);
        let matched = matched_terms(candidate, &inputs.rows, &[]);
        let definitions: Vec<Value> = inputs
            .rows
            .iter()
            .filter(|row| matched.iter().any(|(term, _)| term == &row.term))
            .map(|row| json!({
                "term": row.term,
                "meaning": row.meaning,
                "avoid": row.avoid,
                "source": "terms.md",
            }))
            .collect();
        candidate["representative"] = json!(representative);
        candidate["definitions"] = json!(definitions);
    }
    let mut missing = Vec::new();
    if s(&inputs.register["status"]) != "read" {
        missing.push(json!({"input":"register","status":s(&inputs.register["status"])}));
    }
    if s(&inputs.terms["status"]) != "read" {
        missing.push(json!({"input":"term_table","status":s(&inputs.terms["status"])}));
    }
    let complete = true;
    let outcome = if candidates.is_empty() { "new" } else { "judge" };
    let fingerprint = fingerprint(&proposed, &inputs.topics, &inputs.register, &inputs.rows);
    let mut evidence = json!({
        "schema_version": SCHEMA,
        "step": "prepare",
        "staging": staging,
        "proposed": proposed,
        "fast": fast,
        "inputs": {
            "required": {
                "title_catalog": {"status":"read","topics":inputs.topics.len()},
                "questions": {"status":"read"},
                "read_endpoints": {"status":"read","count":endpoints.len()},
            },
            "register": inputs.register,
            "term_table": inputs.terms,
            "optional_missing": missing.clone(),
        },
        "inputs_fingerprint": fingerprint,
        "complete": complete,
        "outcome": outcome,
        "shortlist": candidates,
        "endpoints": endpoints.values().cloned().collect::<Vec<_>>(),
        "lock": run_gate(staging).to_string_lossy(),
    });
    evidence["id"] = json!(canon::content_hash(&evidence, &["id"]));
    std::fs::create_dir_all(gate_dir(staging))?;
    canon::save_json(&evidence_path(staging).to_string_lossy(), &evidence)?;
    let answer = json!({
        "schema_version": SCHEMA,
        "step": "prepare",
        "outcome": outcome,
        "complete": complete,
        "evidence": evidence_path(staging).to_string_lossy(),
        "evidence_id": evidence["id"],
        "inputs_fingerprint": evidence["inputs_fingerprint"],
        "proposed": evidence["proposed"],
        "fast": evidence["fast"],
        "shortlist": evidence["shortlist"],
        "missing": missing,
        "lock": evidence["lock"],
    });
    canon::save_json(&prepare_path(staging).to_string_lossy(), &answer)?;
    Ok(answer)
}

/// A Card's source evidence from its `来源` Link, or the Literature tree it is filed in.
fn source_evidence(card: &Value) -> Vec<String> {
    let mut sources = zt_check::check::source_evidence(card);
    if sources.is_empty() {
        let tree = s(&card["tree"]);
        if tree.bytes().next().is_some_and(|byte| byte.is_ascii_uppercase()) {
            sources.push(tree);
        }
    }
    sources
}

/// Verify one judge answer against the frozen evidence. Pure, so every refusal rule is a unit test.
pub(crate) fn validate(evidence: &Value, answer: &Value) -> Result<Value> {
    let field = |value: &Value, key: &str| -> Result<String> {
        value[key]
            .as_str()
            .map(str::to_string)
            .with_context(|| format!("the decision is missing `{key}`"))
    };
    let known = [
        "schema_version",
        "proposed_title",
        "question",
        "verdict",
        "candidates",
        "selected_topic",
        "selected_entry",
        "reason",
        "evidence",
        "missing",
        "inputs_fingerprint",
    ];
    for key in answer.as_object().context("the decision must be an object")?.keys() {
        if !known.contains(&key.as_str()) {
            bail!("the decision has an unknown field `{key}`");
        }
    }
    if field(answer, "proposed_title")? != field(&evidence["proposed"], "title")?
        || field(answer, "question")? != field(&evidence["proposed"], "question")?
    {
        bail!("the decision answers a different proposed Topic than the evidence");
    }
    if field(answer, "inputs_fingerprint")? != field(evidence, "inputs_fingerprint")? {
        bail!("the decision binds another input fingerprint than the evidence");
    }
    let verdict = field(answer, "verdict")?;
    if !VERDICTS.contains(&verdict.as_str()) {
        bail!("unknown verdict `{verdict}`: use same, different, or uncertain");
    }
    let shortlist = evidence["shortlist"].as_array().context("the evidence has no shortlist")?;
    let addresses: BTreeSet<String> = shortlist.iter().map(|candidate| s(&candidate["address"])).collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut candidates = Vec::new();
    for candidate in answer["candidates"].as_array().context("the decision has no candidates")? {
        let address = field(candidate, "address")?;
        if !addresses.contains(&address) {
            bail!("the decision names `{address}`, which is not on the shortlist");
        }
        if !seen.insert(address.clone()) {
            bail!("the decision names `{address}` twice");
        }
        let verdict = field(candidate, "verdict")?;
        if !VERDICTS.contains(&verdict.as_str()) {
            bail!("unknown candidate verdict `{verdict}` for `{address}`");
        }
        candidates.push(json!({"address":address,"verdict":verdict,"note":candidate["note"]}));
    }
    if verdict == "different" {
        let missing: Vec<&String> = addresses.iter().filter(|at| !seen.contains(*at)).collect();
        if !missing.is_empty() {
            bail!(
                "a `different` decision must account for every shortlisted Topic; missing: {}",
                missing.iter().map(|at| at.as_str()).collect::<Vec<_>>().join(", ")
            );
        }
        if candidates.iter().any(|candidate| candidate["verdict"] == json!("same")) {
            bail!("a `different` decision cannot also call a shortlisted Topic the same");
        }
    }
    let endpoints: BTreeSet<String> = evidence["endpoints"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|card| s(&card["address"]))
        .collect();
    let mut cited = Vec::new();
    for address in answer["evidence"].as_array().context("the decision cites no evidence")? {
        let address = s(address);
        if address.is_empty() || !(addresses.contains(&address) || endpoints.contains(&address)) {
            bail!("the decision cites `{address}`, which was not read");
        }
        if !cited.contains(&address) {
            cited.push(address);
        }
    }
    if cited.is_empty() && !addresses.is_empty() {
        bail!("the decision must cite the addresses it read");
    }
    let reason = field(answer, "reason")?;
    if reason.trim().is_empty() {
        bail!("the decision needs a reason");
    }
    let mut decision = json!({
        "schema_version": SCHEMA,
        "step": "decide",
        "staging": evidence["staging"],
        "evidence_id": evidence["id"],
        "inputs_fingerprint": evidence["inputs_fingerprint"],
        "proposed": evidence["proposed"],
        "verdict": verdict,
        "candidates": candidates,
        "reason": reason,
        "evidence": cited,
        "lock": evidence["lock"],
    });
    match verdict.as_str() {
        "same" => {
            let topic = field(answer, "selected_topic")?;
            let entry = field(answer, "selected_entry")?;
            if !addresses.contains(&topic) {
                bail!("`same` must name a shortlisted Topic, not `{topic}`");
            }
            if !endpoints.contains(&entry) {
                bail!("`same` must name a read Regular Card, not `{entry}`");
            }
            if !candidates
                .iter()
                .any(|candidate| s(&candidate["address"]) == topic && candidate["verdict"] == json!("same"))
            {
                bail!("`same` must mark `{topic}` as the same on the candidate list");
            }
            decision["selected_topic"] = json!(topic);
            decision["selected_entry"] = json!(entry);
        }
        "different" => {
            if let Some(selected) = answer.get("selected_topic").filter(|value| !value.is_null()) {
                bail!("a `different` decision selects no existing Topic (got `{}`)", s(selected));
            }
        }
        _ => {
            let missing: Vec<String> = answer["missing"]
                .as_array()
                .context("an `uncertain` decision must say what context is missing")?
                .iter()
                .map(|item| s(item))
                .filter(|item| !item.trim().is_empty())
                .collect();
            if missing.is_empty() {
                bail!("an `uncertain` decision must name the context it is missing");
            }
            decision["missing"] = json!(missing);
        }
    }
    decision["id"] = json!(canon::content_hash(&decision, &["id"]));
    Ok(decision)
}

fn decide(staging: &str, answer_path: &str) -> Result<Value> {
    let evidence = canon::read_json(&evidence_path(staging).to_string_lossy())?;
    verify_evidence(&evidence)?;
    let answer = canon::read_json(answer_path)?;
    let decision = validate(&evidence, &answer)?;
    canon::save_json(&decision_path(staging).to_string_lossy(), &decision)?;
    Ok(decision)
}

fn verify_evidence(evidence: &Value) -> Result<()> {
    if evidence["schema_version"] != json!(SCHEMA)
        || canon::content_hash(evidence, &["id"]) != s(&evidence["id"])
    {
        bail!("the gate evidence is not the frozen document it claims to be");
    }
    Ok(())
}

/// Verify that a concept decision file is the frozen document it claims to be.
pub(crate) fn check_decision(decision: &Value) -> Result<()> {
    verify_decision(decision)
}

fn verify_decision(decision: &Value) -> Result<()> {
    if decision["schema_version"] != json!(SCHEMA)
        || canon::content_hash(decision, &["id"]) != s(&decision["id"])
    {
        bail!("the concept decision is not the frozen document it claims to be");
    }
    Ok(())
}

/// The lock a managed creation takes: the run's Topic gate file the decision recorded (the same file
/// the run's group write commands name and `ledger` group-apply guards with).
pub(crate) struct Held {
    #[allow(dead_code)]
    lock: Gate,
}

/// Take the Topic gate and revalidate the decision against the live concept inputs (D146–D151). The
/// lock is returned to the caller, which holds it until the receipt of the creation is known.
pub(crate) fn acquire(zt: &mut Zt, decision: &Value) -> Result<Held> {
    let lock_path = s(&decision["lock"]);
    if lock_path.is_empty() {
        bail!("the concept decision names no Topic gate lock");
    }
    let lock = calls::lock_gate(&lock_path)?;
    let staging = s(&decision["staging"]);
    let inputs = match read_inputs(zt, &staging) {
        Ok(inputs) => inputs,
        Err(error) => {
            drop(lock);
            return Err(error);
        }
    };
    let now = fingerprint(&decision["proposed"], &inputs.topics, &inputs.register, &inputs.rows);
    if now != s(&decision["inputs_fingerprint"]) {
        drop(lock);
        return Err(canon::refuse(
            "stale_decision: the Topics, the register or the term table changed since the concept decision; run the gate again",
        ));
    }
    // The fast first check survives as a hard refusal: an existing agent Topic whose normalised title
    // equals the proposal is the same Topic, whatever a judge said about the shortlist.
    let key = calls::title_key(&s(&decision["proposed"]["title"]));
    if let Some(same) = inputs
        .topics
        .iter()
        .find(|topic| calls::title_key(&s(&topic["title"])) == key)
    {
        let address = s(&same["address"]);
        let title = s(&same["title"]);
        drop(lock);
        return Err(canon::refuse(format!(
            "topic exists: `{address}` {title} has the same normalised title; file the Card under that Topic instead of creating one"
        )));
    }
    match s(&decision["verdict"]).as_str() {
        "different" => Ok(Held { lock }),
        "same" => {
            drop(lock);
            Err(canon::refuse(format!(
                "topic exists: `{}` {} is the same Topic; file the Card under that Topic and its Regular Card `{}`",
                s(&decision["selected_topic"]),
                topic_title(&inputs.topics, &s(&decision["selected_topic"])),
                s(&decision["selected_entry"]),
            )))
        }
        "uncertain" => {
            drop(lock);
            let missing: Vec<String> = decision["missing"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|item| s(item))
                .collect();
            Err(canon::refuse(format!(
                "uncertain: the concept judge needs more context before a Topic can be created: {}",
                missing.join("; ")
            )))
        }
        other => {
            drop(lock);
            Err(canon::refuse(format!("unknown verdict `{other}`")))
        }
    }
}

fn topic_title(topics: &[Value], address: &str) -> String {
    topics
        .iter()
        .find(|topic| s(&topic["address"]) == address)
        .map(|topic| s(&topic["title"]))
        .unwrap_or_default()
}

/// The identified operation key of the one creation.
fn operation_key(prefix: &str) -> String {
    format!("{prefix}/item-000000")
}

/// Whether the operation already committed, before any new concept decision is made (D153).
fn committed(zt: &mut Zt, prefix: &str, call: &Value) -> Result<Option<Value>> {
    let item = json!({
        "op_key": operation_key(prefix),
        "client_ref": "0",
        "tool": call["tool"],
        "arguments": call["arguments"],
    });
    let reply = zt.call("apply_batch", json!({"preview":true,"preview_items":[item]}))?;
    let result = reply["results"].as_array().and_then(|results| results.first()).cloned().unwrap_or(Value::Null);
    if result["replayed"] == json!(true) {
        return Ok(Some(result));
    }
    Ok(None)
}

/// The agent Topics that copy the created Topic's normalised title: a raw client's competing
/// creation, reported after the receipt (D154). The created Topic itself is never competing.
fn competing(topics: &[Value], address: &str, title: &str) -> Vec<Value> {
    let key = calls::title_key(title);
    topics
        .iter()
        .filter(|topic| s(&topic["address"]) != address && calls::title_key(&s(&topic["title"])) == key)
        .map(|topic| json!({"address":s(&topic["address"]),"title":s(&topic["title"])}))
        .collect()
}

/// `create`: the locked revalidation and the receipt-owned write.
fn create(zt: &mut Zt, staging: &str, decision_file: &str, prefix: &str) -> Result<(Value, i32)> {
    let decision = canon::read_json(decision_file)?;
    verify_decision(&decision)?;
    let evidence = canon::read_json(&evidence_path(staging).to_string_lossy())?;
    verify_evidence(&evidence)?;
    if s(&decision["evidence_id"]) != s(&evidence["id"]) {
        bail!("the concept decision does not belong to this gate evidence");
    }
    if evidence["complete"] != json!(true) {
        return Ok((
            json!({
                "created": false,
                "reason": "incomplete_evidence",
                "detail": "the gate has no complete concept input; nothing may be cleared as new",
            }),
            3,
        ));
    }
    let title = s(&decision["proposed"]["title"]);
    let question = s(&decision["proposed"]["question"]);
    let call = json!({"tool":"create_topic","arguments":{"title":title,"body":question}});
    // D153: a committed operation returns its existing Topic receipt before a new concept decision.
    if let Some(receipt) = committed(zt, prefix, &call)? {
        let address = s(&receipt["result"]["address"]);
        let answer = json!({
            "created": false,
            "reason": "replayed",
            "address": address,
            "receipt": operation_key(prefix),
            "note": "the operation had already committed; no second Topic was created",
        });
        canon::save_json(&result_path(staging).to_string_lossy(), &answer)?;
        return Ok((answer, 0));
    }
    let verdict = s(&decision["verdict"]);
    if verdict == "same" || verdict == "uncertain" {
        let inputs = read_inputs(zt, staging)?;
        let now = fingerprint(&decision["proposed"], &inputs.topics, &inputs.register, &inputs.rows);
        if now != s(&decision["inputs_fingerprint"]) {
            let answer = json!({
                "created": false,
                "reason": "stale_decision",
                "verdict": verdict,
                "detail": "the Topics, the register or the term table changed since the decision; run the gate again",
            });
            canon::save_json(&result_path(staging).to_string_lossy(), &answer)?;
            return Ok((answer, 3));
        }
        let answer = if verdict == "same" {
            json!({
                "created": false,
                "reason": "same",
                "verdict": verdict,
                "existing": {
                    "topic": decision["selected_topic"],
                    "title": topic_title(&inputs.topics, &s(&decision["selected_topic"])),
                    "entry": decision["selected_entry"],
                },
            })
        } else {
            json!({
                "created": false,
                "reason": "uncertain",
                "verdict": verdict,
                "missing": decision["missing"],
            })
        };
        canon::save_json(&result_path(staging).to_string_lossy(), &answer)?;
        return Ok((answer, 0));
    }
    if verdict != "different" {
        bail!("unknown verdict `{verdict}` in the concept decision");
    }
    let held = match acquire(zt, &decision) {
        Ok(held) => held,
        Err(error) if canon::is_refusal(&error) => {
            // A decided refusal is an answer, not an error: the caller keeps its draft and either
            // follows the existing Topic or asks for a fresh decision.
            let detail = format!("{error}");
            let stale = detail.starts_with("stale_decision");
            let answer = json!({
                "created": false,
                "reason": if stale { "stale_decision" } else { "refused" },
                "detail": detail,
            });
            canon::save_json(&result_path(staging).to_string_lossy(), &answer)?;
            return Ok((answer, if stale { 3 } else { 0 }));
        }
        Err(error) => return Err(error),
    };
    let receipts = calls::invoke_identified(zt, std::slice::from_ref(&call), prefix, None)?;
    drop(held);
    let receipt = receipts.first().cloned().unwrap_or(Value::Null);
    let address = s(&receipt["result"]["address"]);
    if address.is_empty() {
        bail!("the Topic creation returned no receipt address");
    }
    // Verify the persisted result and report a raw client that created a competing Topic.
    let card = zt.cards(std::slice::from_ref(&address), "text")?;
    let card = card.cards.first().cloned().unwrap_or(Value::Null);
    if s(&card["workspace"]) != "agent" || s(&card["address"]) != address || card["kind"] != json!("topic") {
        bail!("the created Topic was not persisted at `{address}`");
    }
    let topics = read_inputs(zt, staging)?.topics;
    let competing: Vec<Value> = competing(&topics, &address, &title);
    let answer = json!({
        "created": true,
        "address": address,
        "receipt": receipt["op_key"],
        "verdict": verdict,
        "title": s(&card["title"]),
        "competing": competing,
        "note": "the Topic gate coordinates managed digest processes only; a raw Owner or GUI creation is still possible and is reported here when it happened",
    });
    canon::save_json(&result_path(staging).to_string_lossy(), &answer)?;
    Ok((answer, 0))
}

/// `zt-digest gate -Step prepare|decide|create|report`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(
        argv,
        &["Step", "Staging", "Title", "Question", "Answer", "Decision", "OperationPrefix"],
        &[],
    )?;
    let step = a.str_or("Step", "");
    let staging = a.str("Staging");
    if staging.trim().is_empty() {
        bail!("-Staging is required");
    }
    let mut zt = crate::zt::start()?;
    let value = match step.as_str() {
        "prepare" => {
            let title = a.str("Title").trim().to_string();
            let question = a.str("Question").trim().to_string();
            if title.is_empty() {
                bail!("-Title is required");
            }
            if question.is_empty() {
                bail!("-Question is required: the gate compares the Topic's question, not only its name");
            }
            prepare(&mut zt, &staging, &title, &question)?
        }
        "decide" => {
            let answer = a.str("Answer");
            if answer.trim().is_empty() {
                bail!("-Answer is required");
            }
            decide(&staging, &answer)?
        }
        "create" => {
            let decision = if a.has("Decision") {
                a.str("Decision")
            } else {
                decision_path(&staging).to_string_lossy().to_string()
            };
            let prefix = a.str("OperationPrefix");
            if prefix.trim().is_empty() {
                bail!("-OperationPrefix is required: a Topic creation is one receipt-owned operation");
            }
            let (value, code) = create(&mut zt, &staging, &decision, &prefix)?;
            let report = report(&staging)?;
            let _ = report;
            util::print_line(&serde_json::to_string(&value)?)?;
            return Ok(code);
        }
        "report" => report(&staging)?,
        other => bail!("unknown gate step `{other}`: use prepare, decide, create, or report"),
    };
    util::print_line(&serde_json::to_string(&value)?)?;
    Ok(0)
}

/// The gate report: what was prepared, what the judge decided, and what was created.
fn report(staging: &str) -> Result<Value> {
    let prepared = canon::read_json(&prepare_path(staging).to_string_lossy()).unwrap_or(Value::Null);
    let decision = canon::read_json(&decision_path(staging).to_string_lossy()).unwrap_or(Value::Null);
    let result = canon::read_json(&result_path(staging).to_string_lossy()).unwrap_or(Value::Null);
    let complete = !prepared.is_null() && !decision.is_null();
    let value = json!({
        "schema_version": SCHEMA,
        "step": "report",
        "complete": complete,
        "outcome": prepared["outcome"],
        "evidence_id": prepared["evidence_id"],
        "missing": prepared["missing"],
        "shortlist": prepared["shortlist"].as_array().map_or(0, |list| list.len()),
        "verdict": decision["verdict"],
        "reason": decision["reason"],
        "result": result,
        "guarantee": "managed digest processes share the Topic gate lock; a raw Owner or GUI creation is outside it and is reported as `competing` when it happened",
    });
    canon::save_json(&report_path(staging).to_string_lossy(), &value)?;
    canon::save_json(&report_markdown_path(staging).to_string_lossy(), &json!(render(&value)))?;
    Ok(value)
}

fn render(value: &Value) -> String {
    let mut text = String::new();
    text.push_str("# Topic gate\n\n");
    text.push_str(&format!("- outcome: {}\n", s(&value["outcome"])));
    text.push_str(&format!("- shortlist: {}\n", value["shortlist"]));
    // AC08: a missing or ambiguous concept input reaches the report, never silently substituted.
    if let Some(missing) = value["missing"].as_array().filter(|list| !list.is_empty()) {
        text.push_str("- optional inputs missing:\n");
        for item in missing {
            text.push_str(&format!("  - {}: {}\n", s(&item["input"]), s(&item["status"])));
        }
    }
    text.push_str(&format!("- verdict: {}\n", s(&value["verdict"])));
    if !value["reason"].is_null() {
        text.push_str(&format!("- reason: {}\n", s(&value["reason"])));
    }
    text.push_str(&format!("- result: {}\n", s(&value["result"]["reason"])));
    if value["result"]["created"] == json!(true) {
        text.push_str(&format!("- created: `{}`\n", s(&value["result"]["address"])));
    }
    if let Some(competing) = value["result"]["competing"].as_array().filter(|list| !list.is_empty()) {
        text.push_str("- competing raw creation:\n");
        for topic in competing {
            text.push_str(&format!("  - `{}` {}\n", s(&topic["address"]), s(&topic["title"])));
        }
    }
    text.push_str(&format!("\n{}\n", s(&value["guarantee"])));
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(term: &str, avoid: &[&str], meaning: &str) -> TermRow {
        TermRow {
            term: term.to_string(),
            avoid: avoid.iter().map(|word| word.to_string()).collect(),
            meaning: meaning.to_string(),
        }
    }

    fn evidence(shortlist: &[(&str, &str, &str)], endpoints: &[&str]) -> Value {
        let shortlist: Vec<Value> = shortlist
            .iter()
            .map(|(address, title, question)| {
                json!({"address":address,"title":title,"question":question,"tree":address.split('/').next().unwrap(),"exact":false,"near":false,"matched_terms":[]})
            })
            .collect();
        json!({
            "schema_version": SCHEMA,
            "step": "prepare",
            "staging": "C:/out",
            "proposed": {"title":"均衡多重性","question":"为什么同一市场有多个均衡？"},
            "inputs_fingerprint": "f".repeat(64),
            "complete": true,
            "shortlist": shortlist,
            "endpoints": endpoints.iter().map(|address| json!({"address":address,"title":"t"})).collect::<Vec<_>>(),
            "lock": "C:/out/topic-gate.lock",
            "id": "e",
        })
    }

    fn answer(verdict: &str, candidates: &[(&str, &str)]) -> Value {
        json!({
            "proposed_title": "均衡多重性",
            "question": "为什么同一市场有多个均衡？",
            "verdict": verdict,
            "candidates": candidates.iter().map(|(address, verdict)| json!({"address":address,"verdict":verdict})).collect::<Vec<_>>(),
            "reason": "这两个 Topic 追问的问题不同。",
            "evidence": ["1/0"],
            "inputs_fingerprint": "f".repeat(64),
        })
    }

    #[test]
    fn a_different_decision_must_account_for_every_candidate() {
        let evidence = evidence(&[("1/0", "多重均衡", "什么导致多重均衡？"), ("2/0", "均衡的多重性", "为什么有多个均衡？")], &["1/0", "1/1"]);
        let partial = answer("different", &[("1/0", "different")]);
        let error = validate(&evidence, &partial).expect_err("a partial `different` is refused");
        assert!(format!("{error:#}").contains("2/0"), "{error:#}");
        let full = answer("different", &[("1/0", "different"), ("2/0", "different")]);
        let decision = validate(&evidence, &full).expect("a complete `different` clears creation");
        assert_eq!(decision["verdict"], json!("different"));
        assert_eq!(decision["id"].as_str().unwrap().len(), 64);
    }

    #[test]
    fn a_different_decision_cannot_call_one_candidate_the_same() {
        let evidence = evidence(&[("1/0", "多重均衡", "什么？"), ("2/0", "均衡的多重性", "为什么？")], &["1/0"]);
        let contradictory = answer("different", &[("1/0", "same"), ("2/0", "different")]);
        let error = validate(&evidence, &contradictory).expect_err("contradiction is refused");
        assert!(format!("{error:#}").contains("cannot also call"), "{error:#}");
    }

    #[test]
    fn same_names_the_topic_and_a_read_regular_card() {
        let evidence = evidence(&[("1/0", "多重均衡", "什么？"), ("2/0", "均衡的多重性", "为什么？")], &["1/0", "1/1"]);
        let mut good = answer("same", &[("1/0", "same"), ("2/0", "different")]);
        good["selected_topic"] = json!("1/0");
        good["selected_entry"] = json!("1/1");
        let decision = validate(&evidence, &good).expect("a complete `same` is accepted");
        assert_eq!(decision["selected_entry"], json!("1/1"));
        let mut unread = good.clone();
        unread["selected_entry"] = json!("9/9");
        assert!(validate(&evidence, &unread).is_err());
        let mut unmarked = good.clone();
        unmarked["candidates"] = json!([{"address":"1/0","verdict":"different"},{"address":"2/0","verdict":"different"}]);
        assert!(validate(&evidence, &unmarked).is_err());
        let mut no_topic = good.clone();
        no_topic["selected_topic"] = json!("7/0");
        assert!(validate(&evidence, &no_topic).is_err());
    }

    #[test]
    fn uncertain_names_its_missing_context_and_never_selects_a_topic() {
        let evidence = evidence(&[("1/0", "多重均衡", "什么？")], &["1/0"]);
        let mut uncertain = answer("uncertain", &[("1/0", "uncertain")]);
        let error = validate(&evidence, &uncertain).expect_err("uncertain without `missing` is refused");
        assert!(format!("{error:#}").contains("missing"));
        uncertain["missing"] = json!(["本书对「学习」的定义"]);
        let decision = validate(&evidence, &uncertain).expect("a named missing context is accepted");
        assert_eq!(decision["missing"][0], json!("本书对「学习」的定义"));
        assert!(decision["selected_topic"].is_null());
    }

    #[test]
    fn unknown_verdicts_cited_unread_endpoints_and_unknown_fields_are_refused() {
        let evidence = evidence(&[("1/0", "多重均衡", "什么？")], &["1/0"]);
        let mut value = answer("maybe", &[("1/0", "different")]);
        assert!(format!("{:#}", validate(&evidence, &value).unwrap_err()).contains("unknown verdict"));
        value = answer("different", &[("1/0", "different")]);
        value["evidence"] = json!(["9/9"]);
        assert!(format!("{:#}", validate(&evidence, &value).unwrap_err()).contains("not read"));
        value = answer("different", &[("1/0", "different")]);
        value["surprise"] = json!(1);
        assert!(format!("{:#}", validate(&evidence, &value).unwrap_err()).contains("unknown field"));
        value = answer("different", &[("1/0", "different")]);
        value["inputs_fingerprint"] = json!("other");
        assert!(format!("{:#}", validate(&evidence, &value).unwrap_err()).contains("fingerprint"));
        value = answer("different", &[("1/0", "different"), ("1/0", "different")]);
        assert!(format!("{:#}", validate(&evidence, &value).unwrap_err()).contains("twice"));
    }

    #[test]
    fn the_fingerprint_binds_concepts_and_not_the_archive_revision() {
        let topics = vec![json!({"address":"1/0","title":"多重均衡","question":"什么导致多重均衡？","tree":"1"})];
        let register = json!({"status":"read","terms":["多重均衡"]});
        let rows = vec![term("均衡", &["平衡"], "各方的策略互相最佳反应")];
        let proposed = json!({"title":"均衡多重性","question":"为什么有多个均衡？"});
        let first = fingerprint(&proposed, &topics, &register, &rows);
        assert_eq!(first.len(), 64);
        assert_eq!(first, fingerprint(&proposed, &topics, &register, &rows), "the same concepts give the same fingerprint");
        // A changed question is a changed concept input.
        let mut changed = topics.clone();
        changed[0]["question"] = json!("什么导致唯一均衡？");
        assert_ne!(first, fingerprint(&proposed, &changed, &register, &rows));
        // A new register term or a changed term row is a changed concept input.
        assert_ne!(first, fingerprint(&proposed, &topics, &json!({"status":"read","terms":["多重均衡","学习"]}), &rows));
        assert_ne!(first, fingerprint(&proposed, &topics, &register, &vec![term("均衡", &[], "另一个意思")]));
    }

    #[test]
    fn questions_are_one_line_and_bounded() {
        assert_eq!(question_of("为什么  同一市场\n有多个均衡？"), "为什么 同一市场 有多个均衡？");
        let long = "均".repeat(500);
        let question = question_of(&long);
        assert_eq!(question.chars().count(), QUESTION_CHARS + 1);
        assert!(question.ends_with('…'));
    }

    #[test]
    fn term_rows_read_the_book_table() {
        let dir = std::env::temp_dir().join(format!("zt-gate-terms-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("terms.md"),
            "| 术语 | 不用 | 意思 | 首次定义 |\n|---|---|---|---|\n| 均衡 | 平衡 | 各方策略互相最佳反应 | 1.2 |\n| 学习 | 学习效应 | 经验改变行为 | 3.1 |\n",
        )
        .unwrap();
        let (facts, rows) = read_terms(&dir.to_string_lossy());
        assert_eq!(facts["status"], json!("read"));
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].term, "均衡");
        assert_eq!(rows[0].avoid, vec!["平衡".to_string()]);
        assert_eq!(rows[1].meaning, "经验改变行为");
        std::fs::write(dir.join("terms.md"), "没有表格。\n").unwrap();
        let (facts, rows) = read_terms(&dir.to_string_lossy());
        assert_eq!(facts["rows"], json!(0));
        assert!(rows.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_matched_term_puts_a_topic_on_the_shortlist_even_when_the_words_differ() {
        let inputs = Inputs {
            topics: vec![
                json!({"address":"1/0","title":"多重均衡","question":"什么导致多重均衡？","tree":"1"}),
                json!({"address":"2/0","title":"均衡的多重性","question":"一个市场为什么有多个解？","tree":"2"}),
                json!({"address":"3/0","title":"学习","question":"经验如何改变行为？","tree":"3"}),
            ],
            trees: Vec::new(),
            register: json!({"status":"read","terms":["均衡"]}),
            terms: json!({"status":"read"}),
            rows: vec![term("均衡", &["平衡"], "策略互相最佳反应")],
        };
        let proposed = json!({"title":"均衡的多重性","question":"为什么存在多个均衡？"});
        let fast = fast_check(&proposed, &inputs);
        assert_eq!(fast["exact"], json!(["2/0"]), "the normalised title is the fast first check");
        let candidates = shortlist(&proposed, &inputs, &fast);
        let addresses: Vec<String> = candidates.iter().map(|c| s(&c["address"])).collect();
        assert!(addresses.contains(&"1/0".to_string()), "{addresses:?}");
        assert!(addresses.contains(&"2/0".to_string()), "{addresses:?}");
        assert!(!addresses.contains(&"3/0".to_string()), "an unrelated Topic is not a candidate: {addresses:?}");
        assert!(candidates.len() <= SHORTLIST);
    }

    #[test]
    fn a_competing_raw_creation_is_reported_by_title_key_and_never_the_created_topic() {
        let topics = vec![
            json!({"address":"1/0","title":"均衡 多重性"}),
            json!({"address":"2/0","title":"均衡多重性"}),
            json!({"address":"3/0","title":"学习"}),
        ];
        let reported = competing(&topics, "2/0", "均衡多重性");
        assert_eq!(reported.len(), 1, "a raw Topic with the same normalised title is reported");
        assert_eq!(reported[0]["address"], json!("1/0"));
        assert!(competing(&topics, "2/0", "利率平价").is_empty(), "no competitor is invented");
        assert!(competing(&topics, "3/0", "学习").is_empty(), "the created Topic never competes with itself");
    }

    #[test]
    fn an_exact_title_is_the_fast_first_check() {
        let inputs = Inputs {
            topics: vec![
                json!({"address":"1/0","title":"均衡 多重性","question":"什么？","tree":"1"}),
                json!({"address":"2/0","title":"学习","question":"经验？","tree":"2"}),
            ],
            trees: Vec::new(),
            register: json!({"status":"absent"}),
            terms: json!({"status":"absent"}),
            rows: Vec::new(),
        };
        let proposed = json!({"title":"均衡多重性","question":"为什么？"});
        let fast = fast_check(&proposed, &inputs);
        assert_eq!(fast["exact"], json!(["1/0"]));
        assert_eq!(fast["title_key"], json!("均衡多重性"));
    }
}
