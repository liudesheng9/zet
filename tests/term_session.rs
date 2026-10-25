//! Following label-only Term links in the terminal Session (S15, #157; design 15.3).
//!
//! A read view shows a Term use as its authored label with the address hidden; the label's
//! display width decides wrapping, hit ranges and copying; wide Han characters and combining
//! marks keep their columns; raw editing keeps every macro; an accepted write's warnings stay on
//! the Session's status row, and a refused write keeps the edit buffer.
//!
//! The archive is a scratch one (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`); the live archive is never
//! touched. Only the public Session (line and PTY) and MCP surfaces are asserted.

#![allow(dead_code)]

mod support;

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command as StdCommand, Stdio};
use tempfile::TempDir;

use support::pty::{Key, PtySession};

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
                home.archive_root.to_str().expect("archive root"),
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
        let mut child = StdCommand::new(cargo_bin("zt"))
            .env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0")
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn zt mcp");
        let stdin = child.stdin.take().expect("MCP stdin");
        let stdout = BufReader::new(child.stdout.take().expect("MCP stdout"));
        let mut mcp = Mcp {
            child,
            stdin,
            stdout,
            next_id: 0,
        };
        mcp.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "term-session-test", "version": "0" },
            }),
        );
        mcp
    }

    fn card_text(&self, location: &str) -> String {
        let conn =
            rusqlite::Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row(
            "SELECT text FROM cards WHERE location = ?1",
            [location],
            |row| row.get(0),
        )
        .expect("card text")
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.archive_root.join("zt.pid"));
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

    fn ok(&mut self, name: &str, arguments: Value) -> Value {
        let reply = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let result = &reply["result"];
        assert!(
            !result["isError"].as_bool().expect("isError flag"),
            "{name}({arguments}) failed: {result}"
        );
        result["structuredContent"].clone()
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

/// A two-row term table: `均衡` is published as a head, `弹性` is still missing.
const TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n| 弹性 | 弹性系数 | 价格变化的反应程度 | §2.3 |\n";

struct Fixture {
    /// The Idea Card whose prose holds the Term use.
    note: String,
    /// The Term head the use names.
    head: String,
}

/// One agent Literature tree: an anchor with a Term list under `<anchor>|a`, the `均衡` head
/// under the list, and one Idea note that holds the Term use the test supplies.
fn term_fixture(mcp: &mut Mcp, note_body: impl Fn(&str) -> String) -> Fixture {
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{TestBook, title={Test Book}}" }),
    );
    let anchor = address(mcp.ok(
        "create_successor",
        json!({ "at": "TestBook", "title": "第一章", "body": "设置。" }),
    ));
    let list = address(mcp.ok(
        "create_successor",
        json!({
            "at": anchor,
            "kind": "side",
            "title": "术语表：测试",
            "type": "term",
            "body": TABLE,
        }),
    ));
    let head = address(mcp.ok(
        "create_successor",
        json!({
            "at": list,
            "kind": "side",
            "title": "均衡",
            "type": "term",
            "term": "均衡",
            "body": "## 释义\n\n两个参与者都不愿单方面改变策略。\n",
        }),
    ));
    let note = address(mcp.ok(
        "create_successor",
        json!({ "at": anchor, "title": "笔记", "body": note_body(&head) }),
    ));
    Fixture { note, head }
}

fn wait_for_clipboard(path: &Path) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !path.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    fs::read_to_string(path).expect("fake clipboard output")
}

/// S15-AC01: the line Session reads a Term use as its label and never shows the hidden address.
#[test]
fn the_line_session_reads_a_term_use_as_its_label() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let fixture = term_fixture(&mut mcp, |head| format!("需求是 [[均衡@{head}]] 的。"));

    let output = home
        .cmd()
        .write_stdin(format!("go {}\nq\n", fixture.note))
        .output()
        .expect("line Session");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("session output is UTF-8");

    assert!(stdout.contains("需求是 均衡 的。"), "{stdout}");
    assert!(
        !stdout.contains("[[均衡@"),
        "the Term link's address stays hidden: {stdout}"
    );
}

/// S15-AC01/S15-AC02: the TUI shows the label, and a left click on it follows the recorded
/// target instead of the hidden macro bytes.
#[test]
fn clicking_a_term_label_follows_its_head() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let fixture = term_fixture(&mut mcp, |head| format!("需求是 [[均衡@{head}]] 的。"));

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text(&format!("go {}", fixture.note));
    session.send_enter();
    session.wait_for_text("需求是 均衡 的。");
    assert!(
        !session.plain_output().contains("[[均衡@"),
        "the address stays hidden:\n{}",
        session.plain_output().escape_debug()
    );

    session.send_left_click(3, 8);
    session.wait_for_text(&format!("location: {}", fixture.head));

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

