//! Term uses stay out of reasoning and out of the default graph edges (S16, #158; design 10.2,
//! 15.2): they give no Model membership, relation, or neighbour, no Idea reasoning credit, and no
//! separate graph edge a reader must see by default, while every general Link reader — the stored
//! macro, Broken links, Term uses, Locks — still sees them.

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

    fn db(&self) -> Connection {
        Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db")
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
    }
}

/// The owner's browser routes: a human-workspace Card comes from here, never from the MCP server.
struct Gui {
    port: u16,
    token: String,
}

impl Gui {
    fn post(&self, path: &str, body: &Value) -> Value {
        use std::net::TcpStream;
        let payload = serde_json::to_string(body).expect("JSON body");
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let request = format!(
            "POST {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            self.port,
            self.token,
            payload.len()
        );
        stream.write_all(request.as_bytes()).expect("send request");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        let status = response
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse::<u16>().ok())
            .expect("status code");
        let start = response.find("\r\n\r\n").expect("header end") + 4;
        let value: Value = serde_json::from_str(&response[start..]).unwrap_or(Value::Null);
        assert_eq!(status, 200, "{path} answered {status}: {value}");
        value
    }
}

fn gui(home: &TestHome) -> Gui {
    let out = home.cmd().arg("gui").output().expect("zt gui");
    assert!(out.status.success(), "zt gui failed: {out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    let port: u16 = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("zt gui: http://127.0.0.1:"))
        .and_then(|rest| rest.trim_end_matches('/').parse().ok())
        .expect("zt gui prints its URL");
    use std::net::TcpStream;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to gui");
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .expect("request page");
    let mut page = String::new();
    stream.read_to_string(&mut page).expect("read page");
    let token = page
        .split("name=\"zt-token\" content=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("the page carries the GUI token")
        .to_string();
    Gui { port, token }
}

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

/// A two-row term table whose `均衡` head is published.
const TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n";

/// One Topic tree holding a model, and a second Topic tree holding a Term list: the model tree's
/// root `R` and its Model Card `R/1`; the term tree's root, its anchor Card, the Term list as the
/// anchor's Side successor and the head `均衡` under it.
struct SlipBox {
    root: String,
    model: String,
    list: String,
    head: String,
}

fn slip_box(mcp: &mut Mcp) -> SlipBox {
    let root = address(mcp.ok("create_topic", json!({ "title": "术语与模型" })));
    let model = address(mcp.ok(
        "create_successor",
        json!({ "at": &root, "title": "均衡模型", "type": "model", "body": "本模型说明价格。" }),
    ));
    let terms = address(mcp.ok("create_topic", json!({ "title": "术语" })));
    let anchor = address(mcp.ok(
        "create_successor",
        json!({ "at": &terms, "title": "第一章", "body": "设置。" }),
    ));
    let list = address(mcp.ok(
        "create_successor",
        json!({ "at": &anchor, "kind": "side", "title": "术语表：测试", "type": "term", "body": TABLE }),
    ));
    let head = address(mcp.ok(
        "create_successor",
        json!({ "at": &list, "kind": "side", "title": "均衡", "type": "term", "term": "均衡" }),
    ));
    SlipBox {
        root,
        model,
        list,
        head,
    }
}

fn part(mcp: &mut Mcp, model: &str, title: &str, facet: &str, body: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": model, "kind": "side", "title": title, "facet": facet, "body": body }),
    ))
}

