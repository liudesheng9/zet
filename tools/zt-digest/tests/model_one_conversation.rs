//! One DeepSeek conversation per model (#194, docs/model-one-conversation-proposal.md §7).
//!
//! The model compiler's content jobs no longer open a conversation per group of three parts: the
//! brief starts the model's conversation, and every later step — fill, derive, review, split, close —
//! lists ONE job per model that continues it, with a fallback to the files of a fresh conversation.
//! A math model may instead review in its own conversation (`-IndependentReview`): its first review
//! round is the new job `<slug>:review`, which the later review rounds continue.
//!
//! The commands run over a real scratch archive: a Model Card is created through `zt mcp`, the run's
//! `state.json` and the architect's `plan.json` are written beside it, and each step's job list is
//! read from the compiler's own line and its state. The test needs a `zt` binary built from this
//! repository (or on PATH): it starts its own service on a temporary archive. `ZT_TEST_ZT` overrides
//! the binary.

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

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read artifact")).expect("JSON")
}

/// The plan a Sonnet architect writes: seven parts the fill round writes, then a chain of two
/// Derivation parts. `F3` states no premise, so the build reports it `unanchored` and `close` has a
/// note to send back.
fn plan(model: &str) -> Value {
    json!({
        "model": model,
        "level": "formal",
        "card": "问题：一个买者如何选择数量。",
        "symbols": [["$x$", "数量", "本卡"], ["$p$", "价格", "本卡"]],
        "terms": [["均衡", "平衡"]],
        "parts": [
            {"id":"F1","facet":"设定","title":"设定","spec":"买者的效用和预算。","line":"main","src":["sections/s01.md:1-4"]},
            {"id":"F2","facet":"均衡","title":"均衡数量","spec":"买者选择的数量使边际效用等于价格。","line":"main","rests":[["F1","用到设定"]],"src":["sections/s01.md:5-8"]},
            {"id":"F3","facet":"均衡","title":"比较静态","spec":"价格上升时数量下降。","line":"main","src":["sections/s01.md:9-12"]},
            {"id":"F4","facet":"均衡","title":"预算约束","spec":"支出不超过收入。","line":"main","rests":[["F1","用到设定"]],"src":["sections/s01.md:13-16"]},
            {"id":"F5","facet":"均衡","title":"最优条件","spec":"边际效用等于价格。","line":"main","rests":[["F4","用到预算"]],"src":["sections/s01.md:17-20"]},
            {"id":"F6","facet":"均衡","title":"唯一性","spec":"最优数量唯一。","line":"main","rests":[["F2","用到均衡"]],"src":["sections/s01.md:21-24"]},
            {"id":"F7","facet":"旁注","title":"数值例子","spec":"价格为一时的数量。","line":"side","of":"F2","src":["sections/s01.md:25-28"]},
            {"id":"D1","facet":"推导：均衡","title":"推导均衡条件","line":"main","derive":true,"from":"效用函数","to":"$$u'(x)=p$$","rests":[["F2","用到均衡"]],"src":["sections/s01.md:29-36"]},
            {"id":"D2","facet":"推导：结果","title":"推导数量","line":"main","derive":true,"to":"$$x=p^{-1}$$","src":["sections/s01.md:37-44"]}
        ],
        "explain": [
            {"chain": ["D1", "D2"], "route": "对效用函数求导得到一阶条件，再由一阶条件解出数量。",
             "parts": [
                {"id": "D1", "shows": "一阶条件把数量与价格连起来", "notes": "第 1 步对效用函数求导", "meaning": "均衡数量由边际效用等于价格决定"},
                {"id": "D2", "shows": "由一阶条件解出数量", "notes": "", "meaning": "价格上升时数量下降"}
             ]}
        ]
    })
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
    std::fs::write(dir.join("sections").join("s01.md"), "买者的效用是 $u(x)$。\n均衡满足 $u'(x)=p$。\n").expect("write section");
    dir
}

