//! `zt-digest model` (model-compiler.ps1): the deterministic half of the
//! book digest's model compiler. Sonnet plans each model (plan.json);
//! DeepSeek writes briefs, Card bodies, charts, Derivation parts, and reviews;
//! this tool does everything between them without a model.
//!
//!   zt-digest model -Step brief|validate|apply|derive|review|split|close|report|resume
//!                         |explain|explain-check|explain-apply|explain-report
//!                   -Work <folder> [-Staging <folder>] [-Models a,b] [-Plan <file>] [-Need a=N;b=M]
//!                   [-Changed] [-Rebuild] [-IndependentReview] [-Batch] [-Level concept|formal|empirical]
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
use crate::ledger::batch::is_transport_fault;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
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
// A whole 待写 line, or an unwritten line of a setting's **直觉** section (`- 是什么：待写`).
pattern!(TODO_LINE, r"(?i)(^|\n)\s*(-\s*(是什么|为什么需要|怎样进入模型)\s*[：:]\s*)?待写\s*($|\n)");
// An unwritten 直觉 line alone: `intuition_entries` sends it back, so `unwritten_entries` reads the body without it.
pattern!(INTUITION_TODO, r"(?m)^\s*-\s*(是什么|为什么需要|怎样进入模型)\s*[：:]\s*待写\s*$");
pattern!(CHART, r"(?i)```chart");
pattern!(FAULTS_LINE, r"(?i)^faults: \d+");
pattern!(FAULTS_PREFIX, r"(?i)^faults: ");
pattern!(STYLE_FAULT, r"(?i)^(\S+) (R\d+ .*)$");
pattern!(REFUSED, r"(?i)^(\S+): ");
pattern!(LIST_ITEM, r"(?i)^\s*-");
pattern!(COMMA, r",");

const STEPS: [&str; 16] = [
    "brief", "validate", "plan-status", "apply", "derive", "review", "review-audit", "split", "close", "defer", "report", "resume", "explain", "explain-check",
    "explain-apply", "explain-report",
];
/// Folds one move map (old address → new address, every Card a write renumbered) into the run's
/// renames (address when the run began → address now).
fn track_renames(renamed: &mut BTreeMap<String, String>, map: &Map<String, Value>) {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for current in renamed.values_mut() {
        if let Some(to) = map.get(current.as_str()).and_then(Value::as_str) {
            seen.insert(current.clone());
            *current = to.to_string();
        }
    }
    for (from, to) in map {
        if !seen.contains(from) && !renamed.contains_key(from)
            && let Some(to) = to.as_str()
        {
            renamed.insert(from.clone(), to.to_string());
        }
    }
}

/// Where a Card the plan named stands now.
fn current_address(renamed: &BTreeMap<String, String>, at: &str) -> String {
    renamed.get(at).cloned().unwrap_or_else(|| at.to_string())
}

/// One model's folder and artifact stem: the address with `\`, `/` and `|` replaced by `_`.
/// Adoption reads the retained plans and states by the same name the compiler writes them.
pub(crate) fn slug_of(address: &str) -> String {
    SLUG.replace_all(address, "_").into_owned()
}
/// A premise that states a mathematical unit (a Model Card with a unit form) is cited, not taken in.
const UNIT_FORMS: [&str; 6] = ["definition", "axiom", "theorem", "proposition", "lemma", "corollary"];

fn is_math_unit_structure(st: &Value) -> bool {
    !st["form"].is_null() && !s(&st["form"]).trim().is_empty()
}
/// The CRLF line end the ported PowerShell job texts kept.
const CR: &str = "\r\n";

#[derive(Clone)]
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
    /// split only: fail with non-zero exit if any pending formal review JSON is missing, invalid, or incomplete.
    strict_audit: bool,
    /// explain only (§3.7): the batch's own explain round — one job per model continuing `<slug>:brief`,
    /// with the architect's design entry from `plan.json`.
    batch: bool,
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

#[derive(Clone)]
struct BatchSpec {
    work: String,
    models: String,
    need: String,
    jobs_file: String,
}

