//! Authoring Term Cards and completing Term links in the existing editors (S14, #161; design
//! 15.1, 15.2).
//!
//! The verification seam of the issue: the real MCP reads and writes an editor needs (a Term use
//! in `list_links` and in a Card's `outbound`, the sixth type in tree counts and summaries, the
//! write and preview check replies) plus the real GUI routes the writing desk calls. The terminal
//! and GUI editors themselves are exercised by their own suites; this file proves the surfaces
//! they read and write.
//!
//! Every test runs against a scratch archive (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`). The live archive
//! is never touched.

#![allow(dead_code)]

use assert_cmd::Command;
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

    /// One shell command whose editor writes `text` into the opened file.
    fn cmd_with_editor(&self, text: &str) -> Command {
        let script = self.config_dir.join("fake-term-editor.ps1");
        fs::create_dir_all(&self.config_dir).expect("config dir");
        fs::write(
            &script,
            concat!(
                "[IO.File]::WriteAllText($args[0], $env:ZT_EDITOR_TEXT)\n",
                "if ($env:ZT_EDITOR_EXIT) { exit ([int]$env:ZT_EDITOR_EXIT) }\n",
            ),
        )
        .expect("editor script");
        let mut cmd = self.cmd();
        cmd.env(
            "EDITOR",
            format!(
                "powershell -NoProfile -ExecutionPolicy Bypass -File {}",
                script.display()
            ),
        );
        cmd.env("ZT_EDITOR_TEXT", text);
        cmd
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

    fn gui(&self) -> Gui {
        let out = self.cmd().arg("gui").output().expect("zt gui");
        assert!(out.status.success(), "zt gui failed: {out:?}");
        let text = String::from_utf8_lossy(&out.stdout);
        let port: u16 = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("zt gui: http://127.0.0.1:"))
            .and_then(|rest| rest.trim_end_matches('/').parse().ok())
            .expect("zt gui prints its URL");
        let page = http_page(port);
        let token = page
            .split("name=\"zt-token\" content=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the page carries the GUI token")
            .to_string();
        Gui { port, token }
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
        let reply: Value = serde_json::from_str(&line).expect("MCP reply is JSON");
        assert_eq!(reply["id"], id, "replies arrive in request order");
        reply
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

    fn err(&mut self, name: &str, arguments: Value) -> Value {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(is_error, "{name}({arguments}) must fail: {value}");
        value
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

struct Gui {
    port: u16,
    token: String,
}

impl Gui {
    fn get(&self, path: &str) -> Value {
        let (status, body) = self.request("GET", path, None);
        assert_eq!(status, 200, "{path} answered {status}: {body}");
        body
    }

    fn post(&self, path: &str, body: &Value) -> Value {
        let (status, value) = self.request("POST", path, Some(body));
        assert_eq!(status, 200, "{path} answered {status}: {value}");
        value
    }

    /// One POST whose refusal (`4xx`) is part of what the test asserts.
    fn post_status(&self, path: &str, body: &Value) -> (u16, Value) {
        self.request("POST", path, Some(body))
    }

    fn request(&self, method: &str, path: &str, body: Option<&Value>) -> (u16, Value) {
        use std::net::TcpStream;
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let text = body.map(Value::to_string).unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{text}",
            self.port,
            self.token,
            text.len()
        );
        stream.write_all(request.as_bytes()).expect("send request");
        read_reply(stream)
    }
}

fn read_reply(mut stream: std::net::TcpStream) -> (u16, Value) {
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    let status = response
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("status code");
    let start = response.find("\r\n\r\n").expect("header end") + 4;
    let body = serde_json::from_str(&response[start..]).unwrap_or(Value::Null);
    (status, body)
}

fn http_page(port: u16) -> String {
    use std::net::TcpStream;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to gui");
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .expect("request page");
    let mut page = String::new();
    stream.read_to_string(&mut page).expect("read page");
    page
}

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

fn enc(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

/// A two-row term table: `均衡` is published as a head, `弹性` is still missing.
const TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n| 弹性 | 弹性系数 | 价格变化的反应程度 | §2.3 |\n";

/// One agent Literature tree whose anchor holds one Term list under `<key>/1|a`.
fn book_with_table(home: &TestHome, mcp: &mut Mcp, key: &str, table: &str) -> (String, String) {
    mcp.ok(
        "create_literature",
        json!({ "bibtex": format!("@book{{{key}, title={{Test Book}}}}") }),
    );
    let anchor = address(mcp.ok(
        "create_successor",
        json!({ "at": key, "title": "第一章", "body": "设置。" }),
    ));
    let list = address(mcp.ok(
        "create_successor",
        json!({
            "at": anchor,
            "kind": "side",
            "title": "术语表：测试",
            "type": "term",
            "body": table,
        }),
    ));
    let _ = home;
    (anchor, list)
}

fn head(mcp: &mut Mcp, at: &str, term: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({
            "at": at,
            "kind": "side",
            "title": term,
            "type": "term",
            "term": term,
            "body": "## 释义\n\n两个参与者都不愿单方面改变策略。\n",
        }),
    ))
}

