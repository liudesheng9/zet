//! S29 (#171): one fault through every authoring route, in each Workspace the client may change.
//!
//! R0523 asks for a parameterized route matrix across the GUI, MCP, shell, Session, insert and
//! batch paths, testing both Workspaces with each client's real Access. Every cell drives the
//! real route on a scratch archive and then reads the stored Cards, the archive revision, the
//! durable receipts and the next Topic allocation, so a refusal is proven by stored state and
//! not by a message.
//!
//! The archive is a scratch one (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`); the live archive is never
//! touched.

#![allow(dead_code)]

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// The authored fragment the routes carry: the semicolon is a blocking R3 fault in the agent
/// Workspace, and a stored advisory in a Workspace that advises it. The rest of the body passes
/// the size and kinded-Link rules, so R3 is the only new fault an enforcing route meets.
const FAULTY_TITLE: &str = "短的标题";
const FAULTY_BODY: &str = "甲；乙。";

/// A body the current rules accept in an enforcing archive: enough prose for the size rule and
/// one kinded Link to another Card.
fn passing_body(link: &str) -> String {
    format!(
        "这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。对应 [[{link}]]：这条理由说明选择。"
    )
}

/// The faulty authored body: the passing prose plus one semicolon sentence.
fn faulty_body(link: &str) -> String {
    format!("{}甲；乙。", passing_body(link))
}

/// The faulty authored text every route carries.
fn faulty_text(link: &str) -> String {
    format!("# {FAULTY_TITLE}\n\n{}\n", faulty_body(link))
}

struct Home {
    _tmp: TempDir,
    config_dir: PathBuf,
    archive_root: PathBuf,
}

