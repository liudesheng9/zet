//! Explanations written in the model's own conversation (#195, docs/model-one-conversation-
//! proposal.md §3.7 and §7): the Sonnet architect designs every chain of Derivation parts in
//! `plan.json` (`explain`: route, shows, notes, meaning), the batch's explain job is ONE job per
//! model that continues the model's conversation, `finish-batch` completes the Explain unit of a
//! model whose explanations the batch wrote, and mode `explain` plans one job per model.
//!
//! The commands run over a real scratch archive, as model_one_conversation.rs does: a Model Card is
//! created through `zt mcp`, the run's `state.json` and the architect's `plan.json` are written
//! beside it, and each step's job list is read from the compiler's own line and its state.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};

fn zt_program() -> Option<PathBuf> {
    if let Ok(program) = std::env::var("ZT_TEST_ZT") {
        return Some(PathBuf::from(program));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for candidate in [root.join("target/debug/zt.exe"), root.join("target/release/zt.exe")] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    Command::new("zt")
        .arg("version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| PathBuf::from("zt"))
}

struct Zt {
    _dir: tempfile::TempDir,
    config: PathBuf,
    archive: PathBuf,
    program: PathBuf,
}

impl Zt {
    fn new(program: PathBuf) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("config");
        let archive = dir.path().join("archive");
        let zt = Self { _dir: dir, config, archive, program };
        let configured = zt.run(&["config", "set", "archive_root", zt.archive.to_str().unwrap()]);
        assert!(configured.status.success(), "config: {}", String::from_utf8_lossy(&configured.stderr));
        let up = zt.run(&["up"]);
        assert!(
            up.status.success(),
            "up: {} {}",
            String::from_utf8_lossy(&up.stdout),
            String::from_utf8_lossy(&up.stderr)
        );
        zt
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.program)
            .args(args)
            .env("ZT_CONFIG_DIR", &self.config)
            .env("ZT_GUI_PORT", "0")
            .output()
            .expect("run zt")
    }

    fn mcp(&self) -> Mcp {
        let mut child = Command::new(&self.program)
            .arg("mcp")
            .env("ZT_CONFIG_DIR", &self.config)
            .env("ZT_GUI_PORT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start zt mcp");
        let stdin = child.stdin.take().expect("mcp stdin");
        let stdout = BufReader::new(child.stdout.take().expect("mcp stdout"));
        let mut mcp = Mcp { child, stdin, stdout, next: 1 };
        mcp.request(
            "initialize",
            json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"zt-digest-tests","version":"1"}}),
        );
        mcp.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        mcp
    }

    fn digest(&self, args: &[&str]) -> Output {
        let program = std::env::var("ZT_TEST_DIGEST").unwrap_or_else(|_| env!("CARGO_BIN_EXE_zt-digest").to_string());
        Command::new(program)
            .args(args)
            .env("ZT_CONFIG_DIR", &self.config)
            .env("ZT_GUI_PORT", "0")
            .env("ZT_CHECK_ZT", &self.program)
            .output()
            .expect("run zt-digest")
    }

    fn page(&self, tool: &str, arguments: Value) -> Value {
        let mut mcp = self.mcp();
        mcp.call(tool, arguments)
    }
}

