//! `zt-digest math`: the port of `scripts/book-digest/math-compiler.ps1`, the deterministic half of
//! the zt-math-digest workflow (docs/math-cards-design.md). DeepSeek writes every Card body and
//! every JSON answer; this tool finds the book's numbered statements, builds the skeleton of every
//! mathematical unit and Theory Card, writes the job files, applies the answers, and checks the
//! result. It never writes Card prose except skeletons, lists, and Links.
//!
//!   index            the statement finder's jobs: one DeepSeek job per -IndexChars of prepared sections,
//!                    each section with numbered lines (files only) -> math\index\jobs.json
//!   index-check      (-Job) the self-check of one finder answer: one fault per line, then `faults: N`
//!   index-apply      every checked answer -> math\index.json, math\index.md (files only)
//!   docs             the math prompt files (each shared instruction file with its math supplement)
//!   skeleton         (-Apply) one unit Model Card per indexed statement, one Theory Card per chapter,
//!                    the group files' unit lists (writes with -Log owned-gNN.txt, -SrcLog src-gNN.txt)
//!   notation         the notation job; notation-apply (-Apply) writes the Book notation table
//!   boundary         one job per -UnitsPerJob units; boundary-apply creates the parts and prints the
//!                    review jobs; boundary-settle deletes each supplied part a review judged wrong
//!   proof            one review job per -UnitsPerJob formal results; proof-apply writes the fixes
//!                    that keep every Link
//!   theory           one job per four Theory Cards; theory-apply rewrites each Theory Card
//!   check            the math rules M1 to M18; math-audit.json and math-audit.md in -Out
//!
//! Every step prints compact JSON as its last line. One process holds one `zt mcp`; reads go as
//! `Invoke-Zt` sent them (its direct path), writes through `calls::run` (the helper) with the
//! options the script gave the helper. The instruction files `dsh-math-*.zh.md` are read from,
//! and named in job files as, ZT_DIGEST_SCRIPTS (the forwarder sets it) or the scripts\book-digest
//! folder of the repository this binary was built from.

mod index;
mod source;
mod text;

use crate::args::Args;
use crate::calls;
use crate::model::{read_calls, read_one};
use crate::ps::{self, CiMap, NL, eq, field, is_in, join, pipe, s, truthy};
use crate::util;
use crate::zt::Zt;
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::path::Path;
use std::sync::LazyLock;
use text::{RESULTS, dependency_map, form_zh, order_key, section_of, set_section};

const STEPS: &[&str] = &[
    "index",
    "index-check",
    "index-apply",
    "docs",
    "skeleton",
    "notation",
    "notation-apply",
    "boundary",
    "boundary-apply",
    "boundary-settle",
    "proof",
    "proof-apply",
    "theory",
    "theory-apply",
    "check",
];
const PARAMS: &[&str] = &["Step", "Staging", "Out", "UnitsPerJob", "Supply", "MainResults", "CoreBudget", "MaxSymbols", "Job", "IndexChars"];

static OK_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+ ok ").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static NOTATION_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(记作|记为|记成|表示|令|设|定义为|denote)").unwrap());
static EXAMPLE_FACET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(例子|example)").unwrap());
static NON_EXAMPLE_FACET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(非例|non-example)").unwrap());
static ROLE_FACET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^假设作用[：:]\s*(H\d+)").unwrap());
static H_LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(H\d+)").unwrap());
static STARTS_H: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^H").unwrap());
static STARTS_C: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^C").unwrap());
static PROOF_BODY_FACET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(证明|推导|前提)").unwrap());
static SUPPLIED_MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)补：书中未给").unwrap());
static COUNTER_FACET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^反例").unwrap());
// The steps the proof review owns: the proof plan, each 证明 step and the conclusion. Premises are its context and
// derivations are the compiler's independent reviewer's (dsh-math-proof-review.zh.md; thompson pilot, 2026-11-22).
static PROOF_STEP_FACET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(证明|结论|proof|conclusion)").unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]]+)\]\]").unwrap());
static TODO_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)（待写）$").unwrap());
static NOTATION_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^符号表").unwrap());
static M5_FACET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(证明|推导|结论|假设作用|proof|derivation|conclusion|hypothesis)").unwrap());
static PLAN_FACET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(证明思路|proof plan)").unwrap());
static METHODS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)直接|反证|逆否|归纳|构造|分情形|对角线|direct|contradiction|contrapositive|induction|construction|cases|diagonal").unwrap()
});
static CONTRAST_FACET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(反例|非例|例子)").unwrap());
static MARKED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)（补：书中(未给|略去|未写)）").unwrap());
static EXERCISE_FACET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(习题|exercise)").unwrap());
static ANSWER_HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?im)^##\s*答案").unwrap());
static IDEA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)\*\*思路\*\*[：:]?(.*?)(\n\s*\n|\*\*|$)").unwrap());
static INLINE_MATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$[^$]*\$").unwrap());
static SENTENCE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[。！？]").unwrap());

struct Params {
    step: String,
    staging: String,
    out: String,
    units_per_job: i64,
    supply: String,
    main_results: i64,
    max_symbols: i64,
    job: String,
    index_chars: i64,
    apply: bool,
}

struct Compiler {
    p: Params,
    zt: Option<Zt>,
    manifest: Value,
    key: String,
    title: String,
    work: String,
}

/// What the proof review owes one indexed unit.
#[derive(Debug)]
pub(crate) enum ProofDecision {
    /// Review these main-line premise, proof and conclusion steps.
    Review(Vec<Value>),
    /// The unit owes no proof review: not a result, a concept-level result, or a result the
    /// source states without proof.
    Skip,
    /// The source proves the result and the unit holds no proof step (or was never built): an
    /// obligation the run must meet, never an empty queue (docs/math-digest-redesign-proposal.md, Bug C).
    Unmet(String),
}

/// The proof obligation of one indexed unit (`units.json` entry) given its current model read.
pub(crate) fn proof_decision(unit: &Value, model: &Value) -> ProofDecision {
    if !is_in(&field(unit, "form"), RESULTS) {
        return ProofDecision::Skip;
    }
    let proved_at = unit["proof"].as_i64().unwrap_or(0);
    if !truthy(model) || model["head"].is_null() {
        return if proved_at > 0 {
            ProofDecision::Unmet(format!("the source proves it from line {proved_at}; the unit was never built"))
        } else {
            ProofDecision::Skip
        };
    }
    if !eq(&field(model, "level"), "formal") {
        return ProofDecision::Skip;
    }
    let main: Vec<Value> = pipe(&model["build"]["order"]).into_iter().filter(|o| eq(&field(o, "line"), "main")).cloned().collect();
    let steps: Vec<Value> = main.iter().filter(|o| PROOF_STEP_FACET.is_match(&field(o, "facet"))).cloned().collect();
    // Premises, a plan and a conclusion frame a proof; only a 证明 or 推导 step is one.
    let proves = main.iter().any(|st| {
        let facet = field(st, "facet");
        let lower = facet.to_lowercase();
        (facet.starts_with("证明") && !PLAN_FACET.is_match(&facet))
            || facet.starts_with("推导")
            || (lower.starts_with("proof") && !lower.starts_with("proof plan"))
            || lower.starts_with("derivation")
    });
    if proves && !steps.is_empty() {
        ProofDecision::Review(steps)
    } else if proves {
        // A bare derivation proves the result; the compiler's reviewer has checked it.
        ProofDecision::Skip
    } else if proved_at > 0 {
        ProofDecision::Unmet(format!("the source proves it from line {proved_at}; the unit has no proof step"))
    } else {
        ProofDecision::Skip
    }
}

/// Whether a unit may take boundary work: its model is built and its head is written. Examples and
/// contrasts never go onto a 待写 skeleton (docs/math-digest-redesign-proposal.md, Bug C).
pub(crate) fn boundary_ready(model: &Value) -> bool {
    truthy(model)
        && truthy(&model["math"])
        && !crate::report::is_placeholder(&field(&model["head"], "title"), &field(&model["head"], "body"))
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
    repo.join("scripts").join("book-digest").to_string_lossy().into_owned()
}

/// One instruction file of the scripts folder.
fn script_path(name: &str) -> String {
    join(&scripts_dir(), name)
}

/// `Read-Json`: ConvertFrom-Json -AsHashtable (keys keep their case; an empty file is $null).
fn read_json(path: &str) -> Result<Value> {
    let raw = util::read_text(path)?;
    ps::parse_json(&raw).map_err(|e| anyhow::anyhow!("Conversion from JSON failed with error: {e}"))
}

fn save_json(path: &str, value: &Value) -> Result<()> {
    util::write_text(path, &ps::json_pretty(value))
}

fn out_last(value: &Value) -> Result<()> {
    util::print_line(&ps::json_compact(value))
}

/// `$a -eq [int]$n` with a value read from JSON on the left: a number as a number, text as text.
fn num_eq(v: &Value, n: i64) -> bool {
    match v {
        Value::Number(x) => x.as_i64().map_or_else(|| x.as_f64() == Some(n as f64), |i| i == n),
        Value::String(t) => eq(t, &n.to_string()),
        Value::Bool(b) => i64::from(*b) == n,
        _ => false,
    }
}

/// `"$(if ($u.name) { "（$($u.name)）" })"`.
fn name_part(u: &Value) -> String {
    if truthy(&u["name"]) { format!("（{}）", field(u, "name")) } else { String::new() }
}

fn obj(pairs: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (k, v) in pairs {
        map.insert(k.to_string(), v);
    }
    Value::Object(map)
}

fn counts(map: &CiMap<i64>) -> Value {
    Value::Object(map.iter().map(|(k, v)| (k.clone(), json!(v))).collect())
}

fn bump(map: &mut CiMap<i64>, k: &str) {
    let n = map.get(k).copied().unwrap_or(0) + 1;
    map.insert(k, n);
}

/// The prepared sections of a book in prepared group order, each with its group, chapter and lines.
pub(crate) fn prepared_sections(staging: &str, manifest: &Value) -> Result<Vec<index::Section>> {
    let mut out = Vec::new();
    for g in pipe(&manifest["groups"]) {
        for file in pipe(&g["sections"]) {
            let file = s(file);
            let path = join(staging, &format!("sections\\{file}"));
            if !ps::exists(&path) {
                bail!("a prepared section is missing: {path}");
            }
            out.push(index::Section { file, group: ps::int(&g["n"])?, chapter: field(g, "chapter"), lines: ps::read_lines(&path)? });
        }
    }
    Ok(out)
}

/// The finder's jobs: whole groups, in order, until a job holds -IndexChars characters.
fn index_jobs(sections: &[index::Section], max_chars: usize) -> Vec<Vec<index::Section>> {
    let mut jobs: Vec<Vec<index::Section>> = Vec::new();
    let mut cur: Vec<index::Section> = Vec::new();
    let mut chars = 0usize;
    let mut i = 0;
    while i < sections.len() {
        let group = sections[i].group;
        let mut j = i;
        let mut size = 0usize;
        while j < sections.len() && sections[j].group == group {
            size += sections[j].lines.iter().map(|l| l.chars().count() + 1).sum::<usize>();
            j += 1;
        }
        if !cur.is_empty() && chars + size > max_chars {
            jobs.push(std::mem::take(&mut cur));
            chars = 0;
        }
        cur.extend_from_slice(&sections[i..j]);
        chars += size;
        i = j;
    }
    if !cur.is_empty() {
        jobs.push(cur);
    }
    jobs
}

/// The statement index of a prepared book, built from the finder's retained answers
/// (`math\index\plan.json` and each job's `out\<id>.json`). Each answer must pass its check
/// against the sections its job was given; a missing or failing answer is an error, never an
/// empty index. `index-apply` writes this, and the ledger re-derives it from the same files.
pub(crate) fn retained_index(staging: &str, manifest: &Value) -> Result<(Vec<Value>, Vec<Value>)> {
    let sections = prepared_sections(staging, manifest)?;
    let work = join(staging, "math\\index");
    let plan = read_json(&join(&work, "plan.json"))?;
    let mut answers = Vec::new();
    let mut faults = Vec::new();
    for job in pipe(&plan["jobs"]) {
        let id = field(job, "id");
        let files: Vec<String> = pipe(&job["sections"]).into_iter().map(s).collect();
        let mine: Vec<index::Section> = sections.iter().filter(|x| files.iter().any(|f| eq(f, &x.file))).cloned().collect();
        let file = join(&work, &format!("out\\{id}.json"));
        if !ps::exists(&file) {
            faults.push(format!("{id}: no answer"));
            continue;
        }
        let answer = match read_json(&file) {
            Ok(a) => a,
            Err(e) => {
                faults.push(format!("{id}: the answer is not JSON ({e})"));
                continue;
            }
        };
        let f = index::check_answer(&answer, &mine);
        if !f.is_empty() {
            faults.push(format!("{id}: {} faults ({})", f.len(), f[0]));
            continue;
        }
        answers.push(answer);
    }
    if !faults.is_empty() {
        bail!("the statement finder's answers are not complete: {}", faults.join("; "));
    }
    Ok(index::build(&answers, &sections))
}

