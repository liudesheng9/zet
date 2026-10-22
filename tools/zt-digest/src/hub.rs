//! `zt-digest hub`: the port of `scripts/book-digest/hub-compiler.ps1`, the deterministic half of
//! the book digest's hub stage (docs/hub-cards-design.md). A Haiku hub manager chooses the limits
//! from the survey; DeepSeek plans the fields and writes every hub as a JSON file; this tool does
//! everything between them without a model: it surveys the agent Workspace, writes the job files,
//! builds and updates the Hub Cards, checks them with get_hub, and reports. No step writes hub
//! prose of its own except fixed lines (范围, the register's lists).
//!
//!   survey                 survey.json, survey.md; prints the size statistics
//!   plan -Limits <json>    clamps and saves the limits; one topic job per -PerJob trees, one field job
//!   apply                  out\hub-<tree>.json and out\fields.json → line, topic, field hubs, register
//!   stage                  the same changes with no write: out of the drafts to <Work>\hub-plan.json,
//!                          each changed existing hub with its authored base (the S27 gate, #119)
//!   check                  get_hub on every hub of the state; one follow-up job per writer with faults
//!   report                 list_hubs: counts, faults, missing hubs; report.json
//!   precheck -File <file>  read-only: judges one DeepSeek file; fault lines, then `faults: N`
//!
//! Every job file names its precheck as `SELF-CHECK: zt-digest hub -Step precheck …` (this tool, no
//! pwsh) and the helper as `HELPER: zt-digest call`. The jobs' instruction file
//! dsh-hub-instructions.zh.md is named in ZT_DIGEST_SCRIPTS (the forwarder sets it) or in the
//! scripts\book-digest folder of the repository this binary was built from.

mod precheck;
mod text;

use crate::args::Args;
use crate::calls;
use crate::canon;
use crate::model::read_calls;
use crate::ps::{self, CiMap, NL};
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;
use text::{FIELD_REF, LINE_REF, TREE_REF, st, tree_id};

const TOPIC_PROMPT: &str = "按以上说明完成本任务的每个主题：写出它的枢纽 JSON 文件，再用它的 SELF-CHECK 命令自查。最后只回复一行 JSON。";
const FIELD_PROMPT: &str = "按以上说明读主题清单，写出领域与索引的 JSON 文件，再用 SELF-CHECK 命令自查。最后只回复一行 JSON。";
const FIX_PROMPT: &str = "按以上问题修正 JSON 文件，再用 SELF-CHECK 命令自查。最后只回复一行 JSON。";
const PENDING_BODY: &str = "范围：待写。";
const LIMIT_NAMES: &[&str] = &["topic_min", "line_min", "max_lines", "max_groups", "min_reason", "neighbour_links"];

static OK_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+ ok (.+)$").unwrap());
static ERROR_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+ error (.*)$").unwrap());
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());
static HUB_FILE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^hub-.*\.json$").unwrap());
static TOPIC_BODY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)Topic body").unwrap());
static LEGACY_WORDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)枢纽|核心卡片|清单").unwrap());

struct Params {
    step: String,
    file: String,
    limits: String,
    trees: String,
    per_job: i64,
    rebuild: bool,
}

pub(crate) struct Hub {
    p: Params,
    /// The work folder, absolute (job files name absolute paths: DeepSeek runs them from its own cwd).
    work: String,
    out: String,
    state_file: String,
    state: Value,
    /// dsh-hub-instructions.zh.md: the first file of every job.
    doc: String,
    zt: Option<Zt>,
    /// `$script:Cards`: the agent Cards by address (case ignored), loaded on first use.
    cards: Option<HashMap<String, Value>>,
    /// The precheck's fault lines.
    pre: Vec<String>,
    pre_limits: Value,
    /// `-Step stage`: every write recorded instead of sent, with its reviewed evidence.
    stage: Option<Stage>,
}

/// The dry run of `apply`: the exact call list it would send, and the evidence of each call.
#[derive(Default)]
struct Stage {
    calls: Vec<Value>,
    /// For each call, its kind, the Card it changes, the body and fingerprint it replaces, and the
    /// `get_hub` policy facts of an existing hub (S27-AC01, AC05).
    evidence: Vec<Value>,
}

