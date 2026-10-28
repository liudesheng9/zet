//! `zt-digest model` (model-compiler.ps1): the deterministic half of the
//! book digest's model compiler. Sonnet plans each model (plan.json);
//! DeepSeek writes briefs, Card bodies, charts, Derivation parts, and reviews;
//! this tool does everything between them without a model.
//!
//!   zt-digest model -Step brief|validate|apply|derive|review|split|close|report|resume
//!                         |explain|explain-check|explain-apply|explain-report
//!                   -Work <folder> [-Staging <folder>] [-Models a,b] [-Plan <file>] [-Need a=N;b=M]
//!                   [-Changed] [-Rebuild] [-IndependentReview] [-Level concept|formal]
//!                   [-MaxSentences 12] [-MaxChars 400]
//!                   [-MaxSteps 6] [-MaxExplainSentences 16] [-MaxExplainChars 600] [-MaxParts 0]
//!                   [-BriefDoc f] [-FillDoc f] [-ReviewDoc f] [-Job <id>] [-File <file>]
//!
//! One batch shares the work folder -Work: state.json and one folder per model
//! (<slug>: the address with / \ | replaced by _). Every step prints compact
//! JSON last, except validate and explain-check, which print fault lines and
//! `faults: N`. The steps, the files, and the job texts are the script's; see
//! docs/wiki/digest-scripts.md and docs/wiki/zt-digest.md.

use crate::ps::{self, CiMap, each, list, pipe, s, truthy};
use super::text::{self, derives, line_of, ref_of};
use super::{Scripts, context, get_card, get_model, read_calls};
use crate::args::Args;
use crate::calls;
use crate::check::Check;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

macro_rules! pattern {
    ($name:ident, $re:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($re).unwrap());
    };
}

pattern!(SLUG, r"[\\/|]");
pattern!(OTHER_REF, r"(?i)^(.+)#(.+)$");
pattern!(DIRECT_SLOT, r"(?i)^(.*[/|])(\d+)$");
pattern!(OK_LINE, r"(?i)^\d+ ok (.+)$");
pattern!(ERROR_LINE, r"(?i)^\d+ error (.*)$");
pattern!(SETTING_OR_PREMISES, r"(?i)设定|前提|premises");
pattern!(SOURCE_REF, r"(?i)^(.+?):(\d+)(?:-(\d+))?$");
pattern!(FIGURE_PREFIX, r"(?i)^\s*(图|Figure|Fig\.)\s*");
pattern!(TODO_BODY, r"(?i)^\s*待写\s*$");
pattern!(TODO_LINE, r"(?i)(^|\n)\s*待写\s*($|\n)");
pattern!(CHART, r"(?i)```chart");
pattern!(FAULTS_LINE, r"(?i)^faults: \d+");
pattern!(FAULTS_PREFIX, r"(?i)^faults: ");
pattern!(STYLE_FAULT, r"(?i)^(\S+) (R\d+ .*)$");
pattern!(REFUSED, r"(?i)^(\S+): ");
pattern!(LIST_ITEM, r"(?i)^\s*-");
pattern!(COMMA, r",");

const STEPS: [&str; 15] = [
    "brief", "validate", "plan-status", "apply", "derive", "review", "split", "close", "defer", "report", "resume", "explain", "explain-check",
    "explain-apply", "explain-report",
];
/// One model's folder and artifact stem: the address with `\`, `/` and `|` replaced by `_`.
/// Adoption reads the retained plans and states by the same name the compiler writes them.
pub(crate) fn slug_of(address: &str) -> String {
    SLUG.replace_all(address, "_").into_owned()
}
/// A premise that states a mathematical unit (a Model Card with a unit form) is cited, not taken in.
const UNIT_FORMS: [&str; 6] = ["definition", "axiom", "theorem", "proposition", "lemma", "corollary"];
/// The CRLF line end the ported PowerShell job texts kept.
const CR: &str = "\r\n";

struct Params {
    step: String,
    work: String,
    staging: String,
    models: String,
    plan: String,
    need: String,
    changed: bool,
    rebuild: bool,
    /// close only: when nothing goes back to a writer, the batch report runs in the same process.
    report: bool,
    /// review only: the reviewer is its own conversation (`<slug>:review`) instead of the model's.
    independent_review: bool,
    level: String,
    max_sentences: i64,
    max_chars: i64,
    max_steps: i64,
    max_explain_sentences: i64,
    max_explain_chars: i64,
    max_parts: i64,
    job: String,
    file: String,
}

struct Docs {
    brief: String,
    fill: String,
    review: String,
    explain: String,
}

struct Compiler {
    p: Params,
    zt: Option<Zt>,
    state: Value,
    state_file: String,
    /// `$Helper`, `$PSCommandPath`, `$Check`: the names job files carry.
    helper: String,
    check: String,
    docs: Docs,
    explain_args: String,
    limits: String,
    unit_cache: HashMap<String, bool>,
    figures: Option<CiMap<Value>>,
    /// What the accepted plan's source faults recorded, for the step's own report line.
    source_report: Option<Value>,
    /// plan.json per model folder, as Get-PlanPart reads it (the step never writes it).
    plans: HashMap<String, Value>,
}

/// One chain of Derivation parts the explain step plans a job for.
struct Group {
    parts: Vec<String>,
    chain: Vec<String>,
    tos: Vec<String>,
    after: Value,
    before: Value,
}

/// What explain-apply would do with one job's result file.
#[derive(Default)]
struct ExplainCheck {
    faults: CiMap<String>,
    candidates: Vec<Candidate>,
    blocked: Vec<String>,
    refused: Vec<String>,
    kept: Vec<String>,
}

struct Candidate {
    address: String,
    title: String,
    facet: String,
    body: String,
    old: String,
}

/// What a split hands to the conversation that continues.
struct Follow {
    model: String,
    job: Option<String>,
    text: String,
    ids: Vec<String>,
    /// The Cards a note is about when no job of this batch wrote them: the new follow-up job's
    /// allow list names them, so a Card of a model of this batch is never `unowned`.
    cards: Vec<String>,
}

/// Adds one follow-up to the conversation it belongs to (`new:<model>` when no job wrote it yet).
fn add_follow(follow: &mut CiMap<Follow>, r: Follow) {
    let key = r.job.clone().unwrap_or_else(|| format!("new:{}", r.model));
    let (text, mut ids, mut cards) = match follow.get(&key) {
        Some(was) => (was.text.clone(), was.ids.clone(), was.cards.clone()),
        None => (String::new(), Vec::new(), Vec::new()),
    };
    for i in r.ids {
        if !ids.contains(&i) {
            ids.push(i);
        }
    }
    for a in r.cards {
        if !cards.contains(&a) {
            cards.push(a);
        }
    }
    let entry = Follow { model: r.model, job: r.job, text: format!("{text}{}\n", r.text), ids, cards };
    follow.insert(&key, entry);
}

/// The 修正 notes for one model's build faults that a writer can repair in the Card it wrote:
/// (the Card the fault sits in, the note). A closure or forward fault is almost always a sentence
/// that says 依赖 in the wrong direction or about a Card outside the model (grossman2002integration
/// trial 6: a Model Card that "依赖" four Idea Cards of its group, and a 解读 line
/// "依赖这两个方向的 [[later part]] …" meaning the later part uses this one).
/// `-Step defer`: the models of a batch that `model-deps` released to wait leave the batch's order, so no
/// later step (apply, derive, review, split, close, report) writes or reports them, and their status is
/// `deferred`. Their plans are the caller's to set aside. Returns the models it deferred, in order.
fn defer_models(state: &mut Value, models: &[String]) -> Vec<String> {
    let order: Vec<String> = list(&state["order"]).into_iter().map(s).collect();
    let deferred: Vec<String> = order.iter().filter(|m| models.iter().any(|d| ps::eq(d, m))).cloned().collect();
    state["order"] = json!(order.iter().filter(|m| !deferred.contains(m)).collect::<Vec<_>>());
    for model in &deferred {
        state["models"][model]["status"] = json!("deferred");
    }
    deferred
}

fn close_entries(model: &str, build: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let keep = "不要增删其余内容，不要移动卡片。改完照常运行 CHECK。";
    for f in list(&build["closure"]) {
        let (from, to) = (s(&f["from"]), s(&f["to"]));
        out.push((from.clone(), format!(
            "## 修正：[[{from}]]\n\n构建检查（闭合）：这张卡有一句话写它依赖 [[{to}]]，而 [[{to}]] 不在本模型 [[{model}]] 里。原句：{}\n把这句改成不表示依赖的说法，句首用「参照」「对应」「对照」或「例示」，意思不变。这句话里不要留下任何读作依赖的词：依赖、依据、基于、建立在、取自、沿用、承接、用到、借用、代入、推导自。模型只依赖自己的分卡。{keep}\n",
            s(&f["note"])
        )));
    }
    for f in list(&build["forward"]) {
        let (from, to) = (s(&f["from"]), s(&f["to"]));
        out.push((from.clone(), format!(
            "## 修正：[[{from}]]\n\n构建检查（顺序）：这张卡有一句话以「依赖」指向构建顺序在后的 [[{to}]]，读作本卡依赖后面的卡。原句：{}\n若意思是后面的卡用到本卡，把句子改成「[[{to}]] 用到本卡的……」（链接放在句首）或「引出 [[{to}]]：……」。若本卡确实要用 [[{to}]] 的内容，改为依赖前面给出这项内容的那张分卡。{keep}\n",
            s(&f["note"])
        )));
    }
    for f in list(&build["reach_in"]) {
        let from = s(&f["from"]);
        out.push((from.clone(), format!(
            "## 修正：[[{from}]]\n\n构建检查（越界引用）：{}\n原句：{}\n按这个建议只改这句话。{keep}\n",
            s(&f["fix"]),
            s(&f["note"])
        )));
    }
    let setting = s(&build["setting"]);
    for f in list(&build["unanchored"]) {
        let at = s(&f["address"]);
        let anchor = if setting.is_empty() { String::new() } else { format!("，例如「依赖 [[{setting}]]：用到……」") };
        out.push((at.clone(), format!(
            "## 修正：[[{at}]]\n\n构建检查（未接上）：这张分卡没有依赖本模型里在它之前的任何分卡。{}\n加一句依赖它真正用到的前面那张分卡的话{anchor}，说明用到什么。{keep}\n",
            s(&f["fix"])
        )));
    }
    out
}

/// The 修正 notes for the supplied-step markers `model-finish` refuses: a marker is exactly
/// （补：书中未写） or （补：书中略去）; a writer that appends a note inside it (grossman2002integration
/// trial 7: 「（补：书中略去：两式相除）」) gets the Card back with the two allowed forms.
/// The planned Links a written part does not author: for each planned part this build wrote (not taken,
/// not existing), every `rests` and `of` reference that resolves to a part of this model must appear as a
/// relation from that part. `model-finish` refuses the model for each missing one (`does not author the
/// Link to … its accepted plan requires`), and before this nothing sent them back: on
/// grossman2002integration trial 10 close reported no fault and model-finish refused `7|a` because
/// `7|a|a|5` lacked its Link to `3|a|a|2`.
fn planned_link_entries(
    model: &str,
    plan_parts: &[Value],
    ids: &Value,
    models: &Value,
    resolve: &dyn Fn(&str) -> Option<String>,
    parts: &BTreeSet<String>,
    pairs: &BTreeSet<(String, String)>,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for part in plan_parts {
        let id = s(&part["id"]);
        if id.is_empty() || truthy(&part["take"]) || truthy(&part["existing"]) {
            continue;
        }
        let address = s(&ids[&id]);
        if address.is_empty() || address.contains('#') {
            continue;
        }
        let mut wanted: Vec<(String, String)> = Vec::new();
        for rest in list(&part["rests"]) {
            let reference = s(&ps::at(&pipe(rest), 0));
            if !reference.is_empty() {
                wanted.push((reference, s(&ps::at(&pipe(rest), 1))));
            }
        }
        if truthy(&part["of"]) {
            wanted.push((s(&part["of"]), String::new()));
        }
        for (reference, why) in wanted {
            let Some(target) = resolve(&reference) else { continue };
            if ps::eq(&target, model) || !parts.contains(&target) {
                continue;
            }
            let mut pieces = reference_pieces(ids, models, &reference);
            if pieces.is_empty() {
                pieces.push(target.clone());
            }
            if pieces.iter().any(|piece| pairs.contains(&(address.clone(), piece.clone()))) {
                continue;
            }
            let reason = if why.is_empty() { String::new() } else { format!("计划写的关系：{why}。") };
            let split = if pieces.len() > 1 {
                let named: Vec<String> = pieces.iter().map(|p| format!("[[{p}]]")).collect();
                format!("[[{target}]] 已拆成 {}，Link 到给出所用结果的那一张即可。", named.join("、"))
            } else {
                String::new()
            };
            out.push((address.clone(), format!(
                "## 修正：[[{address}]]\n\n构建检查（计划的关系）：模型 [[{model}]] 的计划要求这张分卡写出它与 [[{target}]] 的关系，正文里还没有这条 Link。{reason}{split}\n在用到 [[{target}]] 的那句话里放上这条 Link，关系词紧贴在 Link 前（例如「依赖 [[{target}]]：用到……」），说明用到什么。不要增删其余内容，不要移动卡片。改完照常运行 CHECK。\n"
            )));
        }
    }
    out
}

/// The addresses of a planned part and of the pieces `split` cut it into (`D11`, `D11~2`, `D11~3`, and a piece
/// split again, `D11~2~2`), in id order; empty for an id this model does not map (a taken `K/3|a#X`). A
/// planned relation to a split part is authored by a Link to any of its pieces: the last one holds the result.
pub(crate) fn split_pieces(ids: &Value, id: &str) -> Vec<String> {
    let prefix = format!("{id}~");
    let Some(map) = ids.as_object() else { return Vec::new() };
    let mut keys: Vec<&String> = map.keys().filter(|k| *k == id || k.starts_with(&prefix)).collect();
    keys.sort();
    keys.into_iter().map(|k| s(&map[k])).filter(|a| !a.is_empty() && !a.contains('#')).collect()
}

/// The pieces of a planned reference: an own id reads this model's map, `<model>#<id>` reads that model's map in
/// the compiler state (`models`); empty when neither maps it (a reference to an earlier batch keeps its one address).
pub(crate) fn reference_pieces(own: &Value, models: &Value, reference: &str) -> Vec<String> {
    match reference.split_once('#') {
        Some((other, part)) => split_pieces(&models[other]["ids"], part),
        None => split_pieces(own, reference),
    }
}

/// The text of one 结构 line: the plan's use or title with each semicolon written as a comma. The compiler
/// writes these lines itself, so a `；` the architect put into a part's use would be an R3 fault in a done Model
/// Card that the bounded correction may not reopen (grossman2002integration trial 10: `8|a`'s 共用 line).
fn structure_text(text: &str) -> String {
    text.replace('；', "，").replace(';', ",")
}

fn marker_entries(st: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut cards: Vec<(String, String)> = vec![(s(&st["head"]["address"]), s(&st["head"]["body"]))];
    for p in list(&st["parts"]) {
        cards.push((s(&p["address"]), s(&p["body"])));
    }
    for (at, body) in cards {
        if at.is_empty() || crate::model::source_review::markers_ok(&body) {
            continue;
        }
        out.push((at.clone(), format!(
            "## 修正：[[{at}]]\n\n构建检查（补出标记）：这张卡有一个以「（补」开头的标记写错了。标记只能原样写成「{}」或「{}」，括号里不加别的字；要说明的话写在标记前面的句子里。只改标记，不要增删其余内容，不要移动卡片。改完照常运行 CHECK。\n",
            crate::model::source_review::MARKERS[0],
            crate::model::source_review::MARKERS[1]
        )));
    }
    out
}

/// A Model Card's 结构 list with every 共用 line rebuilt, in order, from the taken parts' current lines;
/// `None` when the list does not carry exactly one 共用 line per taken part (it is then left as it is).
fn shared_lines(section: &str, wanted: &[String]) -> Option<String> {
    let have = section.lines().filter(|l| l.contains("（共用）")).count();
    if have != wanted.len() || wanted.is_empty() {
        return None;
    }
    let mut k = 0;
    let lines: Vec<String> = section
        .lines()
        .map(|l| {
            if l.contains("（共用）") {
                k += 1;
                wanted[k - 1].clone()
            } else {
                l.to_string()
            }
        })
        .collect();
    Some(lines.join("\n"))
}

/// The 修正 notes for the parts the source review records as `supplied` (a step the book does not
/// give) whose body carries neither established marker: `model-finish` refuses them (grossman2002integration
/// trial 7: 8|a part `D1`). `review` is the model's source-review artifact, `ids` the plan-id → address map.
fn supplied_entries(review: &Value, ids: &Value, st: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for part in list(&review["parts"]) {
        if !ps::eq(&s(&part["outcome"]), "supplied") {
            continue;
        }
        let at = s(&ids[s(&part["part"]).as_str()]);
        if at.is_empty() {
            continue;
        }
        let body = list(&st["parts"]).into_iter().find(|p| ps::eq(&s(&p["address"]), &at)).map(|p| s(&p["body"])).unwrap_or_default();
        if crate::model::source_review::supplied_marked(&body) {
            continue;
        }
        out.push((at.clone(), format!(
            "## 修正：[[{at}]]\n\n构建检查（补出标记）：来源复核把这张分卡记为书中没有、由本模型补出的步骤，但正文里没有补出标记。在补出的那一句后面加「{}」（书里没有写）或「{}」（书里略过了），标记原样写，括号里不加别的字。不要增删其余内容，不要移动卡片。改完照常运行 CHECK。\n",
            crate::model::source_review::MARKERS[0],
            crate::model::source_review::MARKERS[1]
        )));
    }
    out
}

/// One split or fix verdict, by model and plan id.
struct Action {
    model: String,
    id: String,
    verdict: String,
    parts: i64,
    cuts: Vec<Value>,
    reason: Value,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(
        argv,
        &[
            "Step",
            "Work",
            "Staging",
            "Models",
            "Plan",
            "Need",
            "Level",
            "MaxSentences",
            "MaxChars",
            "MaxSteps",
            "MaxExplainSentences",
            "MaxExplainChars",
            "MaxParts",
            "BriefDoc",
            "FillDoc",
            "ReviewDoc",
            "Job",
            "File",
        ],
        &["Changed", "Rebuild", "Report", "IndependentReview"],
    )?;
    let step = a.choice("Step", &STEPS)?;
    let work = a.required("Work")?;
    if work.is_empty() {
        bail!("Cannot bind argument to parameter 'Work' because it is an empty string.");
    }
    let p = Params {
        step,
        work,
        staging: a.str("Staging"),
        models: a.str("Models"),
        plan: a.str("Plan"),
        need: a.str("Need"),
        changed: a.flag("Changed"),
        report: a.flag("Report"),
        independent_review: a.flag("IndependentReview"),
        rebuild: a.flag("Rebuild"),
        level: a.str("Level"),
        max_sentences: a.int_or("MaxSentences", 12)?,
        max_chars: a.int_or("MaxChars", 400)?,
        max_steps: a.int_or("MaxSteps", 6)?,
        max_explain_sentences: a.int_or("MaxExplainSentences", 16)?,
        max_explain_chars: a.int_or("MaxExplainChars", 600)?,
        max_parts: a.int_or("MaxParts", 0)?,
        job: a.str("Job"),
        file: a.str("File"),
    };
    let scripts = Scripts::find();
    let or = |given: String, name: &str| if given.is_empty() { scripts.file(name) } else { given };
    let docs = Docs {
        brief: or(a.str("BriefDoc"), "dsh-brief-instructions.zh.md"),
        fill: or(a.str("FillDoc"), "dsh-fill-instructions.zh.md"),
        review: or(a.str("ReviewDoc"), "dsh-review-instructions.zh.md"),
        explain: scripts.file("dsh-explain-instructions.zh.md"),
    };
    let explain_args = format!("-MaxExplainSentences {} -MaxExplainChars {}", p.max_explain_sentences, p.max_explain_chars);
    // Writers check their own new Cards, so a Derivation part must have its 思路 and 解读 (R15).
    let limits = format!(
        "--max-sentences {} --max-chars {} --max-steps {} --max-explain-sentences {} --max-explain-chars {} --require-explain",
        p.max_sentences, p.max_chars, p.max_steps, p.max_explain_sentences, p.max_explain_chars
    );
    // R328 (§15.2): the steps that allocate a model job or write Model Cards into the book's
    // Literature tree stop before their first file or Card when that tree is locked by the owner.
    // A read-only step (validate, close, defer, report, explain-check, explain-report) and a step
    // without -Staging (no prepared book to name) never ask; the storage gate still protects them.
    let zt = if matches!(
        p.step.as_str(),
        "brief" | "apply" | "derive" | "review" | "split" | "resume" | "explain" | "explain-apply"
    ) {
        let key = crate::lockcheck::key_from_staging(&p.staging);
        let mut client = crate::zt::start()?;
        if let Some(refusal) = crate::lockcheck::preflight(&mut client, "model", &p.step, &key)? {
            return crate::lockcheck::stop("model", &refusal);
        }
        Some(client)
    } else {
        None
    };
    std::fs::create_dir_all(&p.work)?;
    let state_file = ps::join(&p.work, "state.json");
    let state = if ps::exists(&state_file) {
        util::read_json(&state_file)?
    } else {
        json!({ "order": [], "models": {}, "jobs": {}, "rereview": [], "reviews": [], "seq": 0 })
    };
    let mut c = Compiler {
        helper: scripts.helper.clone(),
        check: "zt-check".into(),
        p,
        zt,
        state,
        state_file,
        docs,
        explain_args,
        limits,
        unit_cache: HashMap::new(),
        figures: None,
        source_report: None,
        plans: HashMap::new(),
    };
    match c.p.step.as_str() {
        "brief" => c.step_brief(),
        "validate" => c.step_validate(),
        "apply" => c.step_apply(),
        "derive" => c.step_derive(),
        "review" => c.step_review(),
        "split" => c.step_split(),
        "close" => c.step_close(),
        "defer" => c.step_defer(),
        "plan-status" => c.step_plan_status(),
        "report" => c.step_report(),
        "resume" => c.step_resume(),
        "explain" => c.step_explain(),
        "explain-check" => c.step_explain_check(),
        "explain-apply" => c.step_explain_apply(),
        _ => c.step_explain_report(),
    }?;
    Ok(0)
}