impl Drop for Zt {
    fn drop(&mut self) {
        let _ = self.run(&["down"]);
    }
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Mcp {
    fn send(&mut self, value: &Value) {
        writeln!(self.stdin, "{value}").expect("write mcp");
        self.stdin.flush().expect("flush mcp");
    }

    fn recv(&mut self) -> Value {
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("read mcp");
            assert!(read > 0, "zt mcp closed its stdout");
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                return value;
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        loop {
            let value = self.recv();
            if value["id"] == json!(id) {
                if !value["error"].is_null() {
                    panic!("mcp {method} failed: {value}");
                }
                return value["result"].clone();
            }
        }
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let result = self.request("tools/call", json!({"name":tool,"arguments":arguments}));
        if result["isError"] == json!(true) {
            panic!("{tool} was refused: {result}");
        }
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{tool} returned no text: {result}"));
        serde_json::from_str(text).unwrap_or_else(|error| panic!("{tool}: {error}: {text}"))
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn last_json(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| panic!("no output: {}", String::from_utf8_lossy(&output.stderr)));
    serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}"))
}

fn ok_json(output: &Output, step: &str) -> Value {
    assert!(
        output.status.success(),
        "{step}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    last_json(output)
}

fn text_of(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read artifact")).expect("JSON")
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(value).unwrap()).expect("write JSON");
}

/// The plan a Sonnet architect writes: three fill parts, then a chain of two Derivation parts.
/// `with_design` adds the `explain` entry of §3.7 for the chain.
fn plan(model: &str, with_design: bool) -> Value {
    let mut plan = json!({
        "model": model,
        "level": "formal",
        "card": "问题：企业能否融到 $K-w$。",
        "symbols": [["$L$", "清算价值", "S"], ["$P$", "还款额", "S"]],
        "terms": [["清算", "卖资产", "变卖资产"]],
        "parts": [
            {"id":"S","facet":"设定","title":"设定：企业家、投资者与清算价值","line":"main","spec":"参与者与时点","src":["sections/s01.md:1-2"]},
            {"id":"A","facet":"假设","title":"假设：投资者只认清算价值","line":"main","spec":"假设","rests":[["S","用到 $L$"]],"src":["sections/s01.md:2-2"]},
            {"id":"D1","facet":"推导：还款上限","title":"推导：还款不超过清算价值","line":"main","derive":true,"from":"(5.5)","to":"$$P\\le L$$","rests":[["A","用到假设"]],"src":["sections/s01.md:3-3"]},
            {"id":"D2","facet":"推导：融资条件","title":"推导：得到融资条件","line":"main","derive":true,"to":"$$y_1+fy_2>K$$","src":["sections/s01.md:4-4"]},
            {"id":"P","facet":"命题","title":"命题：清算价值够大才能融资","line":"main","spec":"命题","rests":[["D2","用到融资条件"]],"src":["sections/s01.md:4-4"]}
        ]
    });
    if with_design {
        plan["explain"] = json!([
            {
                "chain": ["D1", "D2"],
                "route": "先求还款上限，再把上限代入参与约束；直接比较还款与项目价值看不出上限。",
                "parts": [
                    {"id":"D1","shows":"还款不超过清算价值","notes":"第 2 步分情形讨论清算与不清算","meaning":"还款上限由清算价值决定"},
                    {"id":"D2","shows":"项目价值减去还款后仍超过成本","notes":"","meaning":"项目价值超过成本时投资者才参与"}
                ]
            }
        ]);
    }
    plan
}

/// A staged book folder: the run's manifest and one section file (the source review's spans).
fn staging(parent: &Path, key: &str) -> PathBuf {
    let dir = parent.join(key);
    std::fs::create_dir_all(dir.join("sections")).expect("make sections");
    std::fs::write(
        dir.join("manifest.json"),
        json!({"citation_key": key, "book_title": "Test Book", "staging": dir.to_string_lossy(), "source_sha256": "0".repeat(64)})
            .to_string(),
    )
    .expect("write manifest");
    std::fs::write(
        dir.join("sections").join("s01.md"),
        "企业家有财富 $w$，项目需要 $K$。\n清算价值为 $L$。\n还款 $P\\le L$。\n于是 $y_1+fy_2>K$。\n",
    )
    .expect("write section");
    dir
}

/// The work folder of one batch, as `brief` leaves it: the state's order and the model's status.
fn work(staged: &Path, model: &str, with_design: bool) -> PathBuf {
    let slug = model.replace(['/', '|', '\\'], "_");
    let dir = staged.join("models").join("m-Key_1_a");
    std::fs::create_dir_all(dir.join(&slug)).expect("make model folder");
    let state = json!({
        "order": [model],
        "models": {model: {"level": "formal", "status": "brief", "ids": {}, "titles": {}, "derive": [], "main": [], "split": 0, "steps": {}}},
        "jobs": {}, "rereview": [], "reviews": [], "seq": 0
    });
    write_json(&dir.join("state.json"), &state);
    write_json(&dir.join(&slug).join("plan.json"), &plan(model, with_design));
    dir
}

fn state_of(work: &Path) -> Value {
    read_json(&work.join("state.json"))
}

fn set_state(work: &Path, value: &Value) {
    write_json(&work.join("state.json"), value);
}

fn job_text(work: &Path, id: &str) -> String {
    let state = state_of(work);
    let file = state["jobs"][id]["file"]
        .as_str()
        .or_else(|| state["explain"]["jobs"][id]["file"].as_str())
        .unwrap_or_else(|| panic!("job {id} is not in the state: {state}"));
    std::fs::read_to_string(file).expect("read job file")
}

fn model_card(zt: &Zt, key: &str) -> String {
    zt.page("create_literature", json!({"bibtex": format!("@book{{{key}, title={{Test Book}}}}"), "body": "一本测试书。"}));
    zt.page(
        "create_successor",
        json!({"at": key, "kind": "direct", "title": "融资模型", "body": "问题：企业能否融到 K-w。", "type": "model", "level": "formal"}),
    )["address"]
        .as_str()
        .expect("model address")
        .to_string()
}

#[test]
fn the_batch_explain_job_is_one_per_model_continuing_its_conversation_from_the_design() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let staged = staging(zt._dir.path(), "Key");
    let model = model_card(&zt, "Key");
    let work = work(&staged, &model, true);
    let slug = model.replace(['/', '|', '\\'], "_");
    let work_arg = work.to_str().unwrap().to_string();
    let staged_arg = staged.to_str().unwrap().to_string();
    let step = |name: &'static str| ["model", "-Step", name, "-Work", work_arg.as_str(), "-Staging", staged_arg.as_str()];

