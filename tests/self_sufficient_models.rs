//! Self-sufficient models: the build report of `get_model`, and the tree
//! operations that put a Card between two Cards (`insert_card`, and
//! `move_card` with `before` and `alone`) over MCP and the GUI.

#![allow(dead_code)]

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
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

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Gui {
    child: Child,
    port: u16,
    token: String,
}

impl Gui {
    fn start(home: &TestHome) -> Self {
        let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
            .arg("gui")
            .env("ZT_CONFIG_DIR", &home.config_dir)
            .env("ZT_GUI_PORT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start zt gui");
        let mut stdout = BufReader::new(child.stdout.take().expect("gui stdout"));
        let mut first_line = String::new();
        stdout.read_line(&mut first_line).expect("gui url line");
        let port: u16 = first_line
            .trim()
            .strip_prefix("zt gui: http://127.0.0.1:")
            .and_then(|rest| rest.strip_suffix('/'))
            .expect("gui prints its URL")
            .parse()
            .expect("port");
        let mut gui = Self {
            child,
            port,
            token: String::new(),
        };
        let (_, _, page) = gui.raw("GET", "/", None);
        gui.token = page
            .split("name=\"zt-token\" content=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("token")
            .to_string();
        gui
    }

    /// Status code, headers, and body of one request.
    fn raw(&self, method: &str, path: &str, body: Option<&str>) -> (u16, String, String) {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        let body = body.unwrap_or_default();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            self.port,
            self.token,
            body.len()
        )
        .expect("send request");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        let response = String::from_utf8_lossy(&response).to_string();
        let (head, body) = response.split_once("\r\n\r\n").expect("header end");
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .expect("status");
        (status, head.to_ascii_lowercase(), body.to_string())
    }

    fn api(&self, method: &str, path: &str, body: Value) -> Value {
        let body = (method == "POST").then(|| body.to_string());
        let (_, _, text) = self.raw(method, path, body.as_deref());
        serde_json::from_str(&text).expect("JSON response")
    }
}

