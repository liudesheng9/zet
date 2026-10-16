//! Mathematical units: a Model Card states a theory, definition, axiom, or
//! result (its Math form); proofs cite earlier statements, and get_model reports
//! each unit's attention, citations, and study order, and each theory's order.

#![allow(dead_code)]

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command as StdCommand, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

struct TestHome {
    _tmp: TempDir,
    config_dir: std::path::PathBuf,
    archive_root: std::path::PathBuf,
}

impl TestHome {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config_dir = tmp.path().join("config");
        let archive_root = tmp.path().join("archive");
        let home = Self {
            _tmp: tmp,
            config_dir,
            archive_root,
        };
        home.cmd()
            .args([
                "config",
                "set",
                "archive_root",
                home.archive_root.to_str().unwrap(),
            ])
            .assert()
            .success();
        home.cmd().arg("up").assert().success();
        home
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    fn facet(&self, address: &str) -> Option<String> {
        Connection::open(self.archive_root.join("zt.sqlite3"))
            .expect("open db")
            .query_row(
                "SELECT model_facet FROM cards WHERE location = ?1",
                [address],
                |row| row.get(0),
            )
            .expect("card facet")
    }

    fn mcp(&self) -> Mcp {
        let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
            .arg("mcp")
            .env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start zt mcp");
        let stdin = child.stdin.take().expect("mcp stdin");
        let stdout = BufReader::new(child.stdout.take().expect("mcp stdout"));
        let mut mcp = Mcp {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        mcp.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "zt-tests", "version": "1" } }),
        );
        mcp.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        mcp
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.archive_root.join("zt.pid"));
        thread::sleep(Duration::from_millis(80));
    }
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Mcp {
    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").expect("write MCP message");
        self.stdin.flush().expect("flush MCP message");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read MCP reply");
        serde_json::from_str(&line).expect("MCP reply is JSON")
    }

    fn call(&mut self, name: &str, arguments: Value) -> (Value, bool) {
        let reply = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let result = &reply["result"];
        (
            result["structuredContent"].clone(),
            result["isError"].as_bool().expect("isError flag"),
        )
    }

    fn ok(&mut self, name: &str, arguments: Value) -> Value {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(!is_error, "{name}({arguments}) failed: {value}");
        value
    }

    fn err(&mut self, name: &str, arguments: Value) -> String {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(is_error, "{name}({arguments}) should fail: {value}");
        value["error"].as_str().expect("error message").to_string()
    }
}

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

fn form(home: &TestHome, address: &str) -> Option<String> {
    Connection::open(home.archive_root.join("zt.sqlite3"))
        .expect("open db")
        .query_row(
            "SELECT math_form FROM cards WHERE location = ?1",
            [address],
            |row| row.get(0),
        )
        .expect("card form")
}

fn addresses(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("array")
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .or_else(|| item["address"].as_str().map(str::to_string))
                .or_else(|| item["to"].as_str().map(str::to_string))
                .expect("address")
        })
        .collect()
}

/// A textbook chapter: an axiom of another theory, a definition, a lemma, the
/// intermediate value theorem with its proof, a corollary, and two Theory Cards.
struct Chapter {
    axiom: String,
    definition: String,
    lemma: String,
    theorem: String,
    crux: String,
    corollary: String,
    theory: String,
}