    ok_json(&zt.digest(&["model", "-Step", "brief", "-Work", work_arg.as_str(), "-Staging", staged_arg.as_str(), "-Models", &model]), "brief");
    ok_json(&zt.digest(&step("apply")), "apply");
    let ids = state_of(&work)["models"][&model]["ids"].clone();
    let at = |id: &str| ids[id].as_str().unwrap_or_else(|| panic!("no part {id}")).to_string();
    let (s, a, d1, d2, p) = (at("S"), at("A"), at("D1"), at("D2"), at("P"));
    let head = zt.page("get_card", json!({"at": model}));
    let structure = head["body"].as_str().unwrap_or_default().split("**结构**").nth(1).unwrap_or_default();
    let mut mcp = zt.mcp();
    mcp.call("edit_card", json!({"at": model, "body": format!("问题：企业能否融到 $K-w$。机制：清算价值决定还款上限。结论：清算价值够大才能融资。\n\n来源：第一章。\n\n**结构**{structure}")}));
    mcp.call("edit_card", json!({"at": s, "body": "企业家有财富 $w$，项目需要 $K$，清算价值为 $L$。"}));
    mcp.call("edit_card", json!({"at": a, "body": format!("投资者只认清算价值。依赖 [[{s}]]：用到 $L$。")}));
    mcp.call("edit_card", json!({"at": d1, "body": format!("**起点**\n- 依赖 [[{a}]] 的结果：投资者只认清算价值。\n\n**步骤**\n1. 还款不超过清算价值。\n\n**结果**\n$$P\\le L$$")}));
    mcp.call("edit_card", json!({"at": d2, "body": format!("**起点**\n- 依赖上一步 [[{d1}]] 的结果：还款不超过清算价值。\n\n**步骤**\n1. 项目价值减去还款后仍超过成本。\n\n**结果**\n$$y_1+fy_2>K$$")}));
    mcp.call("edit_card", json!({"at": p, "body": format!("清算价值够大，项目才能融资。依赖 [[{d2}]] 的结果。\n\n来源：第一章。")}));
    drop(mcp);
    ok_json(&zt.digest(&step("report")), "report");
    ok_json(&zt.digest(&step("derive")), "derive");
    let brief_context = |w: &Path| std::fs::read(w.join(&slug).join("context.md")).expect("the brief's context pack");
    let before = brief_context(&work);