/// The unit-list block the skeleton appends to a prepared group file: one entry per
/// addressed unit of that group, then its examples and exercises. The prepared group
/// file is frozen up to this block's marker, so the ledger verifies the block itself
/// against the same index and unit list the compiler worked from (R238).
pub(crate) fn group_extension_marker() -> &'static str {
    "## 本组的数学单元"
}

/// Whether a part's facet is one the proof review must cover: a premise, a proof step or
/// a conclusion. The ledger re-derives the selected steps with this predicate, so a
/// review cannot cover a proof the unit no longer has (R240).
pub(crate) fn is_proof_step(facet: &str) -> bool {
    PROOF_STEP_FACET.is_match(facet)
}

/// One labelled section of a Card body as it stands now (R241).
pub(crate) fn body_section(body: &str, name: &str) -> String {
    text::section_text(body, name)
}

/// The math prompt files: each shared instruction file followed by its math supplement,
/// with this prepared run's key, title, staging folder and index filled in. `step_docs`
/// writes them; the ledger's `math-docs` check re-derives the same bytes, so a retained
/// prompt file cannot drift from the instruction files it claims to be (R238).
pub(crate) fn docs_files(
    key: &str,
    title: &str,
    staging: &str,
    manifest: &Value,
) -> Result<Vec<(String, String)>> {
    let work = join(staging, "math");
    let fill = [
        ("KEY", key.to_string()),
        ("TITLE", title.to_string()),
        ("STAGING", staging.to_string()),
        ("INDEX", join(&work, "index.md")),
    ];
    let pairs = [
        (field(manifest, "shared_prefix"), "dsh-math-instructions.zh.md", join(staging, "math-prefix.md")),
        (field(manifest, "link_prefix"), "dsh-math-link.zh.md", join(staging, "math-link-prefix.md")),
        (script_path("dsh-brief-instructions.zh.md"), "dsh-math-brief.zh.md", join(&work, "brief.zh.md")),
        (script_path("dsh-fill-instructions.zh.md"), "dsh-math-fill.zh.md", join(&work, "fill.zh.md")),
        (script_path("dsh-review-instructions.zh.md"), "dsh-math-review.zh.md", join(&work, "review.zh.md")),
    ];
    let mut files = Vec::new();
    for (shared, supplement, target) in pairs {
        let shared = util::read_text(&shared)?;
        let mut text = util::read_text(&script_path(supplement))?;
        for (k, v) in &fill {
            text = text.replace(&format!("{{{{{k}}}}}"), v);
        }
        files.push((target, format!("{}\n\n{}\n", shared.trim_end(), text.trim_end())));
    }
    Ok(files)
}

/// The Book notation table one applied answer produces: its rows, its title and its body.
/// `notation-apply` writes it; the ledger's `notation` check re-derives it from the same
/// retained answer and unit list, so a completed notation layer is the applied table (R238).
pub(crate) fn notation_table(
    raw: &Value,
    units: &Value,
    key: &str,
    title: &str,
    max_symbols: i64,
) -> (usize, String, String) {
    let mut by_id: CiMap<Value> = CiMap::new();
    if truthy(units) {
        for u in pipe(&units["units"]) {
            if truthy(&u["address"]) {
                by_id.insert(&field(&u, "id"), u["address"].clone());
            }
        }
    }
    let mut rows: Vec<String> = Vec::new();
    // Bug G: a row is one (symbol, meaning) pair, compared case-sensitively ($C$ and $c$ differ), so a symbol
    // with two senses keeps both rows; a later resolved locator replaces an earlier unresolved one.
    let mut seen: std::collections::HashMap<String, (usize, bool)> = std::collections::HashMap::new();
    for item in pipe(&raw["symbols"]) {
        let symbol = field(&item, "symbol").trim().to_string();
        let meaning = field(&item, "meaning").trim().replace('|', "/");
        if symbol.is_empty() || symbol.contains(['|', '\n']) {
            continue;
        }
        let row_key = format!("{symbol}\u{1f}{}", SPACES.replace_all(&meaning, " "));
        let unit = field(&item, "unit").trim().to_string();
        // Only a locator the allocation resolves is written; free text (`第 24 章`) is no locator.
        let resolved = by_id.get(&unit).filter(|a| truthy(a));
        let place = match resolved {
            Some(a) => format!("{unit} [[{}]]", s(a)),
            None => "—".to_string(),
        };
        let row = format!("| {symbol} | {meaning} | {place} |");
        match seen.get(&row_key).copied() {
            Some((at, false)) if resolved.is_some() => {
                rows[at] = row;
                seen.insert(row_key, (at, true));
            }
            Some(_) => {}
            None => {
                if rows.len() as i64 >= max_symbols {
                    continue;
                }
                seen.insert(row_key, (rows.len(), resolved.is_some()));
                rows.push(row);
            }
        }
    }
    let body = format!(
        "本卡是《{}》的符号表。本书的单元用左列的符号表示中列的对象。证明可以依赖本表。\n\n符号表\n\n| 符号 | 含义 | 定义于 |\n| --- | --- | --- |\n{}\n\n来源：[[{}]]",
        title,
        rows.join("\n"),
        key
    );
    let title = format!("符号表：《{}》的 {} 个符号", title, rows.len());
    (rows.len(), title, body)
}

impl Compiler {
    fn zt(&mut self) -> Result<&mut Zt> {
        if self.zt.is_none() {
            self.zt = Some(crate::zt::start()?);
        }
        Ok(self.zt.as_mut().unwrap())
    }

    fn path(&self, rel: &str) -> String {
        join(&self.p.staging, rel)
    }

    fn script_file(&self, name: &str) -> String {
        script_path(name)
    }

    /// `Invoke-Write`: the calls through the helper; the addresses of the writes, in order. A
    /// failed call is an error `zt: <the helper's lines>`.
    fn write(&mut self, calls: &[Value], log: &str, src_log: &str) -> Result<Vec<String>> {
        if calls.is_empty() {
            return Ok(Vec::new());
        }
        let mut options = calls::Options::new();
        options.log = log.to_string();
        options.src_log = src_log.to_string();
        let (done, failure) = calls::run(self.zt()?, calls, &mut options)?;
        let mut lines: Vec<String> = done.into_iter().map(|d| d.line).collect();
        if let Some(failure) = failure {
            lines.extend(failure.lines());
            bail!("zt: {}", lines.join(" | "));
        }
        Ok(lines.iter().filter(|l| OK_LINE.is_match(l)).map(|l| l.split_once(" ok ").map(|x| x.1.trim().to_string()).unwrap_or_default()).collect())
    }

    fn write_one(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        let written = self.write(&[json!({ "tool": tool, "arguments": arguments })], "", "")?;
        Ok(written.into_iter().next().map(Value::String).unwrap_or(Value::Null))
    }

    /// `Invoke-Zt` with a call that is not a read: it went through the helper, so a failure is
    /// `zt-mcp failed: <the helper's lines>`. The answers that are JSON objects, in order.
    fn helper(&mut self, calls: &[Value]) -> Result<Vec<Value>> {
        let (done, failure) = calls::run(self.zt()?, calls, &mut calls::Options::new())?;
        if let Some(failure) = failure {
            let mut lines: Vec<String> = done.into_iter().map(|d| d.line).collect();
            lines.extend(failure.lines());
            bail!("zt-mcp failed: {}", lines.join(" | "));
        }
        Ok(done.into_iter().filter_map(|d| d.value).collect())
    }

    /// The same, keeping every renumbering the archive reported: `-Moves` writes one line
    /// per call whose address mapping moved Cards, which is the only record of what a
    /// deletion renamed (R239).
    fn helper_moves(&mut self, calls: &[Value], moves: &str) -> Result<Vec<Value>> {
        let mut options = calls::Options::new();
        options.moves = moves.to_string();
        let (done, failure) = calls::run(self.zt()?, calls, &mut options)?;
        if let Some(failure) = failure {
            let mut lines: Vec<String> = done.into_iter().map(|d| d.line).collect();
            lines.extend(failure.lines());
            bail!("zt-mcp failed: {}", lines.join(" | "));
        }
        Ok(done.into_iter().filter_map(|d| d.value).collect())
    }

    /// `Get-Zt $address [$tool]`.
    fn get(&mut self, at: &Value, tool: &str) -> Result<Value> {
        read_one(self.zt()?, tool, json!({ "at": at }))
    }

    /// `Get-Section`: lines `from` to `to` of a section file, at most `max`; '' when the file is missing.
    fn section(&self, file: &str, from: i64, to: i64, max: i64) -> Result<String> {
        let path = self.path(&format!("sections\\{file}"));
        if !ps::exists(&path) {
            return Ok(String::new());
        }
        Ok(section_of(&ps::read_lines(&path)?, from, to, max))
    }

    fn units_file(&self) -> String {
        join(&self.work, "units.json")
    }

    /// `Get-Units`: units.json, or null.
    fn units(&self) -> Result<Value> {
        let file = self.units_file();
        if ps::exists(&file) { read_json(&file) } else { Ok(Value::Null) }
    }

    /// `Get-UnitAddresses`: the units that have an address.
    fn unit_addresses(&self) -> Result<Vec<Value>> {
        let data = self.units()?;
        if !truthy(&data) {
            return Ok(Vec::new());
        }
        Ok(pipe(&data["units"]).into_iter().filter(|u| truthy(&u["address"])).cloned().collect())
    }

    /// `Get-Theories`: the Theory Cards that have an address.
    fn theories(&self) -> Result<Vec<Value>> {
        let data = self.units()?;
        if !truthy(&data) {
            return Ok(Vec::new());
        }
        Ok(pipe(&data["theories"]).into_iter().filter(|t| truthy(&t["address"])).cloned().collect())
    }

    fn per_job(&self) -> Result<usize> {
        // The script's loop never ends with a step of 0 or less.
        if self.p.units_per_job <= 0 {
            bail!("-UnitsPerJob must be 1 or more, not {}", self.p.units_per_job);
        }
        Ok(self.p.units_per_job as usize)
    }

    // --- index -----------------------------------------------------------------------------------

    fn index_work(&self) -> String {
        join(&self.work, "index")
    }

    /// The finder's jobs: one DeepSeek job per -IndexChars characters of whole groups. Each job
    /// file holds its sections with numbered lines and the job's self-check command.
    fn step_index(&mut self) -> Result<()> {
        let iw = self.index_work();
        std::fs::create_dir_all(join(&iw, "out"))?;
        if self.p.index_chars < 2000 {
            bail!("-IndexChars must be 2000 or more, not {}", self.p.index_chars);
        }
        let sections = prepared_sections(&self.p.staging, &self.manifest)?;
        let doc = join(&iw, "dsh-math-index-instructions.zh.md");
        let instructions = util::read_text(self.script_file("dsh-math-index-instructions.zh.md"))?;
        util::write_text(&doc, &instructions.replace("{{KEY}}", &self.key).replace("{{TITLE}}", &self.title))?;
        let mut jobs = Vec::new();
        let mut planned = Vec::new();
        let mut chars = 0usize;
        let mut reused = 0usize;
        for (k, share) in index_jobs(&sections, self.p.index_chars as usize).iter().enumerate() {
            let id = format!("ix-{}", ps::d2(k as i64 + 1));
            let out = join(&iw, &format!("out\\{id}.json"));
            let files: Vec<String> = share.iter().map(|x| x.file.clone()).collect();
            planned.push(obj(vec![("id", json!(id)), ("sections", json!(files)), ("groups", json!(ps::select_unique(share.iter().map(|x| x.group.to_string()))))]));
            // A resume reruns no finished work: an answer that passes its check against the same
            // sections stays, so the index (and every unit id the skeleton built from it) stays.
            if ps::exists(&out) && read_json(&out).map(|a| index::check_answer(&a, share).is_empty()).unwrap_or(false) {
                reused += 1;
                continue;
            }
            let check = format!("zt-digest math -Step index-check -Staging \"{}\" -Job {id}", self.p.staging);
            let text = index::job_text(&self.title, &id, &out, &check, share);
            chars += text.chars().count();
            let job = join(&iw, &format!("{id}.md"));
            util::write_text(&job, &text)?;
            jobs.push(obj(vec![
                ("id", json!(id)),
                ("files", json!([doc, job])),
                ("prompt", json!(format!("按以上说明找出 {id} 各文件里的数学陈述，写出 JSON 文件，运行 SELF-CHECK。最后只回复一行 JSON。"))),
            ]));
        }
        save_json(&join(&iw, "plan.json"), &obj(vec![("schema_version", json!(1)), ("index_chars", json!(self.p.index_chars)), ("jobs", json!(planned))]))?;
        out_last(&obj(vec![("ok", json!(true)), ("sections", json!(sections.len())), ("chars", json!(chars)), ("reused", json!(reused)), ("jobs", json!(jobs))]))
    }