/// The addresses of the model's neighbours, in report order.
fn neighbour_addresses(model: &Value) -> Vec<String> {
    model["neighbours"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|neighbour| neighbour["card"]["address"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn a_term_use_is_no_model_neighbour_but_a_general_link_read_still_sees_it() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let boxed = slip_box(&mut mcp);
    // Another Card outside the model, reached by an announced ordinary Link: the control.
    let outside = address(mcp.ok(
        "create_successor",
        json!({ "at": &boxed.model, "kind": "side", "title": "价格指数", "body": "价格指数衡量价格水平。" }),
    ));
    let body = format!(
        "本模型依赖 [[{outside}]] 的价格水平，并参照 [[均衡@{}]] 一词。",
        boxed.head
    );
    mcp.ok("edit_card", json!({ "at": &boxed.model, "body": body.clone() }));

    // The graph projection keeps every authored target in `links` (a general Link reader sees the
    // Term use) and names the Term edges apart, so a default graph view can omit exactly them.
    let tree = mcp.ok("get_tree", json!({ "tree": &boxed.root, "bodies": false }));
    let summary = tree["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|card| card["address"] == json!(boxed.model))
        .cloned()
        .expect("the Model Card");
    assert_eq!(summary["links"], json!([outside, boxed.head]));
    assert_eq!(summary["kinds"], json!({ outside.clone(): "rests-on" }));
    assert_eq!(summary["term_use_targets"], json!([boxed.head]));

    // The model reads ordinary Links only: the announced Link is a neighbour, the Term use is not.
    let model = mcp.ok("get_model", json!({ "at": &boxed.model }));
    assert_eq!(neighbour_addresses(&model), [outside.clone()]);
    assert_eq!(model["head_links"], json!([]));
    assert_eq!(model["parts"], json!([]));

    // The reader's own Card projection agrees: the macro is authored, the Term use is one, and the
    // model structure on the Card carries no Term neighbour.
    let card = mcp.ok("get_card", json!({ "at": &boxed.model }));
    assert_eq!(card["body"], json!(body));
    assert_eq!(neighbour_addresses(&card["model"]), [outside]);
    assert_eq!(card["model"]["head_links"], json!([]));
}

#[test]
fn a_retyped_term_head_takes_no_model_membership_from_a_term_use() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let boxed = slip_box(&mut mcp);
    // The Model Card names the head only by a Term use.
    let body = format!("本模型参照 [[均衡@{}]] 一词。", boxed.head);
    mcp.ok("edit_card", json!({ "at": &boxed.model, "body": body.clone() }));

    // The head is retyped into a Model part, as an existing archive may hold: the stored Term use
    // to it stays authored but is no longer a valid one.
    mcp.ok(
        "edit_card",
        json!({ "at": &boxed.head, "type": "model", "facet": "设定" }),
    );

    // The Term use gives the retyped part no membership: it belongs to no model, and the model
    // takes in no part and no neighbour through it.
    let card = mcp.ok("get_card", json!({ "at": &boxed.head }));
    assert_eq!(card["type"], json!("model"));
    assert_eq!(card["facet"], json!("设定"));
    assert_eq!(card["model"]["head"], json!(null), "{card:#?}");
    assert!(
        card["model"]["issues"]
            .as_array()
            .is_some_and(|issues| !issues.is_empty()),
        "a Model part in no model says so: {card:#?}"
    );
    let model = mcp.ok("get_model", json!({ "at": &boxed.model }));
    assert_eq!(model["parts"], json!([]), "{model:#?}");
    assert_eq!(model["neighbours"], json!([]), "{model:#?}");

    // Nothing is hidden from the general readers: the broken Term use, its label, the stored
    // macro, the generated section and the Term list's own fault all still name it.
    let broken = mcp.ok("list_broken_links", json!({}));
    let rows = broken["broken"].as_array().expect("broken rows");
    let row = rows
        .iter()
        .find(|row| row["source"] == json!(boxed.model))
        .expect("the broken Term use is reported");
    assert_eq!(row["target"], json!(boxed.head));
    assert_eq!(row["label"], json!("均衡"));
    assert_eq!(row["reason"], json!("not_head"));
    let source = mcp.ok("get_card", json!({ "at": &boxed.model }));
    assert_eq!(source["body"], json!(body));
    assert!(
        source["text"]
            .as_str()
            .unwrap()
            .contains(&format!("[[均衡@{}]]", boxed.head)),
        "the authored macro stays byte for byte"
    );
    let terms = mcp.ok("get_terms", json!({ "at": &boxed.list }));
    // The retyped part is no longer a head, so its table row publishes no head: the list keeps the
    // coverage fault, and the broken use above names the same fact from the source side.
    let faults = terms["faults"].as_array().expect("faults");
    assert!(
        faults
            .iter()
            .any(|fault| fault["fault"] == "term_row_without_card" && fault["term"] == json!("均衡")),
        "the list reports its row without a head: {terms:#?}"
    );
    assert_eq!(terms["coverage"]["heads"], json!(0));
    assert_eq!(terms["coverage"]["uses"], json!(0));
}

#[test]
fn a_term_use_is_no_model_relation_between_parts() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let boxed = slip_box(&mut mcp);
    let setting = part(&mut mcp, &boxed.model, "设定", "设定", "双方各有一项资产。");
    // The head is named by an ordinary Link from the Model Card, so it belongs to the model (the
    // control); then a part writes a Term use to it (legal while it is a Term head).
    let uses = format!("见 [[均衡@{}]] 一词，本卡讨论价格。", boxed.head);
    let dynamic = part(&mut mcp, &boxed.model, "动态", "动态", &uses);
    mcp.ok(
        "edit_card",
        json!({ "at": &boxed.model, "body": format!("本模型参照 [[{}]] 的定义。", boxed.head) }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": &boxed.head, "type": "model", "facet": "设定" }),
    );

    // The ordinary Link still takes the retyped part in: the control that membership itself works.
    let model = mcp.ok("get_model", json!({ "at": &boxed.model }));
    let parts = model["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|part| part["address"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        parts,
        [boxed.head.clone(), setting.clone(), dynamic.clone()],
        "the model's parts"
    );
    let relations = model["relations"].as_array().expect("relations");
    assert_eq!(
        relations,
        &Vec::<Value>::new(),
        "a Term use is no relation between parts"
    );
    let dynamic_part = model["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|part| part["address"] == json!(dynamic))
        .cloned()
        .expect("the dynamic part");
    assert_eq!(
        dynamic_part["links"],
        json!([]),
        "a Term use is no part Link: {dynamic_part:#?}"
    );
}

#[test]
fn a_term_use_across_workspaces_is_no_reasoning_edge_and_stays_readable() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    // A Term list and a head in the owner's own Workspace, written from the browser.
    let gui = gui(&home);
    let human_root = address(gui.post(
        "/api/create",
        &json!({ "kind": "topic", "workspace": "human", "text": "# 人类的术语\n\n正文。\n" }),
    ));
    let anchor = address(gui.post(
        "/api/create",
        &json!({ "kind": "direct", "at": human_root, "text": "# 第一章\n\n设置。\n" }),
    ));
    let list = address(gui.post(
        "/api/create",
        &json!({
            "kind": "side", "at": anchor, "type": "term",
            "text": format!("# 术语表：人类\n\n{}", TABLE.trim_start_matches("## 术语表\n\n")),
        }),
    ));
    let head = address(gui.post(
        "/api/create",
        &json!({ "kind": "side", "at": list, "type": "term", "term": "均衡", "text": "# 均衡\n" }),
    ));
    // An agent Model Card links the human head only by a Term use: a Link may cross Workspaces.
    let root = address(mcp.ok("create_topic", json!({ "title": "跨工作区" })));
    let body = format!("本模型参照 [[均衡@{head}]] 一词。");
    let model = address(mcp.ok(
        "create_successor",
        json!({ "title": "均衡模型", "type": "model", "body": body, "at": &root }),
    ));

    // The cross-Workspace Term use is no Model evidence.
    let structure = mcp.ok("get_model", json!({ "at": &model }));
    assert_eq!(structure["parts"], json!([]), "{structure:#?}");
    assert_eq!(structure["neighbours"], json!([]), "{structure:#?}");
    let read = mcp.ok("get_cards", json!({ "addresses": [&model], "view": "text" }));
    assert_eq!(read["cards"][0]["kinds"], json!({}));
    assert_eq!(read["cards"][0]["term_use_targets"], json!([head]));
    // The general readers still see the use from both sides, and it is no Broken link.
    assert_eq!(mcp.ok("list_broken_links", json!({}))["broken"], json!([]));
    let head_card = mcp.ok("get_card", json!({ "at": &head }));
    assert_eq!(head_card["term"], json!("均衡"));
    assert_eq!(head_card["term_uses"], json!(1), "the readable use count");
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    assert_eq!(
        terms["uses"],
        json!([{
            "source": model,
            "source_title": "均衡模型",
            "target": head,
            "label": "均衡",
            "count": 1,
        }])
    );
}

#[test]
fn a_question_walk_never_takes_a_term_edge() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    // One book whose Card names a Term head by a Term use and nothing else, and the register that
    // accepts the Card's term.
    let book = address(mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{bookA, title={Book A}}", "body": "该书。" }),
    ));
    assert_eq!(book, "bookA");
    let anchor = address(mcp.ok(
        "create_successor",
        json!({ "at": "bookA", "title": "第一章", "body": "设置。" }),
    ));
    let list = address(mcp.ok(
        "create_successor",
        json!({ "at": &anchor, "kind": "side", "title": "术语表：书 A", "type": "term", "body": TABLE }),
    ));
    let head = address(mcp.ok(
        "create_successor",
        json!({ "at": &list, "kind": "side", "title": "均衡", "type": "term", "term": "均衡" }),
    ));
    let entry = address(mcp.ok(
        "create_successor",
        json!({
            "at": &anchor,
            "kind": "side",
            "title": "多重均衡来自策略互补",
            "body": format!("多重均衡 [[均衡@{head}]] 来自策略互补。\n\n来源：[[bookA]]"),
        }),
    ));
    let index = address(mcp.ok("create_topic", json!({ "title": "索引", "body": "本主题收放入口。" })));
    mcp.ok(
        "create_successor",
        json!({
            "at": index, "type": "hub", "level": "register",
            "title": "索引：代理工作区的入口",
            "body": format!("## 词条\n- 多重均衡：[[{entry}]]\n"),
        }),
    );

    let proposal = mcp.ok("ask_box", json!({ "question": "多重均衡为何出现？" }));
    let cards = proposal["cards"]
        .as_array()
        .expect("proposal cards")
        .iter()
        .map(|card| card["at"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(cards, [entry], "the walk takes no Term edge: {proposal:#?}");
    assert_eq!(proposal["threads"], json!([]), "{proposal:#?}");
}