impl Stage {
    fn record(&mut self, call: Value, evidence: Value) -> String {
        let index = self.calls.len();
        let address = if evidence["kind"] == json!("create") {
            format!("@@{index}")
        } else {
            st(&evidence["at"])
        };
        self.calls.push(call);
        self.evidence.push(evidence);
        address
    }
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

/// A path as `GetUnresolvedProviderPathFromPSPath` gives it: absolute, `..` resolved, `\` separators.
fn full_path(path: &str) -> Result<String> {
    Ok(std::path::absolute(path).with_context(|| format!("cannot resolve the path `{path}`"))?.to_string_lossy().into_owned())
}

/// `Read-Json`: `ConvertFrom-Json -AsHashtable` of a file (an empty file is null).
fn read_json(path: &str) -> Result<Value> {
    let raw = util::read_text(path)?;
    ps::parse_json(&raw).map_err(|e| anyhow!("Conversion from JSON failed with error: {e}"))
}

/// `$a -ge $n` with a number on the left: false for null.
fn ge(v: &Value, n: i64) -> bool {
    v.as_f64().is_some_and(|x| x >= n as f64)
}

/// `$n -gt $limit` / `$n -lt $limit` with a limit from limits.json: null is below every number.
fn gt_limit(n: i64, limit: &Value) -> bool {
    match limit {
        Value::Null => true,
        other => other.as_f64().map_or_else(|| n > ps::int(other).unwrap_or(0), |x| n as f64 > x),
    }
}

fn lt_limit(n: i64, limit: &Value) -> bool {
    match limit {
        Value::Null => false,
        other => other.as_f64().map_or_else(|| n < ps::int(other).unwrap_or(0), |x| (n as f64) < x),
    }
}

/// `Get-Clamped`: the value as `[int]`, or the default when it is null or empty, kept in [min, max].
fn clamped(value: &Value, default: i64, min: i64, max: i64) -> Result<i64> {
    let v = if value.is_null() || st(value).is_empty() { default } else { ps::int(value)? };
    Ok(min.max(max.min(v)))
}

fn obj(v: &Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

/// `Save-Text`: UTF-8 without a BOM, the folder made first.
fn save(path: &str, text: &str) -> Result<()> {
    util::write_text(path, text)
}

impl Hub {
    fn zt(&mut self) -> Result<&mut Zt> {
        if self.zt.is_none() {
            self.zt = Some(crate::zt::start()?);
        }
        Ok(self.zt.as_mut().unwrap())
    }

    /// `Invoke-Zt` with reads (zt-calls.ps1's direct path): the answers that are JSON objects.
    fn reads(&mut self, calls: &[(&str, Value)]) -> Result<Vec<Value>> {
        if calls.is_empty() {
            return Ok(Vec::new());
        }
        read_calls(self.zt()?, calls)
    }

    fn read_one(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        Ok(self.reads(&[(tool, arguments)])?.into_iter().next().unwrap_or(Value::Null))
    }

    /// The active Lock of each given tree root, keyed by tree id (one `get_cards` per 200 roots).
    /// A root that does not exist carries no Lock: only a real tree can be Locked.
    fn locked_trees(&mut self, roots: &[String]) -> Result<Map<String, Value>> {
        let mut locked = Map::new();
        for chunk in roots.chunks(200) {
            let addresses: Vec<Value> = chunk.iter().map(|root| json!(root)).collect();
            let answer = self.read_one("get_cards", json!({ "addresses": addresses, "view": "text" }))?;
            for card in ps::each(&answer["cards"]) {
                if card["lock"].is_object() {
                    locked.insert(st(&card["tree"]), card["lock"].clone());
                }
            }
        }
        Ok(locked)
    }

    /// `Invoke-Write`: the calls through the helper in one batch; the address of each `N ok` line.
    /// In `-Step stage` every call is recorded instead, and its address is the call's own result
    /// reference (`@@N`), which the review gate resolves when it publishes that change.
    fn write(&mut self, calls: Vec<Value>) -> Result<Vec<String>> {
        if calls.is_empty() {
            return Ok(Vec::new());
        }
        if self.stage.is_some() {
            return self.stage_write(calls);
        }
        let (done, failure) = calls::run(self.zt()?, &calls, &mut calls::Options::new()).map_err(|e| anyhow!("zt helper failed: {e:#}"))?;
        let mut addresses = Vec::new();
        for d in &done {
            if let Some(c) = OK_LINE.captures(&d.line) {
                addresses.push(c[1].trim().to_string());
            }
        }
        if let Some(failure) = failure {
            // A Lock that appeared after the precheck stops the stage as itself (S16-AC05): the
            // affected write is not retried, and the trees already written keep their state.
            if let Some(refusal) = failure.refusal.clone().filter(|r| {
                r["error"] == json!("card_locked") || r["code"] == json!("card_locked")
            }) {
                return Err(anyhow::Error::new(crate::lockcheck::Stopped(refusal)));
            }
            let lines = failure.lines();
            if let Some(c) = ERROR_LINE.captures(&lines[0]) {
                bail!("zt: {}", &c[1]);
            }
            bail!("zt helper failed: {}", lines.join(" | "));
        }
        Ok(addresses)
    }

    /// Record one write batch for `-Step stage`, with the reviewed evidence of every call.
    fn stage_write(&mut self, calls: Vec<Value>) -> Result<Vec<String>> {
        let mut addresses = Vec::new();
        for mut call in calls {
            let tool = st(&call["tool"]);
            let creates = tool != "edit_card";
            let at = st(&call["arguments"]["at"]);
            let mut evidence = json!({
                "kind": if creates { "create" } else { "edit" },
                "at": at,
                "level": call["arguments"]["level"],
                "title": call["arguments"]["title"],
            });
            if !creates && !at.starts_with("@@") {
                let card = self.read_one("get_card", json!({ "at": at }))?;
                if card.is_null() {
                    bail!("the hub stage would edit `{at}`, which is not an agent Card");
                }
                // The identified write needs the reviewed authored base, and the review shows
                // exactly the body it replaces (S27-AC01, AC02).
                call["arguments"]["base_fingerprint"] = json!(crate::calls::authored_fingerprint(&card));
                evidence["old_body"] = card["body"].clone();
                evidence["old_fingerprint"] = json!(crate::calls::authored_fingerprint(&card));
                evidence["type"] = card["type"].clone();
                if ps::eq(&st(&card["type"]), "hub") {
                    let limits = self.read_limits()?;
                    let mut arguments = Self::limit_args(&limits)?;
                    arguments.insert("at".into(), json!(at));
                    let report = self.read_one("get_hub", Value::Object(arguments))?;
                    let anchor = ps::pipe(&report["faults"])
                        .into_iter()
                        .find(|fault| ps::eq(&st(&fault["fault"]), "anchor-title"))
                        .map(|fault| st(&fault["detail"]))
                        .unwrap_or_default();
                    evidence["policy"] = json!({
                        "mirror": report["mirror"],
                        "one_book": report["one_book"]["candidates"],
                        "faults": report["faults"],
                    });
                    evidence["anchor_title"] = json!(anchor);
                }
            }
            let address = self.stage.as_mut().map(|stage| stage.record(call, evidence)).unwrap_or_default();
            addresses.push(address);
        }
        Ok(addresses)
    }

    fn save_state(&self) -> Result<()> {
        if self.stage.is_some() {
            return Ok(());
        }
        save(&self.state_file, &ps::json_pretty(&self.state))
    }

    /// `Read-Limits`: limits.json, or the defaults.
    fn read_limits(&self) -> Result<Value> {
        let file = ps::join(&self.work, "limits.json");
        if ps::exists(&file) {
            return read_json(&file);
        }
        Ok(json!({ "topic_min": 7, "line_min": 12, "max_lines": 25, "max_groups": 7, "min_reason": 8, "neighbour_links": 3, "field_min": 3, "field_max": 12, "per_job": 5 }))
    }

    /// `Get-LimitArgs`: the limits get_hub and list_hubs take, as `[int]`.
    fn limit_args(limits: &Value) -> Result<Map<String, Value>> {
        let mut args = Map::new();
        for name in LIMIT_NAMES {
            args.insert(name.to_string(), json!(ps::int(&limits[*name])?));
        }
        Ok(args)
    }

    fn self_check(&self, file: &str) -> String {
        text::self_check(&self.work, file)
    }

    // --- the Card cache ------------------------------------------------------------------------

    /// `Get-Exists`: whether an agent Card exists. The first call reads every agent tree.
    fn exists(&mut self, address: &str) -> Result<bool> {
        if self.cards.is_none() {
            let listed = self.read_one("list_trees", json!({ "workspace": "agent" }))?;
            let calls: Vec<(&str, Value)> = ps::each(&listed["trees"]).into_iter().map(|t| ("get_tree", json!({ "tree": t["tree"] }))).collect();
            let trees = self.reads(&calls)?;
            let mut cards = HashMap::new();
            for t in &trees {
                for c in ps::each(&t["cards"]) {
                    cards.insert(ps::key(&st(&c["address"])), c.clone());
                }
            }
            self.cards = Some(cards);
        }
        Ok(!address.is_empty() && self.cards.as_ref().unwrap().contains_key(&ps::key(address)))
    }

    /// `Add-Known`: records a Card this tool just made (the cache loads first).
    fn add_known(&mut self, address: &str) -> Result<()> {
        self.exists(address)?;
        self.cards.as_mut().unwrap().insert(ps::key(address), json!({ "address": address }));
        Ok(())
    }

    fn card(&self, address: &str) -> Option<&Value> {
        self.cards.as_ref().and_then(|c| c.get(&ps::key(address)))
    }

    /// `tree:<n>` as every resolver reads it: its topic hub, else its root when it exists.
    fn resolve_tree(&mut self, t: &str, topic_hub: &CiMap<String>) -> Result<Option<String>> {
        if let Some(h) = topic_hub.get(t).filter(|h| !h.is_empty()) {
            return Ok(Some(h.clone()));
        }
        let root = format!("{t}/0");
        Ok(if self.exists(&root)? { Some(root) } else { None })
    }

    /// The topic hub of each tree: the state's, then the survey's (`$topicHub`).
    fn topic_hubs(&self, survey: &Value) -> CiMap<String> {
        let mut topic_hub = CiMap::new();
        if let Some(trees) = self.state["trees"].as_object() {
            for (tree, entry) in trees {
                if ps::truthy(&entry["topic"]) {
                    topic_hub.insert(tree, st(&entry["topic"]));
                }
            }
        }
        if let Some(trees) = survey["trees"].as_object() {
            for s in trees.values() {
                for h in ps::pipe(&s["hubs"]) {
                    if ps::eq(&st(&h["level"]), "topic") {
                        topic_hub.insert(&st(&s["tree"]), st(&h["address"]));
                    }
                }
            }
        }
        topic_hub
    }

    // --- survey --------------------------------------------------------------------------------

    fn step_survey(&mut self) -> Result<()> {
        let listed = self.read_one("list_trees", json!({ "workspace": "agent", "zone": "topic" }))?;
        let calls: Vec<(&str, Value)> = ps::each(&listed["trees"]).into_iter().map(|t| ("get_tree", json!({ "tree": t["tree"] }))).collect();
        let results = self.reads(&calls)?;
        let graph = self.read_one("list_hubs", json!({ "workspace": "agent" }))?;
        // Links between Topic trees, both ways, and the Cards of the other tree that each pair's
        // Links touch. A hub's own Links are not evidence that two subjects meet.
        // Keys are `<tree>><tree>` and Card addresses, so exact keys keep the hashtables' sets.
        let mut pair: Map<String, Value> = Map::new();
        let mut touched: HashMap<String, Map<String, Value>> = HashMap::new();
        let mut titles: HashMap<String, Value> = HashMap::new();
        for t in &results {
            for card in ps::each(&t["cards"]) {
                titles.insert(ps::key(&st(&card["address"])), card["title"].clone());
            }
        }
        for t in &results {
            for card in ps::each(&t["cards"]).into_iter().filter(|c| !ps::eq(&st(&c["type"]), "hub")) {
                let address = st(&card["address"]);
                let from = tree_id(&address);
                for target in ps::pipe(&card["links"]) {
                    let target = st(target);
                    let to = tree_id(&target);
                    if ps::eq(&to, &from) || !DIGITS.is_match(&to) || target.ends_with("/0") {
                        continue;
                    }
                    for key in [format!("{from}>{to}"), format!("{to}>{from}")] {
                        let n = pair.get(&key).and_then(Value::as_i64).unwrap_or(0);
                        pair.insert(key, json!(n + 1));
                    }
                    for (key, member) in [(format!("{from}>{to}"), target.clone()), (format!("{to}>{from}"), address.clone())] {
                        let inner = touched.entry(key).or_default();
                        let n = inner.get(&member).and_then(Value::as_i64).unwrap_or(0);
                        inner.insert(member, json!(n + 1));
                    }
                }
            }
        }
        let mut survey_trees = Map::new();
        let mut register = Value::Null;
        let (mut tree_sizes, mut branch_sizes, mut pair_counts) = (Vec::new(), Vec::new(), Vec::new());
        for t in &results {
            let tree = st(&t["tree"]);
            let all = ps::each(&t["cards"]);
            let cards: Vec<&Value> = all.iter().copied().filter(|c| !ps::eq(&st(&c["edge"]), "root")).collect();
            // A legacy hub: an Idea Card under <n>/1 (or next to it) with many Links, written before Hub Cards.
            let legacy: Vec<String> = cards
                .iter()
                .filter(|c| {
                    ps::eq(&st(&c["type"]), "idea")
                        && (ps::eq(&st(&c["parent"]), &format!("{tree}/1")) || ps::eq(&st(&c["address"]), &format!("{tree}/2")))
                        && ps::ps_count(&c["links"]) >= 8
                        && LEGACY_WORDS.is_match(&st(&c["excerpt"]))
                })
                .map(|c| st(&c["address"]))
                .collect();
            let content: Vec<&Value> =
                cards.iter().copied().filter(|c| !ps::eq(&st(&c["type"]), "hub") && !ps::is_in(&st(&c["address"]), &legacy)).collect();
            let hubs: Vec<Value> = cards
                .iter()
                .filter(|c| ps::eq(&st(&c["type"]), "hub"))
                .map(|c| json!({ "address": c["address"], "level": c["level"], "title": c["title"] }))
                .collect();
            if let Some(h) = hubs.iter().find(|h| ps::eq(&st(&h["level"]), "register")) {
                register = h["address"].clone();
                continue;
            }
            if content.is_empty() {
                continue;
            }
            let is_model = |c: &&&Value| ps::eq(&st(&c["type"]), "model") && !ps::truthy(&c["facet"]);
            let mut branches = Vec::new();
            for b in content.iter().filter(|c| ps::eq(&st(&c["type"]), "idea") && ps::eq(&st(&c["edge"]), "side")) {
                let anchor = st(&b["address"]);
                let prefix = format!("{anchor}|");
                let inside: Vec<&Value> =
                    content.iter().copied().filter(|c| ps::eq(&st(&c["address"]), &anchor) || ps::starts_with(&st(&c["address"]), &prefix)).collect();
                let models = inside.iter().filter(is_model).count();
                branch_sizes.push(inside.len() as i64);
                if inside.len() >= 3 || models >= 2 {
                    branches.push(json!({ "anchor": b["address"], "cards": inside.len(), "models": models, "title": b["title"] }));
                }
            }
            let mut neighbours: Vec<Value> = Vec::new();
            let head = format!("{tree}>");
            for key in pair.keys().filter(|k| ps::starts_with(k, &head)) {
                let other: String = key.chars().skip(tree.chars().count() + 1).collect();
                let mut there: Vec<(String, i64)> =
                    touched.get(key).map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(0))).collect()).unwrap_or_default();
                ps::ps_sort(&mut there, |a, b| b.1.cmp(&a.1));
                let cards_there: Vec<Value> = there
                    .iter()
                    .take(3)
                    .map(|(a, _)| json!({ "address": a, "title": titles.get(&ps::key(a)).cloned().unwrap_or(Value::Null) }))
                    .collect();
                neighbours.push(json!({ "tree": other, "links": pair[key], "cards": cards_there }));
            }
            ps::ps_sort(&mut neighbours, |a, b| b["links"].as_i64().cmp(&a["links"].as_i64()));
            neighbours.truncate(8);
            for n in &neighbours {
                let other = st(&n["tree"]);
                if DIGITS.is_match(&other) && other.parse::<i64>().unwrap_or(0) > tree.parse::<i64>().unwrap_or(0) {
                    pair_counts.push(n["links"].as_i64().unwrap_or(0));
                }
            }
            let root = all.iter().find(|c| ps::eq(&st(&c["edge"]), "root")).map(|c| c["excerpt"].clone()).unwrap_or(Value::Null);
            // Tree Lock S16 (#139): the root's read carries the tree's active Lock, so the plan and
            // the report name a Locked tree without another call.
            let lock = all
                .iter()
                .find(|c| ps::eq(&st(&c["edge"]), "root"))
                .map(|c| c["lock"].clone())
                .unwrap_or(Value::Null);
            tree_sizes.push(content.len() as i64);
            let listing = content.iter().map(|c| format!("{}\t{}", st(&c["address"]), st(&c["title"]))).collect::<Vec<_>>().join("\n");
            let models = content.iter().filter(is_model).count();
            survey_trees.insert(
                tree.clone(),
                json!({
                    "tree": tree, "title": t["title"], "excerpt": root, "cards": content.len(), "models": models,
                    "hubs": hubs, "legacy": legacy, "branches": branches, "neighbours": neighbours, "hash": text::hash16(&listing),
                    "lock": lock,
                }),
            );
        }
        let survey = json!({ "trees": survey_trees, "register": register });
        let survey_file = ps::join(&self.work, "survey.json");
        save(&survey_file, &ps::json_pretty(&survey))?;
        let mut md = format!("# 代理工作区的主题（每行一个主题）{NL}{NL}");
        for s in survey_trees_of(&survey) {
            let near = ps::each(&s["neighbours"]).iter().take(4).map(|n| format!("{}({})", st(&n["tree"]), st(&n["links"]))).collect::<Vec<_>>().join(" ");
            let locked = if s["lock"].is_object() { "（已锁定，只读：可作成员与链接目标，不能写入）" } else { "" };
            md.push_str(&format!(
                "- {} {}{locked}（{} 张，模型 {}）：{} | 邻近：{near}{NL}",
                st(&s["tree"]),
                st(&s["title"]),
                st(&s["cards"]),
                st(&s["models"]),
                st(&s["excerpt"])
            ));
        }
        save(&ps::join(&self.work, "survey.md"), &md)?;
        let legacy: usize = survey_trees_of(&survey).iter().map(|s| ps::each(&s["legacy"]).into_iter().filter(|x| ps::truthy(x)).count()).sum();
        let locked = survey_trees_of(&survey).iter().filter(|s| s["lock"].is_object()).count();
        util::print_line(&ps::json_compact(&json!({
            "topics": survey["trees"].as_object().map_or(0, Map::len),
            "tree_cards": text::quantiles(&tree_sizes),
            "branch_cards": text::quantiles(&branch_sizes),
            "neighbour_links": text::quantiles(&pair_counts),
            "hubs": graph["counts"],
            "legacy": legacy,
            "locked": locked,
            "register": survey["register"],
            "survey": survey_file,
        })))
    }

    // --- plan ----------------------------------------------------------------------------------

    fn step_plan(&mut self) -> Result<()> {
        let given = if self.p.limits.is_empty() { json!({}) } else { read_json_text(&self.p.limits)? };
        let per_job = if self.p.per_job != 0 { json!(self.p.per_job) } else { given["per_job"].clone() };
        let limits = json!({
            "topic_min": clamped(&given["topic_min"], 7, 4, 30)?,
            "line_min": clamped(&given["line_min"], 12, 5, 40)?,
            "max_lines": clamped(&given["max_lines"], 25, 12, 30)?,
            "max_groups": clamped(&given["max_groups"], 7, 3, 9)?,
            "min_reason": clamped(&given["min_reason"], 8, 4, 30)?,
            "neighbour_links": clamped(&given["neighbour_links"], 3, 2, 20)?,
            "field_min": clamped(&given["field_min"], 3, 2, 8)?,
            "field_max": clamped(&given["field_max"], 12, 4, 25)?,
            "per_job": clamped(&per_job, 5, 1, 12)?,
        });
        save(&ps::join(&self.work, "limits.json"), &ps::json_pretty(&limits))?;
        // A new plan starts clean: the jobs and DeepSeek files of an earlier run are not applied again.
        self.state["jobs"] = json!({});
        for name in ps::child_files(&self.out, "", "") {
            let _ = std::fs::remove_file(ps::join(&self.out, &name));
        }
        for (prefix, suffix) in [("job-", ".md"), ("fix-", ".md")] {
            for name in ps::child_files(&self.work, prefix, suffix) {
                let _ = std::fs::remove_file(ps::join(&self.work, &name));
            }
        }
        let survey = read_json(&ps::join(&self.work, "survey.json"))?;
        let mut arguments = Self::limit_args(&limits)?;
        arguments.insert("workspace".into(), json!("agent"));
        let graph = self.read_one("list_hubs", Value::Object(arguments))?;
        let mut faulty = HashSet::new();
        for h in ps::pipe(&graph["hubs"]) {
            if ps::ps_count(&h["faults"]) > 0 || ps::truthy(&h["mirror"]) {
                faulty.insert(ps::key(&st(&h["tree"])));
            }
        }
        let mut missing = HashSet::new();
        for m in ps::pipe(&graph["missing"]) {
            if ps::truthy(&m["tree"]) {
                missing.insert(ps::key(&st(&m["tree"])));
            }
        }
        // A tree whose Topic body carries an accepted `## 入口` section needs no topic hub (§8.2,
        // D168): the plan does not send it to a writer again.
        let mut satisfied = HashSet::new();
        for entry in ps::pipe(&graph["entries"]) {
            if ps::truthy(&entry["valid"])
                && !ps::pipe(&graph["hubs"]).into_iter().any(|h| {
                    ps::eq(&st(&h["tree"]), &st(&entry["tree"])) && ps::eq(&st(&h["level"]), "topic")
                })
            {
                satisfied.insert(ps::key(&st(&entry["tree"])));
            }
        }
        let only: Vec<String> = self.p.trees.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
        let topic_min = limits["topic_min"].as_i64().unwrap_or(0);
        let line_min = limits["line_min"].as_i64().unwrap_or(0);
        let required = |b: &&Value| ge(&b["cards"], line_min) || ge(&b["models"], 2);
        let mut picked: Vec<Value> = Vec::new();
        let mut locked: Vec<String> = Vec::new();
        for s in survey_trees_of(&survey) {
            let tree = st(&s["tree"]);
            if !only.is_empty() && !ps::is_in(&tree, &only) {
                continue;
            }
            // A Locked tree takes no hub write (R331): the plan does not send it to a writer, and
            // names it instead of silently dropping it. Its Cards stay readable and linkable.
            if s["lock"].is_object() {
                locked.push(tree);
                continue;
            }
            let needs = (ge(&s["cards"], topic_min) || ps::each(&s["branches"]).iter().any(required))
                && !satisfied.contains(&ps::key(&tree));
            if !needs {
                continue;
            }
            let known = self.state["trees"].as_object().and_then(|m| m.get(&tree));
            let changed = match known {
                None => true,
                Some(entry) => !ps::eq(&st(&entry["hash"]), &st(&s["hash"])),
            };
            if self.p.rebuild
                || !only.is_empty()
                || missing.contains(&ps::key(&tree))
                || faulty.contains(&ps::key(&tree))
                || ps::ps_count(&s["legacy"]) > 0
                || changed
            {
                picked.push(s.clone());
            }
        }
        // The cards of each picked tree, for its job file.
        let calls: Vec<(&str, Value)> = picked.iter().map(|s| ("get_tree", json!({ "tree": st(&s["tree"]) }))).collect();
        let mut tree_data: HashMap<String, Value> = HashMap::new();
        for t in self.reads(&calls)? {
            tree_data.insert(ps::key(&st(&t["tree"])), t);
        }
        let limits_line = ps::json_compact(&limits);
        let per_job = limits["per_job"].as_i64().unwrap_or(5).max(1) as usize;
        let neighbour_links = limits["neighbour_links"].as_i64().unwrap_or(0);
        let mut jobs = Vec::new();
        let mut i = 0;
        while i < picked.len() {
            let chunk = &picked[i..picked.len().min(i + per_job)];
            let seq = self.next_seq()?;
            let id = format!("t{}", ps::d2(seq));
            let file = ps::join(&self.work, &format!("job-{id}.md"));
            let mut t = String::new();
            let mut line = |s: &str| {
                t.push_str(s);
                t.push_str(NL);
            };
            line(&format!("# 任务 {id}：主题枢纽与线索枢纽"));
            line("");
            line("HELPER: zt-digest call");
            line(&format!("ALLOW: {}", ps::join(&self.work, "none.allow.txt")));
            line(&format!("OUT: {}", self.out));
            line(&format!("LIMITS: {limits_line}"));
            line("");
            for s in chunk {
                let tree = st(&s["tree"]);
                let data = tree_data.get(&ps::key(&tree)).cloned().unwrap_or(Value::Null);
                let hub_file = ps::join(&self.out, &format!("hub-{tree}.json"));
                line(&format!("## 主题 {tree} {}", st(&s["title"])));
                line("");
                line(&format!("输出文件：{hub_file}"));
                line(&text::self_check(&self.work, &hub_file));
                line(&format!("Topic 卡 [[{tree}/0]]：{}", st(&s["excerpt"])));
                line("");
                let legacy: Vec<String> = ps::pipe(&s["legacy"]).into_iter().map(st).collect();
                let mut existing: Vec<String> = ps::each(&s["hubs"])
                    .into_iter()
                    .filter(|h| ps::eq(&st(&h["level"]), "topic"))
                    .map(|h| st(&h["address"]))
                    .collect();
                existing.extend(legacy.iter().cloned());
                if !existing.is_empty() {
                    line(&format!("现有主题枢纽（改写它，不要另写）：{}", existing.iter().map(|a| format!("[[{a}]]")).collect::<Vec<_>>().join("、")));
                }
                let required: Vec<&Value> = ps::each(&s["branches"]).into_iter().filter(|b| required(b)).collect();
                if !required.is_empty() {
                    line("必须写线索枢纽的分支：");
                    for b in required {
                        line(&format!("- [[{}]] {}（{} 张，模型 {}）", st(&b["anchor"]), st(&b["title"]), st(&b["cards"]), st(&b["models"])));
                    }
                } else {
                    line("如果这张主题枢纽只会按地址顺序抄本树的图，不要写它：在 topic 里写 entries（一到三项，每项 ref 和 reason），脚本把它们写进主题卡的“## 入口”一节，这棵树就不再要求主题枢纽。预检按 mirror 拒绝抄地址顺序的稿子。");
                }
                line("只讲一本书的枢纽会被报告为 one-book：同一个概念在别的书里也有讲法时，在 neighbours 里写出那张卡和关系。");
                line("");
                line("本树卡片（地址 | 类型 | 标题 | 摘要）：");
                for c in ps::each(&data["cards"]) {
                    let (ty, edge, address) = (st(&c["type"]), st(&c["edge"]), st(&c["address"]));
                    if ps::eq(&edge, "root") || ps::eq(&ty, "hub") || ps::is_in(&address, &legacy) {
                        continue;
                    }
                    let kind = if ps::eq(&ty, "model") && ps::truthy(&c["facet"]) {
                        format!("模型部件·{}", st(&c["facet"]))
                    } else if ps::eq(&ty, "model") {
                        "模型卡".to_string()
                    } else if ps::eq(&ty, "event") {
                        "事件卡".to_string()
                    } else {
                        "观点卡".to_string()
                    };
                    let mut excerpt = st(&c["excerpt"]);
                    if ps::len16(&excerpt) > 90 {
                        excerpt = ps::cut16(&excerpt, 90) + "…";
                    }
                    line(&format!("- {address} | {kind} | {} | {excerpt}", st(&c["title"])));
                }
                if ps::ps_count(&s["neighbours"]) > 0 {
                    line("");
                    line("邻近主题（链接数）：");
                    for n in ps::pipe(&s["neighbours"]).into_iter().filter(|n| ge(&n["links"], neighbour_links)).take(5) {
                        let other = st(&n["tree"]);
                        let title = survey["trees"].as_object().and_then(|m| m.get(&other)).map(|x| st(&x["title"])).unwrap_or_default();
                        let cards =
                            ps::pipe(&n["cards"]).into_iter().map(|c| format!("[[{}]] {}", st(&c["address"]), st(&c["title"]))).collect::<Vec<_>>().join("；");
                        line(&format!("- {other} {title}（{} 条）：{cards}", st(&n["links"])));
                    }
                }
                line("");
            }
            save(&file, &t)?;
            let trees: Vec<String> = chunk.iter().map(|s| st(&s["tree"])).collect();
            self.state["jobs"][&id] = json!({ "kind": "topic", "trees": trees, "file": file });
            jobs.push(json!({ "id": id, "files": [self.doc, file], "prompt": TOPIC_PROMPT }));
            i += per_job;
        }
        save(&ps::join(&self.work, "none.allow.txt"), "\n")?;
        // One field job: every Topic of the Workspace, on one line each.
        if only.is_empty() || self.p.rebuild {
            let seq = self.next_seq()?;
            let id = format!("f{}", ps::d2(seq));
            let file = ps::join(&self.work, &format!("job-{id}.md"));
            let fields = obj(&self.state["fields"]);
            let fields_now = if fields.is_empty() {
                String::new()
            } else {
                format!("现有领域枢纽（保留它们的名字，可以增删成员）：{}\n", fields.keys().cloned().collect::<Vec<_>>().join("、"))
            };
            let fields_file = ps::join(&self.out, "fields.json");
            save(
                &file,
                &format!(
                    "# 任务 {id}：领域枢纽与索引\n\nOUT: {fields_file}\n{}\nLIMITS: {limits_line}\n{fields_now}\n主题清单：{}\n",
                    self.self_check(&fields_file),
                    ps::join(&self.work, "survey.md")
                ),
            )?;
            self.state["jobs"][&id] = json!({ "kind": "field", "file": file });
            jobs.push(json!({ "id": id, "files": [self.doc, file], "prompt": FIELD_PROMPT }));
        }
        self.save_state()?;
        let trees: Vec<String> = picked.iter().map(|s| st(&s["tree"])).collect();
        util::print_line(&ps::json_compact(&json!({
            "tree_count": picked.len(), "jobs": jobs, "trees": trees, "limits": limits,
            "locked": locked
        })))
    }

    /// `$State.seq++`.
    fn next_seq(&mut self) -> Result<i64> {
        let seq = ps::int(&self.state["seq"])? + 1;
        self.state["seq"] = json!(seq);
        Ok(seq)
    }

    // --- apply ---------------------------------------------------------------------------------

    /// The resolver of a tree's hub file: `line:<anchor>`, `tree:<n>`, or an address.
    fn resolve_apply(&mut self, r: &str, lines: &Map<String, Value>, topic_hub: &CiMap<String>) -> Result<Option<String>> {
        if let Some(c) = LINE_REF.captures(r) {
            return Ok(lines.get(&c[1]).map(st).filter(|a| !a.is_empty()));
        }
        if let Some(c) = TREE_REF.captures(r) {
            return self.resolve_tree(&c[1], topic_hub);
        }
        Ok(if self.exists(r)? { Some(r.to_string()) } else { None })
    }

    /// The resolver of fields.json: `tree:<n>`, `field:<name>`, or an address.
    fn resolve_field(&mut self, r: &str, topic_hub: &CiMap<String>) -> Result<Option<String>> {
        if let Some(c) = TREE_REF.captures(r) {
            return self.resolve_tree(&c[1], topic_hub);
        }
        if let Some(c) = FIELD_REF.captures(r) {
            return Ok(self.state["fields"].as_object().and_then(|m| m.get(&c[1])).map(st).filter(|a| !a.is_empty()));
        }
        Ok(if self.exists(r)? { Some(r.to_string()) } else { None })
    }

    fn step_apply(&mut self) -> Result<i32> {
        match self.apply_run() {
            Ok(summary) => {
                util::print_line(&ps::json_compact(&summary))?;
                Ok(0)
            }
            // A Lock that appeared after the precheck: the archive's own refusal is the last line,
            // the trees already written keep their receipts, and the stage exits with its code.
            Err(error) => match crate::lockcheck::stopped_refusal(&error) {
                Some(refusal) => crate::lockcheck::stop("hub", &refusal),
                None => Err(error),
            },
        }
    }

    // --- stage: the same changes, without a write (S27, #119) ---------------------------------

    /// Run the apply logic with every write recorded instead of sent, then publish the exact plan
    /// under `<Work>\hub-plan.json`. A created Card has no address yet, so the call list names the
    /// N-th call's own result (`@@N`) and the review gate resolves it when it publishes that item
    /// (`publish.rs`). Each changed existing Card carries the authored body and fingerprint it
    /// replaces, so the review shows exactly what it would overwrite (AC01).
    fn step_stage(&mut self) -> Result<()> {
        self.stage = Some(Stage::default());
        let summary = self.apply_run()?;
        let stage = self.stage.take().unwrap_or_default();
        let items = stage_items(&stage.calls, &stage.evidence)?;
        let view = self.zt()?.archive_view()?;
        let limits = self.read_limits()?;
        let mut plan = json!({
            "schema_version": 1,
            "contract": "hub-plan/1",
            "plan_id": "",
            "created_at": canon::utc_now_o(),
            "archive": {"identity": view["archive_identity"], "revision": view["archive_revision"]},
            "work": self.work,
            "limits": limits,
            "calls": stage.calls,
            "items": items,
            "state": self.state,
            "dropped": summary["dropped"],
            "trees": summary["trees"],
            "fields": summary["fields"],
            "register": summary["register"],
        });
        plan["plan_id"] = json!(format!("hp-{}", canon::content_hash(&plan, &["plan_id", "created_at"])));
        let path = ps::join(&self.work, "hub-plan.json");
        canon::save_json(&path, &plan)?;
        util::print_line(&ps::json_compact(&json!({
            "ok": true,
            "step": "stage",
            "plan_id": plan["plan_id"],
            "writes": "none",
            "calls": plan["calls"].as_array().map_or(0, Vec::len),
            "items": plan["items"].as_array().map_or(0, Vec::len),
            "created": plan["items"].as_array().map_or(0, |items| items.iter().filter(|item| item["kind"] == json!("create")).count()),
            "edited": plan["items"].as_array().map_or(0, |items| items.iter().filter(|item| item["kind"] == json!("edit")).count()),
            "trees": plan["trees"],
            "fields": plan["fields"],
            "register": plan["register"],
            "dropped": plan["dropped"],
            "path": path,
        })))
    }

    fn apply_run(&mut self) -> Result<Value> {
        let survey = read_json(&ps::join(&self.work, "survey.json"))?;
        let (mut created, mut updated) = (0i64, 0i64);
        let mut dropped: Vec<String> = Vec::new();
        let mut topic_hub = self.topic_hubs(&survey);
        let files: Vec<String> = ps::child_files(&self.out, "hub-", ".json").into_iter().filter(|n| HUB_FILE.is_match(n)).collect();
        // Tree Lock S16 (#139): only a tree this plan would write must be unlocked (R331). The
        // precheck reads the Lock of every planned tree root once; a Locked tree is left entirely
        // alone and named as blocked, and every other tree keeps its hub work.
        let mut planned: Vec<String> = files
            .iter()
            .filter_map(|name| name.strip_prefix("hub-").and_then(|n| n.strip_suffix(".json")).map(str::to_string))
            .collect();
        let register_now = st(&self.state["register"]);
        if !register_now.is_empty() {
            planned.push(crate::lockcheck::tree_id(&register_now));
        }
        planned.sort();
        planned.dedup();
        let roots: Vec<String> = planned.iter().map(|tree| format!("{tree}/0")).collect();
        let locked = self.locked_trees(&roots)?;
        let mut blocked: Vec<Value> = Vec::new();
        for name in &files {
            let plan = match read_json(&ps::join(&self.out, name)) {
                Ok(plan) => plan,
                Err(_) => {
                    dropped.push(format!("{name}: not JSON"));
                    continue;
                }
            };
            let tree = st(&plan["tree"]);
            let Some(s) = survey["trees"].as_object().and_then(|m| m.get(&tree)).cloned() else {
                dropped.push(format!("{name}: unknown tree {tree}"));
                continue;
            };
            if let Some(lock) = locked.get(&tree) {
                let known = self.state["trees"].as_object().and_then(|m| m.get(&tree)).is_some_and(|e| ps::truthy(&e["topic"]));
                blocked.push(blocked_tree(&tree, if known { "edit" } else { "create-inside" }, "hubs", lock));
                continue;
            }
            let mut entry = match self.state["trees"].as_object().and_then(|m| m.get(&tree)) {
                Some(e) => e.clone(),
                None => json!({ "topic": null, "lines": {}, "hash": "" }),
            };
            if !ps::truthy(&entry["lines"]) {
                entry["lines"] = json!({});
            }
            // 1. Line hubs: one Side successor of each anchor.
            let mut creates = Vec::new();
            let mut pending = Vec::new();
            for line in ps::pipe(&plan["lines"]) {
                let anchor = st(&line["anchor"]);
                if !self.exists(&anchor)? {
                    dropped.push(format!("line:{anchor}"));
                    continue;
                }
                if !entry["lines"].as_object().is_some_and(|m| m.contains_key(&anchor)) {
                    creates.push(json!({ "tool": "create_successor", "arguments": {
                        "at": anchor, "kind": "side", "type": "hub", "level": "line", "title": st(&line["title"]), "body": PENDING_BODY } }));
                    pending.push(anchor);
                }
            }
            let made = self.write(creates)?;
            for (i, anchor) in pending.iter().enumerate() {
                let address = made.get(i).cloned().unwrap_or_default();
                entry["lines"][anchor] = json!(address);
                self.add_known(&address)?;
                created += 1;
            }
            // 2. The topic hub: rewrite the existing or legacy one, or create it under <n>/1. A
            // tree whose draft names one to three entry Cards instead writes the Topic body's
            // `## 入口` section and gets no topic hub (§8.2, D167, D168).
            let entry_only = ps::pipe(&plan["topic"]["entries"])
                .iter()
                .any(|item| ps::truthy(item))
                && !ps::pipe(&plan["lines"]).iter().any(|line| ps::truthy(line))
                && !ps::pipe(&plan["topic"]["groups"]).iter().any(|group| ps::truthy(group));
            let state_topic = st(&entry["topic"]);
            let existing_topic = (ps::truthy(&entry["topic"]) && self.exists(&state_topic)?)
                .then(|| state_topic.clone())
                .or_else(|| topic_hub.get(&tree).filter(|h| !h.is_empty()).cloned())
                .or_else(|| {
                    ps::pipe(&s["legacy"])
                        .first()
                        .map(|legacy| st(legacy))
                        .filter(|legacy| !legacy.is_empty())
                });
            if entry_only {
                match existing_topic.as_ref() {
                    // An existing mirror hub is reported, never deleted (D167): the entry section
                    // is not written over it until a separate change removes the hub.
                    Some(hub) => dropped.push(format!(
                        "entries (主题 {tree} 已经有主题枢纽 [[{hub}]]；删掉旧枢纽要另外授权，这次不改它)"
                    )),
                    None => {
                        let root = self.read_one("get_card", json!({ "at": format!("{tree}/0") }))?;
                        let section = text::entries_body(
                            &plan["topic"]["entries"],
                            &mut |r: &str| Ok(Some(r.to_string())),
                        )?;
                        if section.is_empty() {
                            dropped.push(format!("{name}: entries 都解析不出来"));
                        } else {
                            let body = text::replace_entries(&st(&root["body"]), &section);
                            self.write(vec![json!({ "tool": "edit_card", "arguments": {
                                "at": format!("{tree}/0"), "body": body } })])?;
                            entry["topic"] = Value::Null;
                            entry["entries"] = json!(ps::pipe(&plan["topic"]["entries"])
                                .iter()
                                .map(|item| st(&item["ref"]))
                                .collect::<Vec<_>>());
                            entry["lines"] = json!({});
                            entry["hash"] = s["hash"].clone();
                            self.state["trees"][&tree] = entry;
                            self.save_state()?;
                            updated += 1;
                            continue;
                        }
                    }
                }
            }
            let topic = if let Some(hub) = existing_topic {
                hub
            } else {
                let made = self.write(vec![json!({ "tool": "create_successor", "arguments": {
                    "at": format!("{tree}/1"), "kind": "side", "type": "hub", "level": "topic", "title": st(&plan["topic"]["title"]), "body": PENDING_BODY } })])?;
                let topic = made.into_iter().next().unwrap_or_default();
                self.add_known(&topic)?;
                created += 1;
                topic
            };
            entry["topic"] = json!(topic);
            topic_hub.insert(&tree, topic.clone());
            let lines = obj(&entry["lines"]);
            let hubs_now = topic_hub.clone();
            let mut edits = Vec::new();
            for line in ps::pipe(&plan["lines"]) {
                let anchor = st(&line["anchor"]);
                let Some(at) = lines.get(&anchor) else { continue };
                let mut d = Vec::new();
                let body = text::format_body(
                    &st(&line["scope"]),
                    &line["groups"],
                    &line["neighbours"],
                    &mut |r: &str| self.resolve_apply(r, &lines, &hubs_now),
                    &mut d,
                )?;
                dropped.extend(d);
                edits.push(json!({ "tool": "edit_card", "arguments": { "at": at, "title": st(&line["title"]), "body": body, "type": "hub", "level": "line" } }));
            }
            let topic_plan = &plan["topic"];
            let mut d = Vec::new();
            let mut body = text::format_body(
                &st(&topic_plan["scope"]),
                &topic_plan["groups"],
                &topic_plan["neighbours"],
                &mut |r: &str| self.resolve_apply(r, &lines, &hubs_now),
                &mut d,
            )?;
            dropped.extend(d);
            // Every line hub of the tree is reached from the topic hub (H9), directly or through the
            // line hub of an outer branch; the tool adds one group for those the writer left out.
            let line_bodies = edits.iter().map(|e| st(&e["arguments"]["body"])).collect::<Vec<_>>().join("\n");
            let mut left = Vec::new();
            for (anchor, hub) in &lines {
                let link = Regex::new(&format!("(?i){}", regex::escape(&format!("[[{}]]", st(hub)))))?;
                if !link.is_match(&body) && !link.is_match(&line_bodies) {
                    left.push(format!("- [[{}]]（枢纽）：从 [[{anchor}]] 起的一支。", st(hub)));
                }
            }
            if !left.is_empty() {
                body.push_str(&format!("\n\n## 线索\n{}", left.join("\n")));
            }
            edits.push(json!({ "tool": "edit_card", "arguments": { "at": topic, "title": st(&topic_plan["title"]), "body": body, "type": "hub", "level": "topic" } }));
            updated += edits.len() as i64;
            self.write(edits)?;
            // 3. The Topic body links its topic hub (H7).
            let root = self.read_one("get_card", json!({ "at": format!("{tree}/0") }))?;
            let root_body = st(&root["body"]);
            let link = Regex::new(&format!("(?i){}", regex::escape(&format!("[[{topic}]]"))))?;
            if !link.is_match(&root_body) {
                self.write(vec![json!({ "tool": "edit_card", "arguments": {
                    "at": format!("{tree}/0"), "body": format!("{}\n\n本主题的枢纽卡：[[{topic}]]。", root_body.trim_end()) } })])?;
            }
            entry["hash"] = s["hash"].clone();
            self.state["trees"][&tree] = entry;
            self.save_state()?;
        }
        // 4. Fields and the register, in the Topic 索引.
        let fields_file = ps::join(&self.out, "fields.json");
        let state_register = st(&self.state["register"]);
        let mut register = if ps::truthy(&self.state["register"]) && self.exists(&state_register)? {
            json!(state_register)
        } else if ps::truthy(&survey["register"]) {
            survey["register"].clone()
        } else {
            Value::Null
        };
        if !ps::truthy(&register) && (!files.is_empty() || ps::exists(&fields_file)) {
            let index = self.write(vec![json!({ "tool": "create_topic", "arguments": { "title": "索引", "body": "本主题收放代理工作区的入口：索引卡与领域枢纽。" } })])?;
            let index = index.into_iter().next().unwrap_or_default();
            let made = self.write(vec![json!({ "tool": "create_successor", "arguments": {
                "at": index, "type": "hub", "level": "register", "title": "索引：代理工作区的入口", "body": PENDING_BODY } })])?;
            let address = made.into_iter().next().unwrap_or_default();
            self.add_known(&address)?;
            register = json!(address);
            created += 2;
        }
        self.state["register"] = register.clone();
        // A Locked register tree takes no field or register write either: the field hubs are its
        // Side successors, so both wait for the owner's unlock and are named as blocked.
        let register_tree = if ps::truthy(&register) { crate::lockcheck::tree_id(&st(&register)) } else { String::new() };
        let register_lock = locked.get(&register_tree).cloned();
        if register_lock.is_none() {
        let fields = if ps::exists(&fields_file) {
            match read_json(&fields_file) {
                Ok(v) => v,
                Err(_) => {
                    dropped.push("fields.json: not JSON".into());
                    json!({})
                }
            }
        } else {
            json!({})
        };
        let field_list: Vec<Value> = ps::pipe(&fields["fields"]).into_iter().filter(|f| ps::truthy(f)).cloned().collect();
        let mut in_field: HashSet<String> = HashSet::new();
        for field in &field_list {
            let name = st(&field["name"]);
            let mut members = Vec::new();
            for g in ps::pipe(&field["groups"]) {
                for m in ps::pipe(&g["items"]) {
                    let t = st(&m["tree"]);
                    if topic_hub.get(&t).is_some_and(|h| !h.is_empty()) || self.exists(&format!("{t}/0"))? {
                        members.push(t);
                    }
                }
            }
            if members.len() < 2 {
                dropped.push(format!("field:{name} (fewer than 2 Topics)"));
                continue;
            }
            let current = self.state["fields"].as_object().and_then(|m| m.get(&name)).map(st);
            let keep = match current {
                Some(address) => self.exists(&address)?,
                None => false,
            };
            if !keep {
                let made = self.write(vec![json!({ "tool": "create_successor", "arguments": {
                    "at": register, "kind": "side", "type": "hub", "level": "field", "title": st(&field["title"]), "body": PENDING_BODY } })])?;
                let address = made.into_iter().next().unwrap_or_default();
                self.state["fields"][&name] = json!(address);
                self.add_known(&address)?;
                created += 1;
            }
            for m in members {
                in_field.insert(ps::key(&m));
            }
        }
        let mut edits = Vec::new();
        for field in &field_list {
            let name = st(&field["name"]);
            let Some(at) = self.state["fields"].as_object().and_then(|m| m.get(&name)).cloned() else { continue };
            let groups: Vec<Value> = ps::pipe(&field["groups"])
                .into_iter()
                .map(|g| {
                    let items: Vec<Value> =
                        ps::pipe(&g["items"]).into_iter().map(|i| json!({ "ref": format!("tree:{}", st(&i["tree"])), "reason": i["reason"] })).collect();
                    json!({ "name": g["name"], "items": items })
                })
                .collect();
            let mut d = Vec::new();
            let body = text::format_body(&st(&field["scope"]), &Value::Array(groups), &json!([]), &mut |r: &str| self.resolve_field(r, &topic_hub), &mut d)?;
            dropped.extend(d);
            edits.push(json!({ "tool": "edit_card", "arguments": { "at": at, "title": st(&field["title"]), "body": body } }));
        }
        if ps::truthy(&register) {
            let mut out = format!("范围：代理工作区的全部入口。每个关键词指向一到三个枢纽。{NL}{NL}");
            let state_fields = obj(&self.state["fields"]);
            if !state_fields.is_empty() {
                out.push_str(&format!("## 领域{NL}"));
                for (name, address) in &state_fields {
                    out.push_str(&format!("- {name}：[[{}]]{NL}", st(address)));
                }
                out.push_str(NL);
            }
            let mut terms = Vec::new();
            for t in ps::pipe(&fields["register"]).into_iter().filter(|t| ps::truthy(t)) {
                let mut targets = Vec::new();
                for r in ps::pipe(&t["refs"]) {
                    if let Some(address) = self.resolve_field(&st(r), &topic_hub)? {
                        targets.push(address);
                    }
                }
                let targets: Vec<String> = ps::select_unique(targets).into_iter().take(3).collect();
                if !targets.is_empty() {
                    terms.push(format!("- {}：{}", st(&t["term"]), targets.iter().map(|a| format!("[[{a}]]")).collect::<Vec<_>>().join("、")));
                }
            }
            if !terms.is_empty() {
                out.push_str(&format!("## 关键词{NL}"));
                for l in &terms {
                    out.push_str(l);
                    out.push_str(NL);
                }
                out.push_str(NL);
            }
            let mut loose: Vec<String> = topic_hub.keys().into_iter().filter(|t| !in_field.contains(&ps::key(t))).collect();
            ps::ps_sort(&mut loose, |a, b| a.trim().parse::<i64>().unwrap_or(0).cmp(&b.trim().parse::<i64>().unwrap_or(0)));
            if !loose.is_empty() {
                out.push_str(&format!("## 未归入领域的主题{NL}"));
                for t in &loose {
                    let title = survey["trees"].as_object().and_then(|m| m.get(t)).map(|x| st(&x["title"])).unwrap_or_default();
                    out.push_str(&format!("- {title}：[[{}]]{NL}", topic_hub.get(t).cloned().unwrap_or_default()));
                }
            }
            edits.push(json!({ "tool": "edit_card", "arguments": { "at": register, "body": out.trim_end() } }));
        }
        let count = edits.len() as i64;
        self.write(edits)?;
        updated += count;
        } else if let Some(lock) = &register_lock {
            blocked.push(blocked_tree(&register_tree, "edit", "fields", lock));
        }
        self.save_state()?;
        let dropped: Vec<String> = dropped.into_iter().take(40).collect();
        let complete = blocked.is_empty();
        Ok(json!({
            "created": created, "updated": updated,
            "trees": self.state["trees"].as_object().map_or(0, Map::len),
            "fields": self.state["fields"].as_object().map_or(0, Map::len),
            "register": register, "dropped": dropped,
            "blocked": blocked, "complete": complete,
        }))
    }

    // --- check ---------------------------------------------------------------------------------

    fn step_check(&mut self) -> Result<()> {
        let limits = self.read_limits()?;
        let mut hubs: Vec<(Value, String)> = Vec::new();
        for (tree, e) in obj(&self.state["trees"]) {
            if ps::truthy(&e["topic"]) {
                hubs.push((e["topic"].clone(), tree.clone()));
            }
            for a in obj(&e["lines"]).values() {
                hubs.push((a.clone(), tree.clone()));
            }
        }
        for a in obj(&self.state["fields"]).values() {
            hubs.push((a.clone(), "field".into()));
        }
        if ps::truthy(&self.state["register"]) {
            hubs.push((self.state["register"].clone(), "field".into()));
        }
        let limit_args = Self::limit_args(&limits)?;
        let calls: Vec<(&str, Value)> = hubs
            .iter()
            .map(|(address, _)| {
                let mut a = limit_args.clone();
                a.insert("at".into(), address.clone());
                ("get_hub", Value::Object(a))
            })
            .collect();
        let reports = self.reads(&calls)?;
        let mut by_tree: CiMap<Vec<String>> = CiMap::new();
        let mut one_book: Vec<Value> = Vec::new();
        let mut entry_faults: Vec<String> = Vec::new();
        let mut total = 0;
        for (i, (address, tree)) in hubs.iter().enumerate() {
            let report = reports.get(i).cloned().unwrap_or(Value::Null);
            // A missing upper hub is the field job's or the register's work, not the writer's.
            let mut faults: Vec<String> = ps::pipe(&report["faults"])
                .into_iter()
                .filter(|f| {
                    !(ps::eq(&st(&f["fault"]), "unanchored") && ps::eq(&st(&report["level"]), "topic") && !TOPIC_BODY.is_match(&st(&f["detail"])))
                })
                .map(|f| format!("{}：{}", st(&f["fault"]), st(&f["detail"])))
                .collect();
            if ps::truthy(&report["mirror"]) {
                faults.push("mirror：只按地址顺序抄了本树，没有按主题分组，也没有树外的卡".into());
            }
            // `one-book` is advice, not a fault (§8.2, D166): it is reported for the hub manager,
            // and no fix job demands a relation the writer cannot state.
            for candidate in ps::pipe(&report["one_book"]["candidates"]) {
                one_book.push(json!({
                    "tree": tree,
                    "hub": address,
                    "candidate": candidate["address"],
                    "source": candidate["source"],
                    "term": candidate["term"],
                    "member": candidate["member"],
                }));
            }
            if faults.is_empty() {
                continue;
            }
            total += faults.len();
            let mut list = by_tree.get(tree).cloned().unwrap_or_default();
            list.push(format!("- [[{}]]（{}）：{}", st(address), st(&report["level"]), faults.join("；")));
            by_tree.insert(tree, list);
        }
        // A tree that took the entry-section alternative has no hub to check; its section is
        // checked through `list_hubs`, and a broken one becomes the writer's fix job (§8.2, D168).
        if !obj(&self.state["trees"]).values().all(|e| !ps::truthy(&e["entries"])) {
            let mut arguments = Self::limit_args(&limits)?;
            arguments.insert("workspace".into(), json!("agent"));
            let graph = self.read_one("list_hubs", Value::Object(arguments))?;
            for (tree, e) in obj(&self.state["trees"]) {
                if !ps::truthy(&e["entries"]) {
                    continue;
                }
                let section = ps::pipe(&graph["entries"]).into_iter().find(|row| ps::eq(&st(&row["tree"]), &tree));
                let problem = match section {
                    None => Some("主题卡的“## 入口”一节不见了。".to_string()),
                    Some(row) if !ps::truthy(&row["valid"]) => Some(format!(
                        "主题卡的“## 入口”一节不再有效（{}：{}）。",
                        st(&row["fault"]),
                        st(&row["detail"])
                    )),
                    Some(_) => None,
                };
                if let Some(problem) = problem {
                    total += 1;
                    let line = format!("- 主题 [[{tree}/0]]（入口）：{problem}在 entries 里重写一到三项，或写主题枢纽。");
                    entry_faults.push(line.clone());
                    let mut list = by_tree.get(&tree).cloned().unwrap_or_default();
                    list.push(line);
                    by_tree.insert(&tree, list);
                }
            }
        }
        let mut jobs = Vec::new();
        let seq = st(&self.state["seq"]);
        for (id, job) in obj(&self.state["jobs"]) {
            let topic = ps::eq(&st(&job["kind"]), "topic");
            let keys: Vec<String> = if topic { ps::pipe(&job["trees"]).into_iter().map(st).collect() } else { vec!["field".into()] };
            let lines: Vec<String> = keys.iter().filter_map(|k| by_tree.get(k)).flatten().cloned().collect();
            if lines.is_empty() {
                continue;
            }
            let file = ps::join(&self.work, &format!("fix-{id}-{seq}.md"));
            let checks: Vec<String> = if topic {
                keys.iter().filter(|k| by_tree.contains(k)).map(|k| self.self_check(&ps::join(&self.out, &format!("hub-{k}.json")))).collect()
            } else {
                vec![self.self_check(&ps::join(&self.out, "fields.json"))]
            };
            save(
                &file,
                &format!(
                    "# 修正 {id}\n\nZT 的 get_hub 报告了下列问题。按说明修改对应的 JSON 文件（只改有问题的部分）。每个文件改完后运行它的 SELF-CHECK 命令，改到最后一行是 `faults: 0`，再回复一行 JSON。\n\n{}\n\n{}\n",
                    lines.join("\n"),
                    checks.join("\n")
                ),
            )?;
            jobs.push(json!({ "id": format!("{id}-fix"), "continue": id, "files": [file], "fallback": [self.doc, job["file"], file], "prompt": FIX_PROMPT }));
        }
        self.save_state()?;
        util::print_line(&ps::json_compact(&json!({
            "jobs": jobs,
            "hubs": hubs.len(),
            "faults": total,
            "one_book": one_book,
            "entry_faults": entry_faults,
        })))
    }

    // --- report --------------------------------------------------------------------------------

    fn step_report(&mut self) -> Result<()> {
        let limits = self.read_limits()?;
        let mut arguments = Self::limit_args(&limits)?;
        arguments.insert("workspace".into(), json!("agent"));
        let graph = self.read_one("list_hubs", Value::Object(arguments))?;
        let mut faults: CiMap<i64> = CiMap::new();
        for h in ps::pipe(&graph["hubs"]) {
            for f in ps::pipe(&h["faults"]) {
                let f = st(f);
                let n = faults.get(&f).copied().unwrap_or(0);
                faults.insert(&f, n + 1);
            }
        }
        let faults: Map<String, Value> = faults.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        let missing = ps::pipe(&graph["missing"]);
        let entries = ps::pipe(&graph["entries"]);
        let report = json!({
            "counts": graph["counts"], "faults": faults,
            "mirror": ps::pipe(&graph["hubs"]).into_iter().filter(|h| ps::truthy(&h["mirror"])).count(),
            "entries": entries.iter().map(|e| (*e).clone()).collect::<Vec<_>>(),
            "entries_valid": entries.iter().filter(|e| ps::truthy(&e["valid"])).count(),
            "entries_invalid": entries.iter().filter(|e| !ps::truthy(&e["valid"])).count(),
            "missing": missing.iter().take(30).map(|m| (*m).clone()).collect::<Vec<_>>(), "missing_count": missing.len(),
            "register": self.state["register"], "fields": self.state["fields"], "limits": limits,
        });
        save(&ps::join(&self.work, "report.json"), &ps::json_pretty(&report))?;
        util::print_line(&ps::json_compact(&report))
    }
}

