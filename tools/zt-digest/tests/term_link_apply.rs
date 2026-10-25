//! The Term-link apply seam (S23, #163): the command runs over a real scratch archive and a staged
//! book, revalidates its frozen plan against the Cards the archive really holds, and writes one
//! final edit per changed Card through bounded identified chunks.
//!
//! The test needs a `zt` binary built from this repository (or on PATH): it starts its own service
//! on a temporary archive and, for the Lock refusal, takes the owner's Lock through the real token-
//! guarded GUI route. `ZT_TEST_ZT` overrides the binary, `ZT_TEST_DIGEST` the `zt-digest` one.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};

fn zt_program() -> Option<PathBuf> {
    if let Ok(program) = std::env::var("ZT_TEST_ZT") {
        return Some(PathBuf::from(program));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for candidate in [
        root.join("target/debug/zt.exe"),
        root.join("target/release/zt.exe"),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    Command::new("zt")
        .arg("version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| PathBuf::from("zt"))
}

struct Zt {
    _dir: tempfile::TempDir,
    config: PathBuf,
    archive: PathBuf,
    program: PathBuf,
}

impl Zt {
    fn new(program: PathBuf) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("config");
        let archive = dir.path().join("archive");
        let zt = Self {
            _dir: dir,
            config,
            archive,
            program,
        };
        let configured = zt.run(&[
            "config",
            "set",
            "archive_root",
            zt.archive.to_str().unwrap(),
        ]);
        assert!(
            configured.status.success(),
            "config: {}",
            String::from_utf8_lossy(&configured.stderr)
        );
        let up = zt.run(&["up"]);
        assert!(
            up.status.success(),
            "up: {} {}",
            String::from_utf8_lossy(&up.stdout),
            String::from_utf8_lossy(&up.stderr)
        );
        zt
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.program)
            .args(args)
            .env("ZT_CONFIG_DIR", &self.config)
            .env("ZT_GUI_PORT", "0")
            .output()
            .expect("run zt")
    }

    fn mcp(&self) -> Mcp {
        let mut child = Command::new(&self.program)
            .arg("mcp")
            .env("ZT_CONFIG_DIR", &self.config)
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
            next: 1,
        };
        mcp.request(
            "initialize",
            json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"zt-digest-tests","version":"1"}}),
        );
        mcp.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        mcp
    }

    fn digest(&self, args: &[&str]) -> Output {
        let program = std::env::var("ZT_TEST_DIGEST")
            .unwrap_or_else(|_| env!("CARGO_BIN_EXE_zt-digest").to_string());
        Command::new(program)
            .args(args)
            .env("ZT_CONFIG_DIR", &self.config)
            .env("ZT_GUI_PORT", "0")
            .env("ZT_CHECK_ZT", &self.program)
            .output()
            .expect("run zt-digest")
    }

    /// The running service's GUI: its port and the token the page carries.
    fn gui(&self) -> Gui {
        let said = self.run(&["gui"]);
        assert!(
            said.status.success(),
            "gui: {}",
            String::from_utf8_lossy(&said.stderr)
        );
        let stdout = String::from_utf8_lossy(&said.stdout).into_owned();
        let port = stdout
            .lines()
            .find_map(|line| line.trim().strip_prefix("zt gui: http://127.0.0.1:"))
            .and_then(|rest| rest.trim_end_matches('/').parse().ok())
            .unwrap_or_else(|| panic!("no GUI URL: {stdout}"));
        let gui = Gui {
            port,
            token: String::new(),
        };
        let page = gui.get("/");
        let token = page
            .split("name=\"zt-token\" content=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the page carries the GUI token")
            .to_string();
        Gui { port, token }
    }

    /// The scratch archive's Term-link operation receipts (the test's own setup batches have their
    /// own prefix and are not counted).
    fn receipts(&self) -> i64 {
        let conn = rusqlite::Connection::open(self.archive.join("zt.sqlite3"))
            .expect("open the scratch archive");
        conn.busy_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        conn.query_row(
            "SELECT count(*) FROM applied_ops WHERE op_key LIKE 'term-link-%'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
    }

    /// One Card's stored text, read without the archive's parsing (a damaged import may not parse).
    fn stored_text(&self, at: &str) -> String {
        let conn = rusqlite::Connection::open(self.archive.join("zt.sqlite3"))
            .expect("open the scratch archive");
        conn.busy_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        conn.query_row("SELECT text FROM cards WHERE location=?1", [at], |row| {
            row.get(0)
        })
        .expect("stored Card text")
    }
}

