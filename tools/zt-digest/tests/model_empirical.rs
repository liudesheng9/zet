//! The empirical path of the model compiler (docs/empirical-models-proposal.md §8): an empirical
//! model is briefed with its source tables read by script into tables.json, its plan is checked
//! against the empirical schema, its skeleton writes every number from its table cell, it is
//! reviewed in messages of at most twelve cards, and close sends a number the source does not hold
//! back to the writer. The commands run over a real scratch archive, as model_one_conversation.rs.


use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};

#[path = "support/zt_program.rs"]
mod zt_program;
use zt_program::zt_program;

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


const SECTION: &str = "# 5.2 企业特征\n\n表 2 展示了社交技能与这些代理变量之间的横截面关系。所有回归的标准误均按企业聚类。\n\n表 2：社交集群与企业特征\n\n<table><tr><td colspan=\"3\">Dependent Variable: Social Skill Cluster</td></tr><tr><td></td><td>(1)</td><td>(2)</td></tr><tr><td>Log(Employment)</td><td>0.014***(0.003)</td><td>0.012***(0.003)</td></tr><tr><td>MNE</td><td></td><td>0.047***(0.017)</td></tr><tr><td>Observations</td><td>4,618</td><td>4,618</td></tr></table>\n\n注：\\* p<0.1，\\*\\* p<0.05，\\*\\*\\* p<0.01。所有列均以 OLS 估计。\n\n对数就业的一个标准差变化与纳入社交集群的概率上升 3.5 个百分点相关。跨国公司身份与概率上升 4.7 个百分点相关。\n";

/// A staged book whose group 1 is one section file with the Hansen table.
fn staging(parent: &Path, key: &str) -> PathBuf {
    let dir = parent.join(key);
    std::fs::create_dir_all(dir.join("sections")).expect("make sections");
    std::fs::write(
        dir.join("manifest.json"),
        json!({"citation_key": key, "book_title": "Test Book", "staging": dir.to_string_lossy(), "source_sha256": "0".repeat(64),
               "groups": [{"n": 1, "chapter": "第五章", "file": "sections/s12.md", "sections": ["s12.md"], "headings": ["5.2 企业特征"]}]})
            .to_string(),
    )
    .expect("write manifest");
    std::fs::write(dir.join("sections").join("s12.md"), SECTION).expect("write section");
    // A working paper's tables after its reference list (prepare's back matter): a new label, and
    // a 表 2 that must not displace the section's own table.
    std::fs::write(dir.join("back-matter.zh.md"), BACK).expect("write back matter");
    dir
}

const BACK: &str = "# 参考文献

文献。

表 VI. 任职期

<table><tr><td></td><td>(1)</td></tr><tr><td>测试</td><td>0.10</td></tr></table>

表 2. 另一张

<table><tr><td></td><td>(1)</td></tr><tr><td>x</td><td>9.9</td></tr></table>
";