/// `$survey.trees.Values`.
fn survey_trees_of(survey: &Value) -> Vec<Value> {
    survey["trees"].as_object().map(|m| m.values().cloned().collect()).unwrap_or_default()
}

/// One Locked tree this plan would have written: its root, the action, and the archive's typed
/// refusal vocabulary (a Locked tree takes no new hub, no edit and no field hub).
fn blocked_tree(tree: &str, reason: &str, action: &str, lock: &Value) -> Value {
    let refusal = crate::lockcheck::lock_in(lock)
        .map(|fact| crate::lockcheck::blocked_destination(reason, &format!("{tree}/0"), &fact))
        .unwrap_or(Value::Null);
    json!({"tree": tree, "root": format!("{tree}/0"), "action": action, "refusal": refusal})
}

/// `ConvertFrom-Json -AsHashtable` of a parameter's text.
fn read_json_text(text: &str) -> Result<Value> {
    ps::parse_json(text).map_err(|e| anyhow!("Conversion from JSON failed with error: {e}"))
}

/// The tree id of a Card address (`3/2|a` → `3`).
fn tree_of(address: &str) -> String {
    address.split('/').next().unwrap_or_default().to_string()
}

/// Every `[[address]]` a staged body names.
fn stage_links(body: &str) -> Vec<String> {
    static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]|]+)(?:\|[^\]]*)?\]\]").unwrap());
    let mut out = Vec::new();
    for captures in LINK.captures_iter(body) {
        let target = captures[1].trim().to_string();
        if !target.is_empty() && !out.contains(&target) {
            out.push(target);
        }
    }
    out
}

