//! Reading Term links and explanation Cards in the GUI (S13, #156; design 15.2).
//!
//! A Term use in an open Card's prose is its authored word, in the prose's own type, with the
//! list row's short meaning and the target address for hover and focus and the ordinary encoded
//! Card route for click or keyboard. A Term Card is read by the existing reader, with its computed
//! role and list beside the quiet head metadata and its generated use list kept apart. Broken uses
//! keep their word and say why they cannot be followed.
//!
//! The archive is a scratch one (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`); the live archive is never
//! touched. Only the public MCP and GUI surfaces are asserted.

#![allow(dead_code)]

use assert_cmd::Command;
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

/// One agent Literature tree whose anchor holds one Term list under `<key>/1|a`.
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
        json!({
            "at": at,
            "kind": "side",
            "title": term,
            "type": "term",
            "term": term,
            "body": "## 释义\n\n两个参与者都不愿单方面改变策略。\n",
        }),
    ))
}

fn part(mcp: &mut Mcp, at: &str, term: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({
            "at": at,
            "kind": "direct",
            "title": "推导（续 2/2）：极限情形",
            "type": "term",
            "term": term,
        }),
    ))
}

fn idea(mcp: &mut Mcp, at: &str, title: &str, body: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "title": title, "body": body }),
    ))
}

/// S13-AC01/S13-AC02/S13-AC05/R0462-R0466/R0471: in the open Card's prose, a Term use is only its
/// authored label, in the prose font, with the list row's meaning and the target address for
/// hover/focus and the existing encoded Card route; an ordinary reasoned Link keeps its
/// address/title chip. The Term Card itself reads through the existing Card viewer with its
/// computed role and list, while the generated Term-use list stays a separate, countable list.
#[test]
fn the_gui_reads_a_term_use_as_a_quiet_label_and_a_term_head_as_an_explanation_card() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "Test2026", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    let source = idea(
        &mut mcp,
        &anchor,
        "使用者",
        &format!("均衡 [[均衡@{eq}]] 决定价格，也见 [[{eq}]]。"),
    );
    let gui = home.gui();

    let card = gui.get(&format!("/api/card?at={}", enc(&source)));
    let html = card["html"].as_str().expect("rendered html");
    assert!(
        html.contains(&format!(
            "<a class=\"zt-link zt-term\" href=\"#/card/{}\" data-target=\"{eq}\" data-meaning=\"双方都不改变策略\" title=\"双方都不改变策略 · {eq}\">均衡</a>",
            eq.replace('|', "%7C"),
        )),
        "a Term use is its label, its list-row meaning, and its encoded route: {html}"
    );
    assert!(
        html.contains(&format!(
            "<a class=\"zt-link\" href=\"#/card/{}\" data-target=\"{eq}\"><span class=\"zt-link-addr\">{eq}</span><span class=\"zt-link-title\">均衡</span></a>",
            eq.replace('|', "%7C"),
        )),
        "an ordinary reasoned Link keeps its address/title chip: {html}"
    );
    assert_eq!(
        card["term_facts"],
        json!(null),
        "an ordinary Card carries no quiet Term metadata"
    );

    // The Term Card is read by the existing reader: its computed role and list are published for
    // the quiet head metadata, and its generated Term-use list stays its own countable list.
    let head_card = gui.get(&format!("/api/card?at={}", enc(&eq)));
    assert_eq!(head_card["type"], json!("term"));
    assert_eq!(head_card["term"], json!("均衡"));
    assert_eq!(
        head_card["term_facts"],
        json!({
            "role": "card",
            "list": { "address": list, "title": "术语表：测试" },
        })
    );
    assert_eq!(
        head_card["term_uses"]
            .as_array()
            .expect("the generated Term-use list")
            .iter()
            .map(|row| (row["address"].as_str().unwrap(), row["label"].as_str().unwrap()))
            .collect::<Vec<_>>(),
        [(source.as_str(), "均衡")]
    );
    assert!(
        head_card["html"]
            .as_str()
            .unwrap()
            .contains("<h2>释义</h2>"),
        "the explanation Card keeps its ordinary Markdown sections: {}",
        head_card["html"]
    );
    assert!(
        !head_card["html"].as_str().unwrap().contains("Term uses"),
        "the generated Term-use list is not part of the rendered prose"
    );

    // The list itself, a part of a head, and a named Term Card with no valid placement each name
    // their own computed role, so the reader can render the head metadata quietly.
    assert_eq!(
        gui.get(&format!("/api/card?at={}", enc(&list)))["term_facts"],
        json!({
            "role": "list",
            "list": { "address": list, "title": "术语表：测试" },
        })
    );
    let eq_part = part(&mut mcp, &eq, "均衡");
    assert_eq!(
        gui.get(&format!("/api/card?at={}", enc(&eq_part)))["term_facts"],
        json!({
            "role": "part",
            "list": { "address": list, "title": "术语表：测试" },
        })
    );
    let unfiled = head(&mut mcp, &anchor, "孤立");
    assert_eq!(
        gui.get(&format!("/api/card?at={}", enc(&unfiled)))["term_facts"],
        json!({ "role": "unfiled", "list": null })
    );
}

/// S13-AC03/R0469: a Term use whose target is gone, or whose target row is no Term head, keeps its
/// authored word, says why it cannot be followed in text that needs no hover, and offers no route;
/// the Broken-link report names the same reason.
#[test]
fn the_gui_explains_a_broken_term_use_without_hover() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_table(&home, &mut mcp, "Test2026", TABLE);
    let eq = head(&mut mcp, &list, "均衡");
    let source = idea(
        &mut mcp,
        &anchor,
        "使用者",
        &format!("均衡 [[均衡@{eq}]] 决定价格。"),
    );
    let gui = home.gui();

    // A retyped head leaves its row: the use is a semantic fault, not a missing Card.
    mcp.ok("edit_card", json!({ "at": &eq, "type": "idea" }));
    let html = gui.get(&format!("/api/card?at={}", enc(&source)))["html"]
        .as_str()
        .map(str::to_string)
        .expect("rendered html");
    assert!(
        html.contains(&format!(
            "<span class=\"zt-link zt-term broken\" data-target=\"{eq}\" data-reason=\"not_head\" title=\"Broken Term link: {eq} (not a Term head)\">均衡 <span class=\"zt-term-note\">not a Term head</span></span>"
        )),
        "an existing row that is no Term head is explained in words: {html}"
    );
    assert!(!html.contains("#/card/"), "a broken Term use has no route: {html}");
    assert_eq!(
        gui.get("/api/broken")["broken"][0]["reason"],
        json!("not_head")
    );

    // The deleted head leaves a missing target, named by the same authored word.
    mcp.ok("delete_card", json!({ "at": &eq, "confirm": "delete" }));
    let html = gui.get(&format!("/api/card?at={}", enc(&source)))["html"]
        .as_str()
        .map(str::to_string)
        .expect("rendered html");
    assert!(
        html.contains(&format!(
            "<span class=\"zt-link zt-term broken\" data-target=\"{eq}\" data-reason=\"missing\" title=\"Broken Term link: {eq} (missing)\">均衡 <span class=\"zt-term-note\">missing</span></span>"
        )),
        "a missing Term target is explained in words: {html}"
    );
    assert_eq!(gui.get("/api/broken")["broken"][0]["reason"], json!("missing"));
}
