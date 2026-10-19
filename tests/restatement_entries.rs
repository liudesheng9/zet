//! Restatement entries as ordinary navigation Cards (S08, #107; design §4.5, D084-D087).
//!
//! An accepted restatement keeps the substantive statement at its home and writes a small explicit
//! entry beside it: an ordinary Idea Card whose `## 同述入口` section names exactly one existing
//! substantive Card of the same Workspace. These tests prove the public seam the rest of the slice
//! builds on: the derived navigation role (`restatement` on every Card read and on `export_titles`),
//! the terminal reference, the chain and cycle findings, the exact statement fingerprint, and that
//! the register view never serves a chain or a cycle as a confirmed entry.

use assert_cmd::Command;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
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

    fn mcp(&self) -> Mcp {
        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("zt"))
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

    /// The running service's GUI: its port and the token the page carries (the owner's route,
    /// which may write the human Workspace).
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
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Gui {
    port: u16,
    token: String,
}

impl Gui {
    fn post(&self, path: &str, body: &Value) -> Value {
        use std::net::TcpStream;
        let payload = serde_json::to_string(body).expect("JSON body");
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let port = self.port;
        let request = format!(
            "POST {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            self.token,
            payload.len()
        );
        stream.write_all(request.as_bytes()).expect("send request");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        let start = response.find("\r\n\r\n").expect("header end") + 4;
        serde_json::from_str(&response[start..]).expect("JSON response")
    }
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

/// A Topic and `n` Cards on its main line; the tree id.
fn topic(mcp: &mut Mcp, title: &str, n: usize) -> String {
    let root = address(mcp.ok("create_topic", json!({ "title": title, "body": "问题。" })));
    let tree = root.trim_end_matches("/0").to_string();
    let mut at = root;
    for i in 1..=n {
        at = address(mcp.ok(
            "create_successor",
            json!({ "at": at, "title": format!("{title} 第 {i} 张卡"), "body": format!("陈述 {i}。\n") }),
        ));
    }
    tree
}

/// Publish the one register Hub Card, the first Card of the Topic `索引`.
fn register(mcp: &mut Mcp, body: &str) -> String {
    let index = address(mcp.ok(
        "create_topic",
        json!({ "title": "索引", "body": "本主题收放代理工作区的入口。" }),
    ));
    address(mcp.ok(
        "create_successor",
        json!({
            "at": index,
            "type": "hub",
            "level": "register",
            "title": "索引：代理工作区的入口",
            "body": body,
        }),
    ))
}

fn entry_body(target: &str) -> String {
    format!("陈述入口。\n\n## 同述入口\n\n- [[{target}]] 给出完整定义。\n")
}

fn faults(view: &Value) -> &[Value] {
    view["faults"].as_array().expect("faults")
}

#[test]
fn a_confirmed_entry_names_its_terminal_and_is_navigation() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "斯卢茨基", 3);
    let entry = format!("{tree}/1");
    let substantive = format!("{tree}/2");
    let other = format!("{tree}/3");
    mcp.ok(
        "edit_card",
        json!({ "at": substantive, "title": "斯卢茨基分解", "body": "斯卢茨基分解把收入效应与替代效应分开。\n" }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": entry, "title": "斯卢茨基分解的陈述入口", "body": entry_body(&substantive) }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": other, "title": "斯卢茨基恒等式的另一种写法", "body": "另一种写法。\n" }),
    );
    register(&mut mcp, &format!("## 词条\n- 斯卢茨基分解：[[{entry}]]\n"));

    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(view["complete"], json!(true), "faults: {}", view["faults"]);
    assert_eq!(
        view["restatements"],
        json!([{
            "entry": entry,
            "substantive": substantive,
            "terminal": substantive,
            "title": "斯卢茨基分解",
        }]),
        "the accepted entry names its substantive terminal Card"
    );

    // The derived navigation role and the exact statement fingerprint are on every Card read; an
    // entry states no claim of its own, so it is navigation.
    let read = mcp.ok("get_card", json!({ "at": entry }));
    assert_eq!(read["restatement"], json!(true));
    let fingerprint = read["statement_fingerprint"]
        .as_str()
        .expect("a statement fingerprint")
        .to_string();
    assert_eq!(fingerprint.len(), 64, "a SHA-256 fingerprint");
    let claim = mcp.ok("get_card", json!({ "at": substantive }));
    assert_eq!(claim["restatement"], json!(false));
    assert_ne!(claim["statement_fingerprint"], json!(fingerprint));
    let restated = mcp.ok("get_card", json!({ "at": other }));
    assert_eq!(restated["restatement"], json!(false));

    // The title catalogue carries the same role, so a title-only reader classifies an entry as
    // navigation instead of a claim (D086).
    let titles = mcp.ok("export_titles", json!({}));
    let role = |at: &str| {
        titles["titles"]
            .as_array()
            .expect("titles")
            .iter()
            .find(|row| row["address"] == json!(at))
            .map(|row| row["restatement"].clone())
            .expect("the Card's title row")
    };
    assert_eq!(role(&entry), json!(true));
    assert_eq!(role(&substantive), json!(false));
    assert_eq!(role(&other), json!(false));
}