fn chapter(mcp: &mut Mcp) -> Chapter {
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Abbott2015, title={Understanding Analysis}}" }),
    );
    let anchor = address(mcp.ok(
        "create_successor",
        json!({ "at": "Abbott2015", "title": "第 4 章 函数极限与连续" }),
    ));
    let side = |mcp: &mut Mcp, at: &str, args: Value| {
        let mut args = args;
        args["at"] = json!(at);
        args["kind"] = json!("side");
        address(mcp.ok("create_successor", args))
    };
    let next = |mcp: &mut Mcp, at: &str, args: Value| {
        let mut args = args;
        args["at"] = json!(at);
        address(mcp.ok("create_successor", args))
    };
    let axiom = side(
        mcp,
        &anchor,
        json!({ "type": "model", "form": "axiom", "level": "concept", "title": "完备性公理：有上界的非空实数集有上确界", "body": "(C1) 有上界的非空实数集有上确界。" }),
    );
    let definition = side(
        mcp,
        &anchor,
        json!({ "type": "model", "form": "definition", "level": "concept", "title": "连续：ε-δ 条件成立", "body": "**定义**：\n- (C1) $x_0\\in A$。\n- (C2) 对每个 $\\varepsilon>0$ 都存在 $\\delta>0$。" }),
    );
    side(
        mcp,
        &definition,
        json!({ "facet": "例子", "emphasis": "core", "title": "例子：$x^2$ 处处连续", "body": format!("例示 [[{definition}]]。") }),
    );
    side(
        mcp,
        &definition,
        json!({ "facet": "非例", "emphasis": "core", "title": "非例：符号函数在 0 处跳跃", "body": "(C2) 不成立。" }),
    );
    side(
        mcp,
        &definition,
        json!({ "facet": "直觉", "emphasis": "core", "title": "直觉：图像不断开", "body": "图像不断开。" }),
    );
    let lemma = side(
        mcp,
        &anchor,
        json!({ "type": "model", "form": "lemma", "level": "formal", "title": "引理：连续函数保号", "body": "(H1) $f$ 在 $x_0$ 连续，$f(x_0)<c$。" }),
    );
    let lemma_premises = side(
        mcp,
        &lemma,
        json!({ "facet": "前提", "title": "前提：f 连续", "body": "f 在 x0 连续。" }),
    );
    next(
        mcp,
        &lemma_premises,
        json!({ "facet": "证明：取 ε", "title": "证明：取 ε=c-f(x0)", "body": format!("依赖 [[{lemma_premises}]]。按定义 [[{definition}]] 取 ε。") }),
    );
    let theorem = side(
        mcp,
        &anchor,
        json!({ "type": "model", "form": "theorem", "level": "formal", "title": "介值定理：连续函数取遍中间值", "body": "**陈述**：\n- (H1) $f$ 在 $[a,b]$ 上连续。\n- (H2) $f(a)<c<f(b)$。" }),
    );
    let premises = side(
        mcp,
        &theorem,
        json!({ "facet": "前提", "title": "前提：f 连续且 f(a)<c<f(b)", "body": "(H1) 与 (H2)。" }),
    );
    let plan = next(
        mcp,
        &premises,
        json!({ "facet": "证明思路", "emphasis": "core", "title": "证明思路：取上确界再反证", "body": format!("依赖 [[{premises}]]。") }),
    );
    let exists = next(
        mcp,
        &plan,
        json!({ "facet": "证明：x0 存在", "title": "证明：x0 存在", "body": format!("依赖 [[{plan}]]。依赖 [[{axiom}]]：S 有上确界。") }),
    );
    let crux = next(
        mcp,
        &exists,
        json!({ "facet": "证明：排除小于", "emphasis": "core", "title": "证明：f(x0)<c 不成立", "body": format!("依赖 [[{exists}]]。依赖 [[{lemma}]]：右侧仍小于 c。") }),
    );
    let slip = next(
        mcp,
        &crux,
        json!({ "facet": "推导：右侧点", "emphasis": "aside", "title": "推导：右侧有 S 的点", "body": format!("依赖 [[{crux}]]。") }),
    );
    next(
        mcp,
        &slip,
        json!({ "facet": "证明：排除大于", "title": "证明：f(x0)>c 不成立", "body": format!("依赖 [[{slip}]]。由 (H1)。") }),
    );
    side(
        mcp,
        &theorem,
        json!({ "facet": "直觉", "emphasis": "core", "title": "直觉：图像必须穿过直线", "body": "图像从下方走到上方。" }),
    );
    let counter = side(
        mcp,
        &theorem,
        json!({ "facet": "反例：有理数域", "title": "反例：Q 上 x^2-2 没有零点", "body": format!("反例 [[{theorem}]]：定义域缺少完备性。") }),
    );
    side(
        mcp,
        &theorem,
        json!({ "facet": "假设作用：H1", "title": "假设作用：去掉连续性", "body": format!("见 [[{counter}]]。") }),
    );
    side(
        mcp,
        &theorem,
        json!({ "facet": "习题", "emphasis": "aside", "title": "习题：奇次多项式有实根", "body": "证明它。\n\n## 答案\n\n用介值定理。" }),
    );
    let corollary = side(
        mcp,
        &anchor,
        json!({ "type": "model", "form": "corollary", "level": "concept", "title": "推论：奇次多项式有实根", "body": format!("依赖 [[{theorem}]]。") }),
    );
    let reals = side(
        mcp,
        &anchor,
        json!({ "type": "model", "form": "theory", "title": "实数理论", "body": format!("**结构**：\n- [[{axiom}]]") }),
    );
    let _ = reals;
    let theory = side(
        mcp,
        &anchor,
        json!({ "type": "model", "form": "theory", "title": "连续函数理论", "body": format!("**问题**：连续函数有什么性质？\n\n**主要结果**：\n- [[{theorem}]]：介值定理。\n\n**对象**：[[{definition}]]\n\n**结构**：\n- [[{corollary}]]\n- [[{theorem}]]\n- [[{lemma}]]\n- [[{definition}]]") }),
    );
    Chapter {
        axiom,
        definition,
        lemma,
        theorem,
        crux,
        corollary,
        theory,
    }
}