    // §3.7: the batch's explain round is ONE job for the model, continuing its conversation,
    // carrying the architect's design of the chain.
    let explain = ok_json(
        &zt.digest(&["model", "-Step", "explain", "-Work", work_arg.as_str(), "-Staging", staged_arg.as_str(), "-Models", &model, "-Batch"]),
        "explain",
    );
    // #197 pilot: model-finish judges the source review against the brief's context pack, so the
    // explain round inside the batch must not rewrite it.
    assert!(brief_context(&work) == before, "the explain round keeps the brief's context pack byte for byte");
    assert_eq!(explain["parts"].as_f64(), Some(2.0), "the job covers both unexplained Derivation parts: {explain}");
    let jobs = explain["jobs"].as_array().expect("explain jobs");
    assert_eq!(jobs.len(), 1, "one explain job for the model: {explain}");
    let id = jobs[0]["id"].as_str().expect("explain job id").to_string();
    assert!(id.starts_with(&format!("{slug}:x")), "the explain job is the model's: {explain}");
    assert_eq!(jobs[0]["continue"], json!(format!("{slug}:brief")), "the explain job continues the model's conversation: {explain}");
    assert_eq!(jobs[0]["fallback"].as_array().map(Vec::len), Some(3), "a continued job has its fallback files: {explain}");
    let text = job_text(&work, &id);
    assert!(text.contains("全链路线：先求还款上限"), "the job carries the chain's route from the design: {text}");
    assert!(text.contains(&format!("[[{d1}]] 还款不超过清算价值")) && text.contains("需要说明的步骤：第 2 步分情形讨论清算与不清算"), "the job carries the part's shows and notes: {text}");
    assert!(text.contains(&format!("[[{d2}]]")), "the job names every part of the chain: {text}");
    assert!(text.contains("结果的解读：项目价值超过成本时投资者才参与"), "the job carries the part's meaning: {text}");
    assert!(text.contains("第 1 张，共 2 张") && text.contains("第 2 张，共 2 张"), "the script writes the chain position line: {text}");
    assert!(text.contains(&format!("## 推导分卡 [[{d1}]]")), "the job holds the part's body: {text}");

    // Mode `explain` (a book built earlier) plans one job per model too, but starts its own
    // conversation: there is no model job to continue and no design entry in its plan.
    let explain_work = staged.join("explain");
    let plain = ok_json(
        &zt.digest(&["model", "-Step", "explain", "-Work", explain_work.to_str().unwrap(), "-Staging", staged_arg.as_str(), "-Models", &model]),
        "explain",
    );
    let jobs = plain["jobs"].as_array().expect("explain jobs");
    assert_eq!(jobs.len(), 1, "mode explain plans one job per model: {plain}");
    assert!(jobs[0]["continue"].is_null(), "mode explain starts a new conversation: {plain}");
    let plain_text = std::fs::read_to_string(
        state_of(&explain_work)["explain"]["jobs"][jobs[0]["id"].as_str().unwrap()]["file"].as_str().unwrap(),
    )
    .expect("read the mode explain job");
    assert!(!plain_text.contains("全链路线"), "a mode explain job needs no design entry: {plain_text}");

