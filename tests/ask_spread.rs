//! Ask the box: one bounded Spread proposal from a question, two transports, no write.
//!
//! `src/entry.rs::gather` reads one question over the supplied Cards: it matches accepted register
//! terms, walks at most two authored semantic steps from the entry seeds, and proposes up to nine
//! explained Cards that hold the best entry Card (§9.3-§9.5, D211-D231). The MCP server's
//! `ask_box` and the GUI's authenticated `POST /api/ask` return that one proposal for the same
//! archive and question, and neither writes a Card, a Link, or a Spread. The archive is a scratch
//! one (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`).

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
    env: Vec<(String, String)>,
}

impl TestHome {
    fn new() -> Self {
        Self::new_with(&[])
    }

    fn new_with(env: &[(&str, &str)]) -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config_dir = tmp.path().join("config");
        let archive_root = tmp.path().join("archive");
        let home = Self {
            _tmp: tmp,
            config_dir,
            archive_root,
            env: env
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
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
        for (name, value) in &self.env {
            cmd.env(name, value);
        }
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

    /// The running service's GUI: its port and the token the page carries.
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

    /// Every tool the server advertises, so a test can see the ask tool and its schema.
    fn tools(&mut self) -> Vec<Value> {
        let reply = self.request("tools/list", json!({}));
        reply["result"]["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default()
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
    /// One authenticated GET; the JSON body of the answer.
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

    /// One authenticated POST of a JSON body; the status and the JSON answer.
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

    /// One authenticated POST; the JSON answer, asserting a successful read.
    fn post(&self, path: &str, body: &Value) -> Value {
        let (status, value) = self.post_with(path, &self.token, body);
        assert_eq!(status, 200, "{path} answered {status}: {value}");
        value
    }

    fn ask(&self, body: &Value) -> Value {
        self.post("/api/ask", body)
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

/// One Literature tree with its first Card: a book and a Card filed in it.
fn book(mcp: &mut Mcp, key: &str, title: &str, card_title: &str, body: &str) -> String {
    let at = address(mcp.ok(
        "create_literature",
        json!({ "bibtex": format!("@book{{{key}, title={{{title}}}}}"), "body": "该书。" }),
    ));
    assert_eq!(at, key, "the Citation key is the tree");
    address(mcp.ok(
        "create_successor",
        json!({ "at": key, "title": card_title, "body": body }),
    ))
}

/// The concept register: a Hub Card of level `register`, the first Card of the Topic `索引`.
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

/// Two books that share one accepted term and link to each other, and the register that accepts it.
fn two_books(home: &TestHome) -> (Gui, Value) {
    let mut mcp = home.mcp();
    let focus = book(
        &mut mcp,
        "bookA",
        "Book A",
        "多重均衡的陈述",
        "多重均衡来自策略互补。\n\n来源：[[bookA]]",
    );
    let other = book(
        &mut mcp,
        "bookB",
        "Book B",
        "策略互补",
        "策略互补解释多重均衡。\n\n来源：[[bookB]]",
    );
    mcp.ok(
        "edit_card",
        json!({ "at": focus, "body": "多重均衡来自策略互补。对应 [[bookB/1]]。\n\n来源：[[bookA]]" }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": other, "body": "策略互补解释多重均衡。对照 [[bookA/1]]。\n\n来源：[[bookB]]" }),
    );
    register(
        &mut mcp,
        "## 词条\n- 多重均衡：[[bookA/1]]\n- 策略互补：[[bookB/1]]\n",
    );
    let gui = home.gui();
    (gui, json!({ "focus": focus, "other": other }))
}

/// The archive state a question must leave exactly as it found it.
fn state(gui: &Gui) -> Value {
    json!({
        "overview": gui.get("/api/overview"),
        "graph": gui.get("/api/graph"),
        "spreads": gui.get("/api/spreads"),
        "register": gui.get("/api/register?ws=agent"),
        "broken": gui.get("/api/broken"),
    })
}

#[test]
fn one_question_proposal_serves_both_transports_without_a_write() {
    // No model or helper may be started: a helper that cannot run makes an accidental call fail.
    let home = TestHome::new_with(&[("ZT_DIGEST_HELPER", "Z:\\nowhere\\zt-digest.exe")]);
    let (gui, cards) = two_books(&home);
    assert_eq!(cards["focus"], json!("bookA/1"));
    assert_eq!(cards["other"], json!("bookB/1"));
    let question = json!({ "question": "为什么多重均衡可能出现？" });
    let before = state(&gui);

    let mut mcp = home.mcp();
    let tools = mcp.tools();
    let ask = tools
        .iter()
        .find(|tool| tool["name"] == json!("ask_box"))
        .expect("the server advertises ask_box");
    assert_eq!(ask["annotations"]["readOnlyHint"], json!(true));
    assert_eq!(
        ask["inputSchema"]["required"],
        json!(["question"]),
        "the question is the only required argument"
    );
    assert_eq!(ask["inputSchema"]["properties"]["limit"]["minimum"], json!(5));
    assert_eq!(ask["inputSchema"]["properties"]["limit"]["maximum"], json!(9));

    let proposal = mcp.ok("ask_box", question.clone());
    let value = gui.ask(&question);
    assert_eq!(value, proposal, "both transports return one proposal");

    assert_eq!(proposal["question"], json!("为什么多重均衡可能出现？"));
    assert_eq!(proposal["workspace"], json!("all"));
    assert_eq!(proposal["limit"], json!(7));
    assert_eq!(proposal["matched_terms"], json!(["多重均衡"]));
    assert_eq!(proposal["ambiguous_terms"], json!([]));
    assert_eq!(proposal["fallback"], Value::Null);
    assert_eq!(proposal["held"], json!("bookA/1"));
    assert_eq!(
        proposal["spread"]["name"],
        json!("为什么多重均衡可能出现？")
    );
    assert_eq!(
        proposal["spread"]["question"],
        json!("为什么多重均衡可能出现？")
    );
    assert_eq!(proposal["spread"]["cards"][0]["at"], json!("bookA/1"));
    assert_eq!(proposal["spread"]["cards"][1]["at"], json!("bookB/1"));
    let card = &proposal["cards"][0];
    assert_eq!(card["at"], json!("bookA/1"));
    assert_eq!(card["held"], json!(true));
    assert_eq!(card["term"], json!("多重均衡"));
    assert_eq!(card["depth"], json!(0));
    assert!(card["authored_fingerprint"].is_string());
    assert_eq!(proposal["cards"][1]["at"], json!("bookB/1"));
    assert_eq!(proposal["cards"][1]["depth"], json!(1));
    assert_eq!(
        proposal["cards"][1]["path"][0]["kind"], json!("corresponds"),
        "the step keeps its authored kind"
    );
    assert_eq!(proposal["cards"][1]["path"][0]["direction"], json!("out"));
    // Both Cards link to each other, so two authored threads join them.
    assert_eq!(proposal["threads"].as_array().expect("threads").len(), 2);
    assert_eq!(proposal["sparse"], json!(true), "two Cards is sparse");
    assert_eq!(proposal["truncated"], json!(false));
    assert_eq!(proposal["counts"]["visited"], json!(2));
    assert_eq!(proposal["bounds"]["terms"], json!(6));
    assert!(proposal["archive_revision"].as_u64().is_some());
    assert!(proposal["archive_identity"]["id"].is_string());
    assert_eq!(proposal["registers"][1]["workspace"], json!("agent"));
    assert_eq!(proposal["registers"][1]["complete"], json!(true));

    // Reading a proposal changes nothing in the archive.
    assert_eq!(state(&gui), before, "the question changed the archive");
    // The same read answers the same way.
    assert_eq!(
        mcp.ok("ask_box", question.clone()),
        proposal,
        "a repeated question gives the same proposal"
    );
}

#[test]
fn a_question_refuses_a_blank_or_oversize_input_on_both_transports() {
    let home = TestHome::new();
    let (gui, _cards) = two_books(&home);
    let mut mcp = home.mcp();

    for (body, why) in [
        (json!({}), "a missing question"),
        (json!({ "question": "  " }), "a blank question"),
        (json!({ "question": "学".repeat(513) }), "a question over 512 characters"),
    ] {
        let (value, is_error) = mcp.call("ask_box", body.clone());
        assert!(is_error, "{why} must be refused by the MCP tool: {value}");
        let (status, refused) = gui.post_with("/api/ask", &gui.token, &body);
        assert_eq!(status, 400, "{why} must be refused by the route: {refused}");
        assert!(
            refused["error"].as_str().is_some_and(|error| !error.is_empty()),
            "{why} needs a reason: {refused}"
        );
    }

    // The token guard stays in front of the route.
    let (status, refused) =
        gui.post_with("/api/ask", "not-the-token", &json!({ "question": "问题" }));
    assert_eq!(status, 403, "{refused}");
    assert_eq!(refused["error"], json!("missing or invalid GUI token"));

    // The bound counts characters, not bytes: 512 Chinese characters are accepted.
    let longest = json!({ "question": "学".repeat(512) });
    let (status, value) = gui.post_with("/api/ask", &gui.token, &longest);
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["question"], longest["question"]);
    // A Workspace filter and the selection size are accepted and echoed.
    let value = gui.ask(&json!({ "question": "多重均衡", "workspace": "agent", "limit": 9 }));
    assert_eq!(value["workspace"], json!("agent"));
    assert_eq!(value["limit"], json!(9));
    let value = gui.ask(&json!({ "question": "多重均衡", "workspace": "human" }));
    assert_eq!(value["workspace"], json!("human"));
    assert_eq!(
        value["held"],
        Value::Null,
        "the human Workspace holds no matching entry: {value}"
    );
}
