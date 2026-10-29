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
    /// Freeze one tree through the real two-step page-token route.
    fn lock(&self, root: &str) {
        let (status, plan) = self.post("/api/lock", &json!({ "root": root }));
        assert_eq!(status, 200, "{plan}");
        assert_eq!(plan["applied"], json!(false), "the first request only plans: {plan}");
        let (status, applied) = self.post(
            "/api/lock",
            &json!({
                "root": root,
                "confirm": "lock",
                "plan_hash": plan["plan"]["plan_hash"].clone()
            }),
        );
        assert_eq!(status, 200, "{applied}");
        assert_eq!(applied["applied"], json!(true), "the confirmation commits: {applied}");
    }

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
    assert_eq!(
        written["check"]["rule_version"],
        json!(zt_check::contract::RULE_VERSION),
        "the write names the stable rule contract, not the executable version"
    );
    assert_eq!(written["check"]["checker_version"], json!(zt_check::VERSION));
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

/// A body that passes the enforced gate and still carries advice, so an accepted
/// preview item is proven to keep the ordinary operation-owned warnings.
fn advised_body(target: &str) -> String {
    format!(
        "这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。式 (12) 有该项。对应 [[{target}]]：这条理由说明选择。"
    )
}

/// S05 (#147) / R0179–R0180, US018–US019: a rollback-only preview treats a
/// blocking Card check like a typed per-item refusal. It collects the refusal
/// with its diagnostics, keeps simulating the independent items after it, marks
/// a dependent item blocked instead of inventing a target, and writes nothing.
#[test]
fn a_preview_collects_a_card_check_refusal_and_blocks_its_dependent() {
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
    let advised = advised_body(&target);
    let cards = home.cards();
    let revision = home.revision();
    let next_topic = home.next_topic();
    let receipts = home.receipts();
    let items = json!([
        {
            "op_key": "s05/draft/1/create/bad",
            "client_ref": "bad",
            "tool": "create_successor",
            "arguments": { "at": topic, "title": "拒绝卡", "body": "甲；乙。" }
        },
        {
            "op_key": "s05/draft/1/create/dependent",
            "client_ref": "dependent",
            "tool": "create_successor",
            "arguments": { "at": "@@bad", "title": "依赖卡", "body": advised }
        },
        {
            "op_key": "s05/draft/1/create/good",
            "client_ref": "good",
            "tool": "create_successor",
            "arguments": { "at": topic, "title": "独立卡", "body": advised }
        }
    ]);
    let draft = mcp.ok("apply_batch", json!({ "preview": true, "items": items }));
    assert_eq!(draft["preview"], json!(true), "{draft}");
    assert_eq!(draft["applied"], json!(false), "{draft}");
    assert_eq!(
        draft["complete"],
        json!(false),
        "a refused plan is not complete: {draft}"
    );
    assert_eq!(
        draft["scan"],
        json!({ "items": 3, "checked": 1, "refused": 1, "blocked": 1 }),
        "the preview states exactly what it checked and what it could not: {draft}"
    );
    assert_eq!(draft["archive_revision"], json!(revision), "{draft}");

    let refusal = preview_refusal(&draft, 0);
    assert_eq!(refusal["op_key"], json!("s05/draft/1/create/bad"), "{draft}");
    assert_eq!(refusal["client_ref"], json!("bad"), "{draft}");
    let check = &refusal["refusal"];
    assert_eq!(check["error"], json!("card_check"), "{draft}");
    assert_eq!(check["address"], json!("0/1"), "{draft}");
    assert!(
        check["blocking"].as_u64().expect("blocking") >= 1,
        "{draft}"
    );
    assert!(
        fault_rules(check).iter().any(|rule| rule == "R3"),
        "the refusal keeps its fault lines: {draft}"
    );
    assert!(
        check["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["rule"] == json!("R3")
                && diagnostic["fault"] == json!("semicolon")
                && diagnostic["severity"] == json!("block")),
        "the refusal keeps structured diagnostics: {draft}"
    );
    assert_eq!(check["zt_check_version"], json!(zt_check::VERSION), "{draft}");
    assert_eq!(check["policy_hash"].as_str().map(str::len), Some(64), "{draft}");

    let blocked = draft["blocked"]
        .as_array()
        .unwrap_or_else(|| panic!("{draft}"));
    assert_eq!(blocked.len(), 1, "{draft}");
    assert_eq!(blocked[0]["index"], json!(1), "{draft}");
    assert_eq!(
        blocked[0]["client_ref"],
        json!("dependent"),
        "a dependent item is named: {draft}"
    );
    assert_eq!(
        blocked[0]["reason"],
        json!("blocked_by_refusal"),
        "{draft}"
    );
    assert_eq!(
        blocked[0]["blocked_by"],
        json!([{ "index": 0, "op_key": "s05/draft/1/create/bad", "client_ref": "bad" }]),
        "{draft}"
    );
    assert!(
        blocked[0].get("address").is_none(),
        "no address is invented for a blocked item: {draft}"
    );

    let results = draft["results"].as_array().expect("results");
    assert_eq!(results.len(), 1, "{draft}");
    assert_eq!(results[0]["op_key"], json!("s05/draft/1/create/good"), "{draft}");
    let accepted = &results[0]["result"];
    assert_eq!(accepted["address"], json!("0/1"), "{draft}");
    assert_eq!(accepted["check"]["complete"], json!(true), "{draft}");
    assert_eq!(accepted["check"]["stage"], json!("enforce"), "{draft}");
    assert!(
        accepted["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning["rule"] == json!("R20") && warning["legacy"] == json!(false)),
        "an accepted preview item keeps its operation-owned warnings: {draft}"
    );
    let cards_projected = draft["cards"].as_array().expect("cards");
    assert_eq!(cards_projected.len(), 1, "only the checked item projects a Card: {draft}");
    assert_eq!(cards_projected[0]["address"], json!("0/1"), "{draft}");
    assert_eq!(cards_projected[0]["body"], json!(advised), "{draft}");

    // Preview uses the same writer/gate path: the single-Card route refuses the
    // identical proposal with the same address, faults and diagnostics.
    let (single, is_error) = mcp.call(
        "create_successor",
        json!({ "at": topic, "title": "拒绝卡", "body": "甲；乙。" }),
    );
    assert!(is_error, "the single-Card write is refused: {single}");
    assert_eq!(single["error"], json!("card_check"), "{single}");
    assert_eq!(single["address"], check["address"], "{single}");
    assert_eq!(single["blocking"], check["blocking"], "{single}");
    assert_eq!(single["faults"], check["faults"], "{single}");
    assert_eq!(single["diagnostics"], check["diagnostics"], "{single}");
    assert_eq!(single["policy_hash"], check["policy_hash"], "{single}");

    // Everything rolled back: no Card, receipt, Topic allocation or revision.
    assert_eq!(home.cards(), cards, "the preview created no Card");
    assert_eq!(home.revision(), revision, "the preview moved no revision");
    assert_eq!(home.next_topic(), next_topic, "the preview allocated no Topic id");
    assert_eq!(home.receipts(), receipts, "the preview kept no receipt");
}

/// The refusal entry of one planned preview item, in plan order.
fn preview_refusal(draft: &Value, index: i64) -> &Value {
    draft["refusals"]
        .as_array()
        .unwrap_or_else(|| panic!("{draft}"))
        .iter()
        .find(|refusal| refusal["index"] == json!(index))
        .unwrap_or_else(|| panic!("no refusal at item {index}: {draft}"))
}

/// The durable receipts one OperationPrefix owns.
fn stored_receipts(home: &Home, prefix: &str) -> i64 {
    home.conn()
        .query_row(
            "SELECT COUNT(*) FROM applied_ops WHERE op_key LIKE ?1",
            [format!("{prefix}%")],
            |row| row.get(0),
        )
        .unwrap_or(0)
}

/// S05 (#147) / R0175–R0178, R0181, US017: a real batch stays ordered and
/// atomic. A later item sees an earlier successful candidate in the same
/// transaction, one blocking item names its zero-based index/tool/operation key
/// and rolls back every item, receipt, allocation and revision of this chunk,
/// while the earlier committed chunk stays distinguishable and replayable.
#[test]
fn a_real_batch_names_its_blocking_item_and_keeps_earlier_chunks() {
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
    let advised = advised_body(&target);
    let first_chunk = json!([
        {
            "op_key": "s05/chunk1/1/create/first",
            "client_ref": "first",
            "tool": "create_successor",
            "arguments": { "at": topic, "title": "第一卡", "body": advised }
        },
        {
            "op_key": "s05/chunk1/1/create/topic",
            "client_ref": "topic",
            "tool": "create_successor",
            "arguments": { "at": "@@first", "title": "第二卡", "body": advised }
        }
    ]);
    let committed = mcp.ok("apply_batch", json!({ "items": first_chunk.clone() }));
    let first = committed["results"][0]["result"]["address"]
        .as_str()
        .expect("first address")
        .to_string();
    let second = committed["results"][1]["result"]["address"]
        .as_str()
        .expect("second address")
        .to_string();
    assert_ne!(first, second);
    assert_eq!(home.cards(), 4, "the two setup Topics and the two batch Cards");
    assert_eq!(stored_receipts(&home, "s05/chunk1/"), 2);
    let cards_after_chunk1 = home.cards();
    let revision_after_chunk1 = home.revision();
    let next_topic_after_chunk1 = home.next_topic();

    // A lost response is recovered by repeating the identical accepted input: the
    // durable receipts answer again and allocate nothing.
    let replayed = mcp.ok("apply_batch", json!({ "items": first_chunk.clone() }));
    assert!(
        replayed["results"]
            .as_array()
            .expect("results")
            .iter()
            .all(|result| result["replayed"] == json!(true)),
        "the committed chunk replays its receipts: {replayed}"
    );
    assert_eq!(
        replayed["results"][0]["result"]["address"],
        json!(first),
        "{replayed}"
    );
    assert_eq!(
        replayed["results"][0]["result"]["after_fingerprint"],
        committed["results"][0]["result"]["after_fingerprint"],
        "the receipt keeps its durable output: {replayed}"
    );
    assert_eq!(home.cards(), cards_after_chunk1);
    assert_eq!(home.revision(), revision_after_chunk1, "a replay moves no revision");

    // The next chunk creates one Topic, then meets a blocking Card check. Its item
    // identity is the zero-based index; the whole unseen chunk rolls back.
    let second_chunk = json!([
        {
            "op_key": "s05/chunk2/1/create/topic",
            "client_ref": "topic",
            "tool": "create_topic",
            "arguments": { "title": "未提交主题", "body": "未提交主题。" }
        },
        {
            "op_key": "s05/chunk2/1/create/bad",
            "client_ref": "bad",
            "tool": "create_successor",
            "arguments": { "at": target, "title": "拒绝卡", "body": "甲；乙。" }
        },
        {
            "op_key": "s05/chunk2/1/create/last",
            "client_ref": "last",
            "tool": "create_successor",
            "arguments": { "at": "@@topic", "title": "也不提交", "body": advised }
        }
    ]);
    let (refused, is_error) = mcp.call("apply_batch", json!({ "items": second_chunk }));
    assert!(is_error, "the blocking item refuses the chunk: {refused}");
    assert_eq!(refused["error"], json!("card_check"), "{refused}");
    assert_eq!(
        refused["item"],
        json!({
            "index": 1,
            "tool": "create_successor",
            "op_key": "s05/chunk2/1/create/bad"
        }),
        "the refusal names the zero-based item, its tool and its operation key: {refused}"
    );
    assert!(
        refused["message"].as_str().is_some_and(|message| message
            .starts_with("batch item 1 (create_successor): card_check: ")),
        "the human message keeps the existing batch-item convention: {refused}"
    );
    assert!(fault_rules(&refused).iter().any(|rule| rule == "R3"), "{refused}");
    assert_eq!(home.cards(), cards_after_chunk1, "no item of the chunk persisted");
    assert_eq!(home.revision(), revision_after_chunk1);
    assert_eq!(home.next_topic(), next_topic_after_chunk1, "no allocation persisted");
    assert_eq!(stored_receipts(&home, "s05/chunk2/"), 0, "the refused chunk kept no receipt");
    assert_eq!(
        stored_receipts(&home, "s05/chunk1/"),
        2,
        "the earlier committed chunk stays distinguishable and stored"
    );
    for title in ["未提交主题", "拒绝卡", "也不提交"] {
        let stored: i64 = home
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM cards WHERE text LIKE ?1",
                [format!("%{title}%")],
                |row| row.get(0),
            )
            .expect("stored title count");
        assert_eq!(stored, 0, "the refused chunk wrote no `{title}` Card");
    }
    assert!(
        home.text(&first).contains("这一段说明背景"),
        "the first chunk's Card is still stored"
    );
    assert!(
        home.text(&second).contains("这一段说明背景"),
        "the dependent Card of the first chunk is still stored"
    );
}

