//! Direct Term links (S10, #151): one `[[label@address]]` macro written through the existing Card
//! writes, read back byte for byte, indexed against its Term head, and refused when its target is
//! not a head or its label is malformed. The lexical scan is the shared one in `zt-check`; these
//! tests exercise it through the public MCP reads and writes.

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

/// A two-row term table: `均衡` is published as a head, `弹性` is still missing.
const TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n| 弹性 | 弹性系数 | 价格变化的反应程度 | §2.3 |\n";

/// One agent Literature tree whose anchor holds one Term list under `<key>/1|a`.
fn book_with_a_term_list(home: &TestHome, mcp: &mut Mcp) -> (String, String) {
    book_with_table(home, mcp, "Test2026", TABLE)
}

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

fn part(mcp: &mut Mcp, at: &str, term: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "kind": "direct", "title": "推导（续 2/2）：极限情形", "type": "term", "term": term }),
    ))
}

/// Create one ordinary Idea Card under `at` whose body is `body`.
fn idea(mcp: &mut Mcp, at: &str, title: &str, body: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "title": title, "body": body }),
    ))
}

#[test]
fn a_new_term_use_must_name_a_well_formed_macro_a_regular_location_and_a_head() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let eq = head(&mut mcp, &list, "均衡");
    let eq_part = part(&mut mcp, &eq, "均衡");
    let _ = anchor;
    let topic = address(mcp.ok("create_topic", json!({ "title": "拒绝" })));
    let other = idea(&mut mcp, &topic, "普通卡", "正文。");
    let revision = home.revision();
    let cards = |home: &TestHome| -> i64 {
        home.db()
            .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
            .expect("card count")
    };
    let before = cards(&home);
    let missing = format!("{list}|z");
    let long_label = "概".repeat(41);
    for (body, expected) in [
        (
            format!("均衡 [[均衡@{list}]] 决定价格。"),
            format!("Term link target `{list}` is not a Term head"),
        ),
        (
            format!("均衡 [[均衡@{eq_part}]] 决定价格。"),
            format!("Term link target `{eq_part}` is not a Term head"),
        ),
        (
            format!("均衡 [[均衡@{other}]] 决定价格。"),
            format!("Term link target `{other}` is not a Term head"),
        ),
        (
            format!("均衡 [[均衡@{missing}]] 决定价格。"),
            format!("Term link target `{missing}` does not exist"),
        ),
        (
            "均衡 [[均衡@NotALocation]] 决定价格。".to_string(),
            "invalid Term link macro target `NotALocation`".to_string(),
        ),
        (
            "均衡 [[均衡@0/0]] 决定价格。".to_string(),
            "invalid Term link macro target `0/0`".to_string(),
        ),
        (
            format!("均衡 [[a@b@{eq}]] 决定价格。"),
            "invalid Term link macro: one `@` separates the label and the address".to_string(),
        ),
        (
            format!("均衡 [[@{eq}]] 决定价格。"),
            "invalid Term link label: 1-40 characters on one line".to_string(),
        ),
        (
            format!("均衡 [[{long_label}@{eq}]] 决定价格。"),
            "invalid Term link label: 1-40 characters on one line".to_string(),
        ),
    ] {
        let error = mcp.err(
            "create_successor",
            json!({ "at": other, "title": "被拒绝的写入", "body": body }),
        );
        assert!(error.contains(&expected), "{body}: {error}");
    }
    // A refused write changes no Card, address, revision or index row.
    assert_eq!(cards(&home), before, "no Card was created");
    assert_eq!(home.revision(), revision, "no write advanced the revision");
    let term_rows: i64 = home
        .db()
        .query_row("SELECT COUNT(*) FROM links WHERE kind = 'term'", [], |row| row.get(0))
        .expect("term rows");
    assert_eq!(term_rows, 0);
}

/// Store one body in place of a Card's text, as an archive of an earlier release could hold it:
/// no write validation runs, so an invalid Term use can be seeded for the retention rule.
fn seed_body(home: &TestHome, source: &str, title: &str, body: &str) {
    home.db()
        .execute(
            "UPDATE cards SET text = ?1 WHERE location = ?2",
            rusqlite::params![format!("# {title}\n\n{body}"), source],
        )
        .expect("seed the stored body");
}