/// The staged changes as review items: each created hub together with the call that writes its
/// final body, and each existing Card change on its own (S27-AC01). A create and its filling edit
/// are one change, so a reviewer decides about one hub, not about two operations.
fn stage_items(calls: &[Value], evidence: &[Value]) -> Result<Vec<Value>> {
    struct Group {
        first: usize,
        calls: Vec<usize>,
    }
    let mut groups: Vec<Group> = Vec::new();
    let mut owner: HashMap<usize, usize> = HashMap::new();
    for (index, call) in calls.iter().enumerate() {
        if st(&call["tool"]) != "edit_card" {
            groups.push(Group { first: index, calls: vec![index] });
            owner.insert(index, groups.len() - 1);
            continue;
        }
        let at = st(&call["arguments"]["at"]);
        let parent = at.strip_prefix("@@").and_then(|n| n.parse::<usize>().ok());
        match parent.and_then(|parent| owner.get(&parent).copied()) {
            Some(group) => groups[group].calls.push(index),
            None => groups.push(Group { first: index, calls: vec![index] }),
        }
    }
    let mut items = Vec::new();
    for group in groups {
        let first = group.first;
        let last = *group.calls.last().unwrap_or(&first);
        let created = evidence.get(first).is_some_and(|row| row["kind"] == json!("create"));
        // A call that omits the level keeps the Card's own hub level, so the review shows the kind
        // of hub it changes; a new hub always names its level.
        let level = group
            .calls
            .iter()
            .rev()
            .map(|index| st(&calls[*index]["arguments"]["level"]))
            .find(|level| !level.is_empty())
            .or_else(|| ps::eq(&st(&evidence[first]["type"]), "hub").then_some("existing".to_string()))
            .unwrap_or_default();
        let title = group
            .calls
            .iter()
            .rev()
            .map(|index| st(&calls[*index]["arguments"]["title"]))
            .find(|title| !title.is_empty())
            .unwrap_or_default();
        let new_body = st(&calls[last]["arguments"]["body"]);
        let anchor = st(&evidence[first]["at"]);
        let tree = if level == "register" {
            "register".to_string()
        } else if level == "field" {
            "field".to_string()
        } else {
            tree_of(&anchor)
        };
        let written_body = st(&calls[first]["arguments"]["body"]);
        let linked = stage_links(&new_body);
        let crossings = linked
            .iter()
            // A @@N target is a Card this plan creates; it has no tree yet, and the review
            // shows it as the staged change it is.
            .filter(|target| !target.starts_with("@@") && tree_of(target) != tree && tree != "field" && tree != "register")
            .cloned()
            .collect::<Vec<_>>();
        let policy = evidence[first]["policy"].clone();
        let anchor_title = evidence[first]["anchor_title"].clone();
        let (old_body, old_fingerprint, reused, why) = if created {
            (
                String::new(),
                String::new(),
                false,
                format!("a new {} hub the staged draft needs (opening body `{}`)", level, written_body.lines().next().unwrap_or_default()),
            )
        } else {
            let is_hub = ps::eq(&st(&evidence[first]["type"]), "hub");
            (
                st(&evidence[first]["old_body"]),
                st(&evidence[first]["old_fingerprint"]),
                is_hub,
                if is_hub {
                    "the existing hub is reused: its own body is rewritten in place, no second hub is created".to_string()
                } else {
                    "the Topic Card's own body gains or replaces its `## 入口` section".to_string()
                },
            )
        };
        items.push(json!({
            "id": format!("hub:{first:03}"),
            "kind": if created { "create" } else { "edit" },
            "calls": group.calls,
            "level": level,
            "tree": tree,
            "address": if created { Value::Null } else { json!(anchor) },
            "title": title,
            "old_body": old_body,
            "old_fingerprint": old_fingerprint,
            "new_body": new_body,
            "reuse": reused,
            "why": why,
            "links": linked,
            "crossings": crossings,
            "policy": if policy.is_null() { json!({"mirror": Value::Null, "one_book": [], "faults": []}) } else { policy },
            "anchor_title": if anchor_title.is_null() { json!("") } else { anchor_title },
        }));
    }
    Ok(items)
}