/// S05 (#147) / R0177–R0181, US017–US019: the larger preview sequence collects a
/// Card-check refusal at a numeric item, blocks the item that references it,
/// still simulates the later independent item, and lets that item resolve a Link
/// to a Card created earlier in the same rollback-only transaction. A forward
/// reference remains unsupported and keeps the whole-request failure contract.
#[test]
fn a_preview_sequence_collects_refusals_and_resolves_earlier_candidates_only() {
    let home = Home::new(Some("enforce"));
    let mut mcp = home.mcp();
    let revision = home.revision();
    let stored_cards = home.cards();
    let key = |index: usize| format!("s05/sequence/1/apply/item-{index:04}");
    let linking = "这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。对应 [[@@1]]：这条理由说明选择。";
    let items = json!([
        {
            "op_key": key(0),
            "client_ref": "0",
            "tool": "create_topic",
            "arguments": { "title": "序列范围", "body": "序列说明。" }
        },
        {
            "op_key": key(1),
            "client_ref": "1",
            "tool": "create_topic",
            "arguments": { "title": "序列目标", "body": "目标说明。" }
        },
        {
            "op_key": key(2),
            "client_ref": "2",
            "tool": "create_successor",
            "arguments": { "at": "@@0", "title": "拒绝卡", "body": "甲；乙。" }
        },
        {
            "op_key": key(3),
            "client_ref": "3",
            "tool": "create_successor",
            "arguments": { "at": "@@2", "title": "依赖卡", "body": linking }
        },
        {
            "op_key": key(4),
            "client_ref": "4",
            "tool": "create_successor",
            "arguments": { "at": "@@3", "title": "传递依赖卡", "body": linking }
        },
        {
            "op_key": key(5),
            "client_ref": "5",
            "tool": "create_successor",
            "arguments": { "at": "@@0", "title": "独立卡", "body": linking }
        }
    ]);
    let draft = mcp.ok(
        "apply_batch",
        json!({ "preview": true, "preview_items": items }),
    );
    assert_eq!(draft["preview"], json!(true), "{draft}");
    assert_eq!(draft["applied"], json!(false), "{draft}");
    assert_eq!(draft["complete"], json!(false), "{draft}");
    assert_eq!(draft["archive_revision"], json!(revision), "{draft}");
    assert_eq!(
        draft["scan"],
        json!({ "items": 6, "checked": 3, "refused": 1, "blocked": 2 }),
        "{draft}"
    );
    let refusal = preview_refusal(&draft, 2);
    assert_eq!(refusal["op_key"], json!(key(2)), "{draft}");
    assert_eq!(refusal["refusal"]["error"], json!("card_check"), "{draft}");
    assert!(
        fault_rules(&refusal["refusal"])
            .iter()
            .any(|rule| rule == "R3"),
        "{draft}"
    );
    let blocked = draft["blocked"].as_array().expect("blocked");
    assert_eq!(blocked.len(), 2, "{draft}");
    assert_eq!(
        (
            blocked[0]["index"].as_i64(),
            blocked[0]["blocked_by"][0]["index"].as_i64()
        ),
        (Some(3), Some(2)),
        "a direct dependent is blocked by the refused item: {draft}"
    );
    assert_eq!(
        (
            blocked[1]["index"].as_i64(),
            blocked[1]["blocked_by"][0]["index"].as_i64()
        ),
        (Some(4), Some(3)),
        "a dependent of a blocked item is blocked through the chain, not invented: {draft}"
    );
    assert_eq!(
        draft["results"]
            .as_array()
            .expect("results")
            .iter()
            .map(|result| result["client_ref"].as_str().expect("client_ref").to_string())
            .collect::<Vec<_>>(),
        ["0", "1", "5"],
        "the independent item after the refusal and its blocked dependents still runs: {draft}"
    );
    // The last item resolved `[[@@1]]` against the Topic this same preview created,
    // and its own parent resolved against the first candidate.
    let cards = draft["cards"].as_array().expect("cards");
    assert_eq!(cards.len(), 3, "{draft}");
    let independent = cards
        .iter()
        .find(|card| card["address"] == json!("0/1"))
        .unwrap_or_else(|| panic!("the independent candidate is projected: {draft}"));
    assert_eq!(independent["parent_address"], json!("0/0"), "{draft}");
    assert_eq!(independent["links"], json!(["1/0"]), "{draft}");

    // A forward reference is still unsupported: nothing is bound before its item,
    // and the ordinary whole-request failure leaves the archive untouched.
    let forward = json!([
        {
            "op_key": key(0),
            "client_ref": "0",
            "tool": "create_successor",
            "arguments": { "at": "@@1", "title": "前向卡", "body": linking }
        },
        {
            "op_key": key(1),
            "client_ref": "1",
            "tool": "create_topic",
            "arguments": { "title": "后建主题", "body": "后建说明。" }
        }
    ]);
    let (error, is_error) = mcp.call(
        "apply_batch",
        json!({ "preview": true, "preview_items": forward }),
    );
    assert!(is_error, "a forward reference is refused: {error}");
    assert!(
        error["error"]
            .as_str()
            .is_some_and(|error| error.contains("unresolved earlier client reference `@@1`")),
        "{error}"
    );
    assert_eq!(home.cards(), stored_cards, "a refused preview sequence writes no Card");
    assert_eq!(home.revision(), revision, "a refused preview sequence moves no revision");
}