/// `$items += $value` on a state list: the old list (nulls dropped) plus the value.
fn plus(old: &Value, more: impl IntoIterator<Item = Value>) -> Value {
    let mut out: Vec<Value> = list(old).into_iter().cloned().collect();
    out.extend(more);
    Value::Array(out)
}

fn strings(v: &Value) -> Vec<String> {
    pipe(v).into_iter().map(s).collect()
}

impl Compiler {
    // --- the archive ----------------------------------------------------------------------------

    fn zt(&mut self) -> Result<&mut Zt> {
        if self.zt.is_none() {
            self.zt = Some(crate::zt::start()?);
        }
        Ok(self.zt.as_mut().expect("started"))
    }

    fn reads(&mut self, calls: &[(&str, Value)]) -> Result<Vec<Value>> {
        read_calls(self.zt()?, calls)
    }

    fn card(&mut self, at: &Value) -> Result<Value> {
        get_card(self.zt()?, at)
    }

    /// A Card that the archive does not hold is `None`, not a transport failure.
    fn card_or_missing(&mut self, address: &str) -> Result<Option<Value>> {
        match self.card(&json!(address)) {
            Ok(card) if truthy(&card) => Ok(Some(card)),
            Ok(_) => Ok(None),
            Err(error) if format!("{error:#}").contains("does not exist") => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn model_of(&mut self, at: &Value) -> Result<Value> {
        get_model(self.zt()?, at)
    }

    /// `Invoke-Write`: writes through the helper with -Moves; every address the
    /// state holds follows each moved map. Returns the addresses of `N ok` lines.
    fn write(&mut self, calls: Vec<Value>) -> Result<Vec<String>> {
        let moves = ps::join(&self.p.work, "moves.tmp");
        let _ = std::fs::remove_file(&moves);
        let mut options = calls::Options::new();
        options.moves = moves.clone();
        let result = calls::run(self.zt()?, &calls, &mut options);
        if ps::exists(&moves) {
            for line in ps::read_lines(&moves)? {
                if !line.trim().is_empty() {
                    let entry: Value = serde_json::from_str(&line)?;
                    self.update_moved(&entry["moved"]);
                }
            }
        }
        let (done, failure) = result.map_err(|e| anyhow!("zt helper failed: {e:#}"))?;
        let mut addresses = Vec::new();
        for d in &done {
            if let Some(c) = OK_LINE.captures(&d.line) {
                addresses.push(c[1].trim().to_string());
            }
        }
        if let Some(failure) = failure {
            let lines = failure.lines();
            if let Some(c) = ERROR_LINE.captures(&lines[0]) {
                bail!("zt: {}", &c[1]);
            }
            bail!("zt helper failed: {}", lines.join(" | "));
        }
        Ok(addresses)
    }

    fn edits(&mut self, calls: Vec<Value>) -> Result<()> {
        if !calls.is_empty() {
            self.write(calls)?;
        }
        Ok(())
    }

    /// `Update-Moved`: part ids and job addresses follow a moved map.
    fn update_moved(&mut self, map: &Value) {
        let Some(map) = map.as_object().filter(|m| !m.is_empty()) else { return };
        if let Some(models) = self.state["models"].as_object_mut() {
            for m in models.values_mut() {
                if let Some(ids) = m["ids"].as_object_mut() {
                    for value in ids.values_mut() {
                        let a = s(value);
                        if !a.is_empty()
                            && let Some(to) = map.get(&a)
                        {
                            *value = to.clone();
                        }
                    }
                }
            }
        }
        if let Some(jobs) = self.state["jobs"].as_object_mut() {
            for job in jobs.values_mut() {
                let moved: Vec<Value> = pipe(&job["addresses"])
                    .into_iter()
                    .map(|a| match a.as_str().and_then(|t| map.get(t).filter(|_| truthy(a))) {
                        Some(to) => to.clone(),
                        None => a.clone(),
                    })
                    .collect();
                if job.is_object() {
                    job["addresses"] = Value::Array(moved);
                }
            }
        }
    }

    // --- state ----------------------------------------------------------------------------------

    fn save_state(&self) -> Result<()> {
        util::write_text(&self.state_file, &ps::json_pretty(&self.state))
    }

    fn out_last(&self, value: &Value) -> Result<()> {
        util::print_line(&ps::json_compact(value))
    }

    fn slug(address: &str) -> String {
        slug_of(address)
    }

    fn dir(&self, model: &str) -> String {
        ps::join(&self.p.work, &Self::slug(model))
    }

    fn m(&self, model: &str) -> &Value {
        &self.state["models"][model]
    }

    fn m_mut(&mut self, model: &str) -> &mut Value {
        if !self.state["models"].is_object() {
            self.state["models"] = json!({});
        }
        &mut self.state["models"][model]
    }

    fn order(&self) -> Vec<String> {
        list(&self.state["order"]).into_iter().map(s).collect()
    }

    /// The command evidence of one step: which command ran, when, and its exact facts.
    /// A completion verifier reads these facts instead of trusting an agent's prose.
    fn record_step(&mut self, model: &str, name: &str, facts: Value) {
        let mut facts = facts;
        facts["at"] = json!(crate::canon::utc_now_o());
        if !self.state["models"][model]["steps"].is_object() {
            self.state["models"][model]["steps"] = json!({});
        }
        self.state["models"][model]["steps"][name] = facts;
    }

    /// The SHA-256 of one file's actual bytes (the accepted plan's identity).
    fn file_hash(path: &str) -> Result<String> {
        Ok(crate::canon::sha256_hex(&std::fs::read(path)?))
    }

    /// The plan's parts, by id, from the model's own plan.json.
    fn plan(&mut self, model: &str) -> Result<Value> {
        let path = ps::join(&self.dir(model), "plan.json");
        if !self.plans.contains_key(&path) {
            let plan = util::read_json(&path)?;
            self.plans.insert(path.clone(), plan);
        }
        Ok(self.plans[&path].clone())
    }

    fn next_seq(&mut self) -> String {
        let n = ps::int(&self.state["seq"]).unwrap_or(0) + 1;
        self.state["seq"] = json!(n);
        ps::d2(n)
    }

    /// `$m.ids[$id]` as text.
    fn id_address(&self, model: &str, id: &str) -> String {
        s(&self.m(model)["ids"][id])
    }

    // --- Locations ------------------------------------------------------------------------------

    fn direct_slot(address: &str) -> Result<String> {
        if let Some(c) = DIRECT_SLOT.captures(address) {
            let n = ps::int(&Value::String(c[2].to_string()))?;
            return Ok(format!("{}{}", &c[1], n + 1));
        }
        Ok(format!("{address}|1"))
    }

    fn tree_addresses(&mut self, address: &str) -> Result<HashSet<String>> {
        let tree = address.split('/').next().unwrap_or_default().to_string();
        let answer = self.reads(&[("get_tree", json!({ "tree": tree }))])?;
        let cards = answer.first().map(|t| t["cards"].clone()).unwrap_or(Value::Null);
        Ok(list(&cards).iter().map(|c| s(&c["address"])).collect())
    }

    fn free_side(parent: &str, set: &HashSet<String>) -> Result<String> {
        for c in 'a'..='z' {
            let slot = format!("{parent}|{c}");
            if !set.contains(&slot) {
                return Ok(slot);
            }
        }
        bail!("no free side slot under {parent}")
    }

    fn has_successors(address: &str, set: &HashSet<String>) -> Result<bool> {
        if set.contains(&Self::direct_slot(address)?) {
            return Ok(true);
        }
        Ok(set.iter().any(|a| a.starts_with(&format!("{address}|"))))
    }

    // --- plans ----------------------------------------------------------------------------------

    /// `Resolve-Ref`: a plan id, `<other model>#<id>`, or an address.
    fn resolve(&self, r: &Value, model: &str) -> Result<String> {
        let r = s(r);
        if let Some(c) = OTHER_REF.captures(&r) {
            let other = &self.state["models"][&c[1]];
            if truthy(other) && other["ids"].as_object().is_some_and(|ids| ids.contains_key(&c[2])) {
                return Ok(s(&other["ids"][&c[2]]));
            }
            bail!("reference {r}: model {} has no part {} yet", &c[1], &c[2]);
        }
        let m = self.m(model);
        if m["ids"].as_object().is_some_and(|ids| ids.contains_key(&r)) {
            let value = s(&m["ids"][&r]);
            // a take of a part that another model of the batch planned
            if value.contains('#') {
                return self.resolve(&Value::String(value), model);
            }
            return Ok(value);
        }
        Ok(r)
    }

    fn unit(&mut self, address: &str) -> bool {
        if let Some(known) = self.unit_cache.get(address) {
            return *known;
        }
        let card = self.card(&json!(address)).unwrap_or(Value::Null);
        let is = truthy(&card)
            && ps::eq(&s(&card["type"]), "model")
            && !truthy(&card["facet"])
            && ps::is_in(&s(&card["form"]), &UNIT_FORMS);
        self.unit_cache.insert(address.to_string(), is);
        is
    }

    /// `Get-FigureLine`: the Markdown of a book figure by its label, from figures.json.
    fn figure_line(&mut self, label: &str) -> Result<Value> {
        if self.figures.is_none() {
            let mut lines = CiMap::new();
            // Without -Staging (the architect's validate), the nearest folder above -Work that holds figures.json.
            let mut file = if self.p.staging.is_empty() { String::new() } else { ps::join(&self.p.staging, "figures.json") };
            if self.p.staging.is_empty() {
                let mut dir = ps::parent(&self.p.work);
                while !dir.is_empty() && !ps::exists(&ps::join(&dir, "figures.json")) {
                    dir = ps::parent(&dir);
                }
                if !dir.is_empty() {
                    file = ps::join(&dir, "figures.json");
                }
            }
            if !file.is_empty() && ps::exists(&file) {
                let figures = util::read_json(&file)?;
                for figure in each(&figures["figures"]) {
                    for l in pipe(&figure["labels"]) {
                        if !lines.contains(&s(l)) {
                            lines.insert(&s(l), figure["line"].clone());
                        }
                    }
                }
            }
            self.figures = Some(lines);
        }
        let key = FIGURE_PREFIX.replace(label, "").trim().to_string();
        Ok(self.figures.as_ref().and_then(|f| f.get(&key)).cloned().unwrap_or(Value::Null))
    }

    /// `Test-Plan`: one fault per line; none for a plan the apply step can build.
    fn test_plan(&mut self, plan: &Value, structure: &Value, model: &str) -> Result<Vec<String>> {
        let mut faults = Vec::new();
        if !ps::eq(&s(&plan["model"]), model) || plan["model"].is_null() {
            faults.push(format!("plan.model is '{}', expected '{model}'", s(&plan["model"])));
        }
        if ps::eq(&s(&plan["status"]), "skip") && !plan["status"].is_null() {
            return Ok(faults);
        }
        let level = s(&plan["level"]);
        let formal = ps::eq(&level, "formal") && !plan["level"].is_null();
        let concept = ps::eq(&level, "concept") && !plan["level"].is_null();
        if !formal && !concept {
            faults.push("level must be concept or formal".into());
        }
        // The Model Card job and a concept model's setting.md carry nothing else about the model.
        if s(&plan["card"]).trim().is_empty() {
            faults.push("card is missing: what the Model Card must say (the question, the summary, the outside links to keep)".into());
        }
        let parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
        let mut ids: CiMap<usize> = CiMap::new();
        let members: HashSet<String> = list(&structure["parts"]).iter().map(|p| s(&p["address"])).collect();
        for (i, p) in parts.iter().enumerate() {
            let index = i + 1;
            if !truthy(&p["id"]) {
                faults.push(format!("part {index} has no id"));
                continue;
            }
            let id = s(&p["id"]);
            if ids.contains(&id) {
                faults.push(format!("id {id} is used twice"));
            }
            ids.insert(&id, index);
            for label in pipe(&p["figure"]).into_iter().filter(|l| truthy(l)) {
                if !truthy(&self.figure_line(&s(label))?) {
                    faults.push(format!(
                        "part {id}: figure '{}' is not a book figure ZET holds (figures.json lists them); draw it with a chart instead",
                        s(label)
                    ));
                }
            }
            if ["existing", "take", "move"].iter().filter(|k| truthy(&p[**k])).count() > 1 {
                faults.push(format!("{id}: give only one of existing, take, move"));
            }
            let line = line_of(p);
            if !ps::is_in(&line, &["main", "side", "off", "shared"]) {
                faults.push(format!("{id}: line must be main, side, or off"));
            }
            if ps::eq(&line, "side") && !truthy(&p["of"]) {
                faults.push(format!("{id}: a side part needs of (the part it belongs beside)"));
            }
            if ref_of(p) == "new" {
                if !truthy(&p["facet"]) {
                    faults.push(format!("{id}: a new part needs a facet"));
                } else if ps::len16(&s(&p["facet"])) > 32 {
                    faults.push(format!("{id}: facet is longer than 32 characters"));
                }
                if !truthy(&p["title"]) {
                    faults.push(format!("{id}: a new part needs a title"));
                }
                if !truthy(&p["spec"]) && !derives(p) {
                    faults.push(format!("{id}: a new part needs spec (what it must state)"));
                }
            }
            if derives(p) {
                if concept {
                    faults.push(format!("{id}: a concept model has no Derivation parts"));
                }
                if !ps::eq(&line, "main") {
                    faults.push(format!("{id}: a Derivation part stands on the main line"));
                }
                if truthy(&p["facet"]) && !ps::starts_with(&s(&p["facet"]), "推导") {
                    faults.push(format!("{id}: a Derivation part's facet starts with 推导"));
                }
                if !truthy(&p["to"]) {
                    faults.push(format!("{id}: a Derivation part needs to (its result equation)"));
                }
                if truthy(&p["steps"]) && ps::gt(&p["steps"], self.p.max_steps) {
                    faults.push(format!(
                        "{id}: {} steps is over MaxSteps {}; make it a chain of Derivation parts",
                        s(&p["steps"]),
                        self.p.max_steps
                    ));
                }
            }
            if truthy(&p["existing"]) && !members.contains(&s(&p["existing"])) {
                faults.push(format!("{id}: existing {} is not a part of this model", s(&p["existing"])));
            }
        }
        let main: Vec<&Value> = parts.iter().filter(|p| ps::eq(&line_of(p), "main")).collect();
        for k in 0..main.len() {
            let p = main[k];
            if derives(p) && !truthy(&p["from"]) && list(&p["rests"]).is_empty() && !(k > 0 && derives(main[k - 1])) {
                faults.push(format!("{}: the first Derivation part of a chain needs from or rests (its starting point)", s(&p["id"])));
            }
        }
        if formal {
            if main.is_empty() {
                faults.push("a formal model needs a main line".into());
            } else if !SETTING_OR_PREMISES.is_match(&s(&main[0]["facet"])) && !truthy(&main[0]["existing"]) {
                faults.push("the first main-line part is the setting (facet 设定; a mathematical unit: 前提)".into());
            }
            if list(&plan["symbols"]).is_empty() {
                faults.push("a formal model needs symbols (the setting's symbol table)".into());
            }
            if list(&plan["terms"]).is_empty() {
                faults.push("a formal model needs terms (the setting's term table)".into());
            }
        }
        if concept && parts.iter().filter(|p| !truthy(&p["take"])).count() > 4 {
            faults.push("a concept model has at most four parts".into());
        }
        // premises: earlier in the build order, and members
        let mut position: CiMap<usize> = CiMap::new();
        for (k, p) in main.iter().enumerate() {
            if p["id"].is_null() {
                bail!("Index operation failed; the array index evaluated to null.");
            }
            position.insert(&s(&p["id"]), k);
        }
        for p in &parts {
            let id = s(&p["id"]);
            if truthy(&p["of"]) && !ids.contains(&s(&p["of"])) && !members.contains(&s(&p["of"])) {
                faults.push(format!("{id}: of '{}' is not a part of this plan", s(&p["of"])));
            }
            for r in list(&p["rests"]) {
                let reference = s(&ps::at(&pipe(r), 0));
                if reference.is_empty() {
                    continue;
                }
                if ids.contains(&reference) {
                    if let (Some(at), Some(of)) = (position.get(&reference), position.get(&id))
                        && at >= of
                    {
                        faults.push(format!("forward: {id} rests on {reference}, which comes later in the build order"));
                    }
                } else if reference.contains('#') {
                    // R233: the reference is a producer dependency, not a source-order preference.
                    // It resolves after `model-deps` registers the producer and its checked part;
                    // an earlier batch's model is as valid as one planned in this batch.
                    if self.resolve(&Value::String(reference.clone()), model).is_err() {
                        faults.push(format!(
                            "{id}: {reference} names no checked part; register the producer dependency with model-deps before fill"
                        ));
                    }
                } else if !members.contains(&reference) && !ps::eq(&reference, model) && !self.unit(&reference) {
                    faults.push(format!(
                        "closure: {id} rests on {reference}, which is outside the model; take it in, move it in, or restate it as a new part"
                    ));
                }
            }
        }
        // every filed part is accounted for, and kept main-line parts keep their order
        let mut listed: HashSet<String> = HashSet::new();
        for p in &parts {
            if truthy(&p["existing"]) {
                listed.insert(s(&p["existing"]));
            }
        }
        for m in list(&plan["merge"]) {
            if truthy(&m["from"]) {
                listed.insert(s(&m["from"]));
            }
        }
        for p in pipe(&structure["parts"]).into_iter().filter(|p| !ps::is_false(&p["filed"])) {
            if !listed.contains(&s(&p["address"])) {
                faults.push(format!(
                    "existing part {} ({}) is not in the plan: list it with existing, or merge it",
                    s(&p["address"]),
                    s(&p["facet"])
                ));
            }
        }
        let current_main: Vec<String> =
            pipe(&structure["build"]["order"]).into_iter().filter(|o| ps::eq(&s(&o["line"]), "main")).map(|o| s(&o["address"])).collect();
        let kept_main: Vec<String> = main.iter().filter(|p| truthy(&p["existing"])).map(|p| s(&p["existing"])).collect();
        let in_order: Vec<String> = current_main.iter().filter(|a| ps::is_in(a, &kept_main)).cloned().collect();
        if !ps::eq(&in_order.join(","), &kept_main.join(",")) {
            faults.push(format!("kept main-line parts must stay in their order: {}", in_order.join(", ")));
        }
        if lacks_main_line(structure, &current_main, &main) {
            faults.push("the model has parts but no main line: list the part that should become the setting as existing, line main".into());
        }
        for m in list(&plan["merge"]) {
            if !ids.contains(&s(&m["into"])) && !members.contains(&s(&m["into"])) {
                faults.push(format!("merge into '{}' is not a part", s(&m["into"])));
            }
        }
        Ok(faults)
    }

    /// `Get-PlanPart`: a part of the model's plan.json, or a part a split added.
    fn plan_part(&mut self, model: &str, id: &str) -> Result<Option<Value>> {
        let path = ps::join(&self.dir(model), "plan.json");
        if !self.plans.contains_key(&path) {
            let plan = util::read_json(&path)?;
            self.plans.insert(path.clone(), plan);
        }
        if let Some(p) = list(&self.plans[&path]["parts"]).into_iter().find(|p| !p["id"].is_null() && ps::eq(&s(&p["id"]), id)) {
            return Ok(Some(p.clone()));
        }
        // a part a split added: it continues its origin
        let added = &self.m(model)["added"];
        if truthy(added) && !added[id].is_null() {
            return Ok(Some(added[id].clone()));
        }
        Ok(None)
    }

    // --- skeleton bodies ------------------------------------------------------------------------

    /// `Format-Link`: `<word> [[target]]：why。`
    fn link(&self, r: &Value, model: &str, default: &str) -> Result<String> {
        let items = pipe(r);
        let target = self.resolve(&ps::at(&items, 0), model)?;
        let why = if items.len() > 1 { s(items[1]) } else { String::new() };
        let kind = if items.len() > 2 && truthy(items[2]) { s(items[2]) } else { default.to_string() };
        let word = text::link_word(&kind);
        let why = if why.is_empty() { String::new() } else { format!("：{}", why.trim_end_matches('。')) };
        Ok(format!("{word} [[{target}]]{why}。"))
    }

    /// Plan id -> (k, n, tos): the runs of neighbouring Derivation parts on the plan's main line.
    fn plan_chains(plan: &Value) -> CiMap<(usize, usize, Vec<String>)> {
        let mut out = CiMap::new();
        let mut run: Vec<&Value> = Vec::new();
        let main: Vec<&Value> = list(&plan["parts"]).into_iter().filter(|p| ps::eq(&line_of(p), "main")).collect();
        for p in main.into_iter().map(Some).chain(std::iter::once(None)) {
            if let Some(p) = p.filter(|p| derives(p)) {
                run.push(p);
                continue;
            }
            let tos: Vec<String> = run.iter().map(|r| s(&r["to"])).collect();
            for (i, r) in run.iter().enumerate() {
                out.insert(&s(&r["id"]), (i + 1, run.len(), tos.clone()));
            }
            run.clear();
        }
        out
    }

    /// `Get-Skeleton`: the body a new part starts with.
    fn skeleton(&self, part: &Value, plan: &Value, model: &str, previous: Option<&Value>) -> Result<String> {
        let mut links = Vec::new();
        for r in list(&part["rests"]).into_iter().filter(|r| truthy(&ps::at(&pipe(r), 0))) {
            links.push(self.link(r, model, "rests")?);
        }
        if truthy(&part["of"]) {
            let kind = if truthy(&part["kind"]) { s(&part["kind"]) } else { "illustrates".into() };
            links.insert(0, self.link(&json!([part["of"], part["why"], kind]), model, "rests")?);
        }
        if derives(part) {
            let mut start: Vec<String> = links.iter().map(|l| format!("- {l}")).collect();
            if let Some(prev) = previous.filter(|p| derives(p)) {
                start.insert(0, format!("- 依赖上一步 [[{}]] 的结果：{}。", self.resolve(&prev["id"], model)?, s(&prev["to"])));
            }
            if truthy(&part["from"]) {
                start.push(format!("- 从 {} 出发。", s(&part["from"])));
            }
            let chains = Self::plan_chains(plan);
            let position = chains.get(&s(&part["id"])).map(|(k, n, tos)| text::chain_line(*k, *n, tos)).unwrap_or_default();
            let idea = [position.as_str(), "待写"].iter().filter(|x| !x.is_empty()).copied().collect::<Vec<_>>().join("\n");
            return Ok(format!(
                "**思路**\n{idea}\n\n**起点**\n{}\n\n**步骤**\n待写\n\n**结果**\n{}\n\n**解读**\n待写",
                start.join("\n"),
                s(&part["to"])
            ));
        }
        let mut body = links;
        body.push(String::new());
        body.push("待写".into());
        let mut body = body.join("\n");
        if ps::eq(&s(&plan["level"]), "formal") && text::is_setting_facet(&s(&part["facet"])) && ps::eq(&line_of(part), "main") {
            body += &format!("\n\n{}", text::tables(plan));
        }
        Ok(body.trim().to_string())
    }

    /// `Get-StructureList`: the Model Card's 结构 list, one line per member.
    fn structure_list(&self, plan: &Value, model: &str) -> Result<String> {
        let mut lines = Vec::new();
        for p in list(&plan["parts"]) {
            let address = self.resolve(&p["id"], model)?;
            let m = self.m(model);
            if truthy(&p["take"]) {
                let facet = if truthy(&p["facet"]) { s(&p["facet"]) } else { "共用".into() };
                lines.push(format!("- {facet} [[{address}]]（共用）：{}", structure_text(&s(&p["use"]))));
            } else {
                let id = s(&p["id"]);
                let core = if ps::eq(&s(&p["emphasis"]), "core") { "（核心）" } else { "" };
                lines.push(format!("- {} [[{address}]]{core}：{}", s(&m["facets"][&id]), structure_text(&s(&m["titles"][&id]))));
            }
        }
        Ok(format!("**结构**\n\n{}", lines.join("\n")))
    }

    /// `Update-ChainLines`: the position lines of every chain of a model's own parts.
    fn update_chain_lines(&mut self, model: &str) -> Result<()> {
        let st = self.model_of(&json!(model))?;
        let mut bodies: CiMap<String> = CiMap::new();
        for p in list(&st["parts"]) {
            bodies.insert(&s(&p["address"]), s(&p["body"]));
        }
        let shared: Vec<String> =
            pipe(&st["build"]["order"]).into_iter().filter(|o| ps::eq(&s(&o["line"]), "shared")).map(|o| s(&o["address"])).collect();
        let mut edits = Vec::new();
        for c in list(&st["build"]["chains"]) {
            let parts: Vec<String> = pipe(&c["parts"]).into_iter().map(s).filter(|a| !ps::is_in(a, &shared)).collect();
            let body = |a: &str| bodies.get(a).cloned().unwrap_or_default();
            let tos: Vec<String> = parts.iter().map(|a| text::result_of(&body(a))).collect();
            for k in 1..=parts.len() {
                let old = body(&parts[k - 1]);
                let new = text::set_chain_line(&old, &text::chain_line(k, parts.len(), &tos));
                if !ps::eq(&new, old.trim()) {
                    edits.push(json!({ "tool": "edit_card", "arguments": { "at": parts[k - 1], "body": new } }));
                }
            }
        }
        self.edits(edits)
    }

    // --- job files ------------------------------------------------------------------------------

    /// `Get-SourceLines`: the book lines a part comes from, 12000 characters at most.
    fn source_lines(&self, refs: &[&Value]) -> Result<String> {
        let mut out = String::new();
        let mut budget: i64 = 12000;
        for r in refs {
            let r = s(r);
            let Some(c) = SOURCE_REF.captures(&r) else { continue };
            let file = c[1].to_string();
            let from = ps::int(&Value::String(c[2].to_string()))?;
            let to = match c.get(3) {
                Some(t) if !t.as_str().is_empty() => ps::int(&Value::String(t.as_str().to_string()))?,
                _ => from,
            };
            let path = if ps::rooted(&file) {
                file
            } else {
                if self.p.staging.is_empty() {
                    bail!("Cannot bind argument to parameter 'Path' because it is an empty string.");
                }
                ps::join(&self.p.staging, &file)
            };
            if !ps::exists(&path) {
                out.push_str(&format!("（找不到 {path}）\r\n"));
                continue;
            }
            if from - 1 < 0 {
                bail!("Cannot validate argument on parameter 'Skip'. The {} argument is less than the minimum allowed range of 0.", from - 1);
            }
            if to - from + 1 < 0 {
                bail!("Cannot validate argument on parameter 'First'. The {} argument is less than the minimum allowed range of 0.", to - from + 1);
            }
            let lines = ps::read_lines(&path)?;
            let mut text = lines.iter().skip((from - 1) as usize).take((to - from + 1) as usize).cloned().collect::<Vec<_>>().join("\n");
            if ps::len16(&text) as i64 > budget {
                text = ps::cut16(&text, budget.max(0) as usize) + "……";
            }
            budget -= ps::len16(&text) as i64;
            out.push_str(&format!("`{path}` 第 {from}–{to} 行：\r\n```text\r\n{text}\r\n```\r\n"));
            if budget <= 0 {
                break;
            }
        }
        Ok(out)
    }

    /// `New-Job`: a job file with its HEADER lines and its allow file.
    fn new_job(&mut self, model: &str, round: &str, ids: Vec<Value>, text: &str, prompt: &str) -> Result<Value> {
        let seq = self.next_seq();
        let id = format!("{}:{round}{seq}", Self::slug(model));
        let dir = self.dir(model);
        let file = ps::join(&dir, &format!("job-{round}{seq}.md"));
        let allow = ps::change_extension(&file, ".allow.txt");
        let addresses = ids.iter().map(|i| self.resolve(i, model)).collect::<Result<Vec<_>>>()?;
        util::write_text(&allow, &(addresses.join("\n") + "\n"))?;
        let kind = if ps::eq(&s(&self.m(model)["level"]), "concept") { "概念模型" } else { "形式模型" };
        let head = format!(
            "# 任务 {id}\n\nHELPER: {}\nCHECK: {}\nALLOW: {allow}\nLIMITS: {}\n模型卡: [[{model}]]（{kind}）\n\n",
            self.helper, self.check, self.limits
        );
        util::write_text(&file, &(head + text))?;
        if !self.state["jobs"].is_object() {
            self.state["jobs"] = json!({});
        }
        self.state["jobs"][&id] = json!({ "model": model, "round": round, "ids": ids, "addresses": addresses, "allow": allow, "file": file });
        Ok(json!({ "id": id, "files": [self.docs.fill, ps::join(&dir, "setting.md"), file], "prompt": prompt }))
    }

    /// `Get-Premises`: the premise lines, with their bodies for a Derivation job.
    fn premises(&mut self, part: &Value, model: &str, bodies: bool) -> Result<String> {
        let mut lines = Vec::new();
        for r in list(&part["rests"]) {
            let items = pipe(r);
            if !truthy(&ps::at(&items, 0)) {
                continue;
            }
            let address = self.resolve(&ps::at(&items, 0), model)?;
            let why = if items.len() > 1 { s(items[1]) } else { String::new() };
            lines.push(format!("- [[{address}]]：{why}"));
        }
        let rests = list(&part["rests"]).into_iter().cloned().collect::<Vec<_>>();
        if !bodies || rests.is_empty() {
            return Ok(lines.join("\n"));
        }
        let mut calls = Vec::new();
        for r in &rests {
            calls.push(("get_card", json!({ "at": self.resolve(&ps::at(&pipe(r), 0), model)? })));
        }
        let cards = self.reads(&calls)?;
        let mut text = lines.join("\n") + "\n\n前提卡的正文（已写好，供你引用其中的式子）：\n";
        for c in cards {
            let mut b = s(&c["body"]);
            if ps::len16(&b) > 1500 {
                b = ps::cut16(&b, 1500) + "……";
            }
            text += &format!("\n#### [[{}]] {}\n{b}\n", s(&c["address"]), s(&c["title"]));
        }
        Ok(text)
    }

    /// `Get-PartBlock`: one part's block in a fill or Derivation job.
    fn part_block(&mut self, n: usize, part: &Value, model: &str, bodies: bool) -> Result<String> {
        let address = self.resolve(&part["id"], model)?;
        let mut b = String::new();
        let line = |b: &mut String, text: &str| {
            b.push_str(text);
            b.push_str("\r\n");
        };
        line(&mut b, &format!("## 分卡 {n}：[[{address}]]"));
        line(&mut b, "");
        let emphasis = if ps::eq(&s(&part["emphasis"]), "core") {
            "（核心）"
        } else if ps::eq(&s(&part["emphasis"]), "aside") {
            "（旁注）"
        } else {
            ""
        };
        line(&mut b, &format!("- 分面：{}{emphasis}", s(&part["facet"])));
        line(&mut b, &format!("- 标题：{}", s(&part["title"])));
        if truthy(&part["spec"]) {
            line(&mut b, &format!("- 要陈述的内容：{}", s(&part["spec"])));
        }
        if derives(part) {
            let steps = if truthy(&part["steps"]) { format!("，约 {} 步", s(&part["steps"])) } else { String::new() };
            line(&mut b, &format!("- 推导：从起点写到结果 {}{steps}。骨架里已写好起点和结果。", s(&part["to"])));
            line(&mut b, "- 解释：写思路、步骤下的说明和解读（见“讲清推导”）。思路的位置行原样保留。");
            // A chain member starts from the result of the part before it, which may sit in another job.
            let main = strings(&Value::Array(list(&self.m(model)["main"]).into_iter().cloned().collect()));
            let k = ps::index_of(&main, &s(&part["id"]));
            let before = if k > 0 { self.plan_part(model, &main[(k - 1) as usize])? } else { None };
            if let Some(before) = before.filter(|b| derives(b) && truthy(&b["to"])) {
                line(&mut b, &format!("- 起点：上一步 [[{}]] 的结果 {}", self.resolve(&before["id"], model)?, s(&before["to"])));
            }
            if truthy(&part["from"]) {
                line(&mut b, &format!("- 从 {} 出发。", s(&part["from"])));
            }
        }
        let premises = self.premises(part, model, bodies)?;
        if !premises.is_empty() {
            line(&mut b, "- 前提：");
            line(&mut b, &premises);
        }
        if truthy(&part["of"]) {
            let kind = if truthy(&part["kind"]) { s(&part["kind"]) } else { "illustrates".into() };
            line(&mut b, &format!("- 它站在 [[{}]] 旁边（{kind}）。", self.resolve(&part["of"], model)?));
        }
        if truthy(&part["chart"]) {
            line(&mut b, &format!("- 图表：{}", s(&part["chart"])));
        }
        for label in pipe(&part["figure"]).into_iter().filter(|l| truthy(l)) {
            let figure = self.figure_line(&s(label))?;
            if truthy(&figure) {
                line(&mut b, &format!("- 书中的原图（把这一行原样放进正文，单独成段）：`{}`", s(&figure)));
            } else {
                line(&mut b, &format!("- 书中的图 {}：ZET 没有它的原图，按原文用图表画出。", s(label)));
            }
        }
        let old = &self.m(model)["old"];
        let was = if truthy(old) { old[&s(&part["id"])].clone() } else { Value::Null };
        if truthy(&was) {
            let mut was = s(&was);
            if ps::len16(&was) > 6000 {
                was = ps::cut16(&was, 6000) + "……";
            }
            line(&mut b, "");
            line(&mut b, "### 本卡原来的正文（按新骨架和要陈述的内容改写，可用的式子、例子和图表留用）");
            line(&mut b, &was);
        }
        if truthy(&part["old"]) {
            line(&mut b, "");
            line(&mut b, "### 已有内容（并入本卡，按新骨架改写）");
            line(&mut b, &s(&part["old"]));
        }
        let src = self.source_lines(&list(&part["src"]))?;
        if !src.is_empty() {
            line(&mut b, "");
            line(&mut b, "### 原文");
            line(&mut b, &src);
        }
        Ok(b)
    }

    /// Runs model-context in-process (the script ran model-context.ps1 as a child).
    fn context(&mut self, model: &str, out: &str) -> Result<()> {
        let staging = self.p.staging.clone();
        context::build(self.zt()?, &staging, model, out).map(|_| ())
    }

    fn model_list(&self) -> Vec<String> {
        COMMA.split(&self.p.models).map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).collect()
    }

    // --- steps ----------------------------------------------------------------------------------

    /// R247: the accepted plan's checked source review, recorded beside the plan. It names the
    /// exact source spans the architect reviewed per part and refuses a written part it cannot
    /// account for, so the whole-book Card cycle can report coverage without a second review.
    fn source_review(&mut self, plan: &Value, model: &str, plan_path: &str) -> (Value, Vec<String>) {
        let plan_hash = Self::file_hash(plan_path).unwrap_or_default();
        let brief = self.m(model)["steps"]["brief"].clone();
        let dir = self.dir(model);
        let staging = self.run_folder();
        let review = crate::model::source_review::Review {
            staging: &staging,
            dir: &dir,
            model,
            plan_path,
            plan_hash: &plan_hash,
            context: &s(&brief["context"]),
            context_hash: &s(&brief["context_hash"]),
        };
        crate::model::source_review::record(&review, plan)
    }

    fn step_brief(&mut self) -> Result<()> {
        let mut jobs = Vec::new();
        let mut skipped = Vec::new();
        for model in self.model_list() {
            let st = self.model_of(&json!(model))?;
            // An unwritten placeholder keeps a model unbuilt even when it has many parts (R235).
            let todo = TODO_LINE.is_match(&s(&st["head"]["body"]))
                || pipe(&st["parts"]).into_iter().any(|p| TODO_LINE.is_match(&s(&p["body"])));
            let built = !list(&st["parts"]).is_empty()
                && truthy(&st["build"]["self_sufficient"])
                && list(&st["build"]["beyond_level"]).is_empty()
                && !todo;
            let same_level = self.p.level.is_empty() || (!st["level"].is_null() && ps::eq(&s(&st["level"]), &self.p.level));
            if built && !self.p.rebuild && same_level {
                skipped.push(model);
                continue;
            }
            let dir = self.dir(&model);
            let order = self.order();
            if !ps::is_in(&model, &order) {
                self.state["order"] = plus(&self.state["order"], [json!(model)]);
            }
            *self.m_mut(&model) =
                json!({ "level": st["level"], "status": "brief", "ids": {}, "titles": {}, "derive": [], "main": [], "split": 0, "steps": {} });
            let context = ps::join(&dir, "context.md");
            if self.context(&model, &context).is_err() {
                bail!("model-context.ps1 failed for {model}");
            }
            let context_hash = Self::file_hash(&context)?;
            let read_only = ps::join(&dir, "read-only.allow.txt");
            util::write_text(&read_only, "")?;
            let brief = ps::join(&dir, "brief.md");
            let job = ps::join(&dir, "brief-job.md");
            let level = if truthy(&st["level"]) { s(&st["level"]) } else { "未定".into() };
            let asked = if self.p.level.is_empty() { String::new() } else { format!("\n本次指定层级: {}", self.p.level) };
            util::write_text(
                &job,
                &format!(
                    "# 简报任务：[[{model}]]\n\nHELPER: {}（只读：每条命令末尾加 -Allow \"{read_only}\"）\n上下文包: {context}\n简报文件: {brief}\n现在的层级: {level}{asked}\n分卡数: {}\n",
                    self.helper,
                    list(&st["parts"]).len()
                ),
            )?;
            jobs.push(json!({
                "id": format!("{}:brief", Self::slug(&model)),
                "files": [self.docs.brief, job],
                "prompt": format!("按以上说明写出简报文件 {brief}，最后只回复一行 JSON。"),
            }));
            // The source review is part of the brief: its evidence is the exact context pack,
            // which carries the selected source sections, and the instruction file the writer read.
            self.record_step(
                &model,
                "brief",
                json!({"context":context,"context_hash":context_hash,"job":job,"job_hash":Self::file_hash(&job)?,"level":st["level"]}),
            );
        }
        self.save_state()?;
        self.out_last(&json!({ "jobs": jobs, "skipped": skipped }))
    }

    fn step_validate(&mut self) -> Result<()> {
        let plan = util::read_json(&self.p.plan)?;
        let st = self.model_of(&plan["model"])?;
        let model = s(&plan["model"]);
        if plan["model"].is_null() {
            bail!("Exception calling \"Contains\" with \"1\" argument(s): \"Value cannot be null. (Parameter 'key')\"");
        }
        if !self.state["models"].as_object().is_some_and(|m| m.contains_key(&model)) {
            self.state["order"] = plus(&self.state["order"], [plan["model"].clone()]);
            *self.m_mut(&model) = json!({ "ids": {}, "titles": {} });
        }
        // The architect's source observations go into the run's retained record before the plan's
        // own structure is judged: a bad observation names its entry and counts as a plan fault.
        let path = self.p.plan.clone();
        let mut faults = self.record_source_faults(&plan, &path);
        let (reviewed, review_faults) = self.source_review(&plan, &model, &path);
        faults.extend(review_faults);
        faults.extend(self.test_plan(&plan, &st, &model)?);
        let mut lines = Vec::new();
        if let Some(report) = &self.source_report {
            lines.push(format!(
                "source-faults: {} recorded, {} source faults in {}",
                s(&report["recorded"]),
                s(&report["faults"]),
                s(&report["report"])
            ));
        }
        lines.push(format!(
            "source-review: {} {} parts in {}",
            if reviewed["complete"] == json!(true) { "complete" } else { "incomplete" },
            s(&reviewed["parts"]),
            s(&reviewed["path"])
        ));
        lines.extend(faults.clone());
        lines.push(format!("faults: {}", faults.len()));
        util::print_lines(&lines)
    }

    /// The folder of the prepared run: `-Staging`, or the nearest folder above `-Work` that holds
    /// its manifest.json (the architect validates without `-Staging`).
    fn run_folder(&self) -> String {
        if !self.p.staging.is_empty() {
            return self.p.staging.clone();
        }
        let mut dir = ps::parent(&self.p.work);
        while !dir.is_empty() && !ps::exists(&ps::join(&dir, "manifest.json")) {
            dir = ps::parent(&dir);
        }
        dir
    }

    /// Records one plan's `source_faults` in the run's retained source-fault record and renders
    /// its report (design 11.6, R312). Returns one plan fault per reason it could not.
    fn record_source_faults(&mut self, plan: &Value, origin: &str) -> Vec<String> {
        let entries: Vec<Value> = list(&plan["source_faults"]).into_iter().cloned().collect();
        if entries.is_empty() {
            return Vec::new();
        }
        let folder = self.run_folder();
        if folder.is_empty() {
            return vec!["source_faults: no prepared run folder holds manifest.json, so the fault record has no staging folder".into()];
        }
        let identity = match crate::faults::identity(&folder) {
            Ok(identity) => identity,
            Err(error) => return vec![format!("source_faults: {error:#}")],
        };
        let normalized = match crate::faults::entries(&identity, &entries, origin) {
            Ok(entries) => entries,
            Err(error) => return vec![format!("source_faults: {error:#}")],
        };
        match crate::faults::record(&folder, &identity, normalized) {
            Ok(summary) => {
                self.source_report = Some(summary);
                Vec::new()
            }
            Err(error) => vec![format!("source_faults: {error:#}")],
        }
    }

    fn step_apply(&mut self) -> Result<()> {
        let mut jobs = Vec::new();
        let mut models = Vec::new();
        for model in self.order() {
            let status = s(&self.m(&model)["status"]);
            if !ps::exists(&ps::join(&self.dir(&model), "plan.json")) || !ps::is_in(&status, &["brief", "planned"]) {
                continue;
            }
            match self.build_model(&model) {
                Ok(built) => jobs.extend(built),
                Err(error) => {
                    let m = self.m_mut(&model);
                    m["status"] = json!("failed");
                    m["error"] = json!(format!("{error:#}"));
                }
            }
            let m = self.m(&model);
            models.push(json!({ "model": model, "status": m["status"], "error": m["error"] }));
            self.save_state()?;
        }
        self.save_state()?;
        self.out_last(&json!({ "jobs": jobs, "models": models }))
    }

    /// `Build-Model`: validates the plan, builds the skeleton, writes setting.md,
    /// and returns the round-1 fill jobs.
    fn build_model(&mut self, model: &str) -> Result<Vec<Value>> {
        let dir = self.dir(model);
        let plan_path = ps::join(&dir, "plan.json");
        let plan = util::read_json(&plan_path)?;
        let st = self.model_of(&json!(model))?;
        let faults = self.test_plan(&plan, &st, model)?;
        if !faults.is_empty() {
            let m = self.m_mut(model);
            m["status"] = json!("failed");
            m["error"] = json!(format!("plan faults: {}", faults.join("; ")));
            return Ok(Vec::new());
        }
        // An accepted plan's source observations are recorded here too, so a run whose architect
        // skipped `validate` still keeps them.
        let plan_path = plan_path.clone();
        let source_faults = self.record_source_faults(&plan, &plan_path);
        if !source_faults.is_empty() {
            let m = self.m_mut(model);
            m["status"] = json!("failed");
            m["error"] = json!(format!("source faults: {}", source_faults.join("; ")));
            return Ok(Vec::new());
        }
        // R247: the accepted plan's source review is recorded beside the plan, and a written part
        // this build cannot account for keeps the model unbuilt.
        let (reviewed, review_faults) = self.source_review(&plan, model, &plan_path);
        if !review_faults.is_empty() {
            let m = self.m_mut(model);
            m["status"] = json!("failed");
            m["error"] = json!(format!("source review: {}", review_faults.join("; ")));
            return Ok(Vec::new());
        }
        self.m_mut(model)["source_review"] = reviewed.clone();
        self.record_step(model, "source_review", reviewed);
        if ps::eq(&s(&plan["status"]), "skip") && !plan["status"].is_null() {
            self.m_mut(model)["status"] = json!("skipped");
            return Ok(Vec::new());
        }
        // The accepted plan's identity: a later completion verifier binds its own evidence to it.
        let plan_hash = Self::file_hash(&plan_path)?;
        self.m_mut(model)["plan_hash"] = json!(plan_hash);
        self.m_mut(model)["plan"] = json!({"path":plan_path,"sha256":plan_hash});
        self.record_step(model, "plan", json!({"plan":plan_path,"plan_hash":plan_hash,"faults":0,"level":plan["level"]}));
        self.m_mut(model)["level"] = plan["level"].clone();
        let mut parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
        let mut known: CiMap<Value> = CiMap::new();
        for p in list(&st["parts"]) {
            known.insert(&s(&p["address"]), p.clone());
        }
        let mut ids = Map::new();
        let mut titles = Map::new();
        let mut facets = Map::new();
        for p in &parts {
            let id = s(&p["id"]);
            let r = ref_of(p);
            if r != "new" {
                // A take of another model's part (`X#A`) is stored as the real address: the
                // dependent's part map must name Cards, not unresolved references (R233).
                let address = self.resolve(&Value::String(s(&p[r])), model)?;
                ids.insert(id.clone(), json!(address));
            }
            let was = if truthy(&p["existing"]) { known.get(&s(&p["existing"])).cloned() } else { None };
            let pick = |field: &str| {
                if truthy(&p[field]) {
                    p[field].clone()
                } else if let Some(w) = &was {
                    w[field].clone()
                } else {
                    json!("")
                }
            };
            titles.insert(id.clone(), pick("title"));
            facets.insert(id, pick("facet"));
        }
        {
            let m = self.m_mut(model);
            m["ids"] = Value::Object(ids);
            m["titles"] = Value::Object(titles);
            m["facets"] = Value::Object(facets);
        }
        // Material of merged parts goes into the job of the part that takes it in.
        for mg in list(&plan["merge"]) {
            let card = self.card(&mg["from"])?;
            if let Some(into) = parts.iter_mut().find(|p| !p["id"].is_null() && ps::eq(&s(&p["id"]), &s(&mg["into"])) && !mg["into"].is_null())
            {
                let merged = format!("[[{}]] {}\n{}", s(&mg["from"]), s(&card["title"]), s(&card["body"]));
                let joined = [into["old"].clone(), json!(merged)].into_iter().filter(truthy).map(|v| s(&v)).collect::<Vec<_>>().join("\n\n");
                into["old"] = json!(joined);
            }
        }
        let mut edit = json!({ "at": model, "level": plan["level"] });
        if truthy(&plan["title"]) {
            edit["title"] = plan["title"].clone();
        }
        self.edits(vec![json!({ "tool": "edit_card", "arguments": edit })])?;

        // Main line, in build order. A new part goes before the next kept part, or after the last one.
        let main: Vec<usize> = (0..parts.len()).filter(|&i| ps::eq(&line_of(&parts[i]), "main")).collect();
        let mut previous: Option<usize> = None;
        for k in 0..main.len() {
            let p = parts[main[k]].clone();
            let id = s(&p["id"]);
            let anchor = main[k + 1..].iter().find(|&&i| truthy(&parts[i]["existing"])).map(|&i| s(&parts[i]["id"]));
            let anchor_address = anchor.map(|a| self.m(model)["ids"][&a].clone()).unwrap_or(Value::Null);
            let mut fields = Map::new();
            fields.insert("title".into(), p["title"].clone());
            fields.insert("facet".into(), p["facet"].clone());
            fields.insert("type".into(), json!("model"));
            fields.insert("body".into(), json!("待写"));
            if truthy(&p["emphasis"]) {
                fields.insert("emphasis".into(), p["emphasis"].clone());
            } else if derives(&p) {
                fields.insert("emphasis".into(), json!("aside"));
            }
            if truthy(&p["existing"]) {
                previous = Some(main[k]);
                continue;
            }
            if truthy(&p["move"]) {
                let mut target = Map::new();
                if truthy(&anchor_address) {
                    target.insert("before".into(), anchor_address.clone());
                } else if let Some(prev) = previous {
                    let at = self.id_address(model, &s(&parts[prev]["id"]));
                    target.insert("to".into(), json!(Self::direct_slot(&at)?));
                } else {
                    let set = self.tree_addresses(model)?;
                    target.insert("to".into(), json!(Self::free_side(model, &set)?));
                }
                let at = self.id_address(model, &id);
                let set = self.tree_addresses(&at)?;
                let alone = !set.iter().any(|a| a.starts_with(&format!("{at}|")));
                target.insert("at".into(), self.m(model)["ids"][&id].clone());
                target.insert("alone".into(), json!(alone));
                target.insert("confirm".into(), json!("move"));
                self.write(vec![json!({ "tool": "move_card", "arguments": target })])?;
                let moved = format!("{} -> {}", s(&p["move"]), self.id_address(model, &id));
                let m = self.m_mut(model);
                m["moved"] = plus(&m["moved"], [json!(moved)]);
            } else if truthy(&anchor_address) {
                let mut args = fields.clone();
                args.insert("before".into(), anchor_address);
                args.insert("confirm".into(), json!("insert"));
                let address = self.write(vec![json!({ "tool": "insert_card", "arguments": args })])?.into_iter().next();
                self.m_mut(model)["ids"][&id] = json!(address);
            } else if let Some(prev) = previous {
                let mut args = fields.clone();
                args.insert("at".into(), self.m(model)["ids"][&s(&parts[prev]["id"])].clone());
                args.insert("kind".into(), json!("direct"));
                let address = self.write(vec![json!({ "tool": "create_successor", "arguments": args })])?.into_iter().next();
                self.m_mut(model)["ids"][&id] = json!(address);
            } else {
                let mut args = fields.clone();
                args.insert("at".into(), json!(model));
                args.insert("kind".into(), json!("side"));
                let address = self.write(vec![json!({ "tool": "create_successor", "arguments": args })])?.into_iter().next();
                self.m_mut(model)["ids"][&id] = json!(address);
            }
            previous = Some(main[k]);
        }
        // Side parts beside their step, then off parts beside the Model Card.
        for i in 0..parts.len() {
            let p = parts[i].clone();
            let line = line_of(&p);
            if !ps::is_in(&line, &["side", "off"]) {
                continue;
            }
            let id = s(&p["id"]);
            let parent = if ps::eq(&line, "side") { self.resolve(&p["of"], model)? } else { model.to_string() };
            if truthy(&p["existing"]) {
                continue;
            }
            if truthy(&p["move"]) {
                let set = self.tree_addresses(model)?;
                let to = Self::free_side(&parent, &set)?;
                let at = self.m(model)["ids"][&id].clone();
                self.write(vec![json!({ "tool": "move_card", "arguments": { "at": at, "to": to, "confirm": "move" } })])?;
                let moved = format!("{} -> {}", s(&p["move"]), self.id_address(model, &id));
                let m = self.m_mut(model);
                m["moved"] = plus(&m["moved"], [json!(moved)]);
                continue;
            }
            let mut fields = Map::new();
            fields.insert("at".into(), json!(parent));
            fields.insert("kind".into(), json!("side"));
            fields.insert("title".into(), p["title"].clone());
            fields.insert("facet".into(), p["facet"].clone());
            fields.insert("type".into(), json!("model"));
            fields.insert("body".into(), json!("待写"));
            if truthy(&p["emphasis"]) {
                fields.insert("emphasis".into(), p["emphasis"].clone());
            }
            let address = self.write(vec![json!({ "tool": "create_successor", "arguments": fields })])?.into_iter().next();
            self.m_mut(model)["ids"][&id] = json!(address);
        }
        // Merged parts: only a part without successors can go (a delete takes its successors with it).
        for mg in list(&plan["merge"]) {
            let set = self.tree_addresses(model)?;
            if Self::has_successors(&s(&mg["from"]), &set)? {
                let note = format!("not merged: {} has successors", s(&mg["from"]));
                let m = self.m_mut(model);
                m["notes"] = plus(&m["notes"], [json!(note)]);
                continue;
            }
            let into = self.resolve(&mg["into"], model)?;
            self.write(vec![json!({ "tool": "delete_card", "arguments": { "at": mg["from"], "into": into, "confirm": "delete" } })])?;
        }
        // A refilled part loses its body to the skeleton: keep the old body for the job that rewrites it.
        self.m_mut(model)["old"] = json!({});
        let refill: Vec<Value> = parts.iter().filter(|p| truthy(&p["existing"]) && truthy(&p["refill"])).cloned().collect();
        if !refill.is_empty() {
            let mut calls = Vec::new();
            for p in &refill {
                calls.push(("get_card", json!({ "at": self.resolve(&p["id"], model)? })));
            }
            let cards = self.reads(&calls)?;
            for (k, p) in refill.iter().enumerate() {
                let body = cards.get(k).map(|c| c["body"].clone()).unwrap_or(Value::Null);
                if !s(&body).trim().is_empty() && !TODO_BODY.is_match(&s(&body)) {
                    self.m_mut(model)["old"][&s(&p["id"])] = json!(s(&body));
                }
            }
        }
        // Skeleton bodies, now that every address is final.
        let mut edits = Vec::new();
        let mut prev: Option<Value> = None;
        for p in &parts {
            if truthy(&p["take"]) {
                continue;
            }
            let address = self.resolve(&p["id"], model)?;
            if truthy(&p["existing"]) || truthy(&p["move"]) {
                let mut change = Map::new();
                change.insert("at".into(), json!(address));
                for f in ["title", "facet", "emphasis"] {
                    if truthy(&p[f]) {
                        change.insert(f.into(), p[f].clone());
                    }
                }
                if truthy(&p["refill"]) {
                    change.insert("body".into(), json!(self.skeleton(p, &plan, model, prev.as_ref())?));
                }
                if change.len() > 1 {
                    edits.push(json!({ "tool": "edit_card", "arguments": change }));
                }
            } else {
                edits.push(json!({ "tool": "edit_card", "arguments": { "at": address, "body": self.skeleton(p, &plan, model, prev.as_ref())? } }));
            }
            if ps::eq(&line_of(p), "main") {
                prev = Some(p.clone());
            }
        }
        let card = self.card(&json!(model))?;
        let structure = self.structure_list(&plan, model)?;
        edits.push(json!({ "tool": "edit_card", "arguments": { "at": model, "body": text::set_section(&s(&card["body"]), "结构", &structure) } }));
        self.edits(edits)?;
        {
            let main_ids: Vec<Value> = main.iter().map(|&i| parts[i]["id"].clone()).collect();
            let derive_ids: Vec<Value> = parts.iter().filter(|p| derives(p)).map(|p| p["id"].clone()).collect();
            let m = self.m_mut(model);
            m["main"] = Value::Array(main_ids);
            m["derive"] = Value::Array(derive_ids);
            m["status"] = json!("built");
        }

        // setting.md opens every DeepSeek job of this model, after the shared instructions.
        let formal = ps::eq(&s(&plan["level"]), "formal");
        let setting = main.first().map(|&i| parts[i].clone()).unwrap_or(Value::Null);
        let setting_text = if formal {
            format!(
                "# 本模型的设定：[[{}]] {}\n\n{}\n\n{}\n",
                self.resolve(&setting["id"], model)?,
                s(&setting["title"]),
                s(&setting["spec"]),
                text::tables(&plan)
            )
        } else {
            format!("# 本模型（概念模型）：[[{model}]]\n\n{}\n", s(&plan["card"]))
        };
        util::write_text(ps::join(&dir, "setting.md"), &setting_text)?;

        // Round 1: ONE job per model holding the Model Card and every part that is not a Derivation
        // part, in build order, continuing the model's conversation (`<slug>:brief`, #194).
        let todo: Vec<Value> = parts
            .iter()
            .filter(|p| {
                !truthy(&p["take"]) && !derives(p) && (!(truthy(&p["existing"]) || truthy(&p["move"])) || truthy(&p["refill"]))
            })
            .cloned()
            .collect();
        let head_job = self.fill_job(model, &plan, &todo)?;
        let jobs = vec![head_job];
        let fill_addresses = todo.iter().filter_map(|p| self.m(model)["ids"].get(s(&p["id"])).map(s)).collect::<Vec<_>>();
        self.record_step(
            model,
            "apply",
            json!({"plan_hash":plan_hash,"parts":parts.len(),"fill":fill_addresses,"jobs":jobs.len(),"faults":0}),
        );
        Ok(jobs)
    }

    fn step_derive(&mut self) -> Result<()> {
        let mut jobs = Vec::new();
        for model in self.order() {
            let m = self.m(&model);
            if !ps::eq(&s(&m["status"]), "built") || list(&m["derive"]).is_empty() {
                continue;
            }
            // Chains in build order; one job holds at most three neighbouring parts of one chain.
            let mut ids: Vec<Value> = list(&m["main"]).into_iter().cloned().collect();
            ids.push(json!(""));
            let mut chains: Vec<Vec<Value>> = Vec::new();
            let mut chain = Vec::new();
            for id in ids {
                let p = if truthy(&id) { self.plan_part(&model, &s(&id))? } else { None };
                if let Some(p) = p.filter(derives) {
                    chain.push(p);
                    continue;
                }
                if !chain.is_empty() {
                    chains.push(std::mem::take(&mut chain));
                }
            }
            let before = jobs.len();
            // One job per model holds every chain in order, continuing the model's conversation (#194).
            let mut blocks = Vec::new();
            let mut plan_ids = Vec::new();
            let mut n = 0;
            for c in chains {
                for p in c {
                    n += 1;
                    blocks.push(self.part_block(n, &p, &model, true)?);
                    plan_ids.push(p["id"].clone());
                }
            }
            if !plan_ids.is_empty() {
                let mut job = self.new_job(&model, "d", plan_ids, &blocks.join("\n"), "按以上说明完成你的推导分卡，最后每张分卡回复一行 JSON。")?;
                self.continue_model(&mut job, &model, &self.docs.fill)?;
                jobs.push(job);
            }
            let jobs_now = jobs.len() - before;
            self.m_mut(&model)["status"] = json!("derived");
            self.record_step(&model, "derive", json!({"jobs":jobs_now,"parts":list(&self.m(&model)["derive"]).len()}));
        }
        self.save_state()?;
        self.out_last(&json!({ "jobs": jobs }))
    }

    /// `review` (#194): one job per model. By default it continues the model's conversation, and its
    /// job file tells the reviewer to re-read the stored Derivation Cards from the archive instead of
    /// trusting its memory. With `-IndependentReview` the first round is a new conversation
    /// (`<slug>:review`) whose job file carries the Card bodies, and a later round continues it.
    fn step_review(&mut self) -> Result<()> {
        let mut jobs = Vec::new();
        let independent = self.p.independent_review;
        for model in self.order() {
            let m = self.m(&model);
            if !ps::is_in(&s(&m["status"]), &["derived", "reviewed"]) || list(&m["derive"]).is_empty() {
                continue;
            }
            let st = self.model_of(&json!(model))?;
            let mut bodies: CiMap<Value> = CiMap::new();
            for p in list(&st["parts"]) {
                bodies.insert(&s(&p["address"]), p.clone());
            }
            let mut touched = Vec::new();
            for r in list(&self.state["rereview"]).into_iter().cloned().collect::<Vec<_>>() {
                touched.push(self.resolve(&r, &model)?);
            }
            // Only the model's own Derivation parts: a shared part's chain belongs to its home model.
            let mut own = Vec::new();
            for id in ps::keys(&self.m(&model)["ids"]) {
                let p = self.plan_part(&model, &id)?;
                if !p.is_some_and(|p| truthy(&p["take"])) {
                    own.push(self.id_address(&model, &id));
                }
            }
            // A round that continues a conversation re-reads the Cards from the archive (its memory may
            // hold an older body); a new conversation gets the bodies in its job file.
            let continuing = !independent || self.p.changed;
            let mut chains: Vec<(Vec<String>, String, String)> = Vec::new();
            for c in list(&st["build"]["chains"]) {
                let parts: Vec<String> = pipe(&c["parts"]).into_iter().map(s).filter(|a| ps::is_in(a, &own)).collect();
                if parts.is_empty() {
                    continue;
                }
                if self.p.changed && !parts.iter().any(|a| ps::is_in(a, &touched)) {
                    continue;
                }
                chains.push((parts, s(&c["after"]), s(&c["before"])));
            }
            let before = jobs.len();
            if !chains.is_empty() {
                let seq = self.next_seq();
                let dir = self.dir(&model);
                let out = ps::join(&dir, &format!("review-{seq}.json"));
                let id = if independent {
                    // The reviewer's own conversation: named by the workflow on its first round.
                    if self.p.changed { format!("{}:review+{seq}", Self::slug(&model)) } else { format!("{}:review", Self::slug(&model)) }
                } else {
                    format!("{}:r{seq}", Self::slug(&model))
                };
                let mut text = format!(
                    "# 复核任务 {id}\n\n结果文件: {out}\nMaxSteps: {}\n模型卡: [[{model}]]\n\r\n",
                    self.p.max_steps
                );
                if continuing {
                    text += CR;
                    text += &self.review_reread_text(&chains)?;
                } else {
                    text += CR;
                    for (_, after, before) in &chains {
                        for (label, at) in [("链之前的分卡（推导从它出发）", after), ("链之后的分卡（它用推导的结果）", before)] {
                            if !at.is_empty()
                                && let Some(b) = bodies.get(at).filter(|b| truthy(b))
                            {
                                text += &format!("## {label}：[[{}]] {}\n\n{}\n\r\n", at, s(&b["title"]), s(&b["body"]));
                            }
                        }
                    }
                    for (parts, _, _) in &chains {
                        for a in parts {
                            let b = bodies.get(a).cloned().unwrap_or(Value::Null);
                            text += &format!("## 推导分卡 [[{a}]] {}\n\n{}\n\r\n", s(&b["title"]), s(&b["body"]));
                        }
                    }
                }
                let file = ps::join(&dir, &format!("review-{seq}.md"));
                util::write_text(&file, &text)?;
                let mut job = json!({
                    "id": id,
                    "files": [self.docs.review, ps::join(&dir, "setting.md"), file],
                    "prompt": format!("按以上说明复核这些推导分卡，把结果写进 {out}，最后只回复一行 JSON。"),
                });
                if continuing {
                    let into = if independent { format!("{}:review", Self::slug(&model)) } else { format!("{}:brief", Self::slug(&model)) };
                    job["continue"] = json!(into);
                    job["fallback"] = json!([self.docs.review, ps::join(&dir, "setting.md"), file]);
                }
                jobs.push(job);
            }
            self.m_mut(&model)["status"] = json!("reviewed");
            self.record_step(&model, "review", json!({"jobs":jobs.len()-before,"changed":self.p.changed,"rounds":1}));
        }
        self.state["rereview"] = json!([]);
        self.save_state()?;
        self.out_last(&json!({ "jobs": jobs }))
    }

    /// The job text of a review that continues a conversation: the reviewer must read the stored Cards
    /// back from the archive, because the conversation's memory may hold a body a fix or split replaced.
    fn review_reread_text(&self, chains: &[(Vec<String>, String, String)]) -> Result<String> {
        let mut text = String::from(
            "这次复核接着你写这些分卡的对话。分卡可能已经被后续的修正和拆分改过：先用一条命令从档案读回它们的当前正文，不要凭记忆复核。\r\n\r\n读法：把下面每个地址写成一条 `get_card`，一次传给助手（HELPER）：\r\n\r\n```text\r\n@'\r\n[{\"tool\":\"get_card\",\"arguments\":{\"at\":\"<地址>\"}},…]\r\n'@ | zt-digest call -\r\n```\r\n\r\n## 要复核的推导分卡\r\n\r\n",
        );
        let mut seen: Vec<String> = Vec::new();
        for (parts, _, _) in chains {
            for a in parts {
                if !seen.contains(a) {
                    seen.push(a.clone());
                    text += &format!("- [[{a}]]\r\n");
                }
            }
        }
        let mut around: Vec<(&str, String)> = Vec::new();
        for (_, after, before) in chains {
            if !after.is_empty() {
                around.push(("链之前的分卡（推导从它出发）", after.clone()));
            }
            if !before.is_empty() {
                around.push(("链之后的分卡（它用推导的结果）", before.clone()));
            }
        }
        let mut named: Vec<String> = Vec::new();
        for (label, at) in around {
            if seen.contains(&at) || named.contains(&at) {
                continue;
            }
            named.push(at.clone());
            text += &format!("\r\n## {label}：[[{at}]]\r\n");
        }
        Ok(text)
    }

    /// `Find-Owner`: the model and plan id of a part this batch built (never a shared part).
    fn find_owner(&mut self, address: &str) -> Result<Option<(String, String)>> {
        for model in self.order() {
            for id in ps::keys(&self.m(&model)["ids"]) {
                let value = &self.m(&model)["ids"][&id];
                if value.is_null() || !ps::eq(&s(value), address) {
                    continue;
                }
                let p = self.plan_part(&model, &id)?;
                if p.is_some_and(|p| truthy(&p["take"])) {
                    continue;
                }
                return Ok(Some((model, id)));
            }
        }
        Ok(None)
    }

    /// `Find-Job`: the last job of the model that holds the id.
    fn find_job(&self, model: &str, id: &str) -> Option<String> {
        let mut found = None;
        for k in ps::keys(&self.state["jobs"]) {
            let j = &self.state["jobs"][&k];
            if ps::eq(&s(&j["model"]), model) && !j["model"].is_null() && ps::is_in(id, &strings(&j["ids"])) {
                found = Some(k);
            }
        }
        found
    }

    /// `Split-Part`: inserts the new parts after the part, re-points the next
    /// member, updates the 结构 list, and says what the conversation must do.
    fn split_part(&mut self, model: &str, id: &str, count: i64, cuts: &[Value], reason: &Value) -> Result<Follow> {
        self.plan_part(model, id)?;
        let address = self.m(model)["ids"][id].clone();
        let card = self.card(&address)?;
        let derive = truthy(&card["facet"]) && ps::starts_with(&s(&card["facet"]), "推导");
        let fin = if derive { text::result_of(&s(&card["body"])) } else { String::new() };
        let main = strings(&Value::Array(list(&self.m(model)["main"]).into_iter().cloned().collect()));
        let main_index = ps::index_of(&main, id);
        let next = if main_index >= 0 && ((main_index + 1) as usize) < main.len() { Some(main[(main_index + 1) as usize].clone()) } else { None };
        if !truthy(&self.m(model)["added"]) {
            self.m_mut(model)["added"] = json!({});
        }
        let cuts: Vec<Value> = cuts.iter().filter(|c| !c.is_null()).cloned().collect();
        let cut = |i: i64| if i >= 0 { cuts.get(i as usize).cloned().unwrap_or(Value::Null) } else { Value::Null };
        let mut new: Vec<String> = Vec::new();
        let mut prev_id = id.to_string();
        for k in 2..=count {
            let new_id = format!("{id}~{k}");
            let mut facet = format!("{}（{k}/{count}）", s(&card["facet"]));
            if ps::len16(&facet) > 32 {
                facet = ps::cut16(&facet, 32);
            }
            let title = format!("{}（{k}/{count}）", s(&card["title"]));
            let mut fields = Map::new();
            fields.insert("title".into(), json!(title));
            fields.insert("facet".into(), json!(facet));
            fields.insert("type".into(), json!("model"));
            fields.insert("body".into(), json!("待写"));
            if truthy(&card["emphasis"]) {
                fields.insert("emphasis".into(), card["emphasis"].clone());
            }
            let address = if let Some(next) = &next {
                let mut args = fields.clone();
                args.insert("before".into(), self.m(model)["ids"][next].clone());
                args.insert("confirm".into(), json!("insert"));
                self.write(vec![json!({ "tool": "insert_card", "arguments": args })])?.into_iter().next()
            } else {
                let at = self.id_address(model, &prev_id);
                let slot = Self::direct_slot(&at)?;
                let set = self.tree_addresses(model)?;
                let mut args = fields.clone();
                if set.contains(&slot) {
                    args.insert("before".into(), json!(slot));
                    args.insert("confirm".into(), json!("insert"));
                    self.write(vec![json!({ "tool": "insert_card", "arguments": args })])?.into_iter().next()
                } else {
                    args.insert("at".into(), json!(at));
                    args.insert("kind".into(), json!("direct"));
                    self.write(vec![json!({ "tool": "create_successor", "arguments": args })])?.into_iter().next()
                }
            };
            self.m_mut(model)["ids"][&new_id] = json!(address);
            self.m_mut(model)["titles"][&new_id] = json!(title);
            let from = if truthy(&cut(k - 2)) { s(&cut(k - 2)) } else { "（上一张卡选定的中间结论）".into() };
            let to = if k == count {
                fin.clone()
            } else if truthy(&cut(k - 1)) {
                s(&cut(k - 1))
            } else {
                "（由你选定的中间结论）".into()
            };
            let prev_address = self.id_address(model, &prev_id);
            // The position line of 思路 comes from Update-ChainLines once every new part exists.
            let start = if derive {
                format!("**思路**\n待写\n\n**起点**\n- 依赖上一步 [[{prev_address}]] 的结果：{from}。\n\n**步骤**\n待写\n\n**结果**\n{to}\n\n**解读**\n待写")
            } else {
                format!("依赖上一步 [[{prev_address}]]：本卡接着上一张卡写。\n\n待写")
            };
            self.m_mut(model)["added"][&new_id] = json!({
                "id": new_id, "facet": facet, "title": title, "derive": derive, "to": to, "line": "main",
                "spec": format!("接着 [[{prev_address}]] 写。"),
            });
            self.edits(vec![json!({ "tool": "edit_card", "arguments": { "at": address, "body": start } })])?;
            new.push(new_id.clone());
            prev_id = new_id.clone();
            if main_index >= 0 {
                let mut list_now: Vec<Value> = list(&self.m(model)["main"]).into_iter().cloned().collect();
                let at = (main_index + k - 1) as usize;
                if at > list_now.len() {
                    bail!("Exception calling \"Insert\" with \"2\" argument(s): \"Index must be within the bounds of the List.\"");
                }
                list_now.insert(at, json!(new_id));
                self.m_mut(model)["main"] = Value::Array(list_now);
            }
            if derive {
                let m = self.m_mut(model);
                m["derive"] = plus(&m["derive"], [json!(new_id)]);
            }
        }
        let mut edits = Vec::new();
        let own_address = self.id_address(model, id);
        if derive && !cuts.is_empty() {
            // The part now ends at the first cut: its 结果 is the cut, and its 解读 must be written again.
            let body = s(&self.card(&self.m(model)["ids"][id].clone())?["body"]);
            let mut sections = text::split_sections(&body);
            for sec in &mut sections {
                if ps::eq(&sec.head, "结果") {
                    sec.lines = vec![s(&cuts[0])];
                }
                if ps::eq(&sec.head, "解读") {
                    sec.lines = vec!["待写".into()];
                }
            }
            edits.push(json!({ "tool": "edit_card", "arguments": { "at": self.m(model)["ids"][id], "body": text::join_sections(&sections) } }));
        }
        if derive && let Some(next) = &next {
            let next_at = self.m(model)["ids"][next].clone();
            let next_card = self.card(&next_at)?;
            let last = new.last().map(|n| self.id_address(model, n)).unwrap_or_default();
            let body = s(&next_card["body"]).replace(&format!("[[{own_address}]]"), &format!("[[{last}]]"));
            edits.push(json!({ "tool": "edit_card", "arguments": { "at": next_at, "body": body } }));
        }
        // the 结构 list names the new parts right after the part they continue
        let head = self.card(&json!(model))?;
        let mut lines: Vec<String> = s(&head["body"]).replace('\r', "").split('\n').map(str::to_string).collect();
        let link = format!("[[{own_address}]]");
        let at = lines.iter().rposition(|l| l.contains(&link) && LIST_ITEM.is_match(l));
        if let Some(at) = at {
            let added: Vec<String> = new
                .iter()
                .map(|n| {
                    let a = &self.m(model)["added"][n];
                    format!("- {} [[{}]]：{}", s(&a["facet"]), self.id_address(model, n), s(&a["title"]))
                })
                .collect();
            lines.splice(at + 1..at + 1, added);
            edits.push(json!({ "tool": "edit_card", "arguments": { "at": model, "body": lines.join("\n") } }));
        }
        self.edits(edits)?;
        {
            let m = self.m_mut(model);
            m["split"] = json!(ps::int(&m["split"]).unwrap_or(0) + 1);
        }
        self.record_step(model, "split", json!({"id":id,"parts":count,"added":new.len()}));
        let mut again = vec![json!(format!("{model}#{id}"))];
        again.extend(new.iter().map(|n| json!(format!("{model}#{n}"))));
        self.state["rereview"] = plus(&self.state["rereview"], again);
        // the job that wrote the part continues with the new parts
        let job = self.find_job(model, id);
        if let Some(job) = &job {
            let ids = plus(&self.state["jobs"][job]["ids"], new.iter().map(|n| json!(n)));
            let mut addresses = Vec::new();
            for i in pipe(&ids) {
                addresses.push(json!(self.resolve(i, model)?));
            }
            self.state["jobs"][job]["ids"] = ids;
            let text = list(&Value::Array(addresses.clone())).iter().map(|a| s(a)).collect::<Vec<_>>().join("\n") + "\n";
            self.state["jobs"][job]["addresses"] = Value::Array(addresses);
            util::write_text(s(&self.state["jobs"][job]["allow"]), &text)?;
        }
        let own_address = self.id_address(model, id);
        let reason_text = if truthy(reason) { format!("（{}）", s(reason)) } else { String::new() };
        let mut text = format!("## 拆分：[[{own_address}]] 拆成 {count} 张{reason_text}\n\n");
        let upto = if derive && !cuts.is_empty() { s(&cuts[0]) } else { "你选的中间结论".into() };
        text += &format!("- [[{own_address}]]：只写到 {upto}。{}\n", if derive { "它的解读已清空：按新的结果重写。" } else { "" });
        if derive {
            text += "- 每张推导分卡写自己的思路、说明和解读。思路的位置行由脚本写好，原样保留。\n";
        }
        for n in &new {
            let a = self.m(model)["added"][n].clone();
            let to = if truthy(&a["to"]) { s(&a["to"]) } else { "你选的中间结论".into() };
            text += &format!("- [[{}]]：{}，从上一张的结果写到 {to}。标题可改成本卡的结论。\n", self.id_address(model, n), s(&a["title"]));
        }
        let mut ids = vec![id.to_string()];
        ids.extend(new);
        Ok(Follow { model: model.to_string(), job, text, ids, cards: Vec::new() })
    }

    fn step_split(&mut self) -> Result<()> {
        // job id (or new:<model>) -> what its conversation must do next
        let mut follow: CiMap<Follow> = CiMap::new();
        let mut orphans: Vec<Value> = Vec::new();
        let add = add_follow;
        // Name every target by its model and id first: a split renumbers the addresses after it.
        let mut actions: Vec<Action> = Vec::new();
        for pair in self.p.need.split(';').filter(|p| !p.trim().is_empty()).map(str::to_string).collect::<Vec<_>>() {
            let mut it = pair.splitn(2, '=');
            let address = it.next().unwrap_or_default().to_string();
            let count = it.next().map(|c| Value::String(c.to_string())).unwrap_or(Value::Null);
            match self.find_owner(address.trim())? {
                Some((model, id)) => actions.push(Action {
                    model,
                    id,
                    verdict: "split".into(),
                    parts: ps::int(&count)?,
                    cuts: Vec::new(),
                    reason: json!("写不下"),
                }),
                None => orphans.push(json!(address)),
            }
        }
        for model in self.order() {
            let dir = self.dir(&model);
            for name in ps::child_files(&dir, "review-", ".json") {
                let full = ps::full_name(&ps::join(&dir, &name));
                if ps::is_in(&full, &strings(&Value::Array(list(&self.state["reviews"]).into_iter().cloned().collect()))) {
                    continue;
                }
                self.state["reviews"] = plus(&self.state["reviews"], [json!(full)]);
                let verdicts = match util::read_json(ps::join(&dir, &name)) {
                    Ok(v) => pipe(&v).into_iter().cloned().collect::<Vec<_>>(),
                    Err(e) => {
                        orphans.push(json!(format!("{name}: {e:#}")));
                        Vec::new()
                    }
                };
                for v in verdicts {
                    if !ps::is_in(&s(&v["verdict"]), &["split", "fix"]) || v["verdict"].is_null() {
                        continue;
                    }
                    match self.find_owner(&s(&v["address"]))? {
                        Some((model, id)) => actions.push(Action {
                            model,
                            id,
                            verdict: s(&v["verdict"]),
                            parts: ps::int(&v["parts"])?,
                            cuts: list(&v["cuts"]).into_iter().cloned().collect(),
                            reason: v["reason"].clone(),
                        }),
                        None => orphans.push(v["address"].clone()),
                    }
                }
            }
        }
        let mut resplit: Vec<String> = Vec::new();
        for a in actions {
            if ps::eq(&a.verdict, "split") && a.parts >= 2 {
                let r = self.split_part(&a.model, &a.id, a.parts, &a.cuts, &a.reason)?;
                resplit.push(a.model.clone());
                add(&mut follow, r);
            } else {
                self.state["rereview"] = plus(&self.state["rereview"], [json!(format!("{}#{}", a.model, a.id))]);
                let job = self.find_job(&a.model, &a.id);
                let at = self.resolve(&json!(a.id), &a.model)?;
                let text = format!("## 修正：[[{at}]]\n\n{}\n", s(&a.reason));
                add(&mut follow, Follow { model: a.model.clone(), job, text, ids: vec![a.id.clone()], cards: Vec::new() });
            }
        }
        // A split changes the length of its chain: every member gets its new position line.
        let mut seen = Vec::new();
        for model in resplit {
            if !seen.contains(&model) {
                seen.push(model.clone());
                self.update_chain_lines(&model)?;
            }
        }
        // A split renumbers the parts after it: the 共用 lines of the other models follow.
        self.refresh_shared()?;
        let jobs = self.follow_jobs(&follow)?;
        self.save_state()?;
        self.out_last(&json!({ "jobs": jobs, "orphans": orphans }))
    }

    /// `close`: the build faults each model of the batch still has after its last fill, derive or
    /// split (closure, forward, reach-in, unanchored) go back once, as 修正 notes, to the model's own
    /// conversation (#194); the note carries the allow list of the job that wrote the Card, or a new
    /// job of the model's conversation whose allow list names the Card. `model-finish` refuses a model
    /// with any of these faults, and before this step nothing sent them back (a concept model has no
    /// review round at all). A fault in a Card of another model (a shared part of a model this batch
    /// does not hold) is listed under `unowned`.
    /// `-Step defer -Models "a,b"`: a model `model-deps` released to wait (a part it takes from a model of
    /// another batch is not built yet) leaves this batch before the fill, and its plan.json is set aside as
    /// plan-deferred.json, so the batch that claims it again plans it afresh and the ledger finds one plan.
    /// On grossman2002integration trial 9 the fill held jobs for all four models of a batch after three of
    /// them had been released to wait: they would have been written without a claim and built again later.
    fn step_defer(&mut self) -> Result<()> {
        let wanted = self.model_list();
        let deferred = defer_models(&mut self.state, &wanted);
        for model in &deferred {
            let plan = ps::join(&self.dir(model), "plan.json");
            if ps::exists(&plan) {
                let aside = ps::join(&self.dir(model), "plan-deferred.json");
                std::fs::rename(&plan, &aside).with_context(|| format!("set aside the plan of {model}"))?;
            }
            let at = crate::canon::utc_now_o();
            self.state["models"][model]["deferred_at"] = json!(at);
        }
        self.save_state()?;
        self.out_last(&json!({"ok": true, "step": "defer", "deferred": deferred, "order": self.state["order"]}))
    }

    /// `-Step plan-status` (docs/workflow-simplicity-proposal.md S10): read-only. What the architect planned,
    /// read from the files it wrote: one entry per model of the batch, in order, with `status` (planned when its
    /// plan.json exists, skipped when the brief skipped it, else failed), `level`, `parts` and `derivations` (the
    /// parts with `derive: true`). The model layer chooses fill-only or derive-and-review from this line, never
    /// from the architect's answer.
    fn step_plan_status(&mut self) -> Result<()> {
        let mut plans = Vec::new();
        for model in self.order() {
            let path = ps::join(&self.dir(&model), "plan.json");
            let status = s(&self.m(&model)["status"]);
            if ps::exists(&path) {
                let plan = util::read_json(&path).with_context(|| format!("{model}: plan.json is not JSON"))?;
                let parts = list(&plan["parts"]);
                let derivations = parts.iter().filter(|p| truthy(&p["derive"])).count();
                plans.push(json!({"model": model, "status": "planned", "level": plan["level"], "parts": parts.len(), "derivations": derivations}));
            } else {
                plans.push(json!({"model": model, "status": if ps::eq(&status, "skipped") { "skipped" } else { "failed" }, "parts": 0, "derivations": 0}));
            }
        }
        let planned = plans.iter().filter(|p| p["status"] == json!("planned")).count();
        let derive = plans.iter().any(|p| p["derivations"].as_u64().unwrap_or(0) > 0);
        self.out_last(&json!({"ok": true, "step": "plan-status", "planned": planned, "derive": derive, "plans": plans}))
    }

    fn step_close(&mut self) -> Result<()> {
        let refreshed = self.refresh_shared()?;
        self.snapshot_structures()?;
        let mut follow: CiMap<Follow> = CiMap::new();
        let mut unowned: Vec<Value> = Vec::new();
        let mut faults = 0usize;
        for model in self.order() {
            let st = self.model_of(&json!(model))?;
            let mut entries = close_entries(&model, &st["build"]);
            entries.extend(marker_entries(&st));
            let plan_path = ps::join(&self.dir(&model), "plan.json");
            if ps::exists(&plan_path)
                && let Ok(plan) = util::read_json(&plan_path)
            {
                let own = self.m(&model)["ids"].clone();
                let refs = self.m(&model)["refs"].clone();
                let models = self.state["models"].clone();
                let resolve = |reference: &str| -> Option<String> {
                    if let Some((other, part)) = reference.split_once('#') {
                        let from_state = s(&models[other]["ids"][part]);
                        if !from_state.is_empty() {
                            return Some(from_state);
                        }
                        let from_refs = s(&refs[reference]["address"]);
                        return (!from_refs.is_empty()).then_some(from_refs);
                    }
                    let own_id = s(&own[reference]);
                    if !own_id.is_empty() {
                        return if own_id.contains('#') { None } else { Some(own_id) };
                    }
                    Some(reference.to_string())
                };
                let parts: BTreeSet<String> = list(&st["parts"]).into_iter().map(|p| s(&p["address"])).filter(|a| !a.is_empty()).collect();
                let pairs: BTreeSet<(String, String)> = list(&st["relations"]).into_iter().map(|r| (s(&r["from"]), s(&r["to"]))).collect();
                let plan_parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
                entries.extend(planned_link_entries(&model, &plan_parts, &own, &models, &resolve, &parts, &pairs));
            }
            let review_path = ps::join(&self.dir(&model), "source-review.json");
            if ps::exists(&review_path)
                && let Ok(review) = util::read_json(&review_path)
            {
                let ids = self.m(&model)["ids"].clone();
                entries.extend(supplied_entries(&review, &ids, &st));
            }
            for (at, text) in entries {
                faults += 1;
                // #194: a fault in a Card of a model of this batch is never `unowned`: when no job of this
                // batch wrote it, the model's conversation gets a new job whose allow list names the Card.
                let (job, ids, cards) = if ps::eq(&at, &model) {
                    match self.head_job(&model) {
                        Some(job) => (Some(job), Vec::new(), Vec::new()),
                        None => (None, Vec::new(), vec![at.clone()]),
                    }
                } else {
                    match self.find_owner(&at)? {
                        Some((owner, id)) if ps::eq(&owner, &model) => {
                            let job = self.find_job(&model, &id);
                            let cards = if job.is_some() { Vec::new() } else { vec![at.clone()] };
                            (job, vec![id], cards)
                        }
                        _ => (None, Vec::new(), Vec::new()),
                    }
                };
                match (job, cards.is_empty()) {
                    (Some(job), _) => add_follow(&mut follow, Follow { model: model.clone(), job: Some(job), text, ids, cards }),
                    (None, false) => add_follow(&mut follow, Follow { model: model.clone(), job: None, text, ids, cards }),
                    (None, true) => unowned.push(json!({ "model": model, "card": at })),
                }
            }
        }
        let jobs = self.follow_jobs(&follow)?;
        self.save_state()?;
        // `status` names the outcome, so a relay never reads the clean line (no jobs, no faults) as a failed
        // no-op (acemoglu2025nobel trial 1, close 2 of the formal batch).
        let status = if !jobs.is_empty() { "follow_up" } else if faults > 0 || !unowned.is_empty() { "unresolved" } else { "clean" };
        let mut line = json!({ "status": status, "jobs": jobs, "faults": faults, "unowned": unowned, "refreshed": refreshed });
        // -Report: a close round that sends nothing back is the batch's last round, so its report runs here
        // and needs no step of its own (P7 of docs/digest-claude-token-proposal.md). A round that sends
        // follow-ups reports nothing yet: the workflow runs close again, or the report step after its last round.
        if self.p.report && line["jobs"].as_array().is_some_and(Vec::is_empty) {
            line["report"] = self.report_line()?;
        }
        self.out_last(&line)
    }

    /// Rebuilds the 共用 lines of every Model Card of the batch from its plan's taken parts and the
    /// current address map. A split in one model renumbers its later parts, and the 结构 list of another
    /// model that takes one of them in kept the old address (grossman2002integration trial 7: 3|a named
    /// `2|a|a|12` for π_m after a split in 2|a moved it to `2|a|a|13`, and model-finish refused 3|a).
    fn refresh_shared(&mut self) -> Result<usize> {
        let mut edits = Vec::new();
        for model in self.order() {
            if !ps::exists(&ps::join(&self.dir(&model), "plan.json")) {
                continue;
            }
            let plan = self.plan(&model)?;
            let mut wanted = Vec::new();
            for p in list(&plan["parts"]).into_iter().filter(|p| truthy(&p["take"])).cloned().collect::<Vec<_>>() {
                let address = self.resolve(&p["id"], &model)?;
                let facet = if truthy(&p["facet"]) { s(&p["facet"]) } else { "共用".into() };
                wanted.push(format!("- {facet} [[{address}]]（共用）：{}", structure_text(&s(&p["use"]))));
            }
            if wanted.is_empty() {
                continue;
            }
            let card = self.card(&json!(model))?;
            let body = s(&card["body"]);
            let Some(section) = text::get_section(&body, "结构") else { continue };
            if let Some(rebuilt) = shared_lines(&section, &wanted)
                && rebuilt != section
            {
                edits.push(json!({ "tool": "edit_card", "arguments": { "at": model, "body": text::set_section(&body, "结构", &rebuilt) } }));
            }
        }
        let n = edits.len();
        self.edits(edits)?;
        Ok(n)
    }

    /// Keeps each Model Card's 结构 list as the compiler wrote it across a close round: `close` records the
    /// list, and `report` puts it back when a writer re-sent the whole body from an older copy.
    fn snapshot_structures(&mut self) -> Result<()> {
        for model in self.order() {
            let card = self.card(&json!(model))?;
            if let Some(section) = text::get_section(&s(&card["body"]), "结构") {
                self.m_mut(&model)["structure_close"] = json!(section);
            }
        }
        Ok(())
    }

    fn restore_structures(&mut self) -> Result<usize> {
        let mut edits = Vec::new();
        for model in self.order() {
            let kept = s(&self.m(&model)["structure_close"]);
            if kept.is_empty() {
                continue;
            }
            let card = self.card(&json!(model))?;
            let body = s(&card["body"]);
            if text::get_section(&body, "结构").as_deref() != Some(kept.as_str()) {
                edits.push(json!({ "tool": "edit_card", "arguments": { "at": model, "body": text::set_section(&body, "结构", &kept) } }));
            }
            self.m_mut(&model)["structure_close"] = Value::Null;
        }
        let n = edits.len();
        self.edits(edits)?;
        Ok(n)
    }

    /// The job that wrote a model's Model Card (the model's one fill job): the last job whose
    /// addresses name it.
    fn head_job(&self, model: &str) -> Option<String> {
        let mut found = None;
        for k in ps::keys(&self.state["jobs"]) {
            let j = &self.state["jobs"][&k];
            if ps::eq(&s(&j["model"]), model) && strings(&j["addresses"]).iter().any(|a| ps::eq(a, model)) {
                found = Some(k);
            }
        }
        found
    }

    /// The follow-up jobs of a split or close round (#194): every follow-up continues the model's
    /// conversation (`<slug>:brief`), with a fallback to the files of a fresh conversation. The note
    /// carries the ALLOW file of the job that wrote the Card; a Card no job of this batch wrote gets a
    /// new job whose own allow list names it, so a Card of a model of this batch is never `unowned`.
    fn follow_jobs(&mut self, follow: &CiMap<Follow>) -> Result<Vec<Value>> {
        let mut jobs = Vec::new();
        let entries: Vec<(String, String, String, Vec<String>, Vec<String>)> = follow
            .iter()
            .map(|(k, f)| (k.clone(), f.model.clone(), f.text.clone(), f.ids.clone(), f.cards.clone()))
            .collect();
        for (key, model, text, ids, cards) in entries {
            let seq = self.next_seq();
            let dir = self.dir(&model);
            let file = ps::join(&dir, &format!("follow-{seq}.md"));
            let allow = if ps::like(&key, "new:*") { Value::Null } else { self.state["jobs"][&key]["allow"].clone() };
            if !truthy(&allow) {
                let mut blocks = Vec::new();
                let mut n = 0;
                for id in &ids {
                    if let Some(p) = self.plan_part(&model, id)? {
                        n += 1;
                        blocks.push(self.part_block(n, &p, &model, true)?);
                    }
                }
                let ids = ids.iter().map(|i| json!(i)).collect();
                let mut job = self.new_job(
                    &model,
                    "d",
                    ids,
                    &format!("{text}\n{}", blocks.join("\n")),
                    "按以上说明完成你的分卡，最后每张分卡回复一行 JSON。",
                )?;
                self.add_cards_to_job(&mut job, &cards)?;
                self.continue_model(&mut job, &model, &self.docs.fill)?;
                jobs.push(job);
                continue;
            }
            let allow = s(&allow);
            util::write_text(&file, &format!("# 补充任务（接着你刚才的分卡写）\n\nALLOW 已更新：{allow}\n\n{text}\n写完后照常运行 CHECK（地址加上新分卡）。"))?;
            // fallback: the files of a fresh conversation, when the model's conversation is gone
            let fallback = json!([self.docs.fill, ps::join(&dir, "setting.md"), self.state["jobs"][&key]["file"], file]);
            jobs.push(json!({
                "id": format!("{key}+{seq}"),
                "continue": format!("{}:brief", Self::slug(&model)),
                "files": [file],
                "fallback": fallback,
                "prompt": "按以上补充说明修改和续写你的分卡，最后每张分卡回复一行 JSON。",
            }));
        }
        Ok(jobs)
    }

    /// Names extra Cards in a job's allow list and addresses (a note for a Card no job wrote).
    fn add_cards_to_job(&mut self, job: &Value, cards: &[String]) -> Result<()> {
        if cards.is_empty() {
            return Ok(());
        }
        let job_id = s(&job["id"]);
        let mut addresses = strings(&self.state["jobs"][&job_id]["addresses"]);
        for at in cards {
            if !addresses.contains(at) {
                addresses.push(at.clone());
            }
        }
        self.state["jobs"][&job_id]["addresses"] = json!(addresses);
        let allow = s(&self.state["jobs"][&job_id]["allow"]);
        util::write_text(&allow, &(addresses.join("\n") + "\n"))?;
        Ok(())
    }

    fn step_report(&mut self) -> Result<()> {
        let line = self.report_line()?;
        self.out_last(&line)
    }

    /// The batch report: report.json with every part, and the short line a relay reads.
    fn report_line(&mut self) -> Result<Value> {
        // A close writer may have re-sent a whole Model Card from an older copy: its 结构 list goes back
        // to the one the compiler wrote, and the 共用 lines follow the current addresses.
        self.restore_structures()?;
        self.refresh_shared()?;
        let mut reports = Vec::new();
        for model in self.order() {
            let st = self.model_of(&json!(model))?;
            let mut style: i64 = 0;
            if !list(&st["parts"]).is_empty() {
                // Shared parts belong to their home models, which this batch does not change.
                let check = Check {
                    model: model.clone(),
                    skip_shared: true,
                    max_sentences: self.p.max_sentences,
                    max_chars: self.p.max_chars,
                    max_steps: self.p.max_steps,
                    max_explain_sentences: self.p.max_explain_sentences,
                    max_explain_chars: self.p.max_explain_chars,
                    ..Check::default()
                };
                let lines = check.run(self.zt()?)?;
                let last = lines.iter().filter(|l| FAULTS_LINE.is_match(l)).next_back().cloned().unwrap_or_default();
                style = if last.is_empty() { -1 } else { ps::int(&json!(FAULTS_PREFIX.replace(&last, "").into_owned()))? };
            }
            let mut kinds: CiMap<i64> = CiMap::new();
            for r in list(&st["relations"]) {
                let k = s(&r["kind"]);
                let n = kinds.get(&k).copied().unwrap_or(0);
                kinds.insert(&k, n + 1);
            }
            let kinds: Map<String, Value> = kinds.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
            let order = list(&st["build"]["order"]);
            let chains = list(&st["build"]["chains"]);
            let m = self.m(&model).clone();
            let status = s(&m["status"]);
            let status = if ps::is_in(&status, &["failed", "skipped"]) && !m["status"].is_null() {
                m["status"].clone()
            } else if ps::eq(&status, "brief") && !m["status"].is_null() {
                json!("failed")
            } else {
                json!("done")
            };
            let parts: Vec<Value> = order
                .iter()
                .map(|o| json!({ "address": o["address"], "facet": o["facet"], "emphasis": o["emphasis"], "line": o["line"] }))
                .collect();
            let charts = pipe(&st["parts"]).into_iter().filter(|p| CHART.is_match(&s(&p["body"]))).count();
            let todo: Vec<Value> = pipe(&st["parts"]).into_iter().filter(|p| TODO_LINE.is_match(&s(&p["body"]))).map(|p| p["address"].clone()).collect();
            let total = order.iter().filter(|o| truthy(&o["derivation"]) && !ps::eq(&s(&o["line"]), "shared")).count();
            let longest = chains.iter().map(|c| ps::int(&c["length"]).unwrap_or(0)).chain([0]).max().unwrap_or(0);
            let shared = order.iter().filter(|o| ps::eq(&s(&o["line"]), "shared") && !o["line"].is_null()).count();
            let count = |v: &Value| list(v).len();
            let build = &st["build"];
            let issues: Vec<Value> = pipe(&st["issues"])
                .into_iter()
                .map(|i| match i {
                    Value::String(t) => json!(t),
                    other => json!(ps::json_compact(other)),
                })
                .collect();
            reports.push(json!({
                "model": model,
                "level": st["level"],
                "status": status,
                "error": m["error"],
                "parts": parts,
                "relations": count(&st["relations"]),
                "kinds": kinds,
                "neighbours": count(&st["neighbours"]),
                "loose": count(&st["loose"]),
                "charts": charts,
                "todo": todo,
                "derivations": {
                    "total": total,
                    "explained": Self::explained_of(&st).len(),
                    "chains": chains.iter().filter(|c| c["length"].as_f64().is_some_and(|l| l >= 2.0)).count(),
                    "longest": longest,
                },
                "split": ps::int(&m["split"]).unwrap_or(0),
                "moved": list(&m["moved"]),
                "shared": shared,
                "tolerated": count(&build["tolerated"]),
                "style": style,
                "faults": {
                    "closure": count(&build["closure"]),
                    "forward": count(&build["forward"]),
                    "unanchored": count(&build["unanchored"]),
                    "beyond_level": count(&build["beyond_level"]),
                },
                "issues": issues,
                "notes": list(&m["notes"]),
            }));
            self.record_step(
                &model,
                "report",
                json!({
                    "status":s(&self.m(&model)["status"]),
                    "parts":reports.last().map(|r| list(&r["parts"]).len()).unwrap_or(0),
                    "todo":todo.len(),
                    "derivations":total,
                    "explained":Self::explained_of(&st).len(),
                    "style":style,
                    "build_faults":count(&build["closure"])+count(&build["forward"])+count(&build["unanchored"])+count(&build["beyond_level"])
                }),
            );
        }
        self.save_state()?;
        // The full report (every part with its facet) goes to report.json; the printed line is relayed
        // by an agent and carries counts only. On grossman2002integration trial 12 the relay cut three
        // models' part lists to a note, and the models stage threw on it.
        let file = ps::join(&self.p.work, "report.json");
        util::write_text(&file, &ps::json_pretty(&json!({ "models": reports })))?;
        let brief: Vec<Value> = reports.iter().map(Self::brief_report).collect();
        Ok(json!({ "models": brief, "file": file }))
    }

    /// One model's report line for the relay: the part list becomes its count and its core count,
    /// the moved Cards their count.
    fn brief_report(report: &Value) -> Value {
        let mut brief = report.clone();
        let parts = list(&report["parts"]);
        brief["parts"] = json!(parts.len());
        brief["core"] = json!(parts.iter().filter(|p| ps::eq(&s(&p["emphasis"]), "core")).count());
        brief["moved"] = json!(list(&report["moved"]).len());
        brief
    }

    // --- resume ---------------------------------------------------------------------------------

    /// R235/R236: finish a model that a stopped batch left partly written. The accepted plan
    /// and its part map are reused; only the first missing step's own work is offered, in build
    /// order, so a later fill never starts from a premise that is still `待写`.
    fn step_resume(&mut self) -> Result<()> {
        let mut jobs = Vec::new();
        let mut reports = Vec::new();
        for model in self.model_list() {
            let report = self.resume_model(&model, &mut jobs)?;
            reports.push(report);
            self.save_state()?;
        }
        self.save_state()?;
        self.out_last(&json!({ "models": reports, "jobs": jobs }))
    }

    fn resume_model(&mut self, model: &str, jobs: &mut Vec<Value>) -> Result<Value> {
        let plan_path = ps::join(&self.dir(model), "plan.json");
        if !ps::exists(&plan_path) {
            bail!("{model}: no accepted plan at {plan_path}; plan and build the model first");
        }
        let state = self.m(model).clone();
        let status = s(&state["status"]);
        if !ps::is_in(&status, &["built", "derived", "reviewed"]) {
            bail!(
                "{model}: no accepted build in state.json (status `{status}`); plan and build the model first"
            );
        }
        let ids = state["ids"]
            .as_object()
            .filter(|ids| !ids.is_empty())
            .with_context(|| format!("{model}: no accepted part map; plan and build the model first"))?;
        let plan = self.plan(model)?;
        if !ps::eq(&s(&plan["model"]), model) {
            bail!("{model}: the accepted plan describes `{}`", s(&plan["model"]));
        }
        let plan_hash = Self::file_hash(&plan_path)?;
        if truthy(&state["plan_hash"]) && !ps::eq(&s(&state["plan_hash"]), &plan_hash) {
            bail!("{model}: plan.json changed after the accepted build; keep the accepted plan or start a deliberate rebuild");
        }
        let structure = self.model_of(&json!(model))?;
        let order: Vec<String> = list(&structure["build"]["order"])
            .into_iter()
            .map(|o| s(&o["address"]))
            .collect();
        let mut reused = serde_json::Map::new();
        let mut missing: Vec<String> = Vec::new();
        let mut fill: Vec<String> = Vec::new();
        let mut derive: Vec<String> = Vec::new();
        let mut missing_id = std::collections::BTreeMap::new();
        let plan_parts = list(&plan["parts"]).into_iter().cloned().collect::<Vec<_>>();
        for part in &plan_parts {
            let id = s(&part["id"]);
            if id.is_empty() {
                continue;
            }
            let address = match ids.get(&id) {
                Some(value) => s(value),
                None => String::new(),
            };
            if address.is_empty() {
                bail!("{model}: plan id `{id}` has no address in the part map");
            }
            let card = match self.card_or_missing(&address)? {
                Some(card) => card,
                None => bail!("missing part: plan id `{id}` names `{address}`, which the archive does not hold; keep the accepted part map or rebuild the model as a new revision"),
            };
            let expected = if truthy(&state["facets"][&id]) { s(&state["facets"][&id]) } else { s(&part["facet"]) };
            let actual = s(&card["facet"]);
            if !expected.trim().is_empty() && !ps::eq(actual.trim(), expected.trim()) {
                bail!("stale part: `{address}` has facet `{actual}`, the accepted plan says `{expected}`; keep the accepted plan's part or re-plan the model");
            }
            let written = !TODO_LINE.is_match(&s(&card["body"]));
            if written {
                reused.insert(address.clone(), json!(crate::calls::authored_fingerprint(&card)));
                continue;
            }
            missing.push(address.clone());
            missing_id.insert(address.clone(), id);
            if truthy(&part["take"]) {
                continue;
            }
            if derives(part) {
                derive.push(address);
            } else {
                fill.push(address);
            }
        }
        // A shared part belongs to its home model: this model waits instead of writing it.
        for address in &missing {
            if let Some(id) = missing_id.get(address)
                && plan_parts.iter().any(|p| ps::eq(&s(&p["id"]), id) && truthy(&p["take"]))
            {
                bail!("shared part {address} (plan id `{id}`) is still unwritten in its home model; finish that model instead of writing its Card here");
            }
        }
        let head_card = match self.card_or_missing(model)? {
            Some(card) => card,
            None => bail!("missing part: the Model Card `{model}` is gone; keep the accepted part map or rebuild the model as a new revision"),
        };
        let head_missing = TODO_LINE.is_match(&s(&head_card["body"])) || s(&head_card["body"]).trim().is_empty();
        if !head_missing {
            reused.insert(model.to_string(), json!(crate::calls::authored_fingerprint(&head_card)));
        }
        let from = if head_missing {
            Some(model.to_string())
        } else {
            order.iter().find(|address| missing.iter().any(|m| m == *address)).cloned()
        };
        if from.is_none() && !missing.is_empty() {
            bail!("{model}: the part map names Cards the checked model does not hold; rebuild the model as a new revision");
        }
        let mut emitted = Vec::new();
        let mut round = "none";
        let mut emitted_fill: Vec<String> = Vec::new();
        let mut emitted_derive: Vec<String> = Vec::new();
        if head_missing {
            round = "fill";
            // #194: one job per model holds the Model Card and every missing fill part, continuing the
            // model's conversation; a resume's own batch has no session table, so the job runs once from
            // its fallback files.
            let start = from.as_ref().and_then(|first| fill.iter().position(|a| a == first)).unwrap_or(0);
            let mut todo: Vec<Value> = Vec::new();
            for address in fill.iter().skip(start) {
                let id = missing_id.get(address).context("missing part map id")?;
                let part = plan_parts.iter().find(|p| ps::eq(&s(&p["id"]), id)).context("missing plan part")?;
                todo.push(part.clone());
            }
            let job = self.fill_job(model, &plan, &todo)?;
            let addresses = self.state["jobs"][s(&job["id"])]["addresses"].clone();
            for address in list(&addresses) {
                emitted_fill.push(s(address));
            }
            emitted.push(json!({"tool":"fill","job":job["id"],"addresses":addresses}));
            jobs.push(job);
        } else if let Some(first) = from.clone() {
            if fill.iter().any(|a| *a == first) {
                round = "fill";
                let start = fill.iter().position(|a| *a == first).unwrap_or(0);
                let mut blocks = Vec::new();
                let mut plan_ids = Vec::new();
                for (n, address) in fill.iter().skip(start).enumerate() {
                    let key: &str = address;
                    let id = missing_id.get(key).context("missing part map id")?;
                    let part = plan_parts.iter().find(|p| ps::eq(&s(&p["id"]), id)).context("missing plan part")?;
                    plan_ids.push(json!(id));
                    blocks.push(self.part_block(n + 1, part, model, false)?);
                }
                let mut job = self.new_job(model, "f", plan_ids, &blocks.join("\n"), "按以上说明完成你的任务，最后每张分卡回复一行 JSON。")?;
                self.continue_model(&mut job, model, &self.docs.fill)?;
                let addresses = self.state["jobs"][s(&job["id"])]["addresses"].clone();
                for address in list(&addresses) {
                    emitted_fill.push(s(address));
                }
                emitted.push(json!({"tool":"fill","job":job["id"],"addresses":addresses}));
                jobs.push(job);
            } else {
                round = "derive";
                let start = derive.iter().position(|a| *a == first).unwrap_or(0);
                let mut blocks = Vec::new();
                let mut plan_ids = Vec::new();
                for (n, address) in derive.iter().skip(start).enumerate() {
                    let key: &str = address;
                    let id = missing_id.get(key).context("missing part map id")?;
                    let part = plan_parts.iter().find(|p| ps::eq(&s(&p["id"]), id)).context("missing plan part")?;
                    plan_ids.push(json!(id));
                    blocks.push(self.part_block(n + 1, part, model, true)?);
                }
                let mut job = self.new_job(model, "d", plan_ids, &blocks.join("\n"), "按以上说明完成你的推导分卡，最后每张分卡回复一行 JSON。")?;
                self.continue_model(&mut job, model, &self.docs.fill)?;
                let addresses = self.state["jobs"][s(&job["id"])]["addresses"].clone();
                for address in list(&addresses) {
                    emitted_derive.push(s(address));
                }
                emitted.push(json!({"tool":"derive","job":job["id"],"addresses":addresses}));
                jobs.push(job);
            }
        }
        let complete = !head_missing && missing.is_empty();
        let status_now = if round == "derive" { "derived".to_string() } else { status.clone() };
        if status_now != status {
            self.m_mut(model)["status"] = json!(status_now);
        }
        self.record_step(
            model,
            "resume",
            json!({"round":round,"from":from,"missing":missing.len(),"fill":emitted_fill.len(),"derive":emitted_derive.len(),"reused":reused.len(),"complete":complete,"jobs":emitted.len()}),
        );
        Ok(json!({
            "model": model,
            "status": self.m(model)["status"],
            "plan_hash": plan_hash,
            "from": from,
            "missing": missing,
            "fill": emitted_fill,
            "derive": emitted_derive,
            "reused": reused,
            "complete": complete,
            "round": round,
            "jobs": emitted,
        }))
    }

    /// The Model Card's block in the model's one fill job.
    fn head_text(&mut self, model: &str, plan: &Value) -> Result<String> {
        let card = self.card(&json!(model))?;
        let id_values: Vec<String> = self.m(model)["ids"].as_object().map(|ids| ids.values().map(s).collect()).unwrap_or_default();
        let outside: Vec<String> = pipe(&card["outbound"])
            .into_iter()
            .filter(|o| !truthy(&o["broken"]) && !ps::is_in(&s(&o["address"]), &id_values))
            .map(|o| format!("- [[{}]] {}", s(&o["address"]), s(&o["title"])))
            .collect();
        let mut card_text = format!("## 模型卡：[[{model}]]\n\n- 要写：问题、概要（三到五句）、结构、关联、来源。保留 **结构** 一节原样。\n- 内容：{}\n", s(&plan["card"]));
        if !outside.is_empty() {
            card_text += &format!("- 保留这些模型外的链接，每个放进一句说明关系的话，句首用「参照」「对应」「对照」或「例示」，不用「依赖」「基于」（模型只依赖自己的分卡）：\n{}\n", outside.join("\n"));
        }
        if ps::eq(&s(&plan["level"]), "concept") {
            card_text += "- 概念模型：用文字说清问题、机制和结论，不写推导。\n";
        }
        let mut current = text::set_section(&s(&card["body"]), "结构", "").trim().to_string();
        if !current.is_empty() {
            if ps::len16(&current) > 4000 {
                current = ps::cut16(&current, 4000) + "……";
            }
            card_text += &format!("\n### 模型卡现在的正文（结构一节除外）\n{current}\n");
        }
        Ok(card_text)
    }

    /// Round 1 of a model: ONE job holding the Model Card and every part the plan fills, in build
    /// order, continuing the model's conversation (#194). Its allow list names the Model Card and
    /// every part, and the state's `jobs` map records them, so `close` still finds both.
    fn fill_job(&mut self, model: &str, plan: &Value, todo: &[Value]) -> Result<Value> {
        let mut text = self.head_text(model, plan)?;
        for (n, p) in todo.iter().enumerate() {
            text += "\n";
            text += &self.part_block(n + 1, p, model, false)?;
        }
        let ids: Vec<Value> = todo.iter().map(|p| p["id"].clone()).collect();
        let mut job = self.new_job(model, "f", ids, &text, "按以上说明完成你的任务，最后每张分卡回复一行 JSON。")?;
        let job_id = s(&job["id"]);
        let mut addresses = vec![json!(model)];
        addresses.extend(list(&self.state["jobs"][&job_id]["addresses"]).into_iter().cloned());
        self.state["jobs"][&job_id]["addresses"] = Value::Array(addresses.clone());
        let allow = s(&self.state["jobs"][&job_id]["allow"]);
        util::write_text(&allow, &(addresses.iter().map(s).collect::<Vec<_>>().join("\n") + "\n"))?;
        self.continue_model(&mut job, model, &self.docs.fill)?;
        Ok(job)
    }

    /// A content job that continues the model's conversation: `continue` names the brief's job id,
    /// and `fallback` holds the files of a fresh conversation when that session is gone (#194).
    fn continue_model(&self, job: &mut Value, model: &str, first: &str) -> Result<()> {
        let job_id = s(&job["id"]);
        let dir = self.dir(model);
        job["continue"] = json!(format!("{}:brief", Self::slug(model)));
        job["fallback"] = json!([first, ps::join(&dir, "setting.md"), self.state["jobs"][&job_id]["file"]]);
        Ok(())
    }

    // --- explain --------------------------------------------------------------------------------

    /// `Get-OwnDerivations`: the model's own Derivation parts, in build order.
    fn own_derivations(st: &Value) -> Vec<String> {
        list(&st["build"]["order"])
            .into_iter()
            .filter(|o| truthy(&o["derivation"]) && !ps::eq(&s(&o["line"]), "shared"))
            .map(|o| s(&o["address"]))
            .collect()
    }

    /// The own Derivation parts whose 思路 does not start with the chain position line the
    /// current chain and results give them. A chain of one part needs no position line.
    fn misplaced(st: &Value, own: &[String], body: &dyn Fn(&str) -> String) -> Vec<String> {
        let mut out = Vec::new();
        for c in list(&st["build"]["chains"]) {
            let parts: Vec<String> = pipe(&c["parts"]).into_iter().map(s).filter(|a| ps::is_in(a, own)).collect();
            let tos: Vec<String> = parts.iter().map(|a| text::result_of(&body(a))).collect();
            for (index, a) in parts.iter().enumerate() {
                let expected = text::chain_line(index + 1, parts.len(), &tos);
                if !expected.is_empty() && text::position_of(&body(a)) != expected {
                    out.push(a.clone());
                }
            }
        }
        out
    }

    fn bodies_of(st: &Value) -> CiMap<String> {
        let mut bodies = CiMap::new();
        for p in list(&st["parts"]) {
            bodies.insert(&s(&p["address"]), s(&p["body"]));
        }
        bodies
    }

    /// `Get-Explained`.
    fn explained_of(st: &Value) -> Vec<String> {
        let bodies = Self::bodies_of(st);
        Self::own_derivations(st).into_iter().filter(|a| text::explained(&bodies.get(a).cloned().unwrap_or_default())).collect()
    }

    /// `Get-ExplainCheckCommand`: the SELF-CHECK line of an explain job file.
    fn explain_check_command(&self, id: &str) -> String {
        format!(
            "zt-digest model -Step explain-check -Work \"{}\" -Job \"{id}\" -MaxSentences {} -MaxChars {} -MaxSteps {} {}",
            self.p.work, self.p.max_sentences, self.p.max_chars, self.p.max_steps, self.explain_args
        )
    }

    /// `Get-ExplainState`.
    fn explain_state(&mut self) {
        if !truthy(&self.state["explain"]) {
            self.state["explain"] = json!({ "models": [], "settings": {}, "jobs": {}, "refused": {}, "blocked": {} });
        }
    }

    fn step_explain(&mut self) -> Result<()> {
        self.explain_state();
        let mut jobs = Vec::new();
        let mut left: i64 = if self.p.max_parts > 0 { self.p.max_parts } else { i32::MAX as i64 };
        let mut pending: CiMap<bool> = CiMap::new();
        for k in ps::keys(&self.state["explain"]["jobs"]) {
            let j = &self.state["explain"]["jobs"][&k];
            let done = strings(&j["done"]);
            for a in pipe(&j["addresses"]) {
                if !ps::is_in(&s(a), &done) {
                    pending.insert(&s(a), true);
                }
            }
        }
        for model in self.model_list() {
            if left <= 0 {
                break;
            }
            let st = self.model_of(&json!(model))?;
            if !truthy(&st["head"]) || (ps::eq(&s(&st["level"]), "concept") && !st["level"].is_null()) {
                continue;
            }
            let bodies = Self::bodies_of(&st);
            let mut titles: CiMap<String> = CiMap::new();
            for p in list(&st["parts"]) {
                titles.insert(&s(&p["address"]), s(&p["title"]));
            }
            let body = |a: &str| bodies.get(a).cloned().unwrap_or_default();
            let own = Self::own_derivations(&st);
            // A part whose 思路 is not where the current chain puts it is unfinished too: the
            // writer's answer rewrites the position line from the chain (R301).
            let misplaced: Vec<String> = Self::misplaced(&st, &own, &body);
            let missing: Vec<String> = own
                .iter()
                .filter(|a| (!text::explained(&body(a)) || misplaced.contains(a)) && !pending.contains(a))
                .cloned()
                .collect();
            if missing.is_empty() {
                continue;
            }
            let dir = self.dir(&model);
            if !ps::is_in(&model, &strings(&self.state["explain"]["models"])) {
                let mut models: Vec<Value> = pipe(&self.state["explain"]["models"]).into_iter().cloned().collect();
                models.push(json!(model));
                self.state["explain"]["models"] = Value::Array(models);
            }
            self.state["explain"]["settings"][&model] = json!(s(&st["build"]["setting"]));
            let setting = self.card(&st["build"]["setting"])?;
            util::write_text(
                ps::join(&dir, "setting.md"),
                &format!("# 本模型的设定：[[{}]] {}\n\n{}\n", s(&setting["address"]), s(&setting["title"]), s(&setting["body"])),
            )?;
            let mut context = "无".to_string();
            if !self.p.staging.is_empty() {
                context = ps::join(&dir, "context.md");
                if self.context(&model, &context).is_err() {
                    context = "无".into();
                }
            }
            let head = self.card(&json!(model))?;
            let mut head_body = s(&head["body"]);
            if ps::len16(&head_body) > 3000 {
                head_body = ps::cut16(&head_body, 3000) + "……";
            }
            // Groups: up to three neighbouring parts of one chain, with the members around them.
            let mut groups: Vec<Group> = Vec::new();
            let mut covered: CiMap<bool> = CiMap::new();
            for c in list(&st["build"]["chains"]) {
                let parts: Vec<String> = pipe(&c["parts"]).into_iter().map(s).filter(|a| ps::is_in(a, &own)).collect();
                let tos: Vec<String> = parts.iter().map(|a| text::result_of(&body(a))).collect();
                let todo: Vec<String> = parts.iter().filter(|a| ps::is_in(a, &missing)).cloned().collect();
                for g in todo.chunks(3) {
                    groups.push(Group { parts: g.to_vec(), chain: parts.clone(), tos: tos.clone(), after: c["after"].clone(), before: c["before"].clone() });
                    for a in g {
                        covered.insert(a, true);
                    }
                }
            }
            for a in missing.iter().filter(|a| !covered.contains(a)) {
                groups.push(Group {
                    parts: vec![a.clone()],
                    chain: vec![a.clone()],
                    tos: vec![text::result_of(&body(a))],
                    after: Value::Null,
                    before: Value::Null,
                });
            }
            for mut g in groups {
                if left <= 0 {
                    break;
                }
                g.parts.truncate(left as usize);
                left -= g.parts.len() as i64;
                let seq = self.next_seq();
                let id = format!("{}:x{seq}", Self::slug(&model));
                let out = ps::join(&ps::join(&self.p.work, "out"), &format!("explain-{seq}.json"));
                let file = ps::join(&dir, &format!("explain-{seq}.md"));
                let mut text = format!(
                    "# 讲解任务 {id}\n\n结果文件: {out}\nSELF-CHECK: {}\n上下文包: {context}\n模型卡: [[{model}]] {}\n\n## 模型卡的正文\n\n{head_body}\n\r\n",
                    self.explain_check_command(&id),
                    s(&head["title"])
                );
                let around = |at: &Value| -> Option<(String, String, String)> {
                    if !truthy(at) {
                        return None;
                    }
                    let a = s(at);
                    let b = body(&a);
                    if b.is_empty() {
                        return None;
                    }
                    Some((a.clone(), titles.get(&a).cloned().unwrap_or_default(), b))
                };
                if let Some((a, t, b)) = around(&g.after) {
                    text += &format!("## 链之前的分卡（推导从它出发）：[[{a}]] {t}\n\n{b}\n\r\n");
                }
                let mut positions = Map::new();
                let n = g.chain.len();
                for a in &g.parts {
                    let k = (ps::index_of(&g.chain, a) + 1) as usize;
                    positions.insert(a.clone(), json!(text::chain_line(k, n, &g.tos)));
                    let place = if n >= 2 { format!("（第 {k} 张，共 {n} 张）") } else { String::new() };
                    text += &format!("## 推导分卡 [[{a}]] {}{place}\n\n{}\n\r\n", titles.get(a).cloned().unwrap_or_default(), body(a));
                }
                if let Some((a, t, b)) = around(&g.before) {
                    text += &format!("## 链之后的分卡（它用推导的结果）：[[{a}]] {t}\n\n{b}\n\r\n");
                }
                util::write_text(&file, &text)?;
                let mut job_bodies = Map::new();
                for a in &g.parts {
                    job_bodies.insert(a.clone(), json!(body(a)));
                }
                self.state["explain"]["jobs"][&id] = json!({
                    "model": model, "file": file, "out": out, "addresses": g.parts, "done": [], "round": 1,
                    "bodies": job_bodies, "positions": positions,
                });
                jobs.push(json!({
                    "id": id,
                    "files": [self.docs.explain, ps::join(&dir, "setting.md"), file],
                    "prompt": format!("按以上说明写出结果文件 {out}，最后只回复一行 JSON。"),
                }));
            }
        }
        std::fs::create_dir_all(ps::join(&self.p.work, "out"))?;
        self.save_state()?;
        // Measure-Object -Sum: a double.
        let parts = jobs.iter().fold(0.0_f64, |sum, j| sum + pipe(&self.state["explain"]["jobs"][s(&j["id"])]["addresses"]).len() as f64);
        self.out_last(&json!({ "jobs": jobs, "parts": parts }))
    }

    /// `Test-ExplainJob`: what explain-apply would do with one job's result
    /// file, without a write.
    fn test_explain_job(&mut self, id: &str, file: &str) -> Result<ExplainCheck> {
        self.explain_state();
        let j = self.state["explain"]["jobs"][id].clone();
        if !truthy(&j) {
            bail!("no explain job {id}");
        }
        let mut r = ExplainCheck::default();
        let done = strings(&j["done"]);
        let todo: Vec<String> = pipe(&j["addresses"]).into_iter().map(s).filter(|a| !ps::is_in(a, &done)).collect();
        if todo.is_empty() {
            return Ok(r);
        }
        let mut entries: Vec<Value> = Vec::new();
        if ps::exists(file) {
            match util::read_json(file) {
                Ok(v) => entries = pipe(&v).into_iter().cloned().collect(),
                Err(e) => {
                    for a in &todo {
                        r.faults.insert(a, format!("the result file is not a JSON array: {e:#}"));
                    }
                }
            }
        } else {
            for a in &todo {
                r.faults.insert(a, "no result file".into());
            }
        }
        let calls: Vec<(&str, Value)> = todo.iter().map(|a| ("get_card", json!({ "at": a }))).collect();
        let mut cards: CiMap<Value> = CiMap::new();
        for c in self.reads(&calls)? {
            cards.insert(&s(&c["address"]), c);
        }
        for a in &todo {
            if r.faults.contains(a) {
                continue;
            }
            let Some(e) = entries.iter().find(|e| truthy(e) && ps::eq(&s(&e["address"]), a)) else {
                r.faults.insert(a, "no entry for this part in the result file".into());
                continue;
            };
            if truthy(&e["blocked"]) {
                r.blocked.push(format!("{a}: {}", s(&e["blocked"])));
                continue;
            }
            let card = cards.get(a).cloned().unwrap_or(Value::Null);
            let body = s(&card["body"]);
            if !ps::eq(body.trim(), s(&j["bodies"][a]).trim()) {
                r.refused.push(format!("{a}: the part changed after its job"));
                continue;
            }
            if s(&e["idea"]).trim().is_empty() || s(&e["meaning"]).trim().is_empty() {
                r.faults.insert(a, "idea and meaning must not be empty".into());
                continue;
            }
            let new = text::add_explanation(&body, &s(&e["idea"]), &e["notes"], &s(&e["meaning"]), &s(&j["positions"][a]));
            if !ps::eq(&text::core(&new), &text::core(&body)) {
                r.refused.push(format!("{a}: inserting the explanation would change 起点, a step, or 结果"));
                continue;
            }
            r.candidates.push(Candidate { address: a.clone(), title: s(&card["title"]), facet: s(&card["facet"]), body: new, old: body });
        }
        if r.candidates.is_empty() {
            return Ok(r);
        }
        // The new bodies pass R2 to R19; the setting gives the term table (R6). Only a fault the
        // explanation adds counts: a fault the old body already has stays and is reported as kept.
        let setting = s(&self.state["explain"]["settings"][&s(&j["model"])]);
        let setting_card = if setting.is_empty() { Value::Null } else { self.card(&json!(setting))? };
        let old_items: Vec<Value> =
            r.candidates.iter().map(|c| json!({ "address": c.address, "title": c.title, "body": c.old, "facet": c.facet })).collect();
        let new_items: Vec<Value> =
            r.candidates.iter().map(|c| json!({ "address": c.address, "title": c.title, "body": c.body, "facet": c.facet })).collect();
        let old = self.style_faults(old_items, "old.json", false, &setting, &setting_card)?;
        let all = self.style_faults(new_items, "new.json", true, &setting, &setting_card)?;
        let mut pass = Vec::new();
        for c in std::mem::take(&mut r.candidates) {
            let mut before: Vec<String> = old.get(&c.address).cloned().unwrap_or_default();
            let now: Vec<String> = all.get(&c.address).cloned().unwrap_or_default();
            let mut added = Vec::new();
            for f in &now {
                match before.iter().position(|b| b == f) {
                    Some(i) => {
                        before.remove(i);
                    }
                    None => added.push(f.clone()),
                }
            }
            let was: Vec<String> = now.iter().filter(|f| !ps::is_in(f, &added)).cloned().collect();
            if !was.is_empty() {
                r.kept.push(format!("{}: {}", c.address, was.join(" | ")));
            }
            if !added.is_empty() {
                r.faults.insert(&c.address, added.join(" | "));
            } else {
                pass.push(c);
            }
        }
        r.candidates = pass;
        Ok(r)
    }

    /// The R faults zt-check finds in these entries (with the setting for its term table).
    fn style_faults(&mut self, items: Vec<Value>, name: &str, explain: bool, setting: &str, setting_card: &Value) -> Result<CiMap<Vec<String>>> {
        let mut to_check = items;
        if !setting.is_empty() {
            to_check.push(json!({
                "address": setting,
                "title": s(&setting_card["title"]),
                "body": s(&setting_card["body"]),
                "facet": s(&setting_card["facet"]),
            }));
        }
        let temp = std::env::temp_dir().display().to_string();
        let entries_file = ps::join(&temp, &format!("explain-{}-{name}", std::process::id()));
        util::write_text(&entries_file, &ps::json_pretty(&Value::Array(to_check)))?;
        let check = Check {
            json: entries_file.clone(),
            setting: setting.to_string(),
            max_sentences: self.p.max_sentences,
            max_chars: self.p.max_chars,
            max_steps: self.p.max_steps,
            max_explain_sentences: self.p.max_explain_sentences,
            max_explain_chars: self.p.max_explain_chars,
            require_explain: explain,
            ..Check::default()
        };
        // zt-check failing to run is an error: nothing unchecked is written.
        let checked = check.run(self.zt()?);
        let _ = std::fs::remove_file(&entries_file);
        let mut out: CiMap<Vec<String>> = CiMap::new();
        for line in checked? {
            if let Some(c) = STYLE_FAULT.captures(&line)
                && !ps::eq(&c[1], setting)
            {
                let mut faults = out.get(&c[1]).cloned().unwrap_or_default();
                faults.push(c[2].to_string());
                out.insert(&c[1], faults);
            }
        }
        Ok(out)
    }

    fn step_explain_check(&mut self) -> Result<()> {
        self.explain_state();
        let job = self.p.job.clone();
        if job.is_empty() || !self.state["explain"]["jobs"].as_object().is_some_and(|j| j.contains_key(&job)) {
            bail!("explain-check needs -Job <an explain job id>");
        }
        let file = if self.p.file.is_empty() { s(&self.state["explain"]["jobs"][&job]["out"]) } else { self.p.file.clone() };
        let r = self.test_explain_job(&job, &file)?;
        let mut lines = Vec::new();
        for (a, f) in r.faults.iter() {
            lines.push(format!("{a} explain {f}"));
        }
        for t in &r.refused {
            lines.push(REFUSED.replace(t, "${1} refused ").into_owned());
        }
        for t in &r.blocked {
            lines.push(format!("(blocked, not counted) {t}"));
        }
        for t in &r.kept {
            lines.push(format!("(kept: the old body has these faults; not yours, not counted) {t}"));
        }
        for c in &r.candidates {
            lines.push(format!("{} ok", c.address));
        }
        lines.push(format!("faults: {}", r.faults.len() + r.refused.len()));
        util::print_lines(&lines)
    }

    fn step_explain_apply(&mut self) -> Result<()> {
        self.explain_state();
        let mut applied = 0usize;
        let mut refused: Vec<(String, String)> = Vec::new();
        let mut blocked: Vec<(String, String)> = Vec::new();
        let mut jobs = Vec::new();
        let mut edits = Vec::new();
        let mut done: Vec<(String, String)> = Vec::new();
        let mut kept: Vec<String> = Vec::new();
        let first = |t: &str| t.split(": ").next().unwrap_or_default().to_string();
        for id in ps::keys(&self.state["explain"]["jobs"]) {
            let j = self.state["explain"]["jobs"][&id].clone();
            let jdone = strings(&j["done"]);
            if !pipe(&j["addresses"]).into_iter().any(|a| !ps::is_in(&s(a), &jdone)) {
                continue;
            }
            let model = s(&j["model"]);
            let r = self.test_explain_job(&id, &s(&j["out"]))?;
            kept.extend(r.kept.iter().cloned());
            for t in &r.blocked {
                blocked.push((model.clone(), t.clone()));
                done.push((id.clone(), first(t)));
            }
            for t in &r.refused {
                refused.push((model.clone(), t.clone()));
                done.push((id.clone(), first(t)));
            }
            for c in &r.candidates {
                edits.push(json!({ "tool": "edit_card", "arguments": { "at": c.address, "body": c.body } }));
                applied += 1;
                done.push((id.clone(), c.address.clone()));
            }
            if r.faults.is_empty() {
                continue;
            }
            if ps::int(&j["round"]).unwrap_or(0) >= 2 {
                for (a, f) in r.faults.iter() {
                    refused.push((model.clone(), format!("{a}: {f}")));
                    done.push((id.clone(), a.clone()));
                }
                continue;
            }
            // Once: the same conversation rewrites the refused parts into a new result file.
            let seq = self.next_seq();
            let out = ps::join(&ps::join(&self.p.work, "out"), &format!("explain-{seq}.json"));
            self.state["explain"]["jobs"][&id]["round"] = json!(2);
            self.state["explain"]["jobs"][&id]["out"] = json!(out);
            let dir = self.dir(&model);
            let follow = ps::join(&dir, &format!("explain-follow-{seq}.md"));
            let lines: Vec<String> = r.faults.iter().map(|(a, f)| format!("- [[{a}]]：{f}")).collect();
            util::write_text(
                &follow,
                &format!(
                    "# 补充任务（改正这些分卡的解释）\n\n新的结果文件: {out}\nSELF-CHECK: {}\n\n脚本没有写入下面的分卡。改正每张分卡的问题，只为它们写新的结果文件。写完后运行 SELF-CHECK，直到最后一行是 `faults: 0`：\n\n{}\n",
                    self.explain_check_command(&id),
                    lines.join("\n")
                ),
            )?;
            jobs.push(json!({
                "id": format!("{id}+2"),
                "continue": id,
                "files": [follow],
                "fallback": [self.docs.explain, ps::join(&dir, "setting.md"), s(&j["file"]), follow],
                "prompt": "按以上补充说明改正，写出新的结果文件，最后只回复一行 JSON。",
            }));
        }
        for chunk in edits.chunks(25) {
            self.edits(chunk.to_vec())?;
        }
        for (job, address) in &done {
            let list_now: Vec<Value> = pipe(&self.state["explain"]["jobs"][job]["done"]).into_iter().cloned().chain([json!(address)]).collect();
            self.state["explain"]["jobs"][job]["done"] = Value::Array(list_now);
        }
        for (field, items) in [("refused", &refused), ("blocked", &blocked)] {
            for (model, text) in items {
                let mut now: Vec<Value> = pipe(&self.state["explain"][field][model]).into_iter().filter(|v| truthy(v)).cloned().collect();
                now.push(json!(text));
                self.state["explain"][field][model] = Value::Array(now);
            }
        }
        self.save_state()?;
        self.out_last(&json!({
            "applied": applied,
            "refused": refused.iter().map(|(_, t)| t).collect::<Vec<_>>(),
            "blocked": blocked.iter().map(|(_, t)| t).collect::<Vec<_>>(),
            "kept": kept,
            "jobs": jobs,
        }))
    }

    fn step_explain_report(&mut self) -> Result<()> {
        self.explain_state();
        let mut reports = Vec::new();
        for model in strings(&Value::Array(pipe(&self.state["explain"]["models"]).into_iter().cloned().collect())) {
            let st = self.model_of(&json!(model))?;
            let own = Self::own_derivations(&st);
            let explained = Self::explained_of(&st);
            let missing: Vec<String> = own.iter().filter(|a| !ps::is_in(a, &explained)).cloned().collect();
            let pick = |field: &str| -> Vec<Value> { pipe(&self.state["explain"][field][&model]).into_iter().filter(|v| truthy(v)).cloned().collect() };
            reports.push(json!({
                "model": model,
                "derivations": own.len(),
                "explained": explained.len(),
                "missing": missing,
                "refused": pick("refused"),
                "blocked": pick("blocked"),
            }));
        }
        self.out_last(&json!({ "models": reports }))
    }
}

/// The no-main-line fault of `test_plan`: the model's own parts have no main line, yet the plan
/// adds a new main-line part. Only filed parts count; a part filed under another model and joined
/// by a Link belongs to its home model, so a model whose only members are linked parts has none.
fn lacks_main_line(structure: &Value, current_main: &[String], main: &[&Value]) -> bool {
    let has_filed = pipe(&structure["parts"]).into_iter().any(|p| !ps::is_false(&p["filed"]));
    current_main.is_empty() && has_filed && main.iter().any(|p| !truthy(&p["existing"]))
}

#[cfg(test)]
mod tests {
    use super::{close_entries, lacks_main_line, marker_entries, shared_lines, supplied_entries};
    use serde_json::json;

