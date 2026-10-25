//! Term use lifecycle (S11, #153; design 10.3): the derived Link index keeps a Term occurrence
//! with `kind = 'term'`, the generated tail lists Term uses in their own section beside ordinary
//! Reverse links, moves/key renames rewrite only the target address, Broken and non-head targets
//! carry their authored label in `lsbk`, MCP and GUI reports, and reindex repairs derived drift.
//!
//! The archive is a scratch one (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`); the live archive is never
//! touched. Only the public CLI/MCP/GUI surfaces and the stored generated tail are asserted.

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

    /// Run a shell command whose editor writes `text` into the file it opens.
    fn cmd_with_editor(&self, text: &str) -> Command {
        let script = self.config_dir.join("fake-editor.ps1");
        fs::create_dir_all(&self.config_dir).expect("config dir");
        fs::write(
            &script,
            concat!(
                "if ($env:ZT_EDITOR_PATH_OUT) { [IO.File]::WriteAllText($env:ZT_EDITOR_PATH_OUT, $args[0]) }\n",
                "[IO.File]::WriteAllText($args[0], $env:ZT_EDITOR_TEXT)\n",
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

    fn revision(&self) -> i64 {
        let value: String = self
            .db()
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

    fn err(&mut self, name: &str, arguments: Value) -> String {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(
            is_error,
            "{name}({arguments}) should fail but returned {value}"
        );
        value["error"].as_str().expect("error message").to_string()
    }

    fn text(&mut self, at: &str) -> String {
        self.ok("get_card", json!({ "at": at }))["text"]
            .as_str()
            .expect("Card text")
            .to_string()
    }

    fn body(&mut self, at: &str) -> String {
        self.ok("get_card", json!({ "at": at }))["body"]
            .as_str()
            .expect("Card body")
            .to_string()
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
        json!({ "at": at, "kind": "side", "title": term, "type": "term", "term": term }),
    ))
}

fn idea(mcp: &mut Mcp, at: &str, title: &str, body: String) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "title": title, "body": body }),
    ))
}

/// The generated tail of one Card, after the marker line.
fn generated_at(home: &TestHome, at: &str) -> String {
    let text: String = home
        .db()
        .query_row("SELECT text FROM cards WHERE location = ?1", [at], |row| {
            row.get(0)
        })
        .expect("stored Card text");
    text
}