fn idea(mcp: &mut Mcp, at: &str, title: &str, body: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "kind": "side", "title": title, "body": body }),
    ))
}

#[test]
fn list_links_and_card_outbound_name_a_term_use_with_its_kind_and_label() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "EditBook", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    // A Term use and an ordinary reasoned citation of the same head are two different macros; an
    // editor must be able to write either one (S14-AC03).
    let term_source = idea(
        &mut mcp,
        &anchor,
        "使用术语",
        &format!("这一策略是 [[均衡@{eq}]] 的核心。"),
    );
    let ordinary_source = idea(
        &mut mcp,
        &anchor,
        "引用定义",
        &format!("这一策略按 [[{eq}]] 的定义成立。"),
    );

    let links = mcp.ok("list_links", json!({ "workspace": "agent" }));
    let row = |source: &str| {
        links["links"]
            .as_array()
            .expect("links")
            .iter()
            .find(|link| link["source"] == source && link["target"] == eq)
            .cloned()
            .expect("one source-target row")
    };
    assert_eq!(row(&term_source)["kind"], json!("term"));
    assert_eq!(row(&term_source)["label"], json!("均衡"));
    assert_eq!(
        row(&ordinary_source),
        json!({
            "source": ordinary_source,
            "source_workspace": "agent",
            "target": eq,
            "target_workspace": "agent",
            "broken": false,
        }),
        "an ordinary reasoned citation keeps the existing row shape"
    );

    let card = mcp.ok("get_card", json!({ "at": term_source }));
    assert_eq!(card["outbound"][0]["address"], eq);
    assert_eq!(card["outbound"][0]["kind"], json!("term"));
    assert_eq!(card["outbound"][0]["label"], json!("均衡"));
    let ordinary = mcp.ok("get_card", json!({ "at": ordinary_source }));
    assert_eq!(
        ordinary["outbound"][0].get("kind"),
        None,
        "an ordinary Link is not a Term use: {}",
        ordinary["outbound"][0]
    );
    // The tree summary names the Term targets apart from ordinary Links, as the graph reader
    // expects (S16, #158).
    let tree = mcp.ok("get_tree", json!({ "tree": "EditBook" }));
    let summary = |source: &str| {
        tree["cards"]
            .as_array()
            .expect("cards")
            .iter()
            .find(|card| card["address"] == source)
            .cloned()
            .expect("the source row")
    };
    assert_eq!(summary(&term_source)["term_use_targets"], json!([eq]));
    assert_eq!(summary(&ordinary_source)["term_use_targets"], json!([]));
    assert_eq!(summary(&ordinary_source)["links"], json!([eq]));
}

