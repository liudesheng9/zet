//! The installed-reader release contract (S28, #121): the three independent GUI switches, the
//! reader paths that stay readable while a switch is off, and the unchanged transports.
//!
//! The release can turn GUI search, Ask and Reply off one at a time (`zt config set gui_search
//! off`) without stopping the service. This suite drives that through the real config file, the
//! running service and its HTTP API: one switch changes only its own route, `/api/overview`
//! reports the states, Cards and saved Spreads stay readable, and the MCP server keeps every
//! route (the switches are the GUI's, not the archive's). The archive is a scratch one
//! (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`); the live archive is never touched.

#![allow(dead_code)]

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
            .env("ZT_GUI_PORT", "0")
            .env("ZT_DIGEST_HELPER", "Z:\\nowhere\\zt-digest.exe");
        cmd
    }

    fn config(&self) -> String {
        fs::read_to_string(self.config_dir.join("config.toml")).unwrap_or_default()
    }

    /// One `zt config set` while the service is up: a switch needs no restart.
    fn set_switch(&self, key: &str, value: &str) -> assert_cmd::assert::Assert {
        self.cmd()
            .args(["config", "set", key, value])
            .assert()
    }

    fn mcp(&self) -> Mcp {
        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("zt"))
            .arg("mcp")
            .env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0")
            .env("ZT_DIGEST_HELPER", "Z:\\nowhere\\zt-digest.exe")
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
        serde_json::from_str(&line).expect("MCP reply is JSON")
    }

    fn call(&mut self, name: &str, arguments: Value) -> (Value, bool) {
        let reply = self.request("tools/call", json!({ "name": name, "arguments": arguments }));
        (
            reply["result"]["structuredContent"].clone(),
            reply["result"]["isError"].as_bool().expect("isError flag"),
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
    }
}

struct Gui {
    port: u16,
    token: String,
}

impl Gui {
    fn get(&self, path: &str) -> Value {
        let (status, body) = self.get_with(path, &self.token);
        assert_eq!(status, 200, "{path} answered {status}: {body}");
        body
    }

    fn get_with(&self, path: &str, token: &str) -> (u16, Value) {
        use std::net::TcpStream;
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let request = format!(
            "GET {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {token}\r\nContent-Length: 0\r\n\r\n",
            self.port
        );
        stream.write_all(request.as_bytes()).expect("send request");
        read_reply(stream)
    }

    fn post_with(&self, path: &str, token: &str, body: &Value) -> (u16, Value) {
        use std::net::TcpStream;
        let payload = serde_json::to_string(body).expect("JSON body");
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let request = format!(
            "POST {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            self.port,
            payload.len()
        );
        stream.write_all(request.as_bytes()).expect("send request");
        read_reply(stream)
    }

    fn post(&self, path: &str, body: &Value) -> Value {
        let (status, value) = self.post_with(path, &self.token, body);
        assert_eq!(status, 200, "{path} answered {status}: {value}");
        value
    }