impl Drop for Zt {
    fn drop(&mut self) {
        let _ = self.run(&["down"]);
    }
}

struct Gui {
    port: u16,
    token: String,
}

impl Gui {
    fn get(&self, path: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect gui");
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nContent-Length: 0\r\n\r\n",
            self.port
        )
        .expect("write gui");
        let mut page = String::new();
        stream.read_to_string(&mut page).expect("read gui page");
        page
    }

    fn post(&self, path: &str, body: &Value) -> Value {
        let payload = body.to_string();
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect gui");
        write!(
            stream,
            "POST {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            self.port,
            self.token,
            payload.len(),
            payload
        )
        .expect("write gui");
        let mut page = String::new();
        stream.read_to_string(&mut page).expect("read gui answer");
        let body = page.split("\r\n\r\n").nth(1).unwrap_or(&page);
        serde_json::from_str(body).unwrap_or_else(|error| panic!("{error}: {page}"))
    }

    /// The owner's two-step Lock: plan, then confirm its hash.
    fn lock(&self, root: &str) -> Value {
        let request = json!({ "root": root, "note": "the Term-link apply seam" });
        let plan = self.post("/api/lock", &request);
        assert_eq!(plan["applied"], false, "the first request only plans: {plan}");
        let mut confirmed = request.clone();
        confirmed["confirm"] = json!("lock");
        confirmed["plan_hash"] = plan["plan"]["plan_hash"].clone();
        let applied = self.post("/api/lock", &confirmed);
        assert_eq!(applied["applied"], true, "the confirmation commits: {applied}");
        applied["lock"].clone()
    }

    fn unlock(&self, lock_id: i64) {
        let plan = self.post("/api/unlock", &json!({ "lock_id": lock_id }));
        assert_eq!(plan["applied"], false, "unlock plans first: {plan}");
        let applied = self.post(
            "/api/unlock",
            &json!({"lock_id": lock_id, "confirm": "unlock", "plan_hash": plan["plan"]["plan_hash"]}),
        );
        assert_eq!(applied["applied"], true, "unlock commits: {applied}");
    }
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Mcp {
    fn send(&mut self, value: &Value) {
        writeln!(self.stdin, "{value}").expect("write mcp");
        self.stdin.flush().expect("flush mcp");
    }

    fn recv(&mut self) -> Value {
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("read mcp");
            assert!(read > 0, "zt mcp closed its stdout");
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                return value;
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        loop {
            let value = self.recv();
            if value["id"] == json!(id) {
                if !value["error"].is_null() {
                    panic!("mcp {method} failed: {value}");
                }
                return value["result"].clone();
            }
        }
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let result = self.request("tools/call", json!({"name":tool,"arguments":arguments}));
        if result["isError"] == json!(true) {
            panic!("{tool} was refused: {result}");
        }
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{tool} returned no text: {result}"));
        serde_json::from_str(text).unwrap_or_else(|error| panic!("{tool}: {error}: {text}"))
    }

    fn revision(&mut self) -> u64 {
        self.call("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"]
            .as_u64()
            .expect("archive revision")
    }

    /// One tool call whose typed refusal is read instead of panicking.
    fn try_call(&mut self, tool: &str, arguments: Value) -> (Value, bool) {
        let result = self.request("tools/call", json!({"name":tool,"arguments":arguments}));
        let is_error = result["isError"] == json!(true);
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        let value = serde_json::from_str(text).unwrap_or(Value::Null);
        (value, is_error)
    }

    fn body(&mut self, at: &str) -> String {
        self.call("get_card", json!({"at": at}))["body"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn staging(parent: &Path, name: &str, key: &str, title: &str) -> PathBuf {
    let dir = parent.join(name);
    std::fs::create_dir_all(&dir).expect("make staging");
    std::fs::write(
        dir.join("manifest.json"),
        json!({
            "citation_key": key,
            "book_title": title,
            "staging": dir.to_string_lossy(),
        })
        .to_string(),
    )
    .expect("write manifest");
    dir
}

const TABLE: &str = "术语表：《Test Book》的 3 个术语。本书的卡片用左列的术语。

术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 均衡 | — | 稳定状态 | §1 |
| 生产函数 | 产出函数 | 投入与产出的关系 | §2 |
| 净生产函数 | — | 扣除折旧后的产出 | §3 |

来源：[[testbook2026]]";

fn head_body(book: &str, term: &str, section: &str, auto_off: bool) -> String {
    let mut body = format!("术语：{term}\n类别：概念\n首次定义：§1\n");
    if auto_off {
        body.push_str("自动链接：否\n自动链接理由：这是过于一般的词。\n");
    }
    body.push_str(&format!("\n## 释义\n\n{section}\n\n来源：[[{book}]] §1"));
    body
}

fn last_json(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| panic!("no output: {}", String::from_utf8_lossy(&output.stderr)));
    serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}"))
}

fn plan_json(staging: &Path) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(staging.join("term-link").join("plan.json")).expect("plan.json"),
    )
    .expect("plan JSON")
}