#[test]
fn shell_create_edit_and_insert_accept_type_and_term_arguments() {
    let home = TestHome::new();
    home.cmd_with_editor("# Terms\n\ndescription\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Terms"])
        .assert()
        .success();
    let (title, body) = ("# 术语表\n", format!("\n{TABLE}\n<!-- zt:reverse-links -->\n"));
    home.cmd_with_editor(&(title.to_string() + &body))
        .args(["n", "--at", "0/0", "--type", "term"])
        .assert()
        .success()
        .stdout(predicates::str::contains("0/1"));
    home.cmd_with_editor("# 均衡\n\n## 释义\n\n两个参与者都不愿单方面改变策略。\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "0/1", "--type", "term", "--term", "均衡"])
        .assert()
        .success()
        .stdout(predicates::str::contains("0/1|a"));

    let mut mcp = home.mcp();
    let card = mcp.ok("get_card", json!({ "at": "0/1|a" }));
    assert_eq!(card["type"], json!("term"));
    assert_eq!(card["term"], json!("均衡"));
    assert_eq!(card["term_role"], json!("card"));
    assert_eq!(card["term_list"], json!("0/1"));
    assert_eq!(mcp.ok("get_card", json!({ "at": "0/1" }))["term"], Value::Null);

    // `e` changes the name alone, clears it with an empty value, and retyping away clears it.
    home.cmd_with_editor("# 均衡\n\n## 释义\n\n两个参与者都不愿单方面改变策略。\n\n<!-- zt:reverse-links -->\n")
        .args(["e", "--at", "0/1|a", "--term", "替代"])
        .assert()
        .success();
    assert_eq!(
        mcp.ok("get_card", json!({ "at": "0/1|a" }))["term"],
        json!("替代")
    );
    home.cmd_with_editor("# 均衡\n\n## 释义\n\n两个参与者都不愿单方面改变策略。\n\n<!-- zt:reverse-links -->\n")
        .args(["e", "--at", "0/1|a", "--term", ""])
        .assert()
        .success();
    assert_eq!(
        mcp.ok("get_card", json!({ "at": "0/1|a" }))["term"],
        Value::Null
    );
    home.cmd_with_editor("# 均衡\n\n## 释义\n\n两个参与者都不愿单方面改变策略。\n\n<!-- zt:reverse-links -->\n")
        .args(["e", "--at", "0/1|a", "--type", "idea"])
        .assert()
        .success();
    let card = mcp.ok("get_card", json!({ "at": "0/1|a" }));
    assert_eq!(card["type"], json!("idea"));
    assert_eq!(card["term"], Value::Null, "retyping away clears the name");

    // `ins` writes the new Card with the same fields after its confirmation.
    home.cmd_with_editor("# 插入\n\n## 释义\n\n新插入的术语。\n\n<!-- zt:reverse-links -->\n")
        .write_stdin("insert\n")
        .args(["ins", "--at", "0/1|a", "--type", "term", "--term", "插入"])
        .assert()
        .success()
        .stdout(predicates::str::contains("0/1|a"));
    let inserted = mcp.ok("get_card", json!({ "at": "0/1|a" }));
    assert_eq!(inserted["type"], json!("term"));
    assert_eq!(inserted["term"], json!("插入"));

    // The flags refuse an inconsistent request before the editor opens.
    home.cmd_with_editor("# Refused\n\nbody\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0", "--term", "均衡"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "only Term Cards can have a term name; set `type` to `term` first",
        ));
    home.cmd_with_editor("# Refused\n\nbody\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0", "--type", "fact"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "unknown card type `fact`; expected idea, event, finding, model, hub, or term",
        ));
    home.cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicates::str::contains("--type <type>"))
        .stdout(predicates::str::contains("--term <name>"));
}

