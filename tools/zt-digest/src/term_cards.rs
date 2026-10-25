//! `zt-digest term-cards`: the Term Card compiler (design 12.1–12.6, S18 / #159, S19 / #162 and
//! S27 / #166).
//!
//!   zt-digest term-cards -Step plan|jobs|precheck|check|review-plan|fix-plan|apply|report
//!                        -Staging <folder> [-PerJob 4] [-MathPerJob 1]
//!                        [-MaxStepsPerCard 12] [-MaxProse 1600]
//!                        [-Jobs <ids>] [-Token <claim>] [-Apply]
//!
//! `precheck` (S27, #166) is the writer's own SELF-CHECK: it runs the same `check_job` the
//! deterministic `check` runs and prints one line per diagnostic, then `faults: N` (blocking lines
//! only). It writes nothing and opens no archive, so a writer can run it as often as it needs.
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
//!   out/         writer JSON, one stable output per job (written by the writer, rewritten by its
//!                one permitted fix)
//!   check/       one diagnostics artifact per job, and `check/readiness.json`, the structured
//!                readiness result with its per-term hashes
//!   review/      `requests/` with every bound review request, and `answers/` with the reviewers'
//!                hash-named verdicts
//!   fix/         the one finite fix prompt per writer job
//!   state.json   derived local progress, plan revisions and the ledger unit ids
//!   report.json, report.md   publication coverage (S21)
//!
//! The plan step is read-only over the archive: it calls `get_tree`/`get_cards` and writes no
//! Card. When the staging folder is a prepared run (`ledger.sqlite3`), the plan registers the
//! existing ledger's `termcards/tNNN@<plan revision>` job units and the `termcards/book@<plan
//! revision>` unit; the existing run ledger remains the scheduler and `state.json` is never a
//! second queue. `jobs` reads one claimed job unit and prints the DeepSeek job for that writer.
//!
//! `check`, `review-plan` and `fix-plan` are the S19 (#162) steps: the exact writer JSON is checked
//! against the draft schema, rendered by zt-check's deterministic renderer and checked by the same
//! T rules the service applies; every mathematical draft is reviewed in a fresh conversation bound
//! to its exact term/source/draft/render hashes; and one finite fix prompt goes back to the writer's
//! own conversation through the existing `continue`/`fallback` job fields.
//!
//! Every printed writer, review and fix job carries its managed contract (S20 / #164, design 12.4):
//! the frozen official DeepSeek model at high effort, the job's own `skills`, and whether the
//! conversation ends once its answer is collected. The Haiku manager role stays the workflow's; the
//! job never picks a model and no skill text is copied.
//!
//! `apply` (S21 / #165, design 12.5–12.6) publishes the longest ready ordered row prefix as Side
//! children of the Term list with their continuation parts, verifies every committed Card by
//! reading it back and writes the publication coverage; `report` is the same read without a write.
//! They live in `publish.rs` beside this file.

mod publish;

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

