//! `zt-digest term-cards`: the Term Card compiler (design 12.1–12.6, S18 / #159).
//!
//!   zt-digest term-cards -Step plan|jobs|check|review-plan|fix-plan|apply|report
//!                        -Staging <folder> [-PerJob 4] [-MathPerJob 1]
//!                        [-MaxStepsPerCard 12] [-MaxProse 1600]
//!                        [-Jobs <ids>] [-Token <claim>] [-Apply]
//!
//! One book's Term list is read by role: a Term Card with no name is the list, and a pre-term
//! book's legacy `术语表` table is adapted when no Term list exists. Each row is bound to the
//! staged source: its exact spelling, avoid words, meaning, first-definition locator, the
//! definition sentences with `sections/<file>:<line>`, every section where it occurs, the book's
//! notation rows that name it, its neighbours, and a math/concept classification with the
//! evidence for that decision. Rows are grouped into stable jobs (one math term or four concept
//! terms by default) whose ids come from the frozen row numbers, and each job owns exactly one
//! output file under `out/`.
//!
//! The files under `<Staging>/term-cards/` are the compiler's recoverable artifacts:
//!
//!   plan.json    frozen rows, list/source fingerprints, notation, limits, jobs, plan revision,
//!                compiler/checker/rule versions and the plan hash
//!   jobs/        one source-bound job file per job
//!   out/         writer JSON, one stable output per job (written by the writer)
//!   check/       diagnostics (S19)
//!   review/      hash-bound review answers (S19)
//!   state.json   derived local progress, plan revisions and the ledger unit ids
//!   report.json, report.md   publication coverage (S21)
//!
//! The plan step is read-only over the archive: it calls `get_tree`/`get_cards` and writes no
//! Card. When the staging folder is a prepared run (`ledger.sqlite3`), the plan registers the
//! existing ledger's `termcards/tNNN@<plan revision>` job units and the `termcards/book@<plan
//! revision>` unit; the existing run ledger remains the scheduler and `state.json` is never a
//! second queue. `jobs` reads one claimed job unit and prints the DeepSeek job for that writer.
//!
//! `check`, `review-plan`, `fix-plan`, `apply` and `report` are declared and dispatched here but
//! belong to S19 (#162) and S21 (#165); until those slices fill them this tool answers one
//! compact `not_implemented` line and exits 2, writing nothing.

use crate::args::Args;
use crate::canon;
use crate::organize::common::{self, pstr, Sb};
use crate::ps;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

// --- the command surface -------------------------------------------------------------------------

/// The declared steps, in command order (design 12.1).
pub const STEPS: [&str; 7] = [
    "plan",
    "jobs",
    "check",
    "review-plan",
    "fix-plan",
    "apply",
    "report",
];

/// The work folder under `-Staging`.
const WORK: &str = "term-cards";

pub const DEFAULT_PER_JOB: i64 = 4;
pub const DEFAULT_MATH_PER_JOB: i64 = 1;
pub const DEFAULT_MAX_STEPS: i64 = 12;
pub const DEFAULT_MAX_PROSE: i64 = 1600;
/// The bounds the option parser accepts. They are deliberately wider than the defaults: a slice
/// may lower a limit for one book, never switch grouping off or set an unbounded prose budget.
const PER_JOB_RANGE: (i64, i64) = (1, 8);
const MATH_PER_JOB_RANGE: (i64, i64) = (1, 8);
const MAX_STEPS_RANGE: (i64, i64) = (1, 64);
const MAX_PROSE_RANGE: (i64, i64) = (1, 20_000);

/// A definition sentence, as the term table's own `terms` stage looked for it.
static DEFINES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(称为|叫做|定义为|是指|即所谓|我们把)").unwrap());
/// One formula span. It is a candidate signal outside a definition and evidence inside one.
static FORMULA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\$[^$]*\$\$|\$[^$\n]+\$").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
/// The legacy term table's title prefix (`术语表：《…》的 N 个术语`).
static LEGACY_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^术语表").unwrap());
/// The book notation table's title prefix (`符号表：《…》的 N 个符号`).
static NOTATION_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^符号表").unwrap());

/// The compact answer of a step whose implementation belongs to a later slice.
fn not_implemented(step: &str) -> (i32, Value) {
    let (slice, issue) = match step {
        "check" | "review-plan" | "fix-plan" => ("S19", 162),
        "apply" | "report" => ("S21", 165),
        _ => ("S18", 159),
    };
    (
        2,
        json!({
            "schema_version": 1,
            "ok": false,
            "step": step,
            "code": "not_implemented",
            "slice": slice,
            "issue": issue,
        }),
    )
}

/// One invocation's parsed parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Params {
    pub(crate) step: String,
    pub(crate) staging: String,
    pub(crate) per_job: i64,
    pub(crate) math_per_job: i64,
    pub(crate) max_steps: i64,
    pub(crate) max_prose: i64,
    pub(crate) jobs: Vec<String>,
    pub(crate) token: Option<i64>,
    pub(crate) apply: bool,
}

impl Params {
    pub(crate) fn work(&self) -> String {
        ps::join(&self.staging, WORK)
    }
}

/// The declared parameters and switch of the command contract (S18-AC01).
const NAMES: [&str; 8] = [
    "Step",
    "Staging",
    "PerJob",
    "MathPerJob",
    "MaxStepsPerCard",
    "MaxProse",
    "Jobs",
    "Token",
];

pub(crate) fn params(argv: &[String]) -> Result<Params> {
    let a = Args::parse(&util::bind_positional(argv, &NAMES)?, &NAMES, &["Apply"])?;
    let step = a.choice("Step", &STEPS)?;
    let staging = common::mandatory(&a, "Staging")?;
    if staging.trim().is_empty() {
        bail!("Cannot bind argument to parameter 'Staging' because it is an empty string.");
    }
    let per_job = canon::integer_option(
        &a.str_or("PerJob", &DEFAULT_PER_JOB.to_string()),
        "PerJob",
        PER_JOB_RANGE.0,
        PER_JOB_RANGE.1,
    )?;
    let math_per_job = canon::integer_option(
        &a.str_or("MathPerJob", &DEFAULT_MATH_PER_JOB.to_string()),
        "MathPerJob",
        MATH_PER_JOB_RANGE.0,
        MATH_PER_JOB_RANGE.1,
    )?;
    let max_steps = canon::integer_option(
        &a.str_or("MaxStepsPerCard", &DEFAULT_MAX_STEPS.to_string()),
        "MaxStepsPerCard",
        MAX_STEPS_RANGE.0,
        MAX_STEPS_RANGE.1,
    )?;
    let max_prose = canon::integer_option(
        &a.str_or("MaxProse", &DEFAULT_MAX_PROSE.to_string()),
        "MaxProse",
        MAX_PROSE_RANGE.0,
        MAX_PROSE_RANGE.1,
    )?;
    let jobs = job_ids(&a.str("Jobs"))?;
    let token = if a.has("Token") {
        Some(
            a.required("Token")?
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|value| *value > 0)
                .context("-Token must be a positive integer")?,
        )
    } else {
        None
    };
    let apply = a.flag("Apply");
    if (step == "apply") != apply {
        bail!("term-cards apply requires -Apply; no other step accepts it");
    }
    if token.is_some() && step == "plan" {
        bail!("term-cards plan is not a claimed step; remove -Token");
    }
    if !jobs.is_empty() && matches!(step.as_str(), "plan" | "apply" | "report") {
        bail!("term-cards {step} is not job-scoped; remove -Jobs");
    }
    Ok(Params {
        step,
        staging,
        per_job,
        math_per_job,
        max_steps,
        max_prose,
        jobs,
        token,
        apply,
    })
}

/// `-Jobs t001,t002`: trimmed ids, each once, in the order given.
fn job_ids(text: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for part in text.split(',') {
        let id = part.trim();
        if id.is_empty() {
            continue;
        }
        if out.iter().any(|seen: &String| seen == id) {
            bail!("-Jobs names `{id}` twice");
        }
        out.push(id.to_string());
    }
    Ok(out)
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let p = params(argv)?;
    match p.step.as_str() {
        "plan" => plan(&p),
        "jobs" => jobs(&p),
        other => {
            let (code, value) = not_implemented(other);
            util::print_line(&canon::canonical_json(&value))?;
            Ok(code)
        }
    }
}

// --- the list read -------------------------------------------------------------------------------

/// One Term row, exactly as the list's table spells it (S18-AC02).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Row {
    pub(crate) row: usize,
    pub(crate) term: String,
    pub(crate) avoid: Vec<String>,
    pub(crate) meaning: String,
    pub(crate) defined: String,
}

/// The book's Term list as the compiler reads it: a Term Card with no name, or a pre-term
/// book's legacy `术语表` table. The title and body are raw authored bytes, so the plan
/// fingerprints exactly what the archive holds (S18-AC02).
#[derive(Clone, Debug)]
pub(crate) struct ListRead {
    pub(crate) address: String,
    pub(crate) card_type: String,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) legacy: bool,
    pub(crate) rows: Vec<Row>,
}

/// The list cards of one tree: Term Cards with no name are lists by role; a legacy table is an
/// uncompiled `术语表` Card whose type is not `term`. The two kinds are never mixed silently.
fn list_candidates(cards: &[Value]) -> (Vec<Value>, Vec<Value>) {
    let mut lists = Vec::new();
    let mut legacy = Vec::new();
    for card in cards {
        let card_type = pstr(card, "type");
        let term = card.get("term").cloned().unwrap_or(Value::Null);
        let titled = LEGACY_TITLE.is_match(&pstr(card, "title"));
        if card_type == "term" && term.is_null() {
            lists.push(card.clone());
        } else if card_type != "term" && titled {
            legacy.push(card.clone());
        }
    }
    (lists, legacy)
}

/// Read the one Term list of one book tree (R0314, S18-AC06). `read` fetches the full Card the
/// selected address names; it is called once, after the role selection, so an ambiguous tree
/// reads no body and writes nothing.
pub(crate) fn select_list(
    cards: &[Value],
    mut read: impl FnMut(&str) -> Result<Value>,
) -> Result<ListRead> {
    let (lists, legacy) = list_candidates(cards);
    let (candidate, is_legacy) = match (lists.as_slice(), legacy.as_slice()) {
        ([one], []) => (one, false),
        ([], [one]) => (one, true),
        ([], []) => bail!(
            "the book's Literature tree has no Term list and no legacy 术语表 table; run the Terms stage first"
        ),
        _ => {
            let mut names = Vec::new();
            for card in lists.iter().chain(legacy.iter()) {
                names.push(format!("{} ({})", pstr(card, "address"), pstr(card, "title")));
            }
            bail!(
                "the book's Literature tree holds {} Term lists/legacy tables ({}); name one list or repair the tree",
                names.len(),
                names.join("; ")
            )
        }
    };
    let address = pstr(candidate, "address");
    let card = read(&address)?;
    let title = pstr(&card, "title");
    let body = pstr(&card, "body");
    let rows = parsed_rows(&body)?;
    Ok(ListRead {
        address,
        card_type: pstr(&card, "type"),
        title,
        body,
        legacy: is_legacy,
        rows,
    })
}