#[test]
fn a_chain_is_reported_and_never_confirmed() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "连锁入口", 3);
    let first = format!("{tree}/1");
    let second = format!("{tree}/2");
    let terminal = format!("{tree}/3");
    mcp.ok(
        "edit_card",
        json!({ "at": terminal, "title": "真正的陈述", "body": "真正的陈述在这里。\n" }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": second, "title": "第二层入口", "body": entry_body(&terminal) }),
    );
    // The first entry points at another entry: a chain, reported, and it names the terminal to write.
    mcp.ok(
        "edit_card",
        json!({ "at": first, "title": "第一层入口", "body": entry_body(&second) }),
    );
    register(&mut mcp, &format!("## 词条\n- 链：[[{first}]]\n"));

    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(view["complete"], json!(false), "a chain is not complete");
    assert_eq!(
        view["restatements"],
        json!([{
            "entry": second,
            "substantive": terminal,
            "terminal": terminal,
            "title": "真正的陈述",
        }]),
        "only the entry that names the substantive Card is confirmed"
    );
    let chain = faults(&view)
        .iter()
        .find(|fault| fault["fault"] == json!("restatement"))
        .expect("the chain is reported");
    assert_eq!(chain["kind"], json!("chain"));
    assert_eq!(chain["card"], json!(first));
    assert_eq!(chain["terminal"], json!(terminal), "the terminal to write directly: {chain}");
    assert_eq!(
        chain["cycle"],
        Value::Null,
        "a chain is not a cycle: {chain}"
    );
}

#[test]
fn a_cycle_is_reported_and_confirms_nothing() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "循环入口", 2);
    let first = format!("{tree}/1");
    let second = format!("{tree}/2");
    mcp.ok(
        "edit_card",
        json!({ "at": first, "title": "入口一", "body": entry_body(&second) }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": second, "title": "入口二", "body": entry_body(&first) }),
    );
    register(&mut mcp, &format!("## 词条\n- 环：[[{first}]]\n"));

    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(view["complete"], json!(false));
    assert_eq!(view["restatements"], json!([]), "a cycle is no entry");
    let cycle: Vec<&Value> = faults(&view)
        .iter()
        .filter(|fault| fault["kind"] == json!("cycle"))
        .collect();
    assert_eq!(cycle.len(), 2, "each entry of the cycle is named once: {}", view["faults"]);
    let cards: Vec<&str> = cycle
        .iter()
        .filter_map(|fault| fault["card"].as_str())
        .collect();
    assert!(cards.contains(&first.as_str()) && cards.contains(&second.as_str()), "{cards:?}");
    for fault in cycle {
        assert!(
            fault["cycle"].as_array().is_some_and(|nodes| nodes.len() == 2),
            "the cycle names its members: {fault}"
        );
    }
}