    /// The self-check of one finder answer: one fault per line, then `faults: N`.
    fn step_index_check(&mut self) -> Result<()> {
        let id = self.p.job.trim().to_string();
        if id.is_empty() {
            bail!("-Job is required");
        }
        let iw = self.index_work();
        let plan = read_json(&join(&iw, "plan.json"))?;
        let Some(job) = pipe(&plan["jobs"]).into_iter().find(|j| eq(&field(j, "id"), &id)).cloned() else {
            bail!("no finder job {id} in {}", join(&iw, "plan.json"));
        };
        let files: Vec<String> = pipe(&job["sections"]).into_iter().map(s).collect();
        let mine: Vec<index::Section> = prepared_sections(&self.p.staging, &self.manifest)?.into_iter().filter(|x| files.iter().any(|f| eq(f, &x.file))).collect();
        let file = join(&iw, &format!("out\\{id}.json"));
        let faults = if !ps::exists(&file) {
            vec![format!("no answer file {file}")]
        } else {
            match read_json(&file) {
                Ok(a) => index::check_answer(&a, &mine),
                Err(e) => vec![format!("the answer is not valid JSON: {e}")],
            }
        };
        for f in &faults {
            println!("{f}");
        }
        println!("faults: {}", faults.len());
        Ok(())
    }

    /// The index from every checked answer, in the record shape the skeleton reads.
    fn step_index_apply(&mut self) -> Result<()> {
        let (units, others) = match retained_index(&self.p.staging, &self.manifest) {
            Ok(x) => x,
            Err(e) => return out_last(&obj(vec![("ok", json!(false)), ("error", json!(e.to_string()))])),
        };
        let index_file = join(&self.work, "index.json");
        save_json(&index_file, &obj(vec![("units", json!(units)), ("others", json!(others))]))?;
        let mut md = format!("# 数学单元索引：《{}》{NL}{NL}", self.title);
        md.push_str(&format!("| 编号 | 形式 | 层级 | 名称 | 原文 | 证明 | 引用 |{NL}| --- | --- | --- | --- | --- | --- | --- |{NL}"));
        for u in &units {
            let proof = if ps::int(&u["proof"]).unwrap_or(0) > 0 { format!(":{}", field(u, "proof")) } else { "—".to_string() };
            let cites: Vec<String> = pipe(&u["cites"]).into_iter().map(s).collect();
            md.push_str(&format!(
                "| {} | {} | {} | {} | `sections/{}:{}-{}` | {proof} | {} |{NL}",
                field(u, "id"),
                field(u, "form"),
                field(u, "level"),
                field(u, "name"),
                field(u, "section"),
                field(u, "line"),
                field(u, "end"),
                cites.join("、")
            ));
        }
        if !others.is_empty() {
            md.push_str(&format!("{NL}## 例与习题{NL}{NL}| 编号 | 种类 | 名称 | 原文 | 属于 |{NL}| --- | --- | --- | --- | --- |{NL}"));
            for o in &others {
                md.push_str(&format!(
                    "| {} | {} | {} | `sections/{}:{}-{}` | {} |{NL}",
                    field(o, "id"),
                    field(o, "kind"),
                    field(o, "name"),
                    field(o, "section"),
                    field(o, "line"),
                    field(o, "end"),
                    field(o, "after")
                ));
            }
        }
        util::write_text(join(&self.work, "index.md"), &md)?;
        let mut forms: CiMap<i64> = CiMap::new();
        for u in &units {
            bump(&mut forms, &field(u, "form"));
        }
        let cites: f64 = units.iter().map(|u| ps::ps_count(&u["cites"]) as f64).sum();
        out_last(&obj(vec![
            ("ok", json!(true)),
            ("units", json!(units.len())),
            ("forms", counts(&forms)),
            ("examples", json!(others.iter().filter(|o| eq(&field(o, "kind"), "example")).count())),
            ("exercises", json!(others.iter().filter(|o| eq(&field(o, "kind"), "exercise")).count())),
            ("proofs", json!(units.iter().filter(|u| ps::int(&u["proof"]).unwrap_or(0) > 0).count())),
            ("cites", json!(cites)),
            ("file", json!(index_file)),
        ]))
    }

    // --- docs ------------------------------------------------------------------------------------

    fn step_docs(&mut self) -> Result<()> {
        let mut written = Vec::new();
        for (target, text) in docs_files(
            &self.key,
            &self.title,
            &self.p.staging,
            &self.manifest,
        )? {
            util::write_text(&target, &text)?;
            written.push(target);
        }
        let report = obj(vec![("ok", json!(true)), ("files", json!(written))]);
        save_json(&join(&self.work, "docs.json"), &report)?;
        out_last(&report)
    }

    // --- skeleton --------------------------------------------------------------------------------

    fn step_skeleton(&mut self) -> Result<()> {
        let index_file = join(&self.work, "index.json");
        if !ps::exists(&index_file) {
            bail!("run -Step index first");
        }
        let index = read_json(&index_file)?;
        let mut units: Vec<Value> = ps::ps_array(&index["units"]);
        let done = self.units()?;
        let mut made: CiMap<Value> = CiMap::new();
        if truthy(&done) {
            for u in pipe(&done["units"]) {
                if truthy(&u["address"]) {
                    made.insert(&field(u, "id"), u["address"].clone());
                }
            }
        }
        // A crash between the writes below and the saved list left heads no record names: adopt them, and
        // the Theory Cards below likewise (recoverable allocation).
        let heads: Vec<Value> = if self.p.apply {
            let key = self.key.clone();
            let tree = read_one(self.zt()?, "get_tree", json!({ "tree": key, "bodies": true }))?;
            pipe(&tree["cards"]).into_iter().cloned().collect()
        } else {
            Vec::new()
        };
        let unrecorded: Vec<Value> = units.iter().filter(|u| !made.contains(&field(u, "id"))).cloned().collect();
        for (id, address) in recover_allocation(&self.key, &unrecorded, &heads)? {
            made.insert(&id, json!(address));
        }
        let mut created = 0;
        let groups: Vec<Value> = pipe(&self.manifest["groups"]).into_iter().cloned().collect();
        for g in &groups {
            let n = ps::int(&g["n"])?;
            let mine: Vec<Value> = units.iter().filter(|u| num_eq(&u["group"], n) && !made.contains(&field(u, "id"))).cloned().collect();
            if mine.is_empty() || !self.p.apply {
                continue;
            }
            let id = format!("g{}", ps::d2(n));
            let calls: Vec<Value> = mine
                .iter()
                .map(|u| {
                    let src = format!("sections/{}:{}-{}", field(u, "section"), field(u, "line"), field(u, "end"));
                    let title = format!("{}{}：待写", field(u, "id"), name_part(u));
                    json!({ "tool": "create_successor", "arguments": {
                        "at": field(g, "anchor"), "kind": "side", "type": "model", "form": u["form"], "level": u["level"],
                        "title": title, "body": format!("**陈述**：待写。\n\n来源：[[{}]] {}（`{src}`）", self.key, field(u, "id")), "src": src
                    } })
                })
                .collect();
            let log = self.path(&format!("owned-{id}.txt"));
            let src_log = self.path(&format!("src-{id}.txt"));
            let addresses = self.write(&calls, &log, &src_log)?;
            for (i, u) in mine.iter().enumerate() {
                made.insert(&field(u, "id"), addresses.get(i).map(|a| json!(a)).unwrap_or(Value::Null));
                created += 1;
            }
        }
        for u in units.iter_mut() {
            let address = made.get(&field(u, "id")).cloned().unwrap_or(Value::Null);
            match u.as_object_mut() {
                Some(map) => {
                    map.insert("address".into(), address);
                }
                None => bail!("The property 'address' cannot be found on this object. Verify that the property exists and can be set."),
            }
        }
        // One Theory Card per chapter that holds units, under the chapter's first anchor.
        let mut theories: Vec<Value> = if truthy(&done) { ps::ps_array(&done["theories"]) } else { Vec::new() };
        let chapters = ps::select_unique(groups.iter().map(|g| field(g, "chapter")));
        for chapter in &chapters {
            let members: Vec<&Value> = units.iter().filter(|u| eq(&field(u, "chapter"), chapter) && truthy(&u["address"])).collect();
            if members.is_empty() || theories.iter().any(|t| eq(&field(t, "chapter"), chapter)) {
                continue;
            }
            let anchor = groups.iter().find(|g| eq(&field(g, "chapter"), chapter)).map(|g| field(g, "anchor")).unwrap_or_default();
            let list = members.iter().map(|u| format!("- [[{}]] {}{}", field(u, "address"), field(u, "id"), name_part(u))).collect::<Vec<_>>().join("\n");
            let body = format!("**问题**：待写。\n\n**主要结果**：待写。\n\n**对象**：待写。\n\n**结构**：\n{list}\n\n来源：[[{}]] {chapter}", self.key);
            let addresses: Vec<Value> = members.iter().map(|u| u["address"].clone()).collect();
            let mut address = recover_theory(&self.key, chapter, &heads)?.map_or(Value::Null, |a| json!(a));
            if self.p.apply && address.is_null() {
                address = self.write_one(
                    "create_successor",
                    json!({ "at": anchor, "kind": "side", "type": "model", "form": "theory", "title": format!("理论：{chapter}（待写）"), "body": body }),
                )?;
            }
            theories.push(obj(vec![("chapter", json!(chapter)), ("anchor", json!(anchor)), ("address", address), ("units", json!(addresses))]));
        }
        // Each group file names its units, examples, and exercises, so its digest run writes them.
        if self.p.apply {
            for g in &groups {
                let n = ps::int(&g["n"])?;
                let mine: Vec<&Value> = units.iter().filter(|u| num_eq(&u["group"], n) && truthy(&u["address"])).collect();
                let extra: Vec<&Value> = pipe(&index["others"]).into_iter().filter(|e| num_eq(&e["group"], n)).collect();
                if mine.is_empty() && extra.is_empty() {
                    continue;
                }
                let file = field(g, "file");
                let text = util::read_text(&file)?;
                if text.is_empty() {
                    bail!("You cannot call a method on a null-valued expression.");
                }
                let marker = group_extension_marker();
                if text.contains(marker) {
                    continue;
                }
                let mut sb = format!("{NL}{NL}{marker}（单元流水线写它们，你只链接）{NL}{NL}");
                for u in &mine {
                    let cites: Vec<String> = pipe(&u["cites"])
                        .into_iter()
                        .map(|c| {
                            let c = s(c);
                            match made.get(&c).filter(|a| truthy(a)) {
                                Some(a) => format!("{c} [[{}]]", s(a)),
                                None => c,
                            }
                        })
                        .collect();
                    let proof = if truthy(&u["proof"]) { format!("，证明从第 {} 行起", field(u, "proof")) } else { String::new() };
                    let cited = if cites.is_empty() { String::new() } else { format!("；引用 {}", cites.join("、")) };
                    sb.push_str(&format!(
                        "- [[{}]] {}{}：{}，{}；原文 `sections/{}:{}-{}`{proof}{cited}{NL}",
                        field(u, "address"),
                        field(u, "id"),
                        name_part(u),
                        field(u, "form"),
                        field(u, "level"),
                        field(u, "section"),
                        field(u, "line"),
                        field(u, "end")
                    ));
                }
                let theory = theories.iter().find(|t| eq(&field(t, "chapter"), &field(g, "chapter")));
                if let Some(theory) = theory
                    && truthy(&theory["address"])
                {
                    sb.push_str(&format!("{NL}本章的理论卡：[[{}]]（理论阶段写它，你不改它）。{NL}", field(theory, "address")));
                }
                if !extra.is_empty() {
                    sb.push_str(&format!("{NL}## 本组的例与习题（单元流水线把它们放进所属单元，你不另写卡片）{NL}{NL}"));
                    for e in &extra {
                        let after = if truthy(&e["after"]) {
                            let at = match made.get(&field(e, "after")).filter(|a| truthy(a)) {
                                Some(a) => format!(" [[{}]]", s(a)),
                                None => String::new(),
                            };
                            format!("，属于 {}{at}", field(e, "after"))
                        } else {
                            String::new()
                        };
                        sb.push_str(&format!(
                            "- {}：{}，`sections/{}:{}-{}`{after}{NL}",
                            field(e, "id"),
                            field(e, "kind"),
                            field(e, "section"),
                            field(e, "line"),
                            field(e, "end")
                        ));
                    }
                }
                util::write_text(&file, &format!("{}{sb}", text.trim_end()))?;
            }
        }
        let units_file = self.units_file();
        let with_address = units.iter().filter(|u| truthy(&u["address"])).count();
        let theories_made = theories.iter().filter(|t| truthy(&t["address"])).count();
        save_json(
            &units_file,
            &obj(vec![("key", json!(self.key)), ("units", json!(units)), ("others", json!(ps::ps_array(&index["others"]))), ("theories", json!(theories))]),
        )?;
        out_last(&obj(vec![
            ("ok", json!(true)),
            ("units", json!(with_address)),
            ("created", json!(created)),
            ("theories", json!(theories_made)),
            ("applied", json!(self.p.apply)),
            ("file", json!(units_file)),
        ]))
    }

    // --- notation --------------------------------------------------------------------------------

    fn notation_work(&self) -> String {
        join(&self.work, "notation")
    }