#[derive(Clone)]
struct Docs {
    brief: String,
    fill: String,
    review: String,
    explain: String,
    /// The empirical path's brief and evidence review (docs/empirical-models-proposal.md §8.2).
    brief_empirical: String,
    review_empirical: String,
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
    /// Every Card this run renumbered: its address when the run began → its address now. A plan names
    /// the Cards it merges by their addresses at planning time (ebrill2001modern pilot 2026-11-09).
    renamed: BTreeMap<String, String>,
    /// Multi-batch mode suppresses per-batch stdout and captures the last JSON object here.
    silent_out: bool,
    last_out: Option<Value>,
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
/// atkinson2015lectures (2026-11-01): a batch folder is named after its first model, so a rebuild whose first model
/// was also the first model of a finished batch reuses that folder and its state. The brief begins the batch's own
/// work order with the models it was given: the finished models leave the order (no later step writes or reports
/// them; the fill line of 13 models had overflowed its 400 characters), and their entries and step evidence stay
/// in `models`, which their completions read.
fn begin_batch_order(state: &mut Value, models: &[String]) {
    let kept: Vec<String> = list(&state["order"]).into_iter().map(s).filter(|m| models.iter().any(|x| ps::eq(x, m))).collect();
    state["order"] = json!(kept);
}

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
    rests: &BTreeSet<(String, String)>,
    formal_model: bool,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    // #197: the model's takes. Only a rests-on Link takes a shared part in, so a planned relation to a take
    // is authored by a rests-on Link alone; a `参照` leaves the take outside the build order.
    let taken: BTreeSet<String> = plan_parts
        .iter()
        .filter(|p| truthy(&p["take"]))
        .map(|p| s(&ids[&s(&p["id"])]))
        .filter(|a| !a.is_empty() && !a.contains('#'))
        .collect();
    for part in plan_parts {
        let id = s(&part["id"]);
        // A refilled part is rewritten by this build, so it authors its planned Links like a new one.
        if id.is_empty() || truthy(&part["take"]) || (truthy(&part["existing"]) && !truthy(&part["refill"])) {
            continue;
        }
        let address = s(&ids[&id]);
        if address.is_empty() || address.contains('#') {
            continue;
        }
        // Single authoritative relation-requirements extractor (Astra #1):
        // in Level 2 formal models, plain `of`/`after` are placement-only unless `kind` is explicitly set.
        let wanted = super::formal::planned_relations(part, formal_model);
        for (reference, why) in wanted {
            let Some(target) = resolve(&reference) else { continue };
            let take = taken.contains(&target);
            if ps::eq(&target, model) || (!parts.contains(&target) && !take) {
                continue;
            }
            let mut pieces = reference_pieces(ids, models, &reference);
            if pieces.is_empty() {
                pieces.push(target.clone());
            }
            let authored = if take { rests } else { pairs };
            if pieces.iter().any(|piece| authored.contains(&(address.clone(), piece.clone()))) {
                continue;
            }
            let reason = if why.is_empty() { String::new() } else { format!("计划写的关系：{why}。") };
            if take {
                out.push((address.clone(), format!(
                    "## 修正：[[{address}]]\n\n构建检查（计划的取用）：模型 [[{model}]] 的计划取用 [[{target}]]，并要求这张分卡依赖它。{reason}本模型没有分卡依赖 [[{target}]]，所以它不在模型里。在用到它的那句话里写「依赖 [[{target}]]：用到……」；「参照」「对应」只是提到，不会把它取进本模型。不要增删其余内容，不要移动卡片。改完照常运行 CHECK。\n"
                )));
                continue;
            }
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

/// The 修正 notes for the setting, assumption and definition parts this build wrote whose **直觉** section is
/// missing or not fully written (zt-digest 0.52.0). A part another model lends, or an existing part the plan
/// keeps, is not this build's to explain; an unwritten part (`待写`) goes back as a whole, not here. A setting
/// that `split` cut into pieces explains itself once, on its first piece; the later pieces continue it.
fn intuition_entries(plan_parts: &[Value], ids: &Value, st: &Value, empirical: bool) -> Vec<(String, String)> {
    let bodies: BTreeMap<String, String> = list(&st["parts"]).into_iter().map(|p| (s(&p["address"]), s(&p["body"]))).collect();
    let mut out = Vec::new();
    for part in plan_parts {
        let id = s(&part["id"]);
        if id.is_empty() || truthy(&part["take"]) || (truthy(&part["existing"]) && !truthy(&part["refill"])) || !text::wants_intuition(part, empirical) {
            continue;
        }
        let address = s(&ids[&id]);
        let Some(body) = bodies.get(&address) else { continue };
        if TODO_BODY.is_match(body) {
            continue;
        }
        if let Some(lack) = text::intuition_fault(body) {
            out.push((address.clone(), format!(
                    "## 修正：[[{address}]]\n\n构建检查（直觉）：{lack}。这张卡设定一个对象，读者先要知道它是什么、为什么需要它、它怎样进入模型。在设定内容之后、符号表和术语表之前写出这一节：\n\n**直觉**\n- 是什么：……\n- 为什么需要：……\n- 怎样进入模型：……\n\n按填写说明的“讲清设定”一节写。不要增删其余内容，不要移动卡片。改完照常运行 CHECK。\n"
            )));
        }
    }
    out
}

/// The 修正 notes for the parts this build owns that are still unwritten (`待写`) at close: each goes back to
/// the conversation that should have written it, with the plan's title, content and source. Before this an
/// unwritten part had no way back after its fill: rotunno2026industrial/4|a part O (line `off`) stayed 待写
/// through the close round, the model reached `reviewed` with `todo: 1`, and model-finish refused it. A part
/// whose only 待写 lines are its **直觉** goes back through [`intuition_entries`]; a taken part belongs to its
/// home model, and an existing part the plan keeps is not this build's.
fn unwritten_entries(plan_parts: &[Value], ids: &Value, st: &Value) -> Vec<(String, String)> {
    let bodies: BTreeMap<String, String> = list(&st["parts"]).into_iter().map(|p| (s(&p["address"]), s(&p["body"]))).collect();
    let mut out = Vec::new();
    for part in plan_parts {
        let id = s(&part["id"]);
        if id.is_empty() || truthy(&part["take"]) || (truthy(&part["existing"]) && !truthy(&part["refill"])) {
            continue;
        }
        let address = s(&ids[&id]);
        let Some(body) = bodies.get(&address) else { continue };
        if !TODO_LINE.is_match(&INTUITION_TODO.replace_all(body, "")) {
            continue;
        }
        let sources: Vec<String> = pipe(&part["src"]).into_iter().map(s).filter(|x| !x.is_empty()).collect();
        let kind = if derives(part) { "这是一张推导分卡：按填写说明的推导格式写出思路、起点、步骤、结果和解读，" } else { "" };
        out.push((address.clone(), format!(
            "## 修正：[[{address}]]\n\n构建检查（未写）：这张分卡还是「待写」，模型缺了它就不完整。{kind}按计划写出它，读原文后再写：\n\n- 分面：{}\n- 标题：{}\n- 内容：{}\n- 来源：{}\n\n照填写说明写全这张卡（设定、假设、定义卡连同 **直觉** 一节），保留它已有的链接。不要改别的卡，不要移动卡片。写完照常运行 CHECK。\n",
            s(&part["facet"]),
            s(&part["title"]),
            s(&part["spec"]),
            if sources.is_empty() { "见计划".to_string() } else { sources.join("、") }
        )));
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

/// A take target that is a whole model: a Model Card with no facet (a model's head, never one of its parts).
fn whole_model_take(card: &Value) -> bool {
    truthy(card) && ps::eq(&s(&card["type"]), "model") && !truthy(&card["facet"])
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

/// The Model Card's `**引入**` section (#212, docs/model-shared-blocks-proposal.md D1): one line per taken
/// part, `(block, address, use)`; `整块` takes the Card with its block, `单卡` the Card alone. `None` when the
/// model takes nothing.
fn import_section(takes: &[(bool, String, String)]) -> Option<String> {
    // atkinson2015lectures (2026-11-01): a take that resolved to no address is its bare plan id (a model whose
    // apply failed has an empty part map); written as `[[TN]]` it made the archive refuse the whole batch's edit.
    let takes: Vec<&(bool, String, String)> = takes.iter().filter(|(_, address, _)| address.contains('/')).collect();
    if takes.is_empty() {
        return None;
    }
    let lines = takes
        .iter()
        .map(|(block, address, used)| {
            format!("- {} [[{address}]]：{}", if *block { "整块" } else { "单卡" }, structure_text(used))
        })
        .collect::<Vec<_>>();
    Some(format!("**引入**\n\n{}", lines.join("\n")))
}

/// A Model Card body whose `**引入**` section reads `wanted` (placed before `结构` when the body has none);
/// `None` when it already does.
fn imports_rebuilt(body: &str, wanted: &str) -> Option<String> {
    if text::get_section(body, "引入").as_deref() == Some(wanted) {
        return None;
    }
    if text::get_section(body, "引入").is_none()
        && let Some(at) = body.find("**结构**")
    {
        return Some(format!("{}{wanted}\n\n{}", &body[..at], &body[at..]));
    }
    Some(text::set_section(body, "引入", wanted))
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
    Ok(cli_inner(argv, false, None, None)?.0)
}

/// Runs one `model` compiler step in-process with `silent_out = true`, an optional pre-opened `Zt`,
/// and an optional pre-validated `state.json` snapshot (eliminating TOCTOU state re-reads, H7/H-new).
pub(crate) fn run_silent_with_zt_and_state(
    argv: &[String],
    pre_zt: Option<Zt>,
    preloaded_state: Option<Value>,
) -> Result<(i32, Value)> {
    cli_inner(argv, true, pre_zt, preloaded_state)
}

fn cli_inner(
    argv: &[String],
    silent_out: bool,
    pre_zt: Option<Zt>,
    preloaded_state: Option<Value>,
) -> Result<(i32, Value)> {
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
            "BatchesFile",
            "RenewUnits",
        ],
        &["Changed", "Rebuild", "Report", "IndependentReview", "Batch", "StrictAudit"],
    )?;
    let step = a.choice("Step", &STEPS)?;
    let has_batches = a.has("BatchesFile");
    if has_batches && a.has("Work") {
        bail!("-Work and -BatchesFile are mutually exclusive");
    }
    let work = if has_batches {
        String::new()
    } else {
        let w = a.required("Work")?;
        if w.is_empty() {
            bail!("Cannot bind argument to parameter 'Work' because it is an empty string.");
        }
        let wp = absolutize_work(&w);
        wp
    };
    let batches_file = a.str("BatchesFile");
    let renew_units = a.str("RenewUnits");
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
        strict_audit: a.flag("StrictAudit"),
        batch: a.flag("Batch"),
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
        brief_empirical: scripts.file("dsh-empirical-brief-instructions.zh.md"),
        review_empirical: scripts.file("dsh-empirical-review-instructions.zh.md"),
    };
    let explain_args = format!("-MaxExplainSentences {} -MaxExplainChars {}", p.max_explain_sentences, p.max_explain_chars);
    // Writers check their own new Cards, so a Derivation part must have its 思路 and 解读 (R15).
    let limits = format!(
        "--max-sentences {} --max-chars {} --max-steps {} --max-explain-sentences {} --max-explain-chars {} --require-explain",
        p.max_sentences, p.max_chars, p.max_steps, p.max_explain_sentences, p.max_explain_chars
    );
    // R328 (§15.2): the steps that allocate a model job or write Model Cards into the book's
    // Literature tree stop before their first file or Card when that tree is locked by the owner.
    // `close` and `report` write Model Cards (`refresh_shared`, `complete_structures`), so they
    // preflight too whenever `-Staging` names a book.
    if has_batches {
        let code = run_batches(p, &docs, &explain_args, &limits, &scripts.helper, &batches_file, &renew_units)?;
        return Ok((code, json!({})));
    }
    let zt = if matches!(
        p.step.as_str(),
        "brief" | "apply" | "derive" | "review" | "split" | "close" | "report" | "resume" | "explain" | "explain-apply"
    ) {
        let key = crate::lockcheck::key_from_staging(&p.staging);
        let mut client = match pre_zt {
            Some(client) => client,
            None => crate::zt::start()?,
        };
        if !key.is_empty()
            && let Some(refusal) = crate::lockcheck::preflight(&mut client, "model", &p.step, &key)?
        {
            return Ok((crate::lockcheck::stop("model", &refusal)?, refusal));
        }
        Some(client)
    } else {
        None
    };
    let mut c = new_compiler_with_state(p, docs, explain_args, limits, scripts.helper, zt, silent_out, preloaded_state)?;
    let code = c.run_step()?;
    Ok((code, c.last_out.unwrap_or_else(|| json!({}))))
}

fn new_compiler(
    p: Params,
    docs: Docs,
    explain_args: String,
    limits: String,
    helper: String,
    zt: Option<Zt>,
    silent_out: bool,
) -> Result<Compiler> {
    new_compiler_with_state(p, docs, explain_args, limits, helper, zt, silent_out, None)
}

#[allow(clippy::too_many_arguments)]
fn new_compiler_with_state(
    p: Params,
    docs: Docs,
    explain_args: String,
    limits: String,
    helper: String,
    zt: Option<Zt>,
    silent_out: bool,
    preloaded_state: Option<Value>,
) -> Result<Compiler> {
    std::fs::create_dir_all(&p.work)?;
    let state_file = ps::join(&p.work, "state.json");
    let state = match preloaded_state {
        Some(st) => st,
        None if ps::exists(&state_file) => util::read_json(&state_file)?,
        None => json!({ "order": [], "models": {}, "jobs": {}, "rereview": [], "reviews": [], "seq": 0 }),
    };
    Ok(Compiler {
        helper,
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
        renamed: BTreeMap::new(),
        silent_out,
        last_out: None,
    })
}

fn absolutize_work(raw: &str) -> String {
    let wp = std::path::Path::new(raw);
    if wp.is_absolute() {
        raw.to_string()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(wp).to_string_lossy().into_owned())
            .unwrap_or_else(|_| raw.to_string())
    }
}

fn norm_fs_key(path: &str) -> String {
    let resolved = std::fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| std::path::PathBuf::from(path));
    resolved
        .to_string_lossy()
        .replace('/', "\\")
        .trim_start_matches("\\\\?\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn norm_work_key(path: &str) -> String {
    norm_fs_key(path)
}

fn validate_batches_manifest(
    step: &str,
    staging: &str,
    batches_file: &str,
) -> Result<(String, Vec<BatchSpec>)> {
    if !matches!(
        step,
        "brief" | "derive" | "review" | "review-audit" | "split" | "close" | "report" | "explain" | "explain-apply"
    ) {
        bail!("-BatchesFile supports brief, derive, review, review-audit, split, close, report, explain, explain-apply (not {step})");
    }
    if staging.trim().is_empty() {
        bail!("-BatchesFile requires an explicit -Staging folder");
    }
    let manifest = crate::organize::common::manifest(staging)?;
    let key = crate::organize::common::pstr(&manifest, "citation_key").trim().to_string();
    if key.is_empty() {
        bail!("manifest.json in {staging} has an empty citation_key");
    }
    let raw = util::read_json(batches_file).with_context(|| format!("invalid -BatchesFile {batches_file}"))?;
    let items = raw.as_array().context("-BatchesFile must be a JSON array")?;
    let mut specs = Vec::with_capacity(items.len());
    let mut seen_works: HashSet<String> = HashSet::new();
    let mut seen_jobs_files: HashSet<String> = HashSet::new();
    let mut seen_models: HashSet<String> = HashSet::new();
    let key_prefix = format!("{}/", key.to_ascii_lowercase());
    let requires_existing_state = matches!(
        step,
        "derive" | "review" | "review-audit" | "split" | "close" | "report" | "explain" | "explain-apply"
    );
    for (idx, item) in items.iter().enumerate() {
        let obj = item.as_object().with_context(|| format!("-BatchesFile entry {idx} must be an object"))?;
        let raw_work = obj.get("work").and_then(Value::as_str).map(str::trim).unwrap_or("");
        if raw_work.is_empty() {
            bail!("-BatchesFile entry {idx} is missing a non-empty `work` path");
        }
        let work = absolutize_work(raw_work);
        let wkey = norm_work_key(&work);
        if !seen_works.insert(wkey.clone()) {
            bail!("-BatchesFile lists duplicate `work` folder `{work}`");
        }
        let jobs_file = obj.get("jobs_file").and_then(Value::as_str).map(str::trim).unwrap_or("").to_string();
        if !jobs_file.is_empty() {
            let jkey = norm_fs_key(&jobs_file);
            if !seen_jobs_files.insert(jkey.clone()) {
                bail!("-BatchesFile lists colliding `jobs_file` `{jobs_file}`");
            }
            if jkey == norm_fs_key(&ps::join(&work, "state.json"))
                || std::path::Path::new(&jobs_file).file_name().is_some_and(|n| n.eq_ignore_ascii_case("plan.json") || n.eq_ignore_ascii_case("state.json"))
            {
                bail!("-BatchesFile `jobs_file` `{jobs_file}` cannot target state.json or plan.json");
            }
        }
        let mut model_vec: Vec<String> = Vec::new();
        if let Some(mv) = obj.get("models") {
            if let Some(arr) = mv.as_array() {
                for m in arr {
                    let s_m = m.as_str().map(str::trim).unwrap_or("");
                    if !s_m.is_empty() {
                        model_vec.push(s_m.to_string());
                    }
                }
            } else if let Some(csv) = mv.as_str() {
                for s_m in csv.split(',').map(str::trim).filter(|x| !x.is_empty()) {
                    model_vec.push(s_m.to_string());
                }
            } else if !mv.is_null() {
                bail!("-BatchesFile entry {idx} `models` must be an array or comma-separated string");
            }
        }
        if matches!(step, "brief" | "explain") && model_vec.is_empty() {
            bail!("-BatchesFile entry {idx} ({work}) requires a non-empty `models` list for -Step {step}");
        }
        let state_path = ps::join(&work, "state.json");
        let state_order: Vec<String> = if ps::exists(&state_path) {
            let st = util::read_json(&state_path)
                .with_context(|| format!("-BatchesFile entry {idx}: invalid state.json in {work}"))?;
            if !st.is_object() {
                bail!("-BatchesFile entry {idx}: state.json in {work} must be an object");
            }
            list(&st["order"]).into_iter().map(s).filter(|m| !m.is_empty()).collect()
        } else {
            if requires_existing_state {
                bail!("-BatchesFile entry {idx}: `-Step {step}` requires existing `{state_path}`");
            }
            Vec::new()
        };
        let mut check_models: Vec<String> = Vec::new();
        for m in model_vec.iter().chain(state_order.iter()) {
            if !check_models.iter().any(|x| ps::eq(x, m)) {
                check_models.push(m.clone());
            }
        }
        for m in &check_models {
            let low = m.to_ascii_lowercase();
            if !low.starts_with(&key_prefix) {
                bail!("-BatchesFile entry {idx} model `{m}` (or state order) does not belong to `{key}`");
            }
            if !seen_models.insert(low) {
                bail!("-BatchesFile assigns model `{m}` to more than one batch");
            }
        }
        let need = obj.get("need").and_then(Value::as_str).unwrap_or("").to_string();
        specs.push(BatchSpec {
            work,
            models: model_vec.join(","),
            need,
            jobs_file,
        });
    }
    Ok((key, specs))
}

fn load_renew_units_map(path: &str) -> Result<(Vec<(String, i64)>, HashMap<String, String>)> {
    let raw = util::read_json(path).with_context(|| format!("invalid -RenewUnits {path}"))?;
    let items = if let Some(arr) = raw.as_array() {
        arr
    } else if let Some(arr) = raw.get("units").and_then(Value::as_array) {
        arr
    } else {
        bail!("-RenewUnits must contain a JSON array of units");
    };
    let mut pairs = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut unit_to_model = HashMap::new();
    for (idx, item) in items.iter().enumerate() {
        let obj = item.as_object().with_context(|| format!("-RenewUnits entry {idx} must be an object"))?;
        let id = obj.get("id").or_else(|| obj.get("unit")).and_then(Value::as_str).map(str::trim).unwrap_or("").to_string();
        if id.is_empty() {
            bail!("-RenewUnits entry {idx} has an empty unit id");
        }
        if !seen_ids.insert(id.clone()) {
            bail!("-RenewUnits entry {idx} duplicates unit id `{id}`");
        }
        let token = obj
            .get("token")
            .and_then(Value::as_i64)
            .filter(|t| *t > 0)
            .with_context(|| format!("-RenewUnits entry {idx} (`{id}`) must carry positive integer `token`"))?;
        let model = obj
            .get("inputs")
            .and_then(|inp| inp.get("header").or_else(|| inp.get("model")))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if !model.is_empty() {
            unit_to_model.insert(id.clone(), model);
        }
        pairs.push((id, token));
    }
    Ok((pairs, unit_to_model))
}

fn run_batches(
    base_p: Params,
    docs: &Docs,
    explain_args: &str,
    limits: &str,
    helper: &str,
    batches_file: &str,
    renew_units: &str,
) -> Result<i32> {
    let (key, specs) = validate_batches_manifest(&base_p.step, &base_p.staging, batches_file)?;
    if specs.is_empty() {
        util::print_line(&ps::json_compact(&json!({
            "ok": true,
            "step": base_p.step,
            "batches": [],
            "jobs": [],
        })))?;
        return Ok(0);
    }

    let mut zt = crate::zt::start()?;
    let mut has_renew = false;
    let mut renewed_models: HashSet<String> = HashSet::new();
    let mut released_models: HashSet<String> = HashSet::new();
    let mut renew_summary = Value::Null;
    if !renew_units.trim().is_empty() {
        has_renew = true;
        let (pairs, mut unit_to_model) = load_renew_units_map(renew_units)?;
        if !pairs.is_empty() {
            let p = crate::ledger::Prepared::read(&base_p.staging, &mut zt)?;
            // Also resolve unit -> model from the ledger itself if the units file omitted `inputs`.
            if let Ok(conn) = crate::ledger::connect(&p.out.join("ledger.sqlite3"), false) {
                for (uid, _) in &pairs {
                    if !unit_to_model.contains_key(uid)
                        && let Ok(raw_inp) = conn.query_row("SELECT inputs_json FROM units WHERE id=?1", [uid], |r| r.get::<_, String>(0))
                        && let Ok(v) = serde_json::from_str::<Value>(&raw_inp)
                    {
                        let m = v.get("header").or_else(|| v.get("model")).and_then(Value::as_str).unwrap_or("").trim();
                        if !m.is_empty() {
                            unit_to_model.insert(uid.clone(), m.to_string());
                        }
                    }
                }
            }
            let res = crate::ledger::renew_batch(&p, &pairs)?;
            for ren in list(&res["renewed"]) {
                let uid = s(&ren["unit"]);
                if let Some(m) = unit_to_model.get(&uid) {
                    renewed_models.insert(m.to_ascii_lowercase());
                }
            }
            for rel in list(&res["released"]) {
                let uid = s(&rel["unit"]);
                if let Some(m) = unit_to_model.get(&uid) {
                    released_models.insert(m.to_ascii_lowercase());
                }
            }
            renew_summary = res;
        }
    }

    let needs_lock_preflight = matches!(
        base_p.step.as_str(),
        "brief" | "derive" | "review" | "split" | "close" | "report" | "explain" | "explain-apply"
    );
    let mut batch_results: Vec<Value> = Vec::with_capacity(specs.len());
    let mut all_jobs: Vec<Value> = Vec::new();
    let mut all_ok = true;

    for (idx, spec) in specs.iter().enumerate() {
        let mut p = Params {
            work: spec.work.clone(),
            models: if !spec.models.is_empty() { spec.models.clone() } else { base_p.models.clone() },
            need: if !spec.need.is_empty() { spec.need.clone() } else { base_p.need.clone() },
            ..base_p.clone()
        };
        if !spec.models.is_empty() {
            p.models = spec.models.clone();
        }

        // Compute effective models of this batch (`spec.models` / `base_p.models` / `state.order`) before execution.
        let mut batch_models: Vec<String> = p
            .models
            .split(',')
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(str::to_string)
            .collect();
        let st_path = ps::join(&spec.work, "state.json");
        if ps::exists(&st_path)
            && let Ok(st) = util::read_json(&st_path)
        {
            for m in list(&st["order"]).into_iter().map(s).filter(|m| !m.is_empty()) {
                if !batch_models.iter().any(|x| ps::eq(x, &m)) {
                    batch_models.push(m);
                }
            }
        }

        if has_renew {
            let stale = batch_models.iter().any(|m| {
                let low = m.to_ascii_lowercase();
                released_models.contains(&low)
                    || (needs_lock_preflight && !renewed_models.contains(&low))
            });
            if stale {
                all_ok = false;
                batch_results.push(json!({
                    "work": spec.work,
                    "status": "stale_claim",
                    "error": "claimed model unit was released, expired, or missing from renewed claims before batch step",
                }));
                continue;
            }
        }

        if needs_lock_preflight {
            match crate::lockcheck::preflight(&mut zt, "model", &base_p.step, &key) {
                Ok(Some(refusal)) => {
                    batch_results.push(json!({ "work": spec.work, "status": "locked", "refusal": refusal }));
                    for rem in &specs[idx + 1..] {
                        batch_results.push(json!({ "work": rem.work, "status": "not_run" }));
                    }
                    let code = crate::lockcheck::refusal_code(&refusal);
                    let out = json!({
                        "ok": false,
                        "step": base_p.step,
                        "code": code,
                        "refusal": refusal,
                        "renew": renew_summary,
                        "batches": batch_results,
                        "jobs": all_jobs,
                    });
                    util::print_line(&ps::json_compact(&out))?;
                    return Ok(crate::lockcheck::LOCKED_EXIT);
                }
                Ok(None) => {}
                Err(err) => {
                    if is_transport_fault(&err) {
                        batch_results.push(json!({
                            "work": spec.work,
                            "status": "transport_fault",
                            "error": format!("{err:#}"),
                        }));
                        for rem in &specs[idx + 1..] {
                            batch_results.push(json!({ "work": rem.work, "status": "not_run" }));
                        }
                        let out = json!({
                            "ok": false,
                            "step": base_p.step,
                            "stop_reason": "transport",
                            "error": format!("{err:#}"),
                            "renew": renew_summary,
                            "batches": batch_results,
                            "jobs": all_jobs,
                        });
                        util::print_line(&ps::json_compact(&out))?;
                        return Ok(1);
                    }
                    all_ok = false;
                    batch_results.push(json!({
                        "work": spec.work,
                        "status": "failed",
                        "error": format!("{err:#}"),
                    }));
                    continue;
                }
            }
        }

        let mut taken_zt = Some(zt);
        let step_res: Result<(i32, Value)> = (|| {
            let mut c = new_compiler(
                p,
                docs.clone(),
                explain_args.to_string(),
                limits.to_string(),
                helper.to_string(),
                taken_zt.take(),
                true,
            )?;
            let rc_res = c.run_step();
            taken_zt = c.zt.take();
            let rc = rc_res?;
            let last = c.last_out.unwrap_or_else(|| json!({}));
            if !spec.jobs_file.is_empty() {
                let jobs = last.get("jobs").and_then(Value::as_array).cloned().unwrap_or_default();
                if let Some(parent) = std::path::Path::new(&spec.jobs_file).parent()
                    && !parent.as_os_str().is_empty()
                {
                    std::fs::create_dir_all(parent)?;
                }
                util::write_atomic(&spec.jobs_file, &serde_json::to_string_pretty(&jobs)?)?;
            }
            Ok((rc, last))
        })();
        zt = match taken_zt {
            Some(client) => client,
            None => crate::zt::start()?,
        };
        match step_res {
            Ok((rc, last)) => {
                let jobs = last.get("jobs").and_then(Value::as_array).cloned().unwrap_or_default();
                all_jobs.extend(jobs);
                let ok_batch = rc == 0 && last.get("ok") != Some(&json!(false));
                if !ok_batch {
                    all_ok = false;
                }
                batch_results.push(json!({
                    "work": spec.work,
                    "status": if ok_batch { "ok" } else { "failed" },
                    "rc": rc,
                    "jobs_file": if spec.jobs_file.is_empty() { Value::Null } else { json!(spec.jobs_file) },
                    "result": last,
                }));
            }
            Err(err) => {
                if let Some(refusal) = crate::lockcheck::stopped_refusal(&err) {
                    batch_results.push(json!({ "work": spec.work, "status": "locked", "refusal": refusal }));
                    for rem in &specs[idx + 1..] {
                        batch_results.push(json!({ "work": rem.work, "status": "not_run" }));
                    }
                    let code = crate::lockcheck::refusal_code(&refusal);
                    let out = json!({
                        "ok": false,
                        "step": base_p.step,
                        "code": code,
                        "refusal": refusal,
                        "renew": renew_summary,
                        "batches": batch_results,
                        "jobs": all_jobs,
                    });
                    util::print_line(&ps::json_compact(&out))?;
                    return Ok(crate::lockcheck::LOCKED_EXIT);
                }
                if is_transport_fault(&err) {
                    batch_results.push(json!({
                        "work": spec.work,
                        "status": "transport_fault",
                        "error": format!("{err:#}"),
                    }));
                    for rem in &specs[idx + 1..] {
                        batch_results.push(json!({ "work": rem.work, "status": "not_run" }));
                    }
                    let out = json!({
                        "ok": false,
                        "step": base_p.step,
                        "stop_reason": "transport",
                        "error": format!("{err:#}"),
                        "renew": renew_summary,
                        "batches": batch_results,
                        "jobs": all_jobs,
                    });
                    util::print_line(&ps::json_compact(&out))?;
                    return Ok(1);
                }
                all_ok = false;
                batch_results.push(json!({
                    "work": spec.work,
                    "status": "failed",
                    "error": format!("{err:#}"),
                }));
            }
        }
    }

    let out = json!({
        "ok": all_ok,
        "step": base_p.step,
        "renew": renew_summary,
        "batches": batch_results,
        "jobs": all_jobs,
    });
    util::print_line(&ps::json_compact(&out))?;
    Ok(if all_ok { 0 } else { 1 })
}

impl Compiler {
    fn run_step(&mut self) -> Result<i32> {
        match self.p.step.as_str() {
            "brief" => self.step_brief()?,
            "validate" => self.step_validate()?,
            "apply" => self.step_apply()?,
            "derive" => self.step_derive()?,
            "review" => self.step_review()?,
            "review-audit" => return self.step_review_audit(),
            "split" => return self.step_split(),
            "close" => self.step_close()?,
            "defer" => self.step_defer()?,
            "plan-status" => self.step_plan_status()?,
            "report" => self.step_report()?,
            "resume" => self.step_resume()?,
            "explain" => self.step_explain()?,
            "explain-check" => self.step_explain_check()?,
            "explain-apply" => self.step_explain_apply()?,
            _ => self.step_explain_report()?,
        }
        Ok(0)
    }
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

/// Check one rendered `plan.json` text field with the exact shared `zt_check::text` R22 helpers.
fn check_math_field(faults: &mut Vec<String>, label: &str, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    if zt_check::text::open_math(trimmed) {
        faults.push(format!("{label}: leaves an inline formula open (odd number of $)"));
    }
    for tag in zt_check::text::inline_tag_in_math(trimmed) {
        faults.push(format!(
            "{label}: inline formula uses {tag} (\\tag is allowed only in $$...$$ display blocks)"
        ));
    }
    for tok in zt_check::text::bare_latex_in_prose(trimmed) {
        faults.push(format!(
            "{label}: contains bare LaTeX '{tok}' outside $...$; wrap math expressions in $...$"
        ));
    }
}

/// Validate every `plan.json` field that the model compiler renders verbatim into Cards, using the
/// shared `zt_check::text` helpers so architect validation enforces the same math rules as R22.
pub(crate) fn plan_math_faults(plan: &Value) -> Vec<String> {
    let mut faults = Vec::new();
    for field in ["title", "card", "estimand"] {
        check_math_field(&mut faults, &format!("plan.{field}"), &s(&plan[field]));
    }
    for (i, div) in list(&plan["divisions"]).iter().enumerate() {
        check_math_field(&mut faults, &format!("plan.divisions[{i}].name"), &s(&div["name"]));
    }
    for (i, tag) in pipe(&plan["family_tags"]).iter().enumerate() {
        check_math_field(&mut faults, &format!("plan.family_tags[{i}]"), &s(tag));
    }
    let part_ids: HashSet<String> = list(&plan["parts"])
        .iter()
        .map(|p| ps::key(&s(&p["id"])))
        .filter(|id| !id.is_empty())
        .collect();
    for (i, row) in list(&plan["symbols"]).iter().enumerate() {
        let cells = pipe(row);
        for col in [0usize, 1usize] {
            if let Some(cell) = cells.get(col) {
                check_math_field(&mut faults, &format!("plan.symbols[{i}][{col}]"), &s(cell));
            }
        }
        if let Some(cell) = cells.get(2) {
            let val = s(cell);
            if !val.trim().is_empty() && !part_ids.contains(&ps::key(val.trim())) {
                check_math_field(&mut faults, &format!("plan.symbols[{i}][2]"), &val);
            }
        }
    }
    for (i, row) in list(&plan["terms"]).iter().enumerate() {
        for (col, cell) in pipe(row).iter().enumerate() {
            check_math_field(&mut faults, &format!("plan.terms[{i}][{col}]"), &s(cell));
        }
    }
    for (i, row) in list(&plan["variables"]).iter().enumerate() {
        let cells = pipe(row);
        for col in 0..4usize {
            if let Some(cell) = cells.get(col) {
                check_math_field(&mut faults, &format!("plan.variables[{i}][{col}]"), &s(cell));
            }
        }
    }
    for (idx, part) in list(&plan["parts"]).iter().enumerate() {
        let raw_id = s(&part["id"]);
        let id = if raw_id.is_empty() { format!("#{}", idx + 1) } else { raw_id };
        for field in ["title", "facet", "use", "scope", "from", "to", "why", "regressor", "source_label"] {
            check_math_field(&mut faults, &format!("part {id}.{field}"), &s(&part[field]));
        }
        for (j, rest) in list(&part["rests"]).iter().enumerate() {
            let cells = pipe(rest);
            if let Some(why) = cells.get(1) {
                check_math_field(&mut faults, &format!("part {id}.rests[{j}][1]"), &s(why));
            }
        }
        if part["change"].is_object() {
            check_math_field(&mut faults, &format!("part {id}.change.text"), &s(&part["change"]["text"]));
        }
        if part["terms"].is_object() {
            for key in ["outcome", "regressor", "fe", "controls", "sample", "estimator", "se"] {
                match &part["terms"][key] {
                    Value::Array(items) => {
                        for (k, item) in items.iter().enumerate() {
                            check_math_field(&mut faults, &format!("part {id}.terms.{key}[{k}]"), &s(item));
                        }
                    }
                    other => {
                        check_math_field(&mut faults, &format!("part {id}.terms.{key}"), &s(other));
                    }
                }
            }
        }
        for (j, just) in list(&part["justifications"]).iter().enumerate() {
            check_math_field(
                &mut faults,
                &format!("part {id}.justifications[{j}].extra_assumptions"),
                &s(&just["extra_assumptions"]),
            );
            let loc = s(&just["location"]).to_lowercase();
            let jref = s(&just["ref"]);
            let is_local_part_id = loc == "local" && !jref.starts_with("inline:") && !jref.starts_with('§');
            if !is_local_part_id {
                check_math_field(&mut faults, &format!("part {id}.justifications[{j}].ref"), &jref);
            }
        }
        let eq = s(&part["equation"]);
        let eq_trim = eq.trim();
        if !eq_trim.is_empty() {
            if eq_trim.contains("$$") {
                faults.push(format!(
                    "part {id}.equation: omit $$ delimiters (the compiler wraps empirical equation in $$ ... $$)"
                ));
            } else if zt_check::text::broken_display(&format!("$$ {eq_trim} $$")) {
                faults.push(format!(
                    "part {id}.equation: produces a broken display formula when wrapped in $$ ... $$"
                ));
            }
        }
    }
    faults
}

/// True when a Derivation part's `to` field actually states a mathematical expression rather than
/// only a prose reference label like `式 (18) 与 (19)`.
fn has_math_expr(to: &str) -> bool {
    to.chars().any(|c| matches!(c, '$' | '=' | '\\' | '<' | '>' | '≤' | '≥' | '≠' | '≈' | '∼' | '∝' | '∈' | '→' | '⇒' | '⟺' | '≡'))
}

/// Format a `close` 修正 note for blocking R2 (>40 char) faults on the Model Card head itself.
fn head_r2_note(model: &str, check_lines: &[String]) -> Option<String> {
    let r2: Vec<String> = check_lines
        .iter()
        .filter_map(|line| {
            let c = STYLE_FAULT.captures(line)?;
            if !ps::eq(&c[1], model) {
                return None;
            }
            let msg = &c[2];
            if msg.starts_with("R2 ") && !msg.contains("(advisory)") {
                Some(msg.to_string())
            } else {
                None
            }
        })
        .collect();
    if r2.is_empty() {
        return None;
    }
    Some(format!(
        "## 修正：[[{model}]]\n\n构建检查（模型头卡短句 R2）：这张模型卡正文有以下句子超过 40 个汉字：\n{}\n把上述长句拆成 40 字以内的短句，保留全部问题、概要、数据、样本、比较对象、单位、限定条件和既有链接；不要改动 **结构**、**引入**、**分部**、**主要估计** 等脚本节，不要移动卡片。改完照常运行 CHECK。\n",
        r2.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
    ))
}

/// Returns `true` when `path` begins with a UTF-8 BOM (`EF BB BF`).
fn has_utf8_bom(path: &str) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut prefix = [0u8; 3];
    matches!(file.read_exact(&mut prefix), Ok(()) if prefix == [0xEF, 0xBB, 0xBF])
}

impl Compiler {
    /// The most numbered steps one Derivation part holds: zt-check holds a Model part to twice the
    /// base `MaxSteps` (`zt_check::core::loosen`, rule version 6). The base still goes to zt-check,
    /// which doubles it itself; the plan check and the reviewer see the doubled number.
    fn part_steps(&self) -> i64 {
        zt_check::core::loosen(self.p.max_steps, true)
    }

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
        let (done, failure) = result.context("zt helper failed")?;
        let mut addresses = Vec::new();
        for d in &done {
            if let Some(c) = OK_LINE.captures(&d.line) {
                addresses.push(c[1].trim().to_string());
            }
        }
        if let Some(failure) = failure {
            if let Some(refusal) = failure.refusal.as_ref().filter(|r| crate::lockcheck::is_planning_stop(r)) {
                return Err(anyhow::Error::new(crate::lockcheck::Stopped(refusal.clone())));
            }
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
        track_renames(&mut self.renamed, map);
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
        util::write_atomic(&self.state_file, &ps::json_pretty(&self.state))
    }

    fn out_last(&mut self, value: &Value) -> Result<()> {
        self.last_out = Some(value.clone());
        if !self.silent_out {
            util::print_line(&ps::json_compact(value))?;
        }
        Ok(())
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

    /// The model's `tables.json` (an empirical model's brief writes it), if there is one.
    fn tables_of(&self, model: &str) -> Option<Value> {
        let path = ps::join(&self.dir(model), "tables.json");
        if ps::exists(&path) { util::read_json(&path).ok() } else { None }
    }

    /// The model's pinned `inventory.json` (a formal model's brief writes it), surfacing parse
    /// errors explicitly rather than silently skipping Layer 3 validation (Astra #12) and merging
    /// optional reader-curated `obligations.json` (Astra H3).
    fn inventory_result(&self, model: &str) -> std::result::Result<Option<Value>, String> {
        let dir = self.dir(model);
        let path = ps::join(&dir, "inventory.json");
        if !ps::exists(&path) {
            return Ok(None);
        }
        let mut inv = util::read_json(&path)
            .map_err(|e| format!("inventory_invalid: `{path}` could not be parsed as JSON: {e:#}"))?;
        let ob_path = ps::join(&dir, "obligations.json");
        super::inventory::merge_obligations_file(&mut inv, &ob_path)?;
        Ok(Some(inv))
    }

    fn inventory_of(&self, model: &str) -> Option<Value> {
        self.inventory_result(model).ok().flatten()
    }

    fn inventory_required_for(&self, model: &str, staging: Option<&str>) -> bool {
        ps::exists(&ps::join(&self.dir(model), "brief.md"))
            || staging.is_some_and(|st| std::path::Path::new(&ps::join(st, "sections")).is_dir())
    }

    /// Build an effective plan parts list that includes any split pieces recorded in `m["added"]`
    /// right after their predecessor, inheriting `src` from their origin part and updating any
    /// subsequent Side part's `after` pointer (Astra #7).
    fn effective_plan_parts(&self, model: &str, plan: &Value) -> Vec<Value> {
        let mut parts: Vec<Value> = list(&plan["parts"]).into_iter().cloned().collect();
        let Some(added_map) = self.m(model)["added"].as_object() else {
            return parts;
        };
        if added_map.is_empty() {
            return parts;
        }
        let mut added_entries: Vec<(String, Value)> = added_map
            .iter()
            .filter(|(_, v)| v.is_object())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        // Sort by base id then numeric suffix (`~2`, `~3`, ...) so chained splits insert in order.
        added_entries.sort_by(|a, b| {
            let key = |id: &str| {
                if let Some((base, suf)) = id.rsplit_once('~') {
                    (base.to_string(), suf.parse::<u64>().unwrap_or(u64::MAX))
                } else {
                    (id.to_string(), 0)
                }
            };
            key(&a.0).cmp(&key(&b.0))
        });
        for (new_id, mut entry) in added_entries {
            if parts.iter().any(|p| ps::eq(&s(&p["id"]), &new_id)) {
                continue;
            }
            let base_id = new_id.split_once('~').map(|(b, _)| b).unwrap_or(&new_id);
            if entry["src"].is_null()
                && let Some(origin) = parts.iter().find(|p| ps::eq(&s(&p["id"]), base_id))
            {
                entry["src"] = origin["src"].clone();
            }
            let pred_id = if truthy(&entry["after"]) {
                s(&entry["after"])
            } else {
                base_id.to_string()
            };
            if ps::eq(&s(&entry["line"]), "side") && !pred_id.is_empty() {
                for p in &mut parts {
                    if ps::eq(&s(&p["after"]), &pred_id) && !ps::eq(&s(&p["id"]), &new_id) {
                        p["after"] = json!(new_id);
                    }
                }
            }
            match parts.iter().position(|p| ps::eq(&s(&p["id"]), &pred_id)) {
                Some(idx) => parts.insert(idx + 1, entry),
                None => parts.push(entry),
            }
        }
        parts
    }

    /// Level 2 formal close-round card checks: FM1–FM6 and pinned `inventory.json` equation fidelity.
    /// Checks ONLY the model's own filed parts (never borrowed `take` cards from other models, Astra #15),
    /// scoping equation tags to each part's pinned `src` file(s) and surfacing corrupt/missing inventory (Astra H2/H4).
    fn formal_check(&mut self, model: &str, st: &Value) -> Result<Vec<(String, String)>> {
        let mut own = Vec::new();
        let mut src_files_by_addr: CiMap<Vec<String>> = CiMap::new();
        for id in ps::keys(&self.m(model)["ids"]) {
            let p = self.plan_part(model, &id)?;
            if p.as_ref().is_some_and(|p| truthy(&p["take"])) {
                continue;
            }
            let addr = self.id_address(model, &id);
            if !addr.is_empty() && !addr.contains('#') {
                own.push(addr.clone());
                if let Some(part) = p {
                    let spans: Vec<String> = pipe(&part["src"]).into_iter().map(s).filter(|x| !x.is_empty()).collect();
                    let files = super::inventory::src_files_of_spans(&spans);
                    if !files.is_empty() {
                        src_files_by_addr.insert(&addr, files);
                    }
                }
            }
        }
        let folder = self.run_folder();
        let staging = (!folder.is_empty()).then_some(folder.as_str());
        let mut extra_faults: Vec<(String, String)> = Vec::new();
        let inv = match self.inventory_result(model) {
            Ok(Some(inv)) => Some(inv),
            Ok(None) => {
                if self.inventory_required_for(model, staging) {
                    extra_faults.push((
                        model.to_string(),
                        format!(
                            "## 修正：[[{model}]]\n\n清单检查（模型 [[{model}]]）：\n- inventory_missing: formal model requires pinned inventory.json; re-extract inventory before finish.\n"
                        ),
                    ));
                }
                None
            }
            Err(msg) => {
                extra_faults.push((
                    model.to_string(),
                    format!(
                        "## 修正：[[{model}]]\n\n清单检查（模型 [[{model}]]）：\n- {msg}\n"
                    ),
                ));
                None
            }
        };
        let head_body = s(&st["head"]["body"]);
        let cards: Vec<(String, String, String, Vec<String>)> = list(&st["parts"])
            .into_iter()
            .filter(|p| ps::is_in(&s(&p["address"]), &own))
            .map(|p| {
                let addr = s(&p["address"]);
                let files = src_files_by_addr.get(&addr).cloned().unwrap_or_default();
                (addr, s(&p["facet"]), s(&p["body"]), files)
            })
            .collect();
        let mut out = super::formal::formal_entries_scoped(model, &head_body, &cards, inv.as_ref());
        out.extend(extra_faults);
        Ok(out)
    }

    /// Every number of the source lines the plan's parts name, and of the model's tables: what
    /// the evidence check reads an empirical card's numbers against.
    fn source_numbers(&mut self, model: &str) -> Result<BTreeSet<String>> {
        let mut out: BTreeSet<String> = BTreeSet::new();
        if let Some(tables) = self.tables_of(model) {
            out.extend(super::tables::table_numbers(&tables));
        }
        let plan = self.plan(model)?;
        for p in list(&plan["parts"]) {
            for r in list(&p["src"]) {
                let r = s(r);
                let Some(c) = SOURCE_REF.captures(&r) else { continue };
                let file = c[1].to_string();
                let from: usize = c[2].parse().unwrap_or(1);
                let to: usize = c.get(3).and_then(|t| t.as_str().parse().ok()).unwrap_or(from);
                let path = if ps::rooted(&file) { file } else { ps::join(&self.run_folder(), &file) };
                if let Ok(lines) = ps::read_lines(&path) {
                    let text = lines.iter().skip(from.saturating_sub(1)).take(to + 1 - from.min(to)).cloned().collect::<Vec<_>>().join("\n");
                    out.extend(super::tables::numbers(&text));
                }
            }
        }
        // The Model Card's estimand and the plan's interpretation are the architect's words, not
        // numbers to check; the brief and context pack are not a source either.
        Ok(out)
    }

    /// The evidence check of one empirical model (docs/empirical-models-proposal.md §8.2 step 6):
    /// number fidelity and the EM card rules of its own parts, as 修正 notes.
    fn evidence_check(&mut self, model: &str, st: &Value) -> Result<Vec<(String, String)>> {
        let own: Vec<String> = self.m(model)["ids"].as_object().map(|ids| ids.values().map(s).collect()).unwrap_or_default();
        let source = self.source_numbers(model)?;
        let plan = self.plan(model)?;
        let designs = super::empirical::designs(&plan);
        let cards: Vec<(String, String, String)> = list(&st["parts"])
            .into_iter()
            .filter(|p| ps::is_in(&s(&p["address"]), &own))
            .map(|p| (s(&p["address"]), s(&p["facet"]), s(&p["body"])))
            .collect();
        Ok(super::empirical::evidence_entries(model, &cards, &designs, &source))
    }

    /// The evidence review of one empirical model (docs/empirical-models-proposal.md §8.2 step 5): one job
    /// per twelve cards, each its own conversation of one message (the reviewer is not the writer), each card
    /// with its source lines; the jobs run side by side. The reviewer re-reads its cards, fixes what the source
    /// contradicts, and answers one line. Pilot 2026-11-09: one conversation of six sequential messages took 19
    /// of the model's 35 minutes, and each later message re-read the whole conversation.
    fn evidence_review_jobs(&mut self, model: &str) -> Result<Vec<Value>> {
        let plan = self.plan(model)?;
        let mut parts: Vec<(Value, String)> = Vec::new();
        for p in list(&plan["parts"]) {
            if truthy(&p["take"]) {
                continue;
            }
            let id = s(&p["id"]);
            let Ok(address) = self.resolve(&json!(id), model) else { continue };
            let src = self.source_lines(&list(&p["src"]))?;
            let src = if src.is_empty() { "（规划没有给出原文行：按设定里的原书表格核对。）".to_string() } else { src };
            parts.push((json!(id), format!("## [[{address}]] {}（{}）\r\n\r\n{src}", s(&p["title"]), s(&p["facet"]))));
        }
        let total = parts.len().div_ceil(12);
        let reread = "先用一条命令从档案读回本消息列出的每张卡（把每个地址写成一条 get_card，一次传给 HELPER），不要凭记忆核对。";
        let dir = self.dir(model);
        let mut jobs = Vec::new();
        for (k, chunk) in parts.chunks(12).enumerate() {
            let ids: Vec<Value> = chunk.iter().map(|(id, _)| id.clone()).collect();
            let blocks: Vec<String> = chunk.iter().map(|(_, block)| block.clone()).collect();
            let text = format!("## 第 {} 组，共 {total} 组\r\n\r\n{reread}\r\n\r\n{}", k + 1, blocks.join("\r\n\r\n"));
            let mut job = self.new_job(model, "e", ids, &text, "按以上说明核对这一组卡，改正原文不支持的地方，最后只回复一行 JSON。")?;
            let file = s(&self.state["jobs"][&s(&job["id"])]["file"]);
            job["review_kind"] = json!("empirical");
            job["files"] = json!([self.docs.review_empirical, ps::join(&dir, "setting.md"), file]);
            jobs.push(job);
        }
        Ok(jobs)
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
        let mut faults = division_faults(plan);
        if !ps::eq(&s(&plan["model"]), model) || plan["model"].is_null() {
            faults.push(format!("plan.model is '{}', expected '{model}'", s(&plan["model"])));
        }
        if ps::eq(&s(&plan["status"]), "skip") && !plan["status"].is_null() {
            return Ok(faults);
        }
        let level = s(&plan["level"]);
        let formal = ps::eq(&level, "formal") && !plan["level"].is_null();
        let is_math_unit = is_math_unit_structure(structure);
        let formal_model = super::formal::is_formal_model(plan, is_math_unit);
        let concept = ps::eq(&level, "concept") && !plan["level"].is_null();
        let empirical = ps::eq(&level, "empirical") && !plan["level"].is_null();
        if !formal && !concept && !empirical {
            faults.push("level must be concept, formal or empirical".into());
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
            // atkinson2015lectures (2026-11-01): a take of another model's Model Card itself maps the id to no
            // part, and model-finish refused the model ("mapped but belongs to no model") after the whole build.
            if truthy(&p["take"]) && !s(&p["take"]).contains('#') {
                let card = self.card(&p["take"]).unwrap_or(Value::Null);
                if whole_model_take(&card) && !self.unit(&s(&p["take"])) {
                    faults.push(format!(
                        "take {id} ({}): that is a Model Card, not a part; take the one part this model uses (its address, or <model>#<id>), or drop the take and refer to the model with a Link in prose",
                        s(&p["take"])
                    ));
                }
            }
            let line = line_of(p);
            if !ps::is_in(&line, &["main", "side", "off", "shared"]) {
                faults.push(format!("{id}: line must be main, side, or off"));
            }
            if ps::eq(&line, "side") && !formal_model && !truthy(&p["of"]) {
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
                if !truthy(&p["spec"]) && !derives(p) && !(formal_model && super::formal::role(p) == Some(zt_check::formal::FormalRole::Notation)) {
                    faults.push(format!("{id}: a new part needs spec (what it must state)"));
                }
            }
            if derives(p) {
                if concept {
                    faults.push(format!("{id}: a concept model has no Derivation parts"));
                }
                if !formal_model && !ps::eq(&line, "main") {
                    faults.push(format!("{id}: a Derivation part stands on the main line"));
                }
                if truthy(&p["facet"]) && !ps::starts_with(&s(&p["facet"]), "推导") {
                    faults.push(format!("{id}: a Derivation part's facet starts with 推导"));
                }
                if !truthy(&p["to"]) {
                    faults.push(format!("{id}: a Derivation part needs to (its result equation)"));
                } else if !has_math_expr(&s(&p["to"])) {
                    faults.push(format!(
                        "{id}: to '{}' must state the result equation in math (e.g. $$...$$), not only a reference label",
                        s(&p["to"])
                    ));
                }
                if truthy(&p["steps"]) && ps::gt(&p["steps"], self.part_steps()) {
                    faults.push(format!(
                        "{id}: {} steps is over MaxSteps {}; make it a chain of Derivation parts",
                        s(&p["steps"]),
                        self.part_steps()
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
            let borrows_or_takes_notation = formal_model
                && parts.iter().any(|p| {
                    truthy(&p["borrows_notation"])
                        || (truthy(&p["take"]) && super::formal::role(p) == Some(zt_check::formal::FormalRole::Notation))
                });
            if list(&plan["symbols"]).is_empty() && !borrows_or_takes_notation {
                faults.push("a formal model needs symbols (the setting's symbol table)".into());
            }
            if list(&plan["terms"]).is_empty() && !borrows_or_takes_notation {
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
                        "closure: {id} rests on {reference}, which is outside the model; take it in (an import), move it in, or write a variant that specializes it; restate it only when it is no part of a model"
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
        // §3.7: the architect designs the explanations of every chain of Derivation parts, so a
        // missing design is a plan fault and never a gap the writer fills alone.
        faults.extend(Self::plan_explain_faults(plan));
        faults.extend(plan_math_faults(plan));
        for m in list(&plan["merge"]) {
            if !ids.contains(&s(&m["into"])) && !members.contains(&s(&m["into"])) {
                faults.push(format!("merge into '{}' is not a part", s(&m["into"])));
            }
        }
        if !list(&plan["merge"]).is_empty() {
            let tree = self.tree_addresses(model)?;
            faults.extend(Self::merge_faults(plan, &tree)?);
        }
        faults.extend(Self::take_faults(plan));
        // Level 3: the empirical plan schema (docs/empirical-models-proposal.md §8.3).
        if empirical {
            let tables = self.tables_of(model);
            faults.extend(super::empirical::plan_faults(plan, tables.as_ref()));
        }
        // Level 2 formal economic model: strict role, placement, justification, and inventory gates (docs/formal-models-proposal.md §8).
        if formal_model {
            let folder = self.run_folder();
            let staging = (!folder.is_empty()).then_some(folder.as_str());
            match self.inventory_result(model) {
                Ok(Some(inv)) => {
                    faults.extend(super::formal::plan_faults(plan, Some(&inv), staging));
                }
                Ok(None) => {
                    if self.inventory_required_for(model, staging) {
                        faults.push("inventory_missing: formal model requires pinned inventory.json beside brief.md; run brief extraction before validate".into());
                    }
                    faults.extend(super::formal::plan_faults(plan, None, staging));
                }
                Err(msg) => {
                    faults.push(msg);
                    faults.extend(super::formal::plan_faults(plan, None, staging));
                }
            }
        }
        Ok(faults)
    }

    /// #197: the takes no part of the plan uses. A taken part joins the model only through a rests-on
    /// Link (or as the part a side part is `of`), so a take nothing names stays outside the build order and
    /// `model-finish` refuses the model ("mapped but belongs to no model").
    fn take_faults(plan: &Value) -> Vec<String> {
        let parts = list(&plan["parts"]);
        let mut used: HashSet<String> = HashSet::new();
        for p in &parts {
            for r in list(&p["rests"]) {
                used.insert(s(&ps::at(&pipe(r), 0)));
            }
            for field in ["of", "after", "criterion", "calibration", "borrows_notation"] {
                if truthy(&p[field]) {
                    used.insert(s(&p[field]));
                }
            }
            for a in pipe(&p["assumptions"]) {
                let aid = s(a);
                if !aid.is_empty() {
                    used.insert(aid);
                }
            }
        }
        let mut faults: Vec<String> = parts
            .iter()
            .filter(|p| truthy(&p["take"]) && !used.contains(&s(&p["id"])))
            .map(|p| {
                format!(
                    "take {} ({}): no part rests on it; a taken part joins the model only through rests, so name it in the rests of the part that uses it, or drop the take and refer to it with a Link in prose",
                    s(&p["id"]),
                    s(&p["take"])
                )
            })
            .collect();
        // autor2004will: `5#S` named the model by a number; the build then failed. The model before `#` is its
        // Model Card's address (`<key>/<location>#<id>`).
        for p in parts.iter().filter(|p| truthy(&p["take"])) {
            let take = s(&p["take"]);
            if let Some((model, _)) = take.split_once('#')
                && !model.contains('/')
            {
                faults.push(format!(
                    "take {} ({take}): `{model}` is no Model Card address; write the other model's address before #, as in <key>/5|a#S",
                    s(&p["id"])
                ));
            }
        }
        faults
    }

    /// #197: the merges the build cannot carry out. `apply` deletes a merged part only when it has no
    /// successor (a delete takes its successors with it); any other merge leaves the part in the model
    /// without a plan id, and `model-finish` refuses the model at the end of the batch.
    fn merge_faults(plan: &Value, tree: &HashSet<String>) -> Result<Vec<String>> {
        let mut faults = Vec::new();
        for m in list(&plan["merge"]) {
            let from = s(&m["from"]);
            if !from.is_empty() && Self::has_successors(&from, tree)? {
                faults.push(format!(
                    "merge from {from}: it has successors, so it cannot be deleted; list it with existing (refill: true to rewrite it) instead"
                ));
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

    /// Plan id -> (k, n, tos): every maximal contiguous run of Derivation parts on the plan's
    /// main line (math units) or any Side branch (Level 2 formal models, breaking at non-Derivation steps like `论证`).
    fn plan_chains(plan: &Value) -> CiMap<(usize, usize, Vec<String>)> {
        super::formal::plan_chains(plan)
    }

    /// §3.7 & P2b-2: every maximal contiguous Derivation chain (main line or Side branch),
    /// each as its plan ids in build order.
    fn plan_derivation_chains(plan: &Value) -> Vec<Vec<String>> {
        super::formal::plan_derivation_chains(plan)
    }

    /// §3.7: the architect's `explain` design of every chain of Derivation parts — the chain's route,
    /// and each part's shows, notes and meaning. The batch's explain job writes 思路/说明/解读 from this
    /// entry, so a chain without one, or an incomplete one, is a plan fault (the writer must not invent
    /// the design alone).
    fn plan_explain_faults(plan: &Value) -> Vec<String> {
        let mut faults = Vec::new();
        let chains = Self::plan_derivation_chains(plan);
        let entries: Vec<Value> = list(&plan["explain"]).into_iter().cloned().collect();
        for chain in &chains {
            let named = chain.join(",");
            let Some(entry) = entries.iter().find(|e| pipe(&e["chain"]).into_iter().map(s).collect::<Vec<_>>() == *chain) else {
                faults.push(format!(
                    "chain {named} has no explain entry: design the chain's route and each part's shows, notes and meaning (model-architect.md)"
                ));
                continue;
            };
            if s(&entry["route"]).trim().is_empty() {
                faults.push(format!("explain chain {named}: route is empty (the key move of the whole chain, and why it works)"));
            }
            let designed: Vec<Value> = list(&entry["parts"]).into_iter().cloned().collect();
            for id in chain {
                let Some(part) = designed.iter().find(|p| ps::eq(&s(&p["id"]), id)) else {
                    faults.push(format!("explain chain {named}: part {id} has no shows, notes and meaning"));
                    continue;
                };
                for field in ["shows", "meaning"] {
                    if s(&part[field]).trim().is_empty() {
                        faults.push(format!("explain chain {named}: part {id} has no {field}"));
                    }
                }
                if !part["notes"].is_null() && !part["notes"].is_string() {
                    faults.push(format!("explain chain {named}: part {id}: notes must be one string of the steps that need a 说明 line"));
                }
            }
            for part in &designed {
                let id = s(&part["id"]);
                if !id.is_empty() && !chain.contains(&id) {
                    faults.push(format!("explain chain {named}: part {id} is not in the chain"));
                }
            }
        }
        for entry in &entries {
            let ids: Vec<String> = pipe(&entry["chain"]).into_iter().map(s).collect();
            if !ids.is_empty() && !chains.contains(&ids) {
                faults.push(format!(
                    "explain entry names chain {}, which is not a chain of Derivation parts",
                    ids.join(",")
                ));
            }
        }
        faults
    }

    /// §3.7: the design section of a batch explain job, for every plan chain that still has a part to
    /// write. A missing entry here is a stale plan (the plan's own rules refuse it): the writer is never
    /// asked to invent the design alone.
    fn explain_design(&mut self, model: &str, missing: &[String]) -> Result<String> {
        let plan_path = ps::join(&self.dir(model), "plan.json");
        let plan = util::read_json(&plan_path).with_context(|| format!("no plan.json holds the explain design of {model}"))?;
        let entries: Vec<Value> = list(&plan["explain"]).into_iter().cloned().collect();
        let ids = self.m(model)["ids"].clone();
        let mut out = String::new();
        let mut eff_plan = plan.clone();
        eff_plan["parts"] = Value::Array(self.effective_plan_parts(model, &plan));
        for chain in Self::plan_derivation_chains(&eff_plan) {
            let addresses: Vec<String> = chain.iter().map(|id| s(&ids[id])).collect();
            if !addresses.iter().any(|a| ps::is_in(a, missing)) {
                continue;
            }
            let base_chain: Vec<String> = {
                let mut v = Vec::new();
                for id in &chain {
                    let b = id.split_once('~').map(|(x, _)| x).unwrap_or(id).to_string();
                    if !v.contains(&b) {
                        v.push(b);
                    }
                }
                v
            };
            let named = chain.join(",");
            let entry = entries
                .iter()
                .find(|e| {
                    let ec: Vec<String> = pipe(&e["chain"]).into_iter().map(s).collect();
                    ec == chain || ec == base_chain
                })
                .with_context(|| format!("plan.json of {model} has no explain entry for chain {named}; the plan must be designed again"))?;
            if out.is_empty() {
                out = "## 解释设计（Sonnet 建筑师为本模型写下的；照它写思路、说明和解读）\n\n".into();
            }
            out += &format!("### 链 {named}\n\n全链路线：{}\n", s(&entry["route"]));
            for id in &chain {
                let base_id = id.split_once('~').map(|(x, _)| x).unwrap_or(id);
                if let Some(part) = list(&entry["parts"]).into_iter().find(|p| ps::eq(&s(&p["id"]), id) || ps::eq(&s(&p["id"]), base_id)) {
                    out += &format!("- [[{}]] {}\n", s(&ids[id]), s(&part["shows"]));
                    let notes = s(&part["notes"]);
                    if !notes.trim().is_empty() {
                        out += &format!("  - 需要说明的步骤：{}\n", notes.trim());
                    }
                    out += &format!("  - 结果的解读：{}\n", s(&part["meaning"]));
                }
            }
            out += "\n";
        }
        Ok(out)
    }

    /// One explain job for one model (§3.7, mode `explain`): every Derivation part that still lacks
    /// 思路 or 解读, in its chain's order, with the compiler's position line and (with `-Batch`) the
    /// architect's design entry, continuing the model's own conversation.
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
            let own = Self::own_derivations(&st)?;
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
                // Its own file: the brief's context.md is the source review's evidence, which
                // model-finish checks byte for byte, and §3.7 runs this round before the finish.
                context = ps::join(&dir, "explain-context.md");
                if self.context(&model, &context).is_err() {
                    context = "无".into();
                }
            }
            let head = self.card(&json!(model))?;
            let mut head_body = s(&head["body"]);
            if ps::len16(&head_body) > 3000 {
                head_body = ps::cut16(&head_body, 3000) + "……";
            }
            // §3.7: the batch's job carries the architect's design of every chain it writes.
            let design = if self.p.batch { self.explain_design(&model, &missing)? } else { String::new() };
            let todo: Vec<String> = missing.iter().take(left as usize).cloned().collect();
            left -= todo.len() as i64;
            let seq = self.next_seq();
            let id = format!("{}:x{seq}", Self::slug(&model));
            let out = ps::join(&ps::join(&self.p.work, "out"), &format!("explain-{seq}.json"));
            let file = ps::join(&dir, &format!("explain-{seq}.md"));
            let mut text = format!(
                "# 讲解任务 {id}\n\n结果文件: {out}\nSELF-CHECK: {}\n上下文包: {context}\n模型卡: [[{model}]] {}\n\n## 模型卡的正文\n\n{head_body}\n\r\n",
                self.explain_check_command(&id),
                s(&head["title"])
            );
            text += &design;
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
            let mut positions = Map::new();
            for c in proof_chains(&st["build"]) {
                let parts: Vec<String> = pipe(&c["parts"]).into_iter().map(s).filter(|a| ps::is_in(a, &own)).collect();
                let want: Vec<String> = parts.iter().filter(|a| ps::is_in(a, &todo)).cloned().collect();
                if want.is_empty() {
                    continue;
                }
                let tos: Vec<String> = parts.iter().map(|a| text::result_of(&body(a))).collect();
                if let Some((a, t, b)) = around(&c["after"]) {
                    text += &format!("## 链之前的分卡（推导从它出发）：[[{a}]] {t}\n\n{b}\n\r\n");
                }
                let n = parts.len();
                for a in &want {
                    let k = (ps::index_of(&parts, a) + 1) as usize;
                    positions.insert(a.clone(), json!(text::chain_line(k, n, &tos)));
                    let place = if n >= 2 { format!("（第 {k} 张，共 {n} 张）") } else { String::new() };
                    text += &format!("## 推导分卡 [[{a}]] {}{place}\n\n{}\n\r\n", titles.get(a).cloned().unwrap_or_default(), body(a));
                }
                if let Some((a, t, b)) = around(&c["before"]) {
                    text += &format!("## 链之后的分卡（它用推导的结果）：[[{a}]] {t}\n\n{b}\n\r\n");
                }
            }
            // A Derivation part no chain holds (a split can leave one beside the chain): still its own job part.
            let loose: Vec<String> = todo.iter().filter(|a| !positions.contains_key(*a)).cloned().collect();
            for a in &loose {
                positions.insert(a.clone(), json!(""));
                text += &format!("## 推导分卡 [[{a}]] {}\n\n{}\n\r\n", titles.get(a).cloned().unwrap_or_default(), body(a));
            }
            util::write_text(&file, &text)?;
            let mut job_bodies = Map::new();
            for a in &todo {
                job_bodies.insert(a.clone(), json!(body(a)));
            }
            let mut job = json!({
                "id": id,
                "files": [self.docs.explain, ps::join(&dir, "setting.md"), file],
                "prompt": format!("按以上说明写出结果文件 {out}，最后只回复一行 JSON。"),
            });
            // §3.7: the batch's explain job continues the model's conversation; a lost session runs
            // once from the instruction files and the job file.
            if self.p.batch {
                job["continue"] = json!(format!("{}:brief", Self::slug(&model)));
                job["fallback"] = json!([self.docs.explain, ps::join(&dir, "setting.md"), file]);
            }
            self.state["explain"]["jobs"][&id] = json!({
                "model": model, "file": file, "out": out, "addresses": todo, "done": [], "round": 1,
                "bodies": job_bodies, "positions": positions,
            });
            jobs.push(job);
        }
        std::fs::create_dir_all(ps::join(&self.p.work, "out"))?;
        self.save_state()?;
        // Measure-Object -Sum: a double.
        let parts = jobs.iter().fold(0.0_f64, |sum, j| sum + pipe(&self.state["explain"]["jobs"][s(&j["id"])]["addresses"]).len() as f64);
        self.out_last(&json!({ "jobs": jobs, "parts": parts }))
    }

    /// `Get-Skeleton`: the body a new part starts with.
    fn skeleton(&self, part: &Value, plan: &Value, model: &str, previous: Option<&Value>, is_math_unit: bool) -> Result<String> {
        let formal_model = super::formal::is_formal_model(plan, is_math_unit);
        let mut links = Vec::new();
        for r in list(&part["rests"]).into_iter().filter(|r| truthy(&ps::at(&pipe(r), 0))) {
            links.push(self.link(r, model, "rests")?);
        }
        // In a Level 2 formal model, a filing edge (`of` or `after`) never by itself licenses a
        // premise/illustrates Link unless `kind` is explicitly provided (proposal §8.4).
        if truthy(&part["of"]) && (!formal_model || truthy(&part["kind"])) {
            let kind = if truthy(&part["kind"]) { s(&part["kind"]) } else { "illustrates".into() };
            links.insert(0, self.link(&json!([part["of"], part["why"], kind]), model, "rests")?);
        }
        // Level 3: the role's sections, with the numbers from the source's tables.
        if super::empirical::is_empirical(plan) && !derives(part) {
            let tables = self.tables_of(model);
            let resolve = |id: &str| self.resolve(&json!(id), model);
            return super::empirical::skeleton(part, plan, &links, &resolve, tables.as_ref());
        }
        // Level 2 formal economic model: role skeletons (<head>|a|a 符号表与读法, 判据, 结果, 论证, 校准, 数值实验).
        if formal_model && !derives(part) {
            let resolve = |id: &str| self.resolve(&json!(id), model);
            return super::formal::skeleton(part, plan, &links, &resolve);
        }
        if derives(part) {
            let mut start: Vec<String> = links.iter().map(|l| format!("- {l}")).collect();
            if let Some(prev) = previous.filter(|p| derives(p)) {
                start.insert(
                    0,
                    format!(
                        "- 依赖上一步 [[{}]] 的结果：{}。",
                        self.resolve(&prev["id"], model)?,
                        text::inline(&s(&prev["to"]))
                    ),
                );
            }
            if truthy(&part["from"]) {
                start.push(format!("- 从 {} 出发。", text::inline(&s(&part["from"]))));
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
        // A setting, an assumption or a definition explains itself: what it is, why the model needs it, how it enters.
        if text::wants_intuition(part, false) {
            body += &format!("\n\n{}", text::intuition_skeleton());
        }
        // Mathematical units (`form` set) keep their symbol and term tables inside `前提`.
        if is_math_unit && ps::eq(&s(&plan["level"]), "formal") && text::is_setting_facet(&s(&part["facet"])) && ps::eq(&line_of(part), "main") {
            body += &format!("\n\n{}", text::tables(plan));
        }
        Ok(body.trim().to_string())
    }

    /// `Get-StructureList`: the Model Card's 结构 list, one line per part of its own; a taken part is an
    /// `**引入**` line instead (`imports_of`, #212).
    fn structure_list(&self, plan: &Value, model: &str) -> Result<String> {
        let mut lines = Vec::new();
        for p in list(&plan["parts"]) {
            let address = self.resolve(&p["id"], model)?;
            let m = self.m(model);
            if truthy(&p["take"]) {
                continue;
            } else {
                let id = s(&p["id"]);
                let core = if ps::eq(&s(&p["emphasis"]), "core") { "（核心）" } else { "" };
                lines.push(format!("- {} [[{address}]]{core}：{}", s(&m["facets"][&id]), structure_text(&s(&m["titles"][&id]))));
            }
        }
        Ok(format!("**结构**\n\n{}", lines.join("\n")))
    }

    /// The Model Card's `**引入**` section from the plan's takes at their current addresses (#212):
    /// `"block": true` takes the Card with its block (整块); `None` when the plan takes nothing.
    fn imports_of(&self, plan: &Value, model: &str) -> Result<Option<String>> {
        let mut takes = Vec::new();
        for p in list(&plan["parts"]).into_iter().filter(|p| truthy(&p["take"])) {
            takes.push((truthy(&p["block"]), self.resolve(&p["id"], model)?, s(&p["use"])));
        }
        Ok(import_section(&takes))
    }

    /// A body with the plan's `**引入**` section, and without the legacy （共用） lines of its 结构 list that the
    /// section replaces; `None` when there is nothing to change.
    fn with_imports(&self, body: &str, plan: &Value, model: &str) -> Result<Option<String>> {
        let Some(wanted) = self.imports_of(plan, model)? else { return Ok(None) };
        let mut out = imports_rebuilt(body, &wanted).unwrap_or_else(|| body.to_string());
        if let Some(section) = text::get_section(&out, "结构")
            && section.contains("（共用）")
        {
            let kept = section.lines().filter(|l| !l.contains("（共用）")).collect::<Vec<_>>().join("\n");
            out = text::set_section(&out, "结构", &kept);
        }
        Ok((out != body).then_some(out))
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
        let level = s(&self.m(model)["level"]);
        let kind = if ps::eq(&level, "concept") {
            "概念模型"
        } else if ps::eq(&level, "empirical") {
            "实证模型"
        } else {
            "形式模型"
        };
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
        let plan = self.plan(model).unwrap_or(Value::Null);
        if super::empirical::is_empirical(&plan) {
            let tables = self.tables_of(model);
            if truthy(&part["specification"]) {
                let spec = self.resolve(&part["specification"], model)?;
                if let Some(kept) = super::empirical::kept_lines(part, tables.as_ref(), &spec) {
                    line(&mut b, "- 脚本按原书表格写好的数字行（原样放在 **估计** 一节，一个字符也不改）：");
                    line(&mut b, &kept);
                }
            }
            for (field, label) in [("magnitude", "量级（架构师的设计）"), ("meaning", "为什么（架构师的设计）")] {
                if truthy(&part[field]) {
                    line(&mut b, &format!("- {label}：{}", s(&part[field])));
                }
            }
            if truthy(&part["change"]) {
                line(&mut b, &format!("- 相对基准的改动（{}）：{}", s(&part["change"]["kind"]), s(&part["change"]["text"])));
            }
        }
        if truthy(&part["spec"]) {
            line(&mut b, &format!("- 要陈述的内容：{}", s(&part["spec"])));
        }
        if derives(part) {
            let steps = if truthy(&part["steps"]) { format!("，约 {} 步", s(&part["steps"])) } else { String::new() };
            line(&mut b, &format!("- 推导：从起点写到结果 {}{steps}。骨架里已写好起点和结果。", text::inline(&s(&part["to"]))));
            line(&mut b, "- 解释：写思路、步骤下的说明和解读（见“讲清推导”）。思路的位置行原样保留。");
            // A chain member starts from the result of the part before it (main line or Side `after`).
            let before = if truthy(&part["after"]) {
                self.plan_part(model, &s(&part["after"]))?
            } else {
                let main = strings(&Value::Array(list(&self.m(model)["main"]).into_iter().cloned().collect()));
                let k = ps::index_of(&main, &s(&part["id"]));
                if k > 0 { self.plan_part(model, &main[(k - 1) as usize])? } else { None }
            };
            if let Some(before) = before.filter(|b| derives(b) && truthy(&b["to"])) {
                line(&mut b, &format!("- 起点：上一步 [[{}]] 的结果 {}", self.resolve(&before["id"], model)?, text::inline(&s(&before["to"]))));
            }
            if truthy(&part["from"]) {
                line(&mut b, &format!("- 从 {} 出发。", text::inline(&s(&part["from"]))));
            }
        }
        let formal_model = super::formal::is_formal_model(&plan, truthy(&self.m(model)["math_unit"]));
        if formal_model {
            if truthy(&part["source_kind"]) || truthy(&part["logical_force"]) {
                line(
                    &mut b,
                    &format!(
                        "- 结论属性：source_kind={}，source_label={}，logical_force={}（严格按原文力度陈述，不得升级）。",
                        s(&part["source_kind"]),
                        if truthy(&part["source_label"]) { s(&part["source_label"]) } else { "未编号".into() },
                        s(&part["logical_force"])
                    ),
                );
            }
            if truthy(&part["inventory_ref"]) {
                line(&mut b, &format!("- 源清单引用：{}", s(&part["inventory_ref"])));
            }
        }
        let premises = self.premises(part, model, bodies)?;
        if !premises.is_empty() {
            line(&mut b, "- 前提：");
            line(&mut b, &premises);
        }
        if truthy(&part["of"]) && (!formal_model || truthy(&part["kind"])) {
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
        let mut resumed: Vec<Value> = Vec::new();
        let models = self.model_list();
        begin_batch_order(&mut self.state, &models);
        for model in models {
            let st = self.model_of(&json!(model))?;
            // An unwritten placeholder keeps a model unbuilt even when it has many parts (R235).
            // The run report's own test: a skeleton section or title (`**陈述**：待写。`) is unwritten too.
            let todo = unwritten(&st["head"]) || pipe(&st["parts"]).into_iter().any(unwritten);
            let built = built_as_is(&model, &st, todo);
            let same_level = self.p.level.is_empty() || (!st["level"].is_null() && ps::eq(&s(&st["level"]), &self.p.level));
            if built && !self.p.rebuild && same_level && !failed_build_elsewhere(&self.p.work, &model) {
                skipped.push(model);
                continue;
            }
            // An accepted build whose only gap is unwritten parts is resumed, never re-planned (R235): the first
            // missing part's own round (fill or derive) goes out with the briefs, and the batch's close rounds and
            // report then run over the model like any other. rotunno2026industrial/4|a (2026-11-13): part O stayed
            // 待写 after the build, and a reopened model would otherwise be briefed, planned and filled again.
            let accepted = ps::is_in(&s(&self.m(&model)["status"]), &["built", "derived", "reviewed"])
                && self.m(&model)["ids"].as_object().is_some_and(|ids| !ids.is_empty())
                && ps::exists(&ps::join(&self.dir(&model), "plan.json"));
            if todo && accepted && !self.p.rebuild && same_level {
                if !ps::is_in(&model, &self.order()) {
                    self.state["order"] = plus(&self.state["order"], [json!(model)]);
                }
                let report = self.resume_model(&model, &mut jobs)?;
                self.m_mut(&model)["resumed"] = json!(true);
                resumed.push(json!({ "model": model, "round": report["round"], "missing": list(&report["missing"]).len() }));
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
            let staging = self.p.staging.clone();
            let summary = match context::build(self.zt()?, &staging, &model, &context) {
                Ok(summary) => summary,
                Err(_) => bail!("model-context.ps1 failed for {model}"),
            };
            // An empirical model reads the source's tables by script (docs/empirical-models-proposal.md
            // §8.2 step 1): every table of its group's section files, with its file and line.
            let empirical = if self.p.level.is_empty() { ps::eq(&s(&st["level"]), "empirical") } else { ps::eq(&self.p.level, "empirical") };
            let concept_brief = if self.p.level.is_empty() { ps::eq(&s(&st["level"]), "concept") && !st["level"].is_null() } else { ps::eq(&self.p.level, "concept") };
            let tables_file = ps::join(&dir, "tables.json");
            let inventory_file = ps::join(&dir, "inventory.json");
            let mut has_inventory = false;
            if empirical {
                // The group's own sections first (a label that repeats in another chapter resolves to the
                // model's own table), then every other section of the book: a paper's empirical section
                // often spans several groups (hansen2021demand: Tables 2-5 in s12-s15).
                let mut names: Vec<String> = pipe(&summary["sections"]).into_iter().map(s).collect();
                let mut rest: Vec<String> = std::fs::read_dir(ps::join(&staging, "sections"))
                    .map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect())
                    .unwrap_or_default();
                rest.retain(|n| n.ends_with(".md") && n != "index.md" && !names.contains(n));
                rest.sort();
                names.extend(rest);
                let mut found = Vec::new();
                for name in &names {
                    let path = ps::join(&staging, &format!("sections\\{name}"));
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        found.extend(super::tables::extract(&text, &format!("sections/{name}")));
                    }
                }
                // Last, the tables the digest did not stage (a working paper sets them after its
                // reference list, autor2004will); a label a section already has keeps the section's.
                let back = crate::prepare::BACK_MATTER;
                if let Ok(text) = std::fs::read_to_string(ps::join(&staging, back)) {
                    let known: Vec<String> = found.iter().map(|t| s(&t["label"])).collect();
                    found.extend(super::tables::extract(&text, back).into_iter().filter(|t| !known.contains(&s(&t["label"]))));
                }
                util::write_text(&tables_file, &serde_json::to_string_pretty(&json!({ "model": model, "tables": found }))?)?;
            } else if !concept_brief && !is_math_unit_structure(&st) && !staging.is_empty() {
                // Formal model staged source inventory (docs/formal-models-proposal.md §8.3):
                // deterministic extraction of display equations, \tag{n}, named statements, spans & hashes.
                let mut names: Vec<String> = pipe(&summary["sections"]).into_iter().map(s).collect();
                let mut rest: Vec<String> = std::fs::read_dir(ps::join(&staging, "sections"))
                    .map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect())
                    .unwrap_or_default();
                rest.retain(|n| n.ends_with(".md") && n != "index.md" && !names.contains(n));
                rest.sort();
                names.extend(rest);
                let mut sec_texts: Vec<(String, String)> = Vec::new();
                for name in &names {
                    let path = ps::join(&staging, &format!("sections\\{name}"));
                    if let Ok(text) = std::fs::read_to_string(&path) {
                        sec_texts.push((format!("sections/{name}"), text));
                    }
                }
                let back = crate::prepare::BACK_MATTER;
                if let Ok(text) = std::fs::read_to_string(ps::join(&staging, back)) {
                    sec_texts.push((back.to_string(), text));
                }
                if !sec_texts.is_empty() {
                    let manifest = util::read_json(&ps::join(&staging, "manifest.json")).ok();
                    let work_id = manifest.as_ref().and_then(|m| m.get("work_id"));
                    let file_id = manifest.as_ref().and_then(|m| m.get("file_id"));
                    let src_sha = manifest.as_ref().and_then(|m| m["source_sha256"].as_str());
                    let inv = super::inventory::build_inventory(&model, work_id, file_id, src_sha, &sec_texts);
                    util::write_text(&inventory_file, &serde_json::to_string_pretty(&inv)?)?;
                    has_inventory = true;
                }
            }
            let context_hash = Self::file_hash(&context)?;
            let read_only = ps::join(&dir, "read-only.allow.txt");
            util::write_text(&read_only, "")?;
            let brief = ps::join(&dir, "brief.md");
            let job = ps::join(&dir, "brief-job.md");
            let level = if truthy(&st["level"]) { s(&st["level"]) } else { "未定".into() };
            let asked = if self.p.level.is_empty() { String::new() } else { format!("\n本次指定层级: {}", self.p.level) };
            let extra_file = if empirical {
                format!("表格文件: {tables_file}\n")
            } else if has_inventory {
                format!("清单文件: {inventory_file}\n")
            } else {
                String::new()
            };
            util::write_text(
                &job,
                &format!(
                    "# 简报任务：[[{model}]]\n\nHELPER: {}（只读：每条命令末尾加 -Allow \"{read_only}\"）\n上下文包: {context}\n简报文件: {brief}\n现在的层级: {level}{asked}\n分卡数: {}\n{extra_file}",
                    self.helper,
                    list(&st["parts"]).len(),
                ),
            )?;
            jobs.push(json!({
                "id": format!("{}:brief", Self::slug(&model)),
                "files": [if empirical { self.docs.brief_empirical.clone() } else { self.docs.brief.clone() }, job],
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
        self.out_last(&json!({ "jobs": jobs, "skipped": skipped, "resumed": resumed }))
    }

    fn step_validate(&mut self) -> Result<()> {
        let mut bom_faults = Vec::new();
        if has_utf8_bom(&self.p.plan) {
            bom_faults.push("plan.json has a UTF-8 BOM; write UTF-8 without BOM".to_string());
        }
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
        let mut faults = bom_faults;
        faults.extend(self.record_source_faults(&plan, &path));
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
        let normalized = match crate::faults::entries_in(&folder, &identity, &entries, origin) {
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

    fn handle_apply_result(&mut self, model: &str, res: Result<Vec<Value>>) -> Result<Vec<Value>> {
        match res {
            Ok(built) => Ok(built),
            Err(error) => {
                if crate::lockcheck::stopped_refusal(&error).is_some() || is_transport_fault(&error) {
                    return Err(error);
                }
                let m = self.m_mut(model);
                m["status"] = json!("failed");
                m["error"] = json!(format!("{error:#}"));
                Ok(Vec::new())
            }
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
            let res = self.build_model(&model);
            jobs.extend(self.handle_apply_result(&model, res)?);
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
        if has_utf8_bom(&plan_path) {
            let m = self.m_mut(model);
            m["status"] = json!("failed");
            m["error"] = json!("plan faults: plan.json has a UTF-8 BOM; write UTF-8 without BOM");
            return Ok(Vec::new());
        }
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
        // An empirical part rests on what its role names (an estimate on its specification, a variant
        // on its base, a test on its assumption): the compiler writes those Links itself.
        let empirical = super::empirical::is_empirical(&plan);
        if empirical {
            for p in parts.iter_mut() {
                let mut rests: Vec<Value> = list(&p["rests"]).into_iter().cloned().collect();
                for (id, why) in super::empirical::role_rests(p) {
                    if !rests.iter().any(|r| ps::eq(&s(&ps::at(&pipe(r), 0)), &id)) {
                        rests.push(json!([id, why]));
                    }
                }
                p["rests"] = Value::Array(rests);
            }
        }
        let is_math_unit = is_math_unit_structure(&st);
        let formal_model = super::formal::is_formal_model(&plan, is_math_unit);
        let mut known: CiMap<Value> = CiMap::new();
        for p in list(&st["parts"]) {
            known.insert(&s(&p["address"]), p.clone());
        }
        // Preserve already-allocated live archive addresses on retry so an interrupted build does
        // not orphan Cards and allocate duplicates (Astra H8).
        let prev_ids = self.m(model)["ids"].as_object().cloned().unwrap_or_default();
        let live_addrs = self.tree_addresses(model).unwrap_or_default();
        let own_prefix = format!("{}|", model.to_lowercase());
        let mut reused_new: HashSet<String> = HashSet::new();
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
            } else if let Some(prev_addr) = prev_ids.get(&id).map(s).filter(|a| !a.is_empty())
                && prev_addr.to_lowercase().starts_with(&own_prefix)
                && live_addrs.contains(&prev_addr)
            {
                ids.insert(id.clone(), json!(prev_addr));
                reused_new.insert(id.clone());
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
        let defer_formal_level = formal_model && !list(&st["parts"]).is_empty();
        let mut edit = json!({ "at": model });
        if !defer_formal_level {
            edit["level"] = plan["level"].clone();
        }
        if truthy(&plan["title"]) {
            edit["title"] = plan["title"].clone();
        } else if let Some(title) = cleared_title(&s(&self.card(&json!(model))?["title"])) {
            // A unit skeleton's title (`4.1 幂法则：待写`) never survives a build the architect left untitled.
            edit["title"] = json!(title);
        }
        if edit.as_object().is_some_and(|o| o.len() > 1) {
            self.edits(vec![json!({ "tool": "edit_card", "arguments": edit })])?;
        }

        // Main line, in build order. A new part goes before the next kept part, or after the last one.
        let main: Vec<usize> = (0..parts.len()).filter(|&i| ps::eq(&line_of(&parts[i]), "main")).collect();
        let mut previous: Option<usize> = None;
        for k in 0..main.len() {
            let p = parts[main[k]].clone();
            let id = s(&p["id"]);
            if truthy(&p["existing"]) || reused_new.contains(&id) {
                previous = Some(main[k]);
                continue;
            }
            let anchor = main[k + 1..].iter().find(|&&i| truthy(&parts[i]["existing"])).map(|&i| s(&parts[i]["id"]));
            let anchor_address = anchor.map(|a| self.m(model)["ids"][&a].clone()).unwrap_or(Value::Null);
            let init_body = if formal_model {
                super::formal::initial_skeleton(&p, &plan)
            } else {
                "待写".to_string()
            };
            let mut fields = Map::new();
            fields.insert("title".into(), p["title"].clone());
            fields.insert("facet".into(), p["facet"].clone());
            fields.insert("type".into(), json!("model"));
            fields.insert("body".into(), json!(init_body));
            if truthy(&p["emphasis"]) {
                fields.insert("emphasis".into(), p["emphasis"].clone());
            } else if derives(&p) {
                fields.insert("emphasis".into(), json!("aside"));
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
                self.save_state()?;
            } else if let Some(prev) = previous {
                let mut args = fields.clone();
                args.insert("at".into(), self.m(model)["ids"][&s(&parts[prev]["id"])].clone());
                args.insert("kind".into(), json!("direct"));
                let address = self.write(vec![json!({ "tool": "create_successor", "arguments": args })])?.into_iter().next();
                self.m_mut(model)["ids"][&id] = json!(address);
                self.save_state()?;
            } else {
                let mut args = fields.clone();
                args.insert("at".into(), json!(model));
                args.insert("kind".into(), json!("side"));
                let address = self.write(vec![json!({ "tool": "create_successor", "arguments": args })])?.into_iter().next();
                self.m_mut(model)["ids"][&id] = json!(address);
                self.save_state()?;
            }
            previous = Some(main[k]);
        }
        // Side parts beside their step (topologically ordered so `of` and `after` parents exist
        // before children; §8.5 Invariants 1 & 2), then off parts beside the Model Card.
        let side_order = super::formal::side_placement_order(&parts)
            .map_err(|stuck| anyhow::anyhow!("placement cycle in side parts: {}", stuck.join(", ")))?;
        for i in side_order {
            let p = parts[i].clone();
            let id = s(&p["id"]);
            if truthy(&p["existing"]) || reused_new.contains(&id) {
                continue;
            }
            let init_body = if formal_model {
                super::formal::initial_skeleton(&p, &plan)
            } else {
                "待写".to_string()
            };
            if formal_model && truthy(&p["after"]) {
                let prev_addr = self.resolve(&p["after"], model)?;
                if truthy(&p["move"]) {
                    let to = Self::direct_slot(&prev_addr)?;
                    let at = self.m(model)["ids"][&id].clone();
                    self.write(vec![json!({ "tool": "move_card", "arguments": { "at": at, "to": to, "confirm": "move" } })])?;
                    let moved = format!("{} -> {}", s(&p["move"]), self.id_address(model, &id));
                    let m = self.m_mut(model);
                    m["moved"] = plus(&m["moved"], [json!(moved)]);
                    continue;
                }
                let mut fields = Map::new();
                fields.insert("title".into(), p["title"].clone());
                fields.insert("facet".into(), p["facet"].clone());
                fields.insert("type".into(), json!("model"));
                fields.insert("body".into(), json!(init_body));
                if truthy(&p["emphasis"]) {
                    fields.insert("emphasis".into(), p["emphasis"].clone());
                } else if derives(&p) {
                    fields.insert("emphasis".into(), json!("aside"));
                }
                let slot = Self::direct_slot(&prev_addr)?;
                let set = self.tree_addresses(model)?;
                let address = if set.contains(&slot) {
                    fields.insert("before".into(), json!(slot));
                    fields.insert("confirm".into(), json!("insert"));
                    self.write(vec![json!({ "tool": "insert_card", "arguments": fields })])?.into_iter().next()
                } else {
                    fields.insert("at".into(), json!(prev_addr));
                    fields.insert("kind".into(), json!("direct"));
                    self.write(vec![json!({ "tool": "create_successor", "arguments": fields })])?.into_iter().next()
                };
                self.m_mut(model)["ids"][&id] = json!(address);
                self.save_state()?;
                continue;
            }
            let parent = self.resolve(&p["of"], model)?;
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
            fields.insert("body".into(), json!(init_body));
            if truthy(&p["emphasis"]) {
                fields.insert("emphasis".into(), p["emphasis"].clone());
            } else if derives(&p) {
                fields.insert("emphasis".into(), json!("aside"));
            }
            let address = self.write(vec![json!({ "tool": "create_successor", "arguments": fields })])?.into_iter().next();
            self.m_mut(model)["ids"][&id] = json!(address);
            self.save_state()?;
        }
        // Off parts beside the Model Card.
        for i in 0..parts.len() {
            let p = parts[i].clone();
            if !ps::eq(&line_of(&p), "off") {
                continue;
            }
            let id = s(&p["id"]);
            if truthy(&p["existing"]) || reused_new.contains(&id) {
                continue;
            }
            if truthy(&p["move"]) {
                let set = self.tree_addresses(model)?;
                let to = Self::free_side(model, &set)?;
                let at = self.m(model)["ids"][&id].clone();
                self.write(vec![json!({ "tool": "move_card", "arguments": { "at": at, "to": to, "confirm": "move" } })])?;
                let moved = format!("{} -> {}", s(&p["move"]), self.id_address(model, &id));
                let m = self.m_mut(model);
                m["moved"] = plus(&m["moved"], [json!(moved)]);
                continue;
            }
            let init_body = if formal_model {
                super::formal::initial_skeleton(&p, &plan)
            } else {
                "待写".to_string()
            };
            let mut fields = Map::new();
            fields.insert("at".into(), json!(model));
            fields.insert("kind".into(), json!("side"));
            fields.insert("title".into(), p["title"].clone());
            fields.insert("facet".into(), p["facet"].clone());
            fields.insert("type".into(), json!("model"));
            fields.insert("body".into(), json!(init_body));
            if truthy(&p["emphasis"]) {
                fields.insert("emphasis".into(), p["emphasis"].clone());
            }
            let address = self.write(vec![json!({ "tool": "create_successor", "arguments": fields })])?.into_iter().next();
            self.m_mut(model)["ids"][&id] = json!(address);
            self.save_state()?;
        }
        // Merged parts: only a part without successors can go (a delete takes its successors with it).
        for mg in list(&plan["merge"]) {
            // The plan names the Card where it stood when planned; an insert before it renumbered it.
            let from = current_address(&self.renamed, &s(&mg["from"]));
            let set = self.tree_addresses(model)?;
            if Self::has_successors(&from, &set)? {
                let note = format!("not merged: {from} has successors");
                let m = self.m_mut(model);
                m["notes"] = plus(&m["notes"], [json!(note)]);
                continue;
            }
            let into = self.resolve(&mg["into"], model)?;
            self.write(vec![json!({ "tool": "delete_card", "arguments": { "at": from, "into": into, "confirm": "delete" } })])?;
            // The run ledger follows the merge, so its receipts never name a Card that is gone
            // (becker2025division-appendix, 2026-11-10: `24|a|18` merged into `24|a|b` stopped the Link preparation).
            if !self.p.staging.is_empty() {
                let moves: std::collections::BTreeMap<String, String> = [(from.clone(), into.clone())].into();
                let staging = self.p.staging.clone();
                if let Err(error) = crate::ledger::record_moves(&staging, self.zt()?, &moves) {
                    let note = format!("merge {from} into {into} not recorded in the run ledger: {error}");
                    let m = self.m_mut(model);
                    m["notes"] = plus(&m["notes"], [json!(note)]);
                }
            }
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
            let step_prev: Option<Value> = if formal_model && truthy(&p["after"]) {
                let aid = s(&p["after"]);
                parts.iter().find(|cand| ps::eq(&s(&cand["id"]), &aid)).cloned()
            } else {
                prev.clone()
            };
            if truthy(&p["existing"]) || truthy(&p["move"]) {
                let mut change = Map::new();
                change.insert("at".into(), json!(address));
                for f in ["title", "facet", "emphasis"] {
                    if truthy(&p[f]) {
                        change.insert(f.into(), p[f].clone());
                    }
                }
                if truthy(&p["refill"]) {
                    change.insert("body".into(), json!(self.skeleton(p, &plan, model, step_prev.as_ref(), is_math_unit)?));
                }
                if change.len() > 1 {
                    edits.push(json!({ "tool": "edit_card", "arguments": change }));
                }
            } else {
                edits.push(json!({ "tool": "edit_card", "arguments": { "at": address, "body": self.skeleton(p, &plan, model, step_prev.as_ref(), is_math_unit)? } }));
            }
            if ps::eq(&line_of(p), "main") {
                prev = Some(p.clone());
            }
        }
        let card = self.card(&json!(model))?;
        let structure = self.structure_list(&plan, model)?;
        let mut body = text::set_section(&s(&card["body"]), "结构", &structure);
        if empirical {
            let tables = self.tables_of(model);
            let resolve = |id: &str| self.resolve(&json!(id), model);
            for (name, content) in super::empirical::head_sections(&plan, &resolve, tables.as_ref())? {
                body = text::set_section(&body, &name, &format!("**{name}**\n{content}"));
            }
        } else if formal_model {
            let has_problem = zt_check::formal::section(&body, "问题").is_some()
                || body.lines().any(|l| {
                    let t = l.trim_start();
                    t.starts_with("问题：") || t.starts_with("问题:")
                });
            if !has_problem {
                let prompt_q = if truthy(&plan["card"]) {
                    s(&plan["card"])
                } else {
                    "待写（一句话说明本模型要回答的核心经济问题）。".to_string()
                };
                body = format!("**问题**\n{prompt_q}\n\n{body}");
            }
            let resolve = |id: &str| self.resolve(&json!(id), model);
            for (name, content) in super::formal::head_sections(&plan, &resolve)? {
                body = text::set_section(&body, &name, &format!("**{name}**\n{content}"));
            }
        }
        if let Some(with) = self.with_imports(&body, &plan, model)? {
            body = with;
        }
        if let Some(divisions) = divisions_section(&plan, |id| self.resolve(&json!(id), model))? {
            body = text::set_section(&body, "分部", &divisions);
        }
        let mut head_edit = json!({ "at": model, "body": body });
        if defer_formal_level {
            head_edit["level"] = plan["level"].clone();
        }
        edits.push(json!({ "tool": "edit_card", "arguments": head_edit }));
        self.edits(edits)?;
        {
            let main_ids: Vec<Value> = main.iter().map(|&i| parts[i]["id"].clone()).collect();
            let derive_ids: Vec<Value> = parts.iter().filter(|p| derives(p)).map(|p| p["id"].clone()).collect();
            let m = self.m_mut(model);
            m["main"] = Value::Array(main_ids);
            m["derive"] = Value::Array(derive_ids);
            m["math_unit"] = json!(is_math_unit);
            m["status"] = json!("built");
        }

        // setting.md opens every DeepSeek job of this model, after the shared instructions.
        let formal = ps::eq(&s(&plan["level"]), "formal");
        let setting = main.first().map(|&i| parts[i].clone()).unwrap_or(Value::Null);
        let setting_text = if empirical {
            let tables = self.tables_of(model);
            let resolve = |id: &str| self.resolve(&json!(id), model);
            super::empirical::setting_text(&plan, model, &resolve, tables.as_ref())?
        } else if formal_model {
            let inv = self.inventory_of(model);
            let resolve = |id: &str| self.resolve(&json!(id), model);
            super::formal::setting_text(&plan, model, &resolve, inv.as_ref())?
        } else if formal {
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
                !truthy(&p["take"])
                    && !derives(p)
                    && (!(truthy(&p["existing"]) || truthy(&p["move"])) || truthy(&p["refill"]))
                    && !(empirical && super::empirical::role(p) == Some(zt_check::empirical::EmpiricalRole::Variables))
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
            let main_fallback: Vec<Value> = list(&self.m(&model)["main"]).into_iter().cloned().collect();
            // Branch-aware Derivation chains in build order (including split pieces via effective_plan_parts, Astra #7).
            let mut chains: Vec<Vec<Value>> = Vec::new();
            if let Ok(plan) = self.plan(&model) {
                let mut eff = plan.clone();
                eff["parts"] = Value::Array(self.effective_plan_parts(&model, &plan));
                for chain_ids in Self::plan_derivation_chains(&eff) {
                    let mut parts_in_chain = Vec::new();
                    for id in chain_ids {
                        if let Some(p) = self.plan_part(&model, &id)? {
                            parts_in_chain.push(p);
                        }
                    }
                    if !parts_in_chain.is_empty() {
                        chains.push(parts_in_chain);
                    }
                }
            } else {
                let mut ids: Vec<Value> = main_fallback;
                ids.push(json!(""));
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
            let is_formal_model_hint = ps::eq(&s(&self.m(&model)["level"]), "formal")
                && !truthy(&self.m(&model)["math_unit"]);
            // A model resumed for its unwritten parts kept its review: only a resumed derive round (status
            // `derived`) or a resumed 0-derivation Level 2 formal model not yet reviewed (status != `reviewed`
            // and no `steps.review`) is reviewed again (Astra H13).
            let needs_first_formal_review = is_formal_model_hint
                && !ps::eq(&s(&self.m(&model)["status"]), "reviewed")
                && !self.m(&model)["steps"]["review"].is_object();
            if truthy(&self.m(&model)["resumed"])
                && !ps::eq(&s(&self.m(&model)["status"]), "derived")
                && !needs_first_formal_review
            {
                continue;
            }
            // Level 3: the evidence review, once, one job per twelve cards side by side.
            if ps::eq(&s(&self.m(&model)["level"]), "empirical") && ps::is_in(&s(&self.m(&model)["status"]), &["built", "derived"]) && !self.p.changed {
                let before = jobs.len();
                jobs.extend(self.evidence_review_jobs(&model)?);
                self.m_mut(&model)["status"] = json!("reviewed");
                self.record_step(&model, "review", json!({"jobs": jobs.len() - before, "evidence": true}));
                if list(&self.m(&model)["derive"]).is_empty() {
                    continue;
                }
            }
            let m = self.m(&model);
            let review_ready = if is_formal_model_hint {
                ps::is_in(&s(&m["status"]), &["built", "derived", "reviewed"])
            } else {
                ps::is_in(&s(&m["status"]), &["derived", "reviewed"]) && !list(&m["derive"]).is_empty()
            };
            if !review_ready {
                continue;
            }
            let st = self.model_of(&json!(model))?;
            let is_formal_model = ps::eq(&s(&st["level"]), "formal") && !is_math_unit_structure(&st);
            let mut bodies: CiMap<Value> = CiMap::new();
            for p in list(&st["parts"]) {
                bodies.insert(&s(&p["address"]), p.clone());
            }
            let mut touched = Vec::new();
            for r in list(&self.state["rereview"]).into_iter().cloned().collect::<Vec<_>>() {
                touched.push(self.resolve(&r, &model)?);
            }
            // Only the model's own parts: a shared part belongs to its home model.
            let mut own = Vec::new();
            let mut src_by_addr: CiMap<Vec<String>> = CiMap::new();
            for id in ps::keys(&self.m(&model)["ids"]) {
                let p = self.plan_part(&model, &id)?;
                if !p.as_ref().is_some_and(|p| truthy(&p["take"])) {
                    let addr = self.id_address(&model, &id);
                    own.push(addr.clone());
                    if let Some(p) = p {
                        let spans: Vec<String> = pipe(&p["src"]).into_iter().map(s).filter(|x| !x.is_empty()).collect();
                        if !spans.is_empty() {
                            src_by_addr.insert(&addr, spans);
                        }
                    }
                }
            }
            // A round that continues a conversation re-reads the Cards from the archive (its memory may
            // hold an older body); a new conversation gets the bodies in its job file.
            let continuing = !independent || self.p.changed;
            let mut chains: Vec<(Vec<String>, String, String)> = Vec::new();
            for c in proof_chains(&st["build"]) {
                let parts: Vec<String> = pipe(&c["parts"]).into_iter().map(s).filter(|a| ps::is_in(a, &own)).collect();
                if parts.is_empty() {
                    continue;
                }
                if self.p.changed && !parts.iter().any(|a| ps::is_in(a, &touched)) {
                    continue;
                }
                chains.push((parts, s(&c["after"]), s(&c["before"])));
            }
            // Astra #3: for Level 2 formal models, explicitly assign every owned `结果` and `论证` card (so
            // Spence-style argument-only models and Result clauses receive their own verdicts too).
            let mut extra_claims: Vec<String> = Vec::new();
            if is_formal_model {
                let chain_set: BTreeSet<String> = chains.iter().flat_map(|(p, _, _)| p.iter().cloned()).collect();
                for o in list(&st["build"]["order"]) {
                    let addr = s(&o["address"]);
                    if !ps::is_in(&addr, &own) || chain_set.contains(&addr) {
                        continue;
                    }
                    let facet = s(&o["facet"]);
                    if matches!(
                        zt_check::formal::role_of(&facet),
                        Some(zt_check::formal::FormalRole::Result | zt_check::formal::FormalRole::Argument)
                    ) {
                        if self.p.changed && !ps::is_in(&addr, &touched) {
                            continue;
                        }
                        if !extra_claims.contains(&addr) {
                            extra_claims.push(addr);
                        }
                    }
                }
            }
            let before = jobs.len();
            // Collect all review units and chunk into <= 12 cards per job (proposal §8.6 / Astra #3).
            let mut all_targets: Vec<String> = Vec::new();
            for (parts, _, _) in &chains {
                for a in parts {
                    if !all_targets.contains(a) {
                        all_targets.push(a.clone());
                    }
                }
            }
            for a in &extra_claims {
                if !all_targets.contains(a) {
                    all_targets.push(a.clone());
                }
            }
            let chunk_size = if is_formal_model { 12 } else { all_targets.len().max(1) };
            for chunk in all_targets.chunks(chunk_size) {
                if chunk.is_empty() {
                    continue;
                }
                let chunk_vec: Vec<String> = chunk.to_vec();
                // Trim each chain strictly to this chunk's cards so continuing packets stay <= 12 cards (Astra M2).
                let chunk_chains: Vec<(Vec<String>, String, String)> = chains
                    .iter()
                    .filter_map(|(p, after, before)| {
                        let pts: Vec<String> = p.iter().filter(|a| chunk_vec.contains(a)).cloned().collect();
                        (!pts.is_empty()).then(|| (pts, after.clone(), before.clone()))
                    })
                    .collect();
                let seq = self.next_seq();
                let dir = self.dir(&model);
                let out = ps::join(&dir, &format!("review-{seq}.json"));
                let id = if independent {
                    if self.p.changed { format!("{}:review+{seq}", Self::slug(&model)) } else if jobs.len() == before { format!("{}:review", Self::slug(&model)) } else { format!("{}:review+{seq}", Self::slug(&model)) }
                } else {
                    format!("{}:r{seq}", Self::slug(&model))
                };
                let mut text = format!(
                    "# 复核任务 {id}\n\n结果文件: {out}\nMaxSteps: {}\n模型卡: [[{model}]]\n\r\n",
                    self.part_steps()
                );
                if is_formal_model {
                    text += "## 条款核对清单（形式模型必检，见 formal-models-proposal.md §8.6）\r\n\r\n";
                    text += "1. **条件完整性**：核对每个假设、参数限制、体制限定与关键词（如 separately、仅针对观测到的信号、实际发生收入）是否完整保留在结果卡 **陈述** 中，无遗漏条件。\r\n";
                    text += "2. **逻辑力度忠实度**：必要条件不得升级为充分或充要条件；存在性不得升级为唯一性；局部线性化结论不得升级为全局结论；猜想不得升级为定理；数值结果不得表述为解析定理。\r\n";
                    text += "3. **依据诚实度**：外部引证（如 [4]、在线补充材料）或启发式推导不得伪装为完整局部证明；启发式推导额外引入的强假设必须明示。\r\n\r\n";
                }
                if continuing {
                    text += CR;
                    if !chunk_chains.is_empty() {
                        text += &self.review_reread_text(&chunk_chains)?;
                    }
                    let claim_addrs: Vec<&String> = chunk_vec.iter().filter(|a| extra_claims.contains(a)).collect();
                    if !claim_addrs.is_empty() {
                        text += "\r\n## 要复核的结果与论证分卡\r\n\r\n";
                        for a in claim_addrs {
                            let src_note = src_by_addr.get(a).map(|v| format!("（原书：{}）", v.join(", "))).unwrap_or_default();
                            text += &format!("- [[{a}]]{src_note}\r\n");
                        }
                    }
                } else {
                    text += CR;
                    for (_, after, before) in &chunk_chains {
                        for (label, at) in [("链之前的分卡（推导从它出发）", after), ("链之后的分卡（它用推导的结果）", before)] {
                            if !at.is_empty()
                                && let Some(b) = bodies.get(at).filter(|b| truthy(b))
                            {
                                text += &format!("## {label}：[[{}]] {}\n\n{}\n\r\n", at, s(&b["title"]), s(&b["body"]));
                            }
                        }
                    }
                    for a in &chunk_vec {
                        let b = bodies.get(a).cloned().unwrap_or(Value::Null);
                        let src_note = src_by_addr.get(a).map(|v| format!("\n来源定位：{}\n", v.join(", "))).unwrap_or_default();
                        text += &format!("## 推导分卡 [[{a}]] {}{src_note}\n{}\n\r\n", s(&b["title"]), s(&b["body"]));
                    }
                }
                let assigned_addrs = chunk_vec;
                let assigned_ids: Vec<String> = assigned_addrs
                    .iter()
                    .filter_map(|a| {
                        ps::keys(&self.m(&model)["ids"])
                            .into_iter()
                            .find(|id| ps::eq(&s(&self.m(&model)["ids"][id]), a))
                    })
                    .collect();
                let file = ps::join(&dir, &format!("review-{seq}.md"));
                util::write_text(&file, &text)?;
                let mut addr_to_id = Map::new();
                for (a, id_val) in assigned_addrs.iter().zip(assigned_ids.iter()) {
                    addr_to_id.insert(a.to_ascii_lowercase(), json!(id_val));
                }
                let mut job = json!({
                    "id": id,
                    "review_kind": "formal",
                    "output": out,
                    "addresses": assigned_addrs,
                    "files": [self.docs.review, ps::join(&dir, "setting.md"), file],
                    "prompt": format!("按以上说明复核这些分卡（含条款核对与推导核对），把结果写进 {out}，最后只回复一行 JSON。"),
                });
                if continuing {
                    let into = if independent { format!("{}:review", Self::slug(&model)) } else { format!("{}:brief", Self::slug(&model)) };
                    job["continue"] = json!(into);
                    job["fallback"] = json!([self.docs.review, ps::join(&dir, "setting.md"), file]);
                }
                let full_out = ps::full_name(&out);
                let mut pending: Vec<Value> = list(&self.state["pending_reviews"])
                    .into_iter()
                    .filter(|x| !ps::eq(&s(&x["output"]), &full_out))
                    .cloned()
                    .collect();
                pending.push(json!({
                    "output": full_out,
                    "model": model,
                    "round": seq,
                    "addresses": assigned_addrs,
                    "ids": assigned_ids,
                    "address_to_id": Value::Object(addr_to_id),
                }));
                self.state["pending_reviews"] = Value::Array(pending);
                jobs.push(job);
            }
            let scheduled = jobs.len() - before;
            // Never mark a model `reviewed` on a 0-job round unless it is a `-Changed` no-op
            // on an already-reviewed model (Astra H7).
            if scheduled > 0 || (self.p.changed && ps::eq(&s(&self.m(&model)["status"]), "reviewed")) {
                self.m_mut(&model)["status"] = json!("reviewed");
                self.record_step(&model, "review", json!({"jobs":scheduled,"changed":self.p.changed,"rounds":1}));
            }
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
            if let Some(found) = self.find_model_owner(&model, address)? {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    /// Resolves a part address strictly within one named `model` (excluding taken shared parts).
    fn find_model_owner(&mut self, model: &str, address: &str) -> Result<Option<(String, String)>> {
        for id in ps::keys(&self.m(model)["ids"]) {
            let value = &self.m(model)["ids"][&id];
            if value.is_null() || !ps::eq(&s(value), address) {
                continue;
            }
            let p = self.plan_part(model, &id)?;
            if p.is_some_and(|p| truthy(&p["take"])) {
                continue;
            }
            return Ok(Some((model.to_string(), id)));
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

    /// `Split-Part`: inserts the new parts after the part (on either `main` or a Side proof branch,
    /// preserving `id`'s `card_id` and middle-of-branch continuations per §8.5 Invariant 4),
    /// re-points downstream premise links (next step or parent Result endpoint), updates the 结构 list,
    /// and says what the conversation must do.
    fn split_part(&mut self, model: &str, id: &str, count: i64, cuts: &[Value], reason: &Value) -> Result<Follow> {
        let orig_part = self.plan_part(model, id)?;
        let is_side = orig_part
            .as_ref()
            .is_some_and(|p| ps::eq(&line_of(p), "side"));
        let address = self.m(model)["ids"][id].clone();
        let own_addr_initial = s(&address);
        let card = self.card(&address)?;
        let derive = truthy(&card["facet"]) && ps::starts_with(&s(&card["facet"]), "推导");
        let fin = if derive { text::result_of(&s(&card["body"])) } else { String::new() };
        let main = strings(&Value::Array(list(&self.m(model)["main"]).into_iter().cloned().collect()));
        let main_index = ps::index_of(&main, id);
        let next = if main_index >= 0 && ((main_index + 1) as usize) < main.len() {
            Some(main[(main_index + 1) as usize].clone())
        } else if is_side {
            // Side-branch middle-of-branch continuation: find the part occupying `direct_slot(own_addr)`.
            let direct = Self::direct_slot(&own_addr_initial)?;
            ps::keys(&self.m(model)["ids"])
                .into_iter()
                .find(|cand| ps::eq(&s(&self.m(model)["ids"][cand]), &direct))
        } else {
            None
        };
        if !truthy(&self.m(model)["added"]) {
            self.m_mut(model)["added"] = json!({});
        }
        let cuts: Vec<Value> = cuts.iter().filter(|c| !c.is_null()).cloned().collect();
        let cut = |i: i64| if i >= 0 { cuts.get(i as usize).cloned().unwrap_or(Value::Null) } else { Value::Null };
        let mut new: Vec<String> = Vec::new();
        let mut prev_id = id.to_string();
        let base_id = id.split_once('~').map(|(b, _)| b).unwrap_or(id);
        let mut next_suffix = 2i64;
        for k in 2..=count {
            // Allocate an unused suffix `{base}~{n}` so repeated splits never overwrite an existing piece (Astra #8).
            while !self.m(model)["ids"][format!("{base_id}~{next_suffix}")].is_null() {
                next_suffix += 1;
            }
            let new_id = format!("{base_id}~{next_suffix}");
            next_suffix += 1;
            let mut facet = format!("{}（{k}/{count}）", s(&card["facet"]));
            if ps::len16(&facet) > 32 {
                facet = ps::cut16(&facet, 32);
            }
            let title = format!("{}（{k}/{count}）", s(&card["title"]));
            let is_argument = zt_check::formal::role_of(&facet) == Some(zt_check::formal::FormalRole::Argument);
            let init_split_body = if derive {
                "**思路**\n待写\n\n**起点**\n- 待写\n\n**步骤**\n待写\n\n**结果**\n待写\n\n**解读**\n待写".to_string()
            } else if is_argument {
                "目标：待写。\n\n**依据**\n- 待写\n\n**论证**\n待写\n\n**结论**\n待写".to_string()
            } else {
                super::formal::initial_skeleton(&json!({ "facet": facet }), &json!({}))
            };
            let mut fields = Map::new();
            fields.insert("title".into(), json!(title));
            fields.insert("facet".into(), json!(facet));
            fields.insert("type".into(), json!("model"));
            fields.insert("body".into(), json!(init_split_body));
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
                let inline_from = text::inline(&from);
                format!("**思路**\n待写\n\n**起点**\n- 依赖上一步 [[{prev_address}]] 的结果：{inline_from}。\n\n**步骤**\n待写\n\n**结果**\n{to}\n\n**解读**\n待写")
            } else if is_argument {
                format!("目标：待写（接着 [[{prev_address}]] 确立后续结论）。\n\n**依据**\n- 依赖上一步 [[{prev_address}]]。\n\n**论证**\n待写\n\n**结论**\n待写")
            } else {
                format!("依赖上一步 [[{prev_address}]]：本卡接着上一张卡写。\n\n待写")
            };
            let mut added_entry = json!({
                "id": new_id, "facet": facet, "title": title, "derive": derive, "to": to,
                "line": if is_side { "side" } else { "main" },
                "spec": format!("接着 [[{prev_address}]] 写。"),
            });
            if let Some(ref orig) = orig_part
                && !orig["src"].is_null()
            {
                added_entry["src"] = orig["src"].clone();
            }
            if is_side {
                added_entry["after"] = json!(prev_id);
            }
            self.m_mut(model)["added"][&new_id] = added_entry;
            // Persist the new ID and any renamed addresses immediately after archive allocation (Astra #2).
            self.save_state()?;
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
        if derive {
            let last = new.last().map(|n| self.id_address(model, n)).unwrap_or_default();
            if !last.is_empty() {
                let own_link = format!("[[{own_address}]]");
                let last_link = format!("[[{last}]]");
                let premise_cues = ["依赖", "基于", "沿用", "用到", "代入", "上一步", "起点", "依据", "由"];
                let dep_cues = ["依赖", "基于", "沿用", "用到", "由"];
                // Rewrite downstream premise references across ALL owned non-`take` cards of this
                // model (both the immediate `next` step AND any other downstream consumer in the
                // model), skipping imported `take` cards and keeping `见 [[own_address]]` route roots
                // intact (Astra H9).
                for cand_id in ps::keys(&self.m(model)["ids"]) {
                    if ps::eq(&cand_id, id) || new.iter().any(|n| ps::eq(n, &cand_id)) {
                        continue;
                    }
                    if self.plan_part(model, &cand_id)?.is_some_and(|p| truthy(&p["take"])) {
                        continue;
                    }
                    let is_next = next.as_ref().is_some_and(|n| ps::eq(n, &cand_id));
                    let cand_at = self.m(model)["ids"][&cand_id].clone();
                    if let Ok(cand_card) = self.card(&cand_at) {
                        let cues: &[&str] = if is_next { &premise_cues } else { &dep_cues };
                        let old_body = s(&cand_card["body"]);
                        let body = old_body
                            .split('\n')
                            .map(|line| {
                                if line.contains(&own_link)
                                    && !line.contains(&format!("见 {own_link}"))
                                    && cues.iter().any(|cue| line.contains(cue))
                                {
                                    line.replace(&own_link, &last_link)
                                } else {
                                    line.to_string()
                                }
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        if body != old_body {
                            edits.push(json!({
                                "tool": "edit_card",
                                "arguments": { "at": cand_at, "body": body }
                            }));
                        }
                    }
                }
            }
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
        let upto = if derive && !cuts.is_empty() { text::inline(&s(&cuts[0])) } else { "你选的中间结论".into() };
        text += &format!("- [[{own_address}]]：只写到 {upto}。{}\n", if derive { "它的解读已清空：按新的结果重写。" } else { "" });
        if derive {
            text += "- 每张推导分卡写自己的思路、说明和解读。思路的位置行由脚本写好，原样保留。\n";
        }
        for n in &new {
            let a = self.m(model)["added"][n].clone();
            let to = if truthy(&a["to"]) { text::inline(&s(&a["to"])) } else { "你选的中间结论".into() };
            text += &format!("- [[{}]]：{}，从上一张的结果写到 {to}。标题可改成本卡的结论。\n", self.id_address(model, n), s(&a["title"]));
        }
        let mut ids = vec![id.to_string()];
        ids.extend(new);
        Ok(Follow { model: model.to_string(), job, text, ids, cards: Vec::new() })
    }

    fn audit_and_collect_reviews(&mut self, strict: bool) -> Result<(Vec<String>, Vec<Value>, Vec<Action>, Vec<String>, [usize; 3])> {
        let mut audit_errors: Vec<String> = Vec::new();
        let mut orphans: Vec<Value> = Vec::new();
        let mut actions: Vec<Action> = Vec::new();
        let mut consumed: Vec<String> = Vec::new();
        let mut counts = [0usize; 3]; // ok, fix, split
        let already_done = strings(&Value::Array(list(&self.state["reviews"]).into_iter().cloned().collect()));

        struct Target {
            output: String,
            model: Option<String>,
            expected: Vec<String>,
            address_to_id: HashMap<String, String>,
        }
        let mut targets: Vec<Target> = Vec::new();
        let mut models_with_pending: HashSet<String> = HashSet::new();
        for p in list(&self.state["pending_reviews"]) {
            let output = s(&p["output"]);
            if output.is_empty() || ps::is_in(&output, &already_done) {
                continue;
            }
            let model = s(&p["model"]);
            if !model.is_empty() {
                models_with_pending.insert(model.to_ascii_lowercase());
            }
            let expected = list(&p["addresses"]).into_iter().map(s).filter(|a| !a.is_empty()).collect();
            let mut address_to_id = HashMap::new();
            if let Some(obj) = p["address_to_id"].as_object() {
                for (k, v) in obj {
                    if let Some(id_s) = v.as_str() {
                        address_to_id.insert(k.to_ascii_lowercase(), id_s.to_string());
                    }
                }
            }
            targets.push(Target {
                output,
                model: (!model.is_empty()).then_some(model),
                expected,
                address_to_id,
            });
        }
        for model in self.order() {
            // In strict mode, ignore unassigned stray `review-*.json` files if this model already tracks `pending_reviews`.
            if strict && models_with_pending.contains(&model.to_ascii_lowercase()) {
                continue;
            }
            let dir = self.dir(&model);
            for name in ps::child_files(&dir, "review-", ".json") {
                let full = ps::full_name(&ps::join(&dir, &name));
                if ps::is_in(&full, &already_done) || targets.iter().any(|t| ps::eq(&t.output, &full)) {
                    continue;
                }
                targets.push(Target {
                    output: full,
                    model: Some(model.clone()),
                    expected: Vec::new(),
                    address_to_id: HashMap::new(),
                });
            }
        }

        let mut action_keys: Vec<(String, String)> = Vec::new();
        for t in targets {
            let path = std::path::Path::new(&t.output);
            if !path.is_file() {
                if strict || !t.expected.is_empty() {
                    audit_errors.push(format!(
                        "{}: missing review file {}",
                        t.model.as_deref().unwrap_or("review"),
                        t.output
                    ));
                }
                continue;
            }
            let parsed = match util::read_json(&t.output) {
                Ok(v) => v,
                Err(e) => {
                    let msg = format!("{}: invalid JSON in {}: {e:#}", t.model.as_deref().unwrap_or("review"), t.output);
                    if strict {
                        audit_errors.push(msg);
                    } else {
                        orphans.push(json!(msg));
                    }
                    continue;
                }
            };
            let Some(items) = parsed.as_array() else {
                let msg = format!("{}: review JSON must be a list: {}", t.model.as_deref().unwrap_or("review"), t.output);
                if strict {
                    audit_errors.push(msg);
                } else {
                    orphans.push(json!(msg));
                }
                continue;
            };
            let mut file_ok = true;
            let mut seen_addrs: Vec<String> = Vec::new();
            for (idx, v) in items.iter().enumerate() {
                let Some(obj) = v.as_object() else {
                    if strict {
                        audit_errors.push(format!("{}: entry {idx} in {} is not an object", t.model.as_deref().unwrap_or("review"), t.output));
                        file_ok = false;
                    }
                    continue;
                };
                let addr = obj.get("address").and_then(Value::as_str).map(str::trim).unwrap_or("").to_string();
                let verdict_str = obj.get("verdict").and_then(Value::as_str).unwrap_or("");
                if addr.is_empty() || !matches!(verdict_str, "ok" | "fix" | "split") {
                    if strict {
                        audit_errors.push(format!(
                            "{}: malformed verdict entry {idx} in {} (address={addr:?}, verdict={verdict_str:?})",
                            t.model.as_deref().unwrap_or("review"),
                            t.output
                        ));
                        file_ok = false;
                    }
                    continue;
                }
                if seen_addrs.iter().any(|x| ps::eq(x, &addr)) {
                    if strict {
                        audit_errors.push(format!("{}: duplicate verdict for `{addr}` in {}", t.model.as_deref().unwrap_or("review"), t.output));
                        file_ok = false;
                    }
                    continue;
                }
                seen_addrs.push(addr.clone());
                if !t.expected.is_empty() && !t.expected.iter().any(|e| ps::eq(e, &addr)) {
                    if strict {
                        audit_errors.push(format!("{}: unexpected address `{addr}` in {}", t.model.as_deref().unwrap_or("review"), t.output));
                        file_ok = false;
                    } else {
                        orphans.push(json!(addr));
                    }
                    continue;
                }
                match verdict_str {
                    "ok" => counts[0] += 1,
                    "fix" | "split" => {
                        if verdict_str == "fix" {
                            counts[1] += 1;
                        } else {
                            counts[2] += 1;
                        }
                        let reason_ok = obj.get("reason").and_then(Value::as_str).is_some_and(|r| !r.trim().is_empty());
                        if strict && !reason_ok {
                            audit_errors.push(format!("{}: `{addr}` verdict `{verdict_str}` is missing a non-empty `reason` in {}", t.model.as_deref().unwrap_or("review"), t.output));
                            file_ok = false;
                            continue;
                        }
                        let mut parts_n = 0i64;
                        let mut cuts_vec: Vec<Value> = Vec::new();
                        if verdict_str == "split" {
                            let p_ok = obj.get("parts").and_then(Value::as_i64).filter(|n| *n >= 2);
                            if strict && p_ok.is_none() {
                                audit_errors.push(format!("{}: `{addr}` split requires integer `parts >= 2` in {}", t.model.as_deref().unwrap_or("review"), t.output));
                                file_ok = false;
                                continue;
                            }
                            parts_n = p_ok.unwrap_or_else(|| ps::int(&v["parts"]).unwrap_or(0));
                            let cuts_arr = obj.get("cuts").and_then(Value::as_array);
                            let expected_cuts = usize::try_from(parts_n.saturating_sub(1)).unwrap_or(0);
                            let cuts_valid = cuts_arr.is_some_and(|arr| {
                                arr.len() == expected_cuts
                                    && arr.iter().all(|c| c.as_str().is_some_and(|cs| !cs.trim().is_empty()))
                            });
                            if strict && !cuts_valid {
                                audit_errors.push(format!("{}: `{addr}` split with parts={parts_n} requires {expected_cuts} non-empty `cuts` in {}", t.model.as_deref().unwrap_or("review"), t.output));
                                file_ok = false;
                                continue;
                            }
                            if let Some(arr) = cuts_arr {
                                cuts_vec = arr.clone();
                            }
                        }
                        let owner = match &t.model {
                            Some(m) => {
                                if let Some(mapped_id) = t.address_to_id.get(&addr.to_ascii_lowercase()) {
                                    let exists = !self.m(m)["ids"][mapped_id].is_null();
                                    let is_take = self.plan_part(m, mapped_id)?.is_some_and(|p| truthy(&p["take"]));
                                    if exists && !is_take {
                                        Some((m.clone(), mapped_id.clone()))
                                    } else {
                                        None
                                    }
                                } else {
                                    self.find_model_owner(m, &addr)?
                                }
                            }
                            None => self.find_owner(&addr)?,
                        };
                        match owner {
                            Some((owner_model, id)) => {
                                if action_keys.iter().any(|(m, i)| ps::eq(m, &owner_model) && ps::eq(i, &id)) {
                                    if strict {
                                        audit_errors.push(format!("{owner_model}: conflicting verdicts for part `{id}` (`{addr}`)"));
                                        file_ok = false;
                                    }
                                    continue;
                                }
                                action_keys.push((owner_model.clone(), id.clone()));
                                actions.push(Action {
                                    model: owner_model,
                                    id,
                                    verdict: verdict_str.to_string(),
                                    parts: parts_n,
                                    cuts: cuts_vec,
                                    reason: v["reason"].clone(),
                                });
                            }
                            None => {
                                let msg = format!("{}: `{addr}` is not owned by this model/batch", t.model.as_deref().unwrap_or("review"));
                                if strict {
                                    audit_errors.push(msg);
                                    file_ok = false;
                                } else {
                                    orphans.push(json!(addr));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            for exp in &t.expected {
                if !seen_addrs.iter().any(|x| ps::eq(x, exp)) {
                    audit_errors.push(format!(
                        "{}: omitted assigned address `{exp}` in {}",
                        t.model.as_deref().unwrap_or("review"),
                        t.output
                    ));
                    file_ok = false;
                }
            }
            if file_ok {
                consumed.push(t.output);
            }
        }
        Ok((audit_errors, orphans, actions, consumed, counts))
    }

    fn consume_reviews(&mut self, consumed: &[String]) {
        if consumed.is_empty() {
            return;
        }
        let mut reviews: Vec<Value> = list(&self.state["reviews"]).into_iter().cloned().collect();
        for path in consumed {
            if !reviews.iter().any(|r| ps::eq(&s(r), path)) {
                reviews.push(json!(path));
            }
        }
        self.state["reviews"] = Value::Array(reviews);
        let remaining: Vec<Value> = list(&self.state["pending_reviews"])
            .into_iter()
            .filter(|p| !consumed.iter().any(|c| ps::eq(&s(&p["output"]), c)))
            .cloned()
            .collect();
        self.state["pending_reviews"] = Value::Array(remaining);
    }

    fn step_review_audit(&mut self) -> Result<i32> {
        let (audit_errors, _, _, consumed, counts) = self.audit_and_collect_reviews(true)?;
        let verdicts = json!({ "ok": counts[0], "fix": counts[1], "split": counts[2] });
        if !audit_errors.is_empty() {
            self.out_last(&json!({
                "ok": false,
                "step": "review-audit",
                "verdicts": verdicts,
                "needs_split": false,
                "audit_errors": audit_errors,
            }))?;
            return Ok(1);
        }
        let needs_split = counts[1] + counts[2] > 0;
        if !needs_split && !consumed.is_empty() {
            self.consume_reviews(&consumed);
            self.save_state()?;
        }
        self.out_last(&json!({
            "ok": true,
            "step": "review-audit",
            "verdicts": verdicts,
            "needs_split": needs_split,
            "audit_errors": [],
        }))?;
        Ok(0)
    }

    fn step_split(&mut self) -> Result<i32> {
        // job id (or new:<model>) -> what its conversation must do next
        let mut follow: CiMap<Follow> = CiMap::new();
        let strict = self.p.strict_audit;
        // Name every target by its model and id first: a split renumbers the addresses after it.
        let mut need_actions: Vec<Action> = Vec::new();
        let mut seen_need_keys: Vec<(String, String)> = Vec::new();
        let mut need_orphans: Vec<Value> = Vec::new();
        let mut need_errors: Vec<String> = Vec::new();
        for pair in self.p.need.split(';').filter(|p| !p.trim().is_empty()).map(str::to_string).collect::<Vec<_>>() {
            let mut it = pair.splitn(2, '=');
            let address = it.next().unwrap_or_default().to_string();
            let count = it.next().map(|c| Value::String(c.to_string())).unwrap_or(Value::Null);
            let parts_n = ps::int(&count)?;
            if self.p.strict_audit && parts_n < 2 {
                need_errors.push(format!("-Need `{address}` requires parts >= 2"));
                continue;
            }
            match self.find_owner(address.trim())? {
                Some((model, id)) => {
                    if seen_need_keys.iter().any(|(m, i)| ps::eq(m, &model) && ps::eq(i, &id)) {
                        let msg = format!("{model}: duplicate -Need target `{address}` (`{model}#{id}`)");
                        if strict {
                            need_errors.push(msg);
                        } else {
                            bail!("{msg}");
                        }
                    } else {
                        seen_need_keys.push((model.clone(), id.clone()));
                        need_actions.push(Action {
                            model,
                            id,
                            verdict: "split".into(),
                            parts: parts_n,
                            cuts: Vec::new(),
                            reason: json!("写不下"),
                        });
                    }
                }
                None => {
                    if self.p.strict_audit {
                        need_errors.push(format!("-Need address `{address}` is not owned by this batch"));
                    } else {
                        need_orphans.push(json!(address));
                    }
                }
            }
        }
        let (mut audit_errors, mut orphans, rev_actions, consumed, _) = self.audit_and_collect_reviews(strict)?;
        audit_errors.extend(need_errors);
        orphans.extend(need_orphans);
        for na in &need_actions {
            if rev_actions.iter().any(|ra| ps::eq(&ra.model, &na.model) && ps::eq(&ra.id, &na.id)) {
                audit_errors.push(format!("{}: conflicting -Need and review action for part `{}`", na.model, na.id));
            }
        }
        if strict && !audit_errors.is_empty() {
            self.out_last(&json!({
                "ok": false,
                "step": "split",
                "audit_errors": audit_errors,
                "jobs": [],
                "orphans": orphans,
            }))?;
            return Ok(1);
        }
        let mut actions = need_actions;
        actions.extend(rev_actions);

        let mut resplit: Vec<String> = Vec::new();
        for a in actions {
            if ps::eq(&a.verdict, "split") && a.parts >= 2 {
                let r = self.split_part(&a.model, &a.id, a.parts, &a.cuts, &a.reason)?;
                resplit.push(a.model.clone());
                add_follow(&mut follow, r);
            } else {
                self.state["rereview"] = plus(&self.state["rereview"], [json!(format!("{}#{}", a.model, a.id))]);
                let job = self.find_job(&a.model, &a.id);
                let at = self.resolve(&json!(a.id), &a.model)?;
                let text = format!("## 修正：[[{at}]]\n\n{}\n", s(&a.reason));
                add_follow(&mut follow, Follow { model: a.model.clone(), job, text, ids: vec![a.id.clone()], cards: Vec::new() });
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
        self.consume_reviews(&consumed);
        self.save_state()?;
        self.out_last(&json!({ "ok": true, "jobs": jobs, "orphans": orphans }))?;
        Ok(0)
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
    /// plan.json exists and passes the plan's own rules, skipped when the brief skipped it, else failed),
    /// `level`, `parts`, `derivations` (the parts with `derive: true`) and, for a plan that breaks a rule, its
    /// `faults` count and first `fault` (§3.7: a chain without the architect's `explain` entry is a fault, so
    /// the batch never builds it and asks a writer to invent the design). The model layer chooses fill-only or
    /// derive-and-review from this line, never from the architect's answer.
    fn step_plan_status(&mut self) -> Result<()> {
        let mut plans = Vec::new();
        let mut formal_plans: Vec<(String, Value)> = Vec::new();
        let folder = self.run_folder();
        let staging = (!folder.is_empty()).then_some(folder.as_str());
        for model in self.order() {
            let path = ps::join(&self.dir(&model), "plan.json");
            let status = s(&self.m(&model)["status"]);
            if ps::exists(&path) {
                let plan = util::read_json(&path).with_context(|| format!("{model}: plan.json is not JSON"))?;
                let parts = list(&plan["parts"]);
                let derivations = parts.iter().filter(|p| super::formal::is_deriving_part(p)).count();
                // A plan the batch must not build: a skip is the architect's own decision, a fault is not.
                let mut faults = if has_utf8_bom(&path) {
                    vec!["plan.json has a UTF-8 BOM; write UTF-8 without BOM".to_string()]
                } else {
                    Vec::new()
                };
                let is_math_unit = self
                    .model_of(&json!(model))
                    .map(|st| is_math_unit_structure(&st))
                    .unwrap_or_else(|_| truthy(&self.m(&model)["math_unit"]));
                let formal_model = super::formal::is_formal_model(&plan, is_math_unit);
                let empirical_model = super::empirical::is_empirical(&plan);
                if !(ps::eq(&s(&plan["status"]), "skip") && !plan["status"].is_null()) {
                    faults.extend(Self::plan_explain_faults(&plan));
                    faults.extend(plan_math_faults(&plan));
                    if formal_model {
                        match self.inventory_result(&model) {
                            Ok(Some(inv)) => faults.extend(super::formal::plan_faults(&plan, Some(&inv), staging)),
                            Ok(None) => {
                                if self.inventory_required_for(&model, staging) {
                                    faults.push("inventory_missing: formal model requires pinned inventory.json beside brief.md".into());
                                }
                                faults.extend(super::formal::plan_faults(&plan, None, staging));
                            }
                            Err(msg) => {
                                faults.push(msg);
                                faults.extend(super::formal::plan_faults(&plan, None, staging));
                            }
                        }
                        formal_plans.push((model.clone(), plan.clone()));
                    }
                }
                let status = if faults.is_empty() { "planned" } else { "failed" };
                let needs_review = empirical_model || formal_model || derivations > 0;
                plans.push(json!({
                    "model": model, "status": status, "level": plan["level"], "math_unit": is_math_unit,
                    "parts": parts.len(), "derivations": derivations, "review": needs_review,
                    "faults": faults.len(), "fault": faults.first().cloned().unwrap_or_default(),
                }));
            } else {
                plans.push(json!({"model": model, "status": if ps::eq(&status, "skipped") { "skipped" } else { "failed" }, "parts": 0, "derivations": 0, "review": false, "faults": 0}));
            }
        }
        // Cross-model ownership conflict check across formal plans in the batch (Astra H3).
        if formal_plans.len() >= 2 {
            let refs: Vec<(String, &Value)> = formal_plans.iter().map(|(m, p)| (m.clone(), p)).collect();
            for (conflict_model, msg) in super::inventory::cross_plan_ownership_faults(&refs) {
                if let Some(row) = plans.iter_mut().find(|r| ps::eq(&s(&r["model"]), &conflict_model)) {
                    let prev_faults = row["faults"].as_u64().unwrap_or(0);
                    row["faults"] = json!(prev_faults + 1);
                    if s(&row["fault"]).is_empty() {
                        row["fault"] = json!(msg);
                    }
                    row["status"] = json!("failed");
                }
            }
        }
        let planned = plans.iter().filter(|p| p["status"] == json!("planned")).count();
        let derive = plans
            .iter()
            .filter(|p| p["status"] == json!("planned"))
            .any(|p| p["derivations"].as_u64().unwrap_or(0) > 0);
        let review = plans
            .iter()
            .filter(|p| p["status"] == json!("planned"))
            .any(|p| p["review"] == json!(true));
        let faults = plans.iter().filter_map(|p| p["faults"].as_u64()).sum::<u64>();
        self.out_last(&json!({"ok": true, "step": "plan-status", "planned": planned, "derive": derive, "review": review, "faults": faults, "plans": plans}))
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
            // Give model-head R2 (>40 char) faults an owner before finish: return the exact
            // offending head sentences to the writer during `close` instead of leaving them unfixed.
            if !ps::eq(&s(&self.m(&model)["status"]), "failed") {
                entries.extend(self.head_r2_entries(&model)?);
            }
            // Level 3: the role faults of the build report and the evidence check.
            if ps::eq(&s(&st["build"]["level"]), "empirical") {
                entries.extend(super::empirical::role_entries(&model, &st["build"]));
                if ps::exists(&ps::join(&self.dir(&model), "plan.json")) {
                    entries.extend(self.evidence_check(&model, &st)?);
                }
            }
            // Level 2 formal economic model: Layer 1 role/graph faults + FM1–FM6 + inventory fidelity.
            let is_math_unit = is_math_unit_structure(&st);
            if ps::eq(&s(&st["build"]["level"]), "formal") && !is_math_unit {
                entries.extend(super::formal::role_entries(&model, &st["build"]));
                entries.extend(self.formal_check(&model, &st)?);
            }
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
                let rests: BTreeSet<(String, String)> = list(&st["relations"])
                    .into_iter()
                    .filter(|r| ps::eq(&s(&r["kind"]), "rests-on"))
                    .map(|r| (s(&r["from"]), s(&r["to"])))
                    .collect();
                let plan_parts: Vec<Value> = self.effective_plan_parts(&model, &plan);
                let formal_model = super::formal::is_formal_model(&plan, is_math_unit);
                entries.extend(planned_link_entries(&model, &plan_parts, &own, &models, &resolve, &parts, &pairs, &rests, formal_model));
                let empirical = super::empirical::is_empirical(&plan);
                entries.extend(unwritten_entries(&plan_parts, &own, &st));
                entries.extend(intuition_entries(&plan_parts, &own, &st, empirical));
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
            // A model whose build failed has no part map to rebuild its imports from; its card stays as it is.
            if ps::eq(&s(&self.m(&model)["status"]), "failed") {
                continue;
            }
            let plan = self.plan(&model)?;
            let card = self.card(&json!(model))?;
            let body = s(&card["body"]);
            if let Some(rebuilt) = self.with_imports(&body, &plan, &model)? {
                edits.push(json!({ "tool": "edit_card", "arguments": { "at": model, "body": rebuilt } }));
            }
        }
        let n = edits.len();
        self.edits(edits)?;
        Ok(n)
    }

    /// #197: puts the compiler's 结构 list back on a Model Card whose list does not name every mapped part.
    /// The model's one fill job writes the Model Card too, so a writer may drop the list (or a line of it)
    /// before `close` records it, and `model-finish` refuses a list that misses a part. The list is rebuilt
    /// from the plan and the part map, each split piece after the part it continues; the rest of the body stays.
    fn complete_structures(&mut self) -> Result<usize> {
        let mut edits = Vec::new();
        for model in self.order() {
            let Ok(plan) = self.plan(&model) else { continue };
            let addresses: Vec<String> = self.m(&model)["ids"]
                .as_object()
                .map(|ids| ids.values().map(s).filter(|a| !a.is_empty() && !a.contains('#')).collect())
                .unwrap_or_default();
            if addresses.is_empty() {
                continue;
            }
            let card = self.card(&json!(model))?;
            let body = s(&card["body"]);
            let section = text::get_section(&body, "结构").unwrap_or_default() + &text::get_section(&body, "引入").unwrap_or_default();
            if addresses.iter().all(|a| section.contains(&format!("[[{a}]]"))) {
                continue;
            }
            let rebuilt = self.structure_list(&plan, &model)?;
            let mut lines: Vec<String> = rebuilt.split('\n').map(str::to_string).collect();
            let main = strings(&self.m(&model)["main"]);
            for (k, id) in main.iter().enumerate() {
                let added = self.m(&model)["added"][id].clone();
                if !added.is_object() || k == 0 {
                    continue;
                }
                let link = format!("[[{}]]", self.id_address(&model, &main[k - 1]));
                let line = format!("- {} [[{}]]：{}", s(&added["facet"]), self.id_address(&model, id), s(&added["title"]));
                match lines.iter().rposition(|l| l.contains(&link)) {
                    Some(at) => lines.insert(at + 1, line),
                    None => lines.push(line),
                }
            }
            // Also preserve Side-branch split pieces, ordered so each `after` predecessor
            // is inserted before its successor piece (e.g. `D1` -> `D1~2` -> `D1~3`).
            if let Some(added_map) = self.m(&model)["added"].as_object().cloned() {
                let mut side_added: Vec<(String, Value)> = added_map
                    .into_iter()
                    .filter(|(_, v)| v.is_object() && ps::eq(&s(&v["line"]), "side") && truthy(&v["after"]))
                    .collect();
                side_added.sort_by(|a, b| a.0.cmp(&b.0));
                let mut inserted: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
                for _ in 0..=side_added.len() {
                    let mut progressed = false;
                    for (id, added) in &side_added {
                        if inserted.contains(id) {
                            continue;
                        }
                        let prev = s(&added["after"]);
                        let prev_addr = self.id_address(&model, &prev);
                        let addr = self.id_address(&model, id);
                        if addr.is_empty() || lines.iter().any(|l| l.contains(&format!("[[{addr}]]"))) {
                            inserted.insert(id.clone());
                            progressed = true;
                            continue;
                        }
                        let link = format!("[[{prev_addr}]]");
                        if let Some(at) = lines.iter().rposition(|l| l.contains(&link)) {
                            let line = format!("- {} [[{addr}]]：{}", s(&added["facet"]), s(&added["title"]));
                            lines.insert(at + 1, line);
                            inserted.insert(id.clone());
                            progressed = true;
                        }
                    }
                    if !progressed {
                        break;
                    }
                }
                for (id, added) in &side_added {
                    if inserted.contains(id) {
                        continue;
                    }
                    let addr = self.id_address(&model, id);
                    if !addr.is_empty() && !lines.iter().any(|l| l.contains(&format!("[[{addr}]]"))) {
                        lines.push(format!("- {} [[{addr}]]：{}", s(&added["facet"]), s(&added["title"])));
                    }
                }
            }
            let list = lines.join("\n");
            let list = list.strip_prefix("**结构**\n\n").unwrap_or(&list).to_string();
            let body = text::set_section(&body, "结构", &format!("**结构**\n\n{list}"));
            let body = self.with_imports(&body, &plan, &model)?.unwrap_or(body);
            edits.push(json!({ "tool": "edit_card", "arguments": { "at": model, "body": body } }));
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
            if let Some(section) = text::get_section(&s(&card["body"]), "引入") {
                self.m_mut(&model)["imports_close"] = json!(section);
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
            let mut fixed = body.clone();
            if text::get_section(&fixed, "结构").as_deref() != Some(kept.as_str()) {
                fixed = text::set_section(&fixed, "结构", &kept);
            }
            let imports = s(&self.m(&model)["imports_close"]);
            if !imports.is_empty()
                && let Some(rebuilt) = imports_rebuilt(&fixed, &imports)
            {
                fixed = rebuilt;
            }
            if fixed != body {
                edits.push(json!({ "tool": "edit_card", "arguments": { "at": model, "body": fixed } }));
            }
            self.m_mut(&model)["structure_close"] = Value::Null;
            self.m_mut(&model)["imports_close"] = Value::Null;
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

    /// Blocking R2 (>40 char) faults on the Model Card head itself, returned as a `close` 修正 note.
    fn head_r2_entries(&mut self, model: &str) -> Result<Vec<(String, String)>> {
        let check = Check {
            model: model.to_string(),
            skip_shared: true,
            max_sentences: self.p.max_sentences,
            max_chars: self.p.max_chars,
            max_steps: self.p.max_steps,
            max_explain_sentences: self.p.max_explain_sentences,
            max_explain_chars: self.p.max_explain_chars,
            ..Check::default()
        };
        let lines = check.run(self.zt()?)?;
        Ok(head_r2_note(model, &lines).into_iter().map(|note| (model.to_string(), note)).collect())
    }

    /// The follow-up jobs of a split or close round (#194): every follow-up continues the model's
    /// conversation (`<slug>:brief`), with a fallback to the files of a fresh conversation. The note
    /// carries the ALLOW file of the job that wrote the Card; a Card no job of this batch wrote gets a
    /// new job whose own allow list names it, so a Card of a model of this batch is never `unowned`.
    fn follow_jobs(&mut self, follow: &CiMap<Follow>) -> Result<Vec<Value>> {
        // ONE follow-up per model: every follow-up continues the model's one conversation (`<slug>:brief`),
        // so two of them would run in it side by side. The notes of every job the model's faults sit in go
        // into one job file, and its ALLOW file names every Card those jobs may write. rotunno2026industrial
        // (2026-11-13): once close also sent unwritten parts back, a model's notes fell into its fill job and
        // its derive job alike.
        let mut models: Vec<String> = Vec::new();
        let mut grouped: CiMap<Vec<(String, String, Vec<String>, Vec<String>)>> = CiMap::new();
        for (key, f) in follow.iter() {
            if !models.iter().any(|m| ps::eq(m, &f.model)) {
                models.push(f.model.clone());
            }
            let mut entries = grouped.get(&f.model).cloned().unwrap_or_default();
            entries.push((key.clone(), f.text.clone(), f.ids.clone(), f.cards.clone()));
            grouped.insert(&f.model, entries);
        }
        let mut jobs = Vec::new();
        for model in models {
            let entries = grouped.get(&model).cloned().unwrap_or_default();
            let mut text = String::new();
            let mut new_ids: Vec<String> = Vec::new();
            let mut cards: Vec<String> = Vec::new();
            let mut owned: Vec<String> = Vec::new();
            let mut allowed: Vec<String> = Vec::new();
            for (key, note, ids, entry_cards) in &entries {
                text += note;
                let allow = if ps::like(key, "new:*") { Value::Null } else { self.state["jobs"][key]["allow"].clone() };
                if truthy(&allow) {
                    owned.push(key.clone());
                    for line in std::fs::read_to_string(s(&allow)).unwrap_or_default().lines().map(str::trim).filter(|l| !l.is_empty()) {
                        if !allowed.iter().any(|a| a == line) {
                            allowed.push(line.to_string());
                        }
                    }
                } else {
                    new_ids.extend(ids.iter().filter(|i| !new_ids.contains(i)).cloned().collect::<Vec<_>>());
                }
                for c in entry_cards {
                    if !cards.contains(c) {
                        cards.push(c.clone());
                    }
                }
            }
            let mut blocks = Vec::new();
            let mut n = 0;
            for id in &new_ids {
                if let Some(p) = self.plan_part(&model, id)? {
                    n += 1;
                    blocks.push(self.part_block(n, &p, &model, true)?);
                }
            }
            if owned.is_empty() {
                // No job of this batch wrote these Cards: a new job, whose own allow list names them.
                let ids = new_ids.iter().map(|i| json!(i)).collect();
                let mut job = self.new_job(&model, "d", ids, &format!("{text}\n{}", blocks.join("\n")), "按以上说明完成你的分卡，最后每张分卡回复一行 JSON。")?;
                self.add_cards_to_job(&mut job, &cards)?;
                self.continue_model(&mut job, &model, &self.docs.fill)?;
                jobs.push(job);
                continue;
            }
            // Every Card a note names, at its current address: a split after the job renumbers its parts, and the
            // job's own ALLOW file keeps the old ones.
            let named: Vec<String> = entries.iter().flat_map(|(_, _, ids, _)| ids.clone()).collect();
            let before = allowed.len();
            for at in named.iter().map(|id| self.id_address(&model, id)).chain(cards.iter().cloned()).collect::<Vec<_>>() {
                if !at.is_empty() && !at.contains('#') && !allowed.contains(&at) {
                    allowed.push(at);
                }
            }
            let seq = self.next_seq();
            let dir = self.dir(&model);
            let file = ps::join(&dir, &format!("follow-{seq}.md"));
            let allow = if owned.len() == 1 && allowed.len() == before {
                s(&self.state["jobs"][&owned[0]]["allow"])
            } else {
                let merged = ps::join(&dir, &format!("follow-{seq}.allow.txt"));
                util::write_text(&merged, &(allowed.join("\n") + "\n"))?;
                merged
            };
            let blocks = if blocks.is_empty() { String::new() } else { format!("\n{}", blocks.join("\n")) };
            util::write_text(&file, &format!("# 补充任务（接着你刚才的分卡写）\n\nALLOW 已更新：{allow}\n\n{text}{blocks}\n写完后照常运行 CHECK（地址加上新分卡）。"))?;
            // fallback: the files of a fresh conversation, when the model's conversation is gone
            let mut fallback = vec![json!(self.docs.fill), json!(ps::join(&dir, "setting.md"))];
            fallback.extend(owned.iter().map(|k| self.state["jobs"][k]["file"].clone()));
            fallback.push(json!(file));
            jobs.push(json!({
                "id": format!("{}+{seq}", owned[0]),
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
        self.complete_structures()?;
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
            let formal_faults = count(&build["formal_shape"])
                + count(&build["setting_dependency"])
                + count(&build["criterion_unbound"])
                + count(&build["result_unbound"])
                + count(&build["proof_unbound"])
                + count(&build["experiment_unbound"])
                + count(&build["symbol_uncovered"]);
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
                    "explained": Self::explained_of(&st)?.len(),
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
                    "empirical": count(&build["empirical_shape"]) + count(&build["estimate_unbound"]) + count(&build["spec_unbound"]) + count(&build["test_unbound"]) + count(&build["variable_uncovered"]),
                    "formal": formal_faults,
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
                    "explained":Self::explained_of(&st)?.len(),
                    "style":style,
                    "build_faults":count(&build["closure"])+count(&build["forward"])+count(&build["unanchored"])+count(&build["beyond_level"])+formal_faults
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
        let plan_parts = self.effective_plan_parts(model, &plan);
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
            let written = !unwritten(&card);
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
        let head_missing = unwritten(&head_card) || s(&head_card["body"]).trim().is_empty();
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
        if ps::eq(&s(&plan["level"]), "empirical") {
            card_text += "- 实证模型：写 **问题**、**数据**（一两句）、**关联**、来源。**估计对象** 和 **主要估计** 两节由脚本写好，原样保留。**设计** 一节第一行冒号前的设计类型原样保留，冒号后写一两句比较（谁和谁比，变异从哪里来）。\n";
        }
        if super::formal::is_formal_model(plan, truthy(&self.m(model)["math_unit"])) {
            card_text += "- 形式模型：写 **问题**、**原语与判据**（两三句，链接核心判据卡）、**关联**、来源。**目标对象与标签**、**主要结果**、**结构**（及 **引入**、**分部**）由脚本写好，原样保留。\n";
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

    /// `Get-OwnDerivations`: the model's own filed Derivation parts, in build order (M7).
    fn own_derivations(st: &Value) -> Result<Vec<String>> {
        own_derivations_strict(st)
    }

    /// The own Derivation parts whose 思路 does not start with the chain position line the
    /// current chain and results give them. A chain of one part needs no position line.
    fn misplaced(st: &Value, own: &[String], body: &dyn Fn(&str) -> String) -> Vec<String> {
        stale_positions(&st["build"], own, body)
    }

    fn bodies_of(st: &Value) -> CiMap<String> {
        let mut bodies = CiMap::new();
        for p in list(&st["parts"]) {
            bodies.insert(&s(&p["address"]), s(&p["body"]));
        }
        bodies
    }

    /// `Get-Explained`.
    fn explained_of(st: &Value) -> Result<Vec<String>> {
        let bodies = Self::bodies_of(st);
        Ok(Self::own_derivations(st)?
            .into_iter()
            .filter(|a| text::explained(&bodies.get(a).cloned().unwrap_or_default()))
            .collect())
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
            if body.trim() != s(&j["bodies"][a]).trim() {
                r.refused.push(format!("{a}: the part changed after its job"));
                continue;
            }
            if s(&e["idea"]).trim().is_empty() || s(&e["meaning"]).trim().is_empty() {
                r.faults.insert(a, "idea and meaning must not be empty".into());
                continue;
            }
            let new = text::add_explanation(&body, &s(&e["idea"]), &e["notes"], &s(&e["meaning"]), &s(&j["positions"][a]));
            if text::core(&new) != text::core(&body) {
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
        let model_addr = s(&j["model"]);
        let setting = s(&self.state["explain"]["settings"][&model_addr]);
        let setting_card = if setting.is_empty() { Value::Null } else { self.card(&json!(setting))? };
        let (model_level, model_form) = self
            .model_of(&json!(model_addr))
            .map(|st| (st["level"].clone(), st["form"].clone()))
            .unwrap_or_else(|_| (self.m(&model_addr)["level"].clone(), Value::Null));
        let old_items: Vec<Value> = r
            .candidates
            .iter()
            .map(|c| json!({ "address": c.address, "title": c.title, "body": c.old, "facet": c.facet, "level": model_level, "form": model_form }))
            .collect();
        let new_items: Vec<Value> = r
            .candidates
            .iter()
            .map(|c| json!({ "address": c.address, "title": c.title, "body": c.body, "facet": c.facet, "level": model_level, "form": model_form }))
            .collect();
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
            // P2b-1: include `<setting>|a` (formal notation card) and its 1-hop `沿用 [[...]]`
            // borrowed notation card when present so R6 reads the exact same term table (Astra #11).
            let notation_addr = format!("{setting}|a");
            if let Ok(notation_card) = self.card(&json!(notation_addr))
                && !s(&notation_card["body"]).is_empty()
            {
                let nb = s(&notation_card["body"]);
                to_check.push(json!({
                    "address": notation_addr,
                    "title": s(&notation_card["title"]),
                    "body": nb,
                    "facet": s(&notation_card["facet"]),
                }));
                if zt_check::text::term_table(&nb).is_empty() {
                    for borrowed in zt_check::formal::borrowed_notation_targets(&nb) {
                        if let Ok(bcard) = self.card(&json!(borrowed))
                            && !zt_check::text::term_table(&s(&bcard["body"])).is_empty()
                        {
                            to_check.push(json!({
                                "address": borrowed,
                                "title": s(&bcard["title"]),
                                "body": s(&bcard["body"]),
                                "facet": s(&bcard["facet"]),
                            }));
                            break;
                        }
                    }
                }
            }
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
        let file = if self.p.file.is_empty() {
            resolve_explain_out_path(&self.p.work, &s(&self.state["explain"]["jobs"][&job]["out"]))?
        } else {
            self.p.file.clone()
        };
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
            let out_file = resolve_explain_out_path(&self.p.work, &s(&j["out"]))?;
            let r = self.test_explain_job(&id, &out_file)?;
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
            // §3.7: a refused part goes back to the same conversation, at most three rounds.
            if ps::int(&j["round"]).unwrap_or(0) >= 3 {
                for (a, f) in r.faults.iter() {
                    refused.push((model.clone(), format!("{a}: {f}")));
                    done.push((id.clone(), a.clone()));
                }
                continue;
            }
            // Once per round: the same conversation rewrites the refused parts into a new result file
            // (§3.7: at most three rounds, so round 2 and round 3 each get one follow-up).
            let round = ps::int(&j["round"]).unwrap_or(1) + 1;
            let seq = self.next_seq();
            let out = ps::join(&ps::join(&self.p.work, "out"), &format!("explain-{seq}.json"));
            self.state["explain"]["jobs"][&id]["round"] = json!(round);
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
                "id": format!("{id}+{round}"),
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
            let own = Self::own_derivations(&st)?;
            let explained = Self::explained_of(&st)?;
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

/// `Get-OwnDerivations`: the model's own filed Derivation parts, in build order (M7).
/// Shared by `step_explain`, `explained_of`, `step_explain_report`, and `model_explain` so
/// planning, apply, and recheck agree on one strict predicate: requires `build.order` and `parts`
/// arrays, explicit boolean `filed` on every part (`filed == true` to qualify), explicit boolean
/// `derivation` (`derivation == true`), and an explicit non-empty non-shared `line`.
pub(crate) fn own_derivations_strict(st: &Value) -> Result<Vec<String>> {
    let order = st
        .get("build")
        .and_then(|b| b.get("order"))
        .and_then(Value::as_array)
        .context("get_model answer lacks `build.order` array")?;
    let parts = st
        .get("parts")
        .and_then(Value::as_array)
        .context("get_model answer lacks `parts` array")?;
    let mut filed_parts: HashSet<String> = HashSet::new();
    for (idx, p) in parts.iter().enumerate() {
        let Some(obj) = p.as_object() else {
            bail!("get_model answer parts[{idx}] must be a JSON object");
        };
        let Some(addr) = obj.get("address").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            bail!("get_model answer parts[{idx}] lacks non-empty string `address`");
        };
        let Some(filed) = obj.get("filed").and_then(Value::as_bool) else {
            bail!("get_model answer parts[{idx}] (`{addr}`) lacks boolean `filed`");
        };
        if filed {
            filed_parts.insert(addr.to_string());
        }
    }
    let mut out = Vec::new();
    for (idx, o) in order.iter().enumerate() {
        let Some(obj) = o.as_object() else {
            bail!("get_model answer build.order[{idx}] must be a JSON object");
        };
        let Some(addr) = obj.get("address").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            bail!("get_model answer build.order[{idx}] lacks non-empty string `address`");
        };
        let deriv = match obj.get("derivation") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => {
                bail!("get_model answer build.order[{idx}] (`{addr}`) has non-boolean `derivation`");
            }
        };
        let Some(line) = obj.get("line").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            bail!("get_model answer build.order[{idx}] (`{addr}`) lacks non-empty string `line`");
        };
        if deriv && !line.eq_ignore_ascii_case("shared") && filed_parts.contains(addr) {
            out.push(addr.to_string());
        }
    }
    Ok(out)
}

/// Resolves a saved explain job's `out` path without depending on the resume process's working
/// directory (M3): absolute paths pass through, unambiguous `out/<file>` relative paths resolve
/// inside `work`, and ambiguous relative paths (e.g. `..` traversal or a cwd-relative prefix
/// like `old-work/out/explain-1.json`) are refused before apply.
pub(crate) fn resolve_explain_out_path(work: &str, raw_out: &str) -> Result<String> {
    let trimmed = raw_out.trim();
    if trimmed.is_empty() {
        bail!("saved explain job has empty `out` path");
    }
    let p = std::path::Path::new(trimmed);
    if p.is_absolute() {
        return Ok(trimmed.to_string());
    }
    let mut comps = Vec::new();
    for c in p.components() {
        match c {
            std::path::Component::Normal(seg) => comps.push(seg.to_string_lossy().into_owned()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
            | std::path::Component::RootDir
            | std::path::Component::Prefix(_) => {
                bail!(
                    "saved explain job `out` path `{trimmed}` escapes work directory `{work}`; refusing ambiguous relative path"
                );
            }
        }
    }
    if comps.len() == 2 && comps[0].eq_ignore_ascii_case("out") {
        let work_path = std::path::Path::new(work);
        let base = if work_path.is_absolute() {
            work_path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(work_path))
                .unwrap_or_else(|_| work_path.to_path_buf())
        };
        return Ok(base.join("out").join(&comps[1]).to_string_lossy().into_owned());
    }
    bail!(
        "saved explain job `out` path `{trimmed}` is relative and depends on launch working directory; refusing ambiguous resume against work directory `{work}` (re-run plan or make `out` absolute)"
    )
}

/// The Derivation chains of a model: the runs of zt's `build.chains` (main and shared lines), and
/// every Side-line proof chain as zt-model-restructure files it: a Derivation part X, then its
/// continuations X|1, X|2, … while they are Side-line Derivation parts. A chain may start at X|1
/// when X is no Derivation part (a proof hung under a setting or a result). Each chain is
/// `{parts, after, before}` like a zt chain: `after` is the card it hangs under, `before` the card
/// that ends it, when the model has one.
pub(crate) fn proof_chains(build: &Value) -> Vec<Value> {
    let mut chains: Vec<Value> = list(&build["chains"]).into_iter().cloned().collect();
    // Deduplicate against Side chains already emitted by `src/model.rs` `build_report` (F4).
    let mut seen_parts: HashSet<String> = HashSet::new();
    for c in &chains {
        for a in pipe(&c["parts"]) {
            seen_parts.insert(s(a).to_ascii_lowercase());
        }
    }
    let side: Vec<String> = list(&build["order"])
        .into_iter()
        .filter(|o| truthy(&o["derivation"]) && ps::eq(&s(&o["line"]), "side"))
        .map(|o| s(&o["address"]))
        .collect();
    let known: Vec<String> = list(&build["order"]).into_iter().map(|o| s(&o["address"])).collect();
    let numbered = |a: &str| -> Option<(String, u64)> {
        let (prefix, last) = a.rsplit_once('|')?;
        last.parse::<u64>().ok().map(|n| (prefix.to_string(), n))
    };
    let previous = |a: &str| -> Option<String> { numbered(a).map(|(prefix, n)| if n > 1 { format!("{prefix}|{}", n - 1) } else { prefix }) };
    let next = |a: &str| -> String {
        match numbered(a) {
            Some((prefix, n)) => format!("{prefix}|{}", n + 1),
            None => format!("{a}|1"),
        }
    };
    for head in &side {
        if seen_parts.contains(&head.to_ascii_lowercase()) {
            continue;
        }
        if previous(head).is_some_and(|p| ps::is_in(&p, &side)) {
            continue;
        }
        let mut parts = vec![head.clone()];
        let mut at = next(head);
        while ps::is_in(&at, &side) && !ps::is_in(&at, &parts) {
            parts.push(at.clone());
            at = next(&at);
        }
        for p in &parts {
            seen_parts.insert(p.to_ascii_lowercase());
        }
        let after = previous(head).or_else(|| head.rsplit_once('|').map(|(p, _)| p.to_string()));
        let before = ps::is_in(&at, &known).then_some(at);
        chains.push(json!({ "parts": parts, "length": parts.len(), "after": after, "before": before }));
    }
    chains
}

/// The own Derivation parts whose 思路 does not start with the position line their chain
/// (`proof_chains`) and its current results give them: a stale or missing line in a chain of two
/// or more, and any position line, in either wording, on a part that stands alone. Adoption counts
/// them as an explanation need exactly as the explain step does.
pub(crate) fn stale_positions(build: &Value, own: &[String], body: &dyn Fn(&str) -> String) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut chained: Vec<String> = Vec::new();
    for c in proof_chains(build) {
        let parts: Vec<String> = pipe(&c["parts"]).into_iter().map(s).filter(|a| ps::is_in(a, own)).collect();
        let tos: Vec<String> = parts.iter().map(|a| text::result_of(&body(a))).collect();
        for (index, a) in parts.iter().enumerate() {
            let expected = text::chain_line(index + 1, parts.len(), &tos);
            let found = text::position_of(&body(a));
            let stale = if expected.is_empty() { text::is_position_line(&found) } else { found != expected };
            if stale && !ps::is_in(a, &out) {
                out.push(a.clone());
            }
            chained.push(a.clone());
        }
    }
    for a in own.iter().filter(|a| !ps::is_in(a, &chained)) {
        if text::is_position_line(&text::position_of(&body(a))) && !ps::is_in(a, &out) {
            out.push(a.clone());
        }
    }
    out
}

/// The no-main-line fault of `test_plan`: the model's own parts have no main line, yet the plan
/// adds a new main-line part. Only filed parts count; a part filed under another model and joined
/// by a Link belongs to its home model, so a model whose only members are linked parts has none.
fn lacks_main_line(structure: &Value, current_main: &[String], main: &[&Value]) -> bool {
    let has_filed = pipe(&structure["parts"]).into_iter().any(|p| !ps::is_false(&p["filed"]));
    current_main.is_empty() && has_filed && main.iter().any(|p| !truthy(&p["existing"]))
}

/// #204 (B6): the faults of the plan's `divisions` (the model's parts, 分部): each needs a name and
/// a `start` that is the id of a main-line part of the plan.
fn division_faults(plan: &Value) -> Vec<String> {
    let mut faults = Vec::new();
    let parts = list(&plan["parts"]);
    for (i, d) in list(&plan["divisions"]).iter().enumerate() {
        let start = s(&d["start"]);
        if s(&d["name"]).trim().is_empty() {
            faults.push(format!("division {} has no name", i + 1));
            continue;
        }
        match parts.iter().find(|p| ps::eq(&s(&p["id"]), &start)) {
            None => faults.push(format!("division {}: start {start} is no part of the plan", i + 1)),
            Some(p) if !ps::eq(&text::line_of(p), "main") => {
                faults.push(format!("division {}: start {start} is not on the main line; a division starts at a main-line part", i + 1))
            }
            Some(_) => {}
        }
    }
    faults
}

/// #204 (B6): the Model Card's 分部 section from the plan's `divisions`, or `None` without any.
fn divisions_section(plan: &Value, resolve: impl Fn(&str) -> Result<String>) -> Result<Option<String>> {
    let divisions = list(&plan["divisions"]);
    if divisions.is_empty() {
        return Ok(None);
    }
    let mut lines = Vec::new();
    for d in divisions {
        lines.push(format!("- {}：[[{}]] 起", s(&d["name"]).trim(), resolve(&s(&d["start"]))?));
    }
    Ok(Some(format!("**分部**\n\n{}", lines.join("\n"))))
}

/// furceri2026divide (2026-11-14): models 7|a, 8|a and 9|a had every part and no 待写, but their build in an
/// earlier batch had `failed`; a new batch's brief skipped them as built and the finish refused them again. The
/// newest other batch state (`models\*\state.json` beside this batch) that names the model decides: a status
/// other than a completed build (`built`, `derived`, `reviewed`) keeps the model unbuilt.
fn failed_build_elsewhere(work: &str, model: &str) -> bool {
    let own = std::path::Path::new(work);
    let Some(root) = own.parent() else { return false };
    let Ok(entries) = std::fs::read_dir(root) else { return false };
    let mut latest: Option<(std::time::SystemTime, String)> = None;
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() || dir.file_name() == own.file_name() {
            continue;
        }
        let file = dir.join("state.json");
        let Ok(meta) = std::fs::metadata(&file) else { continue };
        let Ok(time) = meta.modified() else { continue };
        let Ok(state) = util::read_json(&file) else { continue };
        let Some(models) = state["models"].as_object() else { continue };
        let Some(entry) = models.iter().find(|(k, _)| ps::eq(k, model)).map(|(_, v)| v) else { continue };
        if latest.as_ref().is_none_or(|(t, _)| time > *t) {
            latest = Some((time, s(&entry["status"])));
        }
    }
    latest.is_some_and(|(_, status)| !ps::is_in(&status, &["built", "derived", "reviewed"]))
}

/// Whether the brief may skip a model as already built: parts of its own, self-sufficient, nothing beyond its
/// level, nothing unwritten. deming2015growing/13|a (2026-11-21): the Model Card had no part of its own, but its
/// head linked the main-text chapter `2|b|a|2`; `get_model` listed that chapter among `parts` (a false share), the
/// brief skipped the model as built and the finish refused it with "no accepted plan". Only the parts filed under
/// the Model Card count, as in the model listing (`list::own_parts`).
fn built_as_is(model: &str, st: &Value, todo: bool) -> bool {
    let b = &st["build"];
    let no_formal_faults = b["formal_structured"] != json!(false)
        && [
            "import_target",
            "import_cycle",
            "formal_shape",
            "setting_dependency",
            "criterion_unbound",
            "result_unbound",
            "proof_unbound",
            "experiment_unbound",
            "symbol_uncovered",
        ]
        .iter()
        .all(|k| list(&b[*k]).is_empty());
    super::list::own_parts(model, &pipe(&st["parts"])) > 0
        && truthy(&b["self_sufficient"])
        && list(&b["beyond_level"]).is_empty()
        && no_formal_faults
        && !todo
}

#[cfg(test)]
mod built_tests {
    use super::built_as_is;
    use serde_json::json;

    #[test]
    fn a_model_whose_only_part_is_another_models_chapter_is_not_built() {
        let st = |parts: serde_json::Value| json!({"parts": parts, "build": {"self_sufficient": true, "beyond_level": []}});
        let shared = st(json!([{"address": "K/2|b|a|2"}]));
        assert!(!built_as_is("K/13|a", &shared, false), "a false share is not a part of the model");
        let own = st(json!([{"address": "K/13|a|a"}, {"address": "K/2|b|a|2"}]));
        assert!(built_as_is("K/13|a", &own, false));
        assert!(!built_as_is("K/13|a", &own, true), "an unwritten part keeps the model unbuilt");
    }
}

#[cfg(test)]
mod failed_build_tests {
    use super::failed_build_elsewhere;

    #[test]
    fn a_model_whose_latest_other_batch_build_failed_is_not_built() {
        let root = tempfile::tempdir().unwrap();
        let write = |dir: &str, body: &str, age: u64| {
            let d = root.path().join(dir);
            std::fs::create_dir_all(&d).unwrap();
            let f = d.join("state.json");
            std::fs::write(&f, body).unwrap();
            let t = std::time::SystemTime::now() - std::time::Duration::from_secs(age);
            std::fs::File::options().write(true).open(&f).unwrap().set_modified(t).unwrap();
        };
        write("m-old", r#"{"models":{"K/7|a":{"status":"failed"},"K/8|a":{"status":"failed"}}}"#, 7200);
        write("m-new", r#"{"models":{"K/8|a":{"status":"reviewed"}}}"#, 3600);
        let own = root.path().join("m-this");
        std::fs::create_dir_all(&own).unwrap();
        let work = own.to_string_lossy().to_string();
        assert!(failed_build_elsewhere(&work, "K/7|a"), "the only build failed");
        assert!(failed_build_elsewhere(&work, "k/7|A"), "addresses compare without case");
        assert!(!failed_build_elsewhere(&work, "K/8|a"), "a newer reviewed build wins over an older failed one");
        assert!(!failed_build_elsewhere(&work, "K/9|a"), "a model no batch names keeps the listing's verdict");
        // The batch's own state is this run's, never the earlier verdict.
        write("m-this", r#"{"models":{"K/9|a":{"status":"failed"}}}"#, 0);
        assert!(!failed_build_elsewhere(&work, "K/9|a"));
    }
}

/// Whether a Card still holds a placeholder: the compiler's own `待写` lines (with the setting intuition
/// lines) or the run report's skeleton sections and titles, so skip, resume and finish agree.
fn unwritten(card: &Value) -> bool {
    TODO_LINE.is_match(&s(&card["body"])) || crate::report::is_placeholder(&s(&card["title"]), &s(&card["body"]))
}

/// A skeleton title without its placeholder (`4.1 幂法则：待写` → `4.1 幂法则`), or `None` when the title
/// is written already or nothing would remain.
pub(crate) fn cleared_title(title: &str) -> Option<String> {
    let t = title.trim();
    let rest = t
        .strip_suffix("（待写）")
        .or_else(|| t.strip_suffix("待写").and_then(|r| r.trim_end().strip_suffix(['：', ':'])))?;
    let rest = rest.trim();
    (!rest.is_empty()).then(|| rest.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_unit_skeleton_head_is_unwritten_for_skip_and_resume() {
        // Astra round 6: the body-only test took `**陈述**：待写。` as written, so a partly built unit was skipped.
        assert!(super::unwritten(&json!({"title":"4.1 幂法则：待写","body":"**陈述**：待写。"})));
        assert!(super::unwritten(&json!({"title":"幂法则","body":"**陈述**：待写。"})));
        assert!(super::unwritten(&json!({"title":"假设","body":"- 是什么：待写
- 为什么需要：x"})));
        assert!(!super::unwritten(&json!({"title":"幂法则","body":"**陈述**：若 $y=x^n$……"})));
    }

    #[test]
    fn a_skeleton_title_loses_its_placeholder_when_the_plan_names_none() {
        assert_eq!(super::cleared_title("4.1 幂法则：待写").as_deref(), Some("4.1 幂法则"));
        assert_eq!(super::cleared_title("理论：第 4 章（待写）").as_deref(), Some("理论：第 4 章"));
        assert_eq!(super::cleared_title("幂法则：若 y=xⁿ 则 dy/dx=nxⁿ⁻¹"), None, "a written title stays");
        assert_eq!(super::cleared_title("：待写"), None, "nothing left is no title");
    }

    use super::{close_entries, division_faults, divisions_section, import_section, imports_rebuilt, intuition_entries, lacks_main_line, marker_entries, supplied_entries, track_renames, current_address, unwritten_entries, TODO_LINE};
    use std::collections::BTreeMap;
    use serde_json::json;

    // 0.52.0: every setting, assumption and definition part this build writes carries a written **直觉**;
    // close sends one 修正 note per part that lacks it, and an unwritten 直觉 line keeps the part unwritten.
    #[test]
    fn a_setting_part_without_a_written_intuition_goes_back_to_its_writer() {
        let parts = vec![
            json!({"id": "S", "facet": "设定", "line": "main"}),
            json!({"id": "A1", "facet": "假设 1", "line": "side"}),
            json!({"id": "D", "facet": "定义", "line": "side"}),
            json!({"id": "R", "facet": "结果：命题 1", "line": "side"}),
            json!({"id": "T", "facet": "设定", "take": true}),
            json!({"id": "E", "facet": "设定", "existing": true}),
        ];
        // `S~2` is a piece `split` cut from S: it continues S, whose first piece holds the one 直觉.
        let ids = json!({"S": "K/1|a|a", "S~2": "K/1|a|a|e", "A1": "K/1|a|a|a", "D": "K/1|a|a|b", "R": "K/1|a|a|c", "T": "C/2|a|a", "E": "K/1|a|a|d"});
        let written = "**直觉**\n- 是什么：员工分配努力。\n- 为什么需要：没有它就没有权衡。\n- 怎样进入模型：第 2 章用它。";
        let st = json!({"parts": [
            {"address": "K/1|a|a", "body": format!("设定内容。\n\n{written}")},
            {"address": "K/1|a|a|a", "body": "假设内容。\n\n**直觉**\n- 是什么：待写\n- 为什么需要：待写\n- 怎样进入模型：待写"},
            {"address": "K/1|a|a|b", "body": "定义内容，没有直觉。"},
            {"address": "K/1|a|a|c", "body": "结果。"},
            {"address": "C/2|a|a", "body": "他人的设定。"},
            {"address": "K/1|a|a|d", "body": "保留的旧设定。"},
            {"address": "K/1|a|a|e", "body": "依赖上一步 [[K/1|a|a]]：本卡接着上一张卡写。设定的后半。"},
        ]});
        let notes = intuition_entries(&parts, &ids, &st, false);
        let at: Vec<&str> = notes.iter().map(|(a, _)| a.as_str()).collect();
        assert_eq!(at, ["K/1|a|a|a", "K/1|a|a|b"], "{notes:?}");
        assert!(notes[0].1.contains("缺少写好的「是什么」「为什么需要」「怎样进入模型」行") && notes[0].1.contains("**直觉**"), "{}", notes[0].1);
        assert!(notes[1].1.contains("没有 **直觉** 一节"), "{}", notes[1].1);
        assert!(intuition_entries(&parts, &ids, &st, true).is_empty(), "an empirical model's parts follow their roles");
        // An unwritten 直觉 line keeps the part unwritten; the empirical skeleton's labelled 待写 lines do not match.
        assert!(TODO_LINE.is_match("内容。\n\n**直觉**\n- 是什么：待写\n- 为什么需要：好。"));
        assert!(!TODO_LINE.is_match(&format!("内容。\n\n{written}")));
        assert!(!TODO_LINE.is_match("- 值：待写（从原文抄录）"));
    }

    // rotunno2026industrial/4|a (2026-11-13): part O (line `off`, 校准与求解) was in the fill job, the writer
    // left it 待写, the close round sent no note for it, and the model reached `reviewed` with `todo: 1`, which
    // model-finish refuses. Close sends every part this build owns that is still unwritten back to its writer,
    // with the plan's title, content and source; a part whose only 待写 lines are its 直觉 goes back through
    // `intuition_entries`, and a taken or kept part is not this build's.
    #[test]
    fn an_unwritten_part_goes_back_to_its_writer_at_close() {
        let parts = vec![
            json!({"id": "S", "facet": "设定", "line": "main", "title": "设定"}),
            json!({"id": "O", "facet": "校准与求解", "line": "off", "emphasis": "aside", "title": "校准与求解：十国加 ROW、20 个部门、迭代算法", "spec": "十国加 ROW、20 部门、2015 年基年", "src": ["sections/s04.md:75-98"]}),
            json!({"id": "D1", "facet": "推导", "line": "side", "derive": true, "title": "第一步", "spec": "从需求推出份额", "src": ["sections/s03.md:10-20"]}),
            json!({"id": "A1", "facet": "假设 1", "line": "side", "title": "假设"}),
            json!({"id": "T", "facet": "设定", "take": true}),
            json!({"id": "E", "facet": "设定", "existing": true}),
        ];
        let ids = json!({"S": "K/4|a|a", "O": "K/4|a|b", "D1": "K/4|a|a|1", "A1": "K/4|a|a|a", "T": "C/2|a|a", "E": "K/4|a|a|b"});
        let st = json!({"parts": [
            {"address": "K/4|a|a", "body": "设定内容。\n\n**直觉**\n- 是什么：甲。\n- 为什么需要：乙。\n- 怎样进入模型：丙。"},
            {"address": "K/4|a|b", "body": "待写"},
            {"address": "K/4|a|a|1", "body": "**思路**\n待写\n\n**起点**\n- 依赖上一步 [[K/4|a|a]] 的结果：份额。\n\n**步骤**\n待写\n\n**结果**\n份额\n\n**解读**\n待写"},
            {"address": "K/4|a|a|a", "body": "假设内容。\n\n**直觉**\n- 是什么：待写\n- 为什么需要：待写\n- 怎样进入模型：待写"},
            {"address": "C/2|a|a", "body": "待写"},
            {"address": "K/4|a|a|b", "body": "待写"},
        ]});
        let notes = unwritten_entries(&parts, &ids, &st);
        let at: Vec<&str> = notes.iter().map(|(a, _)| a.as_str()).collect();
        assert_eq!(at, ["K/4|a|b", "K/4|a|a|1"], "{notes:?}");
        let o = &notes[0].1;
        assert!(o.starts_with("## 修正：[[K/4|a|b]]"), "{o}");
        assert!(o.contains("待写") && o.contains("校准与求解：十国加 ROW、20 个部门、迭代算法") && o.contains("十国加 ROW、20 部门、2015 年基年") && o.contains("sections/s04.md:75-98"), "{o}");
        assert!(notes[1].1.contains("从需求推出份额") && notes[1].1.contains("推导"), "{}", notes[1].1);
        // A Model Card's own parts written in full send nothing.
        let done = json!({"parts": [{"address": "K/4|a|b", "body": "十国加 ROW，20 个部门。"}]});
        assert!(unwritten_entries(&parts[1..2], &ids, &done).is_empty());
    }

    // #204 (B6): the architect's divisions (the model's parts, 分部) start at main-line parts; the
    // compiler writes them as the Model Card's 分部 section, which the GUI reads as range blocks.
    #[test]
    fn divisions_start_at_main_line_parts_and_become_the_parts_section() {
        let plan = json!({
            "parts": [
                {"id": "S", "facet": "设定", "line": "main"},
                {"id": "C1", "facet": "第 1 章：生产", "line": "main"},
                {"id": "R1", "facet": "结果：效率", "line": "side", "under": "C1"},
                {"id": "C2", "facet": "第 2 章：均衡", "line": "main"}
            ],
            "divisions": [
                {"name": "第一部分 构件", "start": "S"},
                {"name": "第二部分 均衡", "start": "C2"}
            ]
        });
        assert!(division_faults(&plan).is_empty(), "{:?}", division_faults(&plan));
        let section = divisions_section(&plan, |id| Ok(format!("K/4|a|{id}"))).unwrap().unwrap();
        assert_eq!(section, "**分部**\n\n- 第一部分 构件：[[K/4|a|S]] 起\n- 第二部分 均衡：[[K/4|a|C2]] 起");
        let bad = json!({
            "parts": plan["parts"],
            "divisions": [{"name": "第一部分", "start": "R1"}, {"name": "", "start": "C2"}, {"name": "第三部分", "start": "X"}]
        });
        let faults = division_faults(&bad);
        assert_eq!(faults.len(), 3, "{faults:?}");
        assert!(faults[0].contains("R1") && faults[0].contains("main line"));
        assert!(divisions_section(&json!({"parts": []}), |id| Ok(id.to_string())).unwrap().is_none(), "no divisions, no section");
    }

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

    // #212 (I7 of #205): a taken part is an **引入** line, not a （共用） line of the 结构 list: 单卡 for one
    // Card, 整块 when the plan takes the Card with its block; a model that takes nothing has no section.
    #[test]
    fn a_taken_part_is_an_import_line() {
        let section = import_section(&[
            (false, "B/2|a|a|12".to_string(), "π_m (6)；用于定价".to_string()),
            (true, "C/4|a|a".to_string(), "他书的设定".to_string()),
        ])
        .unwrap();
        assert_eq!(section, "**引入**

- 单卡 [[B/2|a|a|12]]：π_m (6)，用于定价
- 整块 [[C/4|a|a]]：他书的设定");
        assert!(import_section(&[]).is_none());
    }

    // #212: after a split renumbers a taken part, the 引入 lines follow the plan's current addresses; a
    // section that already reads so is left alone.
    #[test]
    fn import_lines_follow_the_taken_parts() {
        let body = "问题：x

**引入**

- 单卡 [[B/2|a|a|12]]：π_m (6)

**结构**

- 设定 [[B/3|a|a]]：设定";
        let wanted = import_section(&[(false, "B/2|a|a|13".to_string(), "π_m (6)".to_string())]).unwrap();
        let rebuilt = imports_rebuilt(body, &wanted).unwrap();
        assert_eq!(rebuilt, "问题：x

**引入**

- 单卡 [[B/2|a|a|13]]：π_m (6)

**结构**

- 设定 [[B/3|a|a]]：设定");
        assert!(imports_rebuilt(&rebuilt, &wanted).is_none(), "nothing to do");
        // atkinson2015lectures: a take that resolved to its bare plan id is no import line.
        assert_eq!(
            import_section(&[(false, "TN".to_string(), "记号".to_string()), (false, "B/2|a|a|13".to_string(), "π_m (6)".to_string())]).unwrap(),
            "**引入**\n\n- 单卡 [[B/2|a|a|13]]：π_m (6)"
        );
        assert!(import_section(&[(false, "TN".to_string(), "记号".to_string())]).is_none());
        // A Model Card without the section (an older build) gets it before 结构.
        let old = "问题：x

**结构**

- 设定 [[B/3|a|a]]：设定";
        assert!(imports_rebuilt(old, &wanted).unwrap().contains("**引入**

- 单卡 [[B/2|a|a|13]]"));
    }

    // -Step defer: the released models leave the order (every later step iterates it) and read deferred;
    // a model the batch does not hold is ignored.
    /// ebrill2001modern pilot (2026-11-09): an insert renumbers later Cards twice in one apply; a merge
    /// still finds the Card the plan named.
    #[test]
    fn a_merged_card_is_found_after_the_inserts_renumbered_it() {
        let mut renamed = BTreeMap::new();
        let first = json!({"K/5|a|a|1": "K/5|a|a|2", "K/5|a|a|2": "K/5|a|a|3", "K/5|a|a|2|b|a|a": "K/5|a|a|3|b|a|a"});
        track_renames(&mut renamed, first.as_object().unwrap());
        let second = json!({"K/5|a|a|3": "K/5|a|a|4", "K/5|a|a|3|b|a|a": "K/5|a|a|4|b|a|a"});
        track_renames(&mut renamed, second.as_object().unwrap());
        assert_eq!(current_address(&renamed, "K/5|a|a|2|b|a|a"), "K/5|a|a|4|b|a|a");
        assert_eq!(current_address(&renamed, "K/5|a|a|2"), "K/5|a|a|4");
        assert_eq!(current_address(&renamed, "K/5|a|a|1"), "K/5|a|a|2");
        assert_eq!(current_address(&renamed, "K/9"), "K/9");
    }

    #[test]
    fn deferred_models_leave_the_batch_order() {
        let mut state = json!({"order": ["K/2|a", "K/3|a", "K/4|a"], "models": {"K/2|a": {"status": "planned"}, "K/3|a": {"status": "planned"}, "K/4|a": {"status": "planned"}}});
        let deferred = super::defer_models(&mut state, &["k/3|A".to_string(), "K/4|a".to_string(), "K/9|a".to_string()]);
        assert_eq!(deferred, vec!["K/3|a".to_string(), "K/4|a".to_string()]);
        assert_eq!(state["order"], json!(["K/2|a"]));
        // atkinson2015lectures: a rebuild that reuses a finished batch's folder begins its own order; the finished
        // model keeps its state entry.
        let mut reused = json!({"order":["K/51|a","K/52|a","K/53|a"],"models":{"K/52|a":{"status":"reviewed","steps":{"report":{}}}}});
        super::begin_batch_order(&mut reused, &["K/51|a".to_string(), "K/60|a".to_string()]);
        assert_eq!(reused["order"], json!(["K/51|a"]));
        assert_eq!(reused["models"]["K/52|a"]["status"], json!("reviewed"));
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
        let entries = super::planned_link_entries("K/7|a", &plan_parts, &ids, &json!({}), &resolve, &parts, &pairs, &pairs, true);
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].0, "K/7|a|a|5");
        assert!(entries[0].1.contains("[[K/3|a|a|2]]") && entries[0].1.contains("用到均衡条件"));
    }

    // atkinson2015lectures: the architect took `K/48|a` (another model's Model Card) as a shared part; the take
    // mapped to no part and model-finish refused the model after the whole build. A model head is a Model Card
    // without a facet; its parts carry one.
    #[test]
    fn a_take_of_a_whole_model_is_told_apart_from_a_take_of_a_part() {
        assert!(super::whole_model_take(&json!({"type":"model","facet":null,"title":"资产组合弹性模型"})));
        assert!(!super::whole_model_take(&json!({"type":"model","facet":"命题"})));
        assert!(!super::whole_model_take(&json!({"type":"idea"})));
        assert!(!super::whole_model_take(&serde_json::Value::Null));
    }

    // #197 pilot: a refilled part kept its old `参照 [[K/5|a|a|4]]` where the plan says it rests on the taken
    // definition (G3 = K/5|a#DEF). Only a rests-on Link takes a shared part in, so the take was outside the
    // build order and model-finish refused the model ("mapped but belongs to no model"). Close sends it back.
    #[test]
    fn a_refilled_part_that_only_refers_to_its_take_is_a_close_entry() {
        use std::collections::BTreeSet;

        let plan_parts = vec![
            json!({"id":"G3","take":"K/5|a#DEF"}),
            json!({"id":"COR","existing":"K/7|a|a|3","refill":true,"rests":[["G3","用到两类技术的定义"]]}),
            json!({"id":"KEEP","existing":"K/7|a|a|4","rests":[["G3","用到定义"]]}),
        ];
        let ids = json!({"G3":"K/5|a|a|4","COR":"K/7|a|a|3","KEEP":"K/7|a|a|4"});
        let resolve = |r: &str| -> Option<String> { ids[r].as_str().map(str::to_string) };
        let parts: BTreeSet<String> = ["K/7|a|a|3", "K/7|a|a|4"].into_iter().map(String::from).collect();
        let refers: BTreeSet<(String, String)> = [("K/7|a|a|3".to_string(), "K/5|a|a|4".to_string())].into_iter().collect();
        let entries = super::planned_link_entries("K/7|a", &plan_parts, &ids, &json!({}), &resolve, &parts, &refers, &BTreeSet::new(), true);
        assert_eq!(entries.len(), 1, "the refilled part goes back, the kept part does not: {entries:?}");
        assert_eq!(entries[0].0, "K/7|a|a|3");
        assert!(entries[0].1.contains("依赖 [[K/5|a|a|4]]") && entries[0].1.contains("参照"), "{}", entries[0].1);
        assert!(super::planned_link_entries("K/7|a", &plan_parts, &ids, &json!({}), &resolve, &parts, &refers, &refers, true).is_empty(), "a rests-on Link to the take authors it");
    }

    // #197 pilot: the architect merged `7|b|a|6` into a take; the apply step skipped the merge because the
    // part has a successor, the part stayed unmapped, and model-finish refused `7|b` and `8|a`. plan-status
    // refuses a merge that the build cannot carry out, so the architect revises the plan first.
    #[test]
    fn a_merge_of_a_part_with_successors_is_a_plan_fault() {
        use std::collections::HashSet;

        let plan = json!({"merge":[{"from":"K/7|b|a|6","into":"G5"},{"from":"K/7|b|a|9","into":"E2"}]});
        let tree: HashSet<String> = ["K/7|b", "K/7|b|a", "K/7|b|a|6", "K/7|b|a|7", "K/7|b|a|9"].into_iter().map(String::from).collect();
        let faults = super::Compiler::merge_faults(&plan, &tree).unwrap();
        assert_eq!(faults.len(), 1, "{faults:?}");
        assert!(faults[0].contains("K/7|b|a|6") && faults[0].contains("existing"), "{}", faults[0]);
    }

    // #197 pilot run 5: `3|a` took `2|a#D5` "for comparison" and no part rested on it, so the take stayed
    // outside the build order and model-finish refused the model. A take joins a model only through a
    // rests-on Link, so a take that no part names in `rests` or `of` is a plan fault.
    #[test]
    fn a_take_no_part_rests_on_is_a_plan_fault() {
        let plan = json!({"parts":[
            {"id":"S","line":"main"},
            {"id":"TS","take":"K/2|a#S"},
            {"id":"TD","take":"K/2|a#D5"},
            {"id":"TX","take":"K/2|a#X"},
            {"id":"D1","line":"main","rests":[["TS","用到设定"]]},
            {"id":"N","line":"side","of":"TX"}
        ]});
        let faults = super::Compiler::take_faults(&plan);
        assert_eq!(faults.len(), 1, "{faults:?}");
        assert!(faults[0].contains("TD") && faults[0].contains("rests"), "{}", faults[0]);
    }

    // autor2004will (2026-11-09): `9|a` took `5#S` and `5#R`, a batch number for the model, the plan validated,
    // and the build failed ("model 5 has no part S yet"). The model before `#` is its Model Card's address.
    #[test]
    fn a_take_that_names_no_model_address_is_a_plan_fault() {
        let plan = json!({"parts":[
            {"id":"TS","take":"5#S"},
            {"id":"TR","take":"K/2|a#R"},
            {"id":"D1","line":"main","rests":[["TS","用到设定"],["TR","用到结果"]]}
        ]});
        let faults = super::Compiler::take_faults(&plan);
        assert_eq!(faults.len(), 1, "{faults:?}");
        assert!(faults[0].contains("TS") && faults[0].contains("5#S") && faults[0].contains("address"), "{}", faults[0]);
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
        assert!(super::planned_link_entries("K/7|a", &plan_parts, &ids, &json!({}), &resolve, &parts, &to_last, &to_last, true).is_empty());
        let none = BTreeSet::new();
        let entries = super::planned_link_entries("K/7|a", &plan_parts, &ids, &json!({}), &resolve, &parts, &none, &none, true);
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

    #[test]
    fn derivation_to_must_contain_a_math_expression() {
        assert!(super::has_math_expr("$$x^k = \\theta^k (\\dots) \\quad (18)$$"));
        assert!(super::has_math_expr("P ≤ L"));
        assert!(!super::has_math_expr("式 (18) 与 (19)"));
        assert!(!super::has_math_expr("命题 1"));
    }

    #[test]
    fn head_r2_note_returns_only_blocking_head_r2_lines() {
        let lines = vec![
            "klette2004innovating/3|a R2 (63 characters > 40) 长句一…".to_string(),
            "klette2004innovating/3|a|a|2 R2 (45 characters > 40) 分卡长句…".to_string(),
            "klette2004innovating/3|a R20 (advisory) local reference…".to_string(),
            "faults: 2".to_string(),
        ];
        let note = super::head_r2_note("klette2004innovating/3|a", &lines).expect("head R2 note");
        assert!(note.contains("模型头卡短句 R2"), "{note}");
        assert!(note.contains("R2 (63 characters > 40)"), "{note}");
        assert!(!note.contains("分卡长句"), "{note}");
        assert!(!note.contains("R20"), "{note}");
        assert!(super::head_r2_note("klette2004innovating/3|a", &["faults: 0".to_string()]).is_none());
    }

    fn test_params(step: &str, work: &str, staging: &str) -> super::Params {
        super::Params {
            step: step.to_string(),
            work: work.to_string(),
            staging: staging.to_string(),
            models: String::new(),
            plan: String::new(),
            need: String::new(),
            changed: false,
            rebuild: false,
            report: false,
            independent_review: false,
            strict_audit: false,
            batch: false,
            level: String::new(),
            max_sentences: 6,
            max_chars: 180,
            max_steps: 5,
            max_explain_sentences: 2,
            max_explain_chars: 120,
            max_parts: 3,
            job: String::new(),
            file: String::new(),
        }
    }

    fn test_docs() -> super::Docs {
        super::Docs {
            brief: "b".into(),
            fill: "f".into(),
            review: "r".into(),
            explain: "e".into(),
            brief_empirical: "be".into(),
            review_empirical: "re".into(),
        }
    }

    #[test]
    fn has_utf8_bom_detects_only_leading_ef_bb_bf() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().join("plain.json");
        let bom = dir.path().join("bom.json");
        std::fs::write(&plain, b"{\"ok\":true}").unwrap();
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(b"{\"ok\":true}");
        std::fs::write(&bom, with_bom).unwrap();
        assert!(!super::has_utf8_bom(plain.to_str().unwrap()));
        assert!(super::has_utf8_bom(bom.to_str().unwrap()));
    }

    #[test]
    fn validate_batches_manifest_checks_duplicates_overlap_and_required_models() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("manifest.json"), r#"{"citation_key":"K"}"#).unwrap();
        let w1 = dir.path().join("b1");
        let w2 = dir.path().join("b2");
        let mf = dir.path().join("batches.json");

        // Duplicate work directory
        std::fs::write(&mf, serde_json::to_string(&json!([
            {"work": w1.to_str().unwrap(), "models": ["K/1|a"]},
            {"work": w1.to_str().unwrap(), "models": ["K/2|a"]},
        ])).unwrap()).unwrap();
        let err = super::validate_batches_manifest("brief", staging.to_str().unwrap(), mf.to_str().unwrap()).err().unwrap().to_string();
        assert!(err.contains("duplicate"), "{err}");

        // Overlapping model assignment across batches
        std::fs::write(&mf, serde_json::to_string(&json!([
            {"work": w1.to_str().unwrap(), "models": ["K/1|a"]},
            {"work": w2.to_str().unwrap(), "models": ["K/1|a"]},
        ])).unwrap()).unwrap();
        let err = super::validate_batches_manifest("brief", staging.to_str().unwrap(), mf.to_str().unwrap()).err().unwrap().to_string();
        assert!(err.contains("more than one batch"), "{err}");

        // Continuation step requires existing state.json
        let err = super::validate_batches_manifest("derive", staging.to_str().unwrap(), mf.to_str().unwrap()).err().unwrap().to_string();
        assert!(err.contains("requires existing"), "{err}");

        // Colliding or unsafe jobs_file rejected
        let shared_jobs = dir.path().join("jobs.json");
        std::fs::write(&mf, serde_json::to_string(&json!([
            {"work": w1.to_str().unwrap(), "models": ["K/1|a"], "jobs_file": shared_jobs.to_str().unwrap()},
            {"work": w2.to_str().unwrap(), "models": ["K/2|a"], "jobs_file": shared_jobs.to_str().unwrap()},
        ])).unwrap()).unwrap();
        let err = super::validate_batches_manifest("brief", staging.to_str().unwrap(), mf.to_str().unwrap()).err().unwrap().to_string();
        assert!(err.contains("colliding"), "{err}");
        std::fs::write(&mf, serde_json::to_string(&json!([
            {"work": w1.to_str().unwrap(), "models": ["K/1|a"], "jobs_file": w1.join("state.json").to_str().unwrap()},
        ])).unwrap()).unwrap();
        let err = super::validate_batches_manifest("brief", staging.to_str().unwrap(), mf.to_str().unwrap()).err().unwrap().to_string();
        assert!(err.contains("state.json or plan.json"), "{err}");

        // Missing models for brief
        std::fs::write(&mf, serde_json::to_string(&json!([
            {"work": w1.to_str().unwrap()}
        ])).unwrap()).unwrap();
        let err = super::validate_batches_manifest("brief", staging.to_str().unwrap(), mf.to_str().unwrap()).err().unwrap().to_string();
        assert!(err.contains("requires a non-empty `models`"), "{err}");

        // Valid manifest
        std::fs::write(&mf, serde_json::to_string(&json!([
            {"work": w1.to_str().unwrap(), "models": ["K/1|a"]},
            {"work": w2.to_str().unwrap(), "models": "K/2|a,K/3|a"},
        ])).unwrap()).unwrap();
        let (key, specs) = super::validate_batches_manifest("brief", staging.to_str().unwrap(), mf.to_str().unwrap()).unwrap();
        assert_eq!(key, "K");
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[1].models, "K/2|a,K/3|a");
    }

    #[test]
    fn audit_and_collect_reviews_validates_strictly_and_preserves_state_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let staging = dir.path().join("staging");
        let slug = super::slug_of("K/1|a");
        let mdir = work.join(&slug);
        std::fs::create_dir_all(&mdir).unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(mdir.join("plan.json"), serde_json::to_string(&json!({
            "model": "K/1|a",
            "level": "formal",
            "parts": [
                {"id": "S", "line": "main", "facet": "设定"},
                {"id": "D1", "line": "side", "facet": "推导", "derive": true},
            ]
        })).unwrap()).unwrap();
        let rev_path = crate::ps::full_name(mdir.join("review-1.json").to_str().unwrap());
        let state_val = json!({
            "order": ["K/1|a"],
            "models": {
                "K/1|a": {
                    "ids": {"S": "K/1|a|a", "D1": "K/1|a|a|1"},
                    "status": "reviewed"
                }
            },
            "jobs": {},
            "rereview": [],
            "reviews": [],
            "pending_reviews": [{
                "output": rev_path,
                "model": "K/1|a",
                "round": 1,
                "addresses": ["K/1|a|a|1"],
                "ids": ["D1"]
            }],
            "seq": 1
        });
        std::fs::write(work.join("state.json"), serde_json::to_string_pretty(&state_val).unwrap()).unwrap();

        let make = |step: &str, strict_audit: bool| {
            let mut p = test_params(step, work.to_str().unwrap(), staging.to_str().unwrap());
            p.strict_audit = strict_audit;
            super::new_compiler(p, test_docs(), "-MaxExplainSentences 2".into(), "--max-steps 5".into(), "h".into(), None, true).unwrap()
        };

        // (a) missing review file fails strict audit
        let mut c = make("review-audit", true);
        let (errs, _, _, _, _) = c.audit_and_collect_reviews(true).unwrap();
        assert!(errs.iter().any(|e| e.contains("missing review file")), "{errs:?}");

        // (b) non-array JSON fails strict audit
        std::fs::write(&rev_path, r#"{"not":"an array"}"#).unwrap();
        let mut c = make("review-audit", true);
        let (errs, _, _, _, _) = c.audit_and_collect_reviews(true).unwrap();
        assert!(errs.iter().any(|e| e.contains("must be a list")), "{errs:?}");

        // (c) unexpected / omitted assigned address fails strict audit
        std::fs::write(&rev_path, serde_json::to_string(&json!([
            {"address": "K/9|a|a|1", "verdict": "ok"}
        ])).unwrap()).unwrap();
        let mut c = make("split", true);
        let rc = c.step_split().unwrap();
        assert_eq!(rc, 1, "strict split must fail on unexpected/omitted address");
        let saved_after_fail: serde_json::Value = crate::util::read_json(work.join("state.json").to_str().unwrap()).unwrap();
        assert!(crate::ps::list(&saved_after_fail["reviews"]).is_empty(), "reviews must stay unconsumed on audit error");
        assert_eq!(crate::ps::list(&saved_after_fail["pending_reviews"]).len(), 1);

        // (d) bad split (parts < 2 or wrong cuts.len) fails strict audit
        std::fs::write(&rev_path, serde_json::to_string(&json!([
            {"address": "K/1|a|a|1", "verdict": "split", "parts": 2, "cuts": [], "reason": "too long"}
        ])).unwrap()).unwrap();
        let mut c = make("review-audit", true);
        let rc = c.step_review_audit().unwrap();
        assert_eq!(rc, 1);

        // (e) valid fix/split collects expected Actions without errors
        std::fs::write(&rev_path, serde_json::to_string(&json!([
            {"address": "K/1|a|a|1", "verdict": "split", "parts": 2, "cuts": ["split here"], "reason": "two steps"}
        ])).unwrap()).unwrap();
        let mut c = make("review-audit", true);
        let (errs, _, actions, consumed, counts) = c.audit_and_collect_reviews(true).unwrap();
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(counts, [0, 0, 1]);
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].verdict, "split");
        assert_eq!(actions[0].parts, 2);
        assert_eq!(consumed, vec![rev_path.clone()]);
        assert_eq!(c.step_review_audit().unwrap(), 0);
        // Needs split still leaves pending_reviews for step_split to consume!
        let saved_mid: serde_json::Value = crate::util::read_json(work.join("state.json").to_str().unwrap()).unwrap();
        assert_eq!(crate::ps::list(&saved_mid["pending_reviews"]).len(), 1);

        // (f) all-ok consumes pending_reviews during review-audit
        std::fs::write(&rev_path, serde_json::to_string(&json!([
            {"address": "K/1|a|a|1", "verdict": "ok"}
        ])).unwrap()).unwrap();
        let mut c = make("review-audit", true);
        assert_eq!(c.step_review_audit().unwrap(), 0);
        let saved_ok: serde_json::Value = crate::util::read_json(work.join("state.json").to_str().unwrap()).unwrap();
        assert!(crate::ps::list(&saved_ok["pending_reviews"]).is_empty());
        assert_eq!(crate::ps::list(&saved_ok["reviews"]).len(), 1);
    }

    #[test]
    fn load_renew_units_map_validates_entries_strictly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("units.json");

        std::fs::write(&path, r#"[{"id":"","token":1}]"#).unwrap();
        assert!(super::load_renew_units_map(path.to_str().unwrap()).is_err());

        std::fs::write(&path, r#"[{"id":"u1","token":0}]"#).unwrap();
        assert!(super::load_renew_units_map(path.to_str().unwrap()).is_err());

        std::fs::write(&path, r#"[{"id":"u1","token":1},{"id":"u1","token":2}]"#).unwrap();
        assert!(super::load_renew_units_map(path.to_str().unwrap()).is_err());

        std::fs::write(&path, serde_json::to_string(&json!({
            "units": [{"id": "u1", "token": 7, "inputs": {"header": "K/1|a"}}]
        })).unwrap()).unwrap();
        let (pairs, map) = super::load_renew_units_map(path.to_str().unwrap()).unwrap();
        assert_eq!(pairs, vec![("u1".to_string(), 7)]);
        assert_eq!(map.get("u1").map(String::as_str), Some("K/1|a"));
    }

    #[test]
    fn step_split_rejects_duplicate_need_targets() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let staging = dir.path().join("staging");
        let mdir = work.join(super::slug_of("K/1|a"));
        std::fs::create_dir_all(&mdir).unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(mdir.join("plan.json"), serde_json::to_string(&json!({
            "model": "K/1|a",
            "level": "formal",
            "parts": [{"id": "D1", "line": "side", "facet": "推导", "derive": true}]
        })).unwrap()).unwrap();
        std::fs::write(work.join("state.json"), serde_json::to_string(&json!({
            "order": ["K/1|a"],
            "models": {"K/1|a": {"ids": {"D1": "K/1|a|a|1"}, "status": "reviewed"}},
            "jobs": {},
            "rereview": [],
            "reviews": [],
            "pending_reviews": [],
            "seq": 1
        })).unwrap()).unwrap();

        let mut p = test_params("split", work.to_str().unwrap(), staging.to_str().unwrap());
        p.need = "K/1|a|a|1=2;K/1|a|a|1=3".to_string();
        p.strict_audit = true;
        let mut c = super::new_compiler(p, test_docs(), "-MaxExplainSentences 2".into(), "--max-steps 5".into(), "h".into(), None, true).unwrap();
        assert_eq!(c.step_split().unwrap(), 1);
        let last = c.last_out.as_ref().unwrap();
        let errs = last["audit_errors"].as_array().unwrap();
        assert!(errs.iter().any(|e| e.as_str().unwrap_or("").contains("duplicate -Need")), "{errs:?}");

        let mut p2 = test_params("split", work.to_str().unwrap(), staging.to_str().unwrap());
        p2.need = "K/1|a|a|1=2;K/1|a|a|1=3".to_string();
        p2.strict_audit = false;
        let mut c2 = super::new_compiler(p2, test_docs(), "-MaxExplainSentences 2".into(), "--max-steps 5".into(), "h".into(), None, true).unwrap();
        assert!(c2.step_split().is_err());
    }

    #[test]
    fn audit_and_collect_reviews_uses_address_to_id_after_address_shift() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let staging = dir.path().join("staging");
        let mdir = work.join(super::slug_of("K/1|a"));
        std::fs::create_dir_all(&mdir).unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(mdir.join("plan.json"), serde_json::to_string(&json!({
            "model": "K/1|a",
            "level": "formal",
            "parts": [
                {"id": "D1", "line": "side", "facet": "推导", "derive": true},
                {"id": "D2", "line": "side", "facet": "推导", "derive": true}
            ]
        })).unwrap()).unwrap();
        let rev_path = crate::ps::full_name(mdir.join("review-1.json").to_str().unwrap());
        std::fs::write(&rev_path, serde_json::to_string(&json!([
            {"address": "K/1|a|a|2", "verdict": "fix", "reason": "clarify"}
        ])).unwrap()).unwrap();
        // Simulate address shift after an earlier split: D2 moved to |3, while pending_reviews remembered |2 -> D2.
        std::fs::write(work.join("state.json"), serde_json::to_string(&json!({
            "order": ["K/1|a"],
            "models": {"K/1|a": {"ids": {"D1": "K/1|a|a|1", "D1~2": "K/1|a|a|2", "D2": "K/1|a|a|3"}, "status": "reviewed"}},
            "jobs": {},
            "rereview": [],
            "reviews": [],
            "pending_reviews": [{
                "output": rev_path,
                "model": "K/1|a",
                "round": 1,
                "addresses": ["K/1|a|a|2"],
                "address_to_id": {"k/1|a|a|2": "D2"}
            }],
            "seq": 1
        })).unwrap()).unwrap();

        let mut p = test_params("review-audit", work.to_str().unwrap(), staging.to_str().unwrap());
        p.strict_audit = true;
        let mut c = super::new_compiler(p, test_docs(), "-MaxExplainSentences 2".into(), "--max-steps 5".into(), "h".into(), None, true).unwrap();
        let (errs, _, actions, _, _) = c.audit_and_collect_reviews(true).unwrap();
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].id, "D2");
    }

    #[test]
    fn handle_apply_result_propagates_transport_and_stopped_without_marking_failed() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let staging = dir.path().join("staging");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(work.join("state.json"), serde_json::to_string(&json!({
            "order": ["K/1|a"],
            "models": {"K/1|a": {"ids": {}, "status": "planned"}},
            "jobs": {},
            "rereview": [],
            "reviews": [],
            "pending_reviews": [],
            "seq": 0
        })).unwrap()).unwrap();

        let p = test_params("apply", work.to_str().unwrap(), staging.to_str().unwrap());
        let mut c = super::new_compiler(p, test_docs(), "-MaxExplainSentences 2".into(), "--max-steps 5".into(), "h".into(), None, true).unwrap();

        let tf_err = anyhow::Error::from(zt_check::mcp::TransportFault {
            code: "transport_closed",
            operation: "get_card".into(),
            child_pid: 42,
            cleanup_confirmed: true,
            cause: "closed".into(),
        });
        assert!(c.handle_apply_result("K/1|a", Err(tf_err)).is_err());
        assert_eq!(c.m("K/1|a")["status"], "planned");

        let soft_err = anyhow::anyhow!("bad plan syntax");
        assert!(c.handle_apply_result("K/1|a", Err(soft_err)).unwrap().is_empty());
        assert_eq!(c.m("K/1|a")["status"], "failed");
    }

    #[test]
    fn derivation_skeleton_inlines_previous_to_and_from_without_raw_display_or_tags() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        let staging = dir.path().join("staging");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(work.join("state.json"), serde_json::to_string(&json!({
            "order": ["K/1|a"],
            "models": {"K/1|a": {"ids": {"D1": "K/1|a|a|1", "D2": "K/1|a|a|2"}, "status": "planned"}},
            "jobs": {},
            "rereview": [],
            "reviews": [],
            "pending_reviews": [],
            "seq": 0
        })).unwrap()).unwrap();

        let p = test_params("apply", work.to_str().unwrap(), staging.to_str().unwrap());
        let c = super::new_compiler(p, test_docs(), "-MaxExplainSentences 2".into(), "--max-steps 5".into(), "h".into(), None, true).unwrap();
        let prev = json!({
            "id": "D1", "line": "side", "of": "R1", "derive": true, "facet": "推导：第一步",
            "to": r"$$\phi_1 = \frac{\lambda_H}{\lambda_H + 2\lambda_L} \tag {10}$$"
        });
        let curr = json!({
            "id": "D2", "line": "side", "after": "D1", "derive": true, "facet": "推导：第二步",
            "from": r"$$x = y \tag*{2}$$",
            "to": r"$$\phi_2 = 1 - \phi_1 \tag {11}$$"
        });
        let plan = json!({ "model": "K/1|a", "level": "formal", "parts": [prev.clone(), curr.clone()] });
        let body = c.skeleton(&curr, &plan, "K/1|a", Some(&prev), false).unwrap();
        let start_section = super::text::get_section(&body, "起点").expect("起点 section");
        assert!(!start_section.contains("$$"), "起点 must not embed raw $$ inline: {start_section}");
        assert!(!start_section.contains("\\tag"), "起点 must not embed \\tag inline: {start_section}");
        assert!(start_section.contains(r"$\phi_1 = \frac{\lambda_H}{\lambda_H + 2\lambda_L} \quad (10)$"), "{start_section}");
        assert!(start_section.contains(r"- 从 $x = y \quad 2$ 出发。"), "{start_section}");
        // Result section preserves display math and its original \tag verbatim.
        let result_section = super::text::get_section(&body, "结果").expect("结果 section");
        assert!(result_section.contains(r"$$\phi_2 = 1 - \phi_1 \tag {11}$$"), "{result_section}");
    }

    #[test]
    fn plan_math_faults_catches_bare_latex_and_inline_tags_on_rendered_plan_fields() {
        let bad = json!({
            "model": "K/1|a",
            "level": "formal",
            "title": r"当 \pi\in(0,1) 时的均衡",
            "card": r"问题：求解 \lambda > 0 的最优解。",
            "estimand": r"估计 \beta_{it} 对产出的影响",
            "divisions": [{"name": r"第一部分 \alpha 设定", "start": "S0"}],
            "symbols": [[r"\nu", "稳态份额", "S0"], ["$x$", r"产出 q_{ijt}", "S0"]],
            "variables": [["$y_{it}$", r"当 \gamma > 0 时", "对数", "数据", "S0"]],
            "parts": [
                {
                    "id": "R1",
                    "line": "side",
                    "of": "C1",
                    "facet": r"结果：\theta 阈值",
                    "title": r"结果：条件 $x = 1 \tag {2}$ 成立",
                    "scope": r"仅在 \rho > 0 下",
                    "rests": [["S0", r"用到 \sigma_i^{2}"]],
                    "justifications": [{"force": "heuristic", "location": "local", "ref": "D1", "extra_assumptions": r"\mu 连续"}]
                },
                {
                    "id": "D1",
                    "line": "side",
                    "of": "R1",
                    "derive": true,
                    "facet": "推导：求解",
                    "title": "推导：一步求解",
                    "from": r"条件 \delta = 0",
                    "to": r"\lambda = 1"
                },
                {
                    "id": "S0",
                    "line": "main",
                    "facet": "规格：基准",
                    "title": "规格：基准",
                    "equation": r"y_{it} = \beta x_{it} + \varepsilon_{it} \tag{7}",
                    "terms": {
                        "outcome": r"产出 y_{it}",
                        "regressor": "$x_{it}$",
                        "fe": [r"固定效应 \alpha_i", "年份"],
                        "se": "稳健"
                    },
                    "change": {"kind": "controls", "text": r"加入 \tau_{t}"}
                }
            ]
        });
        let faults = super::plan_math_faults(&bad).join("\n");
        for expected in [
            "plan.title",
            "plan.card",
            "plan.estimand",
            "plan.divisions[0].name",
            "plan.symbols[0][0]",
            "plan.symbols[1][1]",
            "plan.variables[0][1]",
            "part R1.facet",
            "part R1.title",
            "part R1.scope",
            "part R1.rests[0][1]",
            "part R1.justifications[0].extra_assumptions",
            "part D1.from",
            "part D1.to",
            "part S0.terms.outcome",
            "part S0.terms.fe[0]",
            "part S0.change.text",
        ] {
            assert!(faults.contains(expected), "missing fault for `{expected}` in:\n{faults}");
        }
        // Valid empirical bare TeX `equation` with `\tag{7}` must NOT be flagged!
        assert!(!faults.contains("part S0.equation"), "valid empirical equation must pass:\n{faults}");

        // Double-wrapped empirical `equation` (`$$...$$`) IS caught as broken display.
        let double_eq = json!({
            "model": "K/2|a",
            "level": "empirical",
            "parts": [{"id": "S0", "facet": "规格：基准", "title": "规格", "equation": "$$y = x$$"}]
        });
        let df = super::plan_math_faults(&double_eq).join("\n");
        assert!(df.contains("part S0.equation"), "double-wrapped equation must be flagged:\n{df}");

        // Readable Unicode `to` (`P ≤ L`) and properly wrapped TeX pass cleanly.
        let clean = json!({
            "model": "K/1|a",
            "level": "formal",
            "title": r"当 $\pi\in(0,1)$ 时的均衡",
            "card": r"问题：求解 $\lambda > 0$ 的最优解。",
            "symbols": [[r"$\nu$", "稳态份额", "S0"]],
            "parts": [
                {"id": "D1", "facet": "推导：求解", "title": "推导：求解", "derive": true, "to": "P ≤ L"},
                {"id": "D2", "facet": "推导：再求", "title": "推导：再求", "derive": true, "to": r"$$P \le L \tag {10}$$"}
            ]
        });
        assert_eq!(super::plan_math_faults(&clean), Vec::<String>::new());
    }
}