#[test]
fn an_entry_names_a_substantive_card_of_its_own_workspace() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "入口校验", 1);
    let entry = format!("{tree}/1");
    let root = format!("{tree}/0");
    let reg = register(&mut mcp, &format!("## 词条\n- 词一：[[{entry}]]\n"));
    // The person's Card comes through the owner's GUI; the MCP server never writes it.
    let gui = home.gui();
    let human_root = address(gui.post(
        "/api/create",
        &json!({ "kind": "topic", "workspace": "human", "text": "# 人的主题\n\n人的问题。\n" }),
    ));
    let human_card = address(gui.post(
        "/api/create",
        &json!({ "kind": "direct", "at": human_root, "text": "# 人的陈述\n\n重复练习改变技能。\n" }),
    ));

    // A tree root, a Hub Card and a cross-Workspace Card are navigation or read-only: no entry.
    for (target, expected, why) in [
        (root.as_str(), "not-substantive", "a tree root"),
        (reg.as_str(), "not-substantive", "a Hub Card"),
        (human_card.as_str(), "workspace", "a human Card from an agent entry"),
    ] {
        mcp.ok(
            "edit_card",
            json!({ "at": entry, "title": "入口", "body": entry_body(target) }),
        );
        let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
        assert_eq!(view["restatements"], json!([]), "{why} is no confirmed entry");
        assert_eq!(view["complete"], json!(false), "{why} is a fault");
        assert!(
            faults(&view).iter().any(|fault| fault["kind"] == json!(expected)
                && fault["card"] == json!(entry)),
            "{why}: {}",
            view["faults"]
        );
    }
}

#[test]
fn an_exchange_of_two_equal_statements_keeps_one_fingerprint_and_a_changed_sign_keeps_none() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "指纹", 4);
    let root = format!("{tree}/0");
    let cards = (1..=4)
        .map(|n| format!("{tree}/{n}"))
        .collect::<Vec<_>>();
    let source = format!("来源：[[{root}]] §2.1");
    let english_source = format!("Source: [[{root}]] §9");
    // The same statement, written with CRLF, outer blank lines and a separately labelled source
    // line: one claim, so one fingerprint.
    mcp.ok(
        "edit_card",
        json!({
            "at": cards[0],
            "title": "斯卢茨基分解",
            "body": format!("当价格变化时，替代效应与收入效应相加：$dx=\\partial x_s+\\partial x_i$。\n\n假设：价格变化很小。\n\n{source}"),
        }),
    );
    mcp.ok(
        "edit_card",
        json!({
            "at": cards[1],
            "title": "斯卢茨基分解（另一种排印）",
            "body": format!("\n\n当价格变化时，替代效应与收入效应相加：$dx=\\partial x_s+\\partial x_i$。\r\n\r\n假设：价格变化很小。\r\n\r\n{english_source}\r\n  "),
        }),
    );
    // A changed sign, a changed value and a changed variable each give a different statement.
    for (index, body) in [
        format!("当价格变化时，替代效应与收入效应相减：$dx=\\partial x_s-\\partial x_i$。\n\n假设：价格变化很小。\n\n{source}"),
        format!("当价格变化时，替代效应与收入效应相加：$dx=2\\partial x_s+\\partial x_i$。\n\n假设：价格变化很小。\n\n{source}"),
        format!("当价格变化时，替代效应与收入效应相加：$dy=\\partial x_s+\\partial x_i$。\n\n假设：价格变化很小。\n\n{source}"),
    ]
    .iter()
    .enumerate()
    {
        mcp.ok(
            "edit_card",
            json!({ "at": cards[2 + index % 2], "title": "改过的陈述", "body": body }),
        );
        let changed = mcp.ok("get_card", json!({ "at": cards[2 + index % 2] }));
        let unchanged = mcp.ok("get_card", json!({ "at": cards[0] }));
        assert_ne!(
            changed["statement_fingerprint"], unchanged["statement_fingerprint"],
            "a changed statement must change the fingerprint: {body}"
        );
    }
    let first = mcp.ok("get_card", json!({ "at": cards[0] }));
    let second = mcp.ok("get_card", json!({ "at": cards[1] }));
    assert_eq!(
        first["statement_fingerprint"], second["statement_fingerprint"],
        "the same claim written with CRLF, outer whitespace and another source line is one statement"
    );
    // The fingerprint is derived, not stored: a changed assumption changes it.
    mcp.ok(
        "edit_card",
        json!({
            "at": cards[1],
            "body": format!("当价格变化时，替代效应与收入效应相加：$dx=\\partial x_s+\\partial x_i$。\n\n假设：价格变化很大。\n\n{english_source}"),
        }),
    );
    let edited = mcp.ok("get_card", json!({ "at": cards[1] }));
    assert_ne!(edited["statement_fingerprint"], first["statement_fingerprint"]);
}