    fn step_notation(&mut self) -> Result<()> {
        let nw = self.notation_work();
        std::fs::create_dir_all(join(&nw, "out"))?;
        let data = self.units()?;
        let mut sb = format!("# 符号任务：《{}》{NL}{NL}", self.title);
        sb.push_str(&format!("把结果写入 `{}`。{NL}{NL}", join(&nw, "out\\notation.json")));
        sb.push_str(&format!("## 定义与公理的原文开头（符号多在这里引入）{NL}{NL}"));
        for u in pipe(&data["units"]) {
            if !is_in(&field(u, "form"), &["definition", "axiom"]) {
                continue;
            }
            let line = ps::int(&u["line"])?;
            let end = ps::int(&u["end"])?;
            let first = self.section(&field(u, "section"), line, end.min(line + 5), 6)?;
            let mut first = SPACES.replace_all(&first, " ").into_owned();
            if ps::len16(&first) > 220 {
                first = format!("{}…", ps::cut16(&first, 220));
            }
            let at = if truthy(&u["address"]) { format!(" [[{}]]", field(u, "address")) } else { String::new() };
            sb.push_str(&format!("- {}{at}：{first}{NL}", field(u, "id")));
        }
        sb.push_str(&format!("{NL}## 引入记号的句子（记作、记为、表示、令）{NL}{NL}"));
        let dir = self.path("sections");
        if !Path::new(&dir).is_dir() {
            bail!("Cannot find path '{dir}' because it does not exist.");
        }
        let mut names = ps::child_files(&dir, "s", ".md");
        ps::ps_sort(&mut names, |a, b| ps::culture_cmp(a, b));
        let mut count = 0;
        for name in &names {
            let mut n = 0;
            for line in ps::read_lines(&join(&dir, name))? {
                n += 1;
                if count >= 200 {
                    break;
                }
                if line.contains('$') && NOTATION_WORD.is_match(&line) {
                    let mut clip = SPACES.replace_all(&line, " ").trim().to_string();
                    if ps::len16(&clip) > 160 {
                        clip = format!("{}…", ps::cut16(&clip, 160));
                    }
                    sb.push_str(&format!("- `sections/{name}:{n}` {clip}{NL}"));
                    count += 1;
                }
            }
        }
        let job = join(&nw, "job.md");
        util::write_text(&job, &sb)?;
        let doc = join(&nw, "dsh-math-notation-instructions.zh.md");
        let instructions = util::read_text(self.script_file("dsh-math-notation-instructions.zh.md"))?;
        util::write_text(&doc, &instructions.replace("{{KEY}}", &self.key).replace("{{STAGING}}", &self.p.staging))?;
        out_last(&obj(vec![
            ("candidates", json!(count)),
            (
                "jobs",
                json!([obj(vec![
                    ("id", json!("notation")),
                    ("files", json!([doc, job])),
                    ("prompt", json!("按以上说明选出本书的符号，写出 JSON 文件。最后只回复一行 JSON。")),
                ])]),
            ),
        ]))
    }

    fn step_notation_apply(&mut self) -> Result<()> {
        let file = join(&self.notation_work(), "out\\notation.json");
        if !ps::exists(&file) {
            return out_last(&obj(vec![("ok", json!(false)), ("error", json!("no notation.json"))]));
        }
        let raw = read_json(&file)?;
        let data = self.units()?;
        let (symbols, title, body) = notation_table(&raw, &data, &self.key, &self.title, self.p.max_symbols);
        let mut address = Value::Null;
        if self.p.apply {
            let key = self.key.clone();
            let tree = read_one(self.zt()?, "get_tree", json!({ "tree": key }))?;
            let existing = pipe(&tree["cards"]).into_iter().find(|c| NOTATION_TITLE.is_match(&field(c, "title"))).cloned();
            address = match existing {
                Some(card) => self.write_one("edit_card", json!({ "at": field(&card, "address"), "title": title, "body": body }))?,
                None => self.write_one("create_successor", json!({ "at": format!("{key}/1"), "kind": "side", "title": title, "body": body }))?,
            };
        }
        util::write_text(self.path("notation.md"), &body)?;
        // The ledger's `notation` check reads this: which answer was applied, with which
        // symbol bound, and the exact Card it wrote. Availability is what dependent work needs.
        save_json(
            &join(&self.notation_work(), "applied.json"),
            &obj(vec![
                ("schema_version", json!(1)),
                ("kind", json!("notation_apply")),
                ("applied", json!(self.p.apply)),
                ("symbols", json!(symbols)),
                ("max_symbols", json!(self.p.max_symbols)),
                ("card", address.clone()),
                ("title", json!(title)),
                ("body_sha256", json!(crate::canon::sha256_hex(body.as_bytes()))),
                ("disposition", if symbols == 0 { json!("no_symbols") } else { json!("applied") }),
            ]),
        )?;
        out_last(&obj(vec![("ok", json!(true)), ("symbols", json!(symbols)), ("card", address)]))
    }

    // --- boundary --------------------------------------------------------------------------------

    fn boundary_work(&self) -> String {
        join(&self.work, "boundary")
    }

    fn step_boundary(&mut self) -> Result<()> {
        let bw = self.boundary_work();
        std::fs::create_dir_all(join(&bw, "out"))?;
        let mut todo: Vec<(Value, Value, Vec<String>)> = Vec::new();
        let mut unbuilt = 0usize;
        let forms: Vec<&str> = std::iter::once("definition").chain(RESULTS.iter().copied()).collect();
        for u in self.unit_addresses()? {
            if !is_in(&field(&u, "form"), &forms) {
                continue;
            }
            let model = self.get(&u["address"], "get_model")?;
            if !boundary_ready(&model) {
                unbuilt += 1;
                continue;
            }
            let facets: Vec<String> = pipe(&model["parts"]).into_iter().map(|p| field(p, "facet")).collect();
            let mut need: Vec<String> = Vec::new();
            if eq(&field(&u, "form"), "definition") {
                if !facets.iter().any(|f| EXAMPLE_FACET.is_match(f)) {
                    need.push("example".into());
                }
                if !facets.iter().any(|f| NON_EXAMPLE_FACET.is_match(f)) {
                    need.push("non_example".into());
                }
            } else {
                let roles: Vec<String> = facets
                    .iter()
                    .filter(|f| ROLE_FACET.is_match(f))
                    .filter_map(|f| H_LABEL.captures(f).map(|m| m[1].to_string()))
                    .collect();
                let missing: Vec<String> =
                    pipe(&model["math"]["labels"]).into_iter().map(s).filter(|l| STARTS_H.is_match(l) && !is_in(l, &roles)).collect();
                if !missing.is_empty() {
                    need.push(format!("hypotheses:{}", missing.join(",")));
                }
            }
            if !need.is_empty() {
                todo.push((u, model, need));
            }
        }
        let mut jobs: Vec<Value> = Vec::new();
        let mut batches = Map::new();
        let per_job = if todo.is_empty() { 1 } else { self.per_job()? };
        for (k, share) in todo.chunks(per_job).enumerate() {
            let name = format!("b-{}", ps::d2(k as i64 + 1));
            let mut sb = format!("# 边界任务 {name}：《{}》{NL}{NL}", self.title);
            sb.push_str(&format!(
                "把结果写入 `{}`。只读本文件，不读其他文件。补写许可：`{}`。{NL}{NL}",
                join(&bw, &format!("out\\{name}.json")),
                self.p.supply
            ));
            for (u, model, need) in share {
                sb.push_str(&format!("## [[{}]] {}（{}）{NL}{NL}", field(u, "address"), field(u, "id"), field(u, "form")));
                sb.push_str(&format!("要写：{}{NL}{NL}", need.join("；")));
                sb.push_str(&format!("### 模型卡正文{NL}{NL}{}{NL}{NL}", field(&model["head"], "body")));
                let steps: Vec<String> = pipe(&model["build"]["order"])
                    .into_iter()
                    .filter(|o| eq(&field(o, "line"), "main"))
                    .map(|o| format!("- [[{}]] {}：{}", field(o, "address"), field(o, "facet"), field(o, "title")))
                    .collect();
                if !steps.is_empty() {
                    sb.push_str(&format!("### 主线（证明的各步）{NL}{NL}{}{NL}{NL}", steps.join("\n")));
                }
                let bodies: Vec<String> = pipe(&model["parts"])
                    .into_iter()
                    .filter(|p| PROOF_BODY_FACET.is_match(&field(p, "facet")))
                    .map(|p| format!("#### [[{}]] {}\n{}", field(p, "address"), field(p, "facet"), field(p, "body")))
                    .collect();
                if !bodies.is_empty() {
                    sb.push_str(&format!("### 证明分卡正文{NL}{NL}{}{NL}{NL}", bodies.join("\n\n")));
                }
                let source = self.section(&field(u, "section"), ps::int(&u["line"])?, ps::int(&u["end"])?, 60)?;
                sb.push_str(&format!("### 原文{NL}{NL}{source}{NL}{NL}"));
            }
            let job = join(&bw, &format!("{name}.md"));
            util::write_text(&job, &sb)?;
            batches.insert(name.clone(), Value::Array(share.iter().map(|(u, _, _)| u["address"].clone()).collect()));
            jobs.push(obj(vec![
                ("id", json!(name)),
                ("files", json!([self.script_file("dsh-math-boundary-instructions.zh.md"), job])),
                ("prompt", json!(format!("按以上说明写出 {name} 的 JSON 文件。最后只回复一行 JSON。"))),
            ]));
        }
        save_json(&join(&bw, "state.json"), &obj(vec![("batches", Value::Object(batches.clone())), ("created", json!({}))]))?;
        // The ledger adapter reads this plan: one boundary obligation per planned unit (R239).
        let mut planned: Vec<Value> = Vec::new();
        for (u, _, need) in &todo {
            planned.push(obj(vec![
                ("unit", u["address"].clone()),
                ("id", u["id"].clone()),
                ("form", u["form"].clone()),
                ("group", u["group"].clone()),
                ("need", json!(need)),
            ]));
        }
        save_json(&join(&bw, "plan.json"), &obj(vec![("schema_version", json!(1)), ("units", json!(planned)), ("batches", Value::Object(batches))]))?;
        out_last(&obj(vec![("units", json!(todo.len())), ("unbuilt", json!(unbuilt)), ("jobs", json!(jobs))]))
    }