/// The work folder of one batch, as `brief` leaves it: the state's order and the model's status.
fn work(staged: &Path, model: &str, status: &str) -> PathBuf {
    let slug = model.replace(['/', '|', '\\'], "_");
    let dir = staged.join("models").join("m-Key_1_a");
    std::fs::create_dir_all(dir.join(&slug)).expect("make model folder");
    let state = json!({
        "order": [model],
        "models": {model: {"level": "formal", "status": status, "ids": {}, "titles": {}, "derive": [], "main": [], "split": 0, "steps": {}}},
        "jobs": {}, "rereview": [], "reviews": [], "seq": 0
    });
    std::fs::write(dir.join("state.json"), serde_json::to_string_pretty(&state).unwrap()).expect("write state");
    std::fs::write(dir.join(&slug).join("plan.json"), serde_json::to_string_pretty(&plan(model)).unwrap()).expect("write plan");
    dir
}

fn state_of(work: &Path) -> Value {
    read_json(&work.join("state.json"))
}

fn set_state(work: &Path, value: &Value) {
    std::fs::write(work.join("state.json"), serde_json::to_string_pretty(value).unwrap()).expect("write state");
}

fn job_file(work: &Path, id: &str) -> String {
    let state = state_of(work);
    let file = state["jobs"][id]["file"].as_str().unwrap_or_else(|| panic!("job {id} is not in the state: {state}"));
    std::fs::read_to_string(file).expect("read job file")
}

/// The newest `review-*.md` the review step wrote for the model.
fn review_file(work: &Path, slug: &str) -> String {
    let dir = work.join(slug);
    let mut found: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(&dir).expect("model folder") {
        let path = entry.expect("dir entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        if let Some(n) = name.strip_prefix("review-").and_then(|rest| rest.strip_suffix(".md")).and_then(|n| n.parse::<u64>().ok()) {
            if found.as_ref().map(|(was, _)| n > *was).unwrap_or(true) {
                found = Some((n, path));
            }
        }
    }
    std::fs::read_to_string(found.unwrap_or_else(|| panic!("no review job file in {dir:?}")).1).expect("read review file")
}

