//! `zt-digest grow -Step next` and `-Step status`: the idea-grow run as one command that decides
//! what happens next (docs/workflow-simplicity-proposal.md S2, S3, S9; zt-idea-grow 4.0.0).
//!
//!   zt-digest grow -Step next   -Work <folder> -Options <json> [-After <token>]
//!   zt-digest grow -Step status -Work <folder>
//!
//! `next` runs every deterministic grow step it can, in the order zt-grow-cycle v11 ran them, and
//! stops at the next DeepSeek job set or at the end. It prints one line of one of three shapes:
//!
//!   {"next":"jobs","round":…,"token":N,"ids":[…],"job":{cwd,files,prompt,skills,model,effort,expect,approve},"sum":…}
//!   {"next":"done", …counts}
//!   {"next":"fault","code":…,"round":…,"cause":…}
//!
//! The driver hands the jobs to DSMP `dsDispatch` (each id fills `{id}` in the job template) and runs
//! `next -After <token>` again. The steps themselves are the grow steps of `grow.rs` and the review
//! tools (`restate`, `concepts`), run as child processes of this binary: `next` only calls them in
//! order, with `next.json` in the work folder as its memory (the options, the rounds done, the
//! pending job set and the last line). A rerun with the same folder and options continues where the
//! run stopped: a round recorded done is skipped, and a job set that was printed but never confirmed
//! is printed again for the jobs whose answer file is missing.
//!
//! `status` prints the last line `next` printed, unchanged: the one recovery when a relay returned a
//! line that is not one JSON object (S2). `sum` is a checksum of the job set (FNV-1a over the ids and
//! the job template), so a driver detects a line a relay cut or rewrote and reads `status` instead.
//!
//! With `zt_config` (a copy of the box) the first call writes the box pin `zt-box.txt` into the work
//! folder. Every child step, and every DeepSeek job (it runs in the work folder and names paths
//! under it), then reads and writes that copy, so the rounds that 3.2.0 ran on the live box only
//! (restatement review, Topic maintenance, concept review, hubs and register) run on the copy too.
//! The exit code is 0 for every printed line; only an internal error exits 1.

use crate::args::Args;
use crate::util;
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::path::Path;

const NEXT: &str = "next.json";
/// The longest cause a fault line carries.
const CAUSE_CHARS: usize = 600;