#[test]
fn gui_create_edit_and_insert_commit_term_names_and_refuse_incompatible_fields() {
    let home = TestHome::new();
    let gui = home.gui();
    let topic = address(gui.post(
        "/api/create",
        &json!({ "kind": "topic", "text": "# 人类术语\n\n一个人类主题。\n" }),
    ));
    let list = address(gui.post(
        "/api/create",
        &json!({ "kind": "direct", "at": topic, "type": "term", "text": format!("# 术语表\n\n{TABLE}") }),
    ));
    let head = address(gui.post(
        "/api/create",
        &json!({
            "kind": "side", "at": list, "type": "term", "term": "均衡",
            "text": "# 均衡\n\n## 释义\n\n两个参与者都不愿单方面改变策略。\n",
        }),
    ));
    let card = gui.get(&format!("/api/card?at={}", enc(&head)));
    assert_eq!(card["type"], json!("term"));
    assert_eq!(card["term"], json!("均衡"));
    assert_eq!(card["term_facts"]["role"], json!("card"));

    // The writing desk's Save sends text and name together; `ty` sends either alone.
    let saved = gui.post(
        "/api/edit",
        &json!({ "at": head, "text": card["text"], "base": card["text"], "term": " 替代 " }),
    );
    assert_eq!(saved["address"], json!(head));
    assert_eq!(
        gui.get(&format!("/api/card?at={}", enc(&head)))["term"],
        json!("替代"),
        "the route trims the name as every other route does"
    );
    gui.post("/api/edit", &json!({ "at": head, "term": Value::Null }));
    assert_eq!(
        gui.get(&format!("/api/card?at={}", enc(&head)))["term"],
        Value::Null
    );
    gui.post("/api/edit", &json!({ "at": head, "type": "term", "term": "均衡" }));
    gui.post("/api/edit", &json!({ "at": head, "type": "idea" }));
    assert_eq!(
        gui.get(&format!("/api/card?at={}", enc(&head)))["term"],
        Value::Null,
        "retyping away clears the name"
    );

    // Incompatible fields refuse before anything is written.
    let (status, refusal) = gui.post_status(
        "/api/create",
        &json!({ "kind": "side", "at": list, "type": "term", "term": "均衡", "facet": "setting", "text": "# 复合\n\nbody\n" }),
    );
    assert_eq!(status, 400);
    assert!(
        refusal["error"]
            .as_str()
            .unwrap_or_default()
            .contains("only Model Cards can have a Model facet"),
        "{refusal}"
    );
    let (status, refusal) = gui.post_status(
        "/api/create",
        &json!({ "kind": "topic", "type": "term", "term": "均衡", "text": "# 主题\n\nbody\n" }),
    );
    assert_eq!(status, 400);
    assert!(
        refusal["error"]
            .as_str()
            .unwrap_or_default()
            .contains("only Regular Cards"),
        "{refusal}"
    );

    // Insert takes the same fields after the confirmation the plan names.
    let plan = gui.post("/api/insert", &json!({ "before": head, "text": "# (new)\n" }));
    let inserted = gui.post(
        "/api/insert",
        &json!({
            "before": head, "text": "# 插入\n\n## 释义\n\n插入的术语。\n",
            "confirm": plan["confirm"], "type": "term", "term": "插入",
        }),
    );
    assert_eq!(inserted["address"], json!(head), "the new Card takes the place");
    let inserted = gui.get(&format!("/api/card?at={}", enc(&head)));
    assert_eq!(inserted["type"], json!("term"));
    assert_eq!(inserted["term"], json!("插入"));
}

#[test]
fn an_owner_written_term_list_and_its_heads_may_live_in_a_topic_tree() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let topic = address(mcp.ok("create_topic", json!({ "title": "术语专题" })));
    let anchor = address(mcp.ok(
        "create_successor",
        json!({ "at": topic, "title": "锚点", "body": "一条思路。" }),
    ));
    let list = address(mcp.ok(
        "create_successor",
        json!({
            "at": anchor, "kind": "side", "type": "term",
            "title": "术语表：专题", "body": TABLE,
        }),
    ));
    let eq = head(&mut mcp, &list, "均衡");
    let structure = mcp.ok("get_terms", json!({ "at": list }));
    assert_eq!(structure["rows"].as_array().expect("rows").len(), 2);
    assert_eq!(structure["coverage"]["heads"], json!(1));
    assert_eq!(
        mcp.ok("get_card", json!({ "at": eq }))["term_role"],
        json!("card"),
        "a head is computed from its placement, which a Topic tree also offers"
    );
}

