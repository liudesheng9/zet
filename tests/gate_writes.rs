//! S04 (#146): the shared write gate over the real routes.
//!
//! The verification seam of the issue: real MCP, GUI API, shell, insert and batch
//! fixtures on isolated SQLite archives. Each test reads the stored Cards, the
//! archive revision and the next Topic allocation, so a refusal is proven to
//! leave no Card, id, receipt or revision behind. The live archive is never
//! touched.

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

struct Home {
    _tmp: TempDir,
    config_dir: PathBuf,
    archive_root: PathBuf,
}

impl Home {
    /// A configured archive with its service up. `stage` is the write-gate stage:
    /// `None` keeps the rollout default (advisory).
    fn new(stage: Option<&str>) -> Self {
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
            home.archive_root.to_str().unwrap(),
        ]);
        home.run(&["up"]);
        if let Some(stage) = stage {
            home.run(&["config", "set", "gate.stage", stage]);
        }
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

    fn receipts(&self) -> i64 {
        // The table is created inside the first real batch's transaction, so a refused first
        // batch leaves no table at all: neither is a receipt.
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

    /// A shell command whose `$EDITOR` saves `text`.
    fn cmd_with_editor(&self, text: &str) -> Command {
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

    fn get(&self, path: &str) -> (u16, Value) {
        use std::net::TcpStream;
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let request = format!(
            "GET {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Length: 0\r\n\r\n",
            self.port, self.token
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

fn fault_rules(value: &Value) -> Vec<String> {
    value["faults"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(|line| line.split_whitespace().nth(1).map(str::to_string))
        .collect()
}

/// S04-AC04/S04-AC05: an enforcing MCP write with a blocking fault answers the
/// typed `card_check` contract, and the archive keeps no Card, revision or
/// allocated Topic.
#[test]
fn an_enforced_mcp_write_refuses_with_its_contract_and_writes_nothing() {
    let home = Home::new(Some("enforce"));
    let mut mcp = home.mcp();
    let topic = mcp.ok("create_topic", json!({ "title": "主题", "body": "主题说明。" }))
        ["address"]
        .as_str()
        .expect("topic address")
        .to_string();
    let target = mcp.ok("create_topic", json!({ "title": "目标", "body": "目标说明。" }))
        ["address"]
        .as_str()
        .expect("target address")
        .to_string();
    let body = format!(
        "甲；乙。这一段说明背景并给出充分理由。这一段说明结果如何出现。对应 [[{target}]]：这条理由说明选择。"
    );
    let cards = home.cards();
    let revision = home.revision();
    let next = home.next_topic();
    let (value, is_error) = mcp.call(
        "create_successor",
        json!({ "at": topic, "title": "短的标题", "body": body }),
    );
    assert!(is_error, "the enforced write must be refused: {value}");
    assert_eq!(value["error"], json!("card_check"));
    assert_eq!(
        value["address"],
        json!(format!("{}/1", topic.split('/').next().expect("tree")))
    );
    assert!(value["blocking"].as_u64().expect("blocking") >= 1);
    assert!(fault_rules(&value).iter().any(|rule| rule == "R3"), "{value}");
    assert!(
        value["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["rule"] == json!("R3")
                && diagnostic["fault"] == json!("semicolon")
                && diagnostic["severity"] == json!("block")
                && diagnostic["scope"] == json!("local")),
        "{value}"
    );
    assert_eq!(value["zt_check_version"], json!(zt_check::VERSION));
    assert_eq!(value["policy_hash"].as_str().map(str::len), Some(64));
    assert!(value["message"]
        .as_str()
        .is_some_and(|message| message.starts_with("card_check: ")));
    assert_eq!(home.cards(), cards, "a refused write creates no Card");
    assert_eq!(home.revision(), revision, "a refused write moves no revision");
    assert_eq!(home.next_topic(), next, "a refused write allocates no Topic");

    // The same rule as advice lets the write through, with the operation-owned
    // warnings and the check contract (R0170/R0171).
    home.run(&["config", "set", "gate.agent.R3", "advise"]);
    let written = mcp.ok(
        "create_successor",
        json!({ "at": topic, "title": "短的标题", "body": body }),
    );
    let warnings = written["warnings"].as_array().expect("warnings");
    assert!(
        warnings
            .iter()
            .any(|warning| warning["rule"] == json!("R3")
                && warning["fault"] == json!("semicolon")
                && warning["legacy"] == json!(false)),
        "{written}"
    );
    assert_eq!(written["check"]["complete"], json!(true));
    assert_eq!(written["check"]["stage"], json!("enforce"));
    assert_eq!(written["check"]["rule_version"], json!(zt_check::VERSION));
    assert_eq!(home.cards(), cards + 1);
}

/// The rollout default: a quality fault is reported as advice and the Card is
/// stored, so old writers are not stranded before the coordinated release.
#[test]
fn the_advisory_archive_stores_the_card_and_returns_its_warnings() {
    let home = Home::new(None);
    let mut mcp = home.mcp();
    let topic = mcp.ok("create_topic", json!({ "title": "主题", "body": "主题说明。" }))
        ["address"]
        .as_str()
        .expect("topic address")
        .to_string();
    let revision = home.revision();
    let written = mcp.ok(
        "create_successor",
        json!({ "at": topic, "type": "event", "title": "1999 年的一笔交易", "body": "双方签约。" }),
    );
    assert_eq!(written["check"]["stage"], json!("advisory"));
    assert!(
        written["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning["rule"] == json!("E2")),
        "{written}"
    );
    assert!(home.revision() > revision);
    assert!(
        home.text(&format!("{}/1", topic.split('/').next().expect("tree")))
            .contains("双方签约"),
        "the advisory archive stores the Card"
    );
}

/// S04-AC05/R0175: a refused real batch names its item and rolls the whole
/// batch back, receipts included.
#[test]
fn a_refused_real_batch_names_its_item_and_rolls_back() {
    let home = Home::new(Some("enforce"));
    let mut mcp = home.mcp();
    let topic = mcp.ok("create_topic", json!({ "title": "主题", "body": "主题说明。" }))
        ["address"]
        .as_str()
        .expect("topic address")
        .to_string();
    let target = mcp.ok("create_topic", json!({ "title": "目标", "body": "目标说明。" }))
        ["address"]
        .as_str()
        .expect("target address")
        .to_string();
    let cards = home.cards();
    let revision = home.revision();
    let first = format!(
        "这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。对应 [[{target}]]：这条理由说明选择。"
    );
    let items = json!([
        {
            "op_key": "s04/batch/1/item/0000",
            "client_ref": "0",
            "tool": "create_successor",
            "arguments": { "at": topic, "title": "第一个", "body": first }
        },
        {
            "op_key": "s04/batch/1/item/0001",
            "client_ref": "1",
            "tool": "create_successor",
            "arguments": { "at": "@@0", "title": "第二个", "body": "甲；乙。" }
        }
    ]);
    let (value, is_error) = mcp.call("apply_batch", json!({ "items": items }));
    assert!(is_error, "the batch must fail: {value}");
    assert_eq!(value["error"], json!("card_check"));
    assert_eq!(value["item"]["index"], json!(1));
    assert_eq!(value["item"]["tool"], json!("create_successor"));
    assert_eq!(value["item"]["op_key"], json!("s04/batch/1/item/0001"));
    assert!(
        fault_rules(&value).iter().any(|rule| rule == "R3"),
        "{value}"
    );
    assert_eq!(home.cards(), cards, "the whole batch rolled back");
    assert_eq!(home.receipts(), 0, "no receipt survived the refusal");
    assert_eq!(home.revision(), revision);
}

/// S04-AC06/R0164: the GUI's write routes return the same structured check, so a
/// refusal keeps its fault lines instead of one opaque message.
#[test]
fn the_gui_write_returns_warnings_and_a_structured_refusal() {
    let home = Home::new(None);
    let gui = home.gui();
    let (status, topic) = gui.post(
        "/api/create",
        &json!({ "kind": "topic", "workspace": "agent", "text": "# 主题\n\n主题说明。\n" }),
    );
    assert_eq!(status, 200, "{topic}");
    let topic = topic["address"].as_str().expect("topic address").to_string();
    let (status, made) = gui.post(
        "/api/create",
        &json!({ "kind": "direct", "at": topic, "text": "# 短的标题\n\n甲；乙。\n" }),
    );
    assert_eq!(status, 200, "{made}");
    assert_eq!(made["check"]["stage"], json!("advisory"));
    assert!(
        made["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning["rule"] == json!("R3")),
        "{made}"
    );
    let address = made["address"].as_str().expect("address").to_string();
    let stored = home.text(&address);

    home.run(&["config", "set", "gate.stage", "enforce"]);
    let (status, refused) = gui.post(
        "/api/edit",
        &json!({ "at": address, "text": "# 标题\n\n丙；丁。\n" }),
    );
    assert_eq!(status, 400, "{refused}");
    assert!(
        refused["error"]
            .as_str()
            .is_some_and(|error| error.starts_with("card_check: ")),
        "{refused}"
    );
    assert_eq!(refused["check"]["error"], json!("card_check"));
    assert_eq!(refused["check"]["address"], json!(address));
    assert!(
        fault_rules(&refused["check"]).iter().any(|rule| rule == "R3"),
        "{refused}"
    );
    assert_eq!(home.text(&address), stored, "the refused edit changed no text");
    let (status, card) = gui.get(&format!("/api/card?at={address}"));
    assert_eq!(status, 200, "{card}");
    assert_eq!(card["text"], json!(stored), "the draft was not stored");
}

/// S04-AC03: the direct shell writers reach the same gate: warnings go to the
/// diagnostic stream, and an enforced refusal stores nothing.
#[test]
fn the_shell_reports_warnings_and_refuses_through_the_gate() {
    let home = Home::new(None);
    let out = home
        .cmd_with_editor("# 主题\n\n甲；乙。\n")
        .args(["t", "主题"])
        .output()
        .expect("zt t");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("check:"), "a warning reaches stderr: {stderr}");
    let topic = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(home.text(&topic).contains("甲；乙。"));

    home.run(&["config", "set", "gate.stage", "enforce"]);
    let cards = home.cards();
    let out = home
        .cmd_with_editor("# 一笔价格\n\n价格是 $5。\n")
        .args(["n", "--at", &topic])
        .output()
        .expect("zt n");
    assert!(!out.status.success(), "the enforced shell write must fail");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("card_check: "),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(home.cards(), cards, "a refused shell write creates no Card");
}

/// S04-AC08: a citation-key remap is structural, so it runs no prose check and
/// keeps working while the gate is enforcing.
#[test]
fn a_citation_key_remap_keeps_its_structural_path() {
    let home = Home::new(Some("enforce"));
    let mut mcp = home.mcp();
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{bookA, title={Book A}}", "body": "该书。" }),
    );
    let renamed = mcp.ok(
        "edit_literature_metadata",
        json!({ "at": "bookA", "bibtex": "@book{bookB, title={Book A}}", "confirm": "move" }),
    );
    assert_eq!(renamed["address"], json!("bookB"), "{renamed}");
    assert!(home.conn().query_row(
        "SELECT COUNT(*) FROM cards WHERE citation_key = 'bookB'",
        [],
        |row| row.get::<_, i64>(0)
    )
    .expect("renamed Card")
        == 1);
}
