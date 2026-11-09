//! Elaboration levels: a Model Card is a concept model (level 1) or a formal
//! model (level 2), over MCP, the Session view, and the dump manifest.

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

fn level(home: &TestHome, address: &str) -> Option<String> {
    Connection::open(home.archive_root.join("zt.sqlite3"))
        .expect("open db")
        .query_row(
            "SELECT model_level FROM cards WHERE location = ?1",
            [address],
            |row| row.get(0),
        )
        .expect("card level")
}

/// A Literature tree with one anchor Card to file models under.
fn book(mcp: &mut Mcp) {
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Hart1995, title={Firms, Contracts, and Financial Structure}}" }),
    );
    mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995", "title": "第五章" }),
    );
}

#[test]
fn a_model_card_carries_a_concept_or_formal_level_and_only_a_model_card_does() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    book(&mut mcp);
    let concept = mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "kind": "side", "type": "model", "level": "concept", "title": "敲竹杠", "body": "问题：专用投资为何不足？" }),
    );
    assert_eq!(concept["level"], "concept");
    let concept = address(concept);
    assert_eq!(level(&home, &concept).as_deref(), Some("concept"));
    let card = mcp.ok("get_card", json!({ "at": concept }));
    assert_eq!(card["level"], "concept");
    let model = mcp.ok("get_model", json!({ "at": concept }));
    assert_eq!(model["level"], "concept");
    assert_eq!(model["build"]["level"], "concept");
    assert_eq!(model["build"]["level_number"], 1);

    // A number names the level too, and an empty level clears it.
    mcp.ok("edit_card", json!({ "at": concept, "level": "2" }));
    assert_eq!(level(&home, &concept).as_deref(), Some("formal"));
    assert_eq!(
        mcp.ok("get_model", json!({ "at": concept }))["build"]["level_number"],
        2
    );
    mcp.ok("edit_card", json!({ "at": concept, "level": "" }));
    assert_eq!(level(&home, &concept), None);
    assert_eq!(
        mcp.ok("get_model", json!({ "at": concept }))["level"],
        Value::Null
    );

    // Only a Model Card has a level: not an Idea Card, not a Model part.
    let refusal = "only a Model Card (a whole model, without a facet) has an elaboration level";
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "Hart1995/1", "title": "一个想法", "level": "concept" }),
        )
        .contains(refusal)
    );
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": concept, "kind": "side", "facet": "设定", "level": "formal", "title": "设定" }),
        )
        .contains(refusal)
    );
    let idea = address(mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "title": "一个想法" }),
    ));
    assert!(
        mcp.err("edit_card", json!({ "at": idea, "level": "formal" }))
            .contains(refusal)
    );
    assert!(
        mcp.err("edit_card", json!({ "at": concept, "level": "4" }))
            .contains("unknown elaboration level `4`")
    );

    // A Model Card that becomes an Idea Card or a part loses its level.
    mcp.ok("edit_card", json!({ "at": concept, "level": "formal" }));
    mcp.ok("edit_card", json!({ "at": concept, "type": "idea" }));
    assert_eq!(level(&home, &concept), None);
    mcp.ok(
        "edit_card",
        json!({ "at": concept, "type": "model", "level": "concept" }),
    );
    assert_eq!(level(&home, &concept).as_deref(), Some("concept"));
    mcp.ok("edit_card", json!({ "at": concept, "facet": "对应" }));
    assert_eq!(level(&home, &concept), None);
}

