//! `zt-digest bridge`: the port of `scripts/book-digest/bridge-compiler.ps1`. It plans, checks,
//! publishes, and explicitly applies report-first cross-book Bridges. Every step prints compact
//! JSON last. Plan, precheck, report, and preview never edit Cards.
//!
//!   plan      reads the archive (list_trees, get_tree, get_model for math heads, get_card),
//!             picks up to -MaxSources source Cards of -CitationKey, finds up to -Candidates
//!             similar Cards in other books (plus at most two term matches), freezes them in
//!             <Work>\plan.json, and writes one judge job per twelve sources
//!   precheck  checks one judge answer (<Work>\out\<Job>.json) against the frozen plan (BR1-BR9)
//!   report    checks every answer, numbers the proposals, and publishes <Work>\proposals.json
//!             (immutable) and report.md
//!   apply     with -ReportId and -Accept / -AcceptEarlier: previews the accepted proposals;
//!             with -Apply also writes them (guarded edits, journal in state.json) and verifies
//!
//! One process holds one `zt mcp` for its reads and writes; above 40 calls, up to six threads
//! read 40-call chunks at once, each on its own `zt mcp`. zt-check runs in-process. Job files name
//! their self-check as `zt-digest bridge -Step precheck …` (no pwsh). The instruction file
//! dsh-bridge-instructions.zh.md is copied from ZT_DIGEST_SCRIPTS (the forwarder sets it) or from
//! the scripts\book-digest folder of the repository this binary was built from.

mod answer;
mod apply;
use crate::canon;
mod report;

use crate::args::Args;
use crate::audit::book_of::{self, BookIndex};
use crate::card::relatedness::Relatedness;
use crate::check::Check;
use crate::ps::{self, ps_array, ps_count, ps_items, s, truthy};
use crate::util;
use crate::zt::Zt;
use anyhow::{Result, bail};
use canon::{is_refusal, ordinal_cmp, refuse};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;
use zt_check::mcp::{reply_error, reply_text};

const PARAMS: [&str; 14] = ["Step", "Staging", "CitationKey", "Work", "Profile", "MaxSources", "Candidates", "Books", "Job", "ReportId", "Accept", "AcceptEarlier", "Report", "At"];
const READ_TOOLS: [&str; 4] = ["list_trees", "get_tree", "get_card", "get_model"];
const JOB_PROMPT: &str = "按任务文件完成桥接判断，运行 SELF-CHECK，只回复一行 JSON。";
const CHUNK: usize = 40;
const WORKERS: usize = 6;
/// The most current candidates one projection of a selected report returns (§12.2, D293).
const PROJECT_MAX: usize = 20;

static WS_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\r?\n\s*").unwrap());
static JOB_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^br-[0-9]{3}$").unwrap());
static TERM_ROW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*\|\s*([^|]+)\|").unwrap());
static TERM_RULE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[-: ]+$").unwrap());
static TERM_HEAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(术语|词语|term)$").unwrap());
static NAME_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)[：:]").unwrap());
static LINE_BREAKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\r\n]+").unwrap());

// --- PowerShell values -------------------------------------------------------------------------

/// `$object.name`: the member, or null.
pub(super) fn get<'a>(v: &'a Value, name: &str) -> &'a Value {
    v.get(name).unwrap_or(&Value::Null)
}

/// `[string]$object.name`.
pub(super) fn field_of(v: &Value, name: &str) -> String {
    s(get(v, name))
}

/// `$value -ceq 'text'`.
pub(super) fn ceq(v: &Value, text: &str) -> bool {
    match v {
        Value::Null | Value::Array(_) | Value::Object(_) => false,
        other => s(other) == text,
    }
}

/// `$a -ceq $b` for two values.
pub(super) fn ceqv(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Null, _) | (_, Value::Null) => false,
        (Value::String(x), Value::String(y)) => x == y,
        (x, y) => ceq(x, &s(y)),
    }
}

/// `$value -cin @(...)`.
pub(super) fn cin(v: &Value, list: &[&str]) -> bool {
    list.iter().any(|item| ceq(v, item))
}

/// `$value -ne 1` (a number compares as a number, text as text).
pub(super) fn ne_one(v: &Value) -> bool {
    match v {
        Value::Number(n) => n.as_f64() != Some(1.0),
        Value::String(t) => t != "1",
        Value::Bool(b) => !b,
        _ => true,
    }
}