    // `close`: each build fault becomes one 修正 note for the Card it sits in.
    #[test]
    fn each_build_fault_becomes_a_note_for_its_own_card() {
        let build = json!({
            "setting": "B/1|a|a",
            "closure": [{ "from": "B/1|a", "to": "B/1|e", "note": "依赖 「B/1|e」：部门多。", "fix": "move it in" }],
            "forward": [{ "from": "B/1|a|a|6", "to": "B/1|a|a|13", "note": "依赖这两个方向的 「x」 比较利润。", "fix": "reorder" }],
            "unanchored": [{ "address": "B/1|a|a|3", "title": "t", "fix": "rest on an earlier part" }],
            "reach_in": [], "beyond_level": [{ "address": "B/1|a|a|9", "fix": "not a build fault close repairs" }],
        });
        let notes = close_entries("B/1|a", &build);
        let cards: Vec<&str> = notes.iter().map(|(at, _)| at.as_str()).collect();
        assert_eq!(cards, vec!["B/1|a", "B/1|a|a|6", "B/1|a|a|3"]);
        assert!(notes[0].1.starts_with("## 修正：[[B/1|a]]") && notes[0].1.contains("[[B/1|e]]") && notes[0].1.contains("参照"));
        // grossman2002integration trial 17: a writer turned 「时序沿用 [[2|a]]」 into 「信息沿用 [[2|a]]」, still a
        // rests-on sentence; the note names the words that read as rests-on, so the writer drops them.
        assert!(notes[0].1.contains("沿用") && notes[0].1.contains("用到") && notes[0].1.contains("基于"), "{}", notes[0].1);
        assert!(notes[1].1.contains("[[B/1|a|a|13]] 用到本卡") && notes[1].1.contains("依赖这两个方向"));
        assert!(notes[2].1.contains("依赖 [[B/1|a|a]]"));
        assert!(close_entries("B/1|a", &json!({})).is_empty(), "a model with no build faults gets no note");
    }