#[test]
fn a_concept_model_reports_each_derivation_part_beyond_its_level() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    book(&mut mcp);
    let head = address(mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "kind": "side", "type": "model", "level": "concept", "title": "资金挪用", "body": "问题：E 能借到多少？" }),
    ));
    let setting = address(mcp.ok(
        "create_successor",
        json!({ "at": head, "kind": "side", "facet": "设定", "title": "设定：E 与 C", "body": "E 有项目。C 出资。" }),
    ));
    let step = address(mcp.ok(
        "create_successor",
        json!({ "at": setting, "facet": "推导：还款上限", "emphasis": "aside", "title": "推导：还款上限", "body": format!("依赖 [[{setting}]]。") }),
    ));
    let result = address(mcp.ok(
        "create_successor",
        json!({ "at": step, "facet": "结论", "title": "结论：E 借到的有限", "body": format!("依赖 [[{step}]]。") }),
    ));
    let model = mcp.ok("get_model", json!({ "at": result }));
    let beyond = model["build"]["beyond_level"]
        .as_array()
        .expect("beyond_level");
    assert_eq!(beyond.len(), 1);
    assert_eq!(beyond[0]["address"], step.as_str());
    assert!(
        beyond[0]["fix"]
            .as_str()
            .unwrap()
            .contains("raise the model to formal (level 2)")
    );
    // The level does not change the self-sufficiency rules.
    assert_eq!(model["build"]["self_sufficient"], true);

    // A formal model, or a model with no level, may hold Derivation parts.
    mcp.ok("edit_card", json!({ "at": head, "level": "formal" }));
    let model = mcp.ok("get_model", json!({ "at": head }));
    assert_eq!(model["build"]["beyond_level"], json!([]));
    mcp.ok("edit_card", json!({ "at": head, "level": "" }));
    let model = mcp.ok("get_model", json!({ "at": head }));
    assert_eq!(model["build"]["beyond_level"], json!([]));
    assert_eq!(model["build"]["level"], Value::Null);
}

#[test]
fn search_and_tree_summaries_name_the_level() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    book(&mut mcp);
    let head = address(mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "kind": "side", "type": "model", "level": "formal", "title": "GHM 产权模型" }),
    ));
    let hits = mcp.ok("search_cards", json!({ "type": "model" }));
    let hit = hits["results"]
        .as_array()
        .expect("results")
        .iter()
        .find(|hit| hit["address"] == head.as_str())
        .expect("the Model Card is found")
        .clone();
    assert_eq!(hit["level"], "formal");
    let tree = mcp.ok("get_tree", json!({ "tree": "Hart1995" }));
    let summary = tree["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|card| card["address"] == head.as_str())
        .expect("the Model Card is in its tree")
        .clone();
    assert_eq!(summary["level"], "formal");
}

/// Level 3 (docs/empirical-models-proposal.md §4.1): a Model Card may be an empirical
/// model, named `empirical`, `3` or `实证`, stored as `empirical`, with level number 3.
#[test]
fn a_model_card_can_be_an_empirical_model_at_level_three() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    book(&mut mcp);
    let head = mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "kind": "side", "type": "model", "level": "empirical", "title": "增值税收入的跨国回归", "body": "问题：税率高的国家收入高多少？" }),
    );
    assert_eq!(head["level"], "empirical");
    let head = address(head);
    assert_eq!(level(&home, &head).as_deref(), Some("empirical"));
    let model = mcp.ok("get_model", json!({ "at": head }));
    assert_eq!(model["build"]["level"], "empirical");
    assert_eq!(model["build"]["level_number"], 3);
    mcp.ok("edit_card", json!({ "at": head, "level": "concept" }));
    mcp.ok("edit_card", json!({ "at": head, "level": "3" }));
    assert_eq!(level(&home, &head).as_deref(), Some("empirical"));
    mcp.ok("edit_card", json!({ "at": head, "level": "concept" }));
    mcp.ok("edit_card", json!({ "at": head, "level": "实证" }));
    assert_eq!(level(&home, &head).as_deref(), Some("empirical"));
}

/// One Model part filed at `at` (`kind` side or direct), titled after its facet.
fn part(mcp: &mut Mcp, at: &str, kind: &str, facet: &str, body: String) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "kind": kind, "facet": facet, "title": format!("{facet}：标题"), "body": body }),
    ))
}