#[test]
fn a_label_that_differs_from_the_head_is_reported_and_never_rewritten() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let eq = head(&mut mcp, &list, "均衡");
    let other = idea(&mut mcp, &anchor, "价格", &format!("价格 [[平衡@{eq}]] 变化。"));

    // A mismatched label is advice, not a refusal: the authored word stays exactly as written.
    let card = mcp.ok("get_card", json!({ "at": &other }));
    assert_eq!(card["body"], json!(format!("价格 [[平衡@{eq}]] 变化。")));
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    let mismatch = terms["faults"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fault| fault["fault"] == "term_label_mismatch")
        .cloned()
        .expect("the label mismatch fault");
    assert_eq!(mismatch["source"], json!(other));
    assert_eq!(mismatch["label"], json!("平衡"));
    assert_eq!(mismatch["term"], json!("均衡"));
    assert_eq!(mismatch["target"], json!(eq));

    // An edit adds a second, correctly labelled use and keeps the first byte for byte.
    let edited = format!("价格 [[平衡@{eq}]] 与 [[均衡@{eq}]] 变化。");
    mcp.ok("edit_card", json!({ "at": &other, "body": edited }));
    assert_eq!(
        mcp.ok("get_card", json!({ "at": &other }))["body"],
        json!(edited)
    );
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    assert_eq!(terms["coverage"]["uses"], json!(2));
    assert_eq!(terms["uses"][0]["count"], json!(2));
    assert_eq!(terms["uses"][0]["label"], json!("平衡"), "the first use's label");

    // A real batch writes through the same parser, and one wrong item rolls it all back.
    let revision = home.revision();
    let cards: i64 = home
        .db()
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
        .expect("card count");
    let items = json!([
        { "op_key": "s10/batch/1/item/0000", "client_ref": "0", "tool": "create_successor",
          "arguments": { "at": other, "title": "批次一", "body": format!("均衡 [[均衡@{eq}]] 决定价格。") } },
        { "op_key": "s10/batch/1/item/0001", "client_ref": "1", "tool": "create_successor",
          "arguments": { "at": "@@0", "title": "批次二", "body": "均衡 [[均衡@NotALocation]] 决定价格。" } }
    ]);
    let error = mcp.err("apply_batch", json!({ "items": items }));
    assert!(error.contains("invalid Term link macro target `NotALocation`"), "{error}");
    assert_eq!(home.revision(), revision, "the refused batch rolled back");
    let after: i64 = home
        .db()
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
        .expect("card count");
    assert_eq!(after, cards, "no item of the batch survived");
    // The same first item alone commits and indexes its Term use.
    let applied = mcp.ok(
        "apply_batch",
        json!({ "items": [{
            "op_key": "s10/batch/1/item/0000", "client_ref": "0", "tool": "create_successor",
            "arguments": { "at": other, "title": "批次一", "body": format!("均衡 [[均衡@{eq}]] 决定价格。") }
        }] }),
    );
    let made = applied["results"][0]["result"]["address"]
        .as_str()
        .expect("the committed batch address")
        .to_string();
    let kind: Option<String> = home
        .db()
        .query_row(
            "SELECT kind FROM links WHERE source = ?1 AND ord = 0",
            [&made],
            |row| row.get(0),
        )
        .expect("the batch's indexed Term use");
    assert_eq!(kind.as_deref(), Some("term"));
}