/// S05-AC03 / R0179: one preview collects a Tree Lock refusal and a Card-check
/// refusal independently, in plan order, and blocks each refused item's dependent
/// without evaluating it against a fabricated target.
#[test]
fn a_preview_collects_lock_and_card_check_refusals_together() {
    let home = Home::new(Some("enforce"));
    let gui = home.gui();
    let mut mcp = home.mcp();
    let topic = mcp.ok("create_topic", json!({ "title": "主题", "body": "主题说明。" }))
        ["address"]
        .as_str()
        .expect("topic address")
        .to_string();
    let frozen = mcp.ok("create_topic", json!({ "title": "冰封", "body": "冰封说明。" }))
        ["address"]
        .as_str()
        .expect("frozen topic address")
        .to_string();
    let target = mcp.ok("create_topic", json!({ "title": "目标", "body": "目标说明。" }))
        ["address"]
        .as_str()
        .expect("target address")
        .to_string();
    let frozen_card = mcp.ok(
        "create_successor",
        json!({ "at": frozen, "title": "冰封卡", "body": advised_body(&target) }),
    )["address"]
        .as_str()
        .expect("frozen card address")
        .to_string();
    gui.lock(&frozen);
    let cards = home.cards();
    let revision = home.revision();
    let lock_history = lock_facts(&home);
    let items = json!([
        {
            "op_key": "s05/mixed/1/create/locked",
            "client_ref": "locked",
            "tool": "create_successor",
            "arguments": { "at": frozen_card, "title": "锁内卡", "body": advised_body(&target) }
        },
        {
            "op_key": "s05/mixed/1/create/locked-dependent",
            "client_ref": "lockedDependent",
            "tool": "create_successor",
            "arguments": { "at": "@@locked", "title": "锁依赖卡", "body": advised_body(&target) }
        },
        {
            "op_key": "s05/mixed/1/create/bad",
            "client_ref": "bad",
            "tool": "create_successor",
            "arguments": { "at": topic, "title": "拒绝卡", "body": "甲；乙。" }
        },
        {
            "op_key": "s05/mixed/1/create/bad-dependent",
            "client_ref": "badDependent",
            "tool": "create_successor",
            "arguments": { "at": "@@bad", "title": "拒绝依赖卡", "body": advised_body(&target) }
        },
        {
            "op_key": "s05/mixed/1/create/good",
            "client_ref": "good",
            "tool": "create_successor",
            "arguments": { "at": topic, "title": "独立卡", "body": advised_body(&target) }
        }
    ]);
    let draft = mcp.ok("apply_batch", json!({ "preview": true, "items": items }));
    assert_eq!(draft["complete"], json!(false), "{draft}");
    assert_eq!(
        draft["scan"],
        json!({ "items": 5, "checked": 1, "refused": 2, "blocked": 2 }),
        "{draft}"
    );
    let refusals = draft["refusals"].as_array().expect("refusals");
    assert_eq!(
        refusals
            .iter()
            .map(|refusal| refusal["index"].as_i64().expect("index"))
            .collect::<Vec<_>>(),
        [0, 2],
        "both refusals keep plan order: {draft}"
    );
    assert_eq!(
        preview_refusal(&draft, 0)["refusal"]["error"],
        json!("card_locked"),
        "{draft}"
    );
    assert_eq!(
        preview_refusal(&draft, 0)["refusal"]["reason"],
        json!("create-inside"),
        "{draft}"
    );
    assert_eq!(
        preview_refusal(&draft, 2)["refusal"]["error"],
        json!("card_check"),
        "{draft}"
    );
    let blocked = draft["blocked"].as_array().expect("blocked");
    assert_eq!(
        blocked
            .iter()
            .map(|item| (
                item["index"].as_i64().expect("index"),
                item["blocked_by"][0]["index"].as_i64().expect("blocked_by")
            ))
            .collect::<Vec<_>>(),
        [(1, 0), (3, 2)],
        "each dependent is blocked by its own refused item: {draft}"
    );
    assert_eq!(
        draft["results"][0]["op_key"],
        json!("s05/mixed/1/create/good"),
        "{draft}"
    );
    assert_eq!(home.cards(), cards, "the preview wrote no Card");
    assert_eq!(home.revision(), revision, "the preview moved no revision");
    assert_eq!(
        lock_facts(&home),
        lock_history,
        "the preview kept no Lock history"
    );
}