    // `close`: a supplied-step marker with a note inside it goes back to its Card; a clean one does not.
    #[test]
    fn a_malformed_marker_becomes_a_note_for_its_card() {
        let st = json!({
            "head": { "address": "B/1|a", "body": "问题。（补：书中未写）" },
            "parts": [
                { "address": "B/1|a|a", "body": "步骤。（补：书中略去：两式相除）" },
                { "address": "B/1|a|a|1", "body": "步骤。（补：书中略去）" }
            ]
        });
        let notes = marker_entries(&st);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].0, "B/1|a|a");
        assert!(notes[0].1.contains("（补：书中未写）") && notes[0].1.contains("（补：书中略去）"));
    }

    // `close`: a part the source review records as supplied needs a marker; one that has it, or a part
    // reviewed as supported, gets no note.
    #[test]
    fn a_supplied_part_without_a_marker_becomes_a_note() {
        let review = json!({ "parts": [
            { "part": "D1", "outcome": "supplied" },
            { "part": "D2", "outcome": "supplied" },
            { "part": "S", "outcome": "supported" }
        ] });
        let ids = json!({ "D1": "B/8|a|a|1", "D2": "B/8|a|a|2", "S": "B/8|a|a" });
        let st = json!({ "parts": [
            { "address": "B/8|a|a|1", "body": "补出的一步。" },
            { "address": "B/8|a|a|2", "body": "补出的一步（补：书中略去）。" },
            { "address": "B/8|a|a", "body": "设定。" }
        ] });
        let notes = supplied_entries(&review, &ids, &st);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].0, "B/8|a|a|1");
        assert!(notes[0].1.contains("补出标记"));
    }

    // The 共用 lines of a 结构 list follow the taken parts' current addresses, in order; a list whose
    // 共用 lines do not match the plan's taken parts one for one is left alone.
    #[test]
    fn shared_lines_follow_the_taken_parts() {
        let section = "**结构**\n\n- 设定 [[B/3|a|a]]（核心）：设定\n- 共用 [[B/2|a|a|12]]（共用）：π_m (6)\n- 推导 [[B/3|a|a|1]]：推导";
        let wanted = vec!["- 共用 [[B/2|a|a|13]]（共用）：π_m (6)".to_string()];
        assert_eq!(
            shared_lines(section, &wanted).unwrap(),
            "**结构**\n\n- 设定 [[B/3|a|a]]（核心）：设定\n- 共用 [[B/2|a|a|13]]（共用）：π_m (6)\n- 推导 [[B/3|a|a|1]]：推导"
        );
        assert!(shared_lines(section, &[]).is_none());
        assert!(shared_lines(section, &[wanted[0].clone(), wanted[0].clone()]).is_none());
    }

    // -Step defer: the released models leave the order (every later step iterates it) and read deferred;
    // a model the batch does not hold is ignored.
    #[test]
    fn deferred_models_leave_the_batch_order() {
        let mut state = json!({"order": ["K/2|a", "K/3|a", "K/4|a"], "models": {"K/2|a": {"status": "planned"}, "K/3|a": {"status": "planned"}, "K/4|a": {"status": "planned"}}});
        let deferred = super::defer_models(&mut state, &["k/3|A".to_string(), "K/4|a".to_string(), "K/9|a".to_string()]);
        assert_eq!(deferred, vec!["K/3|a".to_string(), "K/4|a".to_string()]);
        assert_eq!(state["order"], json!(["K/2|a"]));
        assert_eq!(state["models"]["K/3|a"]["status"], json!("deferred"));
        assert_eq!(state["models"]["K/2|a"]["status"], json!("planned"));
    }

    // close: a planned rests-on Link a written part does not author goes back to its writer; one the part
    // authors, one to a target outside the model, and a taken part ask for nothing.
    #[test]
    fn a_missing_planned_link_is_a_close_entry() {
        use std::collections::BTreeSet;

        let plan_parts = vec![
            json!({"id":"E","rests":[["F","用到均衡条件"],["G","用到定义"]]}),
            json!({"id":"H","take":"K/3|a#X"}),
            json!({"id":"F"}),
        ];
        let ids = json!({"E":"K/7|a|a|5","F":"K/3|a|a|2","G":"K/7|a|a|1","H":"K/3|a#X"});
        let resolve = |r: &str| -> Option<String> { let v = ids[r].as_str().unwrap_or_default().to_string(); if v.is_empty() || v.contains('#') { None } else { Some(v) } };
        let parts: BTreeSet<String> = ["K/7|a|a|5", "K/7|a|a|1", "K/3|a|a|2"].into_iter().map(String::from).collect();
        let pairs: BTreeSet<(String, String)> = [("K/7|a|a|5".to_string(), "K/7|a|a|1".to_string())].into_iter().collect();
        let entries = super::planned_link_entries("K/7|a", &plan_parts, &ids, &json!({}), &resolve, &parts, &pairs);
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].0, "K/7|a|a|5");
        assert!(entries[0].1.contains("[[K/3|a|a|2]]") && entries[0].1.contains("用到均衡条件"));
    }

    // A split Derivation part is one planned target in several pieces (`D11`, `D11~2`, `D11~3`): a Link to
    // any piece authors the planned relation. grossman2002integration trial 16: `7|a|a|19` rested on the
    // chain's last piece `7|a|a|18` (it holds the result it uses), close sent it back for `7|a|a|16`, and
    // model-finish refused `7|a`; `8|a` the same way. A prefix of another id (`D1` for `D11`) is no piece.
    #[test]
    fn a_link_to_any_piece_of_a_split_part_authors_its_planned_link() {
        use std::collections::BTreeSet;

        let ids = json!({"D1":"K/7|a|a|1","D11":"K/7|a|a|16","D11~2":"K/7|a|a|17","D11~3":"K/7|a|a|18","SC":"K/7|a|a|19"});
        assert_eq!(super::split_pieces(&ids, "D11"), vec!["K/7|a|a|16", "K/7|a|a|17", "K/7|a|a|18"]);
        assert_eq!(super::split_pieces(&ids, "D1"), vec!["K/7|a|a|1"]);
        assert!(super::split_pieces(&ids, "K/3|a#X").is_empty());

        let plan_parts = vec![json!({"id":"SC","rests":[["D11","用到交点的移动"]]}), json!({"id":"D11"})];
        let resolve = |r: &str| -> Option<String> { ids[r].as_str().map(str::to_string) };
        let parts: BTreeSet<String> = ["K/7|a|a|16", "K/7|a|a|17", "K/7|a|a|18", "K/7|a|a|19"].into_iter().map(String::from).collect();
        let to_last: BTreeSet<(String, String)> = [("K/7|a|a|19".to_string(), "K/7|a|a|18".to_string())].into_iter().collect();
        assert!(super::planned_link_entries("K/7|a", &plan_parts, &ids, &json!({}), &resolve, &parts, &to_last).is_empty());
        let none = BTreeSet::new();
        let entries = super::planned_link_entries("K/7|a", &plan_parts, &ids, &json!({}), &resolve, &parts, &none);
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert!(entries[0].1.contains("[[K/7|a|a|16]]") && entries[0].1.contains("[[K/7|a|a|18]]"), "{}", entries[0].1);
    }

    // A reference to another model's part (`K/2|a#D`) whose part `split` cut into pieces is met by a Link to any
    // piece, read from that model's own map (grossman2002integration trial 19: model-finish refused `3|a` because
    // `3|a|a|4` linked a later piece of `2|a`'s split derivation, not `2|a|a|14`).
    #[test]
    fn a_reference_to_another_models_split_part_takes_any_piece() {
        let own = json!({"P":"K/3|a|a|4"});
        let models = json!({"K/2|a": {"ids": {"D":"K/2|a|a|14","D~2":"K/2|a|a|15","D1":"K/2|a|a|2"}}});
        assert_eq!(super::reference_pieces(&own, &models, "K/2|a#D"), vec!["K/2|a|a|14", "K/2|a|a|15"]);
        assert_eq!(super::reference_pieces(&own, &models, "P"), vec!["K/3|a|a|4"]);
        assert!(super::reference_pieces(&own, &models, "K/9|a#D").is_empty());
    }

    // A 结构 line never carries a semicolon (R3 in a done Model Card).
    #[test]
    fn a_structure_line_writes_a_semicolon_as_a_comma() {
        assert_eq!(super::structure_text("用到议价份额 ω；本模型放宽其假设"), "用到议价份额 ω，本模型放宽其假设");
        assert_eq!(super::structure_text("a; b"), "a, b");
    }

    #[test]
    fn linked_parts_do_not_need_a_main_line() {
        let linked = json!({ "parts": [{ "address": "b/7|a|a|1", "filed": false }] });
        let new_part = json!({ "id": "S", "line": "main" });
        assert!(!lacks_main_line(&linked, &[], &[&new_part]));
        let filed = json!({ "parts": [{ "address": "b/8|a|a", "filed": true }] });
        assert!(lacks_main_line(&filed, &[], &[&new_part]));
        let kept = json!({ "id": "S", "existing": "b/8|a|a", "line": "main" });
        assert!(!lacks_main_line(&filed, &[], &[&kept]));
        assert!(!lacks_main_line(&filed, &["b/8|a|a".to_string()], &[&new_part]));
    }
}
