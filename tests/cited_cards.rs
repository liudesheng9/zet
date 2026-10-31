//! Cited Cards (#217): the Card type `cited`. A Cited list Card is a `cited` Card whose parent is
//! not one; its immediate Side successors are its Cited Cards, matched to the table rows by order.
//! The role is computed from the type and the placement, never stored; `get_cited` reads it.
#![allow(dead_code)]

use assert_cmd::Command;
use rusqlite::Connection;
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

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

/// A two-row Cited list table, in the five columns the list Card renders.
const TABLE: &str = "本卡是《Test Book》的参考文献表。\n\n| 编号 | 文献 | 年份 | 引用处 | 关系 |\n| --- | --- | --- | --- | --- |\n| 1 | Deming. The Growing Importance of Social Skills. *QJE* | 2017 | §1 | 依赖 |\n| 2 | Bloom, Van Reenen. Measuring and Explaining Management Practices. *QJE* | 2007 | — | 背景 |\n\n来源：[[Test2026]] 参考文献\n";

/// One agent Literature tree whose first anchor holds a Cited list Card; returns (anchor, list).
fn book_with_a_cited_list(mcp: &mut Mcp) -> (String, String) {
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Test2026, title={Test Book}}" }),
    );
    let anchor = address(mcp.ok(
        "create_successor",
        json!({ "at": "Test2026", "title": "第一章", "body": "设置。\n\n来源：[[Test2026]] §1" }),
    ));
    let list = address(mcp.ok(
        "create_successor",
        json!({ "at": anchor, "kind": "side", "title": "参考文献：《Test Book》引用的 2 篇文献", "type": "cited", "body": TABLE }),
    ));
    (anchor, list)
}

/// One Cited Card as a Side successor of `at`, naming list row `row`.
fn cited(mcp: &mut Mcp, at: &str, title: &str, row: usize) -> String {
    address(mcp.ok(
        "create_successor",
        json!({
            "at": at,
            "kind": "side",
            "title": title,
            "type": "cited",
            "body": format!("文献：{title}\n\n## 为什么引用\n\n本书引用它。\n\n来源：[[Test2026]] 参考文献第 {row} 条"),
        }),
    ))
}

#[test]
fn get_cited_reads_the_rows_and_the_cited_cards_of_one_list_in_order() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (_, list) = book_with_a_cited_list(&mut mcp);
    let first = cited(&mut mcp, &list, "Deming（2017）：社交技能", 1);
    let second = cited(&mut mcp, &list, "Bloom 和 Van Reenen（2007）：管理实践", 2);

    let card = mcp.ok("get_card", json!({ "at": first }));
    assert_eq!(card["type"], json!("cited"), "{card}");

    let value = mcp.ok("get_cited", json!({ "at": "Test2026" }));
    assert_eq!(value["list"], json!(list), "{value}");
    let rows = value["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 2, "{value}");
    assert_eq!(rows[0]["row"], json!(1));
    assert_eq!(rows[0]["year"], json!("2017"));
    assert_eq!(rows[0]["card"], json!(first));
    assert_eq!(rows[1]["relation"], json!("背景"));
    assert_eq!(rows[1]["card"], json!(second));
    let cards = value["cards"].as_array().expect("cards");
    assert_eq!(cards.len(), 2, "{value}");
    assert_eq!(cards[1]["address"], json!(second));
    assert_eq!(cards[1]["row"], json!(2));
    assert_eq!(value["faults"], json!([]), "{value}");
    assert_eq!(value["coverage"], json!({ "rows": 2, "cards": 2, "missing": 0 }), "{value}");
    assert!(value["archive_revision"].is_number(), "{value}");
}

#[test]
fn get_cited_reports_unfiled_cards_missing_rows_extra_cards_and_a_wrong_row() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (_, list) = book_with_a_cited_list(&mut mcp);

    // One Card only: row 2 has no Card.
    let first = cited(&mut mcp, &list, "Deming（2017）：社交技能", 1);
    let value = mcp.ok("get_cited", json!({ "at": list }));
    assert_eq!(
        value["faults"],
        json!([{ "fault": "cited_row_without_card", "row": 2, "entry": "Bloom, Van Reenen. Measuring and Explaining Management Practices. *QJE*" }]),
        "{value}"
    );
    assert_eq!(value["coverage"]["missing"], json!(1));

    // A Card filed second that names row 3, a third Card with no row, and a Card under a Cited Card.
    let second = cited(&mut mcp, &list, "Other（2001）：别的文献", 3);
    let third = cited(&mut mcp, &list, "Extra（2002）：多出的文献", 3);
    let nested = cited(&mut mcp, &first, "Nested（2003）：放错的文献", 1);
    let value = mcp.ok("get_cited", json!({ "at": first }));
    let faults = value["faults"].as_array().expect("faults");
    assert!(faults.contains(&json!({ "fault": "cited_unfiled", "address": nested })), "{value}");
    assert!(faults.contains(&json!({ "fault": "cited_row_mismatch", "address": second, "row": 2, "names": 3 })), "{value}");
    assert!(faults.contains(&json!({ "fault": "cited_card_without_row", "address": third, "row": 3 })), "{value}");
    assert_eq!(faults.len(), 3, "{value}");
    assert_eq!(value["unfiled"], json!([nested]));
}