    fn step_boundary_apply(&mut self) -> Result<()> {
        struct Piece {
            facet: String,
            title: String,
            body: String,
            supplied: bool,
            role: bool,
            hypothesis: String,
            emphasis: bool,
        }
        let bw = self.boundary_work();
        let state_file = join(&bw, "state.json");
        let mut state = read_json(&state_file)?;
        if !state.is_object() {
            bail!("Cannot index into a null array.");
        }
        let none = eq(&self.p.supply, "none");
        let mut reviews: Vec<Value> = Vec::new();
        let mut made = 0;
        let mut skipped = 0;
        for name in ps::keys(&state["batches"]) {
            let file = join(&bw, &format!("out\\{name}.json"));
            if !ps::exists(&file) {
                continue;
            }
            let answer = read_json(&file)?;
            let batch: Vec<String> = pipe(&state["batches"][&name]).into_iter().map(s).collect();
            let mut created: Vec<Value> = Vec::new();
            for item in pipe(&answer["items"]) {
                let unit = field(item, "unit");
                if !is_in(&unit, &batch) {
                    continue;
                }
                // Under supply none, only what the book gives.
                let mut pieces: Vec<Piece> = Vec::new();
                if truthy(&item["hypothesis"]) {
                    let hypothesis = field(item, "hypothesis");
                    let ce = &item["counterexample"];
                    if truthy(ce) && truthy(&ce["title"]) && !(truthy(&ce["supplied"]) && none) {
                        let mark = if truthy(&ce["supplied"]) && !SUPPLIED_MARK.is_match(&field(ce, "body")) { "（补：书中未给）" } else { "" };
                        pieces.push(Piece {
                            facet: format!("反例：去掉 {hypothesis}"),
                            title: field(ce, "title"),
                            body: format!("反例 [[{unit}]]：{}{mark}", field(ce, "body").trim()),
                            supplied: truthy(&ce["supplied"]),
                            role: false,
                            hypothesis: String::new(),
                            emphasis: false,
                        });
                    } else if truthy(ce) && truthy(&ce["supplied"]) {
                        skipped += 1;
                    }
                    let used = if truthy(&item["used_in"]) { format!("使用 {hypothesis} 的步骤：[[{}]]。", field(item, "used_in")) } else { String::new() };
                    let role = field(item, "role").trim().to_string();
                    pieces.push(Piece {
                        facet: format!("假设作用：{hypothesis}"),
                        title: field(item, "title"),
                        body: if used.is_empty() { role } else { format!("{role}\n\n{used}") },
                        supplied: false,
                        role: true,
                        hypothesis,
                        emphasis: false,
                    });
                }
                for kind in ["example", "non_example"] {
                    let e = &item[kind];
                    if !truthy(e) || !truthy(&e["title"]) {
                        continue;
                    }
                    if truthy(&e["supplied"]) && none {
                        skipped += 1;
                        continue;
                    }
                    let mark = if truthy(&e["supplied"]) && !SUPPLIED_MARK.is_match(&field(e, "body")) { "（补：书中未给）" } else { "" };
                    let (facet, word) = if kind == "example" { ("例子", "例示") } else { ("非例", "对照") };
                    pieces.push(Piece {
                        facet: facet.into(),
                        title: field(e, "title"),
                        body: format!("{word} [[{unit}]]：{}{mark}", field(e, "body").trim()),
                        supplied: truthy(&e["supplied"]),
                        role: false,
                        hypothesis: String::new(),
                        emphasis: true,
                    });
                }
                // A counterexample first, so its hypothesis role can link it.
                let mut last = Value::Null;
                for p in pieces {
                    let mut body = p.body.clone();
                    if p.role && truthy(&last) {
                        body = format!("{body}\n\n去掉 {} 的反例：[[{}]]。", p.hypothesis, s(&last));
                    }
                    let mut arguments = json!({ "at": unit, "kind": "side", "facet": p.facet, "title": p.title, "body": body });
                    if p.emphasis {
                        arguments["emphasis"] = json!("core");
                    }
                    let address = self.write_one("create_successor", arguments)?;
                    made += 1;
                    created.push(obj(vec![
                        ("address", address.clone()),
                        ("unit", json!(unit)),
                        ("facet", json!(p.facet)),
                        ("supplied", json!(p.supplied)),
                        ("partner", if p.role { last.clone() } else { Value::Null }),
                    ]));
                    last = if COUNTER_FACET.is_match(&p.facet) { address } else { Value::Null };
                }
            }
            match state["created"].as_object_mut() {
                Some(map) => {
                    map.insert(name.clone(), Value::Array(created.clone()));
                }
                None => bail!("Cannot index into a null array."),
            }
            let supplied: Vec<&Value> = created.iter().filter(|c| truthy(&c["supplied"])).collect();
            if !supplied.is_empty() {
                let mut sb = format!("# 复核任务 {name}{NL}{NL}");
                sb.push_str(&format!("把结论写入 `{}`。只读本文件。{NL}{NL}", join(&bw, &format!("out\\v-{name}.json"))));
                for c in supplied {
                    let unit_card = self.get(&c["unit"], "get_card")?;
                    let card = self.get(&c["address"], "get_card")?;
                    sb.push_str(&format!("## [[{}]] {}：{}{NL}{NL}{}{NL}{NL}", field(c, "address"), field(c, "facet"), field(&card, "title"), field(&card, "body")));
                    sb.push_str(&format!(
                        "### 所属单元 [[{}]] {}{NL}{NL}{}{NL}{NL}",
                        field(c, "unit"),
                        field(&unit_card, "title"),
                        field(&unit_card, "body")
                    ));
                }
                let job = join(&bw, &format!("v-{name}.md"));
                util::write_text(&job, &sb)?;
                reviews.push(obj(vec![
                    ("id", json!(format!("v-{name}"))),
                    ("files", json!([self.script_file("dsh-math-boundary-review.zh.md"), job])),
                    ("prompt", json!(format!("按以上说明复核这些补写的分卡，写出 v-{name} 的 JSON 文件。最后只回复一行 JSON。"))),
                ]));
            }
        }
        save_json(&state_file, &state)?;
        out_last(&obj(vec![("created", json!(made)), ("skipped_supplied", json!(skipped)), ("jobs", json!(reviews))]))
    }

    fn step_boundary_settle(&mut self) -> Result<()> {
        let bw = self.boundary_work();
        let state = read_json(&join(&bw, "state.json"))?;
        let mut wrong: CiMap<String> = CiMap::new();
        let mut kept = 0;
        // Every candidate keeps its own disposition: what it is, whether the book gave it,
        // the independent review of a supplied one, and what the settle did with it. A
        // batch count is never the record (R239).
        let mut reviews: Vec<Value> = Vec::new();
        let mut candidates: Vec<Value> = Vec::new();
        let mut unreviewed = 0;
        for name in ps::keys(&state["created"]) {
            let file = join(&bw, &format!("out\\v-{name}.json"));
            let answer = if ps::exists(&file) { read_json(&file)? } else { Value::Null };
            let verdicts = pipe(&answer["verdicts"]);
            let created = pipe(&state["created"][&name]);
            for c in created.iter() {
                let mut candidate = (*c).clone();
                if let Some(map) = candidate.as_object_mut() {
                    map.insert("batch".into(), json!(name));
                }
                candidates.push(candidate);
            }
            for c in created.iter().filter(|c| truthy(&c["supplied"])) {
                let address = field(c, "address");
                let v = verdicts.iter().find(|v| eq(&field(v, "address"), &address));
                let (verdict, reason) = match v.filter(|v| truthy(v)) {
                    Some(v) if eq(&field(v, "verdict"), "wrong") => ("wrong", field(v, "reason")),
                    Some(v) if eq(&field(v, "verdict"), "ok") => ("ok", String::new()),
                    _ => {
                        unreviewed += 1;
                        ("unreviewed", String::new())
                    }
                };
                reviews.push(obj(vec![
                    ("batch", json!(name)),
                    ("unit", c["unit"].clone()),
                    ("address", json!(address)),
                    ("verdict", json!(verdict)),
                    ("reason", json!(reason)),
                ]));
                if verdict == "unreviewed" {
                    continue;
                }
                if verdict == "ok" {
                    kept += 1;
                    continue;
                }
                // The part that names a wrong counterexample goes with it.
                wrong.insert(&address, reason.clone());
                for p in created.iter().filter(|p| !p["partner"].is_null() && eq(&field(p, "partner"), &address)) {
                    wrong.insert(&field(p, "address"), reason.clone());
                }
            }
        }
        // Delete the last Card first: deleting a Side successor renames its later siblings.
        let mut order: Vec<(String, String)> = wrong.keys().into_iter().map(|a| (order_key(&a), a)).collect();
        ps::ps_sort(&mut order, |a, b| ps::culture_cmp(&b.0, &a.0));
        // Read every candidate before anything moves, so the record carries what each
        // address held. That fingerprint is how the ledger finds the same part again
        // after a confirmed deletion renumbered its neighbours.
        let addresses: Vec<String> = candidates.iter().map(|c| field(c, "address")).collect();
        let reads: Vec<(&str, Value)> = addresses.iter().map(|at| ("get_card", json!({ "at": at }))).collect();
        let cards = if reads.is_empty() { Vec::new() } else { read_calls(self.zt()?, &reads)? };
        for candidate in candidates.iter_mut() {
            let at = field(candidate, "address");
            let card = cards.iter().find(|c| eq(&field(c, "address"), &at));
            if let Some(map) = candidate.as_object_mut() {
                map.insert("fingerprint".into(), card.map(crate::calls::authored_fingerprint).map(Value::String).unwrap_or(Value::Null));
            }
        }
        let mut deleted: Vec<Value> = Vec::new();
        // Deleting a Side successor renumbers its later siblings, so the settle keeps the
        // archive's own mapping. That mapping is how a recorded candidate still resolves
        // to its Card after a rejected neighbour is gone.
        let moves_log = join(&bw, "settle-moves.jsonl");
        if ps::exists(&moves_log) {
            std::fs::remove_file(&moves_log)?;
        }
        for (_, address) in order {
            // Read the part before it goes: the record keeps what was removed and the
            // confirmation the archive issued for exactly that Card.
            let card = self.get(&json!(address), "get_card")?;
            let removed = crate::calls::authored_fingerprint(&card);
            let plan = self.helper(&[json!({ "tool": "delete_card", "arguments": { "at": address } })])?.into_iter().next().unwrap_or(Value::Null);
            let confirm = field(&plan, "confirm");
            let mut moved = Value::Null;
            if !confirm.is_empty() {
                let answers = self.helper_moves(
                    &[json!({ "tool": "delete_card", "arguments": { "at": address, "confirm": confirm.clone() } })],
                    &moves_log,
                )?;
                moved = answers.into_iter().next().unwrap_or(Value::Null);
            }
            let candidate = candidates.iter().find(|c| field(c, "address") == address);
            let reason = wrong.get(&address).cloned().unwrap_or_default();
            deleted.push(obj(vec![
                ("address", json!(address)),
                ("unit", candidate.map(|c| c["unit"].clone()).unwrap_or(Value::Null)),
                ("reason", json!(reason)),
                ("confirm", json!(confirm)),
                ("removed", json!(removed)),
                ("facet", card["facet"].clone()),
                ("moved", moved["moved"].clone()),
            ]));
        }
        let mut renames: Vec<Value> = Vec::new();
        if ps::exists(&moves_log) {
            for line in ps::read_lines(&moves_log)? {
                if line.trim().is_empty() {
                    continue;
                }
                let entry = ps::parse_json(&line)?;
                if truthy(&entry["moved"]) {
                    renames.push(entry);
                }
            }
        }
        let settled = obj(vec![
            ("schema_version", json!(1)),
            ("kind", json!("boundary_settle")),
            ("candidates", json!(candidates)),
            ("reviews", json!(reviews)),
            ("deleted", json!(deleted)),
            ("renames", json!(renames)),
            ("kept", json!(kept)),
            ("unreviewed", json!(unreviewed)),
        ]);
        save_json(&join(&bw, "settled.json"), &settled)?;
        out_last(&obj(vec![("kept", json!(kept)), ("deleted", json!(deleted.len())), ("unreviewed", json!(unreviewed)), ("parts", json!(deleted))]))
    }

    // --- proof -----------------------------------------------------------------------------------

    fn proof_work(&self) -> String {
        join(&self.work, "proof")
    }

    fn step_proof(&mut self) -> Result<()> {
        let pw = self.proof_work();
        std::fs::create_dir_all(join(&pw, "out"))?;
        let mut todo: Vec<(Value, Value, Vec<Value>)> = Vec::new();
        let mut planned: Vec<Value> = Vec::new();
        let mut unmet = 0usize;
        for u in self.unit_addresses()? {
            if !is_in(&field(&u, "form"), RESULTS) {
                continue;
            }
            let model = self.get(&u["address"], "get_model")?;
            match proof_decision(&u, &model) {
                ProofDecision::Review(steps) => todo.push((u, model, steps)),
                ProofDecision::Skip => {}
                ProofDecision::Unmet(reason) => {
                    unmet += 1;
                    planned.push(obj(vec![
                        ("unit", u["address"].clone()),
                        ("id", u["id"].clone()),
                        ("form", u["form"].clone()),
                        ("group", u["group"].clone()),
                        ("model", model["head"]["address"].clone()),
                        ("steps", json!([])),
                        ("unmet", json!(reason)),
                    ]));
                }
            }
        }
        let mut jobs: Vec<Value> = Vec::new();
        let mut batches = Map::new();
        let per_job = if todo.is_empty() { 1 } else { self.per_job()? };
        for (k, share) in todo.chunks(per_job).enumerate() {
            let name = format!("p-{}", ps::d2(k as i64 + 1));
            let mut sb = format!("# 证明复核任务 {name}{NL}{NL}");
            sb.push_str(&format!("把结果写入 `{}`。只读本文件。{NL}{NL}", join(&pw, &format!("out\\{name}.json"))));
            for (u, model, steps) in share {
                sb.push_str(&format!(
                    "## 单元 [[{}]] {}{NL}{NL}{}{NL}{NL}",
                    field(u, "address"),
                    field(&model["head"], "title"),
                    field(&model["head"], "body")
                ));
                for st in steps {
                    let part = pipe(&model["parts"]).into_iter().find(|p| eq(&field(p, "address"), &field(st, "address"))).cloned().unwrap_or(Value::Null);
                    let crux = if eq(&field(st, "emphasis"), "core") { "（关键步）" } else { "" };
                    sb.push_str(&format!(
                        "### [[{}]] {}：{}{crux}{NL}{NL}{}{NL}{NL}",
                        field(st, "address"),
                        field(st, "facet"),
                        field(st, "title"),
                        field(&part, "body")
                    ));
                }
                planned.push(obj(vec![
                    ("unit", u["address"].clone()),
                    ("id", u["id"].clone()),
                    ("form", u["form"].clone()),
                    ("group", u["group"].clone()),
                    ("model", model["head"]["address"].clone()),
                    ("steps", json!(steps.iter().map(|st| obj(vec![("address", st["address"].clone()), ("facet", st["facet"].clone())])).collect::<Vec<_>>())),
                ]));
            }
            batches.insert(name.clone(), Value::Array(share.iter().map(|(u, _, _)| u["address"].clone()).collect()));
            let job = join(&pw, &format!("{name}.md"));
            util::write_text(&job, &sb)?;
            jobs.push(obj(vec![
                ("id", json!(name)),
                ("files", json!([self.script_file("dsh-math-proof-review.zh.md"), job])),
                ("prompt", json!(format!("按以上说明复核这些证明，写出 {name} 的 JSON 文件。最后只回复一行 JSON。"))),
            ]));
        }
        // The ledger adapter reads this plan: one proof obligation per selected formal result (R240), and
        // one unmet obligation per result the source proves and the unit does not (Bug C).
        save_json(&join(&pw, "plan.json"), &obj(vec![("schema_version", json!(1)), ("units", json!(planned)), ("batches", Value::Object(batches))]))?;
        out_last(&obj(vec![("units", json!(todo.len())), ("unmet", json!(unmet)), ("jobs", json!(jobs))]))
    }