/// The stored Lock facts a preview must leave alone: Locks, members and history.
fn lock_facts(home: &Home) -> (i64, i64, i64) {
    let conn = home.conn();
    let count = |table: &str| {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get::<_, i64>(0)
        })
        .expect("lock table count")
    };
    (
        count("locks"),
        count("lock_members"),
        count("lock_events"),
    )
}

/// S29/R0510: a new Event or Finding Card that lacks its required labels is refused by its type's
/// format rule, and the archive keeps no Card, receipt, revision or Topic allocation.
#[test]
fn a_new_event_or_finding_without_its_labels_is_refused_without_residue() {
    let home = Home::new(Some("enforce"));
    let mut mcp = home.mcp();
    let topic = mcp.ok("create_topic", json!({ "title": "主题", "body": "主题说明。" }))
        ["address"]
        .as_str()
        .expect("topic address")
        .to_string();
    for (card_type, title, body, rule) in [
        ("event", "1999 年的一笔交易", "双方签约，交易完成。", "E2"),
        ("finding", "试点提高了收入", "收入增加了 3 个百分点。", "F1"),
    ] {
        let cards = home.cards();
        let revision = home.revision();
        let receipts = home.receipts();
        let next_topic = home.next_topic();
        let (value, is_error) = mcp.call(
            "create_successor",
            json!({ "at": topic, "type": card_type, "title": title, "body": body }),
        );
        assert!(is_error, "the {card_type} Card must be refused: {value}");
        assert_eq!(value["error"], json!("card_check"), "{value}");
        assert!(
            fault_rules(&value).iter().any(|found| found == rule),
            "the refusal names {rule}: {value}"
        );
        assert_eq!(home.cards(), cards, "a refused {card_type} writes no Card");
        assert_eq!(
            home.revision(),
            revision,
            "a refused {card_type} moves no revision"
        );
        assert_eq!(
            home.receipts(),
            receipts,
            "a refused {card_type} keeps no receipt"
        );
        assert_eq!(
            home.next_topic(),
            next_topic,
            "a refused {card_type} allocates no Topic id"
        );
    }
}