#[test]
fn a_term_write_and_preview_return_structured_check_data_and_store_nothing_refused() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "GateBook", TABLE);
    // Advisory stage: a Term head that breaks the authored template is stored with its T advice.
    let written = mcp.ok(
        "create_successor",
        json!({
            "at": list, "kind": "side", "type": "term", "term": "均衡",
            "title": "均衡", "body": "只有一句解释。\n",
        }),
    );
    let warnings = written["warnings"].as_array().expect("warnings");
    assert!(
        warnings.iter().any(|warning| warning["rule"]
            .as_str()
            .is_some_and(|rule| rule.starts_with('T'))),
        "the sixth type's own rules reach the write reply: {warnings:?}"
    );
    assert_eq!(written["check"]["complete"], json!(true));

    // Enforce: a blocking T rule refuses the same write with its typed lines, and the address it
    // would have taken stays free.
    home.cmd()
        .args(["config", "set", "gate.stage", "enforce"])
        .assert()
        .success();
    let refused = mcp.err(
        "create_successor",
        json!({
            "at": list, "kind": "side", "type": "term", "term": "弹性",
            "title": "弹性", "body": "只有一句解释。\n",
        }),
    );
    let refused_address = refused["address"].as_str().expect("address").to_string();
    assert_eq!(refused["error"], json!("card_check"));
    assert!(refused["blocking"].as_u64().expect("blocking count") >= 1);
    let diagnostics = refused["diagnostics"].as_array().expect("diagnostics");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic["rule"]
            .as_str()
            .is_some_and(|rule| rule.starts_with('T'))),
        "{diagnostics:?}"
    );
    assert!(
        refused["faults"]
            .as_array()
            .expect("faults")
            .iter()
            .all(|line| line.as_str().is_some()),
        "the desk paints each line as text: {}",
        refused["faults"]
    );

    // The preview aggregates the same refusal, names the item, and writes nothing.
    let preview = mcp.ok(
        "apply_batch",
        json!({
            "preview": true,
            "items": [{
                "op_key": "term-edit/unit/1/gate/refused-head",
                "client_ref": "refused",
                "tool": "create_successor",
                "arguments": {
                    "at": list, "kind": "side", "type": "term", "term": "弹性",
                    "title": "弹性", "body": "只有一句解释。\n",
                },
            }],
        }),
    );
    assert_eq!(preview["complete"], json!(false));
    assert_eq!(preview["scan"]["refused"], json!(1));
    let refusal = &preview["refusals"][0];
    assert_eq!(refusal["index"], json!(0));
    assert_eq!(refusal["client_ref"], json!("refused"));
    assert_eq!(refusal["refusal"]["error"], json!("card_check"));
    assert!(
        refusal["refusal"]["diagnostics"]
            .as_array()
            .expect("preview diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["rule"]
                .as_str()
                .is_some_and(|rule| rule.starts_with('T'))),
        "{refusal}"
    );
    let (missing, is_error) = mcp.call("get_card", json!({ "at": refused_address }));
    assert!(is_error, "the refused Card is never stored: {missing}");
    let _ = anchor;
}

#[test]
fn a_batch_creates_and_retypes_term_cards_with_their_names() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let topic = address(mcp.ok("create_topic", json!({ "title": "批量术语" })));
    let applied = mcp.ok(
        "apply_batch",
        json!({
            "items": [
                {
                    "op_key": "term-edit/unit/1/batch/list",
                    "client_ref": "list",
                    "tool": "create_successor",
                    "arguments": { "at": topic, "title": "术语表：批量", "body": TABLE, "type": "term" },
                },
                {
                    "op_key": "term-edit/unit/1/batch/head",
                    "client_ref": "head",
                    "tool": "create_successor",
                    "arguments": {
                        "at": "@@list", "kind": "side", "title": "均衡", "type": "term", "term": "均衡",
                        "body": "## 释义\n\n两个参与者都不愿单方面改变策略。\n",
                    },
                },
                {
                    "op_key": "term-edit/unit/1/batch/idea",
                    "client_ref": "idea",
                    "tool": "create_successor",
                    "arguments": { "at": "@@list", "title": "待改类型", "body": "一个普通想法。" },
                },
                {
                    "op_key": "term-edit/unit/1/batch/retype",
                    "client_ref": "retype",
                    "tool": "edit_card",
                    "arguments": {
                        "at": "@@idea", "base_receipt": "term-edit/unit/1/batch/idea",
                        "type": "term", "term": "弹性",
                    },
                },
            ],
        }),
    );
    let results = applied["results"].as_array().expect("results");
    assert_eq!(results.len(), 4);
    let list = results[0]["result"]["address"].as_str().expect("list address").to_string();
    let head = results[1]["result"]["address"].as_str().expect("head address").to_string();
    let retyped = results[3]["result"]["address"].as_str().expect("retyped address").to_string();
    assert_eq!(
        (
            mcp.ok("get_card", json!({ "at": &head }))["term"].clone(),
            mcp.ok("get_card", json!({ "at": &head }))["term_role"].clone(),
        ),
        (json!("均衡"), json!("card")),
        "a batch create stores the exact name and computes the head role"
    );
    let retyped_card = mcp.ok("get_card", json!({ "at": &retyped }));
    assert_eq!(retyped_card["type"], json!("term"));
    assert_eq!(retyped_card["term"], json!("弹性"));
    assert_eq!(mcp.ok("get_card", json!({ "at": &list }))["term"], Value::Null);
}