    // A refused part goes back to the same conversation. The first answer writes nothing for the
    // part it leaves empty and D2 is written; the follow-ups continue the explain job.
    let out = |work: &Path, id: &str| state_of(work)["explain"]["jobs"][id]["out"].as_str().expect("result file").to_string();
    write_json(
        Path::new(&out(&work, &id)),
        &json!([
            {"address": d1, "idea": "", "meaning": ""},
            {"address": d2, "idea": "把还款上限代入参与约束，得到融资条件。", "notes": {}, "meaning": "项目价值超过成本时投资者参与。"}
        ]),
    );
    let applied = ok_json(&zt.digest(&step("explain-apply")), "explain-apply");
    assert_eq!(applied["applied"], json!(1), "only the answered part is written: {applied}");
    let follow = applied["jobs"].as_array().expect("fix jobs");
    assert_eq!(follow.len(), 1, "the refused part gets a follow-up in the same conversation: {applied}");
    assert_eq!(follow[0]["continue"], json!(id), "the follow-up continues the explain job: {applied}");
    let round2 = out(&work, &id);
    // Round 2 and round 3: each refusal comes back once more, then the part is refused.
    for round in 2..=3 {
        write_json(Path::new(&round2), &json!([{"address": d1, "idea": "", "meaning": ""}]));
        let applied = ok_json(&zt.digest(&step("explain-apply")), &format!("explain-apply round {round}"));
        let follow = applied["jobs"].as_array().expect("fix jobs");
        if round == 2 {
            assert_eq!(follow.len(), 1, "the second refusal gets the last follow-up: {applied}");
            assert_eq!(follow[0]["id"], json!(format!("{id}+3")), "the third round continues the same job: {applied}");
            continue;
        }
        assert!(follow.is_empty() && applied["refused"].as_array().is_some_and(|r| r.len() == 1), "after three rounds the part is refused, not retried: {applied}");
    }
    // A new explain round plans the still-unexplained part again and writes it.
    let again = ok_json(
        &zt.digest(&["model", "-Step", "explain", "-Work", work_arg.as_str(), "-Staging", staged_arg.as_str(), "-Models", &model, "-Batch"]),
        "explain",
    );
    let jobs = again["jobs"].as_array().expect("explain jobs");
    assert_eq!(jobs.len(), 1, "the unexplained part is planned again: {again}");
    let id2 = jobs[0]["id"].as_str().expect("explain job id").to_string();
    write_json(
        Path::new(&out(&work, &id2)),
        &json!([{"address": d1, "idea": "先求还款上限，再代入参与约束。", "notes": {}, "meaning": "还款上限由清算价值决定。"}]),
    );
    let applied = ok_json(&zt.digest(&step("explain-apply")), "explain-apply");
    assert_eq!(applied["applied"], json!(1), "the rewritten part is applied: {applied}");
    let card = zt.page("get_card", json!({"at": d1}));
    let body = card["body"].as_str().unwrap_or_default();
    assert!(body.contains("**思路**") && body.contains("**解读**") && body.contains("第 1 张，共 2 张"), "the applied part carries 思路, 解读 and the position line: {body}");
}

#[test]
fn a_chain_of_derivation_parts_needs_the_architects_explain_entry() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let staged = staging(zt._dir.path(), "Key");
    let model = model_card(&zt, "Key");
    let work = work(&staged, &model, false);
    let work_arg = work.to_str().unwrap().to_string();
    let staged_arg = staged.to_str().unwrap().to_string();
    let args = |step: &'static str| ["model", "-Step", step, "-Work", work_arg.as_str(), "-Staging", staged_arg.as_str()];

    // The brief writes the context pack the source review is judged against.
    ok_json(&zt.digest(&args("brief")), "brief");

    // plan-status: the chain has no design, so the plan is a fault, not a gap the writer fills alone.
    let status = ok_json(&zt.digest(&args("plan-status")), "plan-status");
    assert_eq!(status["plans"][0]["status"], json!("failed"), "a plan whose chain has no explain entry is a plan fault: {status}");
    assert_eq!(status["faults"], json!(1), "the line counts the faulty plan: {status}");
    let fault = status["plans"][0]["fault"].as_str().unwrap_or_default();
    assert!(fault.contains("explain"), "the fault names the missing entry: {status}");

    // The architect's own check says the same before the batch builds.
    let plan_file = work
        .join(model.replace(['/', '|', '\\'], "_"))
        .join("plan.json");
    let checked = zt.digest(&["model", "-Step", "validate", "-Work", work_arg.as_str(), "-Staging", staged_arg.as_str(), "-Plan", plan_file.to_str().unwrap()]);
    let lines = text_of(&checked);
    assert!(lines.contains("explain") && lines.contains("faults: 1"), "validate reports the missing entry: {lines}");

    // With the design the same plan is planned and validates clean.
    write_json(&plan_file, &plan(&model, true));
    let status = ok_json(&zt.digest(&args("plan-status")), "plan-status");
    assert_eq!(status["plans"][0]["status"], json!("planned"), "the designed plan is planned: {status}");
    assert_eq!(status["faults"], json!(0), "the designed plan has no fault: {status}");
    let lines = text_of(&zt.digest(&["model", "-Step", "validate", "-Work", work_arg.as_str(), "-Staging", staged_arg.as_str(), "-Plan", plan_file.to_str().unwrap()]));
    assert!(lines.trim_end().ends_with("faults: 0"), "the designed plan validates clean: {lines}");
}