#[test]
fn a_model_card_states_a_math_form_and_only_a_model_card_does() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let c = chapter(&mut mcp);
    assert_eq!(form(&home, &c.theorem).as_deref(), Some("theorem"));
    assert_eq!(
        mcp.ok("get_card", json!({ "at": c.theorem }))["form"],
        "theorem"
    );
    let refusal = "only a Model Card (a whole model, without a facet) has a Math form";
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": c.theorem, "kind": "side", "facet": "历史", "form": "lemma", "title": "历史" }),
        )
        .contains(refusal)
    );
    assert!(
        mcp.err(
            "edit_card",
            json!({ "at": "Abbott2015/1", "form": "theorem" })
        )
        .contains(refusal)
    );
    assert!(
        mcp.err("edit_card", json!({ "at": c.lemma, "form": "formal" }))
            .contains("unknown Math form `formal`")
    );
    // The Chinese name sets the form; an empty form clears it; a Card that stops being a Model Card loses it.
    mcp.ok("edit_card", json!({ "at": c.corollary, "form": "命题" }));
    assert_eq!(form(&home, &c.corollary).as_deref(), Some("proposition"));
    mcp.ok("edit_card", json!({ "at": c.corollary, "form": "" }));
    assert_eq!(form(&home, &c.corollary), None);
    mcp.ok(
        "edit_card",
        json!({ "at": c.corollary, "form": "corollary" }),
    );
    mcp.ok("edit_card", json!({ "at": c.corollary, "type": "idea" }));
    assert_eq!(form(&home, &c.corollary), None);
    mcp.ok(
        "edit_card",
        json!({ "at": c.corollary, "type": "model", "form": "corollary" }),
    );
    // Search lists the Model Cards of one form without a query.
    let theorems = mcp.ok("search_cards", json!({ "form": "theorem" }));
    assert_eq!(addresses(&theorems["results"]), vec![c.theorem.clone()]);
    assert_eq!(theorems["results"][0]["form"], "theorem");
    let tree = mcp.ok("get_tree", json!({ "tree": "Abbott2015" }));
    let summary = tree["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|card| card["address"] == c.lemma.as_str())
        .unwrap()
        .clone();
    assert_eq!(summary["form"], "lemma");
}