#[test]
fn every_content_job_of_a_model_continues_its_brief() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let staged = staging(zt._dir.path(), "Key");
    let mut mcp = zt.mcp();
    mcp.call("create_literature", json!({"bibtex": "@book{Key, title={Test Book}}", "body": "一本测试书。"}));
    let model = mcp.call(
        "create_successor",
        json!({"at": "Key", "kind": "direct", "title": "买者模型", "body": "问题：买者如何选择数量。", "type": "model", "level": "formal"}),
    )["address"]
        .as_str()
        .expect("model address")
        .to_string();
    assert!(model.starts_with("Key/"), "the model's address: {model}");
    let work = work(&staged, &model, "brief");
    let slug = model.replace(['/', '|', '\\'], "_");

    // 1. Fill: ONE job for the model holding the Model Card and all seven parts, continuing its brief.
    let fill = ok_json(&zt.digest(&["model", "-Step", "apply", "-Work", work.to_str().unwrap(), "-Staging", staged.to_str().unwrap()]), "apply");
    let jobs = fill["jobs"].as_array().expect("apply jobs");
    assert_eq!(jobs.len(), 1, "apply lists one fill job per model: {fill}");
    assert!(jobs[0]["id"].as_str().unwrap_or_default().starts_with(&format!("{slug}:f")), "the fill job is the model's: {fill}");
    assert_eq!(jobs[0]["continue"], json!(format!("{slug}:brief")), "the fill job continues the model's conversation");
    assert_eq!(jobs[0]["fallback"].as_array().map(Vec::len), Some(3), "every continued job has a fallback");
    let text = job_file(&work, jobs[0]["id"].as_str().expect("fill job id"));
    assert_eq!(text.matches("## 分卡 ").count(), 7, "the fill job holds all seven fill parts: {text}");
    assert!(text.contains(&format!("## 模型卡：[[{model}]]")), "the fill job holds the Model Card too");
    let state = state_of(&work);
    let addresses = state["jobs"][jobs[0]["id"].as_str().unwrap()]["addresses"].as_array().expect("fill addresses").len();
    assert_eq!(addresses, 8, "the job's state entry names the Model Card and seven parts");

    // 2. Derive: ONE job per model holding both chains' parts, continuing the same conversation.
    let derive = ok_json(&zt.digest(&["model", "-Step", "derive", "-Work", work.to_str().unwrap(), "-Staging", staged.to_str().unwrap()]), "derive");
    let jobs = derive["jobs"].as_array().expect("derive jobs");
    assert_eq!(jobs.len(), 1, "derive lists one job per model: {derive}");
    assert_eq!(jobs[0]["continue"], json!(format!("{slug}:brief")));
    assert_eq!(jobs[0]["fallback"].as_array().map(Vec::len), Some(3));
    let text = job_file(&work, jobs[0]["id"].as_str().expect("derive job id"));
    assert_eq!(text.matches("## 分卡 ").count(), 2, "the derive job holds both Derivation parts: {text}");
    assert!(text.contains("推导均衡条件") && text.contains("推导数量"));

    // 3. Review: one job per model, continuing the model's conversation, and its job file tells the
    //    reviewer to re-read the stored Cards from the archive instead of trusting its memory.
    let review = ok_json(&zt.digest(&["model", "-Step", "review", "-Work", work.to_str().unwrap(), "-Staging", staged.to_str().unwrap()]), "review");
    let jobs = review["jobs"].as_array().expect("review jobs");
    assert_eq!(jobs.len(), 1, "review lists one job per model: {review}");
    assert_eq!(jobs[0]["continue"], json!(format!("{slug}:brief")));
    assert_eq!(jobs[0]["fallback"].as_array().map(Vec::len), Some(3));
    let text = review_file(&work, &slug);
    assert!(text.contains("get_card") && text.contains("不要凭记忆复核"), "the reviewer re-reads the archive: {text}");
    assert!(text.contains(&format!("[[{model}|")), "the job names the stored Derivation Cards: {text}");

    // 4. Split: the follow-up continues the model's conversation with a fallback.
    let f3 = state_of(&work)["models"][&model]["ids"]["F3"].as_str().expect("F3 address").to_string();
    let split = ok_json(
        &zt.digest(&["model", "-Step", "split", "-Work", work.to_str().unwrap(), "-Staging", staged.to_str().unwrap(), "-Need", &format!("{f3}=2")]),
        "split",
    );
    let jobs = split["jobs"].as_array().expect("split jobs");
    assert_eq!(jobs.len(), 1, "split lists the model's one follow-up: {split}");
    assert_eq!(jobs[0]["continue"], json!(format!("{slug}:brief")), "a split follow-up continues the model's conversation");
    assert_eq!(jobs[0]["fallback"].as_array().map(Vec::len), Some(4));

    // The writer re-writes the Model Card without the compiler's 结构 list before the close (see step 6).
    let mut mcp = zt.mcp();
    mcp.call("edit_card", json!({"at": model, "body": "**问题**\n\n买者如何选择数量。\n\n**概要**\n\n买者按价格选数量。\n\n**来源**\n\n第一章。"}));
    drop(mcp);

    // 5. Close: the unanchored part goes back to the model's conversation, never to `unowned`.
    let close = ok_json(&zt.digest(&["model", "-Step", "close", "-Work", work.to_str().unwrap(), "-Staging", staged.to_str().unwrap()]), "close");
    assert!(close["faults"].as_u64().unwrap_or(0) > 0, "the close round sees the unanchored part: {close}");
    assert_eq!(close["unowned"], json!([]), "a Card of a model of this batch is never unowned: {close}");
    let jobs = close["jobs"].as_array().expect("close jobs");
    assert_eq!(jobs.len(), 1, "close lists one follow-up per model conversation: {close}");
    assert_eq!(jobs[0]["continue"], json!(format!("{slug}:brief")));
    assert_eq!(jobs[0]["fallback"].as_array().map(Vec::len), Some(4));

    // 6. #197 pilot: the model's one fill job writes the Model Card too, and a writer that re-wrote the head
    //    without the compiler's 结构 list made model-finish refuse the model. report puts the list back from
    //    the plan and the part map, split pieces included, before the finish reads it.
    ok_json(&zt.digest(&["model", "-Step", "report", "-Work", work.to_str().unwrap(), "-Staging", staged.to_str().unwrap()]), "report");
    let head = zt.mcp().call("get_card", json!({"at": model}));
    let body = head["body"].as_str().unwrap_or_default().to_string();
    let structure = body.split("**结构**").nth(1).unwrap_or_default().to_string();
    let ids = state_of(&work)["models"][&model]["ids"].clone();
    for (id, address) in ids.as_object().expect("the part map") {
        let address = address.as_str().unwrap_or_default();
        assert!(structure.contains(&format!("[[{address}]]")), "the 结构 list names part {id} {address}: {body}");
    }
    assert!(body.contains("买者按价格选数量"), "the writer's own sections stay: {body}");
}

