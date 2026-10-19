//! The one explained distant Card (S19, #108, design §10.2): `/api/distant`.
//!
//! The slot's subject is the Card the reader has open or holds. The service answers zero or one
//! Card of another book that shares an accepted register term (or a model name) with it, with one
//! line of evidence, and it changes nothing: opening, laying, or replying writes no Link and
//! records no viewing telemetry. See `src/entry.rs::distant` and the GUI block doc.

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
    /// Environment the service daemon starts with; a test names an exact helper here.
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
            env: env.iter().map(|(name, value)| (name.to_string(), value.to_string())).collect(),
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
}

struct Gui {
    port: u16,
    token: String,
}

impl Gui {
    /// One authenticated API call with the given token; the JSON body of the answer and its status.
    fn get_with(&self, path: &str, token: &str) -> (u16, Value) {
        use std::net::TcpStream;
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let port = self.port;
        let request = format!(
            "GET {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: {token}\r\nContent-Length: 0\r\n\r\n"
        );
        stream.write_all(request.as_bytes()).expect("send request");
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

    fn get(&self, path: &str) -> Value {
        let (status, body) = self.get_with(path, &self.token);
        assert_eq!(status, 200, "{path} answered {status}: {body}");
        body
    }

    /// One authenticated write with a JSON body: the Spread document a lay-on-Table saves.
    fn post(&self, path: &str, body: &Value) -> Value {
        use std::net::TcpStream;
        let payload = serde_json::to_string(body).expect("JSON body");
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let port = self.port;
        let request = format!(
            "POST {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            self.token,
            payload.len()
        );
        stream.write_all(request.as_bytes()).expect("send request");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        let start = response.find("\r\n\r\n").expect("header end") + 4;
        serde_json::from_str(&response[start..]).expect("JSON response")
    }

    /// The distant slot of one Card, optionally with the Cards lying on the Table.
    fn distant(&self, at: &str, table: &[&str]) -> Value {
        let mut path = format!("/api/distant?at={}", enc(at));
        if !table.is_empty() {
            let list = table.iter().map(|at| enc(at)).collect::<Vec<_>>().join(",");
            path.push_str(&format!("&table={list}"));
        }
        self.get(&path)
    }

    /// The distant slot with one selected Bridge report: the first read starts the optional
    /// helper, so the answer may still be pending.
    fn distant_report(&self, at: &str, table: &[&str], report: &std::path::Path) -> Value {
        let mut path = format!(
            "/api/distant?at={}&report={}",
            enc(at),
            report.display().to_string().replace('/', "%2F")
        );
        if !table.is_empty() {
            let list = table.iter().map(|at| enc(at)).collect::<Vec<_>>().join(",");
            path.push_str(&format!("&table={list}"));
        }
        self.get(&path)
    }

    /// The same read, waiting for the helper's answer instead of its pending status.
    fn distant_report_ready(&self, at: &str, table: &[&str], report: &std::path::Path) -> Value {
        for _ in 0..400 {
            let value = self.distant_report(at, table, report);
            if value["report"]["status"] != json!("pending") {
                return value;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("the Bridge report projection never left pending");
    }

    /// The served page's script and style, for the front end's own wiring.
    fn asset(&self, path: &str) -> String {
        use std::net::TcpStream;
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            self.port
        )
        .expect("request asset");
        let mut page = String::new();
        stream.read_to_string(&mut page).expect("read asset");
        page
    }

    fn register(&self) -> Value {
        self.get("/api/register?ws=agent")
    }
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

fn enc(value: &str) -> String {
    value.replace('/', "%2F").replace('|', "%7C")
}

fn sha256_hex(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The `zt-digest` this checkout carries: `ZT_DIGEST_HELPER` when set, else the installed binary
/// on `PATH` (the one the GUI runs in production), else the crate's own debug or release build.
fn zt_digest_helper() -> Option<std::path::PathBuf> {
    if let Some(value) = std::env::var_os("ZT_DIGEST_HELPER").filter(|value| !value.is_empty()) {
        return Some(std::path::PathBuf::from(value));
    }
    let name = if cfg!(windows) { "zt-digest.exe" } else { "zt-digest" };
    let installed = std::process::Command::new(name)
        .arg("--help")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()
        .map(|_| std::path::PathBuf::from(name));
    if installed.is_some() {
        return installed;
    }
    let built = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools")
        .join("zt-digest")
        .join("target");
    ["debug", "release"]
        .iter()
        .map(|profile| built.join(profile).join(name))
        .find(|path| path.is_file())
}

/// The endpoint snapshot the Bridge compiler freezes: its compared fields, the inferred book,
/// and the fingerprint over its ten fields. `get_card` answers the fields it reads.
fn bridge_endpoint(card: &Value) -> Value {
    let text = |name: &str| {
        card[name]
            .as_str()
            .unwrap_or_default()
            .replace("\r\n", "\n")
            .replace('\r', "\n")
    };
    let mut snapshot = json!({
        "address": card["address"],
        "workspace": card["workspace"],
        "tree": card["tree"],
        "kind": card["kind"],
        "type": card["type"],
        "facet": card["facet"],
        "emphasis": card["emphasis"],
        "level": card["level"],
        "form": card["form"],
        "title": text("title"),
        "body": text("body"),
        "category": "idea",
        "book": card["tree"],
    });
    let values: Vec<Value> = ["workspace", "tree", "kind", "type", "facet", "emphasis", "level", "form", "title", "body"]
        .iter()
        .map(|name| snapshot[*name].clone())
        .collect();
    snapshot["fingerprint"] = json!(format!(
        "sha256:{}",
        sha256_hex(&serde_json::to_string(&values).expect("compact JSON"))
    ));
    snapshot
}

/// One immutable published Bridge report for two Cards, identified exactly as the compiler
/// identifies it: `r-` and the content hash without `report_id` and `created_at`.
fn published_bridge_report(focus: &Value, other: &Value, id: &str, kind: &str) -> Value {
    let focus_at = focus["address"].as_str().expect("focus address").to_string();
    let other_at = other["address"].as_str().expect("target address").to_string();
    let (focus_snapshot, other_snapshot) = (bridge_endpoint(focus), bridge_endpoint(other));
    let sentence = format!("对应 [[{other_at}]]：两处讲同一机制。");
    let mut cards = serde_json::Map::new();
    cards.insert(focus_at.clone(), focus_snapshot.clone());
    cards.insert(other_at.clone(), other_snapshot.clone());
    let mut report = json!({
        "schema": 1,
        "plan_id": "p-distant-report",
        "citation_key": focus["tree"],
        "book_title": "Book A",
        "profile": "book",
        "options": { "max_sources": 1, "candidates": 1, "books": [] },
        "read_coverage": { "complete": true },
        "empty_reason": "",
        "cards": Value::Object(cards),
        "term_tables": {},
        "plan_sources": [],
        "proposals": [{
            "source": focus_at,
            "carrier": focus_at,
            "target": other_at,
            "word": "对应",
            "sentence": sentence,
            "why": "两张卡讨论同一机制。",
            "section": "new",
            "kind": kind,
            "id": id,
            "carrier_fingerprint": focus_snapshot["fingerprint"],
            "target_fingerprint": other_snapshot["fingerprint"],
        }],
        "sources": [],
        "omitted_sources": [],
        "refused": [],
        "counts": { "proposals": 1 },
        "status": "done",
    });
    let mut content = report.as_object().expect("report").clone();
    content.remove("report_id");
    content.remove("created_at");
    let report_id = format!(
        "r-{}",
        sha256_hex(&serde_json::to_string(&Value::Object(content)).expect("canonical JSON"))
    );
    report["report_id"] = json!(report_id);
    report["created_at"] = json!("2026-10-19T00:00:00.0000000Z");
    report
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

/// The Cards of the fixture: two books about one term, and the register that accepts it.
fn two_books(home: &TestHome) -> (Gui, String, String) {
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
        "多重均衡的另一种说法",
        "这里也从策略互补说起。\n\n来源：[[bookB]]",
    );
    register(&mut mcp, "## 词条\n- 多重均衡：[[bookB/1]]");
    let gui = home.gui();
    (gui, focus, other)
}

#[test]
fn one_explained_distant_card_serves_the_reader_and_the_table_without_a_write() {
    let home = TestHome::new();
    let (gui, focus, other) = two_books(&home);
    assert_eq!(focus, "bookA/1");
    assert_eq!(other, "bookB/1");

    // The register revision, the graph, and the Spreads before the suggestion is read.
    let before = json!({
        "graph": gui.get("/api/graph"),
        "overview": gui.get("/api/overview"),
        "spreads": gui.get("/api/spreads"),
        "register": gui.register(),
        "broken": gui.get("/api/broken"),
    });
    let value = gui.distant(&focus, &[]);
    let candidate = &value["candidate"];
    assert_eq!(candidate["address"], json!("bookB/1"), "{value}");
    assert_eq!(candidate["title"], json!("多重均衡的另一种说法"));
    assert_eq!(candidate["source"], json!("register_term"));
    assert_eq!(candidate["term"], json!("多重均衡"));
    assert!(candidate["origin"]["card"].is_string());
    assert!(candidate["authored_fingerprint"].is_string());
    assert_eq!(candidate["evidence"]["sources"], json!(["bookB"]));
    assert_eq!(candidate["evidence"]["added"], json!(["bookB"]));
    let reason = candidate["reason"].as_str().expect("a one-line reason");
    assert!(
        reason.starts_with("Shares register term 多重均衡; source bookB"),
        "{reason}"
    );
    assert_eq!(value["focus"]["sources"], json!(["bookA"]));
    assert_eq!(value["at"], json!("bookA/1"));
    assert_eq!(value["title"], json!("多重均衡的陈述"));
    assert!(value["archive_fingerprint"].is_string());

    // The same read gives the same Card: the slot never rotates on refresh.
    assert_eq!(gui.distant(&focus, &[]), value);
    // A Card already on the Table is no distant Card.
    let lying = gui.distant(&focus, &[&other]);
    assert_eq!(lying["candidate"], Value::Null, "{lying}");
    assert_eq!(lying["excluded"]["table"], json!(1));
    // … and it comes back when the Table no longer holds it.
    assert_eq!(gui.distant(&focus, &[]), value);

    // Reading, opening, or laying the suggestion writes no Card, Link, or metadata row.
    let after = json!({
        "graph": gui.get("/api/graph"),
        "overview": gui.get("/api/overview"),
        "spreads": gui.get("/api/spreads"),
        "register": gui.register(),
        "broken": gui.get("/api/broken"),
    });
    assert_eq!(after, before, "the distant selection changed the archive");
    assert_eq!(
        after["register"]["archive_revision"],
        before["register"]["archive_revision"],
        "no write bumped the archive revision"
    );

    // Laying the suggestion on the Table saves the Spread and nothing else: no Link, no accepted
    // relation, no telemetry row. The Table then excludes it, exactly as the slot promised.
    let spread = json!({
        "current": "distant-slot",
        "spreads": [{ "id": "distant-slot", "name": "Distant", "cards": [{ "at": other, "x": 10, "y": 10 }] }],
    });
    let kept = gui.post("/api/spreads", &spread);
    assert_eq!(kept["spreads"][0]["cards"][0]["at"], json!(other));
    assert_eq!(
        gui.get("/api/graph"),
        before["graph"],
        "laying the suggestion wrote a Card or a Link"
    );
    assert_eq!(
        gui.register()["archive_revision"],
        before["register"]["archive_revision"],
        "laying the suggestion bumped the archive revision"
    );
    assert_eq!(gui.get("/api/broken"), before["broken"]);
    let lying = gui.distant(&focus, &[&other]);
    assert_eq!(lying["candidate"], Value::Null, "{lying}");

    // The reader keeps its ordinary contract: a token, a named Card, and an honest error.
    let (status, body) = gui.get_with("/api/distant?at=bookA%2F1", "not-the-token");
    assert_eq!(status, 403, "{body}");
    let (status, body) = gui.get_with("/api/distant", &gui.token);
    assert_eq!(status, 400, "{body}");
    assert!(
        body["error"].as_str().unwrap_or_default().contains("at"),
        "{body}"
    );
    let (status, body) = gui.get_with("/api/distant?at=bookZ%2F9", &gui.token);
    assert_eq!(status, 400, "a missing Card is an error, not an empty slot: {body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("does not exist"),
        "{body}"
    );

    // The shipped page carries one slot in the reader and one beside the Table's neighbours,
    // and it reads that slot from this one route.
    let app = gui.asset("/app.js");
    assert_eq!(app.matches("api('distant?").count(), 1, "one distant request");
    assert!(
        app.contains("distantSlot('reader')"),
        "the reader has the slot"
    );
    assert!(
        app.contains("distantSlot('table')"),
        "the Table has the slot"
    );
    assert!(
        app.contains("paintDistant();"),
        "a read fills both slots without redrawing the views"
    );
    for action in ["distant-open", "distant-lay", "distant-reply"] {
        assert!(app.contains(action), "the slot offers {action}");
    }
    assert!(
        app.contains("replyTo(candidate)"),
        "Reply answers the distant agent Card through the ordinary draft"
    );
    let css = gui.asset("/app.css");
    assert!(css.contains(".distant {"), "the slot has its own low-salience style");
    assert!(
        css.contains(".distant:hover") && css.contains(".distant:focus-within"),
        "the slot stays faint until focus or hover"
    );

    // Reply from the slot is the ordinary human draft: the candidate's own authored fingerprint is
    // the one `/api/reply` re-checks, so the suggestion can be answered without opening it first.
    let reply = gui.post("/api/reply", &json!({
        "at": other,
        "base": candidate["authored_fingerprint"],
        "text": format!("# 另一个说法\n\n印证 [[{other}]]：同一机制在另一本书里也成立。\n"),
    }));
    assert_eq!(reply["saved"], json!(true), "{reply}");
    assert_eq!(reply["kind"], json!("illustrates"), "{reply}");
    assert_eq!(reply["target"]["address"], json!(other));
    assert!(reply["address"].is_string(), "the reply was filed: {reply}");
}

/// AC07: the cached GUI read answers from a fresh revision, and evidence that is gone leaves the
/// slot empty instead of a stale Card.
#[test]
fn the_cached_gui_read_follows_changed_evidence() {
    let home = TestHome::new();
    let (gui, focus, _) = two_books(&home);
    assert_eq!(
        gui.distant(&focus, &[])["candidate"]["address"],
        json!("bookB/1")
    );
    // The focus stops stating the term: no accepted label carries it any more.
    let mut mcp = home.mcp();
    mcp.ok(
        "edit_card",
        json!({ "at": focus, "title": "无关的陈述", "body": "这张卡不再谈那个术语。" }),
    );
    let value = gui.distant(&focus, &[]);
    assert_eq!(value["candidate"], Value::Null, "{value}");
    assert_eq!(value["focus"]["terms"], json!([]));
    // The focused Card says the term again: the suggestion returns on the next read.
    mcp.ok(
        "edit_card",
        json!({ "at": focus, "body": "多重均衡来自策略互补。" }),
    );
    let value = gui.distant(&focus, &[]);
    assert_eq!(value["candidate"]["address"], json!("bookB/1"), "{value}");
    // The register stops accepting the term: evidence that is invalid leaves the slot empty.
    let register = value["register"]["card"].as_str().expect("register").to_string();
    mcp.ok(
        "edit_card",
        json!({ "at": register, "body": "## 词条\n- 另一个词条：[[bookB/1]]" }),
    );
    let value = gui.distant(&focus, &[]);
    assert_eq!(value["candidate"], Value::Null, "{value}");
    assert_eq!(value["focus"]["terms"], json!([]));
    // The reader is still usable: the Card reads as before.
    let card = gui.get(&format!("/api/card?at={}", enc(&focus)));
    assert_eq!(card["title"], json!("无关的陈述"));
}

/// S20-AC01, AC05, AC06, AC07: one selected Bridge report makes its current proposal the distant
/// slot's first source with the exact report and proposal id; a changed, invalid, or missing
/// report leaves the term suggestion and the reader usable; nothing is applied or written.
#[test]
fn a_selected_bridge_report_supplies_the_distant_slot_without_applying_it() {
    let Some(helper) = zt_digest_helper() else {
        eprintln!("skipping the selected-report read: no zt-digest is built or installed");
        return;
    };
    let home = TestHome::new_with(&[("ZT_DIGEST_HELPER", helper.to_str().expect("helper path"))]);
    let (gui, focus, other) = two_books(&home);
    let mut mcp = home.mcp();
    let focus_card = mcp.ok("get_card", json!({ "at": focus }));
    let other_card = mcp.ok("get_card", json!({ "at": other }));
    let report = published_bridge_report(&focus_card, &other_card, "b001", "corresponds");
    let report_id = report["report_id"].as_str().expect("report id").to_string();
    let report_path = home._tmp.path().join("bridge-report.json");
    fs::write(&report_path, serde_json::to_string(&report).expect("report JSON")).expect("write report");
    let before = json!({
        "graph": gui.get("/api/graph"),
        "overview": gui.get("/api/overview"),
        "spreads": gui.get("/api/spreads"),
        "register": gui.register(),
        "broken": gui.get("/api/broken"),
    });

    // The first read starts the optional helper and answers pending: the reader keeps its Card
    // and its ordinary term evidence while the helper works in the background.
    let first = gui.distant_report(&focus, &[], &report_path);
    assert_eq!(first["report"]["status"], json!("pending"), "{first}");
    assert_eq!(first["report"]["cached"], json!(false));
    assert_eq!(first["candidate"]["source"], json!("register_term"), "{first}");
    assert_eq!(first["candidate"]["address"], json!(other));
    // The answered projection is a current validated proposal: it takes priority and carries the
    // exact report and proposal id, and the same read is then cached by report hash and revision.
    let value = gui.distant_report_ready(&focus, &[], &report_path);
    assert_eq!(value["report"]["status"], json!("done"), "{value}");
    assert_eq!(value["report"]["report_id"], json!(report_id));
    assert_eq!(value["report"]["cached"], json!(true), "the answered read is cached");
    assert_eq!(value["candidate"]["source"], json!("bridge_report"), "{value}");
    assert_eq!(value["candidate"]["address"], json!(other));
    assert_eq!(value["candidate"]["report"]["proposal"], json!("b001"));
    assert_eq!(value["candidate"]["report"]["report"], json!(report_id));
    let reason = value["candidate"]["reason"].as_str().expect("one-line reason");
    assert!(
        reason.starts_with("Bridge report r-") && reason.ends_with("proposal b001: corresponds"),
        "{reason}"
    );
    let again = gui.distant_report(&focus, &[], &report_path);
    assert_eq!(again["report"]["status"], json!("done"));
    assert_eq!(again["report"]["cached"], json!(true));
    assert_eq!(again["candidate"], value["candidate"]);
    // Reading a report writes nothing: no Card, Link, Spread, register row, or archive revision.
    let read_only = json!({
        "graph": gui.get("/api/graph"),
        "overview": gui.get("/api/overview"),
        "spreads": gui.get("/api/spreads"),
        "register": gui.register(),
        "broken": gui.get("/api/broken"),
    });
    assert_eq!(read_only, before, "reading the selected report changed the archive");

    // A changed endpoint makes the reviewed proposal stale: the slot says so and falls back to
    // the register term, and the changed archive revision reads the report again.
    mcp.ok("edit_card", json!({ "at": other, "title": "多重均衡的另一种说法（改）" }));
    let stale = gui.distant_report_ready(&focus, &[], &report_path);
    assert_eq!(stale["report"]["status"], json!("stale"), "{stale}");
    assert_eq!(stale["candidate"]["source"], json!("register_term"), "{stale}");
    assert_eq!(stale["candidate"]["address"], json!(other));
    assert!(stale["report"]["counts"]["at"].as_u64().unwrap_or(0) >= 1, "{stale}");
    mcp.ok("edit_card", json!({ "at": other, "title": "多重均衡的另一种说法" }));
    let restored = gui.distant_report_ready(&focus, &[], &report_path);
    assert_eq!(restored["report"]["status"], json!("done"), "{restored}");
    assert_eq!(restored["candidate"]["source"], json!("bridge_report"));

    // An edited report no longer recomputes its own id: show the status, offer the term result,
    // and write nothing.
    let settled = json!({
        "graph": gui.get("/api/graph"),
        "overview": gui.get("/api/overview"),
        "spreads": gui.get("/api/spreads"),
        "register": gui.register(),
        "broken": gui.get("/api/broken"),
    });
    let mut tampered = report.clone();
    tampered["proposals"][0]["sentence"] = json!("对应 [[bookB/1]]：被改过的一句话。");
    let tampered_path = home._tmp.path().join("tampered-report.json");
    fs::write(&tampered_path, serde_json::to_string(&tampered).expect("tampered JSON")).expect("write");
    let invalid = gui.distant_report_ready(&focus, &[], &tampered_path);
    assert_eq!(invalid["report"]["status"], json!("invalid_report"), "{invalid}");
    assert_eq!(invalid["candidate"]["source"], json!("register_term"), "{invalid}");
    assert!(
        invalid["report"]["errors"][0].as_str().unwrap_or_default().contains("fingerprint"),
        "{invalid}"
    );
    // A missing report path is a status, not a broken reader: no helper run, and the term
    // suggestion still answers.
    let missing = gui.distant_report(&focus, &[], &home._tmp.path().join("missing.json"));
    assert_eq!(missing["report"]["status"], json!("missing_report"), "{missing}");
    assert_eq!(missing["candidate"]["source"], json!("register_term"), "{missing}");

    // No Card, Link, Spread, or report changed: the selection is read-only, and applying still
    // needs the compiler's exact report and proposal acceptance.
    let after = json!({
        "graph": gui.get("/api/graph"),
        "overview": gui.get("/api/overview"),
        "spreads": gui.get("/api/spreads"),
        "register": gui.register(),
        "broken": gui.get("/api/broken"),
    });
    assert_eq!(after["graph"], settled["graph"], "the projection changed a Card or a Link");
    assert_eq!(after["register"]["archive_revision"], settled["register"]["archive_revision"], "the projection bumped the archive revision");
    assert_eq!(after["spreads"], settled["spreads"]);
    assert_eq!(after["broken"], settled["broken"]);
    assert_eq!(fs::read_to_string(&report_path).expect("report"), serde_json::to_string(&report).expect("report JSON"));
    assert_eq!(mcp.ok("get_card", json!({ "at": focus }))["body"], focus_card["body"]);
}

/// S20-AC06: a missing installed helper is one small status; the term suggestion and the reader
/// stay usable, and the selected report is never read by anything else.
#[test]
fn a_missing_bridge_helper_leaves_the_term_suggestion() {
    let home = TestHome::new_with(&[("ZT_DIGEST_HELPER", "Z:\\no-such-zt-digest.exe")]);
    let (gui, focus, other) = two_books(&home);
    let mut mcp = home.mcp();
    let focus_card = mcp.ok("get_card", json!({ "at": focus }));
    let other_card = mcp.ok("get_card", json!({ "at": other }));
    let report = published_bridge_report(&focus_card, &other_card, "b007", "illustrates");
    let report_path = home._tmp.path().join("bridge-report.json");
    fs::write(&report_path, serde_json::to_string(&report).expect("report JSON")).expect("write report");
    let value = gui.distant_report_ready(&focus, &[], &report_path);
    assert_eq!(value["report"]["status"], json!("missing_helper"), "{value}");
    assert_eq!(value["report"]["cached"], json!(true));
    assert_eq!(value["candidate"]["source"], json!("register_term"), "{value}");
    assert_eq!(value["candidate"]["address"], json!(other));
    let card = gui.get(&format!("/api/card?at={}", enc(&focus)));
    assert_eq!(card["title"], json!("多重均衡的陈述"));
}