/// S29/R0516: promoting a concept model to formal makes the formal size rule applicable. The
/// promotion stores as a metadata edit; the part's same write passes while the head is a concept
/// model and is then refused by R13, with no stored change.
#[test]
fn promoting_a_concept_model_to_formal_runs_the_new_formal_checks() {
    let home = Home::new(None);
    let mut mcp = home.mcp();
    let topic = mcp.ok("create_topic", json!({ "title": "模型主题", "body": "主题说明。" }))
        ["address"]
        .as_str()
        .expect("topic address")
        .to_string();
    let model = mcp.ok(
        "create_successor",
        json!({
            "at": topic, "title": "均衡模型", "type": "model", "level": "concept",
            "body": "这一段说明背景并给出充分理由。"
        }),
    )["address"]
        .as_str()
        .expect("model address")
        .to_string();
    let long_body: String = (1..=19).map(|n| format!("第 {n} 句说明这一步。")).collect();
    let part = mcp.ok(
        "create_successor",
        json!({
            "at": model, "kind": "side", "facet": "设定",
            "title": "设定", "body": long_body
        }),
    )["address"]
        .as_str()
        .expect("part address")
        .to_string();
    home.run(&["config", "set", "gate.stage", "enforce"]);
    // A concept model skips the formal size rule, so the part's edit is stored.
    let concept_body = format!("{long_body}第 20 句说明这一步。");
    mcp.ok("edit_card", json!({ "at": part, "body": concept_body }));

    // The promotion is a metadata-only edit of the Model Card.
    let revision = home.revision();
    let promoted = mcp.ok("edit_card", json!({ "at": model, "level": "formal" }));
    assert_eq!(promoted["address"], json!(model), "{promoted}");
    assert_eq!(
        home.revision(),
        revision + 1,
        "the promotion stores one edit"
    );
    let level = home
        .conn()
        .query_row(
            "SELECT model_level FROM cards WHERE location = ?1",
            [&model],
            |row| row.get::<_, String>(0),
        )
        .expect("stored level");
    assert_eq!(level, "formal");

    // The same part write is now refused by the newly applicable formal rule.
    let stored = home.text(&part);
    let revision = home.revision();
    let (refusal, is_error) = mcp.call(
        "edit_card",
        json!({ "at": part, "body": format!("{concept_body}第 21 句说明这一步。") }),
    );
    assert!(is_error, "the formal rule now applies: {refusal}");
    assert_eq!(refusal["error"], json!("card_check"), "{refusal}");
    assert!(
        fault_rules(&refusal).iter().any(|rule| rule == "R13"),
        "the newly applicable rule is R13: {refusal}"
    );
    assert_eq!(home.text(&part), stored, "the refused edit changed no text");
    assert_eq!(home.revision(), revision, "the refused edit moved no revision");
}