impl Home {
    /// A configured archive with its service up, in the rollout's advisory stage.
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = Self {
            config_dir: tmp.path().join("config"),
            archive_root: tmp.path().join("archive"),
            _tmp: tmp,
        };
        home.run(&[
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().expect("archive root"),
        ]);
        home.run(&["up"]);
        home
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    fn run(&self, args: &[&str]) -> String {
        let out = self.cmd().args(args).output().expect("zt command");
        assert!(
            out.status.success(),
            "zt {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn conn(&self) -> Connection {
        Connection::open(self.archive_root.join("zt.sqlite3")).expect("open archive")
    }

    fn cards(&self) -> i64 {
        self.conn()
            .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
            .expect("card count")
    }

    /// The address of the Card with the highest row id: the one an append-only route created.
    fn newest_address(&self) -> Option<String> {
        self.conn()
            .query_row(
                "SELECT COALESCE(location, citation_key) FROM cards ORDER BY row_id DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
    }

    fn receipts(&self) -> i64 {
        self.conn()
            .query_row("SELECT COUNT(*) FROM applied_ops", [], |row| row.get(0))
            .unwrap_or(0)
    }

    fn revision(&self) -> i64 {
        self.conn()
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM metadata WHERE key = 'archive_revision'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
    }

    fn next_topic(&self) -> String {
        self.conn()
            .query_row(
                "SELECT value FROM metadata WHERE key = 'next_topic_id'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "0".to_string())
    }

    fn text(&self, at: &str) -> String {
        self.conn()
            .query_row(
                "SELECT text FROM cards WHERE location = ?1 OR citation_key = ?1",
                [at],
                |row| row.get(0),
            )
            .expect("stored text")
    }

    /// A shell command whose `$EDITOR` saves `text`.
    fn editor_cmd(&self, text: &str) -> Command {
        let script = self.config_dir.join("fake-editor.ps1");
        fs::create_dir_all(&self.config_dir).expect("config dir");
        fs::write(
            &script,
            "[IO.File]::WriteAllText($args[0], $env:ZT_EDITOR_TEXT)\n",
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

impl Drop for Home {
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

    fn refused(&mut self, name: &str, arguments: Value) -> Value {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(is_error, "{name}({arguments}) was not refused: {value}");
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
    fn post(&self, path: &str, body: &Value) -> (u16, Value) {
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

/// The title of an authored text: its first heading line.
fn title_of(text: &str) -> String {
    text.lines()
        .next()
        .unwrap_or_default()
        .trim_start_matches('#')
        .trim()
        .to_string()
}

/// The body of an authored text: everything after its heading.
fn body_of(text: &str) -> String {
    text.split_once('\n')
        .map(|(_, body)| body.trim().to_string())
        .unwrap_or_default()
}

/// Every diagnostic rule a JSON reply carries, whichever route's shape it has.
fn rules_of(value: &Value) -> Vec<String> {
    let mut rules = Vec::new();
    collect_rules(value, &mut rules);
    rules.sort();
    rules.dedup();
    rules
}

fn collect_rules(value: &Value, rules: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(rule) = object.get("rule").and_then(Value::as_str) {
                rules.push(rule.to_string());
            }
            for nested in object.values() {
                collect_rules(nested, rules);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_rules(item, rules);
            }
        }
        _ => {}
    }
}

/// The blocking-fault count a structured refusal carries, wherever the route nests it.
fn blocking_count(value: &Value) -> Option<u64> {
    match value {
        Value::Object(object) => {
            if let Some(blocking) = object.get("blocking").and_then(Value::as_u64) {
                return Some(blocking);
            }
            object.values().find_map(blocking_count)
        }
        Value::Array(items) => items.iter().find_map(blocking_count),
        _ => None,
    }
}

/// Every rule a route printed as a diagnostic line (`check: <address> <rule> ...` or
/// `<address> <rule> ...`), so the shell and Session streams compare with the JSON shapes.
fn rules_in_text(text: &str) -> Vec<String> {
    let mut rules: Vec<String> = text
        .split_whitespace()
        .filter(|token| {
            let mut chars = token.chars();
            matches!(chars.next(), Some('R' | 'I' | 'E' | 'F' | 'L' | 'T'))
                && token.len() > 1
                && chars.all(|c| c.is_ascii_digit())
        })
        .map(str::to_string)
        .collect();
    rules.sort();
    rules.dedup();
    rules
}

/// The authoring routes the matrix drives. Each is a real public path: the shell's `n` and `ins`,
/// the line Session's `n`, the GUI's create and edit routes, and the MCP server's single write and
/// its batch (real and preview).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    ShellDirect,
    ShellInsert,
    SessionLine,
    GuiCreate,
    GuiEdit,
    McpCreate,
    McpBatchReal,
    McpBatchPreview,
}

impl Route {
    const ALL: [Route; 8] = [
        Route::ShellDirect,
        Route::ShellInsert,
        Route::SessionLine,
        Route::GuiCreate,
        Route::GuiEdit,
        Route::McpCreate,
        Route::McpBatchReal,
        Route::McpBatchPreview,
    ];

    fn id(self) -> &'static str {
        match self {
            Route::ShellDirect => "shell n",
            Route::ShellInsert => "shell ins",
            Route::SessionLine => "session n",
            Route::GuiCreate => "gui create",
            Route::GuiEdit => "gui edit",
            Route::McpCreate => "mcp create_successor",
            Route::McpBatchReal => "mcp apply_batch",
            Route::McpBatchPreview => "mcp apply_batch preview",
        }
    }

    /// A route whose write happens on the Card it names (so the matrix has an edit cell) versus a
    /// route that appends a Card.
    fn edits(self) -> bool {
        matches!(self, Route::ShellInsert | Route::GuiEdit)
    }

    /// A route that changes the Card it names in place instead of storing a new Card.
    fn in_place(self) -> bool {
        self == Route::GuiEdit
    }

    /// A route that stores nothing itself: the preview reports the plan.
    fn preview(self) -> bool {
        self == Route::McpBatchPreview
    }
}

/// What one route attempt published: the typed error code (if it refused), every diagnostic rule,
/// the address it reported, and the text the route said, so a failure quotes it.
struct Attempt {
    error: Option<String>,
    rules: Vec<String>,
    address: Option<String>,
    value: Value,
    said: String,
}

fn typed_error(value: &Value) -> Option<String> {
    // The GUI wraps a typed refusal under `check`; its own `error` is the human fault report.
    value
        .get("check")
        .and_then(|check| check.get("error"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

/// Write through one route. `at` is the parent for an append route, the edited Card for an edit.
fn attempt(home: &Home, mcp: &mut Mcp, gui: &Gui, route: Route, at: &str, text: &str) -> Attempt {
    match route {
        Route::ShellDirect | Route::ShellInsert => {
            let command = if route == Route::ShellDirect { "n" } else { "ins" };
            let mut cmd = home.editor_cmd(text);
            cmd.args([command, "--at", at]);
            if route == Route::ShellInsert {
                // An insert shows its renumbering plan first and asks for its own confirmation.
                cmd.write_stdin("insert\n");
            }
            let out = cmd.output().expect("zt shell write");
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let said = format!("{stdout}{stderr}");
            let mut rules = rules_in_text(&said);
            rules.dedup();
            Attempt {
                error: if out.status.success() {
                    None
                } else {
                    Some(if stderr.contains("card_check: ") {
                        "card_check".to_string()
                    } else {
                        stderr.lines().next().unwrap_or_default().to_string()
                    })
                },
                rules,
                address: (out.status.success() && route == Route::ShellDirect)
                    .then(|| stdout.lines().next().unwrap_or_default().trim().to_string()),
                value: Value::Null,
                said,
            }
        }
        Route::SessionLine => {
            let input = format!("go {at}\nn\n{text}\x13q\n");
            let out = home
                .cmd()
                .write_stdin(input)
                .output()
                .expect("zt line Session");
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let said = format!("{stdout}{stderr}");
            Attempt {
                error: said.contains("card_check: ").then(|| "card_check".to_string()),
                rules: rules_in_text(&said),
                address: None,
                value: Value::Null,
                said,
            }
        }
        Route::GuiCreate => {
            let (status, value) = gui.post(
                "/api/create",
                &json!({ "kind": "direct", "at": at, "text": text }),
            );
            let said = value.to_string();
            Attempt {
                error: if status == 200 { None } else { typed_error(&value) },
                rules: rules_of(&value),
                address: (status == 200)
                    .then(|| value["address"].as_str().unwrap_or_default().to_string()),
                value,
                said,
            }
        }
        Route::GuiEdit => {
            let (status, value) = gui.post("/api/edit", &json!({ "at": at, "text": text }));
            let said = value.to_string();
            Attempt {
                error: if status == 200 { None } else { typed_error(&value) },
                rules: rules_of(&value),
                address: (status == 200).then(|| at.to_string()),
                value,
                said,
            }
        }
        Route::McpCreate => {
            let (value, is_error) = mcp.call(
                "create_successor",
                json!({ "at": at, "title": title_of(text), "body": body_of(text) }),
            );
            let said = value.to_string();
            Attempt {
                error: if is_error { typed_error(&value) } else { None },
                rules: rules_of(&value),
                address: (!is_error)
                    .then(|| value["address"].as_str().unwrap_or_default().to_string()),
                value,
                said,
            }
        }
        Route::McpBatchReal | Route::McpBatchPreview => {
            let items = json!([{
                "op_key": format!("s29/routes/1/{}/item-0000", route.id().replace(' ', "-")),
                "client_ref": "only",
                "tool": "create_successor",
                "arguments": { "at": at, "title": title_of(text), "body": body_of(text) }
            }]);
            let (value, _) = mcp.call(
                "apply_batch",
                json!({ "preview": route.preview(), "items": items }),
            );
            let said = value.to_string();
            Attempt {
                error: if typed_error(&value).is_some() {
                    typed_error(&value)
                } else if route.preview() {
                    (!value["refusals"]
                        .as_array()
                        .map(Vec::is_empty)
                        .unwrap_or(true))
                    .then(|| "card_check".to_string())
                } else {
                    None
                },
                rules: rules_of(&value),
                address: value["results"][0]["result"]["address"]
                    .as_str()
                    .map(str::to_string),
                value,
                said,
            }
        }
    }
}


/// Create one agent Topic through MCP and answer its address.
fn agent_topic(mcp: &mut Mcp, title: &str) -> String {
    mcp.ok("create_topic", json!({ "title": title, "body": "主题说明。" }))["address"]
        .as_str()
        .expect("topic address")
        .to_string()
}

/// The Card a passing body links, so every setup write has a kinded Link of its own.
fn link_target(mcp: &mut Mcp) -> String {
    agent_topic(mcp, "链接目标")
}

/// One route's own parent Topic and, for an edit route, the Card it changes.
fn route_fixture(mcp: &mut Mcp, route: Route, link: &str) -> (String, String) {
    let parent = agent_topic(mcp, &format!("{} 母卡", route.id()));
    let target = if route.edits() {
        mcp.ok(
            "create_successor",
            json!({ "at": parent, "title": "编辑目标", "body": passing_body(link) }),
        )["address"]
            .as_str()
            .expect("edit target")
            .to_string()
    } else {
        parent.clone()
    };
    (parent, target)
}

/// Advisory archive: every route stores the same advised fault and publishes the same R3 advice.
#[test]
fn every_authoring_route_stores_the_advised_fault_with_its_warning() {
    let home = Home::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let link = link_target(&mut mcp);
    let text = faulty_text(&link);
    for route in Route::ALL {
        let (_parent, target) = route_fixture(&mut mcp, route, &link);
        let cards = home.cards();
        let attempt = attempt(&home, &mut mcp, &gui, route, &target, &text);
        assert_eq!(
            attempt.error,
            None::<String>,
            "{}: {}",
            route.id(),
            attempt.said
        );
        assert!(
            attempt.rules.iter().any(|rule| rule == "R3"),
            "{}: the advisory fault is published as R3: {}",
            route.id(),
            attempt.said
        );
        if route.preview() {
            assert_eq!(attempt.value["applied"], json!(false), "{}", route.id());
            assert_eq!(
                home.cards(),
                cards,
                "{}: a preview stores nothing",
                route.id()
            );
            assert_eq!(
                attempt.value["cards"][0]["body"],
                json!(body_of(&text)),
                "{}: the preview projects the checked Card",
                route.id()
            );
            continue;
        }
        if route.in_place() {
            assert_eq!(
                home.cards(),
                cards,
                "{}: the edit stores no new Card",
                route.id()
            );
            assert!(
                home.text(&target).contains(FAULTY_BODY),
                "{}: the edited body is stored",
                route.id()
            );
            continue;
        }
        assert_eq!(
            home.cards(),
            cards + 1,
            "{}: the advisory archive stores the Card: {}",
            route.id(),
            attempt.said
        );
        let stored = match &attempt.address {
            Some(address) => home.text(address),
            None => home.text(&home.newest_address().expect("the stored Card")),
        };
        assert!(
            stored.contains(FAULTY_BODY),
            "{}: the authored body is stored: {stored}",
            route.id()
        );
    }
}

/// Enforcing archive: every route refuses the same fault with its typed contract, and the archive
/// keeps no Card, receipt, revision or Topic allocation.
#[test]
fn every_authoring_route_refuses_the_blocked_fault_without_residue() {
    let home = Home::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let link = link_target(&mut mcp);
    let text = faulty_text(&link);
    // Every fixture is prepared in the advisory stage, then one deliberate promotion enforces the
    // agent policy without a restart.
    let mut fixtures = Vec::new();
    for route in Route::ALL {
        fixtures.push((route, route_fixture(&mut mcp, route, &link)));
    }
    home.run(&["config", "set", "gate.stage", "enforce"]);
    for (route, (_parent, target)) in fixtures {
        let before = home.text(&target);
        let cards = home.cards();
        let revision = home.revision();
        let receipts = home.receipts();
        let next_topic = home.next_topic();
        let attempt = attempt(&home, &mut mcp, &gui, route, &target, &text);
        assert_eq!(
            attempt.error.as_deref(),
            Some("card_check"),
            "{}: an enforcing archive refuses: {}",
            route.id(),
            attempt.said
        );
        let mut rules = attempt.rules;
        rules.sort();
        rules.dedup();
        assert!(
            rules.contains(&"R3".to_string()),
            "{}: the refusal names the new blocking rule: {}",
            route.id(),
            attempt.said
        );
        match blocking_count(&attempt.value) {
            Some(blocking) => assert_eq!(
                blocking,
                1,
                "{}: exactly the semicolon blocks: {}",
                route.id(),
                attempt.said
            ),
            None => assert!(
                attempt.said.contains("1 blocking fault"),
                "{}: exactly the semicolon blocks: {}",
                route.id(),
                attempt.said
            ),
        }
        assert_eq!(home.cards(), cards, "{}: no Card is stored", route.id());
        assert_eq!(
            home.revision(),
            revision,
            "{}: no revision advances",
            route.id()
        );
        assert_eq!(
            home.receipts(),
            receipts,
            "{}: no receipt is kept",
            route.id()
        );
        assert_eq!(
            home.next_topic(),
            next_topic,
            "{}: no Topic id is allocated",
            route.id()
        );
        assert_eq!(
            home.text(&target),
            before,
            "{}: a refused write changes no stored text",
            route.id()
        );
    }
}

/// R0523/R0519: each Workspace decides with its own policy, and each client keeps its real Access.
/// The human Workspace advises the fault while the agent Workspace blocks it; the MCP server may
/// not write a human Card at all, and that Access refusal comes before any Card check.
#[test]
fn each_workspace_keeps_its_own_policy_and_each_client_its_own_access() {
    let home = Home::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let link = link_target(&mut mcp);
    let text = faulty_text(&link);
    let setup = format!("# 主题\n\n{}\n", passing_body(&link));
    // The GUI is the human's own tool: it starts the human Topic.
    let (status, topic) = gui.post(
        "/api/create",
        &json!({ "kind": "topic", "workspace": "human", "text": setup }),
    );
    assert_eq!(status, 200, "{topic}");
    let human_topic = topic["address"].as_str().expect("human topic").to_string();
    let agent = agent_topic(&mut mcp, "代理主题");
    // One deliberate promotion, then one archive-specific human override.
    home.run(&["config", "set", "gate.stage", "enforce"]);
    home.run(&["config", "set", "gate.human.R3", "advise"]);

    // The human's shell writes the human Workspace: the human policy advises R3, so it stores.
    let human_before = home.cards();
    let stored = home
        .editor_cmd(&text)
        .args(["n", "--at", &human_topic])
        .output()
        .expect("human shell write");
    let stderr = String::from_utf8_lossy(&stored.stderr).to_string();
    assert!(
        stored.status.success(),
        "the human Workspace advises R3: {stderr}"
    );
    assert_eq!(home.cards(), human_before + 1);
    let human_card = home.newest_address().expect("the stored human Card");
    assert!(home.text(&human_card).contains(FAULTY_BODY));
    assert!(
        rules_in_text(&stderr).iter().any(|rule| rule == "R3"),
        "the human write reports its advice: {stderr}"
    );

    // The agent's shell write to the agent Workspace still blocks: the human override changed one
    // Workspace and one rule only.
    let agent_before = home.cards();
    let refused = home
        .editor_cmd(&text)
        .args(["n", "--at", &agent])
        .output()
        .expect("agent shell write");
    assert!(!refused.status.success(), "the agent policy blocks R3");
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("card_check: "),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    assert_eq!(
        home.cards(),
        agent_before,
        "the refused agent write stores nothing"
    );

    // The GUI is the human's tool too: it may write the human Workspace, under the human policy.
    let gui_before = home.cards();
    let (status, made) = gui.post(
        "/api/create",
        &json!({ "kind": "direct", "at": human_card, "text": text }),
    );
    assert_eq!(status, 200, "the GUI writes a human Card: {made}");
    assert!(
        rules_of(&made).iter().any(|rule| rule == "R3"),
        "the GUI write returns the human advice: {made}"
    );
    assert_eq!(home.cards(), gui_before + 1);

    // The MCP server has agent Access: a human write is refused before any Card check, and the
    // human Card keeps its stored text.
    let card_before = home.cards();
    let refused = mcp.refused(
        "create_successor",
        json!({ "at": &human_card, "title": "代理写入", "body": passing_body(&link) }),
    );
    assert_ne!(
        refused["error"],
        json!("card_check"),
        "an Access refusal is not a Card check: {refused}"
    );    assert!(
        refused["error"]
            .as_str()
            .is_some_and(|error| error.contains("human workspace is read-only")),
        "the refusal names the Access rule: {refused}"
    );
    assert_eq!(
        home.cards(),
        card_before,
        "the refused MCP write stores nothing"
    );
    assert!(
        !home.text(&human_card).contains("代理写入"),
        "the human Card was not rewritten"
    );
}