/// `$_.Exception.Message`.
pub(super) fn message(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

/// `Body-Text`: `[string]` with CRLF and CR made LF.
pub(super) fn body_text(v: &Value) -> String {
    s(v).replace("\r\n", "\n").replace('\r', "\n")
}

/// `Invoke-ZtCheck -Json <file> [-Setting <address>]`, in-process. zt-check --json reads no Card,
/// so it needs no `zt mcp`.
pub(super) fn zt_check_json(json: &str, setting: &str) -> Result<Vec<String>> {
    let check = Check { json: json.to_string(), setting: setting.to_string(), ..Check::default() };
    let lines = match zt_check::check::run_lines(&check.args(), None) {
        Ok(lines) => lines,
        Err(error) => bail!("zt-check did not run: {error:#}"),
    };
    if !lines.iter().any(|l| l.starts_with("faults: ") && !l[8..].is_empty() && l[8..].chars().all(|c| c.is_ascii_digit())) {
        bail!("zt-check did not run: {}", lines.join(" | "));
    }
    Ok(lines)
}

/// `Endpoint`: the frozen snapshot of a Card: its fields, category, inferred book, fingerprint,
/// Links (not broken), and model.
pub(super) fn endpoint(card: &Value, index: &BookIndex) -> Value {
    let mut record = Map::new();
    for name in ["address", "workspace", "tree", "kind", "type", "facet", "emphasis", "level", "form", "title", "body"] {
        record.insert(name.into(), get(card, name).clone());
    }
    let address = field_of(card, "address");
    record.insert("category".into(), index.category.get(&address).map_or(Value::Null, |c| json!(c)));
    record.insert("book".into(), index.book_of.get(&address).map_or(Value::Null, |b| json!(b)));
    record.insert("title".into(), json!(body_text(get(card, "title"))));
    record.insert("body".into(), json!(body_text(get(card, "body"))));
    let values: Vec<Value> = ["workspace", "tree", "kind", "type", "facet", "emphasis", "level", "form", "title", "body"].iter().map(|k| record[*k].clone()).collect();
    record.insert("fingerprint".into(), json!(format!("sha256:{}", canon::hash_value(&Value::Array(values)))));
    let links: Vec<Value> = ps_array(get(card, "outbound")).iter().filter(|o| !truthy(get(o, "broken"))).map(|o| get(o, "address").clone()).collect();
    record.insert("links".into(), Value::Array(links));
    record.insert("model".into(), get(card, "model").clone());
    Value::Object(record)
}

/// Why a projected endpoint is no longer the one the report froze: a compared field (workspace,
/// tree, kind, type, facet, emphasis, level, form, title), its inferred book, or its authored
/// fingerprint changed, or the report never snapshotted it. This is the apply path's own endpoint
/// check (`selection_states`), reused so a projection and an apply agree on what is current
/// (§10.3, D258; S20-AC04). Generated Reverse links are outside the fingerprint: only authored
/// text changes it.
fn changed_endpoint(card: &Value, snapshot: Option<&Value>, index: &BookIndex) -> Option<String> {
    let address = field_of(card, "address");
    let Some(snapshot) = snapshot else {
        return Some(format!("missing endpoint {address} in the report"));
    };
    let now = endpoint(card, index);
    for name in ["workspace", "tree", "kind", "type", "facet", "emphasis", "level", "form", "title"] {
        if !ceqv(get(&now, name), get(snapshot, name)) {
            return Some(format!("changed endpoint {address} {name}"));
        }
    }
    if !ceqv(get(&now, "book"), get(snapshot, "book")) {
        return Some(format!("changed endpoint {address} book"));
    }
    if !ceqv(get(&now, "fingerprint"), get(snapshot, "fingerprint")) {
        return Some(format!("changed body or endpoint {address}"));
    }
    None
}

/// `Substantive`: a Regular Card that a Bridge may join.
fn substantive(card: &Value, index: &BookIndex) -> bool {
    ceq(get(card, "kind"), "regular")
        && index.category.get(&field_of(card, "address")).is_some_and(|c| ["idea", "event", "finding", "model", "part"].contains(&c.as_str()))
        && !ceq(get(card, "form"), "theory")
        && !answer::NOTATION.is_match(&field_of(card, "title"))
}

/// `Source-Rank`: formal models, concept models, other models, Ideas, Findings, the rest.
fn source_rank(card: &Value) -> i64 {
    if ceq(get(card, "type"), "model") {
        return if ceq(get(card, "level"), "formal") {
            0
        } else if ceq(get(card, "level"), "concept") {
            1
        } else {
            2
        };
    }
    if ceq(get(card, "type"), "idea") {
        return 3;
    }
    if ceq(get(card, "type"), "finding") {
        return 4;
    }
    5
}

/// `PS-Literal`.
fn ps_literal(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// The folder of the PowerShell scripts: ZT_DIGEST_SCRIPTS, else this repository's scripts\book-digest.
fn scripts_dir() -> String {
    if let Ok(dir) = std::env::var("ZT_DIGEST_SCRIPTS")
        && !dir.is_empty()
    {
        return dir.trim_end_matches(['\\', '/']).to_string();
    }
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = crate_dir.parent().and_then(Path::parent).unwrap_or(crate_dir);
    repo.join("scripts").join("book-digest").display().to_string()
}

/// `[IO.Path]::GetFullPath`.
fn full_path(path: &str) -> String {
    std::path::absolute(path).map(|p| p.display().to_string()).unwrap_or_else(|_| path.to_string())
}

/// The reads of one batch on one `zt mcp`, as `Invoke-ZtDirect` made them: the answers that are
/// JSON objects. Ok(Err(message)) when a call failed; Err when `zt mcp` itself failed.
fn read_batch(zt: &mut Zt, calls: &[Value]) -> Result<std::result::Result<Vec<Value>, String>> {
    let mut values = Vec::new();
    for (i, call) in calls.iter().enumerate() {
        let arguments = call.get("arguments").filter(|a| !a.is_null()).cloned().unwrap_or(json!({}));
        let reply = zt.request(&field_of(call, "tool"), arguments)?;
        let text = reply_text(&reply);
        if let Some(message) = reply_error(&reply) {
            return Ok(Err(format!("zt-mcp failed: {i} error {}", WS_NEWLINE.replace_all(&message, " "))));
        }
        if text.trim_start().starts_with('{') {
            match serde_json::from_str(&text) {
                Ok(value) => values.push(value),
                Err(e) => return Ok(Err(format!("Conversion from JSON failed with error: {e}"))),
            }
        }
    }
    Ok(Ok(values))
}

/// `Invoke-Zt` with reads only: a batch that hit a dead server runs again on a new one.
fn read_retry(zt: &mut Option<Zt>, calls: &[Value]) -> std::result::Result<Vec<Value>, String> {
    for attempt in 0..2 {
        if zt.is_none() || attempt == 1 {
            *zt = None;
            match crate::zt::start() {
                Ok(started) => *zt = Some(started),
                Err(e) => return Err(format!("{e:#}")),
            }
        }
        match read_batch(zt.as_mut().expect("started"), calls) {
            Ok(result) => return result,
            Err(e) if attempt == 1 => return Err(format!("{e:#}")),
            Err(_) => {}
        }
    }
    Err("zt mcp closed".into())
}

pub(super) struct Bridge {
    step: String,
    staging: String,
    key: String,
    work: String,
    profile: String,
    max_sources: i64,
    candidates: i64,
    books: Vec<String>,
    job: String,
    report_id: String,
    accept: String,
    accept_earlier: String,
    apply: bool,
    /// `project`: the selected immutable report and the Card its read-only projection answers for.
    report: String,
    at: String,
    plan_file: String,
    proposal_file: String,
    zt: Option<Zt>,
    tree_index: Option<Vec<Value>>,
}

impl Bridge {
    /// The one `zt mcp` of the run, started on first use.
    fn zt(&mut self) -> Result<&mut Zt> {
        if self.zt.is_none() {
            self.zt = Some(crate::zt::start()?);
        }
        Ok(self.zt.as_mut().expect("started"))
    }

    /// `Read-Zt`: reads only, every answer in call order. Above 40 calls, up to six workers read
    /// 40-call chunks at once, each on its own `zt mcp`; a chunk that fails fails the read.
    fn read_zt(&mut self, calls: &[Value]) -> Result<Vec<Value>> {
        for call in calls {
            let tool = field_of(call, "tool");
            if !READ_TOOLS.contains(&tool.as_str()) {
                bail!("Bridge read step refuses {tool}");
            }
        }
        if calls.is_empty() {
            return Ok(Vec::new());
        }
        if calls.len() > CHUNK {
            let chunks: Vec<(usize, &[Value])> = calls.chunks(CHUNK).enumerate().map(|(k, c)| (k * CHUNK, c)).collect();
            let workers = WORKERS.min(chunks.len());
            let next = std::sync::atomic::AtomicUsize::new(0);
            let work = |zt: &mut Option<Zt>| -> Vec<(usize, std::result::Result<Vec<Value>, String>)> {
                let mut got = Vec::new();
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((start, part)) = chunks.get(k) else { break };
                    got.push((*start, read_retry(zt, part)));
                }
                got
            };
            let main = &mut self.zt;
            let mut results: Vec<(usize, std::result::Result<Vec<Value>, String>)> = std::thread::scope(|scope| {
                let handles: Vec<_> = (1..workers)
                    .map(|_| {
                        scope.spawn(|| {
                            let mut own: Option<Zt> = None;
                            work(&mut own)
                        })
                    })
                    .collect();
                let mut all = work(main);
                for h in handles {
                    all.extend(h.join().unwrap_or_default());
                }
                all
            });
            results.sort_by_key(|(start, _)| *start);
            if results.len() != chunks.len() {
                bail!("incomplete Bridge read chunks");
            }
            let mut ordered = Vec::with_capacity(calls.len());
            for ((start, result), (_, part)) in results.into_iter().zip(&chunks) {
                match result {
                    Ok(values) if values.len() == part.len() => ordered.extend(values),
                    Ok(_) => bail!("incomplete Bridge read chunk {start}: "),
                    Err(error) => bail!("incomplete Bridge read chunk {start}: {error}"),
                }
            }
            return Ok(ordered);
        }
        let values = match read_retry(&mut self.zt, calls) {
            Ok(values) => values,
            Err(error) => bail!(error),
        };
        if values.len() != calls.len() {
            bail!("incomplete Bridge read");
        }
        Ok(values)
    }

    /// `Load-Plan`: plan.json, its schema, Citation key, and fingerprint.
    fn load_plan(&self) -> Result<Value> {
        let value = canon::read_json(&self.plan_file)?;
        if ne_one(get(&value, "schema")) || !ceq(get(&value, "citation_key"), &self.key) {
            return Err(refuse("invalid plan schema or Citation key"));
        }
        if !ceq(get(&value, "plan_id"), &format!("p-{}", canon::content_hash(&value, &["plan_id", "created_at"]))) {
            return Err(refuse("plan fingerprint does not match its frozen content"));
        }
        Ok(value)
    }

    /// `Ensure-PlanFiles`: the folders, the instructions, the empty allow-list, and every job file
    /// that is missing (a process can stop after saving plan.json but before rendering its inputs).
    fn ensure_plan_files(&self, plan: &Value) -> Result<()> {
        for dir in [self.work.clone(), ps::join_path(&self.work, "jobs"), ps::join_path(&self.work, "out"), ps::join_path(&self.work, "tmp")] {
            std::fs::create_dir_all(&dir)?;
        }
        let instructions = ps::join_path(&self.work, "dsh-bridge-instructions.zh.md");
        if !ps::exists(&instructions) {
            let from = ps::join_path(&scripts_dir(), "dsh-bridge-instructions.zh.md");
            if !ps::exists(&from) {
                bail!("Cannot find path '{from}' because it does not exist.");
            }
            std::fs::copy(&from, &instructions)?;
        }
        let allow = ps::join_path(&self.work, "read-only.allow.txt");
        if !ps::exists(&allow) {
            std::fs::write(&allow, "")?;
        }
        for entry in ps_items(get(plan, "jobs")) {
            let id = field_of(&entry, "id");
            let path = ps::join_path(&self.work, &format!("jobs\\{id}.md"));
            if ps::exists(&path) {
                continue;
            }
            util::write_text(&path, &self.job_text(plan, &entry))?;
        }
        Ok(())
    }

    /// The text of one judge job file.
    fn job_text(&self, plan: &Value, entry: &Value) -> String {
        let id = field_of(entry, "id");
        let cards = get(plan, "cards");
        let mut text = format!(
            "# Bridge job {id}\n\nPlan: {}\nProfile: {}\nSnapshot: {}\nOutput: {}\n",
            field_of(plan, "plan_id"),
            field_of(plan, "profile"),
            ps_literal(&self.plan_file),
            ps_literal(&ps::join_path(&self.work, &format!("out\\{id}.json")))
        );
        let mut targets: Vec<String> = Vec::new();
        for address in ps_items(get(entry, "sources")) {
            let address = s(&address);
            let source = cards.get(&address).cloned().unwrap_or(Value::Null);
            let candidates = ps_items(get(plan, "sources")).into_iter().find(|p| ceq(get(p, "address"), &address)).map(|p| get(&p, "candidates").clone()).unwrap_or(Value::Null);
            text.push_str(&format!(
                "\n## Source {address} — {} ({}, {})\n{}\n\nCandidates:\n",
                field_of(&source, "title"),
                field_of(&source, "type"),
                field_of(&source, "form"),
                field_of(&source, "body")
            ));
            for candidate in ps_items(&candidates) {
                let c = cards.get(field_of(&candidate, "address")).cloned().unwrap_or(Value::Null);
                let at = field_of(&c, "address");
                if !targets.contains(&at) {
                    targets.push(at.clone());
                }
                let via: Vec<String> = ps_items(get(&candidate, "via")).iter().map(s).collect();
                text.push_str(&format!(
                    "- {at} — {}; type={}; form={}; facet={}; workspace={}; book={}; via={}\n",
                    field_of(&c, "title"),
                    field_of(&c, "type"),
                    field_of(&c, "form"),
                    field_of(&c, "facet"),
                    field_of(&c, "workspace"),
                    field_of(&c, "book"),
                    via.join(",")
                ));
            }
        }
        let literals: Vec<String> = targets.iter().map(|t| ps_literal(t)).collect();
        text.push_str(&format!(
            "\nRead only this job's frozen candidate bodies (never relocate by title):\n```powershell\n$snapshot = Get-Content -LiteralPath {} -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String\n@({}) | ForEach-Object {{ $c = $snapshot.cards[$_]; @{{address=$c.address;workspace=$c.workspace;book=$c.book;type=$c.type;form=$c.form;facet=$c.facet;title=$c.title;body=$c.body}} | ConvertTo-Json -Depth 5 }}\n```\n",
            ps_literal(&self.plan_file),
            literals.join(",")
        ));
        text.push_str(&format!(
            "\nSELF-CHECK: zt-digest bridge -Step precheck -Staging {} -CitationKey {} -Work {} -Job {id}\n",
            ps_literal(&self.staging),
            ps_literal(&self.key),
            ps_literal(&self.work)
        ));
        text
    }

    /// `Plan-Result`: the plan step's last line; the jobs whose answer does not pass yet.
    fn plan_result(&self, plan: &Value) -> Result<Value> {
        let mut jobs: Vec<Value> = Vec::new();
        if !ps::exists(&self.proposal_file) {
            self.ensure_plan_files(plan)?;
            for entry in ps_items(get(plan, "jobs")) {
                let id = field_of(&entry, "id");
                let answer = ps::join_path(&self.work, &format!("out\\{id}.json"));
                if ps::exists(&answer)
                    && let Ok(check) = canon::read_json(&answer).and_then(|a| answer::test_answer(&self.work, &a, plan, &entry))
                    && check.faults.is_empty()
                {
                    continue;
                }
                jobs.push(json!({
                    "id": get(&entry, "id"),
                    "files": [ps::join_path(&self.work, "dsh-bridge-instructions.zh.md"), ps::join_path(&self.work, &format!("jobs\\{id}.md"))],
                    "prompt": JOB_PROMPT
                }));
            }
        }
        Ok(json!({ "ok": true, "step": "plan", "status": "done", "plan_id": get(plan, "plan_id"), "selected": plan["counts"]["selected"],
                   "no_candidates": ps_count(get(plan, "no_candidates")), "jobs": jobs }))
    }

    /// The options a plan freezes.
    fn options(&self) -> Value {
        json!({ "max_sources": self.max_sources, "candidates": self.candidates, "books": self.books })
    }

    /// A new plan: read the archive, pick and rank the sources, find their candidates, freeze them.
    fn plan(&mut self) -> Result<Value> {
        let listed = self.read_zt(&[json!({ "tool": "list_trees", "arguments": {} })])?;
        let trees = ps_array(&listed.first().cloned().unwrap_or(Value::Null)["trees"]);
        let calls: Vec<Value> = trees.iter().map(|t| json!({ "tool": "get_tree", "arguments": { "tree": get(t, "tree") } })).collect();
        let details = self.read_zt(&calls)?;
        let summaries: Vec<Value> = details.iter().flat_map(|d| ps_items(get(d, "cards"))).collect();
        let index = book_of::book_index(&trees, &summaries);
        if !index.book_set.contains(&self.key) {
            return Err(refuse("Citation key is not an existing agent Literature tree"));
        }
        for book in &self.books {
            if !index.book_set.contains(book) {
                return Err(refuse(format!("unknown target book {book}")));
            }
        }
        let mut by_address: HashMap<String, Value> = HashMap::new();
        for summary in &summaries {
            let address = field_of(summary, "address");
            if by_address.contains_key(&address) {
                return Err(refuse(format!("An item with the same key has already been added. Key: {address}")));
            }
            by_address.insert(address, summary.clone());
        }
        let book_of_card = |card: &Value| index.book_of.get(&field_of(card, "address")).cloned();
        let math = self.profile == "math";
        let mut priority: HashMap<String, i64> = HashMap::new();
        if math {
            let heads: Vec<Value> = summaries
                .iter()
                .filter(|c| {
                    ceq(get(c, "workspace"), "agent")
                        && book_of_card(c).as_deref() == Some(self.key.as_str())
                        && ceq(get(c, "type"), "model")
                        && !truthy(get(c, "facet"))
                        && truthy(get(c, "form"))
                })
                .map(|c| json!({ "tool": "get_model", "arguments": { "at": get(c, "address") } }))
                .collect();
            let reports = self.read_zt(&heads)?;
            for report in &reports {
                if ceq(get(report, "form"), "theory") {
                    let mut ats = ps_array(&report["math"]["main_results"]);
                    ats.extend(ps_array(&report["math"]["objects"]));
                    for at in ats {
                        if truthy(&at) {
                            priority.insert(s(&at), 0);
                        }
                    }
                }
            }
            for report in &reports {
                let at = s(&report["head"]["address"]);
                if priority.contains_key(&at) {
                    continue;
                }
                let load = &report["math"]["load"];
                if ceq(get(report, "form"), "definition") {
                    priority.insert(at, 1);
                } else if cin(get(report, "form"), &["axiom", "theorem", "proposition", "lemma", "corollary"]) && load.as_f64().is_some_and(|n| n >= 2.0) {
                    priority.insert(at, 2);
                }
            }
        }
        let mut ordered: Vec<(String, Value)> = Vec::new();
        let mut keys: HashSet<String> = HashSet::new();
        for card in &summaries {
            let address = field_of(card, "address");
            if !ceq(get(card, "workspace"), "agent")
                || book_of_card(card).as_deref() != Some(self.key.as_str())
                || !substantive(card, &index)
                || truthy(get(card, "facet"))
            {
                continue;
            }
            let key = if math {
                let Some(p) = priority.get(&address).filter(|_| answer::unit_head(card)) else { continue };
                format!("{p:02}|{address}")
            } else {
                if ceq(get(card, "type"), "idea") && index.book_set.contains(&field_of(card, "tree")) {
                    continue;
                }
                let inbound = if ceq(get(card, "type"), "idea") { i32::MAX as i64 - ps::int(get(card, "inbound"))? } else { 0 };
                format!("{:02}|{inbound:010}|{address}", source_rank(card))
            };
            if !keys.insert(key.clone()) {
                return Err(refuse(format!("An item with the same key has already been added. Key: {key}")));
            }
            ordered.push((key, card.clone()));
        }
        ordered.sort_by(|a, b| ordinal_cmp(&a.0, &b.0));
        let cut = (self.max_sources as usize).min(ordered.len());
        let sources: Vec<Value> = ordered[..cut].iter().map(|(_, c)| c.clone()).collect();
        let term_cards: Vec<Value> = summaries
            .iter()
            .filter(|c| ceq(get(c, "workspace"), "agent") && index.category.get(&field_of(c, "address")).is_some_and(|k| k == "terms"))
            .map(|c| json!({ "tool": "get_card", "arguments": { "at": get(c, "address") } }))
            .collect();
        let term_reads = self.read_zt(&term_cards)?;
        let mut term_tables = Map::new();
        let mut terms: Vec<String> = Vec::new();
        for card in &term_reads {
            let Some(book) = index.book_of.get(&field_of(card, "address")) else { continue };
            term_tables.insert(book.clone(), json!(field_of(card, "address")));
            if *book != self.key {
                continue;
            }
            for line in field_of(card, "body").split('\n') {
                if let Some(c) = TERM_ROW.captures(line) {
                    let term = c[1].trim().to_string();
                    if !TERM_RULE.is_match(&term) && !TERM_HEAD.is_match(&term) && !terms.contains(&term) {
                        terms.push(term);
                    }
                }
            }
        }
        let mut term_hits: HashMap<String, Vec<String>> = HashMap::new();
        let documents: Vec<(String, String)> =
            summaries.iter().map(|c| (field_of(c, "address"), format!("{} {} {}", field_of(c, "title"), field_of(c, "title"), field_of(c, "excerpt")))).collect();
        let relatedness = Relatedness::new(&documents, 80);
        let mut selected: Vec<Value> = Vec::new();
        let mut none: Vec<Value> = Vec::new();
        let mut wanted: Vec<String> = Vec::new();
        let add = |wanted: &mut Vec<String>, at: String| {
            if !wanted.contains(&at) {
                wanted.push(at);
            }
        };
        for source in &sources {
            let source_at = field_of(source, "address");
            let source_links = ps_array(get(source, "links"));
            let mut allowed: HashSet<String> = HashSet::new();
            for target in &summaries {
                let target_at = field_of(target, "address");
                let target_book = book_of_card(target);
                if target_at == source_at || target_book.as_deref() == Some(self.key.as_str()) || !substantive(target, &index) {
                    continue;
                }
                if ceq(get(source, "type"), "model") && truthy(get(target, "facet")) {
                    continue;
                }
                if let Some(book) = target_book.as_deref().filter(|b| !b.is_empty())
                    && !self.books.is_empty()
                    && !self.books.iter().any(|f| f == book)
                {
                    continue;
                }
                if source_links.iter().any(|l| ceq(l, &target_at)) || ps_array(get(target, "links")).iter().any(|l| ceq(l, &source_at)) {
                    continue;
                }
                allowed.insert(target_at);
            }
            let found = relatedness.find(&source_at, self.candidates as usize, |id| !allowed.contains(id));
            let mut candidates: Vec<(String, Vec<String>)> = found.iter().map(|t| (t.clone(), vec!["relatedness".to_string()])).collect();
            // Book terms the source uses, and the model's or unit's name: its title up to the first colon.
            let text = format!("{} {}", field_of(source, "title"), field_of(source, "excerpt"));
            let mut own: Vec<String> = Vec::new();
            for term in &terms {
                if ps::len16(term) >= 2 && canon::contains_ignore_case(&text, term) && !own.contains(term) {
                    own.push(term.clone());
                }
            }
            if ceq(get(source, "type"), "model") {
                let title = field_of(source, "title");
                let name = NAME_END.splitn(&title, 2).next().unwrap_or_default().trim().to_string();
                if ps::len16(&name) >= 2 && !own.contains(&name) {
                    own.push(name);
                }
            }
            let mut counts: Vec<(String, i64)> = Vec::new();
            for term in &own {
                let hits = term_hits.entry(term.clone()).or_insert_with(|| {
                    let mut hits: Vec<String> = Vec::new();
                    for card in &summaries {
                        let at = field_of(card, "address");
                        if canon::contains_ignore_case(&format!("{} {}", field_of(card, "title"), field_of(card, "excerpt")), term) && !hits.contains(&at) {
                            hits.push(at);
                        }
                    }
                    hits
                });
                for at in hits.iter() {
                    if allowed.contains(at) {
                        match counts.iter_mut().find(|(k, _)| k == at) {
                            Some((_, n)) => *n += 1,
                            None => counts.push((at.clone(), 1)),
                        }
                    }
                }
            }
            let mut matches: Vec<(String, String)> = Vec::new();
            for (at, count) in counts {
                if found.contains(&at) {
                    for candidate in candidates.iter_mut().filter(|(a, _)| *a == at) {
                        candidate.1.push("term".into());
                    }
                } else {
                    matches.push((format!("{:010}|{at}", i32::MAX as i64 - count), at));
                }
            }
            matches.sort_by(|a, b| ordinal_cmp(&a.0, &b.0));
            for (_, at) in matches.into_iter().take(2) {
                candidates.push((at, vec!["term".to_string()]));
            }
            add(&mut wanted, source_at.clone());
            for (at, _) in &candidates {
                add(&mut wanted, at.clone());
            }
            if candidates.is_empty() {
                none.push(json!(source_at));
            }
            let list: Vec<Value> = candidates.into_iter().map(|(at, via)| json!({ "address": at, "via": via })).collect();
            selected.push(json!({ "address": source_at, "candidates": list }));
        }
        let mut cards = Map::new();
        let calls: Vec<Value> = wanted.iter().map(|a| json!({ "tool": "get_card", "arguments": { "at": a } })).collect();
        for card in self.read_zt(&calls)? {
            let address = field_of(&card, "address");
            let summary = by_address.get(&address).cloned().unwrap_or(Value::Null);
            for name in ["workspace", "tree", "kind", "type", "facet", "emphasis", "level", "form", "title"] {
                if !ceqv(get(&card, name), get(&summary, name)) {
                    bail!("selected Card changed during planning: {address} {name}");
                }
            }
            let mut record = endpoint(&card, &index);
            record["kinds"] = get(&summary, "kinds").clone();
            cards.insert(address, record);
        }
        let mut settings: Vec<String> = Vec::new();
        for card in cards.values() {
            let setting = &card["model"]["build"]["setting"];
            if truthy(setting) && !cards.contains_key(&s(setting)) && !settings.contains(&s(setting)) {
                settings.push(s(setting));
            }
        }
        let calls: Vec<Value> = settings.iter().map(|a| json!({ "tool": "get_card", "arguments": { "at": a } })).collect();
        for card in self.read_zt(&calls)? {
            cards.insert(field_of(&card, "address"), endpoint(&card, &index));
        }
        for card in &term_reads {
            cards.insert(field_of(card, "address"), endpoint(card, &index));
        }
        let with: Vec<&Value> = selected.iter().filter(|s| ps_count(get(s, "candidates")) > 0).collect();
        let jobs: Vec<Value> = with
            .chunks(12)
            .enumerate()
            .map(|(k, part)| json!({ "id": format!("br-{:03}", k + 1), "sources": part.iter().map(|s| get(s, "address").clone()).collect::<Vec<_>>() }))
            .collect();
        // book_of in the order the index assigned it: the summaries' order.
        let mut book_of = Map::new();
        for card in &summaries {
            let address = field_of(card, "address");
            if let Some(book) = index.book_of.get(&address)
                && !book_of.contains_key(&address)
            {
                book_of.insert(address, json!(book));
            }
        }
        let omitted: Vec<Value> = ordered[cut..].iter().map(|(_, c)| json!({ "address": get(c, "address"), "title": get(c, "title") })).collect();
        let book_title = summaries.iter().find(|c| field_of(c, "address") == self.key).map(|c| field_of(c, "title")).unwrap_or_default();
        let empty_reason = if !sources.is_empty() {
            ""
        } else if math {
            "No qualifying mathematical unit head: theory main results/objects, definitions, or units with load at least two."
        } else {
            "No eligible substantive source Card for this book."
        };
        let mut plan = json!({
            "schema": 1, "citation_key": self.key, "profile": self.profile, "options": self.options(),
            "counts": { "eligible": ordered.len(), "selected": sources.len(), "omitted": ordered.len() - sources.len() },
            "cards": cards, "book_of": book_of, "term_tables": term_tables, "sources": selected, "jobs": jobs, "no_candidates": none,
            "omitted_sources": omitted, "book_title": book_title,
            "read_coverage": { "trees": trees.len(), "summaries": summaries.len(), "endpoints": wanted.len(), "term_tables": term_reads.len(),
                               "model_settings": settings.len(), "complete": true },
            "empty_reason": empty_reason
        });
        plan["plan_id"] = json!(format!("p-{}", canon::hash_value(&plan)));
        plan["created_at"] = json!(canon::utc_now_o());
        for dir in [self.work.clone(), ps::join_path(&self.work, "jobs"), ps::join_path(&self.work, "out"), ps::join_path(&self.work, "tmp")] {
            std::fs::create_dir_all(&dir)?;
        }
        canon::save_json(&self.plan_file, &plan)?;
        self.plan_result(&plan)
    }

    /// The steps after the parameters are bound (the script's `try` block). Returns the lines
    /// to print and the exit code.
    fn run(&mut self, lines: &mut Vec<String>) -> Result<i32> {
        if self.step == "project" {
            let projected = self.project()?;
            lines.push(ps::json_compact(&projected));
            return Ok(0);
        }
        self.staging = full_path(&self.staging);
        if self.work.is_empty() {
            self.work = ps::join_path(&self.staging, "bridge");
        }
        self.work = full_path(&self.work);
        self.plan_file = ps::join_path(&self.work, "plan.json");
        self.proposal_file = ps::join_path(&self.work, "proposals.json");
        let manifest = ps::join_path(&self.staging, "manifest.json");
        if ps::exists(&manifest) && !ceq(get(&canon::read_json(&manifest)?, "citation_key"), &self.key) {
            return Err(refuse("manifest Citation key disagrees with request"));
        }
        if self.step == "precheck" {
            let plan = self.load_plan()?;
            if !JOB_ID.is_match(&self.job) {
                return Err(refuse("invalid job id"));
            }
            let assignment: Vec<Value> = ps_items(get(&plan, "jobs")).into_iter().filter(|j| ceq(get(j, "id"), &self.job)).collect();
            if assignment.len() != 1 {
                return Err(refuse("job is not in the frozen plan"));
            }
            let answer = canon::read_json(&ps::join_path(&self.work, &format!("out\\{}.json", self.job)))?;
            let check = answer::test_answer(&self.work, &answer, &plan, &assignment[0])?;
            let n = check.faults.len();
            lines.extend(check.faults);
            lines.push(format!("faults: {n}"));
            lines.push(ps::json_compact(&json!({ "ok": n == 0, "step": "precheck", "job": self.job, "faults": n, "status": if n > 0 { "failed" } else { "done" } })));
            return Ok(if n > 0 { 1 } else { 0 });
        }
        if self.step == "report" {
            let published = if ps::exists(&self.proposal_file) { self.load_published()? } else { self.publish_report(&self.load_plan()?)? };
            let state = self.get_state(&published)?;
            self.render_report(&published, &state)?;
            lines.push(ps::json_compact(&self.report_summary(&published, &state, "report")));
            return Ok(0);
        }
        if self.step == "apply" {
            let published = self.load_published()?;
            let chosen = self.select_proposals(&published)?;
            let state = self.get_state(&published)?;
            if self.apply {
                // The script also read the preview's context here and did not use it with -Apply:
                // apply reads its own context under the lock, so the tool reads the trees once less.
                let state = self.apply_selected(&chosen, &published)?;
                lines.push(ps::json_compact(&self.report_summary(&published, &state, "apply")));
                return Ok(0);
            }
            let mut wanted: Vec<String> = Vec::new();
            for bridge in &chosen {
                for at in [field_of(bridge, "carrier"), field_of(bridge, "target")] {
                    if !wanted.contains(&at) {
                        wanted.push(at);
                    }
                }
            }
            let ctx = self.current_context(&wanted)?;
            let outcomes = self.selection_states(&chosen, &published, &state, &ctx);
            let mut summary = self.report_summary(&published, &state, "apply");
            summary["preview"] = json!(true);
            let blocked = outcomes.iter().any(|o| cin(get(o, "status"), &["stale", "conflict", "written_unverified"]));
            summary["outcomes"] = Value::Array(outcomes);
            if blocked {
                summary["ok"] = json!(false);
                summary["status"] = json!("partial");
            }
            lines.push(ps::json_compact(&summary));
            return Ok(0);
        }
        if ps::exists(&self.plan_file) {
            let plan = self.load_plan()?;
            if !ceq(get(&plan, "citation_key"), &self.key)
                || !ceq(get(&plan, "profile"), &self.profile)
                || canon::canonical_json(get(&plan, "options")) != canon::canonical_json(&self.options())
            {
                return Err(refuse("frozen plan inputs differ; use a fresh output folder"));
            }
            lines.push(ps::json_compact(&self.plan_result(&plan)?));
            return Ok(0);
        }
        let result = self.plan()?;
        lines.push(ps::json_compact(&result));
        Ok(0)
    }

    /// `Project-Context`: the endpoint Cards of one projection and the book index it compares
    /// them against — the tree list (which trees are agent Literature trees) and each endpoint's
    /// own Card. It deliberately does not read a whole tree's summaries the way an apply does: a
    /// big book tree can exceed the MCP reply bound, and a projection needs only each endpoint's
    /// own attributed book and its own fields.
    fn projection_context(&mut self, addresses: &[String]) -> Result<(HashMap<String, Value>, BookIndex)> {
        let trees = match self.tree_index.clone() {
            Some(trees) => trees,
            None => {
                let read = self.read_zt(&[json!({ "tool": "list_trees", "arguments": {} })])?;
                let trees = ps_array(&read.first().cloned().unwrap_or(Value::Null)["trees"]);
                self.tree_index = Some(trees.clone());
                trees
            }
        };
        let calls: Vec<Value> = addresses.iter().map(|at| json!({ "tool": "get_card", "arguments": { "at": at } })).collect();
        let mut cards: HashMap<String, Value> = HashMap::new();
        match self.read_zt(&calls) {
            Ok(reads) if reads.len() == calls.len() => {
                for (at, card) in addresses.iter().zip(reads) {
                    cards.insert(at.clone(), card);
                }
            }
            _ => {
                // One endpoint may be gone: read each Card alone, so a missing one stays a missing
                // endpoint instead of failing the whole projection.
                for call in &calls {
                    if let Ok(mut read) = self.read_zt(std::slice::from_ref(call))
                        && let Some(card) = read.pop()
                    {
                        cards.insert(field_of(&card, "address"), card);
                    }
                }
            }
        }
        let mut summaries: Vec<Value> = Vec::new();
        for (at, card) in &cards {
            let links: Vec<Value> = ps_array(get(card, "outbound")).iter().filter(|out| !truthy(get(out, "broken"))).map(|out| get(out, "address").clone()).collect();
            summaries.push(json!({
                "address": at,
                "tree": get(card, "tree"),
                "workspace": get(card, "workspace"),
                "kind": get(card, "kind"),
                "type": get(card, "type"),
                "links": links,
            }));
        }
        let index = book_of::book_index(&trees, &summaries);
        Ok((cards, index))
    }

    /// `Project-Report`: the read-only nearby-candidate projection of one selected immutable
    /// report (§10.3, D258; the GUI's optional distant source, S20-AC02/AC04/AC05). It re-reads
    /// the report's own endpoints through `zt mcp`, reuses the report-id schema check and the
    /// apply path's endpoint fingerprints, and returns only the proposals that name the requested
    /// Card and are still current. It writes no Card, runs no judge, and publishes no report.
    fn project(&mut self) -> Result<Value> {
        let published = canon::read_json(&self.report)
            .map_err(|e| refuse(format!("the selected report cannot be read: {e:#}")))?;
        report::validate_published(&published)?;
        let at = self.at.trim().to_string();
        if at.is_empty() || at.chars().count() > 512 {
            return Err(refuse("invalid Card address for the projection"));
        }
        let report_id = get(&published, "report_id").clone();
        let snapshots = get(&published, "cards");
        let proposals = ps_items(get(&published, "proposals"));
        let mut named: Vec<(String, Value)> = Vec::new();
        let mut total = 0usize;
        for proposal in &proposals {
            let carrier = field_of(proposal, "carrier");
            let target = field_of(proposal, "target");
            let other = if carrier == at {
                Some(target)
            } else if target == at {
                Some(carrier)
            } else {
                None
            };
            let Some(other) = other.filter(|other| !other.is_empty()) else { continue };
            total += 1;
            if named.len() < PROJECT_MAX {
                named.push((other, proposal.clone()));
            }
        }
        let mut candidates: Vec<Value> = Vec::new();
        let mut stale: Vec<Value> = Vec::new();
        if !named.is_empty() {
            let mut addresses = vec![at.clone()];
            for (other, _) in &named {
                if !addresses.contains(other) {
                    addresses.push(other.clone());
                }
            }
            let (cards, index) = self.projection_context(&addresses)?;
            for (other, proposal) in &named {
                let mut why = None;
                for address in [&at, other] {
                    let Some(card) = cards.get(address) else {
                        why = Some(format!("missing endpoint {address}"));
                        break;
                    };
                    if let Some(reason) = changed_endpoint(card, snapshots.get(address), &index) {
                        why = Some(reason);
                        break;
                    }
                }
                let Some(why) = why else {
                    candidates.push(json!({
                        "report": report_id,
                        "proposal": get(proposal, "id"),
                        "target": other,
                        "relation": get(proposal, "kind"),
                        "sentence": get(proposal, "sentence"),
                        "section": get(proposal, "section"),
                        "carrier": get(proposal, "carrier"),
                        "focus": if ceq(get(proposal, "carrier"), &at) { "carrier" } else { "target" },
                    }));
                    continue;
                };
                stale.push(json!({ "proposal": get(proposal, "id"), "target": other, "why": why }));
            }
        }
        let status = if candidates.is_empty() {
            if named.is_empty() { "not_in_report" } else { "stale" }
        } else {
            "done"
        };
        let counts = json!({
            "proposals": proposals.len(),
            "at": total,
            "current": candidates.len(),
            "stale": stale.len(),
            "omitted": total.saturating_sub(named.len()),
        });
        Ok(json!({
            "ok": true,
            "step": "project",
            "status": status,
            "report_id": report_id,
            "citation_key": get(&published, "citation_key"),
            "at": at,
            "candidates": candidates,
            "stale": stale,
            "counts": counts,
            "bounds": { "candidates_max": PROJECT_MAX },
        }))
    }
}

/// `zt-digest bridge`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(&util::bind_positional(argv, &PARAMS)?, &PARAMS, &["Apply"])?;
    // `project` is the read-only projection of one selected report: it takes that report and the
    // Card to answer for instead of a staging folder and a Citation key (§10.3, D258).
    let project = a.str("Step") == "project";
    let mut bridge = Bridge {
        step: a.str("Step"),
        staging: a.str("Staging"),
        key: a.str("CitationKey"),
        work: a.str("Work"),
        profile: a.str_or("Profile", "book"),
        max_sources: 0,
        candidates: 0,
        books: Vec::new(),
        job: a.str("Job"),
        report_id: a.str("ReportId"),
        accept: a.str("Accept"),
        accept_earlier: a.str("AcceptEarlier"),
        apply: a.flag("Apply"),
        report: a.str("Report"),
        at: a.str("At"),
        plan_file: String::new(),
        proposal_file: String::new(),
        zt: None,
        tree_index: None,
    };
    let required: &[&str] = if project { &["Step", "Report", "At"] } else { &["Step", "Staging", "CitationKey"] };
    let mut lines: Vec<String> = Vec::new();
    let options = (|| -> Result<()> {
        for name in required {
            if a.required(name)?.is_empty() {
                bail!("Cannot bind argument to parameter '{name}' because it is an empty string.");
            }
        }
        if !["plan", "precheck", "report", "apply", "project"].contains(&bridge.step.as_str()) {
            return Err(refuse("unknown Bridge step"));
        }
        if project {
            return Ok(());
        }
        if !["book", "math"].contains(&bridge.profile.as_str()) {
            return Err(refuse("unknown Bridge profile"));
        }
        bridge.max_sources = canon::integer_option(&a.str_or("MaxSources", "60"), "MaxSources", 1, 120)?;
        bridge.candidates = canon::integer_option(&a.str_or("Candidates", "5"), "Candidates", 1, 8)?;
        let mut books: Vec<String> = a.str("Books").split(',').map(|b| b.trim().to_string()).filter(|b| !b.is_empty()).collect();
        books.sort_by(|x, y| ordinal_cmp(x, y));
        bridge.books = books;
        Ok(())
    })();
    let result = options.and_then(|()| bridge.run(&mut lines));
    let code = match result {
        Ok(code) => code,
        Err(error) => {
            let text = message(&error);
            // A projection's input refusal is a report status the reader can show and fall back
            // from (S20-AC06); an operational failure is its own status.
            let status = if project && is_refusal(&error) { "invalid_report" } else { "failed" };
            let mut failure = json!({ "ok": false, "step": bridge.step, "status": status, "errors": [text] });
            if bridge.step == "precheck" {
                lines.push(format!("BR1 {}", LINE_BREAKS.replace_all(&text, " ")));
                lines.push("faults: 1".into());
                failure["faults"] = json!(1);
            }
            lines.push(ps::json_compact(&failure));
            if is_refusal(&error) { 1 } else { 2 }
        }
    };
    drop(bridge);
    util::print_lines(&lines)?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_fingerprint_their_fields() {
        let trees = vec![json!({ "tree": "Old", "kind": "literature", "workspace": "agent" })];
        let summary = json!({ "address": "Old/1|a", "tree": "Old", "kind": "regular", "type": "idea", "workspace": "agent", "links": [] });
        let index = book_of::book_index(&trees, &[summary]);
        let card = json!({ "address": "Old/1|a", "workspace": "agent", "tree": "Old", "kind": "regular", "type": "idea", "title": "标题",
                           "body": "a\r\nb", "outbound": [{ "address": "0/1" }, { "address": "0/2", "broken": true }], "model": null });
        let e = endpoint(&card, &index);
        assert_eq!(e["body"], json!("a\nb"));
        assert_eq!(e["category"], json!("idea"));
        assert_eq!(e["book"], json!("Old"));
        assert_eq!(e["links"], json!(["0/1"]));
        let keys: Vec<&String> = e.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["address", "workspace", "tree", "kind", "type", "facet", "emphasis", "level", "form", "title", "body", "category", "book", "fingerprint", "links", "model"]);
        let values = json!(["agent", "Old", "regular", "idea", null, null, null, null, "标题", "a\nb"]);
        assert_eq!(e["fingerprint"], json!(format!("sha256:{}", canon::sha256_hex(ps::json_compact(&values).as_bytes()))));
    }

    #[test]
    fn a_projected_endpoint_is_stale_when_its_snapshot_changed() {
        let index = BookIndex { category: HashMap::new(), book_of: HashMap::new(), books: Vec::new(), book_set: HashSet::new() };
        let card = json!({ "address": "Old/1", "workspace": "agent", "tree": "Old", "kind": "regular", "type": "idea",
                           "facet": null, "emphasis": null, "level": null, "form": null, "title": "标题", "body": "正文",
                           "outbound": [], "model": null });
        let snapshot = endpoint(&card, &index);
        assert_eq!(changed_endpoint(&card, Some(&snapshot), &index), None);
        let mut changed = card.clone();
        changed["title"] = json!("另一个标题");
        assert_eq!(changed_endpoint(&changed, Some(&snapshot), &index), Some("changed endpoint Old/1 title".into()));
        let mut changed = card.clone();
        changed["body"] = json!("另一段正文");
        assert_eq!(changed_endpoint(&changed, Some(&snapshot), &index), Some("changed body or endpoint Old/1".into()));
        // Generated Reverse-link churn changes neither the authored body nor the fingerprint.
        let mut reverse = card.clone();
        reverse["inbound"] = json!([{ "address": "0/9" }]);
        assert_eq!(changed_endpoint(&reverse, Some(&snapshot), &index), None);
        assert_eq!(changed_endpoint(&card, None, &index), Some("missing endpoint Old/1 in the report".into()));
    }

    #[test]
    fn job_files_name_the_tool_as_their_self_check() {
        let bridge = Bridge {
            step: "plan".into(),
            staging: "D:\\s".into(),
            key: "New2026".into(),
            work: "D:\\s\\bridge".into(),
            profile: "book".into(),
            max_sources: 60,
            candidates: 5,
            books: Vec::new(),
            job: String::new(),
            report_id: String::new(),
            accept: String::new(),
            accept_earlier: String::new(),
            apply: false,
            report: String::new(),
            at: String::new(),
            plan_file: "D:\\s\\bridge\\plan.json".into(),
            proposal_file: String::new(),
            zt: None,
            tree_index: None,
        };
        let plan = json!({ "plan_id": "p-1", "profile": "book",
            "cards": { "0/1": { "address": "0/1", "title": "源", "type": "idea", "form": null, "body": "正文" },
                       "Old/1|a": { "address": "Old/1|a", "title": "候选", "type": "idea", "workspace": "agent", "book": "Old" } },
            "sources": [{ "address": "0/1", "candidates": [{ "address": "Old/1|a", "via": ["relatedness", "term"] }] }] });
        let text = bridge.job_text(&plan, &json!({ "id": "br-001", "sources": ["0/1"] }));
        assert!(text.starts_with("# Bridge job br-001\n\nPlan: p-1\nProfile: book\nSnapshot: 'D:\\s\\bridge\\plan.json'\nOutput: 'D:\\s\\bridge\\out\\br-001.json'\n"));
        assert!(text.contains("\n## Source 0/1 — 源 (idea, )\n正文\n\nCandidates:\n- Old/1|a — 候选; type=idea; form=; facet=; workspace=agent; book=Old; via=relatedness,term\n"));
        assert!(text.contains("```powershell\n$snapshot = Get-Content -LiteralPath 'D:\\s\\bridge\\plan.json' -Raw -Encoding utf8 | ConvertFrom-Json -AsHashtable -DateKind String\n@('Old/1|a') | ForEach-Object { $c = $snapshot.cards[$_];"));
        assert!(text.ends_with("\nSELF-CHECK: zt-digest bridge -Step precheck -Staging 'D:\\s' -CitationKey 'New2026' -Work 'D:\\s\\bridge' -Job br-001\n"));
    }

    #[test]
    fn values_compare_as_powershell_compares_them() {
        assert!(ceq(&json!("a"), "a") && !ceq(&json!("A"), "a") && !ceq(&Value::Null, ""));
        assert!(ceqv(&Value::Null, &Value::Null) && !ceqv(&json!(""), &Value::Null));
        assert!(!ne_one(&json!(1)) && !ne_one(&json!(1.0)) && ne_one(&json!("x")) && ne_one(&Value::Null));
        assert_eq!(source_rank(&json!({ "type": "model", "level": "formal" })), 0);
        assert_eq!(source_rank(&json!({ "type": "event" })), 5);
    }
}