    fn card(&self, at: &str) -> Value {
        self.get(&format!("/api/card?at={}", enc(at)))
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

fn address(value: &Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

/// One Topic with two Cards: the reader's fixture for the switch tests.
fn reader_fixture(mcp: &mut Mcp) -> (String, String) {
    let topic = address(&mcp.ok(
        "create_topic",
        json!({ "title": "开关测试", "body": "本主题检验三个开关。" }),
    ));
    let first = address(&mcp.ok(
        "create_successor",
        json!({ "at": topic, "title": "第一张卡", "body": "第一张卡的正文。" }),
    ));
    let second = address(&mcp.ok(
        "create_successor",
        json!({ "at": first, "title": "第二张卡", "body": format!("第二张卡的正文，依赖 [[{first}]]。") }),
    ));
    (first, second)
}

#[test]
fn a_switch_turns_one_feature_off_without_stopping_the_service() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (first, second) = reader_fixture(&mut mcp);
    drop(mcp);
    let gui = home.gui();
    assert_eq!(
        gui.get("/api/overview")["features"],
        json!({ "search": true, "ask": true, "reply": true })
    );
    assert_eq!(home.set_switch("gui_search", "off").get_output().status.success(), true);
    let overview = gui.get("/api/overview");
    assert_eq!(overview["features"]["search"], json!(false), "{overview}");
    assert_eq!(overview["features"]["ask"], json!(true));
    assert_eq!(overview["features"]["reply"], json!(true));
    let (status, body) = gui.get_with("/api/search?q=%E5%BC%80%E5%85%B3", &gui.token);
    assert_eq!(status, 409, "search answered {status}: {body}");
    assert_eq!(body["disabled"], json!("gui_search"));
    assert!(body["error"].as_str().is_some_and(|text| text.contains("gui_search on")));
    // Ask, the reader and the saved Spreads are untouched.
    gui.post("/api/ask", &json!({ "question": "开关测试" }));
    assert_eq!(gui.card(&second)["address"], json!(second));
    assert_eq!(gui.card(&first)["title"], json!("第一张卡"));
    assert!(gui.get("/api/spreads")["spreads"].is_array());
    assert!(gui.get("/api/graph")["nodes"].as_array().is_some_and(|nodes| nodes.len() == 3));
    // The MCP server keeps its own search: the switches are the GUI's.
    let mut mcp = home.mcp();
    let found = mcp.ok("search_cards", json!({ "query": "开关测试" }));
    assert!(found["results"].as_array().is_some_and(|rows| !rows.is_empty()), "{found}");
}

#[test]
fn each_switch_gates_only_its_own_route_and_the_reader_stays_usable() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (first, _second) = reader_fixture(&mut mcp);
    drop(mcp);
    let gui = home.gui();

    // Ask off: only Ask's route refuses; search answers and Reply still reaches its own checks.
    home.set_switch("gui_ask", "off").success();
    let (status, body) = gui.post_with("/api/ask", &gui.token, &json!({ "question": "开关测试" }));
    assert_eq!(status, 409, "ask answered {status}: {body}");
    assert_eq!(body["disabled"], json!("gui_ask"));
    assert!(!gui.get("/api/search?q=%E5%BC%80%E5%85%B3")["results"].as_array().is_some_and(Vec::is_empty));
    assert_eq!(gui.post_with("/api/reply", &gui.token, &json!({})).0, 400);

    // Reply off: only Reply's route refuses; Ask still reaches its own checks.
    home.set_switch("gui_ask", "on").success();
    home.set_switch("gui_reply", "off").success();
    let (status, body) = gui.post_with(
        "/api/reply",
        &gui.token,
        &json!({ "at": first, "base": "a1", "text": "印证 [[x]] 的话。" }),
    );
    assert_eq!(status, 409, "reply answered {status}: {body}");
    assert_eq!(body["disabled"], json!("gui_reply"));
    assert_eq!(gui.post_with("/api/ask", &gui.token, &json!({})).0, 400);
    assert!(body["error"].as_str().is_some_and(|text| text.contains("gui_reply on")));

    // Every switch off: the reader keeps the Cards, the tree, the map, the Table and Spreads.
    for key in ["gui_search", "gui_ask", "gui_reply"] {
        home.set_switch(key, "off").success();
    }
    let overview = gui.get("/api/overview");
    assert_eq!(
        overview["features"],
        json!({ "search": false, "ask": false, "reply": false })
    );
    assert_eq!(gui.card(&first)["address"], json!(first));
    let tree = first.split('/').next().expect("tree id").to_string();
    assert_eq!(gui.get(&format!("/api/tree?id={tree}"))["cards"].as_array().map(Vec::len), Some(3));
    assert!(gui.get("/api/graph")["nodes"].as_array().is_some_and(|nodes| nodes.len() >= 3));
    assert!(gui.get("/api/broken")["broken"].is_array());
    let saved = gui.post(
        "/api/spreads",
        &json!({
            "current": "s1",
            "spreads": [{
                "id": "s1",
                "name": "开关 Spread",
                "question": "开关关掉之后这张桌子还在吗？",
                "cards": [{ "at": first, "x": 10, "y": 20 }],
            }],
        }),
    );
    assert_eq!(saved["spreads"][0]["name"], json!("开关 Spread"));
    let kept = gui.get("/api/spreads");
    assert_eq!(kept["current"], json!("s1"));
    assert_eq!(kept["spreads"][0]["cards"][0]["at"], json!(first));
    // The page itself is still served, so the reader can open the Cards.
    assert!(http_page(gui.port).contains("zt-token"));
    // Each disabled route names its own switch.
    for (path, key, body) in [
        ("/api/search?q=x", "gui_search", Value::Null),
        ("/api/ask", "gui_ask", json!({ "question": "开关测试" })),
        ("/api/reply", "gui_reply", json!({ "at": first, "base": "a1", "text": "x" })),
    ] {
        let (status, answer) = if body.is_null() {
            gui.get_with(path, &gui.token)
        } else {
            gui.post_with(path, &gui.token, &body)
        };
        assert_eq!(status, 409, "{path} answered {status}: {answer}");
        assert_eq!(answer["disabled"], json!(key));
    }
    // The MCP server keeps every route: Search and Ask are the archive's reads, not the GUI's.
    let mut mcp = home.mcp();
    assert!(!mcp.ok("search_cards", json!({ "query": "开关测试" }))["results"]
        .as_array()
        .is_some_and(Vec::is_empty));
    let (answer, is_error) = mcp.call(
        "ask_box",
        json!({ "question": "开关测试", "workspace": "agent" }),
    );
    assert!(!is_error, "ask_box failed: {answer}");

    // Back on: every route answers again, without a restart.
    for key in ["gui_search", "gui_ask", "gui_reply"] {
        home.set_switch(key, "on").success();
    }
    assert_eq!(
        gui.get("/api/overview")["features"],
        json!({ "search": true, "ask": true, "reply": true })
    );
    assert!(!gui.get("/api/search?q=%E5%BC%80%E5%85%B3")["results"].as_array().is_some_and(Vec::is_empty));
    gui.post("/api/ask", &json!({ "question": "开关测试" }));
}

#[test]
fn a_switch_persists_across_a_restart_and_unknown_config_lines_survive() {
    let home = TestHome::new();
    home.set_switch("gui_search", "off").success();
    let before = home.config();
    assert!(before.contains("gui_search=off"), "{before}");
    // The switch command owns four lines; anything else in the file stays as it was.
    let path = home.config_dir.join("config.toml");
    fs::write(&path, format!("{before}other_setting=keep\n")).expect("write config");
    home.set_switch("gui_ask", "off").success();
    let after = home.config();
    assert!(after.contains("other_setting=keep"), "{after}");
    assert!(after.contains("gui_search=off") && after.contains("gui_ask=off"), "{after}");
    // A refused value writes nothing.
    home.set_switch("gui_reply", "maybe").failure();
    assert_eq!(home.config(), after);
    // `zt config show` reports every line.
    let out = home.cmd().args(["config", "show"]).output().expect("config show");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "config show failed: {text}");
    assert!(text.contains("gui_search=off") && text.contains("gui_ask=off") && text.contains("gui_reply=on"), "{text}");
    // The switch survives a restart of the installed service.
    home.cmd().arg("down").assert().success();
    home.cmd().arg("up").assert().success();
    let gui = home.gui();
    let overview = gui.get("/api/overview");
    assert_eq!(overview["features"]["search"], json!(false));
    assert_eq!(overview["features"]["ask"], json!(false));
    assert_eq!(overview["features"]["reply"], json!(true));
    assert_eq!(home.config(), after);
}