#[test]
fn a_move_rewrites_only_the_term_addresses_and_moves_the_uses_section() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "Test2026", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    let source = idea(
        &mut mcp,
        &anchor,
        "使用者",
        format!("均衡 [[均衡@{eq}]] 决定价格。"),
    );
    let second = address(mcp.ok(
        "create_successor",
        json!({ "at": &anchor, "kind": "side", "title": "术语表：二", "type": "term", "body": TABLE }),
    ));
    let destination = format!("{second}|a");

    let moved = mcp.ok(
        "move_card",
        json!({ "at": &eq, "to": destination, "confirm": "move" }),
    );
    assert_eq!(moved["address"], json!(destination));
    assert_eq!(
        mcp.body(&source),
        format!("均衡 [[均衡@{destination}]] 决定价格。"),
        "only the address bytes change; the label and the prose stay"
    );
    assert!(
        mcp.text(&destination)
            .contains(&format!("## Term uses\n\n- [[{source}]] 使用者\n")),
        "the uses section follows the head to its new address"
    );
    assert!(
        mcp.err("get_card", json!({ "at": &eq }))
            .contains("does not exist"),
        "the old address holds no Card"
    );
    let row: (String, Option<String>) = home
        .db()
        .query_row(
            "SELECT target, kind FROM links WHERE source = ?1 AND ord = 0",
            [&source],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the rewritten Term use");
    assert_eq!(row, (destination.clone(), Some("term".to_string())));
    let terms = mcp.ok("get_terms", json!({ "at": &second }));
    assert_eq!(terms["coverage"]["uses"], json!(1));
    assert_eq!(terms["uses"][0]["label"], json!("均衡"));
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

#[test]
fn a_deleted_term_head_leaves_a_labelled_broken_use_in_every_report() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "Test2026", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    let source = idea(
        &mut mcp,
        &anchor,
        "使用者",
        format!("均衡 [[均衡@{eq}]] 决定价格。"),
    );

    mcp.ok("delete_card", json!({ "at": &eq, "confirm": "delete" }));
    let line = "均衡 [[均衡@".to_string() + &eq + "]] 决定价格。";

    // The shell line names the target, the authored label and the reason, then the source line.
    let lsbk = home
        .cmd()
        .arg("lsbk")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let lsbk = String::from_utf8(lsbk).expect("utf8 lsbk");
    assert!(
        lsbk.contains(&format!(
            "{source} 使用者 -> {eq} (term `均衡`, missing): {line}"
        )),
        "{lsbk}"
    );

    // The MCP report carries the same source/target/label facts.
    let broken = mcp.ok("list_broken_links", json!({}));
    assert_eq!(
        broken["broken"],
        json!([{
            "source": source,
            "title": "使用者",
            "workspace": "agent",
            "target": eq,
            "label": "均衡",
            "reason": "missing",
            "line": line,
        }])
    );

    // The GUI reports the same facts, so its own broken list can name the word.
    let gui = home.gui();
    assert_eq!(
        gui.get("/api/broken")["broken"],
        json!([{
            "source": source,
            "title": "使用者",
            "workspace": "agent",
            "target": eq,
            "label": "均衡",
            "reason": "missing",
            "line": line,
        }])
    );

    // The Term structure names the use as the missing target it now is.
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    let target = terms["faults"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fault| fault["fault"] == "term_link_target")
        .cloned()
        .expect("the Term-target fault");
    assert_eq!(target["source"], json!(source));
    assert_eq!(target["target"], json!(eq));
    assert_eq!(target["label"], json!("均衡"));
    assert_eq!(target["reason"], json!("missing"));

    // Removing the use clears the Broken report.
    mcp.ok(
        "edit_card",
        json!({ "at": &source, "body": "均衡决定价格。" }),
    );
    assert_eq!(mcp.ok("list_broken_links", json!({}))["broken"], json!([]));
    home.cmd().arg("lsbk").assert().success().stdout("");
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

#[test]
fn term_uses_get_their_own_generated_section_beside_ordinary_reverse_links() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "Test2026", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    // One source writes both forms to the same head; a second writes only a Term use.
    let ordinary = idea(
        &mut mcp,
        &anchor,
        "普通使用者",
        format!("见 [[{eq}]] 与 [[均衡@{eq}]]。"),
    );
    let term_only = address(mcp.ok(
        "create_successor",
        json!({
            "at": &anchor,
            "kind": "side",
            "title": "术语使用者",
            "body": format!("均衡 [[均衡@{eq}]] 决定价格。"),
        }),
    ));

    assert_eq!(
        mcp.text(&eq),
        format!(
            "# 均衡\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[{ordinary}]] 普通使用者\n\n## Term uses\n\n- [[{term_only}]] 术语使用者\n- [[{ordinary}]] 普通使用者\n"
        ),
        "the two sections are separate and each deduplicates its own sources"
    );
    // The authored bodies keep their macros byte for byte.
    assert_eq!(
        mcp.body(&ordinary),
        format!("见 [[{eq}]] 与 [[均衡@{eq}]]。")
    );
    assert_eq!(
        mcp.body(&term_only),
        format!("均衡 [[均衡@{eq}]] 决定价格。")
    );

    // The generated Term uses section is not an authored Link: the head indexes no occurrence.
    let head_rows: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM links WHERE source = ?1",
            [&eq],
            |row| row.get(0),
        )
        .expect("head Link rows");
    assert_eq!(head_rows, 0, "generated macros are never reindexed");

    // S11-AC01: the existing index stores the occurrence kind only; no label column or table.
    let columns: Vec<String> = {
        let db = home.db();
        let mut statement = db
            .prepare("PRAGMA table_info(links)")
            .expect("links schema");
        statement
            .query_map([], |row| row.get::<_, String>(1))
            .expect("schema rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("schema names")
    };
    assert_eq!(columns, ["source", "ord", "target", "kind"]);
    let label_tables: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name LIKE '%label%'",
            [],
            |row| row.get(0),
        )
        .expect("label tables");
    assert_eq!(label_tables, 0, "no second label table exists");

    // The GUI reads the same separation: Term uses are their own list, ordinary Reverse links are
    // another.
    assert!(
        mcp.text(&eq).contains("## Term uses"),
        "the generated section exists"
    );
    let card = home.gui().get(&format!("/api/card?at={}", enc(&eq)));
    assert_eq!(
        card["reverse_links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["address"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [ordinary.as_str()],
        "only the ordinary source is a Reverse link"
    );
    assert_eq!(
        card["term_uses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| (
                row["address"].as_str().unwrap(),
                row["label"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        [(term_only.as_str(), "均衡"), (ordinary.as_str(), "均衡")],
        "Term uses are separate and carry the authored label"
    );

    // The index and the full lifecycle agree, and an explicit rebuild changes no Card.
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
    let before = home.revision();
    home.cmd().arg("reindex").assert().success();
    assert_eq!(home.revision(), before, "a clean rebuild writes nothing");
    assert_eq!(mcp.text(&eq), generated_at(&home, &eq));
    assert!(
        mcp.text(&eq).contains(&format!(
            "## Term uses\n\n- [[{term_only}]] 术语使用者\n- [[{ordinary}]] 普通使用者\n"
        )),
        "the Term uses section survives a rebuild"
    );
    let _ = list;
}

/// R0266/S11-AC06: a head retyped away from `term` makes an existing Term use a semantic fault
/// even though the row still exists, and the fault follows the label into every report. The old
/// use may stay under the legacy retention rule, while a new use of that target is refused.
#[test]
fn a_retyped_head_is_a_non_head_fault_in_every_report() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "Test2026", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    let source = idea(
        &mut mcp,
        &anchor,
        "使用者",
        format!("均衡 [[均衡@{eq}]] 决定价格。"),
    );

    // Retyping clears the name in the same statement and leaves the row at its address.
    mcp.ok("edit_card", json!({ "at": &eq, "type": "idea" }));
    assert_eq!(
        mcp.ok("get_card", json!({ "at": &eq }))["type"],
        json!("idea")
    );
    assert_eq!(
        mcp.ok("get_card", json!({ "at": &eq }))["term"],
        json!(null)
    );

    let line = format!("均衡 [[均衡@{eq}]] 决定价格。");
    let broken = mcp.ok("list_broken_links", json!({}));
    assert_eq!(
        broken["broken"],
        json!([{
            "source": source,
            "title": "使用者",
            "workspace": "agent",
            "target": eq,
            "label": "均衡",
            "reason": "not_head",
            "line": line,
        }])
    );
    let lsbk = String::from_utf8(
        home.cmd()
            .arg("lsbk")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .expect("utf8 lsbk");
    assert!(
        lsbk.contains(&format!(
            "{source} 使用者 -> {eq} (term `均衡`, not a Term head):"
        )),
        "{lsbk}"
    );
    assert_eq!(
        home.gui().get("/api/broken")["broken"][0]["reason"],
        json!("not_head")
    );
    // An ordinary Link could still reach the row, so this is a Term-target fault, not a Broken
    // ordinary Link: `get_terms` names the same reason.
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    let target = terms["faults"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fault| fault["fault"] == "term_link_target")
        .cloned()
        .expect("the Term-target fault");
    assert_eq!(target["reason"], json!("not_head"));
    assert_eq!(target["label"], json!("均衡"));

    // A new Term use of the retyped row is refused; the old one is retained by the legacy rule.
    let other = address(mcp.ok(
        "create_successor",
        json!({ "at": &anchor, "kind": "side", "title": "另一个使用者", "body": "正文。" }),
    ));
    let refusal = mcp.err(
        "edit_card",
        json!({ "at": &other, "body": format!("均衡 [[均衡@{eq}]] 决定价格。") }),
    );
    assert!(refusal.contains("is not a Term head"), "{refusal}");
    mcp.ok(
        "edit_card",
        json!({ "at": &source, "body": format!("新文 [[均衡@{eq}]] 决定价格。") }),
    );
    assert_eq!(
        mcp.ok("list_broken_links", json!({}))["broken"][0]["reason"],
        json!("not_head"),
        "the retained invalid use stays reported"
    );
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

/// S11-AC04: a Citation-key rename is a plan over every address in the tree, so it rewrites a
/// Term use's target bytes too, keeps its label, and the uses section follows the renamed head.
#[test]
fn a_citation_key_rename_rewrites_a_term_use_and_keeps_its_label() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (_anchor, list) = book_with_table(&home, &mut mcp, "OldKey", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    let topic = address(mcp.ok("create_topic", json!({ "title": "引用" })));
    let source = idea(
        &mut mcp,
        &topic,
        "使用者",
        format!("均衡 [[均衡@{eq}]] 决定价格。"),
    );

    mcp.ok(
        "edit_literature_metadata",
        json!({
            "at": "OldKey",
            "bibtex": "@book{NewKey, title={Test Book}}",
            "confirm": "move",
        }),
    );
    let renamed = eq.replacen("OldKey", "NewKey", 1);
    assert_eq!(
        mcp.body(&source),
        format!("均衡 [[均衡@{renamed}]] 决定价格。"),
        "the term address is rewritten, the label and prose stay"
    );
    assert!(
        mcp.text(&renamed)
            .contains(&format!("## Term uses\n\n- [[{source}]] 使用者\n")),
        "the uses section follows to the renamed address"
    );
    assert_eq!(mcp.ok("list_broken_links", json!({}))["broken"], json!([]));
    let terms = mcp.ok("get_terms", json!({ "at": "NewKey" }));
    assert_eq!(terms["coverage"]["uses"], json!(1));
    assert_eq!(terms["uses"][0]["label"], json!("均衡"));
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

/// S11-AC07: an interruption that leaves the authored Term use in place but loses its index row
/// and the generated uses section is repaired by the full lifecycle, and the indexed and
/// full-oracle reports agree again afterwards.
#[test]
fn reindex_repairs_term_index_and_generated_section_drift() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "Test2026", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    let source = idea(
        &mut mcp,
        &anchor,
        "使用者",
        format!("均衡 [[均衡@{eq}]] 决定价格。"),
    );
    let before = mcp.text(&eq);
    assert!(before.contains("## Term uses"), "{before}");

    // The interruption: the authored body and its Card survive, the derived index row and the
    // generated tail do not.
    home.db()
        .execute("DELETE FROM links WHERE kind = 'term'", [])
        .expect("drop the Term index row");
    home.db()
        .execute(
            "UPDATE cards SET text = ?1 WHERE location = ?2",
            rusqlite::params!["# 均衡\n\n<!-- zt:reverse-links -->\ndamaged\n", &eq],
        )
        .expect("damage the generated tail");
    let failed = home
        .cmd()
        .args(["status", "--check-links"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let faults: Value = serde_json::from_slice(&failed).expect("the check report");
    assert_eq!(faults["ok"], json!(false), "{faults}");
    // The explicit repair rebuilds occurrences and generated tails from bodies.
    let revision = home.revision();
    home.cmd().arg("reindex").assert().success();
    assert!(home.revision() > revision, "a repair advances the revision");
    assert_eq!(
        mcp.text(&eq),
        before,
        "the exact generated tail is restored"
    );
    assert_eq!(
        mcp.body(&source),
        format!("均衡 [[均衡@{eq}]] 决定价格。"),
        "authored bytes never change"
    );
    let row: (String, Option<String>) = home
        .db()
        .query_row(
            "SELECT target, kind FROM links WHERE source = ?1 AND ord = 0",
            [&source],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the restored Term index row");
    assert_eq!(row, (eq.clone(), Some("term".to_string())));
    assert_eq!(mcp.ok("list_broken_links", json!({}))["broken"], json!([]));
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    assert_eq!(terms["coverage"]["uses"], json!(1));
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

/// S11-AC04/R0268: a move whose rewrite would touch a human Card is refused to an agent exactly
/// as for an ordinary Link; the owner's own move rewrites the human Card's Term use.
#[test]
fn an_agent_move_that_would_rewrite_a_human_term_use_is_refused() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "Test2026", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    let second = address(mcp.ok(
        "create_successor",
        json!({ "at": &anchor, "kind": "side", "title": "术语表：二", "type": "term", "body": TABLE }),
    ));
    let destination = format!("{second}|a");

    // A human Topic and Card, written by the owner's own shell tools.
    let out = home
        .cmd_with_editor("# 人类主题\n\n正文。\n")
        .args(["t", "人类主题"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let topic = String::from_utf8(out).expect("utf8").trim().to_string();
    let human_body = format!("均衡 [[均衡@{eq}]] 决定价格。");
    let out = home
        .cmd_with_editor(&format!("# 人类使用者\n\n{human_body}\n"))
        .args(["n", "--at", &topic])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let human = String::from_utf8(out).expect("utf8").trim().to_string();

    let refusal = mcp.err(
        "move_card",
        json!({ "at": &eq, "to": destination, "confirm": "move" }),
    );
    assert!(refusal.contains("human workspace"), "{refusal}");
    assert!(
        mcp.ok("get_card", json!({ "at": &eq }))["body"]
            .as_str()
            .is_some(),
        "the refused move wrote nothing"
    );

    // The owner may make the same move; the human Card's target is rewritten and its label stays.
    home.cmd()
        .args(["mv", "--at", &eq, &destination])
        .write_stdin("move\n")
        .assert()
        .success();
    assert_eq!(
        mcp.body(&human),
        format!("均衡 [[均衡@{destination}]] 决定价格。")
    );
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

/// S11-AC07: the indexed report (`list_broken_links`) and the full-lifecycle report (`lsbk`) agree
/// on source order, occurrence count, target, label and reason for a mixed ordinary/Term fixture.
#[test]
fn indexed_and_full_lifecycle_broken_reports_agree() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "Test2026", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    // A normal target first, so the ordinary Link is valid when written; both targets are then
    // deleted, leaving the stored macros Broken.
    let gone = address(mcp.ok(
        "create_successor",
        json!({ "at": &anchor, "title": "将删除", "body": "正文。" }),
    ));
    let first = address(mcp.ok(
        "create_successor",
        json!({
            "at": &anchor,
            "kind": "side",
            "title": "甲",
            "body": format!("旧 [[{gone}]] 与 [[均衡@{eq}]]。"),
        }),
    ));
    let second = address(mcp.ok(
        "create_successor",
        json!({
            "at": &anchor,
            "kind": "side",
            "title": "乙",
            "body": format!("[[均衡@{eq}]] 与 [[均衡@{eq}]]。"),
        }),
    ));
    mcp.ok("delete_card", json!({ "at": &eq, "confirm": "delete" }));
    mcp.ok("delete_card", json!({ "at": &gone, "confirm": "delete" }));

    let broken = mcp.ok("list_broken_links", json!({}));
    let rows = broken["broken"].as_array().expect("broken rows");
    let facts = rows
        .iter()
        .map(|row| {
            (
                row["source"].as_str().unwrap().to_string(),
                row["target"].as_str().unwrap().to_string(),
                row["label"].clone(),
                row["reason"].as_str().unwrap().to_string(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(facts.len(), 4, "{broken}");
    assert_eq!(facts[0].0, first);
    assert_eq!(facts[0].1, gone);
    assert_eq!(facts[0].2, json!(null));
    assert_eq!(facts[0].3, "missing");
    assert_eq!(facts[1].0, first);
    assert_eq!(facts[1].2, json!("均衡"));
    assert_eq!(facts[1].3, "missing");
    assert_eq!(
        facts[2..],
        [
            (
                second.clone(),
                eq.clone(),
                json!("均衡"),
                "missing".to_string()
            ),
            (
                second.clone(),
                eq.clone(),
                json!("均衡"),
                "missing".to_string()
            ),
        ],
        "each repeated occurrence stays its own row, in body order"
    );

    let lsbk = String::from_utf8(
        home.cmd()
            .arg("lsbk")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .expect("utf8 lsbk");
    let lines = lsbk.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 4, "{lsbk}");
    assert!(
        lines[0].contains(&format!("{first} 甲 -> {gone}")),
        "{lsbk}"
    );
    assert!(
        lines[1].contains(&format!("{first} 甲 -> {eq} (term `均衡`, missing)")),
        "{lsbk}"
    );
    assert!(
        lines[2].contains(&format!("{second} 乙 -> {eq} (term `均衡`, missing)"))
            && lines[3].contains(&format!("{second} 乙 -> {eq} (term `均衡`, missing)")),
        "{lsbk}"
    );
}

/// S11-AC04: side compaction is a rename plan too. Deleting an earlier side successor renames a
/// later Term list and its head; a Term use of that head is rewritten to the compacted address
/// with its label, and the regenerated Term uses section follows.
#[test]
fn side_compaction_rewrites_a_term_use_and_keeps_its_label() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "Test2026", TABLE);
    let first_head = head(&mut mcp, &list, "均衡");
    let second = address(mcp.ok(
        "create_successor",
        json!({ "at": &anchor, "kind": "side", "title": "术语表：二", "type": "term", "body": TABLE }),
    ));
    let second_head = head(&mut mcp, &second, "均衡");
    let source = address(mcp.ok(
        "create_successor",
        json!({
            "at": &anchor,
            "kind": "side",
            "title": "使用者",
            "body": format!("均衡 [[均衡@{second_head}]] 决定价格。"),
        }),
    ));
    // Side successors `a` (the first list), `b` (the second list) and `c` (the source).
    assert_eq!(list, "Test2026/1|a");
    assert_eq!(second, "Test2026/1|b");
    assert_eq!(second_head, "Test2026/1|b|a");
    assert_eq!(source, "Test2026/1|c");
    let _ = first_head;

    mcp.ok("delete_card", json!({ "at": &list, "confirm": "delete" }));

    // `b` -> `a`, its head `b|a` -> `a|a`, and the source `c` -> `b`: the deleted addresses are
    // reused by the compacted Cards.
    assert_eq!(
        mcp.ok("get_card", json!({ "at": &list }))["title"],
        json!("术语表：二"),
        "the later Term list takes the deleted side place"
    );
    assert_eq!(
        mcp.ok("get_card", json!({ "at": &second }))["title"],
        json!("使用者"),
        "the source takes the next compacted side place"
    );
    let compacted_head = format!("{list}|a");
    assert_eq!(
        mcp.ok("get_card", json!({ "at": &compacted_head }))["title"],
        json!("均衡")
    );
    assert_eq!(
        mcp.body(&second),
        format!("均衡 [[均衡@{compacted_head}]] 决定价格。"),
        "the compaction rewrites the target bytes and keeps the label"
    );
    assert!(
        mcp.text(&compacted_head).contains(&format!("## Term uses\n\n- [[{second}]] 使用者\n")),
        "the uses section follows the compacted head"
    );
    assert_eq!(mcp.ok("list_broken_links", json!({}))["broken"], json!([]));
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}
