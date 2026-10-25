//! The Term-link plan seam (S22, #160): the command runs over a real scratch archive and a staged
//! book, and the plan's artifacts are checked against the Cards the archive really holds. The plan
//! is read-only: the archive revision and every authored Card text are the proof.
//!
//! The test needs a `zt` binary built from this repository (or on PATH): it starts its own service
//! on a temporary archive. `ZT_TEST_ZT` overrides the binary.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
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
}

impl Drop for Zt {
    fn drop(&mut self) {
        let _ = self.run(&["down"]);
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

fn preview(staging: &Path) -> String {
    std::fs::read_to_string(staging.join("term-link").join("preview.md")).expect("preview.md")
}

fn card_at(plan: &Value, address: &str) -> Value {
    plan["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|card| card["address"] == address)
        .unwrap_or_else(|| panic!("no {address} in the plan"))
        .clone()
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

/// A Literature tree with one Term list, three heads and prose Cards: K/1 is the anchor that holds
/// the structure Card, the list and the notation table; K/2 and K/3 are prose.
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

#[test]
fn the_term_link_plan_reads_a_real_book_and_writes_no_card() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();

    // --- the book and a second book whose Term head a foreign use names -----------------------
    let (root, anchor, list, balance) = book(&mut mcp, "testbook2026", "Test Book");
    let other = mcp.call(
        "create_literature",
        json!({"bibtex":"@book{otherbook2026, title={Other Book}}","body":"另一本书。"}),
    )["address"]
        .as_str()
        .expect("other key")
        .to_string();
    let other_anchor = created(&mut mcp, &other, "direct", "第一章", "另一本书的第一章。");
    let other_list = created_term(
        &mut mcp,
        &other_anchor,
        "side",
        "术语表：《Other Book》的 1 个术语",
        None,
        &TABLE.replace("testbook2026", "otherbook2026"),
    );
    let other_head = created_term(
        &mut mcp,
        &other_list,
        "side",
        "均衡",
        Some("均衡"),
        &head_body(&other, "均衡", "另一个均衡。", false),
    );
    // Two prose Cards beside the anchor: code, math, a heading, a table, and a foreign Term use.
    let code_card = mcp.call(
        "create_successor",
        json!({
            "at": anchor, "kind": "direct", "title": "净生产函数与代码",
            "body": "净生产函数决定产出，例如 `净生产函数` 与 $均衡$ 与\n\n## 均衡\n\n| 均衡 |\n| --- |\n\n来源：[[testbook2026]] §1",
        }),
    )["address"]
        .as_str()
        .expect("code card")
        .to_string();
    let foreign_card = mcp.call(
        "create_successor",
        json!({
            "at": code_card, "kind": "direct", "title": "外来术语",
            "body": format!("见 [[均衡@{other_head}]] 与 生产函数。"),
        }),
    )["address"]
        .as_str()
        .expect("foreign card")
        .to_string();
    let revision = mcp.revision();
    let mut authored: Vec<(String, Value)> = Vec::new();
    for at in [&anchor, &code_card, &foreign_card] {
        let before = mcp.call("get_card", json!({"at": at}));
        authored.push((at.clone(), before));
    }

    let work = tempfile::tempdir().expect("work");
    let staged = staging(work.path(), "book", &root, "Test Book");
    let stage = staged.to_str().unwrap();

    // --- plan: the frozen scope, mapping, edits and counts ------------------------------------
    let output = zt.digest(&["term-link", "-Step", "plan", "-Staging", stage]);
    assert!(
        output.status.success(),
        "plan: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let said = last_json(&output);
    assert_eq!(said["ok"], true);
    assert_eq!(said["book"], root);
    assert_eq!(said["cards_scanned"], 6);
    assert_eq!(said["cards_changed"], 2);
    assert_eq!(said["cards_refused"], 0);
    assert_eq!(said["links_added"], 2);
    assert_eq!(said["complete"], true);
    assert_eq!(said["archive_revision"], revision);
    let plan = plan_json(&staged);
    assert_eq!(plan["kind"], "term_link_plan");
    assert_eq!(plan["schema_version"], 2);
    assert_eq!(plan["syntax_version"], 1);
    assert_eq!(plan["revision"], 1);
    assert!(plan["replaces"].is_null());
    assert_eq!(plan["replanned"], json!([]));
    assert_eq!(plan["list"]["address"], list);
    assert_eq!(plan["list"]["row_count"], 3);
    assert_eq!(plan["scope"]["included"], 6);
    assert_eq!(plan["scope"]["excluded"].as_array().unwrap().len(), 4);
    let reasons: Vec<(&str, &str)> = plan["scope"]["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|card| {
            (
                card["address"].as_str().unwrap(),
                card["reason"].as_str().unwrap(),
            )
        })
        .collect();
    assert!(reasons.contains(&(root.as_str(), "root")), "{reasons:?}");
    assert!(reasons.iter().any(|(_, reason)| *reason == "structure"));
    assert!(reasons.contains(&(list.as_str(), "term_list")), "{reasons:?}");
    assert!(reasons.iter().any(|(_, reason)| *reason == "notation"));
    let term_head = plan["heads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|head| head["term"] == "生产函数")
        .expect("the production head");
    assert_eq!(term_head["auto_link"], false);
    assert!(term_head["head"].as_str().unwrap().starts_with(&list));
    for key in ["list", "mapping", "autolink", "cards"] {
        assert_eq!(
            plan["fingerprints"][key].as_str().unwrap().len(),
            64,
            "{key}"
        );
    }
    assert_eq!(plan["plan_hash"].as_str().unwrap().len(), 64);
    assert_eq!(plan["term_structure"]["faults"], json!([]));

    let anchor_card = card_at(&plan, &anchor);
    assert_eq!(
        anchor_card["new_body"],
        format!("生产函数影响[[均衡@{balance}]]。均衡是稳定的。")
    );
    let code = card_at(&plan, &code_card);
    assert!(code["new_body"].as_str().unwrap().starts_with(&format!(
        "[[净生产函数@{}]]决定产出",
        plan["heads"]
            .as_array()
            .unwrap()
            .iter()
            .find(|head| head["term"] == "净生产函数")
            .unwrap()["head"]
            .as_str()
            .unwrap()
    )));
    let skipped = &code["skips"];
    for reason in ["code", "math", "heading", "table"] {
        assert!(
            skipped
                .as_array()
                .unwrap()
                .iter()
                .any(|skip| skip["reason"] == reason),
            "{reason}: {skipped}"
        );
    }
    let foreign = card_at(&plan, &foreign_card);
    let retained = foreign["retained"].as_array().unwrap();
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0]["reason"], "foreign_book");
    assert_eq!(retained[0]["target"], other_head);
    assert_eq!(foreign["changed"], false);

    // The preview names the totals and every changed line.
    let preview_text = preview(&staged);
    assert!(preview_text.contains("Book: testbook2026 — Test Book"));
    assert!(preview_text.contains("6 Cards scanned, 2 changed, 0 refused"));
    assert!(preview_text.contains("Links: 2 added, 0 repaired, 0 kept, 0 removed"));
    assert!(preview_text.contains("line 1:"));
    assert!(preview_text.contains("Excluded by role"));

    // Planning wrote no Card and advanced no archive revision.
    assert_eq!(mcp.revision(), revision, "planning wrote to the archive");
    for (address, before) in authored {
        let after = mcp.call("get_card", json!({"at": address}));
        assert_eq!(after["text"], before["text"], "{address} changed");
        assert_eq!(after["title"], before["title"]);
    }

    // --- the same frozen inputs produce the same bytes ---------------------------------------
    let plan_before = std::fs::read(staged.join("term-link").join("plan.json")).unwrap();
    let preview_before = std::fs::read(staged.join("term-link").join("preview.md")).unwrap();
    let again = zt.digest(&["term-link", "-Step", "plan", "-Staging", stage]);
    assert!(again.status.success());
    assert_eq!(last_json(&again)["plan_hash"], said["plan_hash"]);
    assert_eq!(
        std::fs::read(staged.join("term-link").join("plan.json")).unwrap(),
        plan_before
    );
    assert_eq!(
        std::fs::read(staged.join("term-link").join("preview.md")).unwrap(),
        preview_before
    );

    // --- the policy, the exact subset, and the refusals --------------------------------------
    let all = zt.digest(&[
        "term-link",
        "-Step",
        "plan",
        "-Staging",
        stage,
        "-Occurrences",
        "all",
    ]);
    assert!(all.status.success(), "{}", String::from_utf8_lossy(&all.stderr));
    let all_said = last_json(&all);
    assert_eq!(all_said["links_added"], 3);
    assert_ne!(all_said["plan_hash"], said["plan_hash"]);
    let subset = zt.digest(&[
        "term-link",
        "-Step",
        "plan",
        "-Staging",
        stage,
        "-Cards",
        &code_card,
        "-CitationKey",
        &root,
    ]);
    assert!(
        subset.status.success(),
        "{}",
        String::from_utf8_lossy(&subset.stderr)
    );
    let subset_plan = plan_json(&staged);
    assert_eq!(subset_plan["scope"]["included"], 1);
    assert_eq!(subset_plan["scope"]["unselected"], 5);
    assert_eq!(subset_plan["options"]["cards"], json!([code_card]));
    assert_eq!(subset_plan["cards"][0]["address"], code_card);
    for (args, needle) in [
        (
            vec!["term-link", "-Step", "plan", "-Staging", stage, "-Cards", &root],
            "excludes",
        ),
        (
            vec!["term-link", "-Step", "plan", "-Staging", stage, "-Cards", "NOPE"],
            "not a Card",
        ),
        (
            vec![
                "term-link",
                "-Step",
                "plan",
                "-Staging",
                stage,
                "-CitationKey",
                "otherbook2026",
            ],
            "another book",
        ),
    ] {
        let out = zt.digest(&args);
        assert!(!out.status.success(), "{args:?} was accepted");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(needle),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // --- a Topic tree is no Literature tree --------------------------------------------------
    let topic_root = mcp.call("create_topic", json!({"title":"主题一","body":"一个主题。"}))["address"]
        .as_str()
        .expect("topic root")
        .to_string();
    let topic_id = topic_root.split('/').next().unwrap_or_default().to_string();
    let topic = staging(work.path(), "topic", &topic_id, "Topic");
    let out = zt.digest(&["term-link", "-Step", "plan", "-Staging", topic.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("not a Literature tree"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // --- every frozen Card carries its apply guard, and misuse still stops the step -----------
    for card in plan["cards"].as_array().unwrap() {
        assert_eq!(
            card["authored_fingerprint"].as_str().unwrap().len(),
            64,
            "{}",
            card["address"]
        );
        assert!(card["new_fingerprint"].is_string() == (card["changed"] == true));
    }
    let bare = zt.digest(&["term-link", "-Step", "apply", "-Staging", stage]);
    assert!(!bare.status.success());
    let before_malformed = mcp.revision();

    // --- malformed Term-like markup refuses that Card, not the whole plan ---------------------
    // No MCP write can author malformed Term markup (the gate refuses it), so the syntax fault
    // enters the exact place a damaged import would leave it: the stored Card text.
    let db = zt.archive.join("zt.sqlite3");
    let conn = rusqlite::Connection::open(&db).expect("open the scratch archive");
    conn.busy_timeout(std::time::Duration::from_secs(10)).unwrap();
    conn.execute(
        "UPDATE cards SET text = ?1 WHERE location = ?2",
        rusqlite::params![
            format!(
                "# 外来术语\n\n见 [[均衡@{}@x]]。\n\n<!-- zt:reverse-links -->\n",
                other_head
            ),
            foreign_card
        ],
    )
    .expect("inject malformed Term markup");
    drop(conn);
    let malformed = zt.digest(&["term-link", "-Step", "plan", "-Staging", stage, "-Occurrences", "first"]);
    assert!(
        malformed.status.success(),
        "{}",
        String::from_utf8_lossy(&malformed.stderr)
    );
    let malformed_said = last_json(&malformed);
    assert_eq!(malformed_said["cards_refused"], 1);
    assert_eq!(malformed_said["complete"], false);
    let plan = plan_json(&staged);
    let fault = card_at(&plan, &foreign_card)["fault"].clone();
    assert_eq!(fault["code"], "malformed_term_markup");
    assert_eq!(fault["line"], 1);
    assert!(preview(&staged).contains("Refused Cards"));
    assert_eq!(mcp.revision(), before_malformed, "the read-only plan never moves the revision");
}