    fn step_proof_apply(&mut self) -> Result<()> {
        let out = join(&self.proof_work(), "out");
        let plan = read_json(&join(&self.proof_work(), "plan.json"))?;
        // Which unit each reviewed step belongs to: the ledger completes one unit at a time
        // and reads the reviewed steps and the applied fixes back from this record (R240).
        let mut owner: CiMap<String> = CiMap::new();
        for entry in pipe(&plan["units"]) {
            for step in pipe(&entry["steps"]) {
                owner.insert(&field(&step, "address"), field(&entry, "unit"));
            }
        }
        let (mut ok, mut fixed) = (0, 0);
        let mut refused: Vec<Value> = Vec::new();
        let mut reviewed: Vec<Value> = Vec::new();
        for name in ps::child_files(&out, "p-", ".json") {
            let answer = read_json(&join(&out, &name))?;
            for v in pipe(&answer["verdicts"]) {
                let address = field(v, "address");
                let unit = owner.get(&address).cloned().unwrap_or_default();
                let reason = field(v, "reason");
                let card = self.get(&json!(address), "get_card")?;
                let before = crate::calls::authored_fingerprint(&card);
                if !eq(&field(v, "verdict"), "fix") {
                    ok += 1;
                    reviewed.push(obj(vec![
                        ("unit", json!(unit)),
                        ("address", json!(address)),
                        ("verdict", json!("ok")),
                        ("reason", json!(reason)),
                        ("before", json!(before)),
                    ]));
                    continue;
                }
                let new = field(v, "body");
                if new.trim().is_empty() {
                    let why = format!("no body: {reason}");
                    refused.push(obj(vec![("address", json!(address)), ("reason", json!(why))]));
                    reviewed.push(obj(vec![
                        ("unit", json!(unit)),
                        ("address", json!(address)),
                        ("verdict", json!("refused")),
                        ("reason", json!(why)),
                        ("before", json!(before)),
                    ]));
                    continue;
                }
                let old = ps::select_unique(LINK.captures_iter(&field(&card, "body")).map(|m| m[1].to_string()));
                let kept: Vec<String> = LINK.captures_iter(&new).map(|m| m[1].to_string()).collect();
                let lost: Vec<String> = old.into_iter().filter(|o| !is_in(o, &kept)).collect();
                if !lost.is_empty() {
                    let why = format!("drops Links: {}", lost.join(", "));
                    refused.push(obj(vec![("address", json!(address)), ("reason", json!(why))]));
                    reviewed.push(obj(vec![
                        ("unit", json!(unit)),
                        ("address", json!(address)),
                        ("verdict", json!("refused")),
                        ("reason", json!(why)),
                        ("before", json!(before)),
                        ("dropped_links", json!(lost)),
                    ]));
                    continue;
                }
                self.write(&[json!({ "tool": "edit_card", "arguments": { "at": address, "body": new } })], "", "")?;
                let after = self.get(&json!(address), "get_card")?;
                if field(&after, "body") != new {
                    bail!("the accepted proof fix for `{address}` is not the body that was written");
                }
                fixed += 1;
                reviewed.push(obj(vec![
                    ("unit", json!(unit)),
                    ("address", json!(address)),
                    ("verdict", json!("fix")),
                    ("reason", json!(reason)),
                    ("before", json!(before)),
                    ("after", json!(crate::calls::authored_fingerprint(&after))),
                    ("kept_links", json!(kept)),
                ]));
            }
        }
        save_json(
            &join(&self.proof_work(), "applied.json"),
            &obj(vec![
                ("schema_version", json!(1)),
                ("kind", json!("proof_apply")),
                ("reviewed", json!(reviewed)),
                ("ok", json!(ok)),
                ("fixed", json!(fixed)),
                ("refused", json!(refused.len())),
            ]),
        )?;
        out_last(&obj(vec![("ok", json!(ok)), ("fixed", json!(fixed)), ("refused", json!(refused.len())), ("parts", json!(refused))]))
    }

    // --- theory ----------------------------------------------------------------------------------

    fn theory_work(&self) -> String {
        join(&self.work, "theory")
    }

    fn step_theory(&mut self) -> Result<()> {
        let tw = self.theory_work();
        std::fs::create_dir_all(join(&tw, "out"))?;
        let theories = self.theories()?;
        let mut jobs: Vec<Value> = Vec::new();
        for (k, share) in theories.chunks(4).enumerate() {
            let name = format!("t-{}", ps::d2(k as i64 + 1));
            let mut sb = format!("# 理论任务 {name}：《{}》{NL}{NL}", self.title);
            sb.push_str(&format!(
                "把结果写入 `{}`。只读本文件。主要结果至多 {} 个。{NL}{NL}",
                join(&tw, &format!("out\\{name}.json")),
                self.p.main_results
            ));
            for t in share {
                let model = self.get(&t["address"], "get_model")?;
                sb.push_str(&format!("## 理论卡 [[{}]]：{}{NL}{NL}", field(t, "address"), field(t, "chapter")));
                sb.push_str(&format!("| 单元 | 形式 | 标题 | 被引次数 | 引用本章单元 |{NL}| --- | --- | --- | --- | --- |{NL}"));
                let order = pipe(&model["math"]["order"]);
                for u in &order {
                    let cites: Vec<String> = pipe(&u["cites"]).into_iter().map(s).collect();
                    sb.push_str(&format!(
                        "| [[{}]] | {} | {} | {} | {} |{NL}",
                        field(u, "address"),
                        field(u, "form"),
                        field(u, "title"),
                        field(u, "load"),
                        cites.join(" ")
                    ));
                }
                sb.push_str(NL);
                for u in order.iter().filter(|u| is_in(&field(u, "form"), &["theorem", "definition"])) {
                    let card = self.get(&u["address"], "get_card")?;
                    let statement = field(&card, "body").split('\n').take(8).collect::<Vec<_>>().join("\n");
                    sb.push_str(&format!("### [[{}]] {}{NL}{NL}{statement}{NL}{NL}", field(u, "address"), field(u, "title")));
                }
            }
            let job = join(&tw, &format!("{name}.md"));
            util::write_text(&job, &sb)?;
            jobs.push(obj(vec![
                ("id", json!(name)),
                ("files", json!([self.script_file("dsh-math-theory-instructions.zh.md"), job])),
                ("prompt", json!(format!("按以上说明写出 {name} 的 JSON 文件。最后只回复一行 JSON。"))),
            ]));
        }
        out_last(&obj(vec![("theories", json!(theories.len())), ("jobs", json!(jobs))]))
    }

    fn step_theory_apply(&mut self) -> Result<()> {
        let theories = self.theories()?;
        let out = join(&self.theory_work(), "out");
        let mut answers: CiMap<Value> = CiMap::new();
        for name in ps::child_files(&out, "t-", ".json") {
            let answer = read_json(&join(&out, &name))?;
            for t in pipe(&answer["theories"]) {
                answers.insert(&field(t, "address"), t.clone());
            }
        }
        if self.p.main_results < 0 {
            bail!(
                "Cannot validate argument on parameter 'First'. The {} argument is less than the minimum allowed range of 0. Supply an argument that is greater than or equal to 0 and then try the command again.",
                self.p.main_results
            );
        }
        let mut written = 0;
        // The checked membership/dependency summary of every Theory Card, so the ledger's
        // `theory` publication can rebuild and verify it after its units or cited-premise
        // Links change (R241).
        let mut applied: Vec<Value> = Vec::new();
        for t in &theories {
            let model = self.get(&t["address"], "get_model")?;
            let card = self.get(&t["address"], "get_card")?;
            let mut body = field(&card, "body");
            let members: Vec<String> = pipe(&model["math"]["order"]).into_iter().map(|u| field(u, "address")).collect();
            let a = answers.get(&field(t, "address")).cloned().unwrap_or(Value::Null);
            let lines = |list: &Value, take: usize| -> Vec<String> {
                pipe(list)
                    .into_iter()
                    .filter(|x| is_in(&field(x, "address"), &members))
                    .take(take)
                    .map(|x| format!("- [[{}]]：{}", field(x, "address"), field(x, "line").trim()))
                    .collect()
            };
            if truthy(&a) {
                if truthy(&a["question"]) {
                    body = set_section(&body, "问题", &format!("**问题**：{}", field(&a, "question").trim()));
                }
                let mains = lines(&a["main_results"], self.p.main_results as usize);
                if !mains.is_empty() {
                    body = set_section(&body, "主要结果", &format!("**主要结果**：\n{}", mains.join("\n")));
                }
                // An answered theory always replaces the skeleton's 对象 line: a theory whose units are all main
                // results has none, which is an answer, not a placeholder (thompson pilot, 5|b).
                let objects = lines(&a["objects"], usize::MAX);
                body = set_section(&body, "对象", &objects_section(&objects));
            }
            // Recompute after the main results changed: the weights depend on them.
            let title = if truthy(&a) && truthy(&a["title"]) {
                field(&a, "title").trim().to_string()
            } else {
                TODO_TITLE.replace(&field(&card, "title"), "").into_owned()
            };
            self.write(&[json!({ "tool": "edit_card", "arguments": { "at": t["address"], "title": title, "body": body } })], "", "")?;
            let model = self.get(&t["address"], "get_model")?;
            let order = pipe(&model["math"]["order"]);
            let structure = format!(
                "**结构**（依赖顺序）：\n{}",
                order.iter().map(|u| format!("- [[{}]] {}：{}", field(u, "address"), form_zh(&field(u, "form")), field(u, "title"))).collect::<Vec<_>>().join("\n")
            );
            body = set_section(&body, "结构", &structure);
            let tools: Vec<String> =
                pipe(&model["math"]["tools"]).into_iter().map(|x| format!("- 依赖 [[{}]]：{}", field(x, "address"), field(x, "title"))).collect();
            body = set_section(&body, "所用工具", &if tools.is_empty() { String::new() } else { format!("**所用工具**：\n{}", tools.join("\n")) });
            let users: Vec<String> = pipe(&model["math"]["users"])
                .into_iter()
                .filter(|x| truthy(&x["theory"]))
                .map(|x| format!("- 引出 [[{}]]：{}", field(&x["theory"], "address"), field(&x["theory"], "title")))
                .collect();
            body = set_section(&body, "后续", &if users.is_empty() { String::new() } else { format!("**后续**：\n{}", users.join("\n")) });
            body = set_section(&body, "图", &format!("**图**：\n\n{}", dependency_map(&order)?));
            self.write(&[json!({ "tool": "edit_card", "arguments": { "at": t["address"], "body": body } })], "", "")?;
            let written_card = self.get(&t["address"], "get_card")?;
            applied.push(obj(vec![
                ("address", t["address"].clone()),
                ("chapter", t["chapter"].clone()),
                ("title", json!(field(&written_card, "title"))),
                ("members", json!(members)),
                ("tools", json!(pipe(&model["math"]["tools"]).into_iter().map(|x| field(&x, "address")).collect::<Vec<_>>())),
                ("body_sha256", json!(crate::canon::sha256_hex(field(&written_card, "body").as_bytes()))),
            ]));
            written += 1;
        }
        save_json(
            &join(&self.theory_work(), "applied.json"),
            &obj(vec![
                ("schema_version", json!(1)),
                ("kind", json!("theory_apply")),
                ("theories", json!(applied)),
                ("applied", json!(self.p.apply)),
            ]),
        )?;
        out_last(&obj(vec![("ok", json!(true)), ("theories", json!(written)), ("answered", json!(answers.len()))]))
    }

    // --- check -----------------------------------------------------------------------------------

    fn step_check(&mut self) -> Result<()> {
        let out = if self.p.out.is_empty() { self.p.staging.clone() } else { self.p.out.clone() };
        let key = self.key.clone();
        let staging = self.p.staging.clone();
        let report = check_report(self.zt()?, &key, &out, &staging)?;
        out_last(&report)
    }
}

/// The skeleton heads a crashed allocation wrote but never recorded: a typed Model Card whose body
/// still carries the skeleton's own source line `来源：[[key]] <id>（` for an indexed unit. A replay
/// adopts them instead of writing the units again (recoverable allocation, proposal §5.1 stage 4).
pub(crate) fn recover_allocation(key: &str, units: &[Value], heads: &[Value]) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for u in units {
        let id = field(u, "id");
        let form = field(u, "form");
        let mark = format!("来源：[[{key}]] {id}（");
        // A head of the unit's own form (never a Theory Card) that carries the unit's own source line.
        let found: Vec<&Value> = heads
            .iter()
            .filter(|h| {
                let f = field(h, "form");
                eq(&field(h, "type"), "model")
                    && !f.is_empty()
                    && !eq(&f, "theory")
                    && (form.is_empty() || eq(&f, &form))
                    && field(h, "body").contains(&mark)
            })
            .collect();
        match found.as_slice() {
            [] => {}
            [head] => out.push((id, field(head, "address"))),
            many => bail!(
                "the unit {id} has {} skeleton heads ({}); remove the duplicates before the skeleton runs again",
                many.len(),
                many.iter().map(|h| field(h, "address")).collect::<Vec<_>>().join(", ")
            ),
        }
    }
    Ok(out)
}