fn report_json(staging: &Path) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(staging.join("term-link").join("apply.json")).expect("apply.json"),
    )
    .expect("apply report JSON")
}

fn preview(staging: &Path) -> String {
    std::fs::read_to_string(staging.join("term-link").join("preview.md")).expect("preview.md")
}

fn plan_of(zt: &Zt, stage: &str) -> Value {
    let output = zt.digest(&["term-link", "-Step", "plan", "-Staging", stage]);
    assert!(
        output.status.success(),
        "plan: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    last_json(&output)
}

fn apply(zt: &Zt, stage: &str) -> Output {
    zt.digest(&["term-link", "-Step", "apply", "-Staging", stage, "-Apply"])
}

fn created(mcp: &mut Mcp, at: &str, kind: &str, title: &str, body: &str) -> String {
    mcp.call(
        "create_successor",
        json!({"at": at, "kind": kind, "title": title, "body": body}),
    )["address"]
        .as_str()
        .expect("created address")
        .to_string()
}

fn created_term(
    mcp: &mut Mcp,
    at: &str,
    kind: &str,
    title: &str,
    term: Option<&str>,
    body: &str,
) -> String {
    let mut arguments = json!({"at": at, "kind": kind, "title": title, "body": body, "type": "term"});
    if let Some(term) = term {
        arguments["term"] = json!(term);
    }
    mcp.call("create_successor", arguments)["address"]
        .as_str()
        .expect("created Term Card")
        .to_string()
}

/// A Literature tree with one anchor, its structure Card, one Term list, the notation table and
/// three Term heads. `K/1` is the anchor; prose Cards made later are its Direct successors.
fn book(mcp: &mut Mcp, key: &str, title: &str) -> (String, String, String, String) {
    let root = mcp.call("create_literature", json!({"bibtex":format!("@book{{{key}, title={{{title}}}}}"),"body":"一本测试书。"}))["address"]
        .as_str()
        .expect("citation key")
        .to_string();
    assert_eq!(root, key);
    let anchor = created(mcp, &root, "direct", "第一章", "生产函数影响均衡。均衡是稳定的。");
    created(
        mcp,
        &anchor,
        "side",
        "书的论证结构",
        &format!("本卡是《{title}》的结构卡：按阅读顺序列出全书的论证。"),
    );
    let list = created_term(
        mcp,
        &anchor,
        "side",
        &format!("术语表：《{title}》的 3 个术语"),
        None,
        TABLE,
    );
    created(
        mcp,
        &anchor,
        "side",
        &format!("符号表：《{title}》的 1 个符号"),
        "| 符号 | 含义 | 定义于 |\n| --- | --- | --- |\n| $K$ | 资本 | §1 |\n",
    );
    let balance = created_term(
        mcp,
        &list,
        "side",
        "均衡",
        Some("均衡"),
        &head_body(&root, "均衡", "均衡是稳定的状态。", false),
    );
    created_term(
        mcp,
        &list,
        "side",
        "生产函数",
        Some("生产函数"),
        &head_body(&root, "生产函数", "生产函数是投入与产出的关系。", true),
    );
    created_term(
        mcp,
        &list,
        "side",
        "净生产函数",
        Some("净生产函数"),
        &head_body(&root, "净生产函数", "净生产函数是扣除折旧后的产出。", false),
    );
    (root, anchor, list, balance)
}

/// Add `count` Direct successors below `from` in identified batches of 64. Each one is prose that
/// mentions the term 均衡 once, so the pass links it.
fn create_many(mcp: &mut Mcp, from: &str, count: usize) -> Vec<String> {
    let mut made: Vec<String> = Vec::new();
    let mut last = from.to_string();
    let mut next = 0usize;
    while next < count {
        let take = (count - next).min(64);
        let mut items = Vec::new();
        for index in 0..take {
            let at = if index == 0 {
                json!(last)
            } else {
                json!(format!("@@{}", index - 1))
            };
            items.push(json!({
                "op_key": format!("seam-setup/create/1/setup/item-{:06}", next + index),
                "client_ref": index.to_string(),
                "tool": "create_successor",
                "arguments": {
                    "at": at,
                    "kind": "direct",
                    "title": format!("第{}段", next + index + 1),
                    "body": "均衡是稳定的状态。",
                },
            }));
        }
        let value = mcp.call("apply_batch", json!({ "items": items }));
        for result in value["results"].as_array().expect("results") {
            made.push(
                result["result"]["address"]
                    .as_str()
                    .expect("allocated address")
                    .to_string(),
            );
        }
        last = made.last().cloned().expect("one created Card");
        next += take;
    }
    made
}

/// The one acceptance seam of design 13.6's verification: a 300-Card book applies its 301 edits in
/// five bounded identified calls, a rerun replays the accepted receipts, and a replanned no-op
/// writes nothing.
#[test]
fn the_term_link_apply_writes_three_hundred_cards_in_bounded_chunks() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (root, anchor, list, _balance) = book(&mut mcp, "testbook2026", "Test Book");
    let prose = create_many(&mut mcp, &anchor, 300);
    let revision = mcp.revision();
    let work = tempfile::tempdir().expect("work");
    let staged = staging(work.path(), "book", &root, "Test Book");
    let stage = staged.to_str().unwrap();

    // --- plan: 304 Cards scanned, 301 changed ------------------------------------------------
    let said = plan_of(&zt, stage);
    assert_eq!(said["ok"], true);
    assert_eq!(said["cards_scanned"], 304);
    assert_eq!(said["cards_changed"], 301);
    assert_eq!(said["cards_refused"], 0);
    assert_eq!(said["complete"], true);
    let plan = plan_json(&staged);
    let old_hash = plan["plan_hash"].as_str().expect("plan hash").to_string();
    assert_eq!(plan["schema_version"], 2);
    assert_eq!(plan["revision"], 1);
    let outputs: Vec<(String, String)> = plan["cards"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|card| card["changed"] == true)
        .map(|card| {
            (
                card["address"].as_str().unwrap().to_string(),
                card["new_body"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(outputs.len(), 301);
    assert!(outputs.iter().any(|(at, _)| at == &anchor));

    // --- apply: five identified calls, one final edit per changed Card ------------------------
    let output = apply(&zt, stage);
    assert!(
        output.status.success(),
        "apply: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let said = last_json(&output);
    assert_eq!(said["ok"], true);
    assert_eq!(said["complete"], true);
    assert_eq!(said["no_op"], false);
    assert_eq!(said["cards_written"], 301);
    assert_eq!(said["cards_replayed"], 0);
    assert_eq!(said["chunks"], 5);
    assert_eq!(said["chunks_committed"], 5);
    assert_eq!(said["chunks_pending"], 0);
    assert_eq!(said["archive_revision_before"], revision);
    assert_eq!(said["archive_revision_after"], revision + 5);
    let report = report_json(&staged);
    assert_eq!(report["kind"], "term_link_apply");
    let chunks = report["chunks"].as_array().expect("chunks");
    assert_eq!(chunks.len(), 5);
    let mut items = 0usize;
    let mut cards_seen: Vec<String> = Vec::new();
    for chunk in chunks {
        assert_eq!(chunk["state"], "committed");
        assert!(chunk["items"].as_u64().unwrap() <= 64, "{chunk}");
        items += chunk["items"].as_u64().unwrap() as usize;
        for card in chunk["cards"].as_array().unwrap() {
            cards_seen.push(card.as_str().unwrap().to_string());
        }
        for receipt in chunk["receipts"].as_array().unwrap() {
            assert_eq!(receipt["replayed"], false);
            assert_eq!(receipt["after_fingerprint"].as_str().unwrap().len(), 64);
        }
    }
    assert_eq!(items, 301);
    cards_seen.sort();
    cards_seen.dedup();
    assert_eq!(cards_seen.len(), 301, "one final edit per changed Card");
    assert!(cards_seen.contains(&anchor));
    // Every stored body is exactly the planned output, and the visible prose never changed.
    for (at, new_body) in &outputs {
        assert_eq!(&mcp.body(at), new_body, "{at}");
        assert_eq!(
            zt_check::text::strip_term_links(new_body),
            zt_check::text::strip_term_links(&mcp.body(at)),
            "{at}"
        );
    }
    assert_eq!(mcp.revision(), revision + 5);
    let receipts = zt.receipts();
    assert_eq!(receipts, 301);

    // --- a rerun replays the same stable operation keys and writes nothing --------------------
    let output = apply(&zt, stage);
    let failed_report = report_json(&staged);
    assert!(
        output.status.success(),
        "replay: {} {}",
        String::from_utf8_lossy(&output.stderr),
        serde_json::to_string(&failed_report).unwrap()
    );
    let said = last_json(&output);
    assert_eq!(said["cards_written"], 0);
    assert_eq!(said["cards_replayed"], 301);
    assert_eq!(said["chunks_committed"], 0);
    assert_eq!(said["chunks_replayed"], 5);
    assert_eq!(said["archive_revision_after"], revision + 5);
    assert_eq!(said["cards_already_applied"], 0);
    assert_eq!(failed_report["observations"]["cards_without_term_use"], 3);
    assert_eq!(failed_report["summary"]["cards_written"], 0);
    assert_eq!(mcp.revision(), revision + 5, "a replay advances no revision");
    assert_eq!(zt.receipts(), receipts, "a replay writes no new receipt");

    // --- an unchanged valid tree plans no edit, and its apply writes no receipt ---------------
    let said = plan_of(&zt, stage);
    assert_eq!(said["cards_changed"], 0);
    let output = apply(&zt, stage);
    assert!(output.status.success());
    let said = last_json(&output);
    assert_eq!(said["ok"], true);
    assert_eq!(said["complete"], true);
    assert_eq!(said["no_op"], true);
    assert_eq!(said["chunks"], 0);
    assert_eq!(said["cards_written"], 0);
    assert_eq!(said["archive_revision_after"], revision + 5);
    assert_eq!(zt.receipts(), receipts, "a no-op plan writes no receipt");
    assert_eq!(mcp.revision(), revision + 5);
    let report = report_json(&staged);
    assert_eq!(report["no_op"], true);
    assert!(plan_json(&staged)["plan_hash"].as_str().unwrap() != old_hash.as_str());
    assert_eq!(prose.len(), 300, "the fixture created every prose Card");
    assert!(!list.is_empty());
}

#[test]
fn a_changed_term_list_invalidates_the_whole_plan_and_writes_nothing() {
    let program = zt_program().expect("this seam needs a zt binary");
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (root, anchor, list, _balance) = book(&mut mcp, "testbook2026", "Test Book");
    created(&mut mcp, &anchor, "direct", "第二章", "净生产函数决定产出。");
    let revision = mcp.revision();
    let work = tempfile::tempdir().expect("work");
    let staged = staging(work.path(), "book", &root, "Test Book");
    let stage = staged.to_str().unwrap();
    plan_of(&zt, stage);
    let plan_bytes = std::fs::read(staged.join("term-link").join("plan.json")).unwrap();
    assert_eq!(plan_json(&staged)["counts"]["cards_changed"], 2);
    // The Term list changed after the preview: one avoid word is another word.
    let changed_table = TABLE.replace("| 生产函数 | 产出函数 |", "| 生产函数 | 产出量 |");
    mcp.call(
        "edit_card",
        json!({"at": list, "body": changed_table}),
    );
    let revision_after_edit = mcp.revision();
    assert!(revision_after_edit > revision);
    let output = apply(&zt, stage);
    assert_eq!(
        output.status.code(),
        Some(1),
        "a stale plan is a content stop: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let said = last_json(&output);
    assert_eq!(said["ok"], false);
    assert_eq!(said["code"], "plan_stale");
    assert!(said["message"].as_str().unwrap().contains("run -Step plan again"));
    let report = report_json(&staged);
    assert_eq!(report["code"], "plan_stale");
    let reasons = report["reasons"].as_array().expect("reasons");
    assert!(
        reasons.iter().any(|reason| reason
            .as_str()
            .unwrap()
            .contains("Term list changed")),
        "{reasons:?}"
    );
    // The plan artifact is exactly as the review left it, the archive holds no receipt, and no
    // Card moved.
    assert_eq!(
        std::fs::read(staged.join("term-link").join("plan.json")).unwrap(),
        plan_bytes
    );
    assert_eq!(zt.receipts(), 0);
    assert_eq!(mcp.revision(), revision_after_edit);
    assert_eq!(
        mcp.body(&anchor),
        "生产函数影响均衡。均衡是稳定的。",
        "the anchor keeps its authored body"
    );
}

#[test]
fn a_changed_body_is_replanned_into_a_new_revision_and_applied_from_its_fresh_offsets() {
    let program = zt_program().expect("this seam needs a zt binary");
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (root, anchor, _list, _balance) = book(&mut mcp, "testbook2026", "Test Book");
    let second = created(&mut mcp, &anchor, "direct", "第二章", "净生产函数决定产出。");
    let revision = mcp.revision();
    let work = tempfile::tempdir().expect("work");
    let staged = staging(work.path(), "book", &root, "Test Book");
    let stage = staged.to_str().unwrap();
    plan_of(&zt, stage);
    let plan = plan_json(&staged);
    let old_hash = plan["plan_hash"].as_str().unwrap().to_string();
    assert_eq!(plan["revision"], 1);
    let old_anchor = plan["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|card| card["address"] == anchor)
        .unwrap()
        .clone();
    assert_eq!(old_anchor["changed"], true);
    // The operator adds a first sentence before apply: the planned offsets are now stale.
    let changed = "第一段先交代背景。生产函数影响均衡。均衡是稳定的。";
    mcp.call("edit_card", json!({"at": anchor, "body": changed}));
    let output = zt.digest(&[
        "term-link", "-Step", "apply", "-Staging", stage, "-Apply", "-Token", "7",
    ]);
    assert!(
        output.status.success(),
        "apply: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let said = last_json(&output);
    assert_eq!(said["ok"], true);
    assert_eq!(said["complete"], true);
    assert_eq!(said["revised"], true);
    assert_eq!(said["replanned"], 1);
    assert_eq!(said["plan_revision"], 2);
    assert_eq!(said["claim_token"], 7);
    assert_eq!(said["cards_written"], 2);
    let report = report_json(&staged);
    assert_eq!(report["claim"]["token"], 7);
    assert_eq!(report["plan"]["revised"], true);
    assert_eq!(report["plan"]["superseded"], old_hash.as_str());
    assert_eq!(report["plan"]["revision"], 2);
    let replanned = report["plan"]["replanned"].as_array().expect("replanned");
    assert_eq!(replanned.len(), 1);
    assert_eq!(replanned[0]["address"], anchor);
    assert_eq!(replanned[0]["reason"], "body_changed");
    assert_eq!(replanned[0]["plan_body"], "生产函数影响均衡。均衡是稳定的。");
    assert_eq!(replanned[0]["current_body"], changed);
    assert_eq!(
        replanned[0]["new_body"],
        "第一段先交代背景。生产函数影响[[均衡@K/4]]。均衡是稳定的。"
            .replace("K/4", plan_head(&plan, "均衡").as_str())
    );
    // The stored plan is the new revision, and its preview names the replanned Card.
    let revised = plan_json(&staged);
    assert_eq!(revised["revision"], 2);
    assert_eq!(revised["replaces"], old_hash.as_str());
    assert_eq!(revised["replanned"][0]["address"], anchor);
    assert_eq!(revised["replanned"][0]["reason"], "body_changed");
    let preview_text = preview(&staged);
    assert!(preview_text.contains("Plan revision 2"), "{preview_text}");
    assert!(preview_text.contains("## Replanned Cards"), "{preview_text}");
    // Both Cards received their one edit at the new offsets.
    assert_eq!(
        mcp.body(&anchor),
        replanned[0]["new_body"].as_str().unwrap()
    );
    assert_eq!(
        mcp.body(&second),
        format!("[[净生产函数@{}]]决定产出。", plan_head(&plan, "净生产函数"))
    );
    assert_eq!(zt.receipts(), 2);
    assert!(mcp.revision() > revision);
}

/// The address of one Term head in the plan's head mapping.
fn plan_head(plan: &Value, term: &str) -> String {
    plan["heads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|head| head["term"] == term)
        .and_then(|head| head["head"].as_str())
        .unwrap_or_else(|| panic!("no head for {term}"))
        .to_string()
}

#[test]
fn a_locked_tree_stops_the_apply_with_the_typed_refusal_and_exit_two() {
    let program = zt_program().expect("this seam needs a zt binary");
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (root, anchor, _list, _balance) = book(&mut mcp, "testbook2026", "Test Book");
    created(&mut mcp, &anchor, "direct", "第二章", "净生产函数决定产出。");
    let work = tempfile::tempdir().expect("work");
    let staged = staging(work.path(), "book", &root, "Test Book");
    let stage = staged.to_str().unwrap();
    plan_of(&zt, stage);
    let gui = zt.gui();
    let lock = gui.lock(&root);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    // Taking the Lock is itself an archive write; the apply must leave this revision alone.
    let revision = mcp.revision();
    let output = apply(&zt, stage);
    assert_eq!(
        output.status.code(),
        Some(2),
        "a Lock is the typed refusal exit: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let last = last_json(&output);
    assert_eq!(last["code"], "card_locked");
    assert_eq!(last["tree"], root.as_str());
    assert!(
        last["error"]
            .as_str()
            .unwrap()
            .contains("locked by the owner"),
        "{last}"
    );
    let report = report_json(&staged);
    assert_eq!(report["code"], "card_locked");
    assert_eq!(report["stop"]["code"], "card_locked");
    assert_eq!(report["stop"]["exit"], 2);
    assert_eq!(report["stop"]["detail"]["tree"], root.as_str());
    assert_eq!(report["summary"]["cards_written"], 0);
    assert_eq!(zt.receipts(), 0);
    assert_eq!(mcp.revision(), revision);
    assert_eq!(mcp.body(&anchor), "生产函数影响均衡。均衡是稳定的。");
    // Released, the same frozen plan applies.
    gui.unlock(lock_id);
    let output = apply(&zt, stage);
    assert!(
        output.status.success(),
        "apply after unlock: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let said = last_json(&output);
    assert_eq!(said["ok"], true);
    assert_eq!(said["cards_written"], 2);
    assert!(mcp.body(&anchor).contains("[[均衡@"));
}

#[test]
fn a_refused_card_is_reported_and_the_rest_of_the_plan_is_applied() {
    let program = zt_program().expect("this seam needs a zt binary");
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (root, anchor, _list, balance) = book(&mut mcp, "testbook2026", "Test Book");
    let second = created(&mut mcp, &anchor, "direct", "第二章", "净生产函数决定产出。");
    // No MCP write can author malformed Term markup (the gate refuses it), so the syntax fault
    // enters the exact place a damaged import would leave it: the stored Card text.
    let db = zt.archive.join("zt.sqlite3");
    let conn = rusqlite::Connection::open(&db).expect("open the scratch archive");
    conn.busy_timeout(std::time::Duration::from_secs(10)).unwrap();
    conn.execute(
        "UPDATE cards SET text = ?1 WHERE location = ?2",
        rusqlite::params![
            format!("# 第二章\n\n见 [[均衡@{}@x]]。\n\n<!-- zt:reverse-links -->\n", balance),
            second
        ],
    )
    .expect("inject malformed Term markup");
    drop(conn);
    let work = tempfile::tempdir().expect("work");
    let staged = staging(work.path(), "book", &root, "Test Book");
    let stage = staged.to_str().unwrap();
    let said = plan_of(&zt, stage);
    assert_eq!(said["cards_refused"], 1);
    assert_eq!(said["complete"], false);
    let output = apply(&zt, stage);
    assert!(
        output.status.success(),
        "apply: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let said = last_json(&output);
    assert_eq!(said["ok"], true);
    assert_eq!(said["complete"], false, "a refused Card keeps the run incomplete");
    assert_eq!(said["cards_refused"], 1);
    assert_eq!(said["cards_written"], 1, "the anchor is still applied");
    assert_eq!(said["chunks"], 1);
    let report = report_json(&staged);
    assert_eq!(report["refused"][0]["address"], second.as_str());
    assert_eq!(report["refused"][0]["code"], "malformed_term_markup");
    assert!(preview(&staged).contains("Refused Cards"));
    // A later get_card parses the whole snapshot, so the proof is the stored bytes: the anchor
    // carries its one new macro, and the refused Card is byte for byte as the damaged import left it.
    let stored_anchor = zt.stored_text(&anchor);
    assert!(
        stored_anchor.contains(&format!("[[均衡@{balance}]]")),
        "{stored_anchor}"
    );
    assert_eq!(
        zt.stored_text(&second),
        format!("# 第二章\n\n见 [[均衡@{balance}@x]]。\n\n<!-- zt:reverse-links -->\n")
    );
    assert_eq!(zt.receipts(), 1);
}

/// R0409: under a deliberate enforcement a stored Card's retained style fault stays legacy advice,
/// and the relinker never rewrites prose to pass. A newly introduced fault is still refused, so the
/// test proves the stage is really enforcing.
#[test]
fn a_retained_style_fault_stays_legacy_advice_and_relinking_changes_no_prose() {
    let program = zt_program().expect("this seam needs a zt binary");
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (root, anchor, _list, _balance) = book(&mut mcp, "testbook2026", "Test Book");
    // Accepted while the archive's gate is advisory: an R3 fault (a semicolon joins two clauses).
    let legacy = created(
        &mut mcp,
        &anchor,
        "direct",
        "旧卡",
        "甲；乙。净生产函数决定产出。",
    );
    let set = zt.run(&["config", "set", "gate.stage", "enforce"]);
    assert!(
        set.status.success(),
        "config: {}",
        String::from_utf8_lossy(&set.stderr)
    );
    // The stage really enforces: a new Card with a new fault is refused with its fault lines.
    let (refusal, is_error) = mcp.try_call(
        "create_successor",
        json!({"at": anchor, "kind": "side", "title": "新卡", "body": "丙；丁。"}),
    );
    assert!(is_error, "a new R3 fault must be refused: {refusal}");
    assert_eq!(refusal["error"], "card_check");
    // The pass rewrites only the macro syntax of the two stored Cards. Their visible prose is
    // unchanged, so their retained faults stay legacy advice and the guarded writes pass.
    let work = tempfile::tempdir().expect("work");
    let staged = staging(work.path(), "book", &root, "Test Book");
    let stage = staged.to_str().unwrap();
    let said = plan_of(&zt, stage);
    assert_eq!(said["cards_changed"], 2);
    let output = apply(&zt, stage);
    assert!(
        output.status.success(),
        "apply under enforcement: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let said = last_json(&output);
    assert_eq!(said["ok"], true);
    assert_eq!(said["complete"], true);
    assert_eq!(said["cards_written"], 2);
    let head = plan_head(&plan_json(&staged), "净生产函数");
    let stored = zt.stored_text(&legacy);
    assert!(
        stored.contains(&format!("甲；乙。[[净生产函数@{head}]]决定产出。")),
        "{stored}"
    );
    assert_eq!(
        zt_check::text::strip_term_links(&mcp.body(&legacy)),
        "甲；乙。净生产函数决定产出。",
        "the visible prose is byte for byte what it was"
    );
}