/// `zt-digest hub`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["Step", "Work", "File", "Limits", "Trees", "PerJob"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &["Rebuild"])?;
    let p = Params {
        step: a.choice("Step", &["survey", "plan", "apply", "stage", "check", "report", "precheck"])?,
        file: a.str("File"),
        limits: a.str("Limits"),
        trees: a.str("Trees"),
        per_job: a.int_or("PerJob", 0)?,
        rebuild: a.flag("Rebuild"),
    };
    let work = a.required("Work")?;
    if work.is_empty() {
        bail!("Cannot bind argument to parameter 'Work' because it is an empty string.");
    }
    let work = full_path(&work)?;
    let out = ps::join(&work, "out");
    std::fs::create_dir_all(&work).with_context(|| format!("failed to make {work}"))?;
    std::fs::create_dir_all(&out).with_context(|| format!("failed to make {out}"))?;
    let state_file = ps::join(&work, "state.json");
    let state = if ps::exists(&state_file) {
        read_json(&state_file)?
    } else {
        json!({ "trees": {}, "fields": {}, "register": null, "jobs": {}, "seq": 0 })
    };
    let doc = ps::join(&scripts_dir(), "dsh-hub-instructions.zh.md");
    let mut hub = Hub { p, work, out, state_file, state, doc, zt: None, cards: None, pre: Vec::new(), pre_limits: Value::Null, stage: None };
    match hub.p.step.as_str() {
        "survey" => hub.step_survey()?,
        "plan" => hub.step_plan()?,
        "apply" => return hub.step_apply(),
        "stage" => hub.step_stage()?,
        "check" => hub.step_check()?,
        "report" => hub.step_report()?,
        _ => hub.step_precheck()?,
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(at: &str, level: &str, title: &str) -> Value {
        json!({"tool": "create_successor", "arguments": {"at": at, "kind": "side", "type": "hub", "level": level, "title": title, "body": "范围：待写。"}})
    }

    fn edit(at: &str, title: &str, body: &str, extra: Value) -> Value {
        let mut arguments = json!({"at": at, "title": title, "body": body, "base_fingerprint": "f"});
        if let Some(map) = extra.as_object() {
            for (key, value) in map {
                arguments[key] = value.clone();
            }
        }
        json!({"tool": "edit_card", "arguments": arguments})
    }

    fn evidence(kind: &str, at: &str, level: &str) -> Value {
        json!({"kind": kind, "at": at, "level": level, "title": "", "old_body": "", "old_fingerprint": ""})
    }

    /// S27 (#119): a new hub and the call that writes its body are one review item, so a reviewer
    /// decides about one hub; an existing hub is its own edit item with the authored base it
    /// replaces and the crossings it adds.
    #[test]
    fn a_staged_line_hub_and_its_filling_edit_are_one_item() {
        let calls = vec![
            create("3/1", "line", "枢纽：一支"),
            edit("@@0", "枢纽：一支", "范围：一支的入口。\n\n## 机制\n- [[3/1]]：甲。", json!({})),
            edit("3/1|a", "主题枢纽", "## 线索\n- [[@@0]]（枢纽）：甲。\n\n## 邻近\n- 对照 [[4/2]]：乙。", json!({"level": "topic"})),
        ];
        let mut items_evidence = vec![
            evidence("create", "3/1", "line"),
            evidence("edit", "@@0", ""),
            evidence("edit", "3/1|a", "topic"),
        ];
        items_evidence[2]["type"] = json!("hub");
        items_evidence[2]["old_body"] = json!("## 旧");
        items_evidence[2]["old_fingerprint"] = json!("0123");
        items_evidence[2]["policy"] = json!({"mirror": false, "one_book": [], "faults": []});
        let items = stage_items(&calls, &items_evidence).unwrap();
        assert_eq!(items.len(), 2, "the create and its edit are one item: {items:?}");
        assert_eq!(ps::s(&items[0]["kind"]), "create");
        assert_eq!(items[0]["calls"].as_array().map(Vec::len), Some(2));
        assert_eq!(ps::s(&items[0]["level"]), "line");
        assert_eq!(ps::s(&items[0]["tree"]), "3");
        assert_eq!(items[0]["address"], Value::Null);
        assert!(ps::s(&items[0]["new_body"]).contains("一支的入口"));
        assert_eq!(ps::s(&items[0]["old_fingerprint"]), "");
        assert_eq!(ps::s(&items[1]["kind"]), "edit");
        assert_eq!(ps::s(&items[1]["address"]), "3/1|a");
        assert_eq!(ps::s(&items[1]["old_fingerprint"]), "0123");
        assert_eq!(items[1]["reuse"], json!(true));
        assert!(ps::s(&items[1]["why"]).contains("reused"));
        assert_eq!(items[1]["crossings"], json!(["4/2"]));
        assert_eq!(items[1]["id"], json!("hub:002"));
    }

    #[test]
    fn a_staged_edit_keeps_the_level_a_new_hub_would_name() {
        let calls = vec![edit("2/1", "领域：学习", "范围：领域。\n\n## 基础\n- [[@]]", json!({"level": "field"}))];
        let mut items_evidence = vec![evidence("edit", "2/1", "")];
        items_evidence[0]["type"] = json!("hub");
        let items = stage_items(&calls, &items_evidence).unwrap();
        assert_eq!(ps::s(&items[0]["level"]), "field");
        assert_eq!(ps::s(&items[0]["tree"]), "field");
    }

    #[test]
    fn a_topic_body_edit_is_not_a_hub_reuse() {
        let calls = vec![edit("3/0", "", "范围：题目。\n\n## 入口\n- [[3/1]]：甲。", json!({}))];
        let mut items_evidence = vec![evidence("edit", "3/0", "")];
        items_evidence[0]["type"] = json!("idea");
        let items = stage_items(&calls, &items_evidence).unwrap();
        assert_eq!(items[0]["reuse"], json!(false));
        assert_eq!(ps::s(&items[0]["tree"]), "3");
    }

    #[test]
    fn a_staged_body_names_each_link_once() {
        assert_eq!(
            stage_links("- [[3/1]] 与 [[3/1|a]] 和 [[pub2020/2]]"),
            vec!["3/1", "pub2020/2"]
        );
        assert!(stage_links("范围：没有链接。").is_empty());
        assert_eq!(tree_of("3/2|a"), "3");
    }
}