/// The exact rows of one list body. A duplicated spelling is ambiguous: the compatibility
/// reader would fold it into one row, so planning stops before allocating any job (S18-AC06).
pub(crate) fn parsed_rows(body: &str) -> Result<Vec<Row>> {
    let duplicates = zt_check::text::term_row_duplicates(body);
    if !duplicates.is_empty() {
        let named: Vec<String> = duplicates
            .iter()
            .map(|group| {
                format!(
                    "{} (rows {})",
                    group.spellings.join(", "),
                    group
                        .rows
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .collect();
        bail!(
            "the Term list repeats a term: {}; one row per term is required before planning",
            named.join("; ")
        );
    }
    let raw = zt_check::text::term_rows_raw(body);
    if raw.is_empty() {
        bail!("the Term list holds no four-column row");
    }
    Ok(raw
        .into_iter()
        .enumerate()
        .map(|(index, row)| Row {
            row: index + 1,
            term: row.term,
            avoid: row.avoid,
            meaning: row.meaning,
            defined: row.defined,
        })
        .collect())
}

// --- grouping, the frozen plan and the job files -------------------------------------------------

/// The effective limits the plan freezes (R0304).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Limits {
    pub(crate) per_job: i64,
    pub(crate) math_per_job: i64,
    pub(crate) max_steps: i64,
    pub(crate) max_prose: i64,
}

impl Limits {
    fn json(self) -> Value {
        json!({
            "per_job": self.per_job,
            "math_per_job": self.math_per_job,
            "max_steps_per_card": self.max_steps,
            "max_prose": self.max_prose,
        })
    }
}

/// The book facts the plan names.
#[derive(Clone, Debug)]
pub(crate) struct BookInfo {
    pub(crate) citation_key: String,
    pub(crate) title: String,
    pub(crate) variant: String,
}

/// One writer job: its stable id from the first frozen row, its terms and its exact output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct JobSpec {
    pub(crate) id: String,
    pub(crate) kind: &'static str,
    pub(crate) rows: Vec<usize>,
    pub(crate) terms: Vec<String>,
}

/// Group the frozen rows in table order: math terms by `math_per_job` (one by default), concept
/// terms by `per_job` (four by default). A class change starts a new job, so a job never mixes
/// math and concept rows; the id is `t<first row>` and never depends on processing order
/// (S18-AC04, R0317).
pub(crate) fn group_jobs(
    evidence: &[RowEvidence],
    per_job: i64,
    math_per_job: i64,
) -> Vec<JobSpec> {
    let mut jobs = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut kind = "";
    let flush = |current: &mut Vec<usize>, kind: &str, jobs: &mut Vec<JobSpec>| {
        if current.is_empty() {
            return;
        }
        let rows: Vec<usize> = current.iter().map(|index| evidence[*index].row.row).collect();
        let terms: Vec<String> = current
            .iter()
            .map(|index| evidence[*index].row.term.clone())
            .collect();
        jobs.push(JobSpec {
            id: format!("t{:03}", rows[0]),
            kind: if kind == "math" { "math" } else { "concept" },
            rows,
            terms,
        });
        current.clear();
    };
    for (index, row) in evidence.iter().enumerate() {
        let row_kind = row.classification.kind;
        let cap = if row_kind == "math" { math_per_job } else { per_job } as usize;
        if !current.is_empty() && (kind != row_kind || current.len() >= cap) {
            flush(&mut current, kind, &mut jobs);
        }
        kind = row_kind;
        current.push(index);
    }
    flush(&mut current, kind, &mut jobs);
    jobs
}

/// The book notation rows and where they were read.
#[derive(Clone, Debug, Default)]
pub(crate) struct NotationRead {
    pub(crate) sources: Vec<Value>,
    pub(crate) rows: Vec<NotationRow>,
}

impl NotationRead {
    pub(crate) fn json(&self) -> Value {
        json!({
            "sources": self.sources,
            "rows": self.rows.iter().map(|row| json!({
                "symbol": row.symbol, "meaning": row.meaning, "defined": row.defined,
            })).collect::<Vec<_>>(),
        })
    }
}

/// Everything one plan read from the staging folder and the archive.
pub(crate) struct PlanInputs<'a> {
    pub(crate) staging: &'a str,
    pub(crate) book: BookInfo,
    pub(crate) list: ListRead,
    pub(crate) sections: Vec<Section>,
    pub(crate) planning_artifact: Option<PlanningArtifact>,
    pub(crate) notation: NotationRead,
    pub(crate) settings: Vec<SettingSymbols>,
    pub(crate) limits: Limits,
}

impl PlanInputs<'_> {
    fn work(&self) -> String {
        ps::join(self.staging, WORK)
    }
}

/// The artifacts one plan run produces. `plan.json` is reused byte for byte when the frozen
/// content did not change; the jobs and the instruction are written every time so a lost file is
/// restored without a new revision.
#[derive(Debug)]
pub(crate) struct BuiltPlan {
    pub(crate) plan: Value,
    pub(crate) state: Value,
    pub(crate) instruction: String,
    pub(crate) job_files: BTreeMap<String, String>,
    pub(crate) incomplete: Vec<Value>,
}

/// The instruction file's placeholders (Appendix A).
const INSTRUCTION: &str = "dsh-term-card-instructions.zh.md";

/// Build the frozen plan, its state and every prompt file. This function writes nothing.
pub(crate) fn build_plan(
    inputs: &PlanInputs<'_>,
    existing_plan: Option<&Value>,
    existing_state: Option<&Value>,
) -> Result<BuiltPlan> {
    let work = inputs.work();
    let artifact = inputs.planning_artifact.as_ref();
    let mut evidence = Vec::new();
    for row in &inputs.list.rows {
        evidence.push(row_evidence(
            row,
            &inputs.sections,
            artifact,
            &inputs.notation.rows,
            &inputs.settings,
        )?);
    }
    let mut incomplete = Vec::new();
    for item in &evidence {
        if item.row.meaning.trim().is_empty()
            && item.snippets.is_empty()
            && item.occurrences.is_empty()
        {
            bail!(
                "term row {} `{}` has no meaning and no staged source passage; repair the list or the staged source before planning",
                item.row.row,
                item.row.term
            );
        }
        if item.snippets.is_empty() {
            incomplete.push(json!({
                "row": item.row.row,
                "term": item.row.term,
                "reason": "no_definition_sentence",
            }));
        }
        if item.row.meaning.trim().is_empty() {
            incomplete.push(json!({
                "row": item.row.row,
                "term": item.row.term,
                "reason": "empty_meaning",
            }));
        }
    }
    let jobs = group_jobs(&evidence, inputs.limits.per_job, inputs.limits.math_per_job);
    let mut job_of_row: BTreeMap<usize, String> = BTreeMap::new();
    for job in &jobs {
        for row in &job.rows {
            job_of_row.insert(*row, job.id.clone());
        }
    }
    let neighbours = |index: usize| -> Vec<Value> {
        let mut out = Vec::new();
        for at in [index.saturating_sub(1), index + 1] {
            if at != index && at < evidence.len() {
                out.push(json!({
                    "row": evidence[at].row.row,
                    "term": evidence[at].row.term,
                    "meaning": evidence[at].row.meaning,
                }));
            }
        }
        out
    };
    let rows: Vec<Value> = evidence
        .iter()
        .enumerate()
        .map(|(index, item)| {
            json!({
                "row": item.row.row,
                "term": item.row.term,
                "avoid": item.row.avoid,
                "meaning": item.row.meaning,
                "defined": item.row.defined,
                "snippets": item.snippets.iter().map(|snippet| json!({
                    "file": snippet.file, "line": snippet.line, "text": snippet.text,
                    "defines": snippet.defines,
                })).collect::<Vec<_>>(),
                "occurrences": item.occurrences.iter().map(|occurrence| json!({
                    "file": occurrence.file, "count": occurrence.count,
                    "first_line": occurrence.first_line,
                })).collect::<Vec<_>>(),
                "notation": notation_matches(&item.row.term, &inputs.notation.rows).into_iter().map(|row| json!({
                    "symbol": row.symbol, "meaning": row.meaning, "defined": row.defined,
                })).collect::<Vec<_>>(),
                "neighbors": neighbours(index),
                "planning_artifact": item.planning_artifact,
                "classification": {
                    "kind": item.classification.kind,
                    "evidence": item.classification.evidence,
                    "candidates": {
                        "formula_occurrences": item.classification.formula_candidates,
                        "setting_term_tables": item.classification.setting_term_tables,
                    },
                    "promoted": item.classification.promoted,
                },
                "job": job_of_row.get(&item.row.row),
            })
        })
        .collect();
    let jobs_json: Vec<Value> = jobs
        .iter()
        .map(|job| {
            json!({
                "id": job.id,
                "kind": job.kind,
                "rows": job.rows,
                "terms": job.terms,
                "file": ps::join(&ps::join(&work, "jobs"), &format!("{}.md", job.id)),
                "output": ps::join(&ps::join(&work, "out"), &format!("{}.json", job.id)),
            })
        })
        .collect();
    let source_files: Vec<Value> = inputs
        .sections
        .iter()
        .map(|section| {
            json!({
                "file": section.file,
                "path": section.path,
                "sha256": section.sha256,
                "lines": section.lines.len(),
            })
        })
        .collect();
    let source_fingerprint = canon::hash_value(&json!(source_files
        .iter()
        .map(|file| [file["file"].clone(), file["sha256"].clone()])
        .collect::<Vec<_>>()));
    let artifact_json = inputs.planning_artifact.as_ref().map(|artifact| {
        json!({"path": artifact.path, "sha256": artifact.sha256, "terms": artifact.terms.len()})
    });
    // The shared instruction is part of the compiler contract: a changed instruction is a new
    // frozen plan, never a silent rewrite of the same revision.
    let instruction_source = instruction_source()?;
    let frozen = json!({
        "schema_version": 1,
        "kind": "term_cards_plan",
        "book": {
            "citation_key": inputs.book.citation_key,
            "title": inputs.book.title,
            "variant": inputs.book.variant,
        },
        "list": {
            "address": inputs.list.address,
            "type": inputs.list.card_type,
            "legacy": inputs.list.legacy,
            "title": inputs.list.title,
            "title_sha256": canon::sha256_hex(inputs.list.title.as_bytes()),
            "body_sha256": canon::sha256_hex(inputs.list.body.as_bytes()),
            "row_count": inputs.list.rows.len(),
        },
        "source": {
            "staging": inputs.staging,
            "sections": source_files,
            "fingerprint": source_fingerprint,
        },
        "notation": inputs.notation.json(),
        "planning_artifact": artifact_json,
        "limits": inputs.limits.json(),
        "rows": rows,
        "jobs": jobs_json,
        "versions": {
            "compiler": env!("CARGO_PKG_VERSION"),
            "checker": zt_check::VERSION,
            "rule": zt_check::contract::RULE_VERSION,
            "instruction": {
                "file": INSTRUCTION,
                "sha256": canon::sha256_hex(instruction_source.as_bytes()),
            },
        },
    });
    let plan_hash = canon::hash_value(&frozen);
    let (revision, revisions) = revision_of(existing_plan, existing_state, &frozen, &plan_hash);
    let mut plan = frozen.clone();
    plan["revision"] = json!(revision);
    plan["plan_hash"] = json!(plan_hash);
    plan["created_at"] = json!(canon::utc_now_o());
    if let Some(previous) = existing_plan.filter(|previous| {
        previous_plan_hash(previous).as_deref() == Some(plan_hash.as_str())
    }) {
        // Same frozen content: keep the accepted plan byte for byte (no new revision).
        plan = previous.clone();
    }
    let instruction = render_instruction(&instruction_source, inputs);
    let mut job_files = BTreeMap::new();
    for job in &jobs_json {
        job_files.insert(pstr(job, "id"), job_text(inputs, &plan, job, &evidence)?);
    }
    let state = state_of(existing_state, &plan, revision, revisions, &jobs_json);
    Ok(BuiltPlan {
        plan,
        state,
        instruction,
        job_files,
        incomplete,
    })
}