/// The plan an empirical architect writes (docs/empirical-models-proposal.md §8.3).
fn plan(model: &str) -> Value {
    json!({
        "model": model, "level": "empirical", "design": "cross-section", "estimand": "企业规模与职位描述要求社交技能的关联",
        "title": "高管社交技能需求的横截面估计", "card": "问题：哪些企业更要高管的社交技能。主要估计：表 2 第 (1)、(2) 列。",
        "variables": [["$Social_{ft}$", "社交集群高于中位数", "0/1", "职位描述", "S0"], ["$X_{ft}$", "企业特征", "对数就业等", "Orbis", "S0"],
                      ["$\\beta$", "企业特征的系数", "", "", "S0"], ["$\\varepsilon_{ft}$", "误差", "", "", "S0"]],
        "parts": [
            {"id": "D", "line": "main", "facet": "数据", "title": "数据：4,618 个高管搜寻的职位描述", "spec": "单位、样本、来源", "src": ["sections/s12.md:1-3"]},
            {"id": "V", "src": ["sections/s12.md:7-7"], "line": "side", "of": "D", "kind": "refers", "facet": "变量表", "title": "变量表：本模型的变量", "spec": "变量", "rests": [["D", "变量来自这些数据"]]},
            {"id": "G", "line": "main", "facet": "设计", "title": "设计：企业之间的横截面比较", "spec": "比较与假设", "rests": [["D", "比较这些搜寻"]], "src": ["sections/s12.md:3-3"]},
            {"id": "A1", "src": ["sections/s12.md:3-3"], "line": "side", "of": "G", "kind": "refers", "facet": "假设 1", "title": "假设 1：企业特征先于职位描述", "spec": "时序", "rests": [["G", "设计的识别假设"]]},
            {"id": "S0", "line": "main", "facet": "规格：基准", "title": "规格：社交集群对企业特征的回归", "spec": "式 (7)",
             "rests": [["D", "在这些数据上估计"], ["G", "按这个设计比较"]], "src": ["sections/s12.md:3-3"],
             "equation": "Social_{ft} = \\beta X_{ft} + \\varepsilon_{ft}", "terms": {"outcome": "社交集群虚拟变量", "regressor": "企业特征", "fe": ["行业", "年份"], "controls": [], "sample": "全部搜寻", "estimator": "OLS", "se": "按企业聚类"}},
            {"id": "S1", "line": "side", "of": "S0", "kind": "refers", "facet": "规格：加入跨国公司", "title": "规格：加入跨国公司虚拟变量", "spec": "第 (2) 列", "base": "S0",
             "change": {"kind": "controls", "text": "加入跨国公司虚拟变量。"}, "src": ["sections/s12.md:7-7"]},
            {"id": "C1", "src": ["sections/s12.md:7-7"], "line": "main", "facet": "第 1 章：主要估计", "title": "第 1 章：大企业更要求社交技能", "spec": "表 2 的估计", "rests": [["S0", "这一章的估计都来自基准规格"]]},
            {"id": "E1", "line": "side", "of": "C1", "kind": "refers", "facet": "估计：企业规模", "title": "估计：规模大的企业更常要求社交技能", "spec": "规模系数",
             "emphasis": "core", "specification": "S0", "regressor": "$\\beta$（对数就业）", "cell": {"table": "表 2", "row": "Log(Employment)", "col": "1"},
             "magnitude": "一个标准差，3.5 个百分点", "meaning": "规模代理协调问题的数量", "src": ["sections/s12.md:11-11"]},
            {"id": "E2", "line": "side", "of": "C1", "kind": "refers", "facet": "估计：跨国公司", "title": "估计：跨国公司更常要求社交技能", "spec": "跨国公司系数",
             "specification": "S1", "regressor": "跨国公司虚拟变量的系数", "cell": {"table": "表 2", "row": "MNE", "col": "2"}, "src": ["sections/s12.md:11-11"]},
            {"id": "K1", "src": ["sections/s12.md:3-3"], "line": "side", "of": "E1", "kind": "refers", "facet": "检验：时序", "title": "检验：特征取自搜寻前三年", "spec": "时序", "assumption": "A1", "specification": "S0"}
        ]
    })
}

/// The work folder of one batch, as `brief` leaves it.
fn work(staged: &Path, model: &str, status: &str) -> PathBuf {
    let slug = model.replace(['/', '|', '\\'], "_");
    let dir = staged.join("models").join("m-Key_1");
    std::fs::create_dir_all(dir.join(&slug)).expect("make model folder");
    let state = json!({
        "order": [model],
        "models": {model: {"level": "empirical", "status": status, "ids": {}, "titles": {}, "derive": [], "main": [], "split": 0, "steps": {}}},
        "jobs": {}, "rereview": [], "reviews": [], "seq": 0
    });
    std::fs::write(dir.join("state.json"), serde_json::to_string_pretty(&state).unwrap()).expect("write state");
    dir
}

fn job_text(work: &Path, id: &str) -> String {
    let state = read_json(&work.join("state.json"));
    let file = state["jobs"][id]["file"].as_str().unwrap_or_else(|| panic!("job {id} is not in the state: {state}"));
    std::fs::read_to_string(file).expect("read job file")
}