#[test]
fn an_old_invalid_term_use_is_retained_only_with_its_label_and_multiplicity() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let eq = head(&mut mcp, &list, "均衡");
    let eq_part = part(&mut mcp, &eq, "均衡");
    // The source sits in the book's tree: that is the list whose coverage debt reports its uses.
    let source = address(mcp.ok(
        "create_successor",
        json!({ "at": &anchor, "kind": "side", "title": "旧使用者", "body": "旧正文。" }),
    ));
    let missing = format!("{list}|zz");
    let other_missing = format!("{list}|zy");
    let uses = format!(
        "[[幽灵@{missing}]] 与 [[幽灵@{missing}]] 与 [[错@{eq_part}]] 与 [[均衡@{eq}]]"
    );
    let seeded = format!("旧文 {uses}。");
    let edited = format!("新文 {uses}。");
    seed_body(&home, &source, "旧使用者", &seeded);

    // The retained uses keep their exact multiplicity, label and target, so the edit commits.
    mcp.ok("edit_card", json!({ "at": &source, "body": edited.clone() }));
    assert_eq!(
        mcp.ok("get_card", json!({ "at": &source }))["body"],
        json!(edited)
    );
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    assert_eq!(terms["coverage"]["uses"], json!(1), "the valid use counts");
    let targets = terms["faults"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fault| fault["fault"] == "term_link_target")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        targets.len(),
        3,
        "each retained missing use and the part target stay reported: {targets:#?}"
    );
    let reasons = targets
        .iter()
        .map(|fault| fault["reason"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(reasons, ["not_head", "missing", "missing"]);

    let refusal = |mcp: &mut Mcp, body: String| {
        mcp.err("edit_card", json!({ "at": &source, "body": body }))
    };
    // A third copy of an invalid use is new, not retained.
    let error = refusal(
        &mut mcp,
        format!("新文 {uses} 与 [[幽灵@{missing}]]。"),
    );
    assert!(error.contains(&format!("Term link target `{missing}` does not exist")), "{error}");
    // Changing the invalid target is not retention.
    let error = refusal(
        &mut mcp,
        format!("新文 [[幽灵@{other_missing}]] 与 [[幽灵@{missing}]] 与 [[幽灵@{missing}]] 与 [[错@{eq_part}]] 与 [[均衡@{eq}]]。"),
    );
    assert!(error.contains(&format!("Term link target `{other_missing}` does not exist")), "{error}");
    // Changing only the label of an invalid use is not retention either.
    let error = refusal(
        &mut mcp,
        format!("新文 [[幽灵甲@{missing}]] 与 [[幽灵@{missing}]] 与 [[错@{eq_part}]] 与 [[均衡@{eq}]]。"),
    );
    assert!(error.contains(&format!("Term link target `{missing}` does not exist")), "{error}");
    // A valid use has no multiplicity bound.
    mcp.ok(
        "edit_card",
        json!({ "at": &source, "body": format!("[[均衡@{eq}]] [[均衡@{eq}]] [[均衡@{eq}]]。") }),
    );

    // A malformed Term macro is never grandfathered, even when an old body already holds it.
    seed_body(&home, &source, "旧使用者", &format!("旧文 [[a@b@{missing}]]。"));
    let error = refusal(&mut mcp, format!("新文 [[a@b@{missing}]]。"));
    assert!(
        error.contains("invalid Term link macro: one `@` separates the label and the address"),
        "{error}"
    );
}

#[test]
fn term_like_examples_in_math_and_code_are_literal_at_the_write_seam() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (_anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let eq = head(&mut mcp, &list, "均衡");

    // Inline math, display math, a fenced block, an inline code span, and a malformed Term-like
    // macro inside a formula are all literal: the write commits and no Term use is indexed.
    let mut parent = address(mcp.ok("create_topic", json!({ "title": "公式" })));
    for body in [
        format!("公式 $[[均衡@{eq}]]$ 不是使用。"),
        format!("$$\n[[均衡@{eq}]]\n$$"),
        format!("```\n[[均衡@{eq}]]\n```"),
        format!("`[[均衡@{eq}]]` 是代码。"),
        format!("公式 $[[a@b@{eq}]]$ 也不是使用。"),
    ] {
        let made = address(mcp.ok(
            "create_successor",
            json!({ "at": &parent, "title": "公式卡", "body": body }),
        ));
        assert_eq!(mcp.ok("get_card", json!({ "at": &made }))["body"], json!(body));
        parent = made;
    }
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    assert_eq!(terms["coverage"]["uses"], json!(0), "{terms}");
    let term_rows: i64 = home
        .db()
        .query_row("SELECT COUNT(*) FROM links WHERE kind = 'term'", [], |row| row.get(0))
        .expect("term rows");
    assert_eq!(term_rows, 0);

    // An ordinary Link inside math keeps the interpretation it always had: it is a Link.
    let ordinary = address(mcp.ok(
        "create_successor",
        json!({ "at": &parent, "title": "普通链接", "body": format!("$[[{eq}]]$") }),
    ));
    let indexed: (String, Option<String>) = home
        .db()
        .query_row(
            "SELECT target, kind FROM links WHERE source = ?1 AND ord = 0",
            [&ordinary],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the ordinary Link");
    assert_eq!(indexed, (eq.clone(), None));
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

#[test]
fn a_move_rewrites_only_the_target_bytes_of_a_term_link() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let eq = head(&mut mcp, &list, "均衡");
    let source = idea(&mut mcp, &anchor, "价格", &format!("均衡 [[均衡@{eq}]] 决定价格。"));
    let second = address(mcp.ok(
        "create_successor",
        json!({ "at": &anchor, "kind": "side", "title": "术语表：二", "type": "term", "body": TABLE }),
    ));
    let destination = format!("{second}|a");

    // The move rewrites the source's target bytes, keeping the label and the punctuation.
    let moved = mcp.ok(
        "move_card",
        json!({ "at": &eq, "to": destination, "confirm": "move" }),
    );
    let new_eq = moved["address"].as_str().expect("moved head").to_string();
    assert_eq!(new_eq, destination);
    assert_eq!(
        mcp.ok("get_card", json!({ "at": &source }))["body"],
        json!(format!("均衡 [[均衡@{new_eq}]] 决定价格。"))
    );
    let row: (String, Option<String>) = home
        .db()
        .query_row(
            "SELECT target, kind FROM links WHERE source = ?1 AND ord = 0",
            [&source],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the rewritten Term use");
    assert_eq!(row, (new_eq.clone(), Some("term".to_string())));
    // The head's new list sees the use, and the index agrees with the bodies.
    let terms = mcp.ok("get_terms", json!({ "at": &second }));
    assert_eq!(terms["coverage"]["uses"], json!(1));
    assert_eq!(terms["uses"][0]["label"], json!("均衡"));
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

#[test]
fn a_written_term_link_keeps_its_macro_and_reaches_the_head() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let eq = head(&mut mcp, &list, "均衡");

    let source = idea(
        &mut mcp,
        &anchor,
        "均衡的价格",
        &format!("均衡 [[均衡@{eq}]] 决定价格。"),
    );

    // The raw body keeps the authored macro byte for byte (S10-AC07), and the read reports the
    // Term facts of the head it reaches.
    let card = mcp.ok("get_card", json!({ "at": &source }));
    assert_eq!(card["body"], json!(format!("均衡 [[均衡@{eq}]] 决定价格。")));
    let target = mcp.ok("get_card", json!({ "at": &eq }));
    assert_eq!(target["term"], json!("均衡"));
    assert_eq!(target["term_role"], json!("card"));

    // One indexed Term use points at the head's address, never at the `label@address` text.
    let db = home.db();
    let row: (String, Option<String>) = db
        .query_row(
            "SELECT target, kind FROM links WHERE source = ?1 AND ord = 0",
            [&source],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the indexed Term use");
    assert_eq!(row, (eq.clone(), Some("term".to_string())));

    // The computed structure sees the use with its authored label and no fault.
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    assert_eq!(
        terms["uses"],
        json!([{ "source": source, "source_title": "均衡的价格", "target": eq, "label": "均衡", "count": 1 }])
    );
    assert_eq!(terms["coverage"]["uses"], json!(1));
    assert_eq!(terms["heads"][0]["uses"], json!(1));
    let faults = terms["faults"].as_array().expect("faults");
    assert_eq!(faults.len(), 1, "{faults:#?}");
    assert_eq!(faults[0]["fault"], json!("term_row_without_card"), "only the missing row");

    // A Term use is no reasoning Link: the head's generated Reverse links name no user, and the
    // separate Term uses section names the source under its own heading (S11, #153).
    assert!(
        mcp.ok("get_card", json!({ "at": &source }))["text"]
            .as_str()
            .unwrap()
            .contains(&format!("均衡 [[均衡@{eq}]] 决定价格。")),
        "the stored Card text keeps the authored macro"
    );
    assert_eq!(card["term_role"], json!(null), "only a Term Card carries a role");
    let head_text = mcp.ok("get_card", json!({ "at": &eq }))["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(!head_text.contains("## Reverse links"), "{head_text}");
    assert!(
        head_text.contains(&format!("## Term uses\n\n- [[{source}]] 均衡的价格\n")),
        "the head lists its Term use in its own generated section: {head_text}"
    );
    let broken = mcp.ok("list_broken_links", json!({}));
    assert_eq!(broken["broken"], json!([]), "{broken}");
}