/// A Theory Card's `**对象**` section from its answered object lines: the list, or `无。` when the theory
/// has no object beside its main results.
pub(crate) fn objects_section(objects: &[String]) -> String {
    if objects.is_empty() { "**对象**：无。".to_string() } else { format!("**对象**：\n{}", objects.join("\n")) }
}

/// The Theory Card a crashed allocation wrote for `chapter` but never recorded: a `theory` head whose
/// body ends on the skeleton's source line `来源：[[key]] <chapter>`. Two candidates are a fault.
pub(crate) fn recover_theory(key: &str, chapter: &str, heads: &[Value]) -> Result<Option<String>> {
    let mark = format!("来源：[[{key}]] {chapter}");
    let found: Vec<String> = heads
        .iter()
        .filter(|h| eq(&field(h, "form"), "theory") && field(h, "body").trim_end().ends_with(&mark))
        .map(|h| field(h, "address"))
        .collect();
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.into_iter().next()),
        _ => bail!("the chapter {chapter} has {} Theory skeletons ({}); remove the duplicates", found.len(), found.join(", ")),
    }
}

/// The indexed units the tree does not hold as unit heads: an allocation without an address, or an
/// address that is no typed head now. The release gate's denominator is the frozen allocation, not
/// the heads that happen to exist (M0, docs/math-digest-redesign-proposal.md §6.1).
pub(crate) fn missing_units(allocation: &Value, heads: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    let mut taken: Vec<String> = Vec::new();
    for u in pipe(&allocation["units"]) {
        let id = field(u, "id");
        let address = field(u, "address");
        let form = field(u, "form");
        // The head must be a unit head (never a Theory Card) of the unit's own form, used by one unit only.
        let head = heads.iter().find(|h| eq(&field(h, "address"), &address));
        let fits = head.is_some_and(|h| {
            let f = field(h, "form");
            !eq(&f, "theory") && !form.is_empty() && eq(&f, &form)
        });
        if address.is_empty() {
            out.push(format!("{id} (no address)"));
        } else if !fits {
            out.push(format!("{id} [[{address}]]"));
        } else if taken.iter().any(|t| eq(t, &address)) {
            out.push(format!("{id} [[{address}]] (shares its head)"));
        } else {
            taken.push(address);
        }
    }
    out
}

/// The M1–M18 report of one book: every indexed unit and Theory Card with the rules it
/// breaks, without writing anything. `math -Step check` writes it; the ledger's
/// `math-check` compares the same read against the retained audit (R241).
pub(crate) fn check_value(zt: &mut Zt, key: &str, staging: &str) -> Result<Value> {
    let key = key.to_string();
    {
        let tree = read_one(zt, "get_tree", json!({ "tree": key }))?;
        let heads: Vec<Value> = pipe(&tree["cards"])
            .into_iter()
            .filter(|c| eq(&field(c, "type"), "model") && !truthy(&c["facet"]) && truthy(&c["form"]))
            .cloned()
            .collect();
        let models: Vec<Value> = if heads.is_empty() {
            Vec::new()
        } else {
            let calls: Vec<(&str, Value)> = heads.iter().map(|h| ("get_model", json!({ "at": h["address"] }))).collect();
            read_calls(zt, &calls)?
        };
        let mut faults: Vec<Value> = Vec::new();
        let mut add = |rule: &str, address: Value, text: Value| {
            faults.push(obj(vec![("rule", json!(rule)), ("address", address), ("text", text)]));
        };
        // M0: every indexed unit of the frozen allocation is a unit head of the tree now.
        let allocation_path = join(&join(staging, "math"), "units.json");
        if Path::new(&allocation_path).is_file() {
            let allocation = read_json(&allocation_path)?;
            for gap in missing_units(&allocation, &heads) {
                add("M0", json!(key), json!(format!("the indexed unit {gap} is not a unit of the tree")));
            }
        } else {
            add("M0", json!(key), json!("the frozen allocation math/units.json is missing"));
        }
        for (i, head) in heads.iter().enumerate() {
            let model = models.get(i).cloned().unwrap_or(Value::Null);
            let form = field(head, "form");
            let address = field(head, "address");
            let card = read_one(zt, "get_card", json!({ "at": address }))?;
            let body = field(&card, "body");
            let at = || json!(address);
            if eq(&form, "theory") {
                for f in pipe(&model["math"]["faults"]) {
                    add("M17", at(), f["fix"].clone());
                }
                for c in pipe(&model["math"]["cycle"]) {
                    add("M4", at(), c["fix"].clone());
                }
                if crate::report::is_placeholder(&field(&card, "title"), &body) {
                    add("M1", at(), json!("Theory Card still holds 待写"));
                }
                continue;
            }
            let result = is_in(&form, RESULTS);
            let formal = eq(&field(head, "level"), "formal");
            // M1: one claim in the title, no skeleton left.
            let title = field(&card, "title");
            if crate::report::is_placeholder(&title, &body) {
                add("M1", at(), json!("the unit still holds 待写"));
            }
            if ps::len16(&title) > 40 {
                add("M1", at(), json!(format!("title over 40 characters: {title}")));
            }
            // M2: labels.
            let labels: Vec<String> = pipe(&model["math"]["labels"]).into_iter().map(s).collect();
            if result && !labels.iter().any(|l| STARTS_H.is_match(l)) {
                add("M2", at(), json!("a result labels its hypotheses (H1), (H2), …"));
            }
            if eq(&form, "definition") && !labels.iter().any(|l| STARTS_C.is_match(l)) {
                add("M2", at(), json!("a definition labels its clauses (C1), (C2), …"));
            }
            // M3: reach-in.
            for r in pipe(&model["build"]["reach_in"]) {
                add("M3", at(), r["fix"].clone());
            }
            for c in pipe(&model["build"]["closure"]) {
                add("M3", at(), c["fix"].clone());
            }
            // M5: each hypothesis used in a proof part, or explained by its role part.
            let parts = pipe(&model["parts"]);
            let proof: Vec<&&Value> = parts.iter().filter(|p| M5_FACET.is_match(&field(p, "facet"))).collect();
            if result && formal && !proof.is_empty() {
                let text = proof.iter().map(|p| format!("{}\n{}", field(p, "title"), field(p, "body"))).collect::<Vec<_>>().join("\n");
                for h in labels.iter().filter(|l| STARTS_H.is_match(l)) {
                    let used = Regex::new(&format!(r"(?i)\b{h}\b"))?;
                    if !used.is_match(&text) {
                        add("M5", at(), json!(format!("hypothesis {h} appears in no proof step and no 假设作用 part")));
                    }
                }
            }
            // M6, M7, M13, M14: the unit report.
            for f in pipe(&model["math"]["faults"]) {
                let kind = field(f, "fault");
                let rule = if eq(&kind, "no_example") || eq(&kind, "no_nonexample") {
                    "M6"
                } else if eq(&kind, "no_intuition") || eq(&kind, "no_plan") {
                    "M7"
                } else if eq(&kind, "over_budget") {
                    "M13"
                } else {
                    "M14"
                };
                add(rule, at(), f["fix"].clone());
            }
            // M8: the proof plan names its method.
            for p in parts.iter().filter(|p| PLAN_FACET.is_match(&field(p, "facet"))) {
                if !METHODS.is_match(&format!("{}\n{}", field(p, "title"), field(p, "body"))) {
                    add("M8", p["address"].clone(), json!("the proof plan names no method (直接, 反证, 逆否, 归纳, 构造, 分情形, 对角线)"));
                }
            }
            // M9: a formal result states its symbols (the setting's 符号表) or cites the notation table.
            if result && formal {
                let all = parts.iter().map(|p| field(p, "body")).collect::<Vec<_>>().join("\n");
                if !all.contains("符号表") && !pipe(&model["build"]["cited"]).into_iter().any(|c| NOTATION_TITLE.is_match(&field(c, "title"))) {
                    add("M9", at(), json!("no symbol table in the premises and no Link to the notation table"));
                }
            }
            // M10: a supplied part is marked.
            for p in parts.iter().filter(|p| CONTRAST_FACET.is_match(&field(p, "facet"))) {
                let b = field(p, "body");
                if b.contains('补') && !MARKED.is_match(&b) {
                    add("M10", p["address"].clone(), json!("a supplied item carries the mark （补：书中未给）"));
                }
            }
            // M12: an exercise answer stands under ## 答案.
            for p in parts.iter().filter(|p| EXERCISE_FACET.is_match(&field(p, "facet"))) {
                let b = field(p, "body");
                if b.contains("答案") && !ANSWER_HEADING.is_match(&b) {
                    add("M12", p["address"].clone(), json!("an exercise answer stands under ## 答案"));
                }
            }
            // M16: the Model Card states the proof idea in one sentence.
            if let Some(m) = IDEA.captures(&body) {
                let idea = INLINE_MATH.replace_all(&m[1], "x");
                if SENTENCE_END.find_iter(&idea).count() > 1 {
                    add("M16", at(), json!("the Model Card states the proof idea in more than one sentence"));
                }
            }
        }
        let mut by_rule: CiMap<i64> = CiMap::new();
        for f in &faults {
            bump(&mut by_rule, &field(f, "rule"));
        }
        let mut forms: CiMap<i64> = CiMap::new();
        for h in &heads {
            bump(&mut forms, &field(h, "form"));
        }
        // Blocking: M0, and an M1 placeholder (a long title is advice).
        let blocking = faults.iter().filter(|f| eq(&field(f, "rule"), "M0") || (eq(&field(f, "rule"), "M1") && field(f, "text").contains("待写"))).count();
        let units = heads.iter().filter(|h| !eq(&field(h, "form"), "theory")).count();
        let theories = heads.iter().filter(|h| eq(&field(h, "form"), "theory")).count();
        let report = obj(vec![
            ("key", json!(key)),
            ("units", json!(units)),
            ("theories", json!(theories)),
            ("forms", counts(&forms)),
            ("faults", json!(faults.len())),
            ("blocking", json!(blocking)),
            ("by_rule", counts(&by_rule)),
            ("list", json!(faults)),
        ]);
        Ok(report)
    }
}

/// Write the M1–M18 report and its Markdown view, and answer with the step's last line.
pub(crate) fn check_report(zt: &mut Zt, key: &str, out: &str, staging: &str) -> Result<Value> {
    let report = check_value(zt, key, staging)?;
    let units = report["units"].clone();
    let theories = report["theories"].clone();
    let faults = report["faults"].as_u64().unwrap_or_default();
    save_json(&join(out, "math-audit.json"), &report)?;
    let mut md = format!("# 数学规则审计：{key}{NL}{NL}");
    md.push_str(&format!("单元 {units} 个，理论卡 {theories} 张，问题 {faults} 处。{NL}{NL}"));
    md.push_str(&format!("| 规则 | 数量 |{NL}| --- | --- |{NL}"));
    for (k, n) in report["by_rule"].as_object().into_iter().flatten() {
        md.push_str(&format!("| {k} | {n} |{NL}"));
    }
    md.push_str(&format!("{NL}## 问题{NL}{NL}"));
    for f in report["list"].as_array().into_iter().flatten() {
        md.push_str(&format!("- {} [[{}]]：{}{NL}", field(f, "rule"), field(f, "address"), field(f, "text")));
    }
    let report_md = join(out, "math-audit.md");
    util::write_text(&report_md, &md)?;
    Ok(obj(vec![
        ("ok", json!(true)),
        ("units", units),
        ("theories", theories),
        ("forms", report["forms"].clone()),
        ("faults", json!(faults)),
        ("blocking", report["blocking"].clone()),
        ("by_rule", report["by_rule"].clone()),
        ("report", json!(report_md)),
    ]))
}