fn s(v: &Value) -> String {
    match v {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn join(dir: &str, name: &str) -> String {
    format!("{}\\{}", dir.trim_end_matches(['\\', '/']), name)
}

fn tail(text: &str) -> String {
    let chars: Vec<char> = text.trim().chars().collect();
    chars[chars.len().saturating_sub(CAUSE_CHARS)..].iter().collect()
}

/// FNV-1a (32 bit) over the Unicode scalar values of a text, as hex. The driver computes the same
/// sum over the same fields (`jobSum` in zt-grow-cycle v12), so any change a relay made shows.
pub fn fnv(text: &str) -> String {
    let mut h: u32 = 0x811c9dc5;
    for c in text.chars() {
        h ^= c as u32;
        h = h.wrapping_mul(0x01000193);
    }
    format!("{h:08x}")
}

/// The text the checksum covers: the ids, then each template field, one per line.
pub fn sum_text(ids: &[String], job: &Value) -> String {
    let files: Vec<String> = job["files"].as_array().into_iter().flatten().map(s).collect();
    [ids.join(","), s(&job["cwd"]), files.join("\n"), s(&job["prompt"]), s(&job["approve"]), s(&job["expect"])].join("\n")
}

// --- the options -------------------------------------------------------------------------------

/// The run's options, frozen at the first call (`next.json`). A later call must pass the same.
#[derive(Clone, Debug)]
struct Opts {
    raw: Value,
}

impl Opts {
    fn parse(text: &str) -> Result<Self> {
        let raw: Value = serde_json::from_str(text).context("-Options is not JSON")?;
        if !raw.is_object() {
            bail!("-Options must be a JSON object");
        }
        let mode = s(&raw["mode"]);
        if !mode.is_empty() && mode != "full" && mode != "hubs" {
            bail!("-Options mode must be full or hubs, not {mode}");
        }
        let register = s(&raw["register"]);
        if !register.is_empty() && !["apply", "plan", "off"].contains(&register.as_str()) {
            bail!("-Options register must be apply, plan or off, not {register}");
        }
        Ok(Self { raw })
    }
    fn mode(&self) -> &str {
        if self.raw["mode"] == json!("hubs") { "hubs" } else { "full" }
    }
    fn text(&self, k: &str) -> String {
        s(&self.raw[k])
    }
    /// A switch that is on unless the options turn it off.
    fn on(&self, k: &str) -> bool {
        self.raw[k] != json!(false)
    }
    fn register(&self) -> String {
        let r = self.text("register");
        if r.is_empty() { "apply".into() } else { r }
    }
    fn tool(&self) -> String {
        let t = self.text("tool");
        if t.is_empty() { "zt-digest".into() } else { t }
    }
    /// The command a DeepSeek job names for this binary (the approval rules and prompts).
    fn exe(&self) -> String {
        let t = self.tool();
        if t == "zt-digest" { t } else { format!("& '{}'", t.replace('\'', "''")) }
    }
    fn effort(&self) -> &str {
        if self.raw["effort"] == json!("max") { "max" } else { "high" }
    }
    fn model(&self) -> String {
        let m = self.text("model");
        if m.is_empty() { "deepseek-official/deepseek-flash".into() } else { m }
    }
    /// The books the run reads: `-Books a,b`, `-Book k`, or none (every Literature tree).
    fn which(&self) -> Vec<String> {
        let books: Vec<String> = self.raw["books"].as_array().into_iter().flatten().map(s).filter(|b| !b.is_empty()).collect();
        if !books.is_empty() {
            return vec!["-Books".into(), books.join(",")];
        }
        let book = self.text("book");
        if book.is_empty() { Vec::new() } else { vec!["-Book".into(), book] }
    }
    /// The options as they decide the run (canonical, for the frozen comparison).
    fn canon(&self) -> String {
        let mut keys: Vec<(&String, &Value)> = self.raw.as_object().map(|m| m.iter().collect()).unwrap_or_default();
        keys.sort_by(|a, b| a.0.cmp(b.0));
        let m: Map<String, Value> = keys.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        Value::Object(m).to_string()
    }
}

// --- the memory --------------------------------------------------------------------------------

struct Memory {
    path: String,
    v: Value,
}

impl Memory {
    fn load(work: &str) -> Result<Self> {
        let path = join(work, NEXT);
        let v = if Path::new(&path).exists() { util::read_json(&path)? } else { json!({ "seq": 0, "done": [], "results": {} }) };
        Ok(Self { path, v })
    }
    fn save(&self) -> Result<()> {
        util::write_text(&self.path, &serde_json::to_string_pretty(&self.v)?)
    }
    fn done(&self, round: &str) -> bool {
        self.v["done"].as_array().is_some_and(|d| d.iter().any(|x| x == round))
    }
    fn finish(&mut self, round: &str, result: Value) {
        if !self.done(round) {
            if let Some(d) = self.v["done"].as_array_mut() {
                d.push(json!(round));
            }
        }
        self.v["results"][round] = result;
    }
    fn result(&self, round: &str) -> &Value {
        &self.v["results"][round]
    }
    fn num(&self, round: &str, field: &str) -> i64 {
        self.v["results"][round][field].as_i64().unwrap_or(0)
    }
}

// --- child steps -------------------------------------------------------------------------------

/// What one child step answered.
enum Step {
    Line(Value),
    Locked(Value),
    Failed(String),
}

fn child(args: &[String]) -> Result<Step> {
    let exe = std::env::current_exe().context("the zt-digest binary")?;
    let out = std::process::Command::new(exe).args(args).output().with_context(|| format!("zt-digest {}", args.join(" ")))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let last = stdout.lines().rev().find_map(|l| serde_json::from_str::<Value>(l.trim()).ok().filter(Value::is_object)).unwrap_or(Value::Null);
    // Tree Lock (#140, R326/R337): the owner's typed refusal is a destination decision, never a fault
    // to retry; any line of the child's stdout may carry it (a stop prints totals, then the refusal).
    for line in stdout.lines() {
        if let Ok(v) = serde_json::from_str::<Value>(line.trim())
            && (v["code"] == json!("card_locked") || v["error"] == json!("card_locked"))
        {
            return Ok(Step::Locked(v));
        }
    }
    let code = out.status.code().unwrap_or(1);
    if code != 0 {
        // The child's own error sentence comes first (its stderr line `zt-digest …: …`); a cause cut from the end
        // of the output kept only quoted Card text (the 2026-10-24 pilot's register stop read as its body's last
        // terms).
        let said = stderr.lines().map(str::trim).find(|l| l.starts_with("zt-digest")).map(|l| l.chars().take(CAUSE_CHARS).collect::<String>());
        return Ok(Step::Failed(format!(
            "zt-digest {} exited {code}: {}",
            args.iter().take(3).cloned().collect::<Vec<_>>().join(" "),
            said.unwrap_or_else(|| tail(&format!("{stdout}\n{stderr}")))
        )));
    }
    if !last.is_object() {
        return Ok(Step::Failed(format!("zt-digest {} printed no JSON line: {}", args.iter().take(3).cloned().collect::<Vec<_>>().join(" "), tail(&stdout))));
    }
    Ok(Step::Line(last))
}

fn grow_args(step: &str, work: &str, more: &[String]) -> Vec<String> {
    let mut v = vec!["grow".to_string(), "-Step".into(), step.into(), "-Work".into(), work.into()];
    v.extend(more.iter().cloned());
    v
}

fn strs(list: &[&str]) -> Vec<String> {
    list.iter().map(|x| x.to_string()).collect()
}

// --- the job families --------------------------------------------------------------------------

/// The DeepSeek job template of one family; `{id}` stands for the job id.
fn template(family: &str, work: &str, o: &Opts) -> Value {
    let exe = o.exe();
    let pin = if o.text("zt_config").is_empty() { String::new() } else { format!(", which may first set $env:ZT_CONFIG_DIR='{}'", o.text("zt_config")) };
    let grow_rule = format!(
        "allow only these: a command that runs the zt helper \"{exe} call\" with -Allow \"{work}\\read-only.allow.txt\" (an empty allow-list: reads only){pin}; the self-check \"{exe} grow -Step precheck\" (read-only); a read-only read or search (Read, Get-Content, Select-String) of a file under \"{work}\"; and writing JSON files under \"{work}\\out\" or \"{work}\\hubs\\out\"; for a hub job (gh-…) also the hub self-check \"{exe} hub -Step precheck\" (read-only) and the zt helper with -Allow \"{work}\\hubs\\none.allow.txt\" (reads only). Reject anything else that writes, moves, or deletes files, and every other zt-digest step. Answer a question with custom text \"按任务说明自行决定，不需要询问。\""
    );
    let grow_prompt = format!("按上面的任务说明和任务文件完成任务 {{id}}。答案只写到任务文件给出的答案文件。\n\nSELF-CHECK: {exe} grow -Step precheck -Work \"{work}\" -Job {{id}}\n回复前运行 SELF-CHECK 命令。改正它报告的每个问题，再运行一次，直到它输出 \"faults: 0\"（最多三次）。最后一行回复 \"OK\" 或 \"FLAG: <剩下的问题>\"。");
    let skills = json!(["zettelkasten", "asd-ste100-zh"]);
    let (files, prompt, expect, approve, skills) = match family {
        "grow" => (vec![join(work, "instructions.zh.md"), join(work, "jobs\\{id}.md")], grow_prompt, "(^|\\n)(OK|FLAG:)".to_string(), grow_rule, skills),
        "plain" => (vec![join(work, "jobs\\{id}.md")], grow_prompt, "(^|\\n)(OK|FLAG:)".to_string(), grow_rule, skills),
        "restate" => {
            let st = join(work, "restate\\{id}");
            let w = join(&st, "work");
            (
                vec![join(&w, "dsh-restate-instructions.zh.md"), join(&w, "rj-01.md")],
                "按以上说明判断这些重述对，写出 JSON 文件，运行 SELF-CHECK 改到没有错误。最后只回复一行 JSON。".to_string(),
                "\\{".to_string(),
                format!("allow only these: the restatement self-check \"{exe} restate -Step judge\" with -Staging \"{st}\" (it validates the answer and writes only under \"{st}\"); a read-only read or search (Read, Get-Content, Select-String) of a file under \"{work}\"; and writing JSON files under \"{w}\\out\". Reject anything else that writes, moves, or deletes files, and every other zt-digest step. Answer a question with custom text \"按任务说明自行决定，不需要询问。\""),
                skills,
            )
        }
        _ => {
            let st = join(work, "concepts\\{id}");
            let w = join(&st, "work");
            (
                vec![join(&w, "dsh-concept-instructions.zh.md"), join(&w, "cj-01.md")],
                "按以上说明判断这些提议的概念，写出 JSON 文件，运行 SELF-CHECK 改到没有错误。最后只回复一行 JSON。".to_string(),
                "\\{".to_string(),
                format!("allow only these: the concept self-check \"{exe} concepts -Step judge\" with -Staging \"{st}\" (it validates the answer and writes only under \"{st}\"); a read-only read or search (Read, Get-Content, Select-String) of a file under \"{work}\"; and writing JSON files under \"{w}\\out\". Reject anything else that writes, moves, or deletes files, and every other zt-digest step. Answer a question with custom text \"按任务说明自行决定，不需要询问。\""),
                json!(["asd-ste100-zh"]),
            )
        }
    };
    json!({ "family": family, "cwd": work, "files": files, "prompt": prompt, "skills": skills, "model": o.model(), "effort": o.effort(), "expect": expect, "approve": approve })
}

/// The answer file a job of a family writes; a rerun prints again only the jobs without one. A hub
/// job writes its own files under `hubs`, so it has none here (it is always printed again).
fn answer_file(family: &str, work: &str, id: &str) -> Option<String> {
    match family {
        "grow" | "plain" if !id.starts_with("gh") => Some(join(work, &format!("out\\{id}.json"))),
        "restate" => Some(join(work, &format!("restate\\{id}\\work\\out\\rj-01.json"))),
        "concepts" => Some(join(work, &format!("concepts\\{id}\\work\\out\\cj-01.json"))),
        _ => None,
    }
}

fn numbered(prefix: &str, n: i64) -> Vec<String> {
    (1..=n.max(0)).map(|i| format!("{prefix}-{i:03}")).collect()
}

// --- the rounds --------------------------------------------------------------------------------

/// What one round did: it finished (with its result), it needs DeepSeek jobs, or it stopped.
enum Did {
    Done(Value),
    Jobs(String, Vec<String>),
    Fault(&'static str, String, Option<Value>),
}

fn step_line(args: Vec<String>) -> Result<std::result::Result<Value, Did>> {
    Ok(match child(&args)? {
        Step::Line(v) => Ok(v),
        Step::Locked(v) => Err(Did::Fault("card_locked", s(&v["message"]).chars().take(CAUSE_CHARS).collect(), Some(v))),
        Step::Failed(cause) => Err(Did::Fault("step_failed", cause, None)),
    })
}

/// A child step whose failure is recorded for its book alone (the review rounds): its line, or null
/// with the cause; a Lock still stops the round.
fn soft(args: Vec<String>) -> Result<std::result::Result<(Value, String), Did>> {
    Ok(match child(&args)? {
        Step::Line(v) => Ok((v, String::new())),
        Step::Locked(v) => Err(Did::Fault("card_locked", s(&v["message"]).chars().take(CAUSE_CHARS).collect(), Some(v))),
        Step::Failed(cause) => Ok((Value::Null, cause)),
    })
}

macro_rules! line {
    ($e:expr) => {
        match step_line($e)? {
            Ok(v) => v,
            Err(d) => return Ok(d),
        }
    };
}
macro_rules! soft {
    ($e:expr) => {
        match soft($e)? {
            Ok(v) => v,
            Err(d) => return Ok(d),
        }
    };
}

/// The rounds of a run, in order. Each name is a round of `next.json`; a job round's name ends
/// in `-jobs`.
fn rounds(o: &Opts) -> Vec<&'static str> {
    let mut r: Vec<&'static str> = Vec::new();
    if o.mode() == "full" {
        r.extend(["plan", "topics-jobs", "topics", "ideas-jobs", "apply", "links", "links-jobs", "links-apply"]);
        // 4.0.1: the restatement review runs before the check, so the one fix round also sees the Cards its
        // accepted rewrites changed (the 2026-10-24 pilot: five rewrites "本卡依赖 [[<own book>/n]]…" left I7 faults
        // after a check that had found none).
        if o.on("restate") {
            r.extend(["restate-prepare", "restate-jobs", "restate-finish"]);
        }
        r.extend(["check", "fix-jobs", "fix-apply"]);
        if o.raw["organize"] == json!(true) {
            r.extend(["organize-plan", "organize-jobs", "organize-apply"]);
        }
    }
    if o.on("hubs") {
        if o.on("concepts") && o.register() != "off" {
            r.extend(["concepts-prepare", "concepts-jobs", "concepts-finish"]);
        }
        r.extend(["hubs-plan", "hubs-jobs", "hubs-apply", "hubs-check", "hubs-fix-jobs", "hubs-fix-apply", "hubs-report"]);
    }
    if o.mode() == "full" {
        r.push("report");
    }
    r
}

/// Runs one round. `m` holds the earlier rounds' results.
fn round(name: &str, work: &str, o: &Opts, m: &mut Memory) -> Result<Did> {
    let skip = |why: &str| Did::Done(json!({ "skipped": why }));
    Ok(match name {
        "plan" => {
            let mut more = o.which();
            more.extend(strs(&["-Instructions"]));
            more.push(o.text("instructions"));
            if o.tool() != "zt-digest" {
                more.extend(["-Tool".into(), o.tool()]);
            }
            if !o.text("zt_config").is_empty() {
                more.extend(["-ZtConfig".into(), o.text("zt_config")]);
            }
            for (k, f) in [("from", "-From"), ("to", "-To")] {
                if let Some(n) = o.raw[k].as_i64() {
                    more.extend([f.to_string(), n.to_string()]);
                }
            }
            Did::Done(line!(grow_args("plan", work, &more)))
        }
        "topics-jobs" => {
            if m.num("plan", "pending") > 0 && m.num("plan", "topic_jobs") > 0 { Did::Jobs("grow".into(), vec!["gt-001".into()]) } else { skip("no Topic job") }
        }
        "topics" => {
            if m.num("plan", "pending") > 0 { Did::Done(line!(grow_args("topics", work, &strs(&["-Apply"])))) } else { skip("no pending section") }
        }
        "ideas-jobs" => {
            let n = m.num("topics", "jobs");
            if n > 0 { Did::Jobs("grow".into(), numbered("gw", n)) } else { skip("no section job") }
        }
        "apply" | "links" => {
            if m.num("topics", "jobs") > 0 { Did::Done(line!(grow_args(name, work, &if name == "apply" { strs(&["-Apply"]) } else { Vec::new() }))) } else { skip("no section job") }
        }
        "links-jobs" => {
            let ids: Vec<String> = m.result("links")["ids"].as_array().into_iter().flatten().map(s).collect();
            if ids.is_empty() { skip("no link job") } else { Did::Jobs("grow".into(), ids) }
        }
        "links-apply" => {
            if m.result("links")["ids"].as_array().is_some_and(|l| !l.is_empty()) { Did::Done(line!(grow_args("apply", work, &strs(&["-Apply"])))) } else { skip("no link job") }
        }
        // S9: one fix round (fixRounds frozen at 1).
        "check" => Did::Done(line!(grow_args("check", work, &strs(&["-Round", "1"])))),
        "fix-jobs" => {
            let n = m.num("check", "fix");
            if n > 0 { Did::Jobs("plain".into(), numbered("fx1", n)) } else { skip("no fault") }
        }
        "fix-apply" => {
            if m.num("check", "fix") > 0 { Did::Done(line!(grow_args("apply", work, &strs(&["-Apply"])))) } else { skip("no fault") }
        }
        "restate-prepare" => {
            let listed = line!(grow_args("restate-cards", work, &[]));
            let mut books = Vec::new();
            for r in listed["reviews"].as_array().into_iter().flatten() {
                let st = s(&r["staging"]);
                let w = join(&st, "work");
                let (prepared, cause) = soft!(vec!["restate".into(), "-Step".into(), "prepare".into(), "-Staging".into(), st.clone(), "-Work".into(), w, "-Cards".into(), join(&st, "cards.txt")]);
                let job = prepared["jobs"].as_array().is_some_and(|j| !j.is_empty());
                books.push(json!({ "id": r["id"], "book": r["book"], "cards": r["cards"], "staging": st, "prepared": !prepared.is_null(), "job": job, "reason": cause }));
            }
            Did::Done(json!({ "books": books }))
        }
        "restate-jobs" => {
            let ids: Vec<String> = m.result("restate-prepare")["books"].as_array().into_iter().flatten().filter(|b| b["job"] == true).map(|b| s(&b["id"])).collect();
            if ids.is_empty() { skip("no judge job") } else { Did::Jobs("restate".into(), ids) }
        }
        "restate-finish" => {
            let run_id = format!("grow-{}", Path::new(work).file_name().map(|f| f.to_string_lossy().replace(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-', "-")).unwrap_or_else(|| "run".into()));
            let mut out = Vec::new();
            for b in m.result("restate-prepare")["books"].as_array().cloned().unwrap_or_default() {
                let (id, st) = (s(&b["id"]), s(&b["staging"]));
                if b["prepared"] != true {
                    out.push(json!({ "id": id, "book": b["book"], "status": "skipped", "reason": tail(&s(&b["reason"])) }));
                    continue;
                }
                let w = join(&st, "work");
                let base = |step: &str| vec!["restate".to_string(), "-Step".into(), step.into(), "-Staging".into(), st.clone(), "-Work".into(), w.clone()];
                let mut judged = Value::Null;
                if b["job"] == true {
                    let mut a = base("judge");
                    a.extend(["-File".into(), join(&w, "out\\rj-01.json")]);
                    judged = soft!(a).0;
                }
                let mut applied = Value::Null;
                if !judged.is_null() {
                    let mut a = base("apply");
                    a.extend(["-OperationPrefix".into(), format!("{run_id}/restate-{id}/1/apply")]);
                    applied = soft!(a).0;
                }
                let reported = soft!(base("report")).0;
                let c = &reported["counts"];
                out.push(json!({
                    "id": id, "book": b["book"], "cards": b["cards"], "status": if reported.is_null() { json!("failed") } else { reported["status"].clone() },
                    "report_id": reported["report_id"], "resolved_by_rewrite": c["resolved_by_rewrite"].as_i64().unwrap_or(0),
                    "pending": c["pending_existing_card_change"].as_i64().unwrap_or(0), "unresolved": c["unresolved"].as_i64().unwrap_or(0),
                    "written": applied["written"].as_i64().unwrap_or(0),
                }));
            }
            Did::Done(json!({ "books": out.len(), "reviews": out }))
        }
        "organize-plan" => {
            let trees = o.text("organize_trees");
            Did::Done(line!(grow_args("organize-plan", work, &if trees.is_empty() { Vec::new() } else { vec!["-Trees".into(), trees] })))
        }
        "organize-jobs" => {
            if m.num("organize-plan", "organize_jobs") > 0 { Did::Jobs("plain".into(), vec!["go-001".into()]) } else { skip("no decision job") }
        }
        "organize-apply" => {
            if m.num("organize-plan", "organize_jobs") > 0 { Did::Done(line!(grow_args("organize-apply", work, &[]))) } else { skip("no decision job") }
        }
        "concepts-prepare" => {
            // Mode hubs reviews the named books; mode full the books the run grew Cards from.
            let which = if o.mode() == "hubs" { o.which() } else { Vec::new() };
            let listed = line!(grow_args("concept-books", work, &which));
            let mut books = Vec::new();
            for r in listed["reviews"].as_array().into_iter().flatten() {
                let st = s(&r["staging"]);
                let w = join(&st, "work");
                let base = |step: &str| vec!["concepts".to_string(), "-Step".into(), step.into(), "-Staging".into(), st.clone(), "-Work".into(), w.clone()];
                let (proposed, cause) = soft!(base("propose"));
                if proposed.is_null() || proposed["proposals"].as_i64().unwrap_or(0) == 0 {
                    books.push(json!({ "id": r["id"], "book": r["book"], "staging": st, "prepared": false, "status": if proposed.is_null() { "failed" } else { "skipped" }, "reason": if proposed.is_null() { tail(&cause) } else { s(&proposed["reason"]) } }));
                    continue;
                }
                let (prepared, cause) = soft!(base("prepare"));
                let job = prepared["jobs"].as_array().is_some_and(|j| !j.is_empty());
                books.push(json!({ "id": r["id"], "book": r["book"], "staging": st, "prepared": !prepared.is_null(), "job": job, "status": if prepared.is_null() { "failed" } else { "prepared" }, "reason": tail(&cause) }));
            }
            Did::Done(json!({ "books": books, "skipped": listed["skipped"] }))
        }
        "concepts-jobs" => {
            let ids: Vec<String> = m.result("concepts-prepare")["books"].as_array().into_iter().flatten().filter(|b| b["job"] == true).map(|b| s(&b["id"])).collect();
            if ids.is_empty() { skip("no judge job") } else { Did::Jobs("concepts".into(), ids) }
        }
        "concepts-finish" => {
            let mut out = Vec::new();
            for b in m.result("concepts-prepare")["books"].as_array().cloned().unwrap_or_default() {
                let (id, st) = (s(&b["id"]), s(&b["staging"]));
                if b["prepared"] != true {
                    out.push(json!({ "id": id, "book": b["book"], "status": b["status"], "reason": b["reason"] }));
                    continue;
                }
                let w = join(&st, "work");
                let base = |step: &str| vec!["concepts".to_string(), "-Step".into(), step.into(), "-Staging".into(), st.clone(), "-Work".into(), w.clone()];
                let mut judged = Value::Null;
                if b["job"] == true {
                    let mut a = base("judge");
                    a.extend(["-File".into(), join(&w, "out\\cj-01.json")]);
                    judged = soft!(a).0;
                }
                let reported = soft!(base("report")).0;
                out.push(json!({
                    "id": id, "book": b["book"], "status": if reported.is_null() { json!("failed") } else { reported["status"].clone() }, "report_id": reported["report_id"],
                    "findings": reported["findings"].as_i64().unwrap_or(0), "judged": judged["judged"].as_i64().unwrap_or(0), "unjudged": reported["unjudged"].as_i64().unwrap_or(0),
                }));
            }
            let reviewed = out.iter().any(|r| !r["report_id"].is_null());
            Did::Done(json!({ "books": out.len(), "reviews": out, "reviewed": reviewed }))
        }
        "hubs-plan" => {
            let mut more = Vec::new();
            if let Some(l) = o.raw["hub_limits"].as_object().filter(|l| !l.is_empty()) {
                more.extend(["-Limits".into(), Value::Object(l.clone()).to_string()]);
            }
            let trees = o.text("hub_trees");
            if !trees.is_empty() {
                more.extend(["-Trees".into(), trees]);
            }
            if o.raw["hub_rebuild"] == json!(true) {
                more.push("-Rebuild".into());
            }
            Did::Done(line!(grow_args("hubs-plan", work, &more)))
        }
        "hubs-jobs" => {
            let n = m.num("hubs-plan", "hub_jobs");
            if n > 0 { Did::Jobs("plain".into(), numbered("gh", n)) } else { skip("no hub job") }
        }
        "hubs-apply" => {
            if m.num("hubs-plan", "hub_jobs") > 0 { Did::Done(line!(grow_args("hubs-apply", work, &[]))) } else { skip("no hub job") }
        }
        // S9: one hub fix round (hubFixRounds frozen at 1).
        "hubs-check" => {
            if m.num("hubs-plan", "hub_jobs") > 0 { Did::Done(line!(grow_args("hubs-check", work, &strs(&["-Round", "1"])))) } else { skip("no hub job") }
        }
        "hubs-fix-jobs" => {
            let n = m.num("hubs-check", "fix");
            if n > 0 { Did::Jobs("plain".into(), numbered("gh1", n)) } else { skip("no hub fault") }
        }
        "hubs-fix-apply" => {
            if m.num("hubs-check", "fix") > 0 { Did::Done(line!(grow_args("hubs-apply", work, &[]))) } else { skip("no hub fault") }
        }
        "hubs-report" => {
            let mut more = vec!["-Register".to_string(), o.register()];
            if m.result("concepts-finish")["reviewed"] == json!(true) {
                more.push("-Concepts".into());
            }
            Did::Done(line!(grow_args("hubs-report", work, &more)))
        }
        "report" => {
            // The review rounds' results go into state.json first, so the report renders them.
            let state_path = join(work, "state.json");
            if Path::new(&state_path).exists() {
                let mut state = util::read_json(&state_path)?;
                state["rounds"] = json!({ "restate": m.result("restate-finish"), "concepts": m.result("concepts-finish"), "organize": m.result("organize-apply") });
                util::write_text(&state_path, &serde_json::to_string_pretty(&state)?)?;
            }
            Did::Done(line!(grow_args("report", work, &[])))
        }
        other => bail!("unknown round {other}"),
    })
}

/// The `done` line: the counts a reader needs, bounded.
fn done_line(o: &Opts, m: &Memory) -> Value {
    let rv = |round: &str, field: &str| m.result(round)[field].clone();
    let restate = m.result("restate-finish");
    let sum = |list: &Value, f: &str| list.as_array().map_or(0, |l| l.iter().map(|r| r[f].as_i64().unwrap_or(0)).sum::<i64>());
    let mut line = json!({ "next": "done", "mode": o.mode() });
    if o.mode() == "full" {
        let r = m.result("report");
        for k in ["created", "topics", "linked", "same", "refused", "duplicates", "stopped", "deeper", "faults_left", "share_before", "share_after", "books", "bridges", "report"] {
            line[k] = r[k].clone();
        }
        line["sections"] = rv("plan", "pending");
        line["left"] = rv("plan", "left");
    }
    if restate.is_object() && !restate["reviews"].is_null() {
        line["restate"] = json!({ "books": restate["books"], "resolved": sum(&restate["reviews"], "resolved_by_rewrite"), "pending": sum(&restate["reviews"], "pending"), "unresolved": sum(&restate["reviews"], "unresolved") });
    }
    let concepts = m.result("concepts-finish");
    if concepts.is_object() && !concepts["reviews"].is_null() {
        line["concepts"] = json!({ "books": concepts["books"], "findings": sum(&concepts["reviews"], "findings"), "judged": sum(&concepts["reviews"], "judged") });
    }
    let organized = m.result("organize-apply");
    if organized.is_object() && organized["skipped"].is_null() {
        line["organize"] = json!({ "merges": organized["merges"], "moves": organized["moves"], "refused": organized["refused"], "invalid": organized["invalid"] });
    }
    let hubs = m.result("hubs-report");
    if hubs.is_object() && !hubs["hubs"].is_null() {
        line["hubs"] = hubs["hubs"].clone();
        line["hubs_missing"] = hubs["missing"].clone();
        line["register"] = hubs["register"]["status"].clone();
        line["register_entries"] = hubs["register"]["entries"].clone();
    }
    line
}

fn fault_line(code: &str, round: &str, cause: &str, refusal: Option<Value>) -> Value {
    let mut v = json!({ "next": "fault", "code": code, "round": round, "cause": cause.chars().take(CAUSE_CHARS).collect::<String>() });
    if let Some(r) = refusal {
        for k in ["tree", "lock_id", "at", "step"] {
            if !r[k].is_null() {
                v[k] = r[k].clone();
            }
        }
    }
    v
}

fn jobs_line(m: &mut Memory, round: &str, family: &str, ids: Vec<String>, work: &str, o: &Opts) -> Value {
    let seq = m.v["seq"].as_i64().unwrap_or(0) + 1;
    m.v["seq"] = json!(seq);
    m.v["pending"] = json!({ "token": seq, "round": round, "family": family, "ids": ids });
    let job = template(family, work, o);
    let sum = fnv(&sum_text(&ids, &job));
    json!({ "next": "jobs", "round": round, "token": seq, "ids": ids, "job": job, "sum": sum })
}

/// Prints one line and remembers it as the line `status` repeats.
fn say(m: &mut Memory, line: Value) -> Result<i32> {
    m.v["last"] = line.clone();
    m.save()?;
    util::print_last(&line)?;
    Ok(0)
}

fn next(a: &Args) -> Result<i32> {
    let work = a.required("Work")?.trim_end_matches(['\\', '/']).to_string();
    std::fs::create_dir_all(&work)?;
    let o = match Opts::parse(&a.required("Options")?) {
        Ok(o) => o,
        Err(e) => return util::print_last(&fault_line("bad_options", "", &e.to_string(), None)).map(|_| 0),
    };
    let mut m = Memory::load(&work)?;
    // The options are frozen at the first call: a rerun with other options would mix two runs.
    if m.v["options"].is_null() {
        m.v["options"] = json!(o.canon());
    }
    if let Some(frozen) = m.v["options"].as_str()
        && frozen != o.canon()
    {
        let line = fault_line("options_changed", "", &format!("this folder was started with other options: {frozen}"), None);
        return say(&mut m, line);
    }
    // The box pin: a run on a copy names its box once, in the work folder.
    let config = o.text("zt_config");
    let pin = join(&work, "zt-box.txt");
    if !config.is_empty() && !Path::new(&pin).exists() {
        util::write_text(&pin, &format!("{config}\n"))?;
    }
    // A job set printed earlier: confirmed by -After, or printed again for the jobs without an answer.
    if let Some(p) = m.v["pending"].as_object().cloned() {
        let round = s(&p["round"]);
        let family = s(&p["family"]);
        let ids: Vec<String> = p["ids"].as_array().into_iter().flatten().map(s).collect();
        let after = a.int_or("After", -1)?;
        if after == p["token"].as_i64().unwrap_or(0) {
            m.v["pending"] = Value::Null;
            m.finish(&round, json!({ "jobs": ids.len() }));
        } else {
            let open: Vec<String> = ids.iter().filter(|id| answer_file(&family, &work, id).is_none_or(|f| !Path::new(&f).exists())).cloned().collect();
            if open.is_empty() {
                m.v["pending"] = Value::Null;
                m.finish(&round, json!({ "jobs": ids.len(), "answered_before": true }));
            } else {
                let line = jobs_line(&mut m, &round, &family, open, &work, &o);
                return say(&mut m, line);
            }
        }
    }
    for name in rounds(&o) {
        if m.done(name) {
            continue;
        }
        match round(name, &work, &o, &mut m)? {
            Did::Done(result) => {
                m.finish(name, result);
                m.save()?;
            }
            Did::Jobs(family, ids) => {
                let line = jobs_line(&mut m, name, &family, ids, &work, &o);
                return say(&mut m, line);
            }
            Did::Fault(code, cause, refusal) => {
                let line = fault_line(code, name, &cause, refusal);
                return say(&mut m, line);
            }
        }
    }
    let line = done_line(&o, &m);
    say(&mut m, line)
}

fn status(a: &Args) -> Result<i32> {
    let work = a.required("Work")?;
    let path = join(work.trim_end_matches(['\\', '/']), NEXT);
    if !Path::new(&path).exists() {
        util::print_last(&json!({ "next": "none", "cause": "no next.json in this folder" }))?;
        return Ok(0);
    }
    let m = util::read_json(&path)?;
    let none = json!({ "next": "none", "cause": "next printed no line yet" });
    util::print_last(if m["last"].is_object() { &m["last"] } else { &none })?;
    Ok(0)
}

/// `-Step next` and `-Step status`.
pub fn cli(step: &str, a: &Args) -> Result<i32> {
    match step {
        "next" => next(a),
        _ => status(a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(v: Value) -> Opts {
        Opts::parse(&v.to_string()).unwrap()
    }

    #[test]
    fn the_checksum_is_fnv1a_over_scalar_values() {
        // FNV-1a 32 of "" and "a" (the reference values), and a Chinese text the driver hashes alike.
        assert_eq!(fnv(""), "811c9dc5");
        assert_eq!(fnv("a"), "e40c292c");
        assert_ne!(fnv("主题"), fnv("主體"));
        // The same job set the driver hashes in JavaScript (zt-grow-cycle v12 jobSum): db6c9284 there.
        let job = json!({ "cwd": "C:\\W", "files": ["C:\\W\\instructions.zh.md", "C:\\W\\jobs\\{id}.md"], "prompt": "完成 {id}。\n\nSELF-CHECK: & 'D:/t.exe' grow -Step precheck -Work \"C:\\W\" -Job {id}\n最后一行回复 \"OK\"。", "approve": "allow \"C:\\W\\out\" — reads", "expect": "(^|\\n)(OK|FLAG:)" });
        assert_eq!(fnv(&sum_text(&["gw-001".into(), "gw-002".into()], &job)), "db6c9284");
    }

    #[test]
    fn the_rounds_follow_the_options() {
        let full = rounds(&opts(json!({ "mode": "full" })));
        assert_eq!(full.first(), Some(&"plan"));
        assert_eq!(full.last(), Some(&"report"));
        let pos = |l: &[&str], n: &str| l.iter().position(|x| *x == n).unwrap();
        // links -> restate -> check and fix -> (organize) -> concepts -> hubs: the check sees the restatement rewrites.
        assert!(pos(&full, "links-apply") < pos(&full, "restate-prepare") && pos(&full, "restate-finish") < pos(&full, "check"));
        let org0 = rounds(&opts(json!({ "organize": true })));
        assert!(pos(&org0, "fix-apply") < pos(&org0, "organize-plan"));
        assert!(pos(&full, "restate-finish") < pos(&full, "concepts-prepare"));
        assert!(pos(&full, "concepts-finish") < pos(&full, "hubs-plan"));
        assert!(!full.contains(&"organize-plan"));
        let org = rounds(&opts(json!({ "organize": true })));
        assert!(pos(&org, "restate-finish") < pos(&org, "organize-plan") && pos(&org, "organize-apply") < pos(&org, "concepts-prepare"));
        let lean = rounds(&opts(json!({ "restate": false, "hubs": false })));
        assert!(!lean.contains(&"restate-prepare") && !lean.contains(&"hubs-plan") && !lean.contains(&"concepts-prepare"));
        let off = rounds(&opts(json!({ "register": "off" })));
        assert!(!off.contains(&"concepts-prepare") && off.contains(&"hubs-plan"));
        let hubs = rounds(&opts(json!({ "mode": "hubs" })));
        assert_eq!(hubs.first(), Some(&"concepts-prepare"));
        assert_eq!(hubs.last(), Some(&"hubs-report"));
        assert!(!hubs.contains(&"plan") && !hubs.contains(&"report"));
    }

    #[test]
    fn a_job_template_names_its_files_and_rule() {
        let o = opts(json!({ "tool": "D:\\t\\zt-digest.exe", "zt_config": "D:\\box\\config" }));
        let g = template("grow", "D:\\w", &o);
        assert_eq!(g["files"], json!(["D:\\w\\instructions.zh.md", "D:\\w\\jobs\\{id}.md"]));
        assert!(s(&g["prompt"]).contains("SELF-CHECK: & 'D:\\t\\zt-digest.exe' grow -Step precheck -Work \"D:\\w\" -Job {id}"));
        assert!(s(&g["approve"]).contains("$env:ZT_CONFIG_DIR='D:\\box\\config'"));
        assert_eq!(g["model"], json!("deepseek-official/deepseek-flash"));
        assert_eq!(g["effort"], json!("high"));
        let r = template("restate", "D:\\w", &o);
        assert_eq!(r["files"], json!(["D:\\w\\restate\\{id}\\work\\dsh-restate-instructions.zh.md", "D:\\w\\restate\\{id}\\work\\rj-01.md"]));
        assert!(s(&r["approve"]).contains("-Staging \"D:\\w\\restate\\{id}\""));
        let c = template("concepts", "D:\\w", &o);
        assert_eq!(c["skills"], json!(["asd-ste100-zh"]));
        assert_eq!(answer_file("grow", "D:\\w", "gw-002"), Some("D:\\w\\out\\gw-002.json".into()));
        assert_eq!(answer_file("plain", "D:\\w", "gh-001"), None);
        assert_eq!(answer_file("concepts", "D:\\w", "c01"), Some("D:\\w\\concepts\\c01\\work\\out\\cj-01.json".into()));
    }

    #[test]
    fn options_must_be_an_object_with_known_choices() {
        assert!(Opts::parse("[]").is_err());
        assert!(Opts::parse("{\"mode\":\"cards\"}").is_err());
        assert!(Opts::parse("{\"register\":\"maybe\"}").is_err());
        let o = opts(json!({ "books": ["a", "b"], "book": "c" }));
        assert_eq!(o.which(), vec!["-Books".to_string(), "a,b".into()]);
        assert_eq!(opts(json!({ "b": 1, "a": 2 })).canon(), opts(json!({ "a": 2, "b": 1 })).canon());
    }

    #[test]
    fn a_pending_job_set_is_confirmed_or_printed_again() {
        let dir = std::env::temp_dir().join(format!("zt-grow-next-{}", std::process::id()));
        let work = dir.to_string_lossy().to_string();
        std::fs::create_dir_all(dir.join("out")).unwrap();
        let o = opts(json!({}));
        let mut m = Memory::load(&work).unwrap();
        let line = jobs_line(&mut m, "ideas-jobs", "grow", vec!["gw-001".into(), "gw-002".into()], &work, &o);
        assert_eq!(line["token"], json!(1));
        assert_eq!(line["sum"], json!(fnv(&sum_text(&["gw-001".into(), "gw-002".into()], &line["job"]))));
        m.save().unwrap();
        // gw-001 answered: a call without -After prints only gw-002 again, under a new token.
        std::fs::write(dir.join("out").join("gw-001.json"), "{}").unwrap();
        let p = m.v["pending"].clone();
        let open: Vec<String> = p["ids"].as_array().unwrap().iter().map(s).filter(|id| answer_file("grow", &work, id).is_none_or(|f| !Path::new(&f).exists())).collect();
        assert_eq!(open, vec!["gw-002".to_string()]);
        m.finish("ideas-jobs", json!({ "jobs": 2 }));
        assert!(m.done("ideas-jobs"));
        m.finish("ideas-jobs", json!({ "jobs": 2 }));
        assert_eq!(m.v["done"].as_array().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