#[test]
fn an_empirical_model_is_briefed_planned_built_reviewed_and_checked() {
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
        json!({"at": "Key", "kind": "direct", "title": "高管社交技能需求", "body": "问题：哪些企业更要社交技能。", "type": "model", "level": "empirical"}),
    )["address"]
        .as_str()
        .expect("model address")
        .to_string();
    assert_eq!(model, "Key/1");
    let slug = model.replace(['/', '|', '\\'], "_");
    let work = work(&staged, &model, "brief");
    let w = work.to_str().unwrap();
    let st = staged.to_str().unwrap();

    // 1. Brief: the script reads the group's tables into tables.json; the job reads the empirical brief.
    let brief = ok_json(&zt.digest(&["model", "-Step", "brief", "-Work", w, "-Staging", st, "-Models", &model]), "brief");
    let job = &brief["jobs"][0];
    assert!(job["files"][0].as_str().unwrap().ends_with("dsh-empirical-brief-instructions.zh.md"), "{brief}");
    let tables = read_json(&work.join(&slug).join("tables.json"));
    assert_eq!(tables["tables"][0]["label"], "表 2", "{tables}");
    assert_eq!(tables["tables"][0]["file"], "sections/s12.md");
    let labels: Vec<&str> = tables["tables"].as_array().unwrap().iter().map(|t| t["label"].as_str().unwrap()).collect();
    assert_eq!(labels.iter().filter(|l| **l == "表 2").count(), 1, "the section's 表 2 stays the only one: {tables}");
    let back = tables["tables"].as_array().unwrap().iter().find(|t| t["label"] == "表 VI").expect("the back matter's table");
    assert_eq!(back["file"], "back-matter.zh.md");

    // 2. Validate: the empirical plan has no fault; a cell the table does not hold is one.
    let plan_path = work.join(&slug).join("plan.json");
    std::fs::write(&plan_path, serde_json::to_string_pretty(&plan(&model)).unwrap()).unwrap();
    let out = zt.digest(&["model", "-Step", "validate", "-Work", w, "-Plan", plan_path.to_str().unwrap()]);
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("faults: 0"), "{text}");
    let mut bad = plan(&model);
    bad["parts"][7]["cell"]["row"] = json!("Exports");
    let bad_path = work.join(&slug).join("bad-plan.json");
    std::fs::write(&bad_path, serde_json::to_string_pretty(&bad).unwrap()).unwrap();
    let out = zt.digest(&["model", "-Step", "validate", "-Work", w, "-Plan", bad_path.to_str().unwrap()]);
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("E1: cell 表 2 / Exports / 1 is not in tables.json"), "{text}");
    assert!(text.contains("faults: 1"), "{text}");

    // 3. Apply: the skeleton, numbers from cells, the Model Card's script sections, one fill job.
    let fill = ok_json(&zt.digest(&["model", "-Step", "apply", "-Work", w, "-Staging", st]), "apply");
    let jobs = fill["jobs"].as_array().expect("apply jobs");
    assert_eq!(jobs.len(), 1, "{fill}");
    assert_eq!(jobs[0]["continue"], json!(format!("{slug}:brief")));
    let fill_text = job_text(&work, jobs[0]["id"].as_str().unwrap());
    assert!(fill_text.contains("模型卡: [[Key/1]]（实证模型）"), "{fill_text}");
    assert_eq!(fill_text.matches("## 分卡 ").count(), 9, "every part but the 变量表 is the writer's: {fill_text}");
    assert!(fill_text.contains("- 值：0.014。"), "the writer gets the script's numbers: {fill_text}");
    assert!(fill_text.contains("- 量级（架构师的设计）：一个标准差，3.5 个百分点"), "{fill_text}");
    let built = mcp.call("get_model", json!({"at": model}));
    assert_eq!(built["build"]["level"], "empirical");
    assert_eq!(built["build"]["empirical_shape"], json!([]), "{}", built["build"]);
    assert_eq!(built["build"]["estimate_unbound"], json!([]), "{}", built["build"]);
    assert_eq!(built["build"]["test_unbound"], json!([]), "{}", built["build"]);
    assert_eq!(built["build"]["spec_unbound"], json!([]), "{}", built["build"]);
    assert_eq!(built["build"]["variable_uncovered"], json!([]), "{}", built["build"]);
    let state = read_json(&work.join("state.json"));
    let id = |plan_id: &str| state["models"][&model]["ids"][plan_id].as_str().unwrap_or_else(|| panic!("{plan_id}")).to_string();
    let e1 = mcp.call("get_card", json!({"at": id("E1")}));
    let body = e1["body"].as_str().unwrap();
    for line in ["- 值：0.014。", "- 标准误：0.003。", "- 显著性：p < 0.01（表中标 3 颗星）。", "- 样本量：N = 4,618。"] {
        assert!(body.contains(line), "{line} in {body}");
    }
    assert!(body.contains(&format!("依赖 [[{}]]", id("S0"))), "{body}");
    let head = mcp.call("get_card", json!({"at": model}));
    let head_body = head["body"].as_str().unwrap();
    assert!(head_body.contains("**设计**\ncross-section："), "{head_body}");
    assert!(head_body.contains("**主要估计**"), "{head_body}");
    assert!(head_body.contains("| 0.014 | 标准误 0.003 |"), "{head_body}");
    let vars = mcp.call("get_card", json!({"at": id("V")}));
    assert!(vars["body"].as_str().unwrap().contains("| $Social_{ft}$ | 社交集群高于中位数 | 0/1 | 职位描述 | 规格：基准 |"), "{vars}");
    let setting = std::fs::read_to_string(work.join(&slug).join("setting.md")).unwrap();
    assert!(setting.contains("# 本模型（实证模型）") && setting.contains("| Log(Employment) | 0.014***(0.003) | 0.012***(0.003) |"), "{setting}");

    // 4. Review: one evidence review job in the model's conversation, each card with its source lines.
    let review = ok_json(&zt.digest(&["model", "-Step", "review", "-Work", w, "-Staging", st]), "review");
    let jobs = review["jobs"].as_array().expect("review jobs");
    assert_eq!(jobs.len(), 1, "{review}");
    assert!(jobs[0]["files"][0].as_str().unwrap().ends_with("dsh-empirical-review-instructions.zh.md"));
    // The reviewer reads the cards in a conversation of its own, not the writer's (pilot 2026-11-09:
    // a manager took the fallback for a continued job with messages; now the design says so).
    assert!(jobs[0]["continue"].is_null(), "the evidence review is its own conversation: {review}");
    assert!(jobs[0]["then"].is_null(), "ten cards fit one message");
    let review_text = job_text(&work, jobs[0]["id"].as_str().unwrap());
    assert_eq!(review_text.matches("## [[").count(), 10, "{review_text}");

    // 5. Close: a number the source does not hold goes back to the writer as a 修正 note.
    let wrong = body.replace("- 标准误：0.003。", "- 标准误：0.009。").replace("待写\n\n**量级**", "规模大的企业更常要求社交技能。\n\n**量级**");
    mcp.call("edit_card", json!({"at": id("E1"), "body": wrong}));
    let close = ok_json(&zt.digest(&["model", "-Step", "close", "-Work", w, "-Staging", st]), "close");
    let jobs = close["jobs"].as_array().expect("close jobs");
    let notes = jobs
        .iter()
        .map(|j| {
            let file = j["files"].as_array().and_then(|f| f.last()).and_then(Value::as_str).expect("a note file");
            std::fs::read_to_string(file).expect("read note")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(notes.contains("数字核对：这张卡的数字 .009"), "{close}\n{notes}");
}

/// An empirical model with more than twelve cards reviews in messages of twelve (`then`).
#[test]
fn an_evidence_review_of_many_cards_sends_messages_of_twelve() {
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
        json!({"at": "Key", "kind": "direct", "title": "高管社交技能需求", "type": "model", "level": "empirical"}),
    )["address"]
        .as_str()
        .unwrap()
        .to_string();
    let slug = model.replace(['/', '|', '\\'], "_");
    let work = work(&staged, &model, "brief");
    let w = work.to_str().unwrap();
    let st = staged.to_str().unwrap();
    ok_json(&zt.digest(&["model", "-Step", "brief", "-Work", w, "-Staging", st, "-Models", &model]), "brief");
    let mut p = plan(&model);
    for k in 0..5 {
        p["parts"].as_array_mut().unwrap().push(json!({"id": format!("L{k}"), "line": "off", "facet": format!("局限：第 {} 点", k + 1), "title": format!("局限：第 {} 点", k + 1), "spec": "局限", "src": ["sections/s12.md:3-3"]}));
    }
    std::fs::write(work.join(&slug).join("plan.json"), serde_json::to_string_pretty(&p).unwrap()).unwrap();
    ok_json(&zt.digest(&["model", "-Step", "apply", "-Work", w, "-Staging", st]), "apply");
    let review = ok_json(&zt.digest(&["model", "-Step", "review", "-Work", w, "-Staging", st]), "review");
    // Pilot 2026-11-09: six sequential messages took 19 of the model's 35 minutes, and each later message
    // re-read the whole conversation. Every twelve cards are now a job of their own, run side by side.
    let jobs = review["jobs"].as_array().expect("review jobs");
    assert_eq!(jobs.len(), 2, "fifteen cards are two jobs: {review}");
    assert!(jobs.iter().all(|j| j["then"].is_null() && j["continue"].is_null()), "each job is one conversation of one message: {review}");
    let state = read_json(&work.join("state.json"));
    let allow = |j: &Value| std::fs::read_to_string(state["jobs"][j["id"].as_str().unwrap()]["allow"].as_str().unwrap()).unwrap();
    assert_eq!(allow(&jobs[0]).lines().count(), 12, "a job may change only its own twelve cards");
    assert_eq!(allow(&jobs[1]).lines().count(), 3);
    let second = job_text(&work, jobs[1]["id"].as_str().unwrap());
    assert!(second.contains("第 2 组，共 2 组"), "{second}");
}
