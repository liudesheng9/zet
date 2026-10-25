//! Projection of visible Term prose in the public reads (S12, #154; design 10.4): raw reads stay the
//! default and keep the authored macros, the optional `term_links:"label"` mode returns the prose a
//! reader sees, and every projected reply says so. A projected body never feeds a fingerprint: the
//! statement fingerprint is computed from the raw authored text in either mode.

#![allow(dead_code)]

use assert_cmd::Command;
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
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    fn revision(&self) -> i64 {
        let conn = rusqlite::Connection::open(self.archive_root.join("zt.sqlite3"))
            .expect("open db");
        let value: String = conn
            .query_row(
                "SELECT value FROM metadata WHERE key='archive_revision'",
                [],
                |row| row.get(0),
            )
            .expect("archive revision");
        value.parse().expect("numeric revision")
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

    fn err(&mut self, name: &str, arguments: Value) -> String {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(
            is_error,
            "{name}({arguments}) should fail but returned {value}"
        );
        value["error"].as_str().expect("error message").to_string()
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

const TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n";

/// One agent Literature tree with one Term head, and one Idea Card that uses it and one ordinary
/// Link. The card's authored body is returned with the anchor and the head's address.
fn book_with_a_term_use(home: &TestHome, mcp: &mut Mcp) -> (String, String, String, String) {
    let _ = home;
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Test2026, title={Test Book}}" }),
    );
    let anchor = address(mcp.ok(
        "create_successor",
        json!({ "at": "Test2026", "title": "第一章", "body": "设置。" }),
    ));
    let list = address(mcp.ok(
        "create_successor",
        json!({
            "at": &anchor, "kind": "side", "title": "术语表：测试", "type": "term", "body": TABLE,
        }),
    ));
    let head = address(mcp.ok(
        "create_successor",
        json!({ "at": &list, "kind": "side", "title": "均衡", "type": "term", "term": "均衡" }),
    ));
    let body = format!("价格由 [[均衡@{head}]] 与 [[{anchor}]] 决定。");
    let card = address(mcp.ok(
        "create_successor",
        json!({ "at": &anchor, "title": "价格决定", "body": &body }),
    ));
    (anchor, head, card, body)
}

/// S12-AC03/S12-AC04 (R0273–R0275): `get_card` keeps the authored macros by default, the optional
/// label mode returns the visible prose and marks the reply, and the statement fingerprint is the
/// same in both modes because it is computed from the raw authored body.
#[test]
fn get_card_projects_term_labels_only_when_asked_and_marks_the_reply() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, _head, card, body) = book_with_a_term_use(&home, &mut mcp);
    let projected = format!("价格由 均衡 与 [[{anchor}]] 决定。");
    let revision = home.revision();

    let raw = mcp.ok("get_card", json!({ "at": &card }));
    assert_eq!(raw["body"], json!(body), "the raw read keeps the authored macro");
    assert!(raw["text"].as_str().unwrap().contains(&body));
    assert!(raw.get("term_links").is_none(), "{raw}");
    let fingerprint = raw["statement_fingerprint"].clone();

    let label = mcp.ok("get_card", json!({ "at": &card, "term_links": "label" }));
    assert_eq!(label["body"], json!(projected), "only the Term macro is projected");
    assert!(label["text"].as_str().unwrap().contains(&projected));
    assert!(
        !label["text"].as_str().unwrap().contains("[[均衡@"),
        "the projected text carries no Term macro"
    );
    assert_eq!(label["term_links"], json!("label"), "the reply is marked");
    assert_eq!(
        label["statement_fingerprint"], fingerprint,
        "a projected body never feeds the statement fingerprint"
    );

    // An explicit raw request is the default, and an unknown mode is refused.
    let explicit = mcp.ok("get_card", json!({ "at": &card, "term_links": "raw" }));
    assert_eq!(explicit["body"], json!(body));
    assert!(explicit.get("term_links").is_none());
    let error = mcp.err("get_card", json!({ "at": &card, "term_links": "visible" }));
    assert!(error.contains("term_links"), "{error}");

    // A projected read writes nothing and the stored body is still the authored one.
    assert_eq!(home.revision(), revision, "reads advance no revision");
    assert_eq!(mcp.ok("get_card", json!({ "at": &card }))["body"], json!(body));
}

/// S12-AC03/S12-AC05 (R0273–R0275): `get_cards` and `get_tree` carry the same option and the same
/// marker. A summary read marks only the cards whose text it projected; `get_tree` needs bodies.
#[test]
fn bulk_reads_project_term_labels_and_mark_every_projected_reply() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, _head, card, body) = book_with_a_term_use(&home, &mut mcp);
    let projected = format!("价格由 均衡 与 [[{anchor}]] 决定。");

    let raw = mcp.ok("get_cards", json!({ "addresses": [&card], "view": "text" }));
    assert!(raw.get("term_links").is_none(), "{raw}");
    assert_eq!(raw["cards"][0]["body"], json!(body));
    assert!(
        raw["cards"][0]["excerpt"].as_str().unwrap().contains("均衡@"),
        "the raw excerpt shows the authored macro's text"
    );
    let fingerprint = raw["cards"][0]["statement_fingerprint"].clone();

    let label = mcp.ok(
        "get_cards",
        json!({ "addresses": [&card], "view": "text", "term_links": "label" }),
    );
    assert_eq!(label["term_links"], json!("label"), "the reply is marked");
    assert_eq!(label["cards"][0]["term_links"], json!("label"));
    assert_eq!(label["cards"][0]["body"], json!(projected));
    let excerpt = label["cards"][0]["excerpt"].as_str().unwrap();
    assert!(
        excerpt.contains("价格由 均衡 与") && !excerpt.contains("均衡@"),
        "the projected excerpt reads the visible term: {excerpt}"
    );
    assert_eq!(label["cards"][0]["statement_fingerprint"], fingerprint);

    let tree_raw = mcp.ok(
        "get_tree",
        json!({ "tree": "Test2026", "bodies": true }),
    );
    assert!(tree_raw.get("term_links").is_none(), "{tree_raw}");
    let at = |read: &Value, card: &str| -> Value {
        read["cards"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["address"] == json!(card))
            .cloned()
            .expect("the card in the tree read")
    };
    assert_eq!(at(&tree_raw, &card)["body"], json!(body));

    let tree_label = mcp.ok(
        "get_tree",
        json!({ "tree": "Test2026", "bodies": true, "term_links": "label" }),
    );
    assert_eq!(tree_label["term_links"], json!("label"));
    let row = at(&tree_label, &card);
    assert_eq!(row["term_links"], json!("label"));
    assert_eq!(row["body"], json!(projected));
    assert!(!row["text"].as_str().unwrap().contains("[[均衡@"));

    // A summary tree read has no body or text to project, so the mode is refused, not faked.
    let error = mcp.err(
        "get_tree",
        json!({ "tree": "Test2026", "term_links": "label" }),
    );
    assert!(error.contains("bodies"), "{error}");
    let error = mcp.err(
        "get_cards",
        json!({ "addresses": [&card], "term_links": "label", "view": "summary" }),
    );
    assert!(error.contains("view"), "{error}");
}