impl Drop for Gui {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Addresses of a model's `build.order`, with their lines.
fn build_order(model: &Value) -> Vec<(String, String)> {
    model["build"]["order"]
        .as_array()
        .expect("build order")
        .iter()
        .map(|entry| {
            (
                entry["address"].as_str().unwrap().to_string(),
                entry["line"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn faults(model: &Value, list: &str) -> Vec<(String, String)> {
    model["build"][list]
        .as_array()
        .unwrap_or_else(|| panic!("build.{list}"))
        .iter()
        .map(|fault| {
            (
                fault["from"].as_str().unwrap_or_default().to_string(),
                fault["to"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

#[test]
fn get_model_reports_how_a_reader_builds_the_model_and_what_keeps_it_from_being_self_sufficient() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({ "title": "Corporate finance" }));
    mcp.ok(
        "create_successor",
        json!({ "at": "0/0", "title": "清算价值高于零", "body": "一张主题卡。" }),
    );
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Hart1995, title={Firms, Contracts, and Financial Structure}}" }),
    );
    mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995", "title": "第五章" }),
    );
    mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "title": "清算价值的定义", "body": "清算价值 L 是资产在时点 1 的变卖价。" }),
    );
    // Model A: a setting, then a chain of steps, then a limit off the build.
    let a = address(mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "kind": "side", "type": "model", "title": "资金挪用模型", "body": "问题：E 能借到多少？" }),
    ));
    let setting = address(mcp.ok(
        "create_successor",
        json!({ "at": a, "kind": "side", "facet": "设定", "title": "设定：E 与 C", "body": "E 有项目，C 出资。见 [[Hart1995]]。依赖 [[Hart1995]] 第五章。" }),
    ));
    let repay = address(mcp.ok(
        "create_successor",
        json!({ "at": setting, "facet": "可承诺的还款", "emphasis": "core", "title": "可承诺的还款", "body": format!("依赖设定 [[{setting}]]。") }),
    ));
    let ratio = address(mcp.ok(
        "create_successor",
        json!({ "at": repay, "facet": "清算比例", "title": "清算比例", "body": format!("依赖可承诺的还款 [[{repay}]] 与清算价值 [[Hart1995/2]]。依赖清算价值高于零 [[0/1]]。") }),
    ));
    let finance = address(mcp.ok(
        "create_successor",
        json!({ "at": ratio, "facet": "融资条件", "emphasis": "core", "title": "融资条件", "body": format!("依赖可承诺的还款 [[{repay}]]。") }),
    ));
    let example = address(mcp.ok(
        "create_successor",
        json!({ "at": finance, "facet": "事后低效率", "title": "事后低效率（例 1）", "body": format!("例示清算比例 [[{ratio}]]。") }),
    ));
    // 融资条件 also rests on the example, which comes later in the build.
    mcp.ok(
        "edit_card",
        json!({ "at": finance, "body": format!("依赖可承诺的还款 [[{repay}]]。依赖事后低效率 [[{example}]]。") }),
    );
    let limits = address(mcp.ok(
        "create_successor",
        json!({ "at": a, "kind": "side", "facet": "局限", "title": "局限", "body": format!("对照融资条件 [[{finance}]]。") }),
    ));
    assert_eq!(
        [&a, &setting, &repay, &ratio, &finance, &example, &limits],
        [
            "Hart1995/1|a",
            "Hart1995/1|a|a",
            "Hart1995/1|a|a|1",
            "Hart1995/1|a|a|2",
            "Hart1995/1|a|a|3",
            "Hart1995/1|a|a|4",
            "Hart1995/1|a|b"
        ]
    );
    // Model B takes in one part of A, and rests on A as a whole and on a part it does not take in.
    let b = address(mcp.ok(
        "create_successor",
        json!({ "at": "Hart1995/1", "kind": "side", "type": "model", "title": "多个债权人", "body": format!("结构：[[{repay}]]。") }),
    ));
    let b_setting = address(mcp.ok(
        "create_successor",
        json!({ "at": b, "kind": "side", "facet": "设定", "title": "设定", "body": format!("模型沿用 [[{a}]] 的两期结构。依赖清算比例 [[{ratio}]]。") }),
    ));
    let b_step = address(mcp.ok(
        "create_successor",
        json!({ "at": b_setting, "facet": "不违约条件", "title": "不违约条件", "body": format!("依赖设定 [[{b_setting}]] 与可承诺的还款 [[{repay}]]。对照融资条件 [[{finance}]]。") }),
    ));