#[test]
fn a_proof_cites_earlier_statements_and_a_result_reports_its_attention_and_study_order() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let c = chapter(&mut mcp);
    let model = mcp.ok("get_model", json!({ "at": c.theorem }));
    assert_eq!(model["form"], "theorem");
    let build = &model["build"];
    // Statements of other units are cited premises, not closure faults.
    assert_eq!(build["closure"], json!([]), "{build}");
    assert_eq!(build["reach_in"], json!([]));
    assert_eq!(build["self_sufficient"], true);
    let mut cited = addresses(&build["cited"]);
    cited.sort();
    let mut expected = vec![c.axiom.clone(), c.lemma.clone()];
    expected.sort();
    assert_eq!(cited, expected);

    let math = &model["math"];
    assert_eq!(math["labels"], json!(["H1", "H2"]));
    assert_eq!(math["crux"], json!([c.crux]));
    assert_eq!(math["budget"]["chunks"], 3);
    assert_eq!(math["faults"], json!([]), "{math}");
    assert_eq!(math["tiers"][c.theorem.as_str()], "focus");
    // The corollary cites the theorem's statement: one citer, in the theory that lists it.
    assert_eq!(math["load"], 1);
    assert_eq!(math["used_by"][0]["theory"]["address"], c.theory.as_str());
    assert_eq!(math["theories"][0]["address"], c.theory.as_str());

    // Study order: meet the statement beside its intuition; the crux step lies face down
    // with the lemma as a just-in-time tool tab; at most three companions per station.
    let stations = math["study"].as_array().expect("study");
    assert!(
        stations
            .iter()
            .all(|s| s["companions"].as_array().unwrap().len() <= 3)
    );
    let crux = stations
        .iter()
        .find(|s| s["crux"] == true)
        .expect("a crux station");
    assert_eq!(crux["focus"], json!([c.crux]));
    assert_eq!(crux["face_down"], true);
    assert_eq!(crux["tools"][0]["address"], c.lemma.as_str());
    assert_eq!(crux["tools"][0]["just_in_time"], true);
    assert_eq!(crux["details"].as_array().unwrap().len(), 1);
    assert!(stations.iter().any(|s| s["pass"] == "boundary"));
    assert!(stations.iter().any(|s| s["pass"] == "recall"));

    // A definition: the contrast pair counts once in the budget, and is met first.
    let definition = mcp.ok("get_model", json!({ "at": c.definition }));
    let math = &definition["math"];
    assert_eq!(math["budget"]["pair"], true);
    assert_eq!(math["budget"]["chunks"], 2);
    assert_eq!(math["faults"], json!([]));
    assert_eq!(math["study"][0]["focus"].as_array().unwrap().len(), 2);
    assert_eq!(math["study"][1]["focus"], json!([c.definition]));
}