/// The empirical build rules (proposal §5.5), read from the facet prefixes of §5.1:
/// an estimate rests on a specification, a variant on its base, a test on an assumption,
/// every symbol of a specification's equation has a row in the 变量表, and the only
/// Derivation part is one that derives an estimating equation or an estimator.
#[test]
fn an_empirical_model_reports_each_role_fault() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    book(&mut mcp);
    let head = address(mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "kind": "side", "type": "model", "level": "empirical", "title": "增值税收入的跨国回归" }),
    ));
    let m = &mut mcp;
    let data = part(m, &head, "side", "数据", "原始设定：99 个国家。".into());
    part(
        m,
        &data,
        "side",
        "变量表",
        format!("| 变量 | 含义 |\n| --- | --- |\n| $SR$ | 标准税率 |\n| $\\beta$ | 税率的系数 |\n\n依赖 [[{data}]]。"),
    );
    let design = part(m, &data, "direct", "设计", format!("依赖 [[{data}]]。"));
    let assumption = part(m, &design, "side", "假设 1", format!("依赖 [[{design}]]。"));
    let spec = part(
        m,
        &design,
        "direct",
        "规格：基准",
        format!("依赖 [[{data}]]、[[{design}]]。\n\n$$ R = \\beta SR + \\varepsilon \\tag{{1}} $$"),
    );
    part(m, &spec, "side", "推导：估计式", format!("依赖 [[{data}]]。"));
    part(m, &spec, "side", "规格：B 列", format!("依赖 [[{spec}]]。"));
    let loose_variant = part(m, &spec, "side", "规格：C 列", format!("依赖 [[{data}]]。"));
    let chapter = part(m, &spec, "direct", "第 1 章：主要估计", format!("依赖 [[{spec}]]。"));
    let estimate = part(m, &chapter, "side", "估计：税率弹性", format!("依赖 [[{spec}]]。"));
    let loose_estimate = part(m, &chapter, "side", "估计：开放度", format!("依赖 [[{chapter}]]。"));
    let test = part(m, &estimate, "side", "检验：前趋势", format!("依赖 [[{spec}]]。"));
    part(m, &estimate, "side", "检验：安慰剂", format!("依赖 [[{assumption}]]、[[{spec}]]。"));
    let arithmetic = part(m, &estimate, "side", "推导：系数换算", format!("依赖 [[{estimate}]]。"));

    let build = mcp.ok("get_model", json!({ "at": head }))["build"].clone();
    let at = |list: &Value| -> Vec<String> {
        list.as_array()
            .unwrap_or_else(|| panic!("a list: {list}"))
            .iter()
            .map(|entry| entry["address"].as_str().expect("address").to_string())
            .collect()
    };
    assert_eq!(build["level_number"], 3);
    assert_eq!(build["empirical_shape"], json!([]), "{build}");
    let role_of = |address: &str| {
        build["order"]
            .as_array()
            .expect("order")
            .iter()
            .find(|entry| entry["address"] == address)
            .map(|entry| entry["role"].clone())
    };
    assert_eq!(role_of(&spec), Some(json!("spec")));
    assert_eq!(role_of(&data), Some(json!("data")));
    assert_eq!(role_of(&assumption), Some(json!("assumption")));
    assert_eq!(at(&build["estimate_unbound"]), [loose_estimate.clone()]);
    assert_eq!(at(&build["spec_unbound"]), [loose_variant.clone()]);
    assert_eq!(at(&build["test_unbound"]), [test.clone()]);
    assert_eq!(at(&build["beyond_level"]), [arithmetic.clone()]);
    assert!(
        build["beyond_level"][0]["fix"].as_str().unwrap().contains("换算"),
        "{build}"
    );
    let uncovered = build["variable_uncovered"]
        .as_array()
        .expect("variable_uncovered")
        .iter()
        .map(|entry| entry["symbol"].as_str().expect("symbol").to_string())
        .collect::<Vec<_>>();
    assert_eq!(uncovered, ["R", "\\varepsilon"], "{build}");
    assert_eq!(build["variable_uncovered"][0]["address"], spec.as_str());
    // The role rules add faults; they do not change self-sufficiency.
    assert_eq!(build["self_sufficient"], true, "{build}");
    assert_eq!(build["evidence_bound"], false);
}

/// `empirical_shape`: the main line starts with a 数据 part and holds a 设计 part and a
/// 规格 part, and the model holds an estimate. A formal model takes none of the role rules.
#[test]
fn an_empirical_model_out_of_shape_says_what_its_main_line_lacks() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    book(&mut mcp);
    let head = address(mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "kind": "side", "type": "model", "level": "empirical", "title": "一个回归" }),
    ));
    let setting = part(&mut mcp, &head, "side", "设定", "原始设定。".into());
    let build = mcp.ok("get_model", json!({ "at": head }))["build"].clone();
    let lacks = build["empirical_shape"]
        .as_array()
        .expect("empirical_shape")
        .iter()
        .map(|entry| entry["lacks"].as_str().expect("lacks").to_string())
        .collect::<Vec<_>>();
    assert_eq!(lacks, ["数据", "设计", "规格", "估计"], "{build}");
    assert_eq!(build["empirical_shape"][0]["address"], setting.as_str());
    assert_eq!(build["evidence_bound"], false);

    mcp.ok("edit_card", json!({ "at": head, "level": "formal" }));
    let build = mcp.ok("get_model", json!({ "at": head }))["build"].clone();
    assert_eq!(build["empirical_shape"], json!([]));
    assert_eq!(build["evidence_bound"], Value::Null);
}