#[test]
fn tree_counts_and_summaries_recognize_the_sixth_card_type() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (_anchor, list) = book_with_table(&home, &mut mcp, "CountBook", TABLE);
    let eq = head(&mut mcp, &list, "均衡");

    let trees = mcp.ok("list_trees", json!({ "workspace": "agent" }));
    let tree = trees["trees"]
        .as_array()
        .expect("trees")
        .iter()
        .find(|tree| tree["root"] == "CountBook")
        .cloned()
        .expect("the book tree");
    assert_eq!(tree["terms"], json!(2), "the list and its head: {tree}");

    // The same count and the head facts the writing desk completes from, through the GUI routes.
    let gui = home.gui();
    let overview = gui.get("/api/overview");
    let tree = overview["trees"]
        .as_array()
        .expect("overview trees")
        .iter()
        .find(|tree| tree["root"] == "CountBook")
        .cloned()
        .expect("the book tree in the overview");
    assert_eq!(tree["terms"], json!(2));
    let node = |nodes: &Value| {
        nodes
            .as_array()
            .expect("nodes")
            .iter()
            .find(|node| node["address"] == eq)
            .cloned()
            .expect("the head node")
    };
    let graph = node(&gui.get("/api/graph")["nodes"]);
    assert_eq!(graph["type"], json!("term"));
    assert_eq!(graph["term"], json!("均衡"));
    assert_eq!(graph["term_role"], json!("card"));
    assert_eq!(graph["term_list"], json!(list));
    let tree_view = node(&gui.get(&format!("/api/tree?id=CountBook"))["cards"]);
    assert_eq!(tree_view["term"], json!("均衡"));
    assert_eq!(tree_view["term_role"], json!("card"));

    // `get_tree` names the exact term beside the computed role and list.
    let read = mcp.ok("get_tree", json!({ "tree": "CountBook" }));
    let row = read["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|card| card["address"] == eq)
        .cloned()
        .expect("the head row");
    assert_eq!(row["term"], json!("均衡"));
    assert_eq!(row["term_role"], json!("card"));
    assert_eq!(row["term_list"], json!(list));

    // The lean bulk read answers the same Term facts, so an editor that reads text only still
    // knows the name, role, and list.
    let lean = mcp.ok("get_cards", json!({ "addresses": [eq], "view": "text" }));
    assert_eq!(lean["cards"][0]["term"], json!("均衡"));
    assert_eq!(lean["cards"][0]["term_role"], json!("card"));
    assert_eq!(lean["cards"][0]["term_list"], json!(list));

    // The GUI's search chips count and filter the sixth type through the existing route.
    let search = gui.get("/api/search?q=CountBook&type=term");
    assert_eq!(search["types"]["term"], json!(2));
    assert_eq!(
        search["results"]
            .as_array()
            .expect("search results")
            .iter()
            .map(|row| row["type"].clone())
            .collect::<Vec<_>>(),
        vec![json!("term"), json!("term")]
    );
}

#[test]
fn a_term_list_body_stays_the_unchanged_four_column_table() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (_anchor, list) = book_with_table(&home, &mut mcp, "BodyBook", TABLE);
    let card = mcp.ok("get_card", json!({ "at": list }));
    assert_eq!(
        card["body"],
        json!(TABLE.trim_end_matches('\n')),
        "a Term list keeps its authored four-column body byte for byte"
    );
    let rows = zt_check::text::term_table(card["body"].as_str().expect("body"));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].term, "均衡");
    assert_eq!(rows[1].term, "弹性");
}