/// S15-AC02/S15-AC03: the label decides the wrap, and copying a dragged selection takes the
/// visible text exactly, with a combining mark kept on its base and Han characters in their own
/// cells.
#[test]
fn copying_a_wrapped_term_line_takes_the_visible_text() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let fixture = term_fixture(&mut mcp, |head| format!("e\u{301}x [[均衡@{head}]] 中"));

    let clipboard = home.config_dir.join("term-clipboard.txt");
    let mut session = PtySession::spawn_with_test_clipboard_output_at_size(
        &cargo_bin("zt"),
        &home.config_dir,
        "seed",
        &clipboard,
        24,
        20,
    );
    session.wait_for_text("ROOT");
    session.send_text(&format!("go {}", fixture.note));
    session.send_enter();
    session.wait_for_text("e\u{301}x 均衡 中");
    assert!(
        !session.plain_output().contains("[[均衡@"),
        "the hidden address never widens the line:\n{}",
        session.plain_output().escape_debug()
    );

    session.send_left_drag(3, 0, 3, 19);
    session.wait_for_raw_text("\x1b[7m");
    session.send_key(Key::CtrlShiftC);
    assert_eq!(wait_for_clipboard(&clipboard), "e\u{301}x 均衡 中");

    session.send_left_click(3, 5);
    session.wait_for_text(&format!("location: {}", fixture.head));

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

/// S15-AC04: edit mode shows the complete stored macro, and saving keeps it byte for byte.
#[test]
fn raw_editing_keeps_the_complete_term_macro() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let fixture = term_fixture(&mut mcp, |head| format!("需求是 [[均衡@{head}]] 的。"));
    let stored = home.card_text(&fixture.note);

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text(&format!("go {}", fixture.note));
    session.send_enter();
    session.wait_for_text("需求是 均衡 的。");
    session.send_text("e");
    session.send_enter();
    session.wait_for_text("edit mode");
    session.wait_for_text(&format!("[[均衡@{}]]", fixture.head));

    session.send_key(Key::CtrlS);
    session.wait_for_text("需求是 均衡 的。");
    assert_eq!(home.card_text(&fixture.note), stored);

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

/// S15-AC05: an accepted write's warnings are presented on the Session's status row, in its
/// existing one-line convention.
#[test]
fn accepted_write_warnings_stay_on_the_session_status_row() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let fixture = term_fixture(&mut mcp, |head| format!("需求是 [[均衡@{head}]] 的。"));

    let mut session = PtySession::spawn_with_test_clipboard_at_size(
        &cargo_bin("zt"),
        &home.config_dir,
        "seed",
        24,
        300,
    );
    session.wait_for_text("ROOT");
    session.send_text(&format!("go {}", fixture.note));
    session.send_enter();
    session.wait_for_text("需求是 均衡 的。");
    session.send_text("e");
    session.send_enter();
    session.wait_for_text("edit mode");

    for _ in 0..2 {
        session.send_key(Key::Down);
    }
    session.send_key(Key::End);
    session.send_text("；乙。");
    session.send_key(Key::CtrlS);
    session.wait_for_text(&format!("check: {}", fixture.note));
    assert!(
        session.plain_output().contains("R3"),
        "the refused semicolon warning is shown:\n{}",
        session.plain_output().escape_debug()
    );

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

/// S15-AC05: a write the mandatory Link rule refuses keeps the edit buffer, so the author can
/// fix the draft instead of losing it.
#[test]
fn a_refused_term_write_keeps_the_edit_buffer_and_writes_nothing() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let fixture = term_fixture(&mut mcp, |head| format!("需求是 [[均衡@{head}]] 的。"));
    let stored = home.card_text(&fixture.note);
    let missing = "[[缺失@TestBook/1|a|z]]";

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text(&format!("go {}", fixture.note));
    session.send_enter();
    session.wait_for_text("需求是 均衡 的。");
    session.send_text("e");
    session.send_enter();
    session.wait_for_text("edit mode");

    for _ in 0..2 {
        session.send_key(Key::Down);
    }
    session.send_key(Key::End);
    session.send_text(missing);
    session.send_key(Key::CtrlS);
    session.wait_for_text("Term link target");

    // The same buffer is saved again: the identical refusal proves the draft survived.
    session.send_key(Key::CtrlS);
    session.wait_for_text_count("Term link target", 2);
    session.send_key(Key::Esc);
    session.wait_for_text(&format!("location: {}", fixture.note));
    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
    assert_eq!(home.card_text(&fixture.note), stored);
}
