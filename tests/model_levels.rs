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
        cmd.env("ZT_CONFIG_DIR", &self.config_dir);
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
        mcp.err("edit_card", json!({ "at": concept, "level": "3" }))
            .contains("unknown elaboration level `3`")
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