#[test]
fn the_math_variant_reviews_in_its_own_conversation() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let staged = staging(zt._dir.path(), "Key");
    {
        let mut mcp = zt.mcp();
        mcp.call("create_literature", json!({"bibtex": "@book{Key, title={Test Book}}", "body": "一本测试书。"}));
        mcp.call(
            "create_successor",
            json!({"at": "Key", "kind": "direct", "title": "买者模型", "body": "问题：买者如何选择数量。", "type": "model", "level": "formal"}),
        );
    }
    let model = "Key/1".to_string();
    let work = work(&staged, &model, "brief");
    let slug = model.replace(['/', '|', '\\'], "_");
    ok_json(&zt.digest(&["model", "-Step", "apply", "-Work", work.to_str().unwrap(), "-Staging", staged.to_str().unwrap()]), "apply");
    ok_json(&zt.digest(&["model", "-Step", "derive", "-Work", work.to_str().unwrap(), "-Staging", staged.to_str().unwrap()]), "derive");

    // The first round is a new conversation named `<slug>:review`; its job file carries the bodies.
    let first = ok_json(
        &zt.digest(&["model", "-Step", "review", "-Work", work.to_str().unwrap(), "-Staging", staged.to_str().unwrap(), "-IndependentReview"]),
        "review",
    );
    let jobs = first["jobs"].as_array().expect("review jobs");
    assert_eq!(jobs.len(), 1, "the independent review lists one job per model: {first}");
    assert_eq!(jobs[0]["id"], json!(format!("{slug}:review")));
    assert!(jobs[0]["continue"].is_null(), "the first independent round starts a new conversation: {first}");
    let text = review_file(&work, &slug);
    assert!(text.contains("## 推导分卡"), "a new conversation gets the Card bodies in its job file: {text}");

    // A later round continues `<slug>:review` with a fallback.
    let mut state = state_of(&work);
    state["rereview"] = json!([format!("{model}#D1")]);
    set_state(&work, &state);
    let second = ok_json(
        &zt.digest(&["model", "-Step", "review", "-Work", work.to_str().unwrap(), "-Staging", staged.to_str().unwrap(), "-IndependentReview", "-Changed"]),
        "review 2",
    );
    let jobs = second["jobs"].as_array().expect("review jobs");
    assert_eq!(jobs.len(), 1, "a later round lists one job per model: {second}");
    assert_eq!(jobs[0]["continue"], json!(format!("{slug}:review")), "a later round continues the reviewer's conversation");
    assert_eq!(jobs[0]["fallback"].as_array().map(Vec::len), Some(3));
}