/// `zt-digest math`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(&util::bind_positional(argv, PARAMS)?, PARAMS, &["Apply"])?;
    let step = a.choice("Step", STEPS)?;
    let staging = a.required("Staging")?;
    let supply = a.str_or("Supply", "examples");
    if !is_in(&supply, &["none", "examples", "examples+answers"]) {
        bail!("Cannot validate argument on parameter 'Supply'. The argument \"{supply}\" does not belong to the set \"none,examples,examples+answers\" specified by the ValidateSet attribute. Supply an argument that is in the set and then try the command again.");
    }
    let p = Params {
        step,
        staging,
        out: a.str("Out"),
        units_per_job: a.int_or("UnitsPerJob", 4)?,
        supply,
        main_results: a.int_or("MainResults", 7)?,
        max_symbols: a.int_or("MaxSymbols", 80)?,
        job: a.str("Job"),
        index_chars: a.int_or("IndexChars", 30000)?,
        apply: a.flag("Apply"),
    };
    // -CoreBudget is a parameter of the script that no step uses.
    a.int_or("CoreBudget", 3)?;
    let manifest = read_json(&join(&p.staging, "manifest.json"))?;
    let key = field(&manifest, "citation_key");
    let title = field(&manifest, "book_title");
    // R328 (§15.2): a math step that allocates a job or writes Cards in the book's Literature tree
    // stops before its first file or Card when that tree is locked by the owner. `index` and `docs`
    // read only the staging book and write prompt or job files (no Card and no archive read), as
    // do `index-check` and `index-apply`, and `check`
    // is the read-only checker (R327): none of them asks.
    let zt = if matches!(p.step.as_str(), "check" | "index" | "index-check" | "index-apply" | "docs") {
        None
    } else {
        let mut client = crate::zt::start()?;
        if let Some(refusal) = crate::lockcheck::preflight(&mut client, "math", &p.step, &key)? {
            return crate::lockcheck::stop("math", &refusal);
        }
        Some(client)
    };
    let work = join(&p.staging, "math");
    std::fs::create_dir_all(&work)?;
    let mut compiler = Compiler { p, zt, manifest, key, title, work };
    match compiler.p.step.as_str() {
        "index" => compiler.step_index()?,
        "index-check" => compiler.step_index_check()?,
        "index-apply" => compiler.step_index_apply()?,
        "docs" => compiler.step_docs()?,
        "skeleton" => compiler.step_skeleton()?,
        "notation" => compiler.step_notation()?,
        "notation-apply" => compiler.step_notation_apply()?,
        "boundary" => compiler.step_boundary()?,
        "boundary-apply" => compiler.step_boundary_apply()?,
        "boundary-settle" => compiler.step_boundary_settle()?,
        "proof" => compiler.step_proof()?,
        "proof-apply" => compiler.step_proof_apply()?,
        "theory" => compiler.step_theory()?,
        "theory-apply" => compiler.step_theory_apply()?,
        _ => compiler.step_check()?,
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(level: &str, order: Value) -> Value {
        json!({"head":{"address":"k/5|a","title":"幂法则","body":"…"},"level":level,"build":{"order":order},"parts":[]})
    }

    #[test]
    fn a_formal_result_the_source_proves_is_reviewed_through_its_main_line_steps() {
        let unit = json!({"form":"theorem","proof":123,"address":"k/5|a"});
        let order = json!([
            {"address":"k/5|a|1","facet":"前提","line":"main"},
            {"address":"k/5|a|2","facet":"证明：二项展开","line":"main"},
            {"address":"k/5|a|3","facet":"例子","line":"main"},
            {"address":"k/5|a|4","facet":"证明：另一法","line":"side"}
        ]);
        match proof_decision(&unit, &model("formal", order)) {
            ProofDecision::Review(steps) => {
                let at: Vec<String> = steps.iter().map(|s| field(s, "address")).collect();
                // The proof review reads the premises as context and owns the plan, the 证明 steps and the
                // conclusion; the compiler's independent reviewer owns the derivations (thompson pilot).
                assert_eq!(at, vec!["k/5|a|2"]);
            }
            other => panic!("expected a review, got {other:?}"),
        }
    }

    #[test]
    fn a_proof_the_source_gives_and_the_unit_lacks_is_an_unmet_obligation() {
        // thompson1914calculus: 96 results had 0 parts, the proof queue was empty, and the
        // stage reported done.
        let unit = json!({"form":"theorem","proof":123,"address":"k/5|a"});
        assert!(matches!(proof_decision(&unit, &model("formal", json!([]))), ProofDecision::Unmet(_)));
        // A unit the model layer never built is unmet too, never skipped.
        assert!(matches!(proof_decision(&unit, &Value::Null), ProofDecision::Unmet(_)));
        // Premises and a conclusion without a single proof step are no proof (Astra round 5).
        let framed = json!([{"address":"k/5|a|1","facet":"前提","line":"main"},{"address":"k/5|a|2","facet":"结论","line":"main"}]);
        assert!(matches!(proof_decision(&unit, &model("formal", framed)), ProofDecision::Unmet(_)));
        // A derivation proves the result (Astra round 6), so the unit is not unmet; the proof review owns its plan
        // and conclusion, and the compiler's reviewer the derivation itself (thompson pilot, 2026-11-22).
        let derived = json!([{"address":"k/5|a|1","facet":"证明思路","line":"main"},{"address":"k/5|a|2","facet":"推导：二项展开","line":"main"},{"address":"k/5|a|3","facet":"结论","line":"main"}]);
        match proof_decision(&unit, &model("formal", derived)) {
            ProofDecision::Review(steps) => assert_eq!(steps.iter().map(|s| field(s, "address")).collect::<Vec<_>>(), vec!["k/5|a|1", "k/5|a|3"]),
            other => panic!("a derivation proof is reviewed through its plan and conclusion, got {other:?}"),
        }
        assert!(!is_proof_step("推导：二项展开") && !is_proof_step("前提"), "derivations and premises are not the proof review's");
        assert!(is_proof_step("证明思路") && is_proof_step("证明：n 为正整数") && is_proof_step("结论"));
        let english = json!([{"address":"k/5|a|1","facet":"Derivation: expand","line":"main"}]);
        assert!(matches!(proof_decision(&unit, &model("formal", english)), ProofDecision::Skip), "a bare derivation proves the result and leaves the proof review nothing");
        let planned = json!([{"address":"k/5|a|1","facet":"证明思路","line":"main"},{"address":"k/5|a|2","facet":"结论","line":"main"}]);
        assert!(matches!(proof_decision(&unit, &model("formal", planned)), ProofDecision::Unmet(_)), "a proof plan is no proof step");
    }

    #[test]
    fn the_notation_table_keeps_each_sense_of_a_symbol_and_only_checked_locators() {
        // Bug G: one row per symbol string, and `第 24 章` written in a 21-chapter book.
        let units = json!({"units":[{"id":"定义「微分 dx」","address":"k/2|a"}]});
        let raw = json!({"symbols":[
            {"symbol":"$dx$","meaning":"x 的一小部分","unit":"定义「微分 dx」"},
            {"symbol":"$dx$","meaning":"x 的一小部分","unit":"定义「微分 dx」"},
            {"symbol":"$C$","meaning":"积分常数","unit":"第 24 章"},
            {"symbol":"$C$","meaning":"圆周长","unit":""}
        ]});
        let (n, _, body) = notation_table(&raw, &units, "k", "微积分", 80);
        assert_eq!(n, 3, "the same symbol with two meanings keeps two rows; an exact repeat is one");
        assert!(body.contains("| $dx$ | x 的一小部分 | 定义「微分 dx」 [[k/2|a]] |"));
        assert!(body.contains("| $C$ | 积分常数 | — |"), "an unchecked locator is not written");
        assert!(body.contains("| $C$ | 圆周长 | — |"));
        assert!(!body.contains("第 24 章"));
        // Astra round 5: a case-insensitive key merged $C$ and $c$; an unresolved first row hid a later locator.
        let raw = json!({"symbols":[
            {"symbol":"$C$","meaning":"常数","unit":"第 3 章"},
            {"symbol":"$c$","meaning":"常数","unit":""},
            {"symbol":"$C$","meaning":"常数","unit":"定义「微分 dx」"}
        ]});
        let (n, _, body) = notation_table(&raw, &units, "k", "微积分", 80);
        assert_eq!(n, 2);
        assert!(body.contains("| $C$ | 常数 | 定义「微分 dx」 [[k/2|a]] |") && body.contains("| $c$ | 常数 | — |"));
    }

    #[test]
    fn a_replay_adopts_the_skeleton_heads_a_crash_left_unrecorded() {
        // Astra round 5: the skeleton writes heads, then saves units.json; a crash between them made a
        // replay allocate the same units twice.
        let units = vec![json!({"id":"定理「幂法则」"}), json!({"id":"定义「微分 dx」"})];
        let heads = vec![
            json!({"address":"k/5|a","type":"model","form":"theorem","body":"**陈述**：待写。\n\n来源：[[k]] 定理「幂法则」（`sections/s05.md:107-123`）"}),
            json!({"address":"k/5|z","type":"model","form":"theorem","body":"**陈述**：待写。\n\n来源：[[other]] 定义「微分 dx」（`x`）"}),
            json!({"address":"k/5|y","type":"idea","body":"来源：[[k]] 定义「微分 dx」（`x`）"}),
        ];
        let adopted = recover_allocation("k", &units, &heads).unwrap();
        assert_eq!(adopted, vec![("定理「幂法则」".to_string(), "k/5|a".to_string())], "only a typed head of this book with the unit's own source line");
        // Astra round 6: the head must have the unit's form, and two candidates are a fault, not a guess.
        let typed = vec![json!({"id":"定理「幂法则」","form":"theorem"})];
        let wrong_form = vec![json!({"address":"k/5|a","type":"model","form":"definition","body":"来源：[[k]] 定理「幂法则」（`x`）"})];
        assert!(recover_allocation("k", &typed, &wrong_form).unwrap().is_empty());
        let twice = vec![
            json!({"address":"k/5|a","type":"model","form":"theorem","body":"来源：[[k]] 定理「幂法则」（`x`）"}),
            json!({"address":"k/5|b","type":"model","form":"theorem","body":"来源：[[k]] 定理「幂法则」（`x`）"}),
        ];
        assert!(recover_allocation("k", &typed, &twice).is_err());
        // The chapter's Theory Card is adopted the same way, by its own source line.
        let theories = vec![
            json!({"address":"k/5|b","type":"model","form":"theory","body":"**结构**：\n- [[k/5|a]]\n\n来源：[[k]] 第四章"}),
            json!({"address":"k/6|b","type":"model","form":"theory","body":"来源：[[k]] 第四章之二"}),
        ];
        assert_eq!(recover_theory("k", "第四章", &theories).unwrap().as_deref(), Some("k/5|b"));
        assert_eq!(recover_theory("k", "第五章", &theories).unwrap(), None);
    }

    #[test]
    fn an_answered_theory_without_objects_says_so_instead_of_keeping_the_placeholder() {
        // thompson pilot, 2026-11-22: the one-member theory 5|b had no object, and the skeleton's
        // `**对象**：待写。` stayed, so the release gate (rightly) found a placeholder.
        assert_eq!(objects_section(&[]), "**对象**：无。");
        assert_eq!(objects_section(&["- [[k/4|b]]：撇号记号".to_string()]), "**对象**：\n- [[k/4|b]]：撇号记号");
        assert!(!crate::report::is_placeholder("理论", &objects_section(&[])));
    }

    #[test]
    fn an_indexed_unit_the_tree_lacks_is_missing() {
        // Astra round 5: a unit with no allocated head contributed 0 placeholders and 0 obligations.
        let head = |a: &str, form: &str| json!({"address":a,"form":form});
        let allocation = json!({"units":[
            {"id":"定理「幂法则」","address":"k/5|a","form":"theorem"},
            {"id":"定理「链式法则」","address":"","form":"theorem"},
            {"id":"定义「微分 dx」","address":"k/2|a","form":"definition"}
        ]});
        let heads = vec![head("k/5|a", "theorem")];
        assert_eq!(missing_units(&allocation, &heads), vec!["定理「链式法则」 (no address)".to_string(), "定义「微分 dx」 [[k/2|a]]".to_string()]);
        assert!(missing_units(&allocation, &[head("k/5|a", "theorem"), head("k/2|a", "definition")]).len() == 1);
        // Astra round 6: an allocation pointing at a Theory Card, or two units on one head, is no unit.
        let theory = json!({"units":[{"id":"定理「幂法则」","address":"k/5|b","form":"theorem"}]});
        assert_eq!(missing_units(&theory, &[head("k/5|b", "theory")]).len(), 1);
        let shared = json!({"units":[{"id":"定理「a」","address":"k/5|a","form":"theorem"},{"id":"定理「b」","address":"k/5|a","form":"theorem"}]});
        assert_eq!(missing_units(&shared, &[head("k/5|a", "theorem")]), vec!["定理「b」 [[k/5|a]] (shares its head)".to_string()]);
        let formless = json!({"units":[{"id":"定理「a」","address":"k/5|a","form":""}]});
        assert_eq!(missing_units(&formless, &[head("k/5|a", "theorem")]).len(), 1, "an allocation without a form names no unit (Astra round 7)");
    }

    #[test]
    fn boundary_work_waits_for_a_written_unit_head() {
        // thompson1914calculus: 123 例子/非例 parts were bolted onto 待写 definitions.
        let head = |title: &str, body: &str| json!({"head":{"title":title,"body":body},"math":{"labels":[]},"parts":[]});
        assert!(!boundary_ready(&head("微分 dx：待写", "**陈述**：待写。")));
        assert!(!boundary_ready(&Value::Null));
        assert!(boundary_ready(&head("微分 dx", "**定义**：(C1) $dx$ 是 $x$ 的一小部分。")));
    }

    #[test]
    fn definitions_concept_results_and_unproved_results_owe_no_proof_review() {
        let def = json!({"form":"definition","proof":0,"address":"k/2|a"});
        assert!(matches!(proof_decision(&def, &model("concept", json!([]))), ProofDecision::Skip));
        let concept = json!({"form":"theorem","proof":40,"address":"k/5|a"});
        assert!(matches!(proof_decision(&concept, &model("concept", json!([]))), ProofDecision::Skip));
        let asserted = json!({"form":"theorem","proof":0,"address":"k/5|a"});
        assert!(matches!(proof_decision(&asserted, &model("formal", json!([]))), ProofDecision::Skip));
    }
}