/// The plan hash that identifies an existing plan's frozen content.
fn previous_plan_hash(plan: &Value) -> Option<String> {
    plan.get("plan_hash")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// The previous revision and its history: `state.json` is the derived local progress, and an
/// older `plan.json` still answers when the state was lost.
fn previous_revision(plan: Option<&Value>, state: Option<&Value>) -> (Option<i64>, Option<String>) {
    let from_state = state.map(|state| {
        (
            state.get("plan_revision").and_then(Value::as_i64),
            state
                .get("plan_hash")
                .and_then(Value::as_str)
                .map(str::to_string),
        )
    });
    match from_state {
        Some((Some(revision), hash)) => (Some(revision), hash),
        _ => (
            plan.and_then(|plan| plan.get("revision"))
                .and_then(Value::as_i64),
            plan.and_then(previous_plan_hash),
        ),
    }
}

/// The new revision: unchanged when the frozen content is unchanged, else one more than the
/// previous plan (S18-AC06). The revision list is the local, small history of plan hashes.
fn revision_of(
    existing_plan: Option<&Value>,
    existing_state: Option<&Value>,
    frozen: &Value,
    plan_hash: &str,
) -> (i64, Vec<Value>) {
    let (previous, previous_hash) = previous_revision(existing_plan, existing_state);
    let same = previous_hash.as_deref() == Some(plan_hash);
    let revision = match previous {
        Some(previous) if same => previous,
        Some(previous) => previous + 1,
        None => 1,
    };
    let mut revisions = Vec::new();
    if let Some(state) = existing_state
        && let Some(list) = state.get("revisions").and_then(Value::as_array)
    {
        revisions.extend(list.iter().cloned());
    }
    let entry = json!({
        "revision": revision,
        "plan_hash": plan_hash,
        "list_sha256": frozen["list"]["body_sha256"],
        "source_fingerprint": frozen["source"]["fingerprint"],
        "at": canon::utc_now_o(),
    });
    if !revisions
        .iter()
        .any(|known| known["revision"] == json!(revision) && known["plan_hash"] == json!(plan_hash))
    {
        revisions.push(entry);
    }
    (revision, revisions)
}

/// The derived local progress. A new revision starts every job over; the same revision keeps a
/// job's state and a promotion an earlier slice recorded.
fn state_of(
    existing: Option<&Value>,
    plan: &Value,
    revision: i64,
    revisions: Vec<Value>,
    jobs: &[Value],
) -> Value {
    let same = existing.is_some_and(|old| {
        old.get("plan_revision").and_then(Value::as_i64) == Some(revision)
            && old.get("plan_hash").and_then(Value::as_str)
                == plan.get("plan_hash").and_then(Value::as_str)
    });
    let previous_jobs = if same {
        existing
            .and_then(|old| old.get("jobs"))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default()
    } else {
        Map::new()
    };
    let mut job_state = Map::new();
    for job in jobs {
        let id = pstr(job, "id");
        let previous = previous_jobs.get(&id).cloned();
        job_state.insert(
            id,
            previous.unwrap_or_else(|| {
                json!({
                    "state": "planned",
                    "file": job["file"],
                    "output": job["output"],
                    "started_at": Value::Null,
                })
            }),
        );
    }
    let promotions = if same {
        existing
            .and_then(|old| old.get("promotions"))
            .cloned()
            .unwrap_or_else(|| json!([]))
    } else {
        json!([])
    };
    json!({
        "schema_version": 1,
        "kind": "term_cards_state",
        "book": plan["book"]["citation_key"],
        "plan_revision": revision,
        "plan_hash": plan["plan_hash"],
        "revisions": revisions,
        "jobs": job_state,
        "units": if same { existing.and_then(|old| old.get("units")).cloned().unwrap_or(Value::Null) } else { Value::Null },
        "promotions": promotions,
        "receipts": if same { existing.and_then(|old| old.get("receipts")).cloned().unwrap_or(json!({})) } else { json!({}) },
    })
}

/// The shared instruction file's source text (Appendix A).
fn instruction_source() -> Result<String> {
    util::read_text(ps::join(&common::scripts_dir(), INSTRUCTION))
        .with_context(|| format!("missing the term-card instruction file {INSTRUCTION}"))
}

/// The rendered shared instruction: the plan's own values filled in, no placeholder left.
fn render_instruction(source: &str, inputs: &PlanInputs<'_>) -> String {
    source
        .replace("{{KEY}}", &inputs.book.citation_key)
        .replace("{{STAGING}}", inputs.staging)
        .replace("{{MAX_STEPS}}", &inputs.limits.max_steps.to_string())
        .replace("{{MAX_PROSE}}", &inputs.limits.max_prose.to_string())
}

/// One job file: the exact rows it owns, their source evidence, the book's other terms, the
/// notation when the job may need it, and the one output path it owns (R0308, S18-AC04).
fn job_text(
    inputs: &PlanInputs<'_>,
    plan: &Value,
    job: &Value,
    evidence: &[RowEvidence],
) -> Result<String> {
    let work = inputs.work();
    let id = pstr(job, "id");
    let rows: Vec<usize> = job["rows"]
        .as_array()
        .context("incomplete plan job rows")?
        .iter()
        .filter_map(Value::as_u64)
        .map(|row| row as usize)
        .collect();
    let mut sb = Sb::new();
    sb.line(format!(
        "# 术语卡任务 {id}（术语表第 {} 行）",
        rows.iter().map(usize::to_string).collect::<Vec<_>>().join("、")
    ))
    .blank();
    sb.line(format!(
        "- 本书：`[[{}]]`《{}》",
        inputs.book.citation_key, inputs.book.title
    ));
    sb.line(format!(
        "- 共享说明（先读它）：`{}`",
        ps::join(&work, INSTRUCTION)
    ));
    sb.line(format!(
        "- 只写一个 JSON 文件：`{}`",
        pstr(job, "output")
    ));
    sb.line(format!("- 本书原文小节：`{}`", ps::join(inputs.staging, "sections")));
    sb.line(format!(
        "- 冻结计划（只读，核对行号与指纹）：`{}`",
        ps::join(&work, "plan.json")
    ));
    sb.line(format!(
        "- 任务绑定：plan_revision={} plan_hash=`{}` job=`{id}` rows={}",
        pstr(plan, "revision"),
        pstr(plan, "plan_hash"),
        rows.iter().map(usize::to_string).collect::<Vec<_>>().join(",")
    ))
    .blank();
    if pstr(job, "kind") == "math" {
        sb.line("本任务含数学概念：按共享说明写全数学部分，定义公式、符号、推导、极限和数值例子都不能省略。")
            .blank();
    } else {
        sb.line("本任务按概念术语分组。若其中某个术语实际是数学概念，`category` 写 `math` 并按共享说明写全数学部分；该术语仍在本组完成，但会单独审查后才能发布。")
            .blank();
    }
    sb.line("## 本组术语").blank();
    for row_number in &rows {
        let item = evidence
            .iter()
            .find(|item| item.row.row == *row_number)
            .with_context(|| format!("plan job {id} names row {row_number} that the plan does not hold"))?;
        sb.line(format!("### {} {}", item.row.row, item.row.term)).blank();
        let avoid = if item.row.avoid.is_empty() {
            "—".to_string()
        } else {
            item.row.avoid.join("、")
        };
        sb.line(format!("- 不用：{avoid}"));
        sb.line(format!(
            "- 意思：{}",
            if item.row.meaning.trim().is_empty() { "（表里没有）" } else { item.row.meaning.as_str() }
        ));
        sb.line(format!(
            "- 首次定义：{}",
            if item.row.defined.trim().is_empty() { "（表里没有）" } else { item.row.defined.as_str() }
        ));
        let artifact_first = item
            .planning_artifact
            .as_ref()
            .map(|record| pstr(record, "section"))
            .unwrap_or_default();
        if !artifact_first.is_empty() {
            sb.line(format!("- 术语任务给的首次定义：{artifact_first}"));
        }
        let classification = &item.classification;
        let kind = if classification.kind == "math" { "数学概念" } else { "概念" };
        let reasons: Vec<String> = classification
            .evidence
            .iter()
            .map(|record| match pstr(record, "kind").as_str() {
                "definition_formula" => format!(
                    "定义句含公式 `{}:{}`",
                    pstr(record, "file"),
                    pstr(record, "line")
                ),
                "meaning_formula" => "表里的意思含公式".to_string(),
                "notation_table" => format!("本书符号表列为 {}", pstr(record, "symbol")),
                "setting_symbol" => format!(
                    "设定 `{}` 的符号表列为 {}",
                    pstr(record, "address"),
                    pstr(record, "symbol")
                ),
                "setting_term_table" => format!(
                    "设定 `{}` 的术语表列为本词",
                    pstr(record, "address")
                ),
                _ => String::new(),
            })
            .filter(|reason| !reason.is_empty())
            .collect();
        let mut why = if reasons.is_empty() {
            format!(
                "没有公式定义、符号表或设定把它列为符号；全小节出现公式 {} 处（只作候选）",
                classification.formula_candidates
            )
        } else {
            reasons.join("；")
        };
        if kind == "概念" && !classification.setting_term_tables.is_empty() {
            why.push_str(&format!(
                "；设定的术语表也列有本词：{}（写全数学部分即可提升为数学概念）",
                classification
                    .setting_term_tables
                    .iter()
                    .map(|address| format!("`{address}`"))
                    .collect::<Vec<_>>()
                    .join("、")
            ));
        }
        sb.line(format!("- 初始分类：{kind}（依据：{why}）"));
        if item.snippets.is_empty() {
            sb.line("- 定义句：（没有找到；读下面的出现小节）");
        } else {
            sb.line("- 定义句：");
            for snippet in &item.snippets {
                sb.line(format!(
                    "  - `{}:{}` {}",
                    snippet.file, snippet.line, snippet.text
                ));
            }
        }
        if item.occurrences.is_empty() {
            sb.line("- 出现：本书小节里没有找到这个词");
        } else {
            let occurrences: Vec<String> = item
                .occurrences
                .iter()
                .map(|occurrence| {
                    format!(
                        "`{}`（{} 次，首行 {}）",
                        occurrence.file, occurrence.count, occurrence.first_line
                    )
                })
                .collect();
            sb.line(format!("- 出现：{}", occurrences.join("、")));
        }
        if !item.classification.evidence.is_empty() {
            let symbols: Vec<String> = item
                .classification
                .evidence
                .iter()
                .filter(|record| {
                    matches!(pstr(record, "kind").as_str(), "notation_table" | "setting_symbol")
                })
                .map(|record| {
                    format!("`{}`（{}）", pstr(record, "symbol"), pstr(record, "meaning"))
                })
                .collect();
            if !symbols.is_empty() {
                sb.line(format!("- 相关符号：{}", symbols.join("、")));
            }
        }
        let neighbor = {
            let index = evidence
                .iter()
                .position(|item| item.row.row == *row_number)
                .context("plan job row missing from evidence")?;
            let mut rows = Vec::new();
            for at in [index.checked_sub(1), Some(index + 1)] {
                if let Some(at) = at.filter(|at| *at < evidence.len()) {
                    rows.push(format!(
                        "`{}`（第 {} 行，{}）",
                        evidence[at].row.term, evidence[at].row.row, evidence[at].row.meaning
                    ));
                }
            }
            rows
        };
        if !neighbor.is_empty() {
            sb.line(format!("- 相邻术语：{}", neighbor.join("；")));
        }
        sb.blank();
    }
    sb.line("## 本书其他术语（`distinctions` 用这里的原名）").blank();
    let own: BTreeSet<usize> = rows.iter().copied().collect();
    let others: Vec<String> = evidence
        .iter()
        .filter(|item| !own.contains(&item.row.row))
        .map(|item| format!("- {}：{}", item.row.term, item.row.meaning))
        .collect();
    if others.is_empty() {
        sb.line("（本书没有其他术语）");
    } else {
        for line in others {
            sb.line(line);
        }
    }
    sb.blank();
    sb.line("## 本书符号表").blank();
    if inputs.notation.rows.is_empty() && inputs.settings.is_empty() {
        sb.line("（没有符号表；按原文用符号）");
    } else {
        for source in &inputs.notation.sources {
            let kind = pstr(source, "kind");
            if kind == "card" {
                sb.line(format!(
                    "- 本书符号表卡片：`{}`（{} 行）",
                    pstr(source, "address"),
                    pstr(source, "rows")
                ));
            } else if kind == "file" {
                sb.line(format!("- 本书记号文件：`{}`", pstr(source, "path")));
            } else if kind == "setting" {
                sb.line(format!(
                    "- 设定 `{}` 的符号表：`{}`",
                    pstr(source, "address"),
                    pstr(source, "title")
                ));
            }
        }
        if pstr(job, "kind") == "math" {
            sb.blank();
            sb.line("| 符号 | 含义 | 定义于 |");
            sb.line("| --- | --- | --- |");
            let mut shown: Vec<NotationRow> = inputs.notation.rows.clone();
            for setting in &inputs.settings {
                for row in &setting.symbols {
                    if !shown.contains(row) {
                        shown.push(row.clone());
                    }
                }
            }
            for row in &shown {
                sb.line(format!(
                    "| {} | {} | {} |",
                    row.symbol,
                    row.meaning.replace('|', "/"),
                    row.defined
                ));
            }
        }
    }
    sb.blank();
    sb.line("## 输出").blank();
    sb.line(format!(
        "把结果写入 `{}`，UTF-8，只写这一个文件。",
        pstr(job, "output")
    ));
    sb.line(format!(
        "格式：`{{\"terms\":[…]}}`。本组只写这些术语，拼写必须和任务文件完全一致：{}。",
        rows_terms(job)
    ));
    sb.line("写完后只回复一行 JSON：`{\"terms\":术语数,\"math\":数学概念数,\"steps\":推导总步数}`。");
    Ok(sb.0)
}

fn rows_terms(job: &Value) -> String {
    job_terms(job)
        .iter()
        .map(|term| format!("`{term}`"))
        .collect::<Vec<_>>()
        .join("、")
}

/// The job's terms as plain strings.
fn job_terms(job: &Value) -> Vec<String> {
    job["terms"]
        .as_array()
        .map(|terms| {
            terms
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

// --- plan and jobs -------------------------------------------------------------------------------

/// The writer's prompt for one job: the job file carries the exact paths and named terms.
const PROMPT: &str =
    "按共享说明和任务文件，写好本组术语的 JSON 草稿，只写任务文件指定的输出文件，最后只回复一行 JSON。";

fn plan(p: &Params) -> Result<i32> {
    let staging = p.staging.as_str();
    let manifest = common::manifest(staging)?;
    let key = pstr(&manifest, "citation_key");
    if key.trim().is_empty() {
        bail!("manifest.json names no citation_key; term-card planning needs its book");
    }
    let book_title = {
        let title = pstr(&manifest, "book_title");
        if title.trim().is_empty() { key.clone() } else { title }
    };
    let variant = pstr(&manifest["run_contract"], "variant");
    let mut zt = crate::zt::start()?;
    if let Some(refusal) = crate::lockcheck::preflight(&mut zt, "term-cards", "plan", &key)? {
        return crate::lockcheck::stop("term-cards", &refusal);
    }
    let tree = zt.call("get_tree", json!({"tree": key}))?;
    let archive_revision = tree["archive_revision"]
        .as_u64()
        .context("the tree read carries no archive revision")?;
    let cards = tree["cards"].as_array().cloned().unwrap_or_default();
    let list = {
        let mut read = |at: &str| zt.call("get_card", json!({"at": at}));
        select_list(&cards, &mut read)?
    };
    let notation = read_notation(&mut zt, staging, &cards, &key)?;
    let settings = read_settings(&mut zt, &cards, &list)?;
    let mut notation = notation;
    for setting in &settings {
        notation.sources.push(json!({
            "kind": "setting",
            "address": setting.address,
            "title": setting.title,
            "symbols": setting.symbols.len(),
            "terms": setting.terms.len(),
        }));
    }
    let sections = read_sections(staging)?;
    let planning_artifact = read_planning_artifact(staging)?;
    let after = zt.call("get_cards", json!({"addresses":[],"view":"text"}))?;
    if after["archive_revision"].as_u64() != Some(archive_revision) {
        bail!(
            "the archive changed while planning; run -Step plan again for a new plan revision"
        );
    }
    let inputs = PlanInputs {
        staging,
        book: BookInfo {
            citation_key: key.clone(),
            title: book_title,
            variant,
        },
        list,
        sections,
        planning_artifact,
        notation,
        settings,
        limits: Limits {
            per_job: p.per_job,
            math_per_job: p.math_per_job,
            max_steps: p.max_steps,
            max_prose: p.max_prose,
        },
    };
    let work = p.work();
    let plan_path = ps::join(&work, "plan.json");
    let state_path = ps::join(&work, "state.json");
    let existing_plan = read_existing(&plan_path, "plan")?;
    let existing_state = read_existing(&state_path, "state")?;
    let mut built = build_plan(&inputs, existing_plan.as_ref(), existing_state.as_ref())?;
    std::fs::create_dir_all(ps::join(&work, "jobs"))?;
    std::fs::create_dir_all(ps::join(&work, "out"))?;
    std::fs::create_dir_all(ps::join(&work, "check"))?;
    std::fs::create_dir_all(ps::join(&work, "review"))?;
    util::write_atomic(ps::join(&work, INSTRUCTION), &built.instruction)?;
    for (id, text) in &built.job_files {
        util::write_atomic(
            ps::join(&ps::join(&work, "jobs"), &format!("{id}.md")),
            text,
        )?;
    }
    if existing_plan
        .as_ref()
        .is_none_or(|old| previous_plan_hash(old).as_deref() != Some(pstr(&built.plan, "plan_hash").as_str()))
    {
        util::write_atomic(&plan_path, &ps::json_pretty(&built.plan))?;
    }
    if std::path::Path::new(&ps::join(staging, "ledger.sqlite3")).is_file() {
        let prepared = crate::ledger::Prepared::read(staging, &mut zt)?;
        let units = crate::ledger::term_cards_register(&prepared, &built.plan)?;
        built.state["units"] = units["units"].clone();
    }
    util::write_atomic(&state_path, &ps::json_pretty(&built.state))?;
    let jobs: Vec<Value> = built.plan["jobs"]
        .as_array()
        .context("incomplete plan jobs")?
        .iter()
        .map(|job| {
            json!({
                "id": job["id"],
                "kind": job["kind"],
                "rows": job["rows"],
                "terms": job["terms"],
                "file": job["file"],
                "output": job["output"],
            })
        })
        .collect();
    let math = built.plan["rows"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter(|row| row["classification"]["kind"] == "math")
                .count()
        })
        .unwrap_or(0);
    let terms = built.plan["list"]["row_count"].as_u64().unwrap_or(0) as usize;
    let summary = json!({
        "schema_version": 1,
        "ok": true,
        "step": "plan",
        "book": key,
        "revision": built.plan["revision"],
        "plan_hash": built.plan["plan_hash"],
        "terms": terms,
        "math": math,
        "concept": terms.saturating_sub(math),
        "jobs": jobs,
        "complete": built.incomplete.is_empty(),
        "incomplete": built.incomplete,
        "units": built.state["units"],
        "plan": plan_path,
        "instruction": ps::join(&work, INSTRUCTION),
        "work": work,
    });
    util::print_line(&canon::canonical_json(&summary))?;
    Ok(0)
}

/// A previously published compiler artifact (`plan.json` or `state.json`): absent is `None`, and
/// an unreadable file is a refusal rather than a silent replacement.
fn read_existing(path: &str, kind: &str) -> Result<Option<Value>> {
    if !std::path::Path::new(path).is_file() {
        return Ok(None);
    }
    let value = util::read_json(path)?;
    if value["schema_version"] != 1 || pstr(&value, "kind") != format!("term_cards_{kind}") {
        bail!("{path} is not a term-cards {kind} artifact; repair or remove it");
    }
    Ok(Some(value))
}

/// The notation-table cards of one tree: titles starting `符号表`, except a named Term Card that
/// happens to define the word 符号表.
fn notation_candidates(cards: &[Value]) -> Vec<&Value> {
    cards
        .iter()
        .filter(|card| {
            NOTATION_TITLE.is_match(&pstr(card, "title"))
                && !(pstr(card, "type") == "term" && card["term"].is_string())
        })
        .collect()
}

/// The notation rows of one book: the `符号表` Card (one), the staged `notation.md` (math runs
/// write it), or both. A second notation Card is ambiguous and stops the plan.
fn read_notation(
    zt: &mut Zt,
    staging: &str,
    cards: &[Value],
    key: &str,
) -> Result<NotationRead> {
    let mut notation = NotationRead::default();
    let candidates = notation_candidates(cards);
    if candidates.len() > 1 {
        let names: Vec<String> = candidates
            .iter()
            .map(|card| pstr(card, "address"))
            .collect();
        bail!(
            "the book's Literature tree holds {} notation tables ({}); keep one before planning",
            names.len(),
            names.join(", ")
        );
    }
    if let Some(card) = candidates.first() {
        let address = pstr(card, "address");
        let full = zt.call("get_card", json!({"at": address}))?;
        let body = pstr(&full, "body");
        let rows = notation_rows(&body);
        notation.sources.push(json!({
            "kind": "card",
            "address": address,
            "title": pstr(&full, "title"),
            "sha256": canon::sha256_hex(body.as_bytes()),
            "rows": rows.len(),
            "book": key,
        }));
        notation.rows.extend(rows);
    }
    let file = ps::join(staging, "notation.md");
    if std::path::Path::new(&file).is_file() {
        let text = util::read_text(&file)?;
        let rows = notation_rows(&text);
        notation.sources.push(json!({
            "kind": "file",
            "path": file,
            "sha256": canon::sha256_hex(text.as_bytes()),
            "rows": rows.len(),
        }));
        for row in rows {
            if !notation.rows.contains(&row) {
                notation.rows.push(row);
            }
        }
    }
    Ok(notation)
}

/// At most this many Model setting Cards are read for their symbol tables.
const MAX_SETTING_CARDS: usize = 256;

/// The symbol tables of the book's Model setting Cards (a `setting` or `设定` facet), when the
/// model layer has run at plan time. Only rows that name a list term are relevant here, so the
/// plan freezes exactly the notation a classification used.
fn read_settings(
    zt: &mut Zt,
    cards: &[Value],
    list: &ListRead,
) -> Result<Vec<SettingSymbols>> {
    let addresses: Vec<String> = cards
        .iter()
        .filter(|card| pstr(card, "type") == "model" && is_setting_facet(&pstr(card, "facet")))
        .map(|card| pstr(card, "address"))
        .collect();
    if addresses.len() > MAX_SETTING_CARDS {
        bail!(
            "the book has {} Model setting Cards, more than the {MAX_SETTING_CARDS} this plan reads; split the book before planning",
            addresses.len()
        );
    }
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let read = zt.cards(&addresses, "text")?;
    let mut settings = Vec::new();
    for (card, address) in read.cards.iter().zip(&addresses) {
        let body = pstr(card, "body");
        let symbols: Vec<NotationRow> = notation_rows(&body)
            .into_iter()
            .filter(|row| {
                list.rows.iter().any(|list_row| {
                    !notation_matches(&list_row.term, std::slice::from_ref(row)).is_empty()
                })
            })
            .collect();
        let mut terms: Vec<String> = zt_check::text::term_rows_raw(&body)
            .into_iter()
            .map(|row| row.term)
            .filter(|term| list.rows.iter().any(|list_row| &list_row.term == term))
            .collect();
        terms.sort_by(|a, b| canon::culture_cmp_cs(a, b));
        terms.dedup();
        if !symbols.is_empty() || !terms.is_empty() {
            settings.push(SettingSymbols {
                address: address.clone(),
                title: pstr(card, "title"),
                symbols,
                terms,
            });
        }
    }
    Ok(settings)
}

fn jobs(p: &Params) -> Result<i32> {
    let work = p.work();
    let plan_path = ps::join(&work, "plan.json");
    if !std::path::Path::new(&plan_path).is_file() {
        bail!("no term-cards plan at `{plan_path}`; run -Step plan first");
    }
    let plan = util::read_json(&plan_path)?;
    if plan["schema_version"] != 1 || plan["kind"] != "term_cards_plan" {
        bail!("`{plan_path}` is not a term-cards plan");
    }
    let frozen = frozen_of(&plan)?;
    let plan_hash = pstr(&plan, "plan_hash");
    if canon::hash_value(&frozen) != plan_hash {
        bail!("`{plan_path}` changed since it was frozen; rerun -Step plan");
    }
    let all = plan["jobs"].as_array().context("incomplete plan jobs")?;
    let selected: Vec<&Value> = if p.jobs.is_empty() {
        all.iter().collect()
    } else {
        let mut out = Vec::new();
        for id in &p.jobs {
            let job = all
                .iter()
                .find(|job| pstr(job, "id") == *id)
                .with_context(|| format!("the plan has no job `{id}`"))?;
            out.push(job);
        }
        out
    };
    let instruction = ps::join(&work, INSTRUCTION);
    if !std::path::Path::new(&instruction).is_file() {
        bail!("the shared instruction file `{instruction}` is missing; rerun -Step plan");
    }
    let mut unit = Value::Null;
    let mut token = Value::Null;
    if let Some(claim) = p.token {
        if p.jobs.len() != 1 {
            bail!("-Token claims exactly one job; name one -Jobs id");
        }
        let job = selected[0];
        let unit_id = format!("termcards/{}@{}", pstr(job, "id"), pstr(&plan, "revision"));
        let mut zt = crate::zt::start()?;
        let prepared = crate::ledger::Prepared::read(&p.staging, &mut zt)?;
        let claimed = crate::ledger::term_cards_claim(&prepared, &unit_id, claim)?;
        let expected = canon::hash_value(&job_unit_inputs(&plan, job));
        if claimed["input_hash"] != json!(expected) {
            bail!("the claimed job unit `{unit_id}` is bound to another plan input; rerun -Step plan");
        }
        unit = json!(unit_id);
        token = json!(claim);
        record_job_started(&work, job)?;
    }
    let printed: Vec<Value> = selected
        .iter()
        .map(|job| {
            let file = pstr(job, "file");
            if !std::path::Path::new(&file).is_file() {
                bail!("the job file `{file}` is missing; rerun -Step plan");
            }
            Ok(json!({
                "id": job["id"],
                "kind": job["kind"],
                "rows": job["rows"],
                "terms": job["terms"],
                "files": [instruction, file],
                "prompt": PROMPT,
                "output": job["output"],
                "permissions": {"archive_read": true, "archive_write": false},
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let summary = json!({
        "schema_version": 1,
        "ok": true,
        "step": "jobs",
        "plan_revision": plan["revision"],
        "plan_hash": plan_hash,
        "unit": unit,
        "token": token,
        "jobs": printed,
    });
    util::print_line(&canon::canonical_json(&summary))?;
    Ok(0)
}

/// Record that a claimed job has started, in the compiler's derived state only.
fn record_job_started(work: &str, job: &Value) -> Result<()> {
    let state_path = ps::join(work, "state.json");
    if !std::path::Path::new(&state_path).is_file() {
        return Ok(());
    }
    let mut state = util::read_json(&state_path)?;
    let id = pstr(job, "id");
    if let Some(entry) = state["jobs"].get_mut(&id) {
        entry["state"] = json!("started");
        entry["started_at"] = json!(canon::utc_now_o());
    }
    util::write_atomic(&state_path, &ps::json_pretty(&state))
}

/// The frozen content of a plan: every key except the revision, hash and creation time that are
/// derived from it. This is the plan hash's exact input, both at plan time and when jobs reads
/// the plan back.
fn frozen_of(plan: &Value) -> Result<Value> {
    let mut map = plan.as_object().context("plan is not a JSON object")?.clone();
    for key in ["revision", "plan_hash", "created_at"] {
        map.remove(key);
    }
    Ok(Value::Object(map))
}

/// The ledger input of one job unit: the frozen job plus the plan and source identity it came
/// from. The same function builds the register input and the claim check.
pub(crate) fn job_unit_inputs(plan: &Value, job: &Value) -> Value {
    json!({
        "schema_version": 1,
        "kind": "term_card_job",
        "book": plan["book"]["citation_key"],
        "plan_revision": plan["revision"],
        "plan_hash": plan["plan_hash"],
        "list_sha256": plan["list"]["body_sha256"],
        "source_fingerprint": plan["source"]["fingerprint"],
        "job": {
            "id": job["id"],
            "kind": job["kind"],
            "rows": job["rows"],
            "terms": job["terms"],
        },
    })
}

/// The ledger input of the book unit: every job id and its unit, so the book waits on all jobs.
pub(crate) fn book_unit_inputs(plan: &Value) -> Value {
    let revision = pstr(plan, "revision");
    let jobs: Vec<Value> = plan["jobs"]
        .as_array()
        .map(|jobs| {
            jobs.iter()
                .map(|job| {
                    json!({
                        "id": job["id"],
                        "unit": format!("termcards/{}@{}", pstr(job, "id"), revision),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    json!({
        "schema_version": 1,
        "kind": "term_cards_book",
        "book": plan["book"]["citation_key"],
        "plan_revision": plan["revision"],
        "plan_hash": plan["plan_hash"],
        "list_sha256": plan["list"]["body_sha256"],
        "source_fingerprint": plan["source"]["fingerprint"],
        "rows": plan["list"]["row_count"],
        "jobs": jobs,
    })
}

// --- the source evidence and the classification --------------------------------------------------

/// One staged source section (`sections/sNN.md`).
#[derive(Clone, Debug)]
pub(crate) struct Section {
    pub(crate) file: String,
    pub(crate) path: String,
    pub(crate) sha256: String,
    pub(crate) lines: Vec<String>,
}

/// One source line that carries a term's spelling: a definition sentence when `defines`, else a
/// plain occurrence the writer may read around.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Snippet {
    pub(crate) file: String,
    pub(crate) line: usize,
    pub(crate) text: String,
    pub(crate) defines: bool,
}

/// One section file where a term occurs, with its exact occurrence count and first line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Occurrence {
    pub(crate) file: String,
    pub(crate) count: usize,
    pub(crate) first_line: usize,
}

/// One row of a notation table: the book's `符号表` or a Model setting's symbol table.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct NotationRow {
    pub(crate) symbol: String,
    pub(crate) meaning: String,
    pub(crate) defined: String,
}

/// One Model setting Card's naming evidence: its symbol table and its term table. Either names a
/// term that the book's formal setting uses (R0316).
#[derive(Clone, Debug)]
pub(crate) struct SettingSymbols {
    pub(crate) address: String,
    pub(crate) title: String,
    pub(crate) symbols: Vec<NotationRow>,
    pub(crate) terms: Vec<String>,
}

/// Whether a Model facet label is the setting part. Current Cards store the common facet as
/// `setting`; archives written by the Chinese prompts store `设定`, and a variant may add a
/// qualifier after it.
pub(crate) fn is_setting_facet(facet: &str) -> bool {
    facet.eq_ignore_ascii_case("setting") || facet.contains("设定")
}

/// One row's frozen source evidence (S18-AC02, R0315).
#[derive(Clone, Debug)]
pub(crate) struct RowEvidence {
    pub(crate) row: Row,
    pub(crate) snippets: Vec<Snippet>,
    pub(crate) occurrences: Vec<Occurrence>,
    pub(crate) planning_artifact: Option<Value>,
    pub(crate) classification: Classification,
}

/// The initial math/concept classification and the exact evidence for it (S18-AC03, R0316).
#[derive(Clone, Debug)]
pub(crate) struct Classification {
    pub(crate) kind: &'static str,
    pub(crate) evidence: Vec<Value>,
    pub(crate) formula_candidates: usize,
    /// Settings whose term table names the term. That is naming context, not formal notation: a
    /// writer may promote the term to math, but the plan does not classify it as math on this
    /// evidence alone (R0316).
    pub(crate) setting_term_tables: Vec<String>,
    /// A later slice records a DeepSeek concept-to-math promotion here; the plan always states
    /// the initial classification and never claims a promotion on its own.
    pub(crate) promoted: bool,
}

/// Read the staged source sections in culture order. `index.md` is the catalogue, not a section.
pub(crate) fn read_sections(staging: &str) -> Result<Vec<Section>> {
    let dir = ps::join(staging, "sections");
    if !std::path::Path::new(&dir).is_dir() {
        bail!("the staged source has no `{dir}` folder; run prepare first");
    }
    let mut names = ps::child_files(&dir, "s", ".md");
    names.retain(|name| !name.eq_ignore_ascii_case("index.md"));
    ps::ps_sort(&mut names, |a, b| ps::culture_cmp(a, b));
    if names.is_empty() {
        bail!("the staged source has no section file in `{dir}`");
    }
    let mut sections = Vec::new();
    for name in names {
        let path = ps::join(&dir, &name);
        let text = util::read_text(&path)?;
        sections.push(Section {
            file: format!("sections/{name}"),
            sha256: canon::sha256_hex(text.as_bytes()),
            lines: ps::split_lines(&text),
            path,
        });
    }
    Ok(sections)
}

/// One line of evidence: white-space runs made one space, cut to `max` UTF-16 units with `…`.
fn clip(text: &str, max: usize) -> String {
    let collapsed = SPACES.replace_all(text, " ");
    let trimmed = collapsed.trim();
    if ps::len16(trimmed) <= max {
        return trimmed.to_string();
    }
    format!("{}…", ps::cut16(trimmed, max))
}

/// The formula spans of one text, exactly as written.
pub(crate) fn formulas(text: &str) -> Vec<String> {
    FORMULA
        .find_iter(text)
        .map(|found| found.as_str().trim().to_string())
        .filter(|formula| formula.len() > 2)
        .collect()
}

/// The source evidence of one row (R0315): definition sentences with `sections/<file>:<line>`,
/// every section where the term occurs, the formula candidates outside definitions, and the
/// planning artifact's own locator when it names the term.
pub(crate) fn row_evidence(
    row: &Row,
    sections: &[Section],
    planning: Option<&PlanningArtifact>,
    notation: &[NotationRow],
    settings: &[SettingSymbols],
) -> Result<RowEvidence> {
    let mut snippets = Vec::new();
    let mut occurrences = Vec::new();
    let mut fallback: Vec<Snippet> = Vec::new();
    let mut formula_candidates = 0;
    for section in sections {
        let mut count = 0;
        let mut first_line = 0;
        for (index, line) in section.lines.iter().enumerate() {
            let found = line.matches(&row.term).count();
            if found == 0 {
                continue;
            }
            count += found;
            if first_line == 0 {
                first_line = index + 1;
            }
            let has_formula = FORMULA.is_match(line);
            let defines = DEFINES.is_match(line);
            if has_formula && !defines {
                formula_candidates += found;
            }
            let snippet = Snippet {
                file: section.file.clone(),
                line: index + 1,
                text: clip(line, 240),
                defines,
            };
            if defines {
                if snippets.len() < 6 {
                    snippets.push(snippet);
                }
            } else if fallback.len() < 3 {
                fallback.push(snippet);
            }
        }
        if count > 0 {
            occurrences.push(Occurrence {
                file: section.file.clone(),
                count,
                first_line,
            });
        }
    }
    if snippets.is_empty() {
        snippets = fallback;
    }
    let coverage = RowCoverage {
        term: &row.term,
        meaning: &row.meaning,
        snippets: &snippets,
        notation,
        settings,
        formula_candidates,
    };
    let classification = classify(&coverage);
    Ok(RowEvidence {
        row: row.clone(),
        snippets,
        occurrences,
        planning_artifact: planning.and_then(|artifact| artifact.row(&row.term)),
        classification,
    })
}

/// The book's Terms planning artifact (`<Staging>/terms/out/terms.json`): the exact first-definition
/// locator and meaning the earlier Terms job chose, kept as evidence beside the list's own row.
#[derive(Clone, Debug)]
pub(crate) struct PlanningArtifact {
    pub(crate) path: String,
    pub(crate) sha256: String,
    terms: BTreeMap<String, Value>,
}

impl PlanningArtifact {
    fn row(&self, term: &str) -> Option<Value> {
        self.terms.get(term).map(|row| {
            json!({
                "path": self.path,
                "sha256": self.sha256,
                "section": pstr(row, "section"),
                "meaning": pstr(row, "meaning"),
            })
        })
    }
}

/// Read the existing Terms planning artifact when it exists (R0314). A malformed artifact is a
/// refusal: it is input the plan names, never silently ignored.
pub(crate) fn read_planning_artifact(staging: &str) -> Result<Option<PlanningArtifact>> {
    let path = ps::join(&ps::join(&ps::join(staging, "terms"), "out"), "terms.json");
    if !std::path::Path::new(&path).is_file() {
        return Ok(None);
    }
    let text = util::read_text(&path)?;
    let value = canon::parse_json(&text).context("invalid Terms planning artifact")?;
    let mut terms = BTreeMap::new();
    for row in ps::pipe(&value["terms"]) {
        let term = pstr(row, "term").trim().to_string();
        if term.is_empty() {
            bail!("invalid Terms planning artifact: a row names no term");
        }
        terms.entry(term).or_insert_with(|| row.clone());
    }
    Ok(Some(PlanningArtifact {
        path,
        sha256: canon::sha256_hex(text.as_bytes()),
        terms,
    }))
}

/// The notation rows of one body: every Markdown table whose header's first cell names a symbol
/// (`符号`). A book `符号表` and a Model setting's symbol table share this parser, so both are read
/// once and the same way.
pub(crate) fn notation_rows(body: &str) -> Vec<NotationRow> {
    let mut rows: Vec<NotationRow> = Vec::new();
    let mut in_table = false;
    for line in ps::split_lines(body) {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            in_table = false;
            continue;
        }
        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect();
        if !in_table {
            in_table = cells
                .first()
                .is_some_and(|cell| cell.contains("符号") && cells.len() >= 2);
            continue;
        }
        if cells
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':' | ' ')))
        {
            continue;
        }
        if cells.len() < 2 || cells[0].is_empty() {
            continue;
        }
        let row = NotationRow {
            symbol: cells[0].clone(),
            meaning: cells[1].clone(),
            defined: cells.get(2).cloned().unwrap_or_default(),
        };
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    rows
}

/// The notation rows that name a term: its exact spelling in the meaning, or as the symbol.
pub(crate) fn notation_matches<'a>(
    term: &str,
    rows: &'a [NotationRow],
) -> Vec<&'a NotationRow> {
    rows.iter()
        .filter(|row| row.meaning.contains(term) || row.symbol.trim() == term)
        .collect()
}

/// The classification evidence of one row. Math requires a formal definition, the book's
/// notation, or a Model setting's symbol table to name the term; a formula outside a definition
/// stays a candidate signal (R0316).
pub(crate) fn classify(
    coverage: &RowCoverage<'_>,
) -> Classification {
    let mut evidence: Vec<Value> = Vec::new();
    let mut setting_term_tables = Vec::new();
    for snippet in coverage.snippets.iter().filter(|snippet| snippet.defines) {
        if let Some(formula) = formulas(&snippet.text).into_iter().next() {
            evidence.push(json!({
                "kind": "definition_formula",
                "file": snippet.file,
                "line": snippet.line,
                "formula": formula,
            }));
        }
    }
    if let Some(formula) = formulas(&coverage.meaning).into_iter().next() {
        evidence.push(json!({"kind": "meaning_formula", "formula": formula}));
    }
    for row in notation_matches(&coverage.term, coverage.notation) {
        evidence.push(json!({
            "kind": "notation_table",
            "source": "book",
            "symbol": row.symbol,
            "meaning": row.meaning,
        }));
    }
    for setting in coverage.settings {
        for row in notation_matches(&coverage.term, &setting.symbols) {
            evidence.push(json!({
                "kind": "setting_symbol",
                "address": setting.address,
                "symbol": row.symbol,
                "meaning": row.meaning,
            }));
        }
        if setting.terms.iter().any(|term| term == coverage.term) {
            setting_term_tables.push(setting.address.clone());
        }
    }
    evidence.truncate(12);
    Classification {
        kind: if evidence.is_empty() { "concept" } else { "math" },
        evidence,
        formula_candidates: coverage.formula_candidates,
        setting_term_tables,
        promoted: false,
    }
}

/// The inputs of one row's classification.
pub(crate) struct RowCoverage<'a> {
    pub(crate) term: &'a str,
    pub(crate) meaning: &'a str,
    pub(crate) snippets: &'a [Snippet],
    pub(crate) notation: &'a [NotationRow],
    pub(crate) settings: &'a [SettingSymbols],
    pub(crate) formula_candidates: usize,
}

/// Assemble the evidence and classification of one row from its source sections.
#[cfg(test)]
mod tests {
    use super::*;

    fn argv(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn every_declared_step_and_option_is_accepted() {
        for step in STEPS {
            let mut args = vec!["-Step", step, "-Staging", "D:\\s"];
            if step == "apply" {
                args.push("-Apply");
            }
            let parsed = params(&argv(&args)).unwrap();
            assert_eq!(parsed.step, step);
        }
        let p = params(&argv(&[
            "-Step",
            "plan",
            "-Staging",
            "D:\\s",
            "-PerJob",
            "3",
            "-MathPerJob",
            "2",
            "-MaxStepsPerCard",
            "9",
            "-MaxProse",
            "1200",
        ]))
        .unwrap();
        assert_eq!(
            (p.per_job, p.math_per_job, p.max_steps, p.max_prose),
            (3, 2, 9, 1200)
        );
        let defaults = params(&argv(&["-Step", "jobs", "-Staging", "D:\\s"])).unwrap();
        assert_eq!(
            (
                defaults.per_job,
                defaults.math_per_job,
                defaults.max_steps,
                defaults.max_prose
            ),
            (DEFAULT_PER_JOB, DEFAULT_MATH_PER_JOB, DEFAULT_MAX_STEPS, DEFAULT_MAX_PROSE)
        );
    }

    #[test]
    fn a_malformed_command_is_refused_before_any_read() {
        assert!(params(&argv(&["-Step", "nope", "-Staging", "s"])).is_err());
        assert!(params(&argv(&["-Step", "plan"])).is_err());
        assert!(params(&argv(&["-Step", "plan", "-Staging", " "])).is_err());
        assert!(params(&argv(&["-Step", "plan", "-Staging", "s", "-PerJob", "0"])).is_err());
        assert!(params(&argv(&["-Step", "plan", "-Staging", "s", "-PerJob", "9"])).is_err());
        assert!(params(&argv(&["-Step", "plan", "-Staging", "s", "-Apply"])).is_err());
        assert!(params(&argv(&["-Step", "apply", "-Staging", "s"])).is_err());
        assert!(params(&argv(&["-Step", "plan", "-Staging", "s", "-Token", "3"])).is_err());
        assert!(params(&argv(&["-Step", "jobs", "-Staging", "s", "-Token", "0"])).is_err());
        assert!(params(&argv(&["-Step", "plan", "-Staging", "s", "-Jobs", "t001"])).is_err());
        assert!(
            params(&argv(&["-Step", "jobs", "-Staging", "s", "-Jobs", "t001,t001"])).is_err()
        );
        assert_eq!(
            params(&argv(&["-Step", "jobs", "-Staging", "s", "-Jobs", " t001, t002 "]))
                .unwrap()
                .jobs,
            vec!["t001", "t002"]
        );
    }

    fn card(address: &str, card_type: &str, term: Option<&str>, title: &str) -> Value {
        json!({"address":address,"type":card_type,"term":term,"title":title})
    }

    const TABLE: &str = "术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 剩余控制权 | 残余控制权 | 权利 | §1 |\n| 敲竹杠 | 要挟 | 机会主义 | §2 |\n";

    fn read_returns(card: Value) -> impl FnMut(&str) -> Result<Value> {
        let card = card.clone();
        move |_| Ok(card.clone())
    }

    #[test]
    fn the_term_list_is_read_by_its_role_and_keeps_the_exact_rows() {
        let cards = vec![
            card("K", "idea", None, "The Book"),
            card("K/1|a", "term", None, "术语表：《The Book》的 2 个术语"),
        ];
        let list = select_list(&cards, read_returns(json!({
            "address":"K/1|a","type":"term","term":null,
            "title":"术语表：《The Book》的 2 个术语","body":TABLE
        })))
        .unwrap();
        assert!(!list.legacy);
        assert_eq!(list.address, "K/1|a");
        assert_eq!(list.rows.len(), 2);
        assert_eq!(list.rows[0].term, "剩余控制权");
        assert_eq!(list.rows[0].avoid, vec!["残余控制权"]);
        assert_eq!(list.rows[0].defined, "§1");
        assert_eq!(list.rows[1].row, 2);
        // A retyped list keeps its compiled role even though its title is the legacy one.
        let retyped = vec![card("K/1|a", "term", None, "术语表：《The Book》的 2 个术语")];
        assert!(!select_list(&retyped, read_returns(json!({
            "address":"K/1|a","type":"term","title":"术语表","body":TABLE
        }))).unwrap().legacy);
    }

    #[test]
    fn a_pre_term_book_adapts_its_legacy_table() {
        let cards = vec![
            card("K/1|a", "idea", None, "术语表：《The Book》的 2 个术语"),
            card("K/2|a", "idea", None, "Unrelated"),
        ];
        let list = select_list(&cards, read_returns(json!({
            "address":"K/1|a","type":"idea","term":null,
            "title":"术语表：《The Book》的 2 个术语","body":TABLE
        })))
        .unwrap();
        assert!(list.legacy);
        assert_eq!(list.rows[0].term, "剩余控制权");
    }

    #[test]
    fn no_list_or_several_lists_stop_planning() {
        assert!(
            select_list(&[card("K", "idea", None, "The Book")], |_| unreachable!())
                .unwrap_err()
                .to_string()
                .contains("no Term list")
        );
        let two = vec![
            card("K/1|a", "term", None, "A"),
            card("K/2|a", "term", None, "B"),
        ];
        assert!(
            select_list(&two, |_| unreachable!())
                .unwrap_err()
                .to_string()
                .contains("2 Term lists")
        );
        let both = vec![
            card("K/1|a", "term", None, "A"),
            card("K/2|a", "idea", None, "术语表：《K》的 2 个术语"),
        ];
        assert!(select_list(&both, |_| unreachable!()).is_err());
    }

    #[test]
    fn duplicate_rows_stop_before_any_job() {
        let body = TABLE.replace(
            "| 敲竹杠 | 要挟 | 机会主义 | §2 |",
            "| 剩余控制权 | 别的 | 另一个意思 | §2 |",
        );
        let error = parsed_rows(&body).unwrap_err().to_string();
        assert!(error.contains("repeats a term"), "{error}");
        assert!(error.contains("剩余控制权"), "{error}");
    }

    #[test]
    fn steps_owned_by_later_slices_answer_compactly_and_write_nothing() {
        for step in ["check", "review-plan", "fix-plan", "apply", "report"] {
            let (code, value) = not_implemented(step);
            assert_eq!(code, 2);
            assert_eq!(value["ok"], false);
            assert_eq!(value["code"], "not_implemented");
            assert_eq!(value["step"], step);
        }
    }

    // --- source evidence and classification ------------------------------------------------------

    fn section(name: &str, text: &str) -> Section {
        Section {
            file: format!("sections/{name}"),
            path: format!("D:\\s\\sections\\{name}"),
            sha256: canon::sha256_hex(text.as_bytes()),
            lines: ps::split_lines(text),
        }
    }

    fn row(number: usize, term: &str, meaning: &str) -> Row {
        Row {
            row: number,
            term: term.to_string(),
            avoid: Vec::new(),
            meaning: meaning.to_string(),
            defined: String::new(),
        }
    }

    #[test]
    fn staged_sections_are_read_in_order_without_the_index() {
        let dir = tempfile::tempdir().unwrap();
        let sections = dir.path().join("sections");
        std::fs::create_dir_all(&sections).unwrap();
        std::fs::write(sections.join("s02.md"), "第二\n").unwrap();
        std::fs::write(sections.join("s01.md"), "第一\n").unwrap();
        std::fs::write(sections.join("index.md"), "目录\n").unwrap();
        let read = read_sections(dir.path().to_str().unwrap()).unwrap();
        assert_eq!(
            read.iter().map(|s| s.file.as_str()).collect::<Vec<_>>(),
            ["sections/s01.md", "sections/s02.md"]
        );
        assert_eq!(read[0].lines, ["第一"]);
        assert_eq!(read[0].sha256, canon::sha256_hex("第一\n".as_bytes()));
        let error = read_sections(&ps::join(dir.path().to_str().unwrap(), "missing"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("run prepare first"), "{error}");
    }

    #[test]
    fn a_term_binds_to_its_definition_sentences_and_occurrence_sections() {
        let sections = vec![
            section("s01.md", "剩余控制权是指对资产用途的决定权。\n另一个句子。\n剩余控制权很重要。\n"),
            section("s02.md", "这里再提剩余控制权和剩余控制权。\n"),
        ];
        let evidence = row_evidence(
            &row(1, "剩余控制权", "对资产用途的决定权"),
            &sections,
            None,
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(evidence.snippets.len(), 1);
        assert!(evidence.snippets[0].defines);
        assert_eq!(evidence.snippets[0].file, "sections/s01.md");
        assert_eq!(evidence.snippets[0].line, 1);
        assert_eq!(evidence.snippets[0].text, "剩余控制权是指对资产用途的决定权。");
        assert_eq!(
            evidence.occurrences,
            vec![
                Occurrence { file: "sections/s01.md".into(), count: 2, first_line: 1 },
                Occurrence { file: "sections/s02.md".into(), count: 2, first_line: 1 },
            ]
        );
        // A term with no definition sentence still names the first lines a writer should read.
        let plain = row_evidence(&row(2, "敲竹杠", "机会主义"), &sections, None, &[], &[]).unwrap();
        assert!(plain.snippets.is_empty());
        let occurrences = vec![section("s01.md", "前一句。\n敲竹杠出现了。\n再一句。\n")];
        let fallback = row_evidence(&row(2, "敲竹杠", "机会主义"), &occurrences, None, &[], &[]).unwrap();
        assert_eq!(fallback.snippets.len(), 1);
        assert!(!fallback.snippets[0].defines);
        assert_eq!(fallback.snippets[0].line, 2);
    }

    #[test]
    fn a_formula_outside_a_definition_is_only_a_candidate() {
        let sections = vec![section(
            "s01.md",
            "剩余控制权 $x$ 出现在这里，但不是定义。\n剩余控制权是指决定权。\n",
        )];
        let evidence = row_evidence(
            &row(1, "剩余控制权", "决定权"),
            &sections,
            None,
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(evidence.classification.formula_candidates, 1);
        assert_eq!(evidence.classification.kind, "concept");
        assert!(evidence.classification.evidence.is_empty());
    }

    #[test]
    fn notation_tables_are_read_by_their_symbol_header() {
        let body = "本卡是《书》的符号表。\n\n符号表\n\n| 符号 | 含义 | 定义于 |\n| --- | --- | --- |\n| $\\sigma$ | 替代弹性 | §3 |\n| $F$ | 净生产函数 $G-C$ | 本卡 |\n";
        let rows = notation_rows(body);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].symbol, "$\\sigma$");
        assert_eq!(rows[0].meaning, "替代弹性");
        assert_eq!(rows[0].defined, "§3");
        assert_eq!(rows[1].meaning, "净生产函数 $G-C$");
        // A term table is not a notation table.
        assert!(notation_rows(TABLE).is_empty());
        // A headerless table of formulas is ignored too.
        assert!(notation_rows("| $x$ | something |\n| --- | --- |\n| $y$ | other |\n").is_empty());
    }

    #[test]
    fn math_classification_records_the_evidence_and_never_promotes_itself() {
        let math_sections = vec![section("s01.md", "替代弹性定义为 $\\sigma$。\n")];
        let evidence = row_evidence(
            &row(1, "替代弹性", "两种要素的替代难易"),
            &math_sections,
            None,
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(evidence.classification.kind, "math");
        assert_eq!(evidence.classification.evidence[0]["kind"], "definition_formula");
        assert_eq!(evidence.classification.promoted, false);

        let notation = vec![NotationRow {
            symbol: "$\\sigma$".into(),
            meaning: "替代弹性".into(),
            defined: "§3".into(),
        }];
        let via_notation = row_evidence(
            &row(1, "替代弹性", "替代难易"),
            &[],
            None,
            &notation,
            &[],
        )
        .unwrap();
        assert_eq!(via_notation.classification.kind, "math");
        assert_eq!(
            via_notation.classification.evidence[0]["kind"],
            "notation_table"
        );

        let settings = vec![SettingSymbols {
            address: "K/2|a|a".to_string(),
            title: "设定：企业家".to_string(),
            symbols: vec![NotationRow {
                symbol: "$\\sigma$".into(),
                meaning: "替代弹性".into(),
                defined: "§3".into(),
            }],
            terms: Vec::new(),
        }];
        let via_setting = row_evidence(
            &row(1, "替代弹性", "替代难易"),
            &[],
            None,
            &[],
            &settings,
        )
        .unwrap();
        assert_eq!(via_setting.classification.kind, "math");
        assert_eq!(
            via_setting.classification.evidence[0]["kind"],
            "setting_symbol"
        );
        assert_eq!(
            via_setting.classification.evidence[0]["address"],
            "K/2|a|a"
        );

        let via_meaning = row_evidence(
            &row(1, "替代弹性", "记作 $\\sigma$ 的替代难易"),
            &[],
            None,
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(via_meaning.classification.kind, "math");
        assert_eq!(
            via_meaning.classification.evidence[0]["kind"],
            "meaning_formula"
        );

        // A formal model's setting term table names the term: recorded naming context for a
        // later promotion, never formal notation on its own (R0316).
        let term_table = vec![SettingSymbols {
            address: "K/3|a|a".to_string(),
            title: "设定：竞争均衡".to_string(),
            symbols: Vec::new(),
            terms: vec!["替代弹性".to_string()],
        }];
        let via_term_table = row_evidence(
            &row(1, "替代弹性", "替代难易"),
            &[],
            None,
            &[],
            &term_table,
        )
        .unwrap();
        assert_eq!(via_term_table.classification.kind, "concept");
        assert!(via_term_table.classification.evidence.is_empty());
        assert_eq!(
            via_term_table.classification.setting_term_tables,
            vec!["K/3|a|a"]
        );

        // Both the English common facet and the Chinese label name the setting part.
        for facet in ["setting", "Setting", "设定", "设定：竞争均衡"] {
            assert!(is_setting_facet(facet), "{facet}");
        }
        for facet in ["", "assumptions", "假设", "推导：一阶条件"] {
            assert!(!is_setting_facet(facet), "{facet}");
        }
    }

    #[test]
    fn the_terms_artifact_supplies_the_first_definition_locator() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().to_str().unwrap();
        let out = ps::join(&ps::join(staging, "terms"), "out");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            ps::join(&out, "terms.json"),
            json!({"terms":[
                {"term":"剩余控制权","avoid":["残余控制权"],"meaning":"决定权","section":"§1"},
                {"term":"敲竹杠","avoid":[],"meaning":"机会主义","section":"§2"}
            ]})
            .to_string(),
        )
        .unwrap();
        let artifact = read_planning_artifact(staging).unwrap().unwrap();
        assert!(artifact.sha256.len() == 64);
        let evidence = row_evidence(&row(1, "剩余控制权", "决定权"), &[], Some(&artifact), &[], &[])
            .unwrap();
        let record = evidence.planning_artifact.unwrap();
        assert_eq!(record["section"], "§1");
        assert_eq!(record["path"], ps::join(&out, "terms.json"));
        // The list's own row is never rewritten by the artifact.
        assert_eq!(evidence.row.defined, "");
        assert!(read_planning_artifact(&ps::join(staging, "elsewhere"))
            .unwrap()
            .is_none());
        // A malformed artifact is refused, never ignored.
        std::fs::write(ps::join(&out, "terms.json"), "{\"terms\":[{\"meaning\":\"x\"}]}")
            .unwrap();
        assert!(read_planning_artifact(staging).is_err());
    }

    // --- grouping and the frozen plan ------------------------------------------------------------

    fn plan_row(number: usize, term: &str, meaning: &str) -> Row {
        Row {
            row: number,
            term: term.to_string(),
            avoid: Vec::new(),
            meaning: meaning.to_string(),
            defined: "§1".to_string(),
        }
    }

    fn evidence_rows(rows: &[Row], section: &Section) -> Vec<RowEvidence> {
        rows.iter()
            .map(|row| row_evidence(row, std::slice::from_ref(section), None, &[], &[]).unwrap())
            .collect()
    }

    fn built(staging: &str, rows: &[Row], section: &Section, notation: Vec<NotationRow>) -> BuiltPlan {
        let inputs = PlanInputs {
            staging,
            book: BookInfo {
                citation_key: "K".into(),
                title: "The Book".into(),
                variant: "econ".into(),
            },
            list: ListRead {
                address: "K/1|a".into(),
                card_type: "idea".into(),
                title: "术语表：《The Book》的 N 个术语".into(),
                body: TABLE.into(),
                legacy: true,
                rows: rows.to_vec(),
            },
            sections: vec![section.clone()],
            planning_artifact: None,
            notation: NotationRead {
                sources: Vec::new(),
                rows: notation,
            },
            settings: Vec::new(),
            limits: Limits {
                per_job: DEFAULT_PER_JOB,
                math_per_job: DEFAULT_MATH_PER_JOB,
                max_steps: DEFAULT_MAX_STEPS,
                max_prose: DEFAULT_MAX_PROSE,
            },
        };
        build_plan(&inputs, None, None).unwrap()
    }

    fn concept_rows(count: usize) -> Vec<Row> {
        (1..=count)
            .map(|number| plan_row(number, &format!("概念{number}"), "一个概念"))
            .collect()
    }

    #[test]
    fn concept_terms_group_four_per_job_and_math_terms_one_per_job() {
        let section = section(
            "s01.md",
            "概念1。\n概念2。\n概念3。\n概念4。\n概念5。\n概念6。\n数学1定义为 $x$。\n数学2定义为 $y$。\n概念9。\n",
        );
        let mut rows = concept_rows(6);
        rows.push(plan_row(7, "数学1", "形式定义"));
        rows.push(plan_row(8, "数学2", "形式定义"));
        rows.push(plan_row(9, "概念9", "一个概念"));
        let evidence = evidence_rows(&rows, &section);
        assert_eq!(
            evidence.iter().filter(|row| row.classification.kind == "math").count(),
            2
        );
        let jobs = group_jobs(&evidence, 4, 1);
        let ids: Vec<(&str, Vec<usize>)> = jobs
            .iter()
            .map(|job| (job.id.as_str(), job.rows.clone()))
            .collect();
        assert_eq!(
            ids,
            vec![
                ("t001", vec![1, 2, 3, 4]),
                ("t005", vec![5, 6]),
                ("t007", vec![7]),
                ("t008", vec![8]),
                ("t009", vec![9]),
            ]
        );
        assert_eq!(jobs[0].kind, "concept");
        assert_eq!(jobs[2].kind, "math");
        // A tighter concept group keeps jobs in table order with the same row-named ids.
        let tight = group_jobs(&evidence, 2, 2);
        assert_eq!(
            tight
                .iter()
                .map(|job| (job.id.as_str(), job.rows.clone()))
                .collect::<Vec<_>>(),
            vec![
                ("t001", vec![1, 2]),
                ("t003", vec![3, 4]),
                ("t005", vec![5, 6]),
                ("t007", vec![7, 8]),
                ("t009", vec![9]),
            ]
        );
    }

    #[test]
    fn the_plan_freezes_the_list_source_notation_limits_and_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().to_str().unwrap();
        let section = section(
            "s01.md",
            "剩余控制权是指决定权。\n敲竹杠是机会主义。\n",
        );
        let rows = vec![
            plan_row(1, "剩余控制权", "决定权"),
            plan_row(2, "敲竹杠", "机会主义"),
        ];
        let notation = vec![NotationRow {
            symbol: "$x$".into(),
            meaning: "剩余控制权".into(),
            defined: "§1".into(),
        }];
        let plan = built(staging, &rows, &section, notation).plan;
        assert_eq!(plan["schema_version"], 1);
        assert_eq!(plan["kind"], "term_cards_plan");
        assert_eq!(plan["revision"], 1);
        assert_eq!(plan["plan_hash"].as_str().unwrap().len(), 64);
        assert_eq!(plan["list"]["legacy"], true);
        assert_eq!(
            plan["list"]["title_sha256"],
            json!(canon::sha256_hex(plan["list"]["title"].as_str().unwrap().as_bytes()))
        );
        assert_eq!(
            plan["list"]["body_sha256"],
            json!(canon::sha256_hex(TABLE.as_bytes()))
        );
        assert_eq!(plan["source"]["sections"][0]["file"], "sections/s01.md");
        assert_eq!(
            plan["source"]["sections"][0]["sha256"],
            json!(canon::sha256_hex(section_text(&section).as_bytes()))
        );
        assert_eq!(plan["source"]["fingerprint"].as_str().unwrap().len(), 64);
        assert_eq!(plan["limits"]["per_job"], 4);
        assert_eq!(plan["limits"]["math_per_job"], 1);
        assert_eq!(plan["limits"]["max_steps_per_card"], 12);
        assert_eq!(plan["limits"]["max_prose"], 1600);
        assert_eq!(plan["versions"]["compiler"], env!("CARGO_PKG_VERSION"));
        assert_eq!(plan["versions"]["checker"], zt_check::VERSION);
        assert_eq!(plan["versions"]["rule"], zt_check::contract::RULE_VERSION);
        assert_eq!(plan["versions"]["instruction"]["file"], INSTRUCTION);
        assert_eq!(
            plan["versions"]["instruction"]["sha256"],
            json!(canon::sha256_hex(
                std::fs::read(ps::join(&common::scripts_dir(), INSTRUCTION))
                    .expect("instruction file")
                    .as_slice()
            ))
        );
        // The evidence is frozen: definition line, occurrence, neighbouring row, classification.
        let first = &plan["rows"][0];
        assert_eq!(first["term"], "剩余控制权");
        assert_eq!(first["snippets"][0]["line"], 1);
        assert_eq!(first["snippets"][0]["defines"], true);
        assert_eq!(first["occurrences"][0]["file"], "sections/s01.md");
        assert_eq!(first["neighbors"][0]["term"], "敲竹杠");
        assert_eq!(first["classification"]["kind"], "math");
        assert_eq!(first["classification"]["promoted"], false);
        assert_eq!(first["job"], "t001");
        // A math row carries the notation rows that name it.
        assert_eq!(first["notation"][0]["symbol"], "$x$");
        assert_eq!(plan["notation"]["rows"][0]["meaning"], "剩余控制权");
        // The math row is its own job; the concept row follows in table order.
        assert_eq!(plan["jobs"][0]["rows"], json!([1]));
        assert_eq!(plan["jobs"][0]["kind"], "math");
        assert_eq!(plan["jobs"][1]["rows"], json!([2]));
        assert_eq!(plan["jobs"][1]["kind"], "concept");
        assert_eq!(plan["jobs"][0]["file"], ps::join(&ps::join(staging, "term-cards"), "jobs\\t001.md"));
        assert_eq!(plan["jobs"][0]["output"], ps::join(&ps::join(staging, "term-cards"), "out\\t001.json"));
        // The frozen content hashes to the plan hash and the derived keys are not part of it.
        let frozen = frozen_of(&plan).unwrap();
        assert_eq!(canon::hash_value(&frozen), plan["plan_hash"]);
    }

    fn section_text(section: &Section) -> String {
        section.lines.join("\n") + "\n"
    }

    #[test]
    fn the_same_plan_keeps_its_revision_and_a_changed_section_starts_a_new_one() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().to_str().unwrap();
        let rows = vec![plan_row(1, "剩余控制权", "决定权")];
        let inputs = PlanInputs {
            staging,
            book: BookInfo {
                citation_key: "K".into(),
                title: "The Book".into(),
                variant: "econ".into(),
            },
            list: ListRead {
                address: "K/1|a".into(),
                card_type: "idea".into(),
                title: "术语表".into(),
                body: TABLE.into(),
                legacy: true,
                rows: rows.clone(),
            },
            sections: vec![section("s01.md", "剩余控制权是指决定权。\n")],
            planning_artifact: None,
            notation: NotationRead::default(),
            settings: Vec::new(),
            limits: Limits {
                per_job: DEFAULT_PER_JOB,
                math_per_job: DEFAULT_MATH_PER_JOB,
                max_steps: DEFAULT_MAX_STEPS,
                max_prose: DEFAULT_MAX_PROSE,
            },
        };
        let first = build_plan(&inputs, None, None).unwrap();
        let same = build_plan(&inputs, Some(&first.plan), Some(&first.state)).unwrap();
        assert_eq!(same.plan["revision"], 1);
        assert_eq!(same.plan, first.plan);
        assert_eq!(same.state["plan_revision"], 1);
        assert_eq!(same.state["revisions"].as_array().unwrap().len(), 1);
        let changed = PlanInputs {
            sections: vec![section("s01.md", "剩余控制权是指另一种权力。\n")],
            ..inputs
        };
        let next = build_plan(&changed, Some(&first.plan), Some(&first.state)).unwrap();
        assert_eq!(next.plan["revision"], 2);
        assert_ne!(next.plan["plan_hash"], first.plan["plan_hash"]);
        assert_eq!(next.state["plan_revision"], 2);
        assert_eq!(next.state["revisions"].as_array().unwrap().len(), 2);
        // The new revision starts the jobs over rather than inheriting progress.
        assert_eq!(next.state["jobs"]["t001"]["state"], "planned");
    }

    #[test]
    fn a_job_file_names_its_own_output_terms_and_the_shared_instruction_first() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().to_str().unwrap();
        let rows = vec![
            plan_row(1, "剩余控制权", "决定权"),
            plan_row(2, "敲竹杠", "机会主义"),
        ];
        let section = section("s01.md", "剩余控制权是指决定权。\n敲竹杠是机会主义。\n");
        let mut plan = built(staging, &rows, &section, Vec::new());
        let job_text = plan.job_files.remove("t001").unwrap();
        assert!(job_text.contains("术语卡任务 t001"));
        assert!(job_text.contains(&ps::join(&ps::join(staging, "term-cards"), INSTRUCTION)));
        assert!(job_text.contains(&ps::join(&ps::join(staging, "term-cards"), "out\\t001.json")));
        assert!(job_text.contains("剩余控制权"));
        assert!(job_text.contains("`sections/s01.md:1` 剩余控制权是指决定权。"));
        assert!(job_text.contains("本书其他术语"));
        assert!(job_text.contains("若其中某个术语实际是数学概念"));
        assert!(job_text.contains("只写这些术语"));
        assert!(plan.instruction.contains("[[K]]"));
        assert!(plan.instruction.contains(staging));
        assert!(plan.instruction.contains("12"));
        assert!(plan.instruction.contains("1600"));
        assert!(!plan.instruction.contains("{{"));
    }

    #[test]
    fn the_notation_table_is_found_by_title_but_not_as_a_term_definition() {
        let cards = vec![
            card("K/1|a", "idea", None, "符号表：《书》的 3 个符号"),
            card("K/2|a", "term", Some("符号表"), "符号表"),
            card("K/1|b", "idea", None, "术语表：《书》的 2 个术语"),
        ];
        let found = notation_candidates(&cards);
        assert_eq!(found.len(), 1);
        assert_eq!(pstr(found[0], "address"), "K/1|a");
    }

    #[test]
    fn a_row_without_any_source_evidence_stops_planning() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().to_str().unwrap();
        let rows = vec![
            plan_row(1, "剩余控制权", "决定权"),
            Row {
                row: 2,
                term: "没有出处的词".into(),
                avoid: Vec::new(),
                meaning: String::new(),
                defined: String::new(),
            },
        ];
        let inputs = PlanInputs {
            staging,
            book: BookInfo {
                citation_key: "K".into(),
                title: "The Book".into(),
                variant: "econ".into(),
            },
            list: ListRead {
                address: "K/1|a".into(),
                card_type: "idea".into(),
                title: "术语表".into(),
                body: TABLE.into(),
                legacy: true,
                rows,
            },
            sections: vec![section("s01.md", "剩余控制权是指决定权。\n")],
            planning_artifact: None,
            notation: NotationRead::default(),
            settings: Vec::new(),
            limits: Limits {
                per_job: DEFAULT_PER_JOB,
                math_per_job: DEFAULT_MATH_PER_JOB,
                max_steps: DEFAULT_MAX_STEPS,
                max_prose: DEFAULT_MAX_PROSE,
            },
        };
        let error = build_plan(&inputs, None, None).unwrap_err().to_string();
        assert!(error.contains("no meaning and no staged source passage"), "{error}");
        assert!(error.contains("没有出处的词"), "{error}");
    }

    #[test]
    fn an_incomplete_plan_reports_the_rows_it_cannot_source() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().to_str().unwrap();
        let rows = vec![plan_row(1, "剩余控制权", "决定权")];
        let built = built(
            staging,
            &rows,
            &section("s01.md", "另一段文字。\n"),
            Vec::new(),
        );
        assert_eq!(built.incomplete.len(), 1);
        assert_eq!(built.incomplete[0]["row"], 1);
        assert_eq!(built.incomplete[0]["reason"], "no_definition_sentence");
    }

    #[test]
    fn job_and_book_inputs_bind_the_plan_and_the_source_fingerprints() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().to_str().unwrap();
        let rows = vec![plan_row(1, "剩余控制权", "决定权")];
        let plan = built(staging, &rows, &section("s01.md", "剩余控制权是指决定权。\n"), Vec::new()).plan;
        let job = &plan["jobs"][0];
        let job_inputs = job_unit_inputs(&plan, job);
        assert_eq!(job_inputs["kind"], "term_card_job");
        assert_eq!(job_inputs["job"]["id"], "t001");
        assert_eq!(job_inputs["job"]["terms"], json!(["剩余控制权"]));
        assert_eq!(job_inputs["plan_hash"], plan["plan_hash"]);
        assert_eq!(
            job_inputs["source_fingerprint"],
            plan["source"]["fingerprint"]
        );
        let book_inputs = book_unit_inputs(&plan);
        assert_eq!(book_inputs["kind"], "term_cards_book");
        assert_eq!(book_inputs["jobs"][0]["unit"], "termcards/t001@1");
        assert_eq!(book_inputs["rows"], 1);
    }
}