/// The declared steps, in command order (design 12.1). `precheck` is the writer's own SELF-CHECK
/// (S27, #166, design 14.5): the same `check_job` the deterministic `check` runs, printed as fault
/// lines, with no compiler artifact and no Card written.
pub const STEPS: [&str; 8] = [
    "plan",
    "jobs",
    "precheck",
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

/// The managed content role (design 12.4, R0326; S20 / #164): DeepSeek V4.1 Flash from the official
/// source at high effort writes, reviews and fixes the drafts. The Haiku manager stays the
/// blueprint's low-effort role, so a job never picks a model of its own.
pub const JOB_MODEL: &str = "deepseek-official/deepseek-flash";
pub const JOB_EFFORT: &str = "high";
/// The skills a content job receives through its own field (R0333), never copied skill text: the
/// writer composes ZT Cards in Chinese, the reviewer reads and answers in Chinese.
pub const WRITER_SKILLS: [&str; 2] = ["zettelkasten", "asd-ste100-zh"];
pub const REVIEWER_SKILLS: [&str; 1] = ["asd-ste100-zh"];

/// Add the managed contract to one printed job (S20, #164). `delete_after_collect` is true for a
/// conversation that ends once its answer is durably collected (a review); a writer job keeps it
/// false because its wave retains the writer through the review/fix opportunity and deletes it
/// when that opportunity is complete.
fn managed_job(job: Value, skills: &[&str], delete_after_collect: bool) -> Value {
    let mut job = job.as_object().cloned().unwrap_or_default();
    job.insert("model".into(), json!(JOB_MODEL));
    job.insert("effort".into(), json!(JOB_EFFORT));
    job.insert("skills".into(), json!(skills));
    job.insert("delete_after_collect".into(), json!(delete_after_collect));
    Value::Object(job)
}

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
    if token.is_some() && step != "jobs" {
        bail!("term-cards {step} does not claim a job unit; remove -Token");
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
        "precheck" => precheck_step(&p),
        "check" => check_step(&p),
        "review-plan" => review_plan_step(&p),
        "fix-plan" => fix_plan_step(&p),
        "apply" => publish::apply(&p),
        "report" => publish::report(&p),
        // `params` binds -Step to one of STEPS, so no other step reaches this dispatch.
        _ => unreachable!("-Step is bound to one of STEPS"),
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
    let plan_hash = plan_hash_of(&frozen);
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

/// The plan content hash: the frozen plan without the list's *kind* (`type`, `legacy`). The kind is
/// no list content: the first publication retypes a legacy table metadata-only and keeps every
/// authored byte, so a resume that re-reads the retyped list must keep the same revision, its
/// check artifacts and its receipts (S21, #165; R0336, R0342). The authored title, body, rows,
/// source and limits all stay in the hash.
fn plan_hash_of(frozen: &Value) -> String {
    let mut value = frozen.clone();
    if let Some(list) = value.get_mut("list").and_then(Value::as_object_mut) {
        list.remove("type");
        list.remove("legacy");
    }
    canon::hash_value(&value)
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
    let fixes = if same {
        existing
            .and_then(|old| old.get("fixes"))
            .cloned()
            .unwrap_or_else(|| json!([]))
    } else {
        json!([])
    };
    // The publication record is not plan progress: a Card an earlier revision published stays
    // published, so the record survives a new revision and a resume can name its receipts
    // (S21, #165; R0341, R0349).
    let published = existing
        .and_then(|old| old.get("published"))
        .cloned()
        .unwrap_or_else(|| json!({"rows": {}}));
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
        "fixes": fixes,
        "receipts": if same { existing.and_then(|old| old.get("receipts")).cloned().unwrap_or(json!({})) } else { json!({}) },
        "published": published,
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
    // The writer's exact SELF-CHECK (S27, #166, design 14.5): the same check the deterministic
    // `check` step runs, printed as fault lines, writing nothing. Its command is not invented here:
    // it is this tool's own `precheck` step on this job.
    sb.blank();
    sb.line(format!(
        "SELF-CHECK: zt-digest term-cards -Step precheck -Staging \"{}\" -Jobs {id}",
        inputs.staging
    ));
    sb.line("写完运行这一行命令自查：改正它报告的每个问题，再运行，直到最后一行是 `faults: 0`（最多三次）。命令没有打印 `faults:` 行时，视为没有自查。");
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

/// The staging folder's `manifest.json` with a usable Citation key: the book every term-card step
/// is about. A missing or empty key stops before any archive read.
fn book_manifest(p: &Params) -> Result<Value> {
    let manifest = common::manifest(&p.staging)?;
    let key = pstr(&manifest, "citation_key");
    if key.trim().is_empty() {
        bail!("manifest.json names no citation_key; term-card planning needs its book");
    }
    Ok(manifest)
}

/// Everything one coherent read of the staging folder and the archive says, exactly as `plan`
/// reads and freezes it. The publication steps (S21, #165) re-read the same inputs and compare the
/// rebuilt plan hash before they write or report anything, so no step ever acts on a stale plan.
struct Fresh {
    tree: Value,
    list: ListRead,
    built: BuiltPlan,
    existing_plan: Option<Value>,
}

/// The one read sequence of `plan`, shared with the publication steps. It writes nothing.
fn read_fresh(p: &Params, manifest: &Value, zt: &mut Zt) -> Result<Fresh> {
    let staging = p.staging.as_str();
    let key = pstr(manifest, "citation_key");
    let book_title = {
        let title = pstr(manifest, "book_title");
        if title.trim().is_empty() { key.clone() } else { title }
    };
    let variant = pstr(&manifest["run_contract"], "variant");
    let tree = zt.call("get_tree", json!({"tree": key}))?;
    let archive_revision = tree["archive_revision"]
        .as_u64()
        .context("the tree read carries no archive revision")?;
    let cards = tree["cards"].as_array().cloned().unwrap_or_default();
    let list = {
        let mut read = |at: &str| zt.call("get_card", json!({"at": at}));
        select_list(&cards, &mut read)?
    };
    let notation = read_notation(zt, staging, &cards, &key)?;
    let settings = read_settings(zt, &cards, &list)?;
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
            "the archive changed while reading it; run the step again for the new archive revision"
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
    let existing_plan = read_existing(&ps::join(&work, "plan.json"), "plan")?;
    let existing_state = read_existing(&ps::join(&work, "state.json"), "state")?;
    let built = build_plan(&inputs, existing_plan.as_ref(), existing_state.as_ref())?;
    Ok(Fresh {
        tree,
        list: inputs.list.clone(),
        built,
        existing_plan,
    })
}

fn plan(p: &Params) -> Result<i32> {
    let staging = p.staging.as_str();
    let manifest = book_manifest(p)?;
    let key = pstr(&manifest, "citation_key");
    let mut zt = crate::zt::start()?;
    if let Some(refusal) = crate::lockcheck::preflight(&mut zt, "term-cards", "plan", &key)? {
        return crate::lockcheck::stop("term-cards", &refusal);
    }
    let fresh = read_fresh(p, &manifest, &mut zt)?;
    let work = p.work();
    let plan_path = ps::join(&work, "plan.json");
    let state_path = ps::join(&work, "state.json");
    let existing_plan = fresh.existing_plan;
    let mut built = fresh.built;
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
    let compiled = Compiled::load(p)?;
    let work = compiled.work.clone();
    let plan = compiled.plan.clone();
    let plan_hash = compiled.plan_hash.clone();
    let all: Vec<Value> = compiled.jobs()?.clone();
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
            Ok(managed_job(json!({
                "id": job["id"],
                "kind": job["kind"],
                "rows": job["rows"],
                "terms": job["terms"],
                "files": [instruction, file],
                "prompt": PROMPT,
                "output": job["output"],
                "permissions": {"archive_read": true, "archive_write": false},
            }), &WRITER_SKILLS, false))
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

// --- the check, review and fix steps (S19, #162) -------------------------------------------------

/// The reviewer's instruction file beside the writer's (design 1.1: two prompt files).
const REVIEW_INSTRUCTION: &str = "dsh-term-review-instructions.zh.md";

/// A review job runs in a fresh conversation and writes one bound answer file.
const REVIEW_PROMPT: &str =
    "按审查说明和任务文件审查这一张数学术语卡草稿，只写任务文件指定的答案文件，最后只回复一行 JSON。";

/// A fix job returns to the writer's own conversation through `continue`/`fallback`.
const FIX_PROMPT: &str =
    "按修复任务只改任务文件列出的问题，重写你负责的那一份 JSON 草稿，只写任务文件指定的输出文件，最后只回复一行 JSON。";

/// The head a rendered continuation part links to. A draft has no address yet, so the draft check
/// binds the same batch reference that publication substitutes (design 12.5).
const HEAD_REFERENCE: &str = "@@term_head";

/// The plan a later step reads back, with its frozen hash verified exactly as `jobs` verifies it.
struct Compiled {
    plan: Value,
    work: String,
    plan_hash: String,
    revision: i64,
}

impl Compiled {
    fn load(p: &Params) -> Result<Self> {
        let work = p.work();
        let path = ps::join(&work, "plan.json");
        if !std::path::Path::new(&path).is_file() {
            bail!("no term-cards plan at `{path}`; run -Step plan first");
        }
        let plan = util::read_json(&path)?;
        if plan["schema_version"] != 1 || plan["kind"] != "term_cards_plan" {
            bail!("`{path}` is not a term-cards plan");
        }
        let plan_hash = pstr(&plan, "plan_hash");
        if plan_hash_of(&frozen_of(&plan)?) != plan_hash {
            bail!("`{path}` changed since it was frozen; rerun -Step plan");
        }
        let revision = plan["revision"]
            .as_i64()
            .context("the plan carries no revision; rerun -Step plan")?;
        Ok(Self {
            plan,
            work,
            plan_hash,
            revision,
        })
    }

    fn jobs(&self) -> Result<&Vec<Value>> {
        self.plan["jobs"].as_array().context("incomplete plan jobs")
    }

    /// The jobs `-Jobs` names, in the order given, or every job.
    fn selected(&self, p: &Params) -> Result<Vec<&Value>> {
        let all = self.jobs()?;
        if p.jobs.is_empty() {
            return Ok(all.iter().collect());
        }
        let mut out = Vec::new();
        for id in &p.jobs {
            let job = all
                .iter()
                .find(|job| pstr(job, "id") == *id)
                .with_context(|| format!("the plan has no job `{id}`"))?;
            out.push(job);
        }
        Ok(out)
    }

    fn rows(&self) -> &[Value] {
        self.plan["rows"].as_array().map(Vec::as_slice).unwrap_or(&[])
    }

    fn row(&self, number: u64) -> Option<&Value> {
        self.rows()
            .iter()
            .find(|row| row["row"].as_u64() == Some(number))
    }

    /// Every exact table-row spelling, as the draft schema validates distinctions against.
    fn row_terms(&self) -> Vec<String> {
        self.rows()
            .iter()
            .map(|row| pstr(row, "term"))
            .filter(|term| !term.is_empty())
            .collect()
    }

    fn book(&self) -> String {
        pstr(&self.plan["book"], "citation_key")
    }

    fn source_fingerprint(&self) -> String {
        pstr(&self.plan["source"], "fingerprint")
    }

    /// The checker's limits, from the limits the plan froze (a raised per-book limit is what the
    /// renderer already used, so the T rules must read the same numbers).
    fn checker_limits(&self) -> Result<zt_check::core::Limits> {
        let number = |key: &str| -> Result<i64> {
            self.plan["limits"][key]
                .as_i64()
                .with_context(|| format!("the plan freezes no `{key}` limit; rerun -Step plan"))
        };
        let mut limits = zt_check::core::Limits::default();
        limits.max_term_steps = number("max_steps_per_card")?;
        limits.max_term_prose = number("max_prose")?;
        Ok(limits)
    }

    fn check_path(&self, id: &str) -> String {
        ps::join(&ps::join(&self.work, "check"), &format!("{id}.json"))
    }

    fn readiness_path(&self) -> String {
        ps::join(&ps::join(&self.work, "check"), "readiness.json")
    }

    fn instruction(&self) -> String {
        ps::join(&self.work, INSTRUCTION)
    }

    fn review_dir(&self) -> String {
        ps::join(&self.work, "review")
    }

    fn request_dir(&self) -> String {
        ps::join(&self.review_dir(), "requests")
    }

    fn answer_dir(&self) -> String {
        ps::join(&self.review_dir(), "answers")
    }

    fn fix_dir(&self) -> String {
        ps::join(&self.work, "fix")
    }

    fn state_path(&self) -> String {
        ps::join(&self.work, "state.json")
    }

    /// The compiler's version set, the same one `plan.json` freezes.
    fn versions(&self) -> Value {
        json!({
            "compiler": env!("CARGO_PKG_VERSION"),
            "checker": zt_check::VERSION,
            "rule": zt_check::contract::RULE_VERSION,
        })
    }
}

/// The derived local progress. A lost or foreign `state.json` is rebuilt from the plan: the file
/// is progress, never the authority (design 12.1).
fn local_state(compiled: &Compiled) -> Result<Value> {
    let path = compiled.state_path();
    if std::path::Path::new(&path).is_file()
        && let Ok(state) = util::read_json(&path)
        && state["plan_hash"] == json!(compiled.plan_hash)
        && state["plan_revision"] == json!(compiled.revision)
    {
        return Ok(state);
    }
    let mut jobs = Map::new();
    for job in compiled.jobs()? {
        jobs.insert(
            pstr(job, "id"),
            json!({
                "state": "planned",
                "file": job["file"],
                "output": job["output"],
                "started_at": Value::Null,
            }),
        );
    }
    Ok(json!({
        "schema_version": 1,
        "kind": "term_cards_state",
        "book": compiled.book(),
        "plan_revision": compiled.revision,
        "plan_hash": compiled.plan_hash,
        "revisions": [{
            "revision": compiled.revision,
            "plan_hash": compiled.plan_hash,
            "list_sha256": compiled.plan["list"]["body_sha256"],
            "source_fingerprint": compiled.plan["source"]["fingerprint"],
            "at": canon::utc_now_o(),
        }],
        "jobs": jobs,
        "units": Value::Null,
        "promotions": [],
        "receipts": {},
        "fixes": [],
        "published": {"rows": {}},
    }))
}

fn save_state(compiled: &Compiled, state: &Value) -> Result<()> {
    util::write_atomic(compiled.state_path(), &ps::json_pretty(state))
}

/// One check artifact as `check/<job>.json`: the writer's exact file, every term the job owns,
/// and what the draft schema, the renderer and the T rules said about it (S19-AC01).
fn check_job(compiled: &Compiled, job: &Value) -> Result<Value> {
    let id = pstr(job, "id");
    let output = pstr(job, "output");
    let numbers: Vec<u64> = job["rows"]
        .as_array()
        .context("incomplete plan job rows")?
        .iter()
        .filter_map(Value::as_u64)
        .collect();
    let terms_of_job: Vec<String> = numbers
        .iter()
        .filter_map(|number| compiled.row(*number))
        .map(|row| pstr(row, "term"))
        .collect();
    let limits = compiled.checker_limits()?;
    let contract = zt_check::contract::Contract::built_in(&limits);
    let rows = compiled.row_terms();

    let mut document = json!({
        "schema_version": 1,
        "kind": "term_cards_check",
        "book": compiled.book(),
        "plan_revision": compiled.revision,
        "plan_hash": compiled.plan_hash,
        "job": id,
        "job_kind": job["kind"],
        "output": output,
        "output_sha256": Value::Null,
        "checked_at": canon::utc_now_o(),
        "versions": compiled.versions(),
        "contract": {
            "checker": zt_check::VERSION,
            "rule_version": zt_check::contract::RULE_VERSION,
            "policy_hash": contract.policy_hash,
            "limits": limits.to_json(),
        },
        "ok": false,
        "terms": [],
        "unexpected": [],
        "duplicates": [],
        "missing": [],
    });

    // The writer's file is read as it is. A missing or unparsable file is a per-term fault, not a
    // step failure: the chain must reach its one fix instead of stopping here.
    let text = if std::path::Path::new(&output).is_file() {
        Some(util::read_text(&output)?)
    } else {
        None
    };
    let parsed = text
        .as_deref()
        .map(|text| ps::parse_json(text).map_err(|error| error.to_string()));
    if let Some(text) = &text {
        document["output_sha256"] = json!(canon::sha256_hex(text.as_bytes()));
    }
    let (items, whole_fault) = match &parsed {
        None => (Vec::new(), Some("the writer wrote no output file".to_string())),
        Some(Err(error)) => (Vec::new(), Some(format!("the output is not JSON: {error}"))),
        Some(Ok(value)) => match value.get("terms").and_then(Value::as_array) {
            None => (
                Vec::new(),
                Some("the output is no `{\"terms\":[…]}` document".to_string()),
            ),
            Some(items) => (items.clone(), None),
        },
    };

    let mut checks: Vec<Value> = Vec::new();
    let mut duplicates: Vec<String> = Vec::new();
    let mut unexpected: Vec<String> = Vec::new();
    for item in &items {
        let spell = item.get("term").and_then(Value::as_str).map(str::to_string);
        match spell {
            None => unexpected.push("<no term>".to_string()),
            Some(term) if !terms_of_job.iter().any(|known| *known == term) => unexpected.push(term),
            Some(_) => {}
        }
    }
    for term in &terms_of_job {
        // The first item that names the term owns it; a second one is a duplicated draft.
        let found: Vec<&Value> = items
            .iter()
            .filter(|item| item.get("term").and_then(Value::as_str) == Some(term.as_str()))
            .collect();
        if found.len() > 1 {
            duplicates.push(term.clone());
        }
        let row = numbers
            .iter()
            .filter_map(|number| compiled.row(*number))
            .find(|row| pstr(row, "term") == *term)
            .with_context(|| format!("the plan holds no row for `{term}`"))?;
        let mut check = term_check_document(compiled, row, &id)?;
        if let Some(reason) = whole_fault.clone().filter(|_| found.is_empty()) {
            check["status"] = json!("missing");
            check["reason"] = json!(reason);
            check["schema_faults"] = json!([]);
            check["render_faults"] = json!([]);
            check["diagnostics"] = json!([]);
            check["blocking"] = json!(0);
            check["advisory"] = json!(0);
            check["cards"] = json!([]);
            check["draft"] = Value::Null;
            check["draft_sha256"] = Value::Null;
            check["render_sha256"] = Value::Null;
        } else if let Some(item) = found.first() {
            check = check_draft_item(compiled, &limits, &contract, &rows, row, &id, item)?;
            if duplicates.iter().any(|duplicate| duplicate == term) {
                let mut faults = check["schema_faults"].as_array().cloned().unwrap_or_default();
                faults.push(json!({
                    "path": "terms",
                    "message": format!("`{term}` is written twice; one job writes one draft per term"),
                }));
                check["schema_faults"] = json!(faults);
                check["status"] = json!("faulty");
            }
        } else {
            check["status"] = json!("missing");
            check["reason"] = json!("the writer's output holds no draft for this row");
        }
        checks.push(check);
    }

    let ok = checks.iter().all(|check| check["status"] == json!("ok"))
        && unexpected.is_empty()
        && duplicates.is_empty();
    document["terms"] = json!(checks);
    document["unexpected"] = json!(unexpected);
    document["duplicates"] = json!(duplicates);
    document["missing"] = json!(checks
        .iter()
        .filter(|check| check["status"] == json!("missing"))
        .map(|check| check["term"].clone())
        .collect::<Vec<_>>());
    document["ok"] = json!(ok);
    Ok(document)
}

/// One term's entry of a check artifact before its draft is read: the frozen identity and the
/// hashes every later review binds.
fn term_check_document(compiled: &Compiled, row: &Value, job: &str) -> Result<Value> {
    let number = row["row"].as_u64().unwrap_or(0);
    Ok(json!({
        "row": number,
        "term": pstr(row, "term"),
        "job": job,
        "planned_category": row["classification"]["kind"],
        "promoted": false,
        "category": Value::Null,
        "status": "missing",
        "reason": Value::Null,
        "draft_sha256": Value::Null,
        "render_sha256": Value::Null,
        "source_sha256": canon::hash_value(&json!({
            "source": compiled.source_fingerprint(),
            "row": row,
        })),
        "cards": [],
        "draft": Value::Null,
        "schema_faults": [],
        "render_faults": [],
        "diagnostics": [],
        "blocking": 0,
        "advisory": 0,
        "supplements": [],
        "verification": Value::Null,
    }))
}

/// One written draft's schema, render and T-rule result (S19-AC01). Every fault the checker finds
/// is kept per term, so readiness can report the exact rejected requirement.
fn check_draft_item(
    compiled: &Compiled,
    limits: &zt_check::core::Limits,
    contract: &zt_check::contract::Contract,
    rows: &[String],
    row: &Value,
    job: &str,
    item: &Value,
) -> Result<Value> {
    let number = row["row"].as_u64().unwrap_or(0);
    let term = pstr(row, "term");
    let planned = pstr(&row["classification"], "kind");
    let mut check = term_check_document(compiled, row, job)?;
    check["draft"] = item.clone();
    check["draft_sha256"] = json!(canon::hash_value(item));
    let draft = match zt_check::term::parse_draft(item, rows) {
        Ok(draft) => draft,
        Err(faults) => {
            check["status"] = json!("faulty");
            check["schema_faults"] = json!(draft_faults_json(&faults));
            return Ok(check);
        }
    };
    check["category"] = json!(match draft.category {
        zt_check::term::Category::Math => "math",
        zt_check::term::Category::Concept => "concept",
    });
    // A plan-classified mathematical term cannot clear its required independent review by
    // declaring itself a concept; a concept row written as math is the promotion S18 allowed.
    if planned == "math" && draft.category == zt_check::term::Category::Concept {
        check["status"] = json!("faulty");
        check["schema_faults"] = json!([{
            "path": "category",
            "message": format!(
                "the plan classified `{term}` as math (the book gives formal notation); a concept draft cannot clear the required review"
            ),
        }]);
        return Ok(check);
    }
    let promoted = planned != "math" && draft.category == zt_check::term::Category::Math;
    check["promoted"] = json!(promoted);
    let term_row = zt_check::text::TermRow {
        term: term.clone(),
        avoid: row["avoid"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        meaning: pstr(row, "meaning"),
        defined: pstr(row, "defined"),
    };
    let rendered = match zt_check::term::render(&draft, &term_row, &compiled.book(), HEAD_REFERENCE, limits)
    {
        Ok(rendered) => rendered,
        Err(faults) => {
            check["status"] = json!("faulty");
            check["render_faults"] = json!(draft_faults_json(&faults));
            return Ok(check);
        }
    };
    let category = match draft.category {
        zt_check::term::Category::Math => "math",
        zt_check::term::Category::Concept => "concept",
    };
    let (entries, cards) = term_entries(job, number, &term, category, rows, &rendered);
    let diagnostics: Vec<zt_check::core::Diagnostic> = zt_check::core::Check::new(
        limits.clone(),
        false,
        "",
    )
    .with_policies(contract.policy.clone())
    .report(&entries);
    let blocking = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.blocking())
        .count();
    let grouped: Vec<Value> = cards
        .iter()
        .map(|card| {
            let address = pstr(card, "address");
            json!({
                "address": address,
                "role": card["role"],
                "title": card["title"],
                "steps": card["steps"],
                "prose": card["prose"],
                "sha256": card["sha256"],
                "diagnostics": diagnostics
                    .iter()
                    .filter(|diagnostic| diagnostic.address == address)
                    .map(zt_check::core::Diagnostic::to_json)
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let supplements = supplements_of(&grouped, &rendered.cards());
    check["cards"] = json!(grouped);
    check["supplements"] = json!(supplements);
    check["verification"] = json!({
        "numerical_checks": [],
        "note": "the draft records no executed numerical check as machine evidence; the independent review recomputes the example",
        "proof": "format checks and the independent review are evidence of review, not proof of mathematical truth",
    });
    check["render_sha256"] = json!(render_sha256(&rendered));
    check["diagnostics"] = json!(diagnostics
        .iter()
        .map(zt_check::core::Diagnostic::to_json)
        .collect::<Vec<_>>());
    check["blocking"] = json!(blocking);
    check["advisory"] = json!(diagnostics.len() - blocking);
    check["status"] = json!(if blocking == 0 { "ok" } else { "faulty" });
    Ok(check)
}

fn draft_faults_json(faults: &zt_check::term::DraftFaults) -> Vec<Value> {
    faults
        .0
        .iter()
        .map(|fault| json!({"path": fault.path, "message": fault.message}))
        .collect()
}

/// The two established source-fidelity markers of a supplied step (design 11.1). The check keeps
/// them verbatim in the rendered text and reports where they are, so readiness names what the book
/// does not state (S19-AC06).
const SUPPLEMENTS: [&str; 2] = ["（补：书中未写）", "（补：书中略去）"];

fn supplements_of(cards: &[Value], rendered: &[&zt_check::term::RenderedCard]) -> Vec<Value> {
    let mut out = Vec::new();
    for (card, rendered) in cards.iter().zip(rendered) {
        for marker in SUPPLEMENTS {
            for _ in 0..rendered.body.matches(marker).count() {
                out.push(json!({
                    "marker": marker,
                    "card": card["address"],
                }));
            }
        }
    }
    out
}

/// The exact rendered chain hash: every Card's role, title and body, in order (R0321).
fn render_sha256(rendered: &zt_check::term::Rendered) -> String {
    canon::hash_value(&json!(rendered
        .cards()
        .iter()
        .map(|card| json!({"role": card.role, "title": card.title, "body": card.body}))
        .collect::<Vec<_>>()))
}

/// The exact entries the checker reads and the metadata a reader can compare: one head and its
/// continuation parts, each with its deterministic draft address (`<job>/r<row>[/p<n>]`).
fn term_entries(
    job: &str,
    row: u64,
    term: &str,
    category: &str,
    rows: &[String],
    rendered: &zt_check::term::Rendered,
) -> (Vec<Value>, Vec<Value>) {
    let mut entries = Vec::new();
    let mut cards = Vec::new();
    for (index, card) in rendered.cards().iter().enumerate() {
        let address = if index == 0 {
            format!("{job}/r{row:03}")
        } else {
            format!("{job}/r{row:03}/p{index}")
        };
        let mut entry = json!({
            "address": address,
            "title": card.title,
            "body": card.body,
            "type": "term",
            "term": term,
            "term_role": card.role,
            "term_rows": rows,
        });
        if card.role == "part" {
            entry["term_head"] = json!(HEAD_REFERENCE);
            entry["term_head_category"] = json!(match category {
                "math" => zt_check::term::CATEGORY_MATH,
                _ => zt_check::term::CATEGORY_CONCEPT,
            });
        }
        cards.push(json!({
            "address": address,
            "role": card.role,
            "title": card.title,
            "steps": card.steps,
            "prose": card.prose,
            "sha256": canon::hash_value(&json!({"title": card.title, "body": card.body})),
        }));
        entries.push(entry);
    }
    (entries, cards)
}

/// `-Step precheck`: the writer's exact SELF-CHECK (S27, #166, design 14.5). It runs the same
/// `check_job` the deterministic `check` runs on the job's output file and prints the result the way
/// every writer's check prints it: one line per diagnostic, then `faults: N`, the number of blocking
/// lines. It writes nothing — no check artifact, no readiness, no state, no Card — so a writer can
/// run it as many times as it needs before it replies. A run that prints no `faults:` line did not
/// check, exactly as `zt-check`'s own contract says.
fn precheck_step(p: &Params) -> Result<i32> {
    let compiled = Compiled::load(p)?;
    let documents: Vec<Value> = compiled
        .selected(p)?
        .iter()
        .map(|job| check_job(&compiled, job))
        .collect::<Result<Vec<_>>>()?;
    let mut lines: Vec<String> = Vec::new();
    let mut blocking: usize = 0;
    for document in &documents {
        let job = pstr(document, "job");
        for term in document["terms"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            let row = term["row"].as_u64().unwrap_or(0);
            let address = format!("{job}/r{row:03}");
            if let Some(reason) = term["reason"].as_str().filter(|reason| !reason.is_empty()) {
                lines.push(format!("{address} missing {reason}"));
                blocking += 1;
            }
            for (kind, key) in [("schema", "schema_faults"), ("render", "render_faults")] {
                for fault in term[key].as_array().map(Vec::as_slice).unwrap_or(&[]) {
                    lines.push(format!(
                        "{address} {kind} {}: {}",
                        pstr(fault, "path"),
                        pstr(fault, "message")
                    ));
                    blocking += 1;
                }
            }
            for diagnostic in term["diagnostics"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
                let advisory = diagnostic["severity"].as_str() == Some("advise");
                if !advisory {
                    blocking += 1;
                }
                lines.push(format!(
                    "{} {} {}{}",
                    pstr(diagnostic, "address"),
                    pstr(diagnostic, "rule"),
                    if advisory { "(advisory) " } else { "" },
                    pstr(diagnostic, "message")
                ));
            }
        }
        for term in document["unexpected"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
            lines.push(format!(
                "{job} unexpected {}: this job does not own that term",
                ps::s(term)
            ));
            blocking += 1;
        }
    }
    for line in &lines {
        util::print_line(line)?;
    }
    util::print_line(&format!("faults: {blocking}"))?;
    Ok(0)
}

/// The check step: check the selected writers' outputs against the draft schema, the deterministic
/// renderer and the T rules, write one artifact per job, and publish the readiness result.
///
/// Content faults are the step's answer, not its failure: it exits 0 and names them, so the chain
/// reaches its one fix. An invalid invocation, a stale plan, or an unreadable artifact fails.
fn check_step(p: &Params) -> Result<i32> {
    let compiled = Compiled::load(p)?;
    let selected = compiled.selected(p)?;
    std::fs::create_dir_all(ps::join(&compiled.work, "check"))?;
    std::fs::create_dir_all(compiled.answer_dir())?;
    let mut state = local_state(&compiled)?;
    let mut printed = Vec::new();
    for job in &selected {
        let id = pstr(job, "id");
        let document = check_job(&compiled, job)?;
        let path = compiled.check_path(&id);
        util::write_atomic(&path, &ps::json_pretty(&document))?;
        let sha = canon::sha256_hex(&std::fs::read(&path)?);
        record_check(&mut state, &compiled, job, &document, &path, &sha);
        printed.push(json!({
            "id": id,
            "kind": document["job_kind"],
            "output": document["output"],
            "ok": document["ok"],
            "terms": document["terms"].as_array().map_or(0, Vec::len),
            "faults": document["terms"].as_array().map(|terms| terms.iter()
                .filter(|term| term["status"] == json!("faulty")).count()).unwrap_or(0),
            "missing": document["missing"],
            "check": path,
        }));
    }
    save_state(&compiled, &state)?;
    let checks = current_checks(&compiled)?;
    let readiness = readiness_of(&compiled, &checks, &state, &[]);
    let published = publish_readiness(&compiled, &readiness)?;
    let summary = json!({
        "schema_version": 1,
        "ok": true,
        "step": "check",
        "book": compiled.book(),
        "plan_revision": compiled.revision,
        "plan_hash": compiled.plan_hash,
        "work": compiled.work,
        "jobs": printed,
        "readiness": published,
        "counts": readiness["counts"],
        "complete": readiness["complete"],
        "terms": readiness["terms"].as_array().map(|terms| terms.iter().map(|term| json!({
            "row": term["row"], "term": term["term"], "job": term["job"],
            "status": term["status"], "category": term["category"],
        })).collect::<Vec<_>>()).unwrap_or_default(),
    });
    util::print_line(&canon::canonical_json(&summary))?;
    Ok(0)
}

/// Record one job's checked result in the derived state: its progress, and every promotion the
/// writer made so a grouped concept job cannot hide a term that now needs the mathematics review.
fn record_check(
    state: &mut Value,
    compiled: &Compiled,
    job: &Value,
    document: &Value,
    path: &str,
    sha: &str,
) {
    let id = pstr(job, "id");
    if !state["jobs"].is_object() {
        state["jobs"] = json!({});
    }
    let entry = state["jobs"]
        .as_object_mut()
        .map(|jobs| jobs.entry(id.clone()).or_insert(Value::Null));
    if let Some(entry) = entry {
        *entry = json!({
            "state": if document["output_sha256"].is_string() { "checked" } else { "missing_output" },
            "file": job["file"],
            "output": job["output"],
            "check": {"path": path, "sha256": sha, "ok": document["ok"]},
        });
    }
    let mut promotions = state["promotions"].as_array().cloned().unwrap_or_default();
    for term in document["terms"].as_array().cloned().unwrap_or_default() {
        if term["promoted"] != json!(true) {
            continue;
        }
        let row = term["row"].as_u64().unwrap_or(0);
        if promotions
            .iter()
            .any(|known| known["row"].as_u64() == Some(row))
        {
            continue;
        }
        promotions.push(json!({
            "row": row,
            "term": term["term"],
            "job": term["job"],
            "from": term["planned_category"],
            "to": term["category"],
            "draft_sha256": term["draft_sha256"],
            "at": canon::utc_now_o(),
        }));
    }
    state["promotions"] = json!(promotions);
    state["plan_hash"] = json!(compiled.plan_hash);
    state["plan_revision"] = json!(compiled.revision);
}

/// Every job's check artifact that still describes the current plan, output bytes and checker.
fn current_checks(compiled: &Compiled) -> Result<BTreeMap<String, Value>> {
    let mut checks = BTreeMap::new();
    for job in compiled.jobs()? {
        let id = pstr(job, "id");
        if let Some(document) = current_check(compiled, job)? {
            checks.insert(id, document);
        }
    }
    Ok(checks)
}

fn current_check(compiled: &Compiled, job: &Value) -> Result<Option<Value>> {
    let path = compiled.check_path(&pstr(job, "id"));
    if !std::path::Path::new(&path).is_file() {
        return Ok(None);
    }
    let document = util::read_json(&path)?;
    if document["schema_version"] != 1
        || document["kind"] != "term_cards_check"
        || document["plan_hash"] != json!(compiled.plan_hash)
        || document["plan_revision"] != json!(compiled.revision)
        || pstr(&document, "job") != pstr(job, "id")
    {
        return Ok(None);
    }
    if document["versions"] != compiled.versions() {
        return Ok(None);
    }
    let output = pstr(job, "output");
    let current = if std::path::Path::new(&output).is_file() {
        let text = util::read_text(&output)?;
        Some(canon::sha256_hex(text.as_bytes()))
    } else {
        None
    };
    let recorded = document["output_sha256"].as_str().map(str::to_string);
    if current != recorded {
        return Ok(None);
    }
    Ok(Some(document))
}

/// One term's review binding: the plan, source evidence, exact draft and exact rendered chain. A
/// changed content, rendering, source or plan produces another binding, so an old answer can never
/// approve it (R0321).
fn review_binding(
    compiled: &Compiled,
    job: &str,
    row: u64,
    term: &str,
    draft_sha: &str,
    render_sha: &str,
) -> Value {
    json!({
        "schema_version": 1,
        "kind": "term_card_review_binding",
        "plan_hash": compiled.plan_hash,
        "plan_revision": compiled.revision,
        "source_fingerprint": compiled.source_fingerprint(),
        "job": job,
        "row": row,
        "term": term,
        "draft_sha256": draft_sha,
        "render_sha256": render_sha,
    })
}

/// The stable file stem a review's request and answer share: the job, the row and the first eight
/// hex digits of each exact hash. The hashes decide the name, so a changed draft or rendering looks
/// for an answer that cannot exist yet.
fn review_stem(job: &str, row: u64, draft_sha: &str, render_sha: &str) -> String {
    format!(
        "{job}-r{row:03}-{}-{}",
        &draft_sha[..8.min(draft_sha.len())],
        &render_sha[..8.min(render_sha.len())]
    )
}

/// What one review answer says, as the compiler reads it.
enum Answer {
    Absent,
    Invalid(String),
    Verdict { ok: bool, faults: Vec<Value> },
}

fn read_answer(path: &str) -> Answer {
    if !std::path::Path::new(path).is_file() {
        return Answer::Absent;
    }
    let text = match util::read_text(path) {
        Ok(text) => text,
        Err(error) => return Answer::Invalid(format!("{error:#}")),
    };
    let value = match ps::parse_json(&text) {
        Ok(value) => value,
        Err(error) => return Answer::Invalid(format!("the answer is not JSON: {error}")),
    };
    let Some(object) = value.as_object() else {
        return Answer::Invalid("the answer is no JSON object".to_string());
    };
    let Some(ok) = object.get("ok").and_then(Value::as_bool) else {
        return Answer::Invalid("the answer has no `ok` boolean".to_string());
    };
    let mut faults = Vec::new();
    if !ok {
        let Some(items) = object.get("faults").and_then(Value::as_array) else {
            return Answer::Invalid("a refused review names its `faults` array".to_string());
        };
        for (index, item) in items.iter().enumerate() {
            for field in ["where", "what", "fix"] {
                let present = item
                    .get(field)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty());
                if !present {
                    return Answer::Invalid(format!("fault {} has no `{field}`", index + 1));
                }
            }
            faults.push(item.clone());
        }
        if faults.is_empty() {
            return Answer::Invalid("a refused review names at least one fault".to_string());
        }
    }
    Answer::Verdict { ok, faults }
}

/// A file's SHA-256, or null when it cannot be read.
fn file_sha256(path: &str) -> Value {
    match std::fs::read(path) {
        Ok(bytes) => json!(canon::sha256_hex(&bytes)),
        Err(_) => Value::Null,
    }
}

/// Every review answer file that was written for another binding of this row: the visible evidence
/// that a changed draft invalidated its old review.
fn stale_answers(compiled: &Compiled, job: &str, row: u64, current: &str) -> Vec<String> {
    let dir = compiled.answer_dir();
    let prefix = format!("{job}-r{row:03}-");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name.ends_with(".json") && name != format!("{current}.json") {
            out.push(entry.path().to_string_lossy().into_owned());
        }
    }
    out.sort();
    out
}

/// One row's review facts: whether the exact current draft has an answer, and what it says.
fn review_of(
    compiled: &Compiled,
    job: &str,
    row: u64,
    term: &str,
    draft_sha: &str,
    render_sha: &str,
) -> Value {
    let stem = review_stem(job, row, draft_sha, render_sha);
    let answer_path = ps::join(&compiled.answer_dir(), &format!("{stem}.json"));
    let request_path = ps::join(&compiled.request_dir(), &format!("{stem}.md"));
    let answer = read_answer(&answer_path);
    let stale = stale_answers(compiled, job, row, &stem);
    match answer {
        Answer::Absent => json!({
            "required": true,
            "status": if stale.is_empty() { "pending" } else { "stale" },
            "binding": canon::hash_value(&review_binding(compiled, job, row, term, draft_sha, render_sha)),
            "stem": stem,
            "request": request_path,
            "answer": answer_path,
            "faults": [],
            "stale": stale,
        }),
        Answer::Invalid(reason) => json!({
            "required": true,
            "status": "invalid",
            "reason": reason,
            "binding": canon::hash_value(&review_binding(compiled, job, row, term, draft_sha, render_sha)),
            "stem": stem,
            "request": request_path,
            "answer": answer_path,
            "faults": [],
            "stale": stale,
        }),
        Answer::Verdict { ok, faults } => json!({
            "required": true,
            "status": if ok { "approved" } else { "refused" },
            "binding": canon::hash_value(&review_binding(compiled, job, row, term, draft_sha, render_sha)),
            "stem": stem,
            "request": request_path,
            "answer": answer_path,
            "answer_sha256": file_sha256(&answer_path),
            "faults": faults,
            "stale": stale,
        }),
    }
}

/// The readiness result: per row its check, its review and its one permitted fix, reported
/// separately from publication (S19-AC07).
fn readiness_of(
    compiled: &Compiled,
    checks: &BTreeMap<String, Value>,
    state: &Value,
    notices: &[Value],
) -> Value {
    let fixes: Vec<Value> = state["fixes"].as_array().cloned().unwrap_or_default();
    let mut terms = Vec::new();
    for row in compiled.rows() {
        let number = row["row"].as_u64().unwrap_or(0);
        let term = pstr(row, "term");
        let job = pstr(row, "job");
        let planned = pstr(&row["classification"], "kind");
        let fix = fixes
            .iter()
            .find(|fix| fix["row"].as_u64() == Some(number))
            .cloned();
        let mut value = json!({
            "row": number,
            "term": term,
            "job": job,
            "planned_category": planned,
            "category": Value::Null,
            "promoted": false,
            "status": "unchecked",
            "reason": Value::Null,
            "draft_sha256": Value::Null,
            "render_sha256": Value::Null,
            "source_sha256": Value::Null,
            "check": Value::Null,
            "review": Value::Null,
            "fix": Value::Null,
            "cards": [],
            "supplements": [],
            "verification": Value::Null,
        });
        if let Some(record) = fix {
            let baseline = pstr(&record, "baseline_draft_sha256");
            value["fix"] = json!({
                "planned": true,
                "job": pstr(&record, "job_id"),
                "planned_at": record["planned_at"],
                "baseline_draft_sha256": baseline,
            });
        } else {
            value["fix"] = json!({"planned": false});
        }
        let Some(check) = checks.get(&job) else {
            value["reason"] = json!("this job has no current check artifact");
            terms.push(value);
            continue;
        };
        value["check"] = json!({
            "ok": check["ok"],
            "path": compiled.check_path(&job),
            "contract": check["contract"],
        });
        let Some(term_check) = check["terms"]
            .as_array()
            .and_then(|terms| terms.iter().find(|entry| entry["row"].as_u64() == Some(number)))
        else {
            value["status"] = json!("missing");
            value["reason"] = json!("the check artifact holds no entry for this row");
            terms.push(value);
            continue;
        };
        for key in ["category", "promoted", "draft_sha256", "render_sha256", "source_sha256"] {
            value[key] = term_check[key].clone();
        }
        value["supplements"] = term_check["supplements"].clone();
        value["verification"] = term_check["verification"].clone();
        value["cards"] = json!(term_check["cards"]
            .as_array()
            .map(|cards| cards.iter().map(|card| json!({
                "address": card["address"], "role": card["role"], "title": card["title"],
                "steps": card["steps"], "prose": card["prose"], "sha256": card["sha256"],
            })).collect::<Vec<_>>())
            .unwrap_or_default());
        let fix_planned = value["fix"]["planned"] == json!(true);
        let draft_sha = pstr(term_check, "draft_sha256");
        let applied = fix_planned && match value["fix"]["baseline_draft_sha256"].as_str() {
            Some(baseline) => !draft_sha.is_empty() && draft_sha != baseline,
            None => false,
        };
        match pstr(term_check, "status").as_str() {
            "missing" => {
                value["status"] = json!("missing");
                value["reason"] = term_check["reason"].clone();
            }
            "faulty" => {
                if !fix_planned {
                    value["status"] = json!("needs_fix");
                } else if !applied {
                    value["status"] = json!("refused");
                    value["reason"] = json!("fix_not_applied");
                } else {
                    value["status"] = json!("refused");
                    value["reason"] = json!("check_failed_after_fix");
                }
            }
            _ => {
                let math = pstr(term_check, "category") == "math";
                if !math {
                    value["status"] = json!("ready");
                    value["review"] = json!({"required": false, "status": "not_required"});
                    terms.push(value);
                    continue;
                }
                let render_sha = pstr(term_check, "render_sha256");
                let review = review_of(compiled, &job, number, &term, &draft_sha, &render_sha);
                let status = pstr(&review, "status");
                value["review"] = review.clone();
                match status.as_str() {
                    "approved" => {
                        value["status"] = json!("ready");
                    }
                    "refused" => {
                        if !fix_planned {
                            value["status"] = json!("needs_fix");
                        } else if !applied {
                            value["status"] = json!("refused");
                            value["reason"] = json!("fix_not_applied");
                        } else {
                            value["status"] = json!("refused");
                            value["reason"] = json!("review_refused_after_fix");
                        }
                    }
                    _ => {
                        // Pending, stale or invalid: the independent review of this exact draft is
                        // still owed before publication.
                        value["status"] = json!("needs_review");
                        if status == "invalid" {
                            value["reason"] = json!("invalid_answer");
                        }
                    }
                }
            }
        }
        terms.push(value);
    }
    let mut counts = BTreeMap::new();
    for status in [
        "ready",
        "needs_fix",
        "needs_review",
        "refused",
        "missing",
        "unchecked",
    ] {
        counts.insert(
            status.to_string(),
            json!(terms.iter().filter(|term| term["status"] == json!(status)).count()),
        );
    }
    let complete = !terms.is_empty() && terms.iter().all(|term| term["status"] == json!("ready"));
    json!({
        "schema_version": 1,
        "kind": "term_cards_readiness",
        "book": compiled.book(),
        "plan_revision": compiled.revision,
        "plan_hash": compiled.plan_hash,
        "list": {
            "address": compiled.plan["list"]["address"],
            "body_sha256": compiled.plan["list"]["body_sha256"],
        },
        "source": {"fingerprint": compiled.source_fingerprint()},
        "versions": compiled.versions(),
        "at": canon::utc_now_o(),
        "complete": complete,
        "counts": counts,
        "publication": {
            "state": "unverified",
            "reason": "readiness is not publication: -Step apply rechecks every Card through the service",
        },
        "notices": notices,
        "terms": terms,
    })
}

/// Publish the readiness artifact and describe it: the path, the hash and the counts.
fn publish_readiness(compiled: &Compiled, readiness: &Value) -> Result<Value> {
    let path = compiled.readiness_path();
    util::write_atomic(&path, &ps::json_pretty(readiness))?;
    Ok(json!({
        "path": path,
        "sha256": canon::sha256_hex(&std::fs::read(&path)?),
        "complete": readiness["complete"],
        "counts": readiness["counts"],
    }))
}

/// One row's promotion or fault notice, as `review-plan` and `fix-plan` report it.
fn term_notice(term: &Value, status: &str, reason: &str) -> Value {
    json!({
        "row": term["row"],
        "term": term["term"],
        "job": term["job"],
        "status": status,
        "reason": reason,
    })
}

/// The review step: for every mathematical draft that passed the check and has no answer bound to
/// its exact content, write the request and the fresh-conversation job (R0321, S19-AC02).
fn review_plan_step(p: &Params) -> Result<i32> {
    let compiled = Compiled::load(p)?;
    let selected = compiled.selected(p)?;
    let checks = current_checks(&compiled)?;
    for job in &selected {
        let id = pstr(job, "id");
        if !std::path::Path::new(&compiled.check_path(&id)).is_file() {
            bail!("no check artifact for job `{id}`; run -Step check first");
        }
        if !checks.contains_key(&id) {
            bail!(
                "the check artifact of job `{id}` no longer describes this plan or its writer output; run -Step check again"
            );
        }
    }
    let instruction = ensure_review_instruction(&compiled)?;
    std::fs::create_dir_all(compiled.request_dir())?;
    std::fs::create_dir_all(compiled.answer_dir())?;
    let mut jobs = Vec::new();
    let mut reviews = Vec::new();
    for job in &selected {
        let id = pstr(job, "id");
        let check = checks.get(&id).cloned().unwrap_or(Value::Null);
        for term in check["terms"].as_array().cloned().unwrap_or_default() {
            if term["status"] != json!("ok") || term["category"] != json!("math") {
                continue;
            }
            let row = term["row"].as_u64().unwrap_or(0);
            let name = pstr(&term, "term");
            let draft_sha = pstr(&term, "draft_sha256");
            let render_sha = pstr(&term, "render_sha256");
            let review = review_of(&compiled, &id, row, &name, &draft_sha, &render_sha);
            let status = pstr(&review, "status");
            if matches!(status.as_str(), "approved" | "refused") {
                reviews.push(term_notice(&term, &status, ""));
                continue;
            }
            let request = pstr(&review, "request");
            let request_text = review_request_text(
                &compiled,
                job,
                &term,
                &review,
                &instruction,
            )?;
            util::write_atomic(&request, &request_text)?;
            let answer = pstr(&review, "answer");
            let job_id = format!("{id}-r{row:03}-review");
            jobs.push(managed_job(json!({
                "id": job_id,
                "kind": "review",
                "row": row,
                "term": name,
                "files": [instruction, request],
                "prompt": REVIEW_PROMPT,
                "answer": answer,
                "writes": compiled.answer_dir(),
                "binding": review["binding"],
                "request": request,
            }), &REVIEWER_SKILLS, true));
            reviews.push(term_notice(&term, "planned", ""));
        }
    }
    let state = local_state(&compiled)?;
    let readiness = readiness_of(&compiled, &checks, &state, &[]);
    let published = publish_readiness(&compiled, &readiness)?;
    let summary = json!({
        "schema_version": 1,
        "ok": true,
        "step": "review-plan",
        "book": compiled.book(),
        "plan_revision": compiled.revision,
        "plan_hash": compiled.plan_hash,
        "instruction": instruction,
        "jobs": jobs,
        "reviews": reviews,
        "readiness": published,
        "counts": readiness["counts"],
        "complete": readiness["complete"],
    });
    util::print_line(&canon::canonical_json(&summary))?;
    Ok(0)
}

/// The shared review instruction, copied into the work folder so a run keeps the exact text its
/// reviews were asked against.
fn ensure_review_instruction(compiled: &Compiled) -> Result<String> {
    let source = util::read_text(ps::join(&common::scripts_dir(), REVIEW_INSTRUCTION))
        .with_context(|| format!("missing the Term-card review instruction file {REVIEW_INSTRUCTION}"))?;
    let dest = ps::join(&compiled.work, REVIEW_INSTRUCTION);
    util::write_atomic(&dest, &source)?;
    Ok(dest)
}

/// One review request: the exact term, its source evidence, every bound hash, the draft JSON and
/// the one answer file the reviewer owns.
fn review_request_text(
    compiled: &Compiled,
    job: &Value,
    term: &Value,
    review: &Value,
    instruction: &str,
) -> Result<String> {
    let id = pstr(job, "id");
    let row = term["row"].as_u64().unwrap_or(0);
    let name = pstr(term, "term");
    let meta = compiled.row(row).cloned().unwrap_or(Value::Null);
    let mut sb = Sb::new();
    sb.line(format!("# 术语卡审查任务 {id}-r{row:03}"))
        .blank();
    sb.line(format!(
        "- 本书：`[[{}]]`《{}》",
        compiled.book(),
        pstr(&compiled.plan["book"], "title")
    ));
    sb.line(format!(
        "- 术语：`{name}`（术语表第 {row} 行；任务 {id}；计划分类：{}）",
        if pstr(&meta["classification"], "kind") == "math" { "数学概念" } else { "概念（已提升为数学概念）" }
    ));
    sb.line(format!("- 审查说明（先读它）：`{instruction}`"));
    sb.line(format!(
        "- 原任务文件（定义句、出现小节、符号表）：`{}`",
        pstr(job, "file")
    ));
    sb.line(format!(
        "- 原文小节：`{}`",
        ps::join(&compiled.plan["source"]["staging"].as_str().unwrap_or_default(), "sections")
    ))
    .line(format!(
        "- 渲染出的卡：{} 张（1 张正文卡，{} 张续卡）",
        term["cards"].as_array().map_or(0, Vec::len),
        term["cards"].as_array().map_or(0, Vec::len).saturating_sub(1)
    ))
    .blank();
    sb.line("## 绑定（草稿或渲染一变，这个审查就作废）").blank();
    sb.line(format!("- plan_revision={} plan_hash=`{}`", compiled.revision, compiled.plan_hash));
    sb.line(format!("- source_fingerprint=`{}`", compiled.source_fingerprint()));
    sb.line(format!("- draft_sha256=`{}`", pstr(term, "draft_sha256")));
    sb.line(format!("- render_sha256=`{}`", pstr(term, "render_sha256")));
    sb.line(format!(
        "- 审查绑定=`{}`",
        review["binding"].as_str().unwrap_or_default()
    ))
    .blank();
    sb.line("## 草稿 JSON").blank();
    sb.line("```json");
    sb.line(ps::json_pretty(&term["draft"]));
    sb.line("```").blank();
    sb.line("## 来源证据").blank();
    sb.line(format!(
        "任务文件里的定义句与出现小节是你要核对的原文依据：`{}`。原文在 `{}`。",
        pstr(job, "file"),
        ps::join(&compiled.plan["source"]["staging"].as_str().unwrap_or_default(), "sections")
    ))
    .blank();
    sb.line("## 答案").blank();
    sb.line(format!(
        "把审查结果写入：`{}`（UTF-8，只写这一个文件）。",
        pstr(review, "answer")
    ));
    sb.line("通过：`{\"ok\":true}`。不通过：`{\"ok\":false,\"faults\":[{\"where\":\"derivations[0].steps[4]\",\"what\":\"一句话说错在哪\",\"fix\":\"一句话说怎么改\"}]}`。")
        .blank();
    sb.line(format!(
        "再次强调：你只按 `{}` 逐项核对这张草稿，不改草稿，不写卡。",
        instruction
    ));
    Ok(sb.0)
}

/// The fix step: one finite fix prompt per writer job, sent back to the writer's own conversation.
/// A term whose permitted fix is already planned gets no second prompt (R0321, S19-AC04/AC05).
fn fix_plan_step(p: &Params) -> Result<i32> {
    let compiled = Compiled::load(p)?;
    let selected = compiled.selected(p)?;
    let checks = current_checks(&compiled)?;
    for job in &selected {
        let id = pstr(job, "id");
        if !std::path::Path::new(&compiled.check_path(&id)).is_file() {
            bail!("no check artifact for job `{id}`; run -Step check first");
        }
        if !checks.contains_key(&id) {
            bail!(
                "the check artifact of job `{id}` no longer describes this plan or its writer output; run -Step check again"
            );
        }
    }
    let mut state = local_state(&compiled)?;
    let mut fixes: Vec<Value> = state["fixes"].as_array().cloned().unwrap_or_default();
    let mut jobs = Vec::new();
    let mut planned = Vec::new();
    let mut final_terms = Vec::new();
    std::fs::create_dir_all(compiled.fix_dir())?;
    for job in &selected {
        let id = pstr(job, "id");
        let check = checks.get(&id).cloned().unwrap_or(Value::Null);
        let mut need: Vec<Value> = Vec::new();
        let mut keep: Vec<String> = Vec::new();
        for term in check["terms"].as_array().cloned().unwrap_or_default() {
            let row = term["row"].as_u64().unwrap_or(0);
            let name = pstr(&term, "term");
            let already = fixes
                .iter()
                .find(|fix| fix["row"].as_u64() == Some(row))
                .cloned();
            let math = pstr(&term, "category") == "math";
            let review = if math && term["status"] == json!("ok") {
                Some(review_of(
                    &compiled,
                    &id,
                    row,
                    &name,
                    &pstr(&term, "draft_sha256"),
                    &pstr(&term, "render_sha256"),
                ))
            } else {
                None
            };
            let refused = review
                .as_ref()
                .is_some_and(|review| review["status"] == json!("refused"));
            let faulty = pstr(&term, "status") == "faulty";
            if pstr(&term, "status") == "missing" {
                final_terms.push(term_notice(&term, "missing", "the writer wrote no draft; retry the writer job"));
                continue;
            }
            if !faulty && !refused {
                keep.push(name);
                continue;
            }
            if let Some(already) = already {
                let baseline = pstr(&already, "baseline_draft_sha256");
                let current = pstr(&term, "draft_sha256");
                let reason = if current == baseline {
                    "fix_not_applied"
                } else if faulty {
                    "check_failed_after_fix"
                } else {
                    "review_refused_after_fix"
                };
                final_terms.push(term_notice(&term, "refused", reason));
                continue;
            }
            need.push(json!({"term": term, "review": review}));
        }
        if need.is_empty() {
            continue;
        }
        let fix_id = format!("{id}-fix");
        let fix_file = ps::join(&compiled.fix_dir(), &format!("{fix_id}.md"));
        let text = fix_text(&compiled, job, &check, &need, &keep)?;
        util::write_atomic(&fix_file, &text)?;
        for item in &need {
            let term = &item["term"];
            let row = term["row"].as_u64().unwrap_or(0);
            fixes.push(json!({
                "row": row,
                "term": term["term"],
                "job": id,
                "job_id": fix_id,
                "baseline_draft_sha256": term["draft_sha256"],
                "planned_at": canon::utc_now_o(),
            }));
            planned.push(term_notice(term, "planned", ""));
        }
        jobs.push(managed_job(json!({
            "id": fix_id,
            "kind": "fix",
            "continue": id,
            "files": [compiled.instruction(), fix_file],
            "fallback": [
                compiled.instruction(),
                pstr(job, "file"),
                pstr(job, "output"),
                fix_file,
            ],
            "prompt": FIX_PROMPT,
            "output": pstr(job, "output"),
            "permissions": {"archive_read": false, "archive_write": false},
            "fix": fix_file,
        }), &WRITER_SKILLS, false));
    }
    state["fixes"] = json!(fixes);
    save_state(&compiled, &state)?;
    let readiness = readiness_of(&compiled, &checks, &state, &[]);
    let published = publish_readiness(&compiled, &readiness)?;
    let summary = json!({
        "schema_version": 1,
        "ok": true,
        "step": "fix-plan",
        "book": compiled.book(),
        "plan_revision": compiled.revision,
        "plan_hash": compiled.plan_hash,
        "jobs": jobs,
        "planned": planned,
        "final": final_terms,
        "readiness": published,
        "counts": readiness["counts"],
        "complete": readiness["complete"],
    });
    util::print_line(&canon::canonical_json(&summary))?;
    Ok(0)
}

/// One fix prompt: every rejected requirement of the job's terms, the terms that passed and must
/// stay as they are, and the one output file the writer rewrites (S19-AC04).
fn fix_text(
    compiled: &Compiled,
    job: &Value,
    check: &Value,
    need: &[Value],
    keep: &[String],
) -> Result<String> {
    let id = pstr(job, "id");
    let rows: Vec<String> = need
        .iter()
        .map(|item| format!("第 {} 行 `{}`", item["term"]["row"].as_u64().unwrap_or(0), pstr(&item["term"], "term")))
        .collect();
    let mut sb = Sb::new();
    sb.line(format!("# 术语卡修复任务 {id}-fix（{}）", rows.join("、")))
        .blank();
    sb.line("这是本组唯一一次修复。只改下面列出的问题，写出完整的 JSON 文件，不新建文件。")
        .line("不要改其他术语，不要改写作格式，不要调用 zt。")
        .blank();
    sb.line(format!("- 共享说明（重读它）：`{}`", compiled.instruction()));
    sb.line(format!("- 原任务文件（术语与原文证据）：`{}`", pstr(job, "file")));
    sb.line(format!("- 你上一版草稿：`{}`", pstr(job, "output")));
    sb.line(format!("- 只写这个文件（整份 JSON）：`{}`", pstr(job, "output")));
    sb.line(format!(
        "- 任务绑定：plan_revision={} plan_hash=`{}` job={id}",
        compiled.revision, compiled.plan_hash
    ))
    .blank();
    for item in need {
        let term = &item["term"];
        let row = term["row"].as_u64().unwrap_or(0);
        sb.line(format!("## 第 {row} 行 `{}`", pstr(term, "term"))).blank();
        let schema = term["schema_faults"].as_array().cloned().unwrap_or_default();
        let render = term["render_faults"].as_array().cloned().unwrap_or_default();
        let diagnostics = term["diagnostics"].as_array().cloned().unwrap_or_default();
        if !schema.is_empty() || !render.is_empty() || !diagnostics.is_empty() {
            sb.line("### 格式与检查").blank();
            for fault in schema.iter().chain(render.iter()) {
                sb.line(format!(
                    "- `{}`：{}",
                    pstr(fault, "path"),
                    pstr(fault, "message")
                ));
            }
            for diagnostic in &diagnostics {
                if pstr(diagnostic, "severity") != "block" {
                    continue;
                }
                sb.line(format!(
                    "- `{}` {} {}：{}",
                    pstr(diagnostic, "address"),
                    pstr(diagnostic, "rule"),
                    pstr(diagnostic, "fault"),
                    pstr(diagnostic, "message")
                ));
            }
            sb.blank();
        }
        if let Some(review) = item.get("review").filter(|review| review["status"] == json!("refused")) {
            sb.line("### 独立审查").blank();
            for fault in review["faults"].as_array().cloned().unwrap_or_default() {
                sb.line(format!(
                    "- `{}`：{} 修法：{}",
                    pstr(&fault, "where"),
                    pstr(&fault, "what"),
                    pstr(&fault, "fix")
                ));
            }
            sb.blank();
        }
    }
    sb.line("## 本组其他术语（已通过检查，原样保留内容）").blank();
    if keep.is_empty() {
        sb.line("（没有）");
    } else {
        for name in keep {
            sb.line(format!("- `{name}`"));
        }
    }
    sb.blank();
    sb.line("## 输出").blank();
    sb.line(format!(
        "重写 `{}`，UTF-8，只写这一个文件，格式同前（`{{\"terms\":[…]}}`，本组 {} 个术语都要在）。",
        pstr(job, "output"),
        check["terms"].as_array().map_or(0, Vec::len)
    ));
    sb.line("写完每个术语后自己核对上面每条问题都已修掉。");
    sb.line("写完后只回复一行 JSON：`{\"terms\":术语数,\"fixed\":true}`。");
    Ok(sb.0)
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
        for step in ["check", "review-plan", "fix-plan"] {
            assert!(
                params(&argv(&["-Step", step, "-Staging", "s", "-Token", "3"])).is_err(),
                "{step} claims no job unit"
            );
        }
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
    fn the_plan_hash_does_not_depend_on_the_lists_kind() {
        // The first publication retypes a legacy table metadata-only (S21, #165; R0336). A resume
        // that re-reads the retyped list must keep the same plan hash, its check artifacts and its
        // receipts, so the list's kind is no list content.
        let legacy = json!({"list": {"address": "K/1|a", "type": "idea", "legacy": true, "body_sha256": "s"}});
        let retyped = json!({"list": {"address": "K/1|a", "type": "term", "legacy": false, "body_sha256": "s"}});
        assert_eq!(plan_hash_of(&legacy), plan_hash_of(&retyped));
        let mut changed = retyped.clone();
        changed["list"]["body_sha256"] = json!("other");
        assert_ne!(plan_hash_of(&legacy), plan_hash_of(&changed));
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
        // The frozen content hashes to the plan hash and the derived keys are not part of it; the
        // list's kind is no list content either (S21, #165).
        let frozen = frozen_of(&plan).unwrap();
        assert_eq!(plan_hash_of(&frozen), plan["plan_hash"]);
        assert_ne!(
            canon::hash_value(&frozen),
            plan["plan_hash"],
            "the list's kind is not part of the plan content hash"
        );
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

    // --- the check, review and fix steps (S19, #162) ----------------------------------------

    fn s19_row(number: u64, term: &str, job: &str, kind: &str) -> Value {
        json!({
            "row": number,
            "term": term,
            "avoid": [],
            "meaning": "双方都不改变选择",
            "defined": "§1.1",
            "snippets": [],
            "occurrences": [],
            "notation": [],
            "neighbors": [],
            "planning_artifact": Value::Null,
            "classification": {"kind": kind, "evidence": [], "candidates": {}, "promoted": false},
            "job": job,
        })
    }

    fn s19_jobs(work: &str, rows: &[Value]) -> Vec<Value> {
        let mut ids: Vec<String> = rows.iter().map(|row| pstr(row, "job")).collect();
        ids.sort();
        ids.dedup();
        ids.iter()
            .map(|id| {
                let owned: Vec<Value> = rows
                    .iter()
                    .filter(|row| pstr(row, "job") == *id)
                    .cloned()
                    .collect();
                json!({
                    "id": id,
                    "kind": if owned.iter().all(|row| row["classification"]["kind"] == json!("math")) {
                        "math"
                    } else {
                        "concept"
                    },
                    "rows": owned.iter().map(|row| row["row"].clone()).collect::<Vec<_>>(),
                    "terms": owned.iter().map(|row| row["term"].clone()).collect::<Vec<_>>(),
                    "file": ps::join(&ps::join(work, "jobs"), &format!("{id}.md")),
                    "output": ps::join(&ps::join(work, "out"), &format!("{id}.json")),
                })
            })
            .collect()
    }

    /// A frozen plan shaped and hashed exactly as `plan` freezes one.
    fn s19_plan(work: &str, rows: Vec<Value>) -> Value {
        let jobs = s19_jobs(work, &rows);
        let mut frozen = json!({
            "schema_version": 1,
            "kind": "term_cards_plan",
            "book": {"citation_key": "Test2026", "title": "Test Book", "variant": ""},
            "list": {
                "address": "Test2026/1|a",
                "type": "term",
                "legacy": false,
                "title": "术语表：《Test Book》的 2 个术语",
                "title_sha256": "title",
                "body_sha256": "body",
                "row_count": rows.len(),
            },
            "source": {"staging": work, "sections": [], "fingerprint": "source-fingerprint"},
            "notation": {"rows": [], "sources": []},
            "planning_artifact": Value::Null,
            "limits": {"per_job": 4, "math_per_job": 1, "max_steps_per_card": 12, "max_prose": 1600},
            "rows": rows,
            "jobs": jobs,
            "versions": {"compiler": "test", "checker": "test", "rule": "test", "instruction": {"file": "x", "sha256": "y"}},
        });
        let hash = canon::hash_value(&frozen);
        frozen["revision"] = json!(1);
        frozen["plan_hash"] = json!(hash);
        frozen["created_at"] = json!("2026-01-01T00:00:00Z");
        frozen
    }

    /// The compiled plan a step reads: `plan.json` under `<staging>/term-cards`.
    fn s19_compiled(dir: &std::path::Path, rows: Vec<Value>) -> Compiled {
        s19_compiled_with(dir, rows, 1600)
    }

    fn s19_compiled_with(dir: &std::path::Path, rows: Vec<Value>, max_prose: i64) -> Compiled {
        let work = ps::join(dir.to_str().unwrap(), WORK);
        std::fs::create_dir_all(ps::join(&work, "out")).unwrap();
        std::fs::create_dir_all(ps::join(&work, "check")).unwrap();
        let mut plan = s19_plan(&work, rows);
        plan["limits"]["max_prose"] = json!(max_prose);
        let hash = plan_hash_of(&frozen_of(&plan).unwrap());
        plan["plan_hash"] = json!(hash);
        util::write_atomic(ps::join(&work, "plan.json"), &ps::json_pretty(&plan)).unwrap();
        Compiled {
            plan_hash: pstr(&plan, "plan_hash"),
            revision: 1,
            plan,
            work,
        }
    }

    fn s19_params(step: &str, staging: &str) -> Params {
        Params {
            step: step.to_string(),
            staging: staging.to_string(),
            per_job: DEFAULT_PER_JOB,
            math_per_job: DEFAULT_MATH_PER_JOB,
            max_steps: DEFAULT_MAX_STEPS,
            max_prose: DEFAULT_MAX_PROSE,
            jobs: Vec::new(),
            token: None,
            apply: step == "apply",
        }
    }

    /// The draft zt-check's own Term tests use: a complete mathematical head.
    fn s19_math_draft() -> Value {
        json!({
            "term": "均衡",
            "original": "equilibrium",
            "category": "math",
            "first_defined": "§1.1",
            "explanation": [
                "均衡是双方都不改变选择的状态。",
                "本书用它说明市场的稳定点。",
                "均衡不等于最优。"
            ],
            "book_definition": { "text": ["§1.1 把均衡定义为一个不动点。"], "section": "§1.1", "quote": "" },
            "hypotheses": [ { "id": "H1", "text": "策略集是紧凸集。" } ],
            "definition": {
                "formula": "x^* \\in \\arg\\max_{x} u(x) \\tag{T1}",
                "reading": ["$x^*$ 是没有人愿意偏离的策略。"]
            },
            "symbols": [
                { "symbol": "$x^*$", "meaning": "均衡策略", "type": "紧凸集", "auxiliary": false }
            ],
            "equivalent_forms": [ { "formula": "u(x^*)\\ge u(x)", "reason": "由 (T1) 直接得到。" } ],
            "derivations": [ {
                "claim": "均衡是纳什均衡",
                "steps": [
                    { "text": "起点是定义 (T1)。", "basis": "定义 (T1)", "formula": "x^*=x" },
                    { "text": "两边比较收益。", "basis": "H1", "formula": "u(x^*)\\ge u(x)" }
                ],
                "result": "均衡是纳什均衡。"
            } ],
            "limits": [ { "case": "无", "text": "本定义没有需要说明的极限情形。", "formula": "" } ],
            "example": ["取 $u(x)=-x^2$。", "唯一均衡是 $x^*=0$。"],
            "intuition": ["没有人愿意单方面偏离。", "均衡是相互一致的选择。"],
            "distinctions": [ { "term": "剩余控制权", "text": "剩余控制权说的是决定权。" } ],
            "misreadings": ["均衡不是最优。"],
            "role_in_book": ["本书第一章用均衡定义市场结果。"]
        })
    }

    fn s19_concept_draft(term: &str, distinction: &str) -> Value {
        json!({
            "term": term,
            "category": "concept",
            "first_defined": "§2",
            "explanation": [
                "这个词指对资产用途的决定权。",
                "本书用它区分所有权与控制。",
                "这个概念不等于日常的经营权。"
            ],
            "book_definition": { "text": ["§2 把这个词定义为合同没有写明的决定权。"], "section": "§2", "quote": "" },
            "distinctions": [ { "term": distinction, "text": "均衡指双方都不改变选择。" } ],
            "misreadings": ["这个概念不是剩余收益。"],
            "role_in_book": ["本书第二章用它解释企业边界。"]
        })
    }

    fn s19_two_rows() -> Vec<Value> {
        vec![
            s19_row(1, "均衡", "t001", "math"),
            s19_row(2, "剩余控制权", "t002", "concept"),
        ]
    }

    fn s19_write_output(compiled: &Compiled, job: &str, items: &[Value]) {
        let path = ps::join(&ps::join(&compiled.work, "out"), &format!("{job}.json"));
        util::write_atomic(&path, &ps::json_pretty(&json!({"terms": items}))).unwrap();
    }

    fn s19_readiness(compiled: &Compiled) -> Value {
        util::read_json(compiled.readiness_path()).unwrap()
    }

    fn s19_read(compiled: &Compiled, name: &str) -> Value {
        util::read_json(ps::join(&compiled.work, name)).unwrap()
    }

    #[test]
    fn a_check_reports_each_terms_schema_fault_and_ready_result() {
        let dir = tempfile::tempdir().unwrap();
        let compiled = s19_compiled(dir.path(), s19_two_rows());
        let mut broken = s19_math_draft();
        broken["explanation"] = json!(["只有一句话。"]);
        s19_write_output(&compiled, "t001", &[broken]);
        s19_write_output(&compiled, "t002", &[s19_concept_draft("剩余控制权", "均衡")]);
        let p = s19_params("check", dir.path().to_str().unwrap());
        assert_eq!(check_step(&p).unwrap(), 0);

        let math = s19_read(&compiled, &ps::join("check", "t001.json"));
        assert_eq!(math["kind"], "term_cards_check");
        assert_eq!(math["ok"], false);
        assert_eq!(math["terms"][0]["status"], "faulty");
        assert_eq!(math["terms"][0]["schema_faults"][0]["path"], "term.explanation");
        assert_eq!(math["terms"][0]["draft_sha256"].is_string(), true);
        let concept = s19_read(&compiled, &ps::join("check", "t002.json"));
        assert_eq!(concept["ok"], true, "{concept}");
        assert_eq!(concept["terms"][0]["status"], "ok");
        assert!(concept["terms"][0]["render_sha256"].is_string());
        assert_eq!(concept["terms"][0]["cards"].as_array().unwrap().len(), 1);

        let readiness = s19_readiness(&compiled);
        assert_eq!(readiness["kind"], "term_cards_readiness");
        assert_eq!(readiness["complete"], false);
        assert_eq!(readiness["terms"][0]["status"], "needs_fix");
        assert_eq!(readiness["terms"][1]["status"], "ready");
        assert_eq!(readiness["publication"]["state"], "unverified");
        assert_eq!(readiness["counts"]["needs_fix"], 1);
        assert_eq!(readiness["counts"]["ready"], 1);
        // The state records the checked progress, never a publication claim.
        let state = s19_read(&compiled, "state.json");
        assert_eq!(state["jobs"]["t001"]["state"], "checked");
        assert_eq!(state["jobs"]["t002"]["state"], "checked");
    }

    #[test]
    fn a_plan_math_row_cannot_be_demoted_to_a_concept_draft() {
        let dir = tempfile::tempdir().unwrap();
        let compiled = s19_compiled(dir.path(), s19_two_rows());
        s19_write_output(&compiled, "t001", &[s19_concept_draft("均衡", "剩余控制权")]);
        s19_write_output(&compiled, "t002", &[s19_concept_draft("剩余控制权", "均衡")]);
        let p = s19_params("check", dir.path().to_str().unwrap());
        check_step(&p).unwrap();
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        assert_eq!(check["terms"][0]["status"], "faulty");
        assert_eq!(check["terms"][0]["schema_faults"][0]["path"], "category");
        assert!(
            pstr(&check["terms"][0]["schema_faults"][0], "message").contains("cannot clear"),
            "{}",
            check["terms"][0]
        );
    }

    #[test]
    fn a_promoted_concept_draft_is_recorded_and_waits_for_its_review() {
        let dir = tempfile::tempdir().unwrap();
        let mut rows = s19_two_rows();
        rows[0] = s19_row(1, "均衡", "t001", "concept");
        let compiled = s19_compiled(dir.path(), rows);
        // A concept-classified row may promote itself by writing `math`.
        s19_write_output(&compiled, "t001", &[s19_math_draft()]);
        s19_write_output(&compiled, "t002", &[s19_concept_draft("剩余控制权", "均衡")]);
        let p = s19_params("check", dir.path().to_str().unwrap());
        check_step(&p).unwrap();
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        assert_eq!(check["terms"][0]["status"], "ok");
        assert_eq!(check["terms"][0]["promoted"], true);
        assert_eq!(check["terms"][0]["category"], "math");
        let state = s19_read(&compiled, "state.json");
        assert_eq!(state["promotions"][0]["row"], 1);
        assert_eq!(state["promotions"][0]["from"], "concept");
        assert_eq!(state["promotions"][0]["to"], "math");
        let readiness = s19_readiness(&compiled);
        assert_eq!(readiness["terms"][0]["status"], "needs_review");
        assert_eq!(readiness["terms"][0]["review"]["required"], true);
    }

    #[test]
    fn a_review_is_bound_to_the_exact_draft_and_render_and_a_change_invalidates_it() {
        let dir = tempfile::tempdir().unwrap();
        let compiled = s19_compiled(dir.path(), s19_two_rows());
        s19_write_output(&compiled, "t001", &[s19_math_draft()]);
        s19_write_output(&compiled, "t002", &[s19_concept_draft("剩余控制权", "均衡")]);
        let p = s19_params("check", dir.path().to_str().unwrap());
        check_step(&p).unwrap();
        let review = s19_params("review-plan", dir.path().to_str().unwrap());
        assert_eq!(review_plan_step(&review).unwrap(), 0);

        // The request names the exact draft and render hashes, and carries the draft JSON.
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        let term = &check["terms"][0];
        let draft_sha = pstr(term, "draft_sha256");
        let render_sha = pstr(term, "render_sha256");
        let stem = review_stem("t001", 1, &draft_sha, &render_sha);
        let request = ps::join(
            &ps::join(&compiled.review_dir(), "requests"),
            &format!("{stem}.md"),
        );
        let text = util::read_text(&request).unwrap();
        assert!(text.contains(&draft_sha), "{text}");
        assert!(text.contains(&render_sha), "{text}");
        assert!(text.contains("\"均衡\""), "{text}");
        assert!(text.contains("source-fingerprint"), "{text}");

        // An approval bound to this exact content makes the term ready.
        let answer = ps::join(
            &ps::join(&compiled.review_dir(), "answers"),
            &format!("{stem}.json"),
        );
        util::write_atomic(&answer, "{\"ok\":true}").unwrap();
        assert_eq!(review_plan_step(&review).unwrap(), 0);
        let readiness = s19_readiness(&compiled);
        assert_eq!(readiness["terms"][0]["status"], "ready");
        assert_eq!(readiness["terms"][0]["review"]["status"], "approved");
        assert_eq!(readiness["terms"][1]["review"]["status"], "not_required");
        assert_eq!(readiness["complete"], true);

        // A changed mathematical draft keeps the old answer as visible stale evidence and owes a
        // fresh review; the approval does not travel to the new content.
        let mut changed = s19_math_draft();
        changed["explanation"] = json!([
            "均衡是双方都不改变选择的状态。",
            "本书用它说明市场的稳定点。",
            "均衡不等于社会最优。"
        ]);
        s19_write_output(&compiled, "t001", &[changed]);
        check_step(&p).unwrap();
        let readiness = s19_readiness(&compiled);
        assert_eq!(readiness["terms"][0]["status"], "needs_review");
        assert_eq!(readiness["terms"][0]["review"]["status"], "stale");
        assert_eq!(readiness["terms"][0]["review"]["stale"][0], json!(answer));
        assert_eq!(readiness["complete"], false);
        assert_eq!(review_plan_step(&review).unwrap(), 0);
        let fresh = s19_readiness(&compiled);
        let new_render = pstr(&fresh["terms"][0], "render_sha256");
        assert_ne!(new_render, render_sha, "the changed draft renders other bytes");
    }

    #[test]
    fn one_finite_fix_returns_to_the_writer_and_a_second_refusal_is_final() {
        let dir = tempfile::tempdir().unwrap();
        let compiled = s19_compiled(dir.path(), s19_two_rows());
        let mut faulty = s19_math_draft();
        faulty["derivations"][0]["steps"][0]["text"] = json!("显然成立。");
        s19_write_output(&compiled, "t001", &[faulty]);
        s19_write_output(&compiled, "t002", &[s19_concept_draft("剩余控制权", "均衡")]);
        let p = s19_params("check", dir.path().to_str().unwrap());
        check_step(&p).unwrap();
        let readiness = s19_readiness(&compiled);
        assert_eq!(readiness["terms"][0]["status"], "needs_fix");
        let t6 = readiness["terms"][0]["check"]["ok"].clone();
        assert_eq!(t6, false, "{}", readiness["terms"][0]);
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        assert!(
            check["terms"][0]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic["rule"] == json!("T6")),
            "{}",
            check["terms"][0]["diagnostics"]
        );

        // One fix prompt goes back to the writer's own conversation through continue/fallback.
        let fix = s19_params("fix-plan", dir.path().to_str().unwrap());
        assert_eq!(fix_plan_step(&fix).unwrap(), 0);
        let state = s19_read(&compiled, "state.json");
        assert_eq!(state["fixes"][0]["row"], 1);
        assert_eq!(state["fixes"][0]["job_id"], "t001-fix");
        let fix_file = ps::join(&ps::join(&compiled.work, "fix"), "t001-fix.md");
        let text = util::read_text(&fix_file).unwrap();
        assert!(text.contains("显然"), "{text}");
        assert!(text.contains("唯一一次修复"), "{text}");

        // A second attempt without a changed draft is final for the run.
        assert_eq!(fix_plan_step(&fix).unwrap(), 0);
        let readiness = s19_readiness(&compiled);
        assert_eq!(readiness["terms"][0]["status"], "refused");
        assert_eq!(readiness["terms"][0]["reason"], "fix_not_applied");

        // The writer's fix changes the draft: the check passes and a fresh review is owed.
        s19_write_output(&compiled, "t001", &[s19_math_draft()]);
        check_step(&p).unwrap();
        let review = s19_params("review-plan", dir.path().to_str().unwrap());
        review_plan_step(&review).unwrap();
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        let term = &check["terms"][0];
        let stem = review_stem(
            "t001",
            1,
            &pstr(term, "draft_sha256"),
            &pstr(term, "render_sha256"),
        );
        let answer = ps::join(
            &ps::join(&compiled.review_dir(), "answers"),
            &format!("{stem}.json"),
        );
        util::write_atomic(
            &answer,
            &json!({"ok": false, "faults": [{"where": "derivations[0].steps[1]", "what": "这一步的符号不对。", "fix": "按 H1 重写这一步。"}]})
                .to_string(),
        )
        .unwrap();
        // The refused re-review is final: no second fix prompt.
        assert_eq!(fix_plan_step(&fix).unwrap(), 0);
        let readiness = s19_readiness(&compiled);
        assert_eq!(readiness["terms"][0]["status"], "refused");
        assert_eq!(readiness["terms"][0]["reason"], "review_refused_after_fix");
        assert_eq!(readiness["terms"][0]["review"]["faults"][0]["where"], "derivations[0].steps[1]");
    }

    #[test]
    fn a_missing_output_is_reported_without_a_fix_or_a_publication_claim() {
        let dir = tempfile::tempdir().unwrap();
        let compiled = s19_compiled(dir.path(), s19_two_rows());
        s19_write_output(&compiled, "t002", &[s19_concept_draft("剩余控制权", "均衡")]);
        let p = s19_params("check", dir.path().to_str().unwrap());
        check_step(&p).unwrap();
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        assert_eq!(check["terms"][0]["status"], "missing");
        let readiness = s19_readiness(&compiled);
        assert_eq!(readiness["terms"][0]["status"], "missing");
        assert_eq!(readiness["complete"], false);
        assert_eq!(readiness["publication"]["state"], "unverified");
        let fix = s19_params("fix-plan", dir.path().to_str().unwrap());
        assert_eq!(fix_plan_step(&fix).unwrap(), 0);
        let state = s19_read(&compiled, "state.json");
        assert_eq!(state["fixes"], json!([]), "a missing draft is no content fix");
    }

    #[test]
    fn a_changed_writer_file_makes_an_old_check_stale_before_review_or_fix() {
        let dir = tempfile::tempdir().unwrap();
        let compiled = s19_compiled(dir.path(), s19_two_rows());
        s19_write_output(&compiled, "t001", &[s19_math_draft()]);
        s19_write_output(&compiled, "t002", &[s19_concept_draft("剩余控制权", "均衡")]);
        let p = s19_params("check", dir.path().to_str().unwrap());
        check_step(&p).unwrap();
        let mut changed = s19_math_draft();
        changed["explanation"] = json!([
            "均衡是双方都不改变选择的状态。",
            "本书用它说明市场的稳定点。",
            "均衡不等于社会最优。"
        ]);
        s19_write_output(&compiled, "t001", &[changed]);
        let review = s19_params("review-plan", dir.path().to_str().unwrap());
        let refused = review_plan_step(&review).unwrap_err().to_string();
        assert!(refused.contains("run -Step check again"), "{refused}");
        let fix = s19_params("fix-plan", dir.path().to_str().unwrap());
        assert!(fix_plan_step(&fix).is_err());
    }

    #[test]
    fn a_check_carries_the_exact_row_autolink_delimiter_and_per_card_bound_faults() {
        let dir = tempfile::tempdir().unwrap();
        let compiled = s19_compiled(dir.path(), s19_two_rows());
        s19_write_output(&compiled, "t002", &[s19_concept_draft("剩余控制权", "均衡")]);
        let p = s19_params("check", dir.path().to_str().unwrap());

        // The exact row spelling: another spelling is no draft for the row.
        let mut wrong_term = s19_math_draft();
        wrong_term["term"] = json!("均衡 ");
        s19_write_output(&compiled, "t001", &[wrong_term]);
        check_step(&p).unwrap();
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        assert_eq!(check["unexpected"], json!(["均衡 "]));
        assert_eq!(check["terms"][0]["status"], "missing");

        // A duplicated draft for one row is refused rather than silently taking one.
        s19_write_output(&compiled, "t001", &[s19_math_draft(), s19_math_draft()]);
        check_step(&p).unwrap();
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        assert_eq!(check["duplicates"], json!(["均衡"]));
        assert!(
            check["terms"][0]["schema_faults"]
                .as_array()
                .unwrap()
                .iter()
                .any(|fault| pstr(fault, "message").contains("written twice")),
            "{}",
            check["terms"][0]
        );

        // The automatic-link opt-out names its exact missing reason.
        let mut no_reason = s19_math_draft();
        no_reason["link"] = json!(false);
        no_reason["link_reason"] = json!("");
        s19_write_output(&compiled, "t001", &[no_reason]);
        check_step(&p).unwrap();
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        assert_eq!(
            check["terms"][0]["schema_faults"][0]["path"],
            "term.link_reason"
        );

        // The unbalanced delimiter reaches the check artifact as the T10 rule.
        let mut open = s19_math_draft();
        open["intuition"] = json!(["价格是 $5 元。", "均衡是相互一致的选择。"]);
        s19_write_output(&compiled, "t001", &[open]);
        check_step(&p).unwrap();
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        assert!(
            check["terms"][0]["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|diagnostic| diagnostic["rule"] == json!("T10")),
            "{}",
            check["terms"][0]["diagnostics"]
        );

        // A frozen prose bound the head's mandatory material cannot meet is a refused render, never
        // a truncation (per-Card bounds).
        let bound_dir = tempfile::tempdir().unwrap();
        let bounded = s19_compiled_with(bound_dir.path(), s19_two_rows(), 80);
        s19_write_output(&bounded, "t001", &[s19_math_draft()]);
        s19_write_output(&bounded, "t002", &[s19_concept_draft("剩余控制权", "均衡")]);
        let p = s19_params("check", bound_dir.path().to_str().unwrap());
        check_step(&p).unwrap();
        let check = s19_read(&bounded, &ps::join("check", "t001.json"));
        assert_eq!(check["terms"][0]["status"], "faulty");
        assert_eq!(check["terms"][0]["render_faults"][0]["path"], "render.head");
        assert_eq!(check["terms"][0]["draft_sha256"].is_string(), true);
    }

    #[test]
    fn a_check_retains_the_source_fidelity_markers_and_claims_no_unperformed_check() {
        let dir = tempfile::tempdir().unwrap();
        let compiled = s19_compiled(dir.path(), s19_two_rows());
        let mut draft = s19_math_draft();
        draft["derivations"][0]["steps"][1]["basis"] = json!("（补：书中略去）由 H1 代入。");
        s19_write_output(&compiled, "t001", &[draft]);
        s19_write_output(&compiled, "t002", &[s19_concept_draft("剩余控制权", "均衡")]);
        let p = s19_params("check", dir.path().to_str().unwrap());
        check_step(&p).unwrap();
        let check = s19_read(&compiled, &ps::join("check", "t001.json"));
        assert_eq!(check["terms"][0]["status"], "ok", "{}", check["terms"][0]);
        let supplements = check["terms"][0]["supplements"].as_array().unwrap();
        assert_eq!(supplements.len(), 1, "{supplements:?}");
        assert_eq!(supplements[0]["marker"], "（补：书中略去）");
        assert_eq!(supplements[0]["card"], "t001/r001");
        // The report never claims a numerical check or a proof that was not performed.
        assert_eq!(check["terms"][0]["verification"]["numerical_checks"], json!([]));
        assert!(
            pstr(&check["terms"][0]["verification"], "note").contains("no executed numerical check"),
            "{}",
            check["terms"][0]["verification"]
        );
        assert!(
            pstr(&check["terms"][0]["verification"], "proof").contains("not proof"),
            "{}",
            check["terms"][0]["verification"]
        );
        let readiness = s19_readiness(&compiled);
        assert_eq!(readiness["terms"][0]["supplements"][0]["marker"], "（补：书中略去）");
        // The rendered text itself keeps the marker verbatim: the review request embeds it.
        let review = s19_params("review-plan", dir.path().to_str().unwrap());
        review_plan_step(&review).unwrap();
        let request = std::fs::read_dir(compiled.request_dir())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let text = util::read_text(request).unwrap();
        assert!(text.contains("（补：书中略去）"), "{text}");
    }

    #[test]
    fn every_content_job_carries_its_managed_role_effort_and_skills() {
        // S20 (#164), R0326/R0333: the writer, the reviewer and the fix name the frozen official
        // model and high effort, and the Chinese language skill travels in the job's own field.
        let writer = managed_job(json!({"id": "t001", "kind": "math"}), &WRITER_SKILLS, false);
        assert_eq!(writer["model"], json!(JOB_MODEL));
        assert_eq!(writer["effort"], json!("high"));
        assert_eq!(writer["skills"], json!(["zettelkasten", "asd-ste100-zh"]));
        assert_eq!(writer["delete_after_collect"], json!(false));
        assert_eq!(writer["id"], json!("t001"), "the job's own fields stay: {writer}");
        let review = managed_job(json!({"id": "t001-r001-review"}), &REVIEWER_SKILLS, true);
        assert_eq!(review["model"], json!("deepseek-official/deepseek-flash"));
        assert_eq!(review["effort"], json!("high"));
        assert_eq!(review["skills"], json!(["asd-ste100-zh"]));
        assert_eq!(review["delete_after_collect"], json!(true));
        // The protocol's role is frozen: the same model and the same effort everywhere.
        assert_eq!(JOB_MODEL, "deepseek-official/deepseek-flash");
        assert_eq!(JOB_EFFORT, "high");
    }
}