    let model = mcp.ok("get_model", json!({ "at": a }));
    let build = &model["build"];
    assert_eq!(build["setting"], json!(setting));
    assert_eq!(
        build_order(&model),
        pairs(&[
            (&setting, "main"),
            (&repay, "main"),
            (&ratio, "main"),
            (&finance, "main"),
            (&example, "main"),
            (&limits, "off"),
        ])
    );
    // A tree root is never a premise; an outside Card the range rests on is a closure fault.
    assert_eq!(
        faults(&model, "closure"),
        pairs(&[(&ratio, "Hart1995/2"), (&ratio, "0/1")])
    );
    let fix = |model: &Value, at: usize| {
        model["build"]["closure"][at]["fix"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert!(
        fix(&model, 0).contains("move it into the range"),
        "{}",
        fix(&model, 0)
    );
    assert!(
        fix(&model, 1).contains("restate it in a part"),
        "{}",
        fix(&model, 1)
    );
    assert_eq!(build["closure"][0]["card"]["type"], "idea");
    assert_eq!(faults(&model, "forward"), pairs(&[(&finance, &example)]));
    // The example rests on nothing: no rests-on path leads back to the setting.
    assert_eq!(
        build["unanchored"]
            .as_array()
            .unwrap()
            .iter()
            .map(|fault| fault["address"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [example.as_str()]
    );
    assert_eq!(build["self_sufficient"], false);
    // 0.10.0 issues keep their meaning.
    assert_eq!(model["issues"], json!([]));

    let model_b = mcp.ok("get_model", json!({ "at": b_step }));
    assert_eq!(model_b["head"]["address"], json!(b));
    // The shared part stands just before the first step that rests on it.
    assert_eq!(
        build_order(&model_b),
        pairs(&[(&b_setting, "main"), (&repay, "shared"), (&b_step, "main")])
    );
    assert_eq!(model_b["build"]["order"][1]["home"], json!(a));
    assert_eq!(
        faults(&model_b, "closure"),
        pairs(&[(&b_setting, &a), (&b_setting, &ratio)])
    );
    assert!(
        fix(&model_b, 0).contains("a model rests on parts, not on a whole model"),
        "{}",
        fix(&model_b, 0)
    );
    assert!(
        fix(&model_b, 1).contains(&format!("a part of `{a}`")),
        "{}",
        fix(&model_b, 1)
    );
    // The shared part's own premise stays in its home model: tolerated, not a fault.
    assert_eq!(model_b["build"]["tolerated"][0]["part"], json!(repay));
    assert_eq!(
        model_b["build"]["tolerated"][0]["premise"]["address"],
        json!(setting)
    );
    assert_eq!(model_b["build"]["tolerated"][0]["home"], json!(a));
    assert_eq!(model_b["build"]["unanchored"], json!([]));

    // Insert a Derivation part before 融资条件: the chain moves down, and Links follow.
    let insert = json!({
        "before": finance,
        "facet": "推导：参与约束",
        "emphasis": "aside",
        "title": "推导：E 的参与约束化简为 (5.10)",
        "body": format!("起点：依赖清算比例 [[{ratio}]]。\n\n1. 代入 $P=B$。（补：书中略去）\n\n结果用于融资条件 [[{finance}]]。"),
    });
    let plan = mcp.ok("insert_card", insert.clone());
    assert_eq!(plan["applied"], false);
    assert_eq!(plan["confirm"], "insert");
    assert_eq!(
        plan["plan"],
        json!([
            "insert verification:",
            "new card -> Hart1995/1|a|a|3",
            "Hart1995/1|a|a|3 -> Hart1995/1|a|a|4",
            "Hart1995/1|a|a|4 -> Hart1995/1|a|a|5",
            "moved cards: 2",
            "link macros rewritten: 3"
        ])
    );
    assert_eq!(
        mcp.ok("get_card", json!({ "at": finance }))["title"],
        "融资条件",
        "a plan changes nothing"
    );
    let mut confirmed = insert.clone();
    confirmed["confirm"] = json!("insert");
    let done = mcp.ok("insert_card", confirmed);
    assert_eq!(done["applied"], true);
    assert_eq!(done["address"], "Hart1995/1|a|a|3");
    let derivation = mcp.ok("get_card", json!({ "at": "Hart1995/1|a|a|3" }));
    assert_eq!(derivation["facet"], "推导：参与约束");
    assert_eq!(derivation["emphasis"], "aside");
    // The new Card's Link to today's 融资条件 follows it down.
    assert!(
        derivation["body"]
            .as_str()
            .unwrap()
            .contains("融资条件 [[Hart1995/1|a|a|4]]"),
        "{}",
        derivation["body"]
    );
    let finance = "Hart1995/1|a|a|4".to_string();
    let example = "Hart1995/1|a|a|5".to_string();
    assert_eq!(
        mcp.ok("get_card", json!({ "at": finance }))["title"],
        "融资条件"
    );
    assert!(
        mcp.ok("get_card", json!({ "at": b_step }))["body"]
            .as_str()
            .unwrap()
            .contains("对照融资条件 [[Hart1995/1|a|a|4]]")
    );
    let inbound = mcp.ok("get_card", json!({ "at": finance }))["inbound"].clone();
    assert!(
        inbound
            .as_array()
            .unwrap()
            .iter()
            .any(|link| link["address"] == "Hart1995/1|a|a|3"),
        "Reverse links follow: {inbound}"
    );

    // Take the example off the main line, beside the step it illustrates.
    let moved = mcp.ok(
        "move_card",
        json!({ "at": example, "to": format!("{ratio}|a"), "alone": true, "confirm": "move" }),
    );
    assert_eq!(moved["address"], json!(format!("{ratio}|a")));
    // Move the outside Idea Card into the range before 清算比例, then give it a facet.
    let plan = mcp.ok("move_card", json!({ "at": "Hart1995/2", "before": ratio }));
    assert!(
        plan["plan"]
            .as_array()
            .unwrap()
            .contains(&json!("Hart1995/2 -> Hart1995/1|a|a|2")),
        "{plan}"
    );
    mcp.ok(
        "move_card",
        json!({ "at": "Hart1995/2", "before": ratio, "confirm": "move" }),
    );
    let definition = "Hart1995/1|a|a|2".to_string();
    let ratio = "Hart1995/1|a|a|3".to_string();
    assert_eq!(
        mcp.ok("get_card", json!({ "at": ratio }))["title"],
        "清算比例"
    );
    mcp.ok(
        "edit_card",
        json!({ "at": definition, "facet": "清算价值", "body": format!("依赖设定 [[{setting}]]。清算价值 L 是资产在时点 1 的变卖价。") }),
    );
    // Restate the Topic Card's claim in the model; keep the Topic Card as a neighbour.
    mcp.ok(
        "edit_card",
        json!({ "at": ratio, "body": format!("依赖可承诺的还款 [[{repay}]] 与清算价值 [[{definition}]]。$L>0$。对照 [[0/1]]。") }),
    );
    // Model B takes in the part it rests on, and rests on parts, not on model A.
    mcp.ok(
        "edit_card",
        json!({ "at": b, "body": format!("结构：[[{repay}]] [[{ratio}]]。") }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": b_setting, "body": format!("两期结构，对应 [[{a}]]。依赖清算比例 [[{ratio}]]。") }),
    );

    let model = mcp.ok("get_model", json!({ "at": a }));
    assert_eq!(
        build_order(&model),
        pairs(&[
            (&setting, "main"),
            (&repay, "main"),
            (&definition, "main"),
            (&ratio, "main"),
            (&format!("{ratio}|a"), "side"),
            ("Hart1995/1|a|a|4", "main"),
            ("Hart1995/1|a|a|5", "main"),
            (&limits, "off"),
        ])
    );
    assert_eq!(model["build"]["order"][4]["from"], json!(ratio));
    assert_eq!(model["build"]["order"][5]["derivation"], true);
    assert_eq!(
        model["build"]["self_sufficient"], true,
        "{}",
        model["build"]
    );
    let model_b = mcp.ok("get_model", json!({ "at": b }));
    assert_eq!(
        build_order(&model_b),
        pairs(&[
            (&ratio, "shared"),
            (&b_setting, "main"),
            (&repay, "shared"),
            (&b_step, "main")
        ])
    );
    assert_eq!(
        model_b["build"]["self_sufficient"], true,
        "{}",
        model_b["build"]
    );
    assert_eq!(
        model_b["build"]["tolerated"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (
                t["part"].as_str().unwrap(),
                t["premise"]["address"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        [
            (repay.as_str(), setting.as_str()),
            (ratio.as_str(), definition.as_str())
        ]
    );
}

#[test]
fn insert_and_move_between_refuse_what_they_cannot_do_and_reorder_one_step() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({ "title": "Chain" }));
    for at in ["0/0", "0/1", "0/2"] {
        mcp.ok(
            "create_successor",
            json!({ "at": at, "title": format!("after {at}") }),
        );
    }
    mcp.ok(
        "create_successor",
        json!({ "at": "0/2", "kind": "side", "title": "branch" }),
    );
    assert!(
        mcp.err("insert_card", json!({ "before": "0/0", "title": "x" }))
            .contains("cannot insert before a tree root")
    );
    assert!(
        mcp.err(
            "move_card",
            json!({ "at": "0/2", "to": "0/1|a", "alone": true })
        )
        .contains("`0/2` has Side successors")
    );
    assert!(
        mcp.err("move_card", json!({ "at": "0/1", "before": "0/3" }))
            .contains("cannot move a card before one of its own successors")
    );
    assert!(
        mcp.err(
            "move_card",
            json!({ "at": "0/1", "to": "0/1|a", "before": "0/3" })
        )
        .contains("not both")
    );
    assert!(
        mcp.err("move_card", json!({ "at": "0/1" }))
            .contains("`to` or `before`")
    );
    assert!(
        mcp.err(
            "move_card",
            json!({ "at": "0/1", "before": "0/2", "alone": true })
        )
        .contains("nothing moves")
    );
    // alone + before reorders one step: 0/3 goes first, 0/1 and 0/2 move down.
    let done = mcp.ok(
        "move_card",
        json!({ "at": "0/3", "before": "0/1", "alone": true, "confirm": "move" }),
    );
    assert_eq!(done["address"], "0/1");
    assert_eq!(
        done["moved"],
        json!({ "0/1": "0/2", "0/2": "0/3", "0/2|a": "0/3|a", "0/3": "0/1" })
    );
    let tree = mcp.ok("get_tree", json!({ "tree": "0" }));
    let titles = tree["cards"]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| {
            (
                card["address"].as_str().unwrap().to_string(),
                card["title"].as_str().unwrap().to_string(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        titles,
        pairs(&[
            ("0/0", "Chain"),
            ("0/1", "after 0/2"),
            ("0/2", "after 0/0"),
            ("0/3", "after 0/1"),
            ("0/3|a", "branch"),
        ])
    );
    // A tool list names both new fields.
    let tools = mcp.request("tools/list", json!({}));
    let tool = |name: &str| {
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("tool {name}"))
    };
    assert_eq!(
        tool("insert_card")["inputSchema"]["required"],
        json!(["before", "title"])
    );
    assert_eq!(tool("move_card")["inputSchema"]["required"], json!(["at"]));
    assert_eq!(
        tool("move_card")["inputSchema"]["properties"]["alone"]["type"],
        "boolean"
    );
}

#[test]
fn the_gui_inserts_a_card_and_moves_one_between_two_cards() {
    let home = TestHome::new();
    let gui = Gui::start(&home);
    gui.api(
        "POST",
        "/api/create",
        json!({ "kind": "topic", "text": "# Chain\n" }),
    );
    gui.api(
        "POST",
        "/api/create",
        json!({ "kind": "direct", "at": "0/0", "text": "# first\n\nsee [[0/0]]\n" }),
    );
    gui.api(
        "POST",
        "/api/create",
        json!({ "kind": "direct", "at": "0/1", "text": "# second\n\nsee [[0/1]]\n" }),
    );
    let plan = gui.api(
        "POST",
        "/api/insert",
        json!({ "before": "0/1", "facet": "推导", "text": "# step\n\nfrom [[0/1]]\n" }),
    );
    assert_eq!(plan["confirm"], "insert", "{plan}");
    assert_eq!(plan["lines"][1], "new card -> 0/1");
    let done = gui.api(
        "POST",
        "/api/insert",
        json!({ "before": "0/1", "facet": "推导", "text": "# step\n\nfrom [[0/1]]\n", "confirm": "insert" }),
    );
    assert_eq!(done["address"], "0/1");
    assert_eq!(done["moved"], json!({ "0/1": "0/2", "0/2": "0/3" }));
    let card = gui.api("GET", "/api/card?at=0%2F1", json!(null));
    assert_eq!(card["facet"], "推导");
    assert!(card["text"].as_str().unwrap().contains("from [[0/2]]"));
    let card = gui.api("GET", "/api/card?at=0%2F3", json!(null));
    assert!(card["text"].as_str().unwrap().contains("see [[0/2]]"));
    let done = gui.api(
        "POST",
        "/api/move",
        json!({ "at": "0/3", "before": "0/1", "alone": true, "confirm": "move" }),
    );
    assert_eq!(done["address"], "0/1");
    let card = gui.api("GET", "/api/card?at=0%2F1", json!(null));
    assert_eq!(card["title"], "second");
}

#[test]
fn a_delete_names_the_links_it_breaks_and_into_re_points_them() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({ "title": "Model" }));
    // 0/1 is a Model Card with three parts; two outside Cards link the parts.
    mcp.ok(
        "create_successor",
        json!({ "at": "0/0", "title": "模型", "type": "model" }),
    );
    for title in ["设定", "机制三", "机制四"] {
        mcp.ok(
            "create_successor",
            json!({ "at": "0/1", "kind": "side", "title": title, "facet": title }),
        );
    }
    mcp.ok(
        "create_successor",
        json!({ "at": "0/1", "title": "讨论", "body": "依赖机制三 [[0/1|b]]。对照机制四 [[0/1|c]]。" }),
    );
    mcp.ok(
        "create_successor",
        json!({ "at": "0/2", "title": "另一张", "body": "见 [[0/1|b]]。" }),
    );

    // Without `into`, the plan says which Links the delete would break.
    let plan = mcp.ok("delete_card", json!({ "at": "0/1|b" }));
    assert_eq!(
        plan["plan"],
        json!([
            "delete verification:",
            "delete 0/1|b",
            "successors: 0",
            "links that will break: 2",
            "0/2 -> 0/1|b",
            "0/3 -> 0/1|b"
        ])
    );
    // `into` must stay.
    assert!(
        mcp.err("delete_card", json!({ "at": "0/1", "into": "0/1|a" }))
            .contains("is deleted too")
    );

    // With `into`, the Links follow the content to the part that now holds it. Deleting 0/1|b
    // compacts 0/1|c to 0/1|b, so a Link re-pointed to 0/1|c follows it there too.
    let plan = mcp.ok("delete_card", json!({ "at": "0/1|b", "into": "0/1|c" }));
    assert_eq!(
        plan["plan"],
        json!([
            "delete verification:",
            "delete 0/1|b",
            "successors: 0",
            "links re-pointed to 0/1|c: 2",
            "0/2: [[0/1|b]] -> [[0/1|c]]",
            "0/3: [[0/1|b]] -> [[0/1|c]]"
        ])
    );
    let done = mcp.ok(
        "delete_card",
        json!({ "at": "0/1|b", "into": "0/1|c", "confirm": "delete" }),
    );
    assert_eq!(done["moved"], json!({ "0/1|c": "0/1|b" }));
    assert_eq!(
        mcp.ok("get_card", json!({ "at": "0/2" }))["body"],
        "依赖机制三 [[0/1|b]]。对照机制四 [[0/1|b]]。"
    );
    assert_eq!(
        mcp.ok("get_card", json!({ "at": "0/3" }))["body"],
        "见 [[0/1|b]]。"
    );
    assert_eq!(
        mcp.ok("get_card", json!({ "at": "0/1|b" }))["title"],
        "机制四"
    );
    assert_eq!(mcp.ok("list_broken_links", json!({}))["broken"], json!([]));
    assert_eq!(
        mcp.ok("get_model", json!({ "at": "0/1" }))["issues"],
        json!([])
    );

    // The re-point rewrites the linking Cards, so a human Card's Link stops it for an agent.
    let gui = Gui::start(&home);
    gui.api(
        "POST",
        "/api/create",
        json!({ "kind": "topic", "workspace": "human", "text": "# Mine\n\n见 [[0/1|a]]。\n" }),
    );
    drop(gui);
    assert!(
        mcp.err("delete_card", json!({ "at": "0/1|a", "into": "0/1|b" }))
            .contains("human")
    );
}