/// An archive whose level CHECK predates level 3 is rebuilt in place, after one backup,
/// and keeps its Cards; then it stores an empirical level.
#[test]
fn an_archive_from_before_empirical_models_upgrades_in_place() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    book(&mut mcp);
    let head = address(mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "kind": "side", "type": "model", "level": "formal", "title": "旧模型", "body": "旧卡。" }),
    ));
    drop(mcp);
    home.cmd().arg("down").assert().success();
    {
        let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let old = sql.replace(", 'empirical'", "");
        assert_ne!(old, sql);
        conn.execute_batch("PRAGMA writable_schema = ON;").unwrap();
        conn.execute(
            "UPDATE sqlite_master SET sql = ?1 WHERE type = 'table' AND name = 'cards'",
            [&old],
        )
        .unwrap();
        conn.execute_batch("PRAGMA writable_schema = OFF;").unwrap();
    }
    home.cmd().arg("up").assert().success();
    let backups = fs::read_dir(&home.archive_root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("zt.sqlite3.pre-empirical-models-")
        })
        .count();
    assert_eq!(backups, 1, "the upgrade backs up a non-empty archive once");
    let mut mcp = home.mcp();
    assert_eq!(level(&home, &head).as_deref(), Some("formal"));
    mcp.ok("edit_card", json!({ "at": head, "level": "empirical" }));
    assert_eq!(level(&home, &head).as_deref(), Some("empirical"));
}

/// The write gate applies EM1–EM4 to a part of an empirical model, and EM5 reads the
/// design from the Model Card's **设计** section; a formal model's parts take none of them.
#[test]
fn the_write_gate_applies_the_em_rules_to_the_parts_of_an_empirical_model() {
    let home = TestHome::new();
    home.cmd().args(["config", "set", "gate.stage", "enforce"]).assert().success();
    let mut mcp = home.mcp();
    book(&mut mcp);
    let head = address(mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "kind": "side", "type": "model", "level": "empirical", "title": "增值税收入的跨国回归",
                "body": "**问题**\n税率高的国家收入高多少？\n\n**设计**\ncross-section：99 个国家的横截面比较。\n\n来源：[[Hart1995]] §4.2" }),
    ));
    let data = part(&mut mcp, &head, "side", "数据", "原始设定：99 个国家。".into());
    let (refusal, is_error) = mcp.call(
        "create_successor",
        json!({ "at": data, "facet": "设计", "title": "设计：横截面比较", "body": "国家之间比较。" }),
    );
    assert!(is_error, "{refusal}");
    assert_eq!(refusal["error"], json!("card_check"), "{refusal}");
    assert!(refusal.to_string().contains("EM4"), "{refusal}");
    let design = part(
        &mut mcp,
        &data,
        "direct",
        "设计",
        format!("国家之间比较。\n\n**变异来源**\n各国税率不同。\n\n**识别假设**\n- 依赖 [[{data}]]：税率与误差无关。"),
    );
    let estimate = format!(
        "税率提高导致收入上升。\n\n**估计**\n- 估计量：[[{design}]] 中 $\\beta$，表 4.2 A 列。\n- 值：0.70。\n- 标准误：0.21。\n\n**量级**\n税率升 10%，收入升约 7%。\n\n**依据**\n依赖 [[{design}]]。\n\n来源：[[Hart1995]] §4.2"
    );
    let written = mcp.ok(
        "create_successor",
        json!({ "at": design, "kind": "side", "facet": "估计：税率弹性", "title": "估计：税率弹性", "body": estimate }),
    );
    assert!(written.to_string().contains("EM5"), "EM5 is a warning: {written}");

    // The same unlabelled design part is no fault in a formal model.
    mcp.ok("edit_card", json!({ "at": head, "level": "formal" }));
    part(&mut mcp, &data, "side", "设计", "国家之间比较。".into());
}