#[test]
fn a_unit_reports_a_missing_crux_example_and_intuition() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let c = chapter(&mut mcp);
    // Without a core step, a formal proof of three or more steps has no crux.
    mcp.ok("edit_card", json!({ "at": c.crux, "emphasis": "" }));
    let math = mcp.ok("get_model", json!({ "at": c.theorem }))["math"].clone();
    let faults = math["faults"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["fault"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(faults, ["no_crux"]);
    // A bare definition asks for an example, a non-example, and an intuition.
    let bare = address(mcp.ok(
        "create_successor",
        json!({ "at": "Abbott2015/1", "kind": "side", "type": "model", "form": "definition", "title": "一致连续" }),
    ));
    let math = mcp.ok("get_model", json!({ "at": bare }))["math"].clone();
    let faults = math["faults"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["fault"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(faults, ["no_example", "no_nonexample", "no_intuition"]);
}

#[test]
fn a_reach_in_is_a_fault_and_any_model_may_cite_a_unit() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let c = chapter(&mut mcp);
    // An economics model rests on the theorem's statement and on a step inside its proof.
    let topic = address(mcp.ok("create_topic", json!({ "title": "定价" })));
    let first = address(mcp.ok(
        "create_successor",
        json!({ "at": topic, "title": "定价问题" }),
    ));
    let model = address(mcp.ok(
        "create_successor",
        json!({ "at": first, "kind": "side", "type": "model", "title": "垄断定价模型" }),
    ));
    let setting = address(mcp.ok(
        "create_successor",
        json!({ "at": model, "kind": "side", "facet": "设定", "title": "设定", "body": format!("依赖 [[{}]]：g 在两端变号，所以有零点。", c.theorem) }),
    ));
    mcp.ok(
        "create_successor",
        json!({ "at": setting, "facet": "推导：零点", "title": "推导：零点", "body": format!("依赖 [[{setting}]]。依赖 [[{}]]。", c.crux) }),
    );
    let build = mcp.ok("get_model", json!({ "at": model }))["build"].clone();
    assert_eq!(addresses(&build["cited"]), vec![c.theorem.clone()]);
    assert_eq!(build["closure"], json!([]));
    let reach = build["reach_in"].as_array().expect("reach_in");
    assert_eq!(reach.len(), 1);
    assert_eq!(reach[0]["unit"], c.theorem.as_str());
    assert!(
        reach[0]["fix"]
            .as_str()
            .unwrap()
            .contains("make the step a lemma")
    );
    assert_eq!(build["self_sufficient"], false);
    // The theorem now counts the economics model among its citers.
    let math = mcp.ok("get_model", json!({ "at": c.theorem }))["math"].clone();
    assert_eq!(math["load"], 2);
}

#[test]
fn a_theory_orders_its_units_by_dependency_and_names_its_tools() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let c = chapter(&mut mcp);
    let model = mcp.ok("get_model", json!({ "at": c.theory }));
    assert_eq!(model["form"], "theory");
    let math = &model["math"];
    // Each unit after the units it cites, whatever order the Theory Card lists them in.
    assert_eq!(
        addresses(&math["order"]),
        vec![
            c.definition.clone(),
            c.lemma.clone(),
            c.theorem.clone(),
            c.corollary.clone()
        ]
    );
    assert_eq!(math["main_results"], json!([c.theorem]));
    assert_eq!(math["objects"], json!([c.definition]));
    let weight = |unit: &str| {
        math["order"]
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["address"] == unit)
            .unwrap()["weight"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(weight(&c.theorem), "focus");
    assert_eq!(weight(&c.definition), "focus");
    assert_eq!(weight(&c.lemma), "detail");
    assert_eq!(weight(&c.corollary), "support");
    // The axiom belongs to another theory: a tool of this one.
    assert_eq!(addresses(&math["tools"]), vec![c.axiom.clone()]);
    assert_eq!(math["cycle"], json!([]));
    assert_eq!(math["faults"], json!([]));
    // The prerequisite path of the main result leaves the lemma for just in time,
    // and with it the definition that only the lemma's proof cites.
    let path = &math["study"][1]["results"][0];
    assert_eq!(path["result"], c.theorem.as_str());
    assert_eq!(path["path"], json!([]));
    // The axiom is a building block: its own theory and this one use it.
    let axiom = mcp.ok("get_model", json!({ "at": c.axiom }))["math"].clone();
    assert_eq!(axiom["building_block"], true);
}

#[test]
fn a_prerequisite_path_follows_statements_and_leaves_a_lemmas_proof_for_just_in_time() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let c = chapter(&mut mcp);
    let theory = |mcp: &mut Mcp| mcp.ok("get_model", json!({ "at": c.theory }))["math"].clone();
    let entry = |math: &Value, unit: &str| {
        math["order"]
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["address"] == unit)
            .unwrap()
            .clone()
    };
    // Only the lemma's proof cites the definition: a proof premise, not a statement premise.
    let math = theory(&mut mcp);
    let lemma = entry(&math, &c.lemma);
    assert_eq!(lemma["cites"], json!([c.definition]));
    assert_eq!(lemma["statement_cites"], json!([]));
    assert_eq!(math["study"][1]["results"][0]["path"], json!([]));
    // The dependency order still counts every premise.
    assert_eq!(
        addresses(&math["order"])[..2],
        [c.definition.clone(), c.lemma.clone()]
    );
    // When the lemma's statement cites the definition, the reader needs the
    // definition to read the lemma at its step, so the path holds it.
    mcp.ok(
        "edit_card",
        json!({ "at": c.lemma, "body": format!("(H1) $f$ 在 $x_0$ 连续，$f(x_0)<c$。\n\n基于 [[{}]] 的连续。", c.definition) }),
    );
    let math = theory(&mut mcp);
    assert_eq!(
        entry(&math, &c.lemma)["statement_cites"],
        json!([c.definition])
    );
    assert_eq!(
        math["study"][1]["results"][0]["path"],
        json!([c.definition])
    );
}

#[test]
fn special_cases_and_definitions_announce_their_relation_kinds() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let c = chapter(&mut mcp);
    let uniform = address(mcp.ok(
        "create_successor",
        json!({ "at": "Abbott2015/1", "kind": "side", "type": "model", "form": "definition", "title": "一致连续", "body": format!("特例 [[{}]]：一致连续的函数处处连续。", c.definition) }),
    ));
    let card = mcp.ok("get_card", json!({ "at": uniform }));
    let tree = mcp.ok("get_tree", json!({ "tree": "Abbott2015" }));
    let summary = tree["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|card| card["address"] == uniform.as_str())
        .unwrap()
        .clone();
    assert_eq!(
        summary["kinds"][c.definition.as_str()],
        "specializes",
        "{card}"
    );
    // The definition's connect station names its narrower concept's broader one.
    let math = mcp.ok("get_model", json!({ "at": uniform }))["math"].clone();
    let connect = math["study"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["pass"] == "connect")
        .unwrap()
        .clone();
    assert_eq!(connect["companions"][0], c.definition.as_str());
}