#[test]
fn a_cited_type_is_refused_on_a_tree_root_and_a_tree_without_a_list_is_named() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    mcp.ok("create_literature", json!({ "bibtex": "@book{Plain2026, title={Plain}}" }));
    let error = mcp.err("edit_card", json!({ "at": "Plain2026", "type": "cited" }));
    assert!(error.contains("only Regular Cards can be Event, Finding, Model, Hub, Term, or Cited Cards"), "{error}");
    let error = mcp.err("get_cited", json!({ "at": "Plain2026" }));
    assert!(error.contains("tree `Plain2026` has no Cited list"), "{error}");
}

#[test]
fn an_archive_from_before_cited_cards_upgrades_in_place_and_keeps_its_cards() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    mcp.ok("create_literature", json!({ "bibtex": "@book{Old2026, title={Old}}" }));
    let anchor = address(mcp.ok("create_successor", json!({ "at": "Old2026", "title": "第一章", "body": "旧卡。\n\n来源：[[Old2026]] §1" })));
    drop(mcp);
    home.cmd().arg("down").assert().success();

    // Put the type CHECK back to its pre-#217 form, as an older zt left it.
    {
        let conn = home.db();
        let sql: String = conn
            .query_row("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'", [], |row| row.get(0))
            .unwrap();
        let old = sql.replace(", 'cited'", "");
        assert_ne!(old, sql);
        conn.execute_batch("PRAGMA writable_schema = ON;").unwrap();
        conn.execute("UPDATE sqlite_master SET sql = ?1 WHERE type = 'table' AND name = 'cards'", [&old]).unwrap();
        conn.execute_batch("PRAGMA writable_schema = OFF;").unwrap();
    }

    home.cmd().arg("up").assert().success();
    let backups = fs::read_dir(&home.archive_root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("zt.sqlite3.pre-cited-cards-"))
        .count();
    assert_eq!(backups, 1, "the upgrade backs up a non-empty archive once");
    let mut mcp = home.mcp();
    let card = mcp.ok("get_card", json!({ "at": anchor }));
    assert!(card["body"].as_str().unwrap().contains("旧卡"), "{card}");
    let list = address(mcp.ok(
        "create_successor",
        json!({ "at": anchor, "kind": "side", "title": "参考文献：《Old》引用的 2 篇文献", "type": "cited", "body": TABLE.replace("Test2026", "Old2026") }),
    ));
    assert_eq!(mcp.ok("get_card", json!({ "at": list }))["type"], json!("cited"));
}

/// A Cited Card body the C rules accept, naming list row `row`.
fn good_body(row: usize) -> String {
    format!(
        "文献：Deming. The Growing Importance of Social Skills. *QJE*\n引用处：§1（共 1 处）\n关系：依赖\n\n## 为什么引用\n\n本书在引言中引用这篇文献，用来说明社交技能的回报在上升。\n\n## 在本书研究中的作用\n\n这篇文献给出了本书模型的出发点。\n\n## 本书与该文献的关系\n\n本书依赖它对社交技能的度量。\n\n来源：[[Test2026]] 参考文献第 {row} 条"
    )
}

#[test]
fn the_write_gate_applies_the_c_rules_to_a_cited_card_and_keeps_the_idea_rules_off() {
    let home = TestHome::new();
    home.cmd().args(["config", "set", "gate.stage", "enforce"]).assert().success();
    let mut mcp = home.mcp();
    let (_, list) = book_with_a_cited_list(&mut mcp);

    let missing = good_body(1).replace("## 为什么引用", "## 引用原因");
    let (refusal, is_error) = mcp.call(
        "create_successor",
        json!({ "at": list, "kind": "side", "title": "Deming（2017）：社交技能", "type": "cited", "body": missing }),
    );
    assert!(is_error, "{refusal}");
    assert_eq!(refusal["error"], json!("card_check"), "{refusal}");
    assert!(refusal.to_string().contains("C1"), "{refusal}");

    // A long sentence with a semicolon is no Idea-card fault on a Cited Card.
    let long = good_body(1).replace(
        "本书在引言中引用这篇文献，用来说明社交技能的回报在上升。",
        "本书在引言中引用这篇文献，用来说明社交技能在劳动力市场中的回报持续上升；高管层面缺少同样直接的证据，这正是本书要补上的空白。",
    );
    let card = address(mcp.ok(
        "create_successor",
        json!({ "at": list, "kind": "side", "title": "Deming（2017）：社交技能", "type": "cited", "body": long }),
    ));
    assert_eq!(mcp.ok("get_card", json!({ "at": card }))["type"], json!("cited"));
}