/// S29/R0518: a move is a structural change. Under enforce, a Card that keeps its old prose fault
/// still moves, its Link still points at the same target, the target's generated Reverse section
/// is refreshed to the new address, and the authored text keeps the fault unchanged.
#[test]
fn a_move_keeps_a_retained_fault_and_refreshes_the_reverse_section() {
    let home = Home::new(None);
    let mut mcp = home.mcp();
    let topic = mcp.ok("create_topic", json!({ "title": "来源主题", "body": "主题说明。" }))
        ["address"]
        .as_str()
        .expect("topic address")
        .to_string();
    let target = mcp.ok("create_topic", json!({ "title": "目标主题", "body": "目标说明。" }))
        ["address"]
        .as_str()
        .expect("target address")
        .to_string();
    let host = mcp.ok(
        "create_successor",
        json!({ "at": topic, "title": "目标卡", "body": "这一段说明背景并给出充分理由。" }),
    )["address"]
        .as_str()
        .expect("host address")
        .to_string();
    let host2 = mcp.ok(
        "create_successor",
        json!({ "at": host, "title": "目标卡二", "body": "这一段说明背景并给出充分理由。" }),
    )["address"]
        .as_str()
        .expect("second host address")
        .to_string();
    let body = format!(
        "这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。对应 [[{target}]]：这条理由说明选择。甲；乙。"
    );
    let card = mcp.ok(
        "create_successor",
        json!({ "at": host, "kind": "side", "title": "保留缺陷", "body": body }),
    )["address"]
        .as_str()
        .expect("card address")
        .to_string();
    assert!(
        home.text(&target).contains(&format!("- [[{card}]]")),
        "the target lists the Card before the move: {}",
        home.text(&target)
    );
    home.run(&["config", "set", "gate.stage", "enforce"]);

    let cards = home.cards();
    let destination = format!("{host2}|a");
    let moved = mcp.ok(
        "move_card",
        json!({ "at": card, "to": destination, "confirm": "move" }),
    );
    assert_eq!(moved["applied"], json!(true), "{moved}");
    assert_eq!(home.cards(), cards, "a move stores no new Card");
    let stored = home.text(&destination);
    assert!(
        stored.contains("甲；乙。"),
        "the retained prose fault is unchanged: {stored}"
    );
    assert!(
        stored.contains(&format!("[[{target}]]")),
        "the Link target is unchanged: {stored}"
    );
    let reverse = home.text(&target);
    assert!(
        reverse.contains(&format!("- [[{destination}]]")),
        "the target's generated Reverse section names the new address: {reverse}"
    );
    assert!(
        !reverse.contains(&format!("- [[{card}]]")),
        "the old address is gone from the Reverse section: {reverse}"
    );
}
