//! The computed Term structure (S08, #150): one Term list's rows, heads, parts, uses, coverage and
//! faults, read through `get_terms` and the existing Card reads. A role is computed from the Card's
//! type, its exact name, and its placement, never stored.

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

/// A two-row term table: one row published as a head, one row still missing.
const TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n| 弹性 | 弹性系数 | 价格变化的反应程度 | §2.3 |\n";

/// One agent Literature tree `key` whose anchor holds a Term list at `<key>/1|a`, plus the two
/// addresses. Every test starts from a book fixture like this one.
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

/// The book fixture every structure test starts from.
fn book_with_a_term_list(home: &TestHome, mcp: &mut Mcp) -> (String, String) {
    book_with_table(home, mcp, "Test2026", TABLE)
}

/// Create one Term Card as a Side successor of `at` (a head when `at` is the list).
fn head(mcp: &mut Mcp, at: &str, term: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "kind": "side", "title": term, "type": "term", "term": term }),
    ))
}

/// Create one Term Card as a Direct successor of `at` (a part when `at` is a head or part).
fn part(mcp: &mut Mcp, at: &str, term: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "kind": "direct", "title": "推导（续 2/2）：极限情形", "type": "term", "term": term }),
    ))
}

#[test]
fn the_term_role_comes_from_the_exact_name_and_the_placement() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let eq = head(&mut mcp, &list, "均衡");
    let other_head = head(&mut mcp, &list, "弹性");
    let eq_part = part(&mut mcp, &eq, "均衡");
    let eq_next = part(&mut mcp, &eq_part, "均衡");
    // A Side successor of a head is not a continuation, even with the same name.
    let branched = address(mcp.ok(
        "create_successor",
        json!({ "at": eq, "kind": "side", "title": "均衡", "type": "term", "term": "均衡" }),
    ));
    // A Direct successor with another name continues nothing.
    let renamed = address(mcp.ok(
        "create_successor",
        json!({ "at": other_head, "kind": "direct", "title": "利率", "type": "term", "term": "利率" }),
    ));
    // A named Term Card under an ordinary Card belongs to no list.
    let outside_list = address(mcp.ok(
        "create_successor",
        json!({ "at": anchor, "title": "普通卡", "body": "正文。" }),
    ));
    let orphan = address(mcp.ok(
        "create_successor",
        json!({ "at": outside_list, "kind": "side", "title": "散落", "type": "term", "term": "散落" }),
    ));
    // A non-Term Card has no role at all.
    let plain = mcp.ok("get_card", json!({ "at": outside_list }));

    let role = |mcp: &mut Mcp, at: &str| {
        let card = mcp.ok("get_card", json!({ "at": at }));
        (
            card["term_role"].clone(),
            card["term_list"].clone(),
            card["term_uses"].clone(),
        )
    };
    assert_eq!(
        role(&mut mcp, &list),
        (json!("list"), json!(list), json!(0)),
        "a list owns itself"
    );
    assert_eq!(
        role(&mut mcp, &eq),
        (json!("card"), json!(list), json!(0)),
        "a head is the wire role card"
    );
    for continued in [&eq_part, &eq_next] {
        assert_eq!(
            role(&mut mcp, continued),
            (json!("part"), json!(list), json!(0)),
            "a direct successor of a same-named head or part is a part"
        );
    }
    for unplaced in [&branched, &renamed, &orphan] {
        assert_eq!(
            role(&mut mcp, unplaced),
            (json!("unfiled"), json!(null), json!(0)),
            "no valid list/head/part placement is unfiled"
        );
    }
    assert_eq!(plain["term_role"], json!(null), "a non-Term Card has no role");
    assert_eq!(plain["term_list"], json!(null));

    // The same facts appear in the tree and bulk reads.
    let tree = mcp.ok("get_tree", json!({ "tree": "Test2026" }));
    let row = |tree: &Value, at: &str| {
        tree["cards"]
            .as_array()
            .unwrap()
            .iter()
            .find(|card| card["address"] == at)
            .cloned()
            .expect("tree row")
    };
    assert_eq!(row(&tree, &eq)["term_role"], json!("card"));
    assert_eq!(row(&tree, &eq)["term_list"], json!(list));
    assert_eq!(row(&tree, &eq_part)["term_role"], json!("part"));
    let bulk = mcp.ok("get_cards", json!({ "addresses": [eq, eq_part, eq_next] }));
    let cards = bulk["cards"].as_array().expect("cards");
    assert_eq!(cards[0]["term_role"], json!("card"));
    assert_eq!(cards[1]["term_role"], json!("part"));
    assert_eq!(cards[2]["term_role"], json!("part"));
}

#[test]
fn get_terms_names_a_list_by_itself_a_descendant_or_an_unambiguous_tree() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let eq = head(&mut mcp, &list, "均衡");
    let eq_part = part(&mut mcp, &eq, "均衡");

    // The list itself, any Card under it, and the book's single list name the same read.
    for at in [&list, &eq, &eq_part] {
        assert_eq!(
            mcp.ok("get_terms", json!({ "at": at }))["list"],
            json!(list),
            "{at}"
        );
    }
    assert_eq!(
        mcp.ok("get_terms", json!({ "at": "Test2026" }))["list"],
        json!(list),
        "a tree with one Term list selects it"
    );

    // A Card outside the list names no list, and a missing Card says so.
    let unrelated = address(mcp.ok(
        "create_successor",
        json!({ "at": anchor, "title": "另一张", "body": "正文。" }),
    ));
    let error = mcp.err("get_terms", json!({ "at": unrelated }));
    assert!(
        error.contains("is not a Term list and is not under one"),
        "{error}"
    );
    let error = mcp.err("get_terms", json!({ "at": "Test2026/999" }));
    assert!(error.contains("does not exist"), "{error}");

    // A tree with two lists is ambiguous: the read names both and selects neither.
    let second = address(mcp.ok(
        "create_successor",
        json!({ "at": anchor, "kind": "side", "title": "术语表：二", "type": "term", "body": TABLE }),
    ));
    let error = mcp.err("get_terms", json!({ "at": "Test2026" }));
    assert!(
        error.contains("2 Term lists") && error.contains(&list) && error.contains(&second),
        "{error}"
    );
    // Naming one of them stays unambiguous.
    assert_eq!(
        mcp.ok("get_terms", json!({ "at": &second }))["list"],
        json!(second)
    );

    // The read is whole: a page bound is refused rather than silently cutting it.
    let error = mcp.err("get_terms", json!({ "at": &list, "limit": 1 }));
    assert!(error.contains("takes no page bound"), "{error}");
    let error = mcp.err("get_terms", json!({ "at": &list, "entry_offset": 0 }));
    assert!(error.contains("remove `entry_offset`"), "{error}");
}

/// Put Term uses into one created Card's body and its link index, as a later release's writer
/// stores them: the macro form `[[label@target]]` and one `kind = 'term'` index row per use.
fn seed_term_uses(home: &TestHome, source: &str, uses: &[(&str, &str)]) {
    let body = uses
        .iter()
        .map(|(label, target)| format!("[[{label}@{target}]]"))
        .collect::<Vec<_>>()
        .join(" 与 ");
    let db = home.db();
    db.execute(
        "UPDATE cards SET text = ?1 WHERE location = ?2",
        rusqlite::params![format!("# 使用者\n\n{body}。"), source],
    )
    .expect("seed the source body");
    for (ord, (_, target)) in uses.iter().enumerate() {
        db.execute(
            "INSERT INTO links(source, ord, target, kind) VALUES(?1, ?2, ?3, 'term')",
            rusqlite::params![source, ord as i64, target],
        )
        .expect("seed the Term use");
    }
}

#[test]
fn raw_duplicate_rows_are_reported_before_compatibility_dedup_and_names_stay_exact() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let table = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 意思一 | §1 |\n| 均衡 | 平衡 | 意思二 | §2 |\n| gdp | 国民生产总值 | 产出 | §3 |\n| GDP | 国内生产总值 | 产出 | §4 |\n";
    let (_anchor, list) = book_with_table(&home, &mut mcp, "Dup2026", table);
    let eq = head(&mut mcp, &list, "均衡");
    let upper = head(&mut mcp, &list, "GDP");
    let lower = head(&mut mcp, &list, "gdp");

    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    // The compatibility reader keeps one row per case-folded name, the later spelling winning,
    // so R6 and the digest keep their existing input.
    let rows = terms["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 2, "{rows:#?}");
    assert_eq!(rows[0]["term"], json!("均衡"));
    assert_eq!(rows[0]["meaning"], json!("意思二"));
    assert_eq!(rows[1]["term"], json!("GDP"));
    assert_eq!(rows[1]["defined"], json!("§4"));
    // The raw duplicate rows are reported, not silently erased.
    assert_eq!(
        terms["row_duplicates"],
        json!([
            { "term": "均衡", "spellings": ["均衡", "均衡"], "rows": [1, 2] },
            { "term": "gdp", "spellings": ["gdp", "GDP"], "rows": [3, 4] },
        ])
    );
    assert_eq!(terms["coverage"]["rows"], json!(2));
    assert_eq!(terms["coverage"]["raw_rows"], json!(4));

    // A head is matched to its row by the exact spelling only: `GDP` publishes its row, `gdp`
    // does not publish the same row, and its own row is not silently folded in.
    assert_eq!(rows[1]["heads"], json!([upper]));
    let faults = terms["faults"].as_array().expect("faults");
    assert_eq!(faults.len(), 1, "{faults:#?}");
    assert_eq!(faults[0]["fault"], json!("term_card_without_row"));
    assert_eq!(faults[0]["address"], json!(lower));
    assert_eq!(faults[0]["term"], json!("gdp"));
    // The stored names are exact: no case folding or normalization anywhere.
    let upper_card = mcp.ok("get_card", json!({ "at": &upper }));
    let lower_card = mcp.ok("get_card", json!({ "at": &lower }));
    assert_eq!(upper_card["term"], json!("GDP"));
    assert_eq!(upper_card["term_role"], json!("card"));
    assert_eq!(lower_card["term"], json!("gdp"));
    assert_eq!(lower_card["term_role"], json!("card"));
    assert_ne!(upper, lower, "each spelling keeps its own head");
    assert_ne!(eq, upper);
}

#[test]
fn a_nested_list_reads_separately_and_a_named_term_card_with_no_list_is_refused() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (_anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let eq = head(&mut mcp, &list, "均衡");
    // A second Term list inside the first one's subtree: it is a list of its own, so the outer
    // read neither counts it nor reads its head as one of its own.
    let nested = address(mcp.ok(
        "create_successor",
        json!({ "at": &list, "kind": "side", "title": "术语表：分册", "type": "term", "body": TABLE }),
    ));
    let nested_head = head(&mut mcp, &nested, "均衡");
    let outer = mcp.ok("get_terms", json!({ "at": &list }));
    assert_eq!(outer["heads"].as_array().unwrap().len(), 1, "{outer}");
    assert_eq!(outer["heads"][0]["address"], json!(eq));
    let inner = mcp.ok("get_terms", json!({ "at": &nested }));
    assert_eq!(inner["list"], json!(nested));
    assert_eq!(inner["heads"][0]["address"], json!(nested_head));
    // Any Card under a list names that list, including an unfiled one.
    let unfiled = address(mcp.ok(
        "create_successor",
        json!({ "at": eq, "kind": "side", "title": "均衡", "type": "term", "term": "均衡" }),
    ));
    assert_eq!(
        mcp.ok("get_terms", json!({ "at": &unfiled }))["list"],
        json!(list)
    );

    // A named Term Card in a tree with no list above it has no structure to read.
    let topic = address(mcp.ok("create_topic", json!({ "title": "散落" })));
    let idea = address(mcp.ok(
        "create_successor",
        json!({ "at": &topic, "title": "普通卡", "body": "正文。" }),
    ));
    let lonely = address(mcp.ok(
        "create_successor",
        json!({ "at": &idea, "kind": "side", "title": "孤立", "type": "term", "term": "孤立" }),
    ));
    let error = mcp.err("get_terms", json!({ "at": &lonely }));
    assert!(
        error.contains("is not a Term list and is not under one"),
        "{error}"
    );
    assert_eq!(
        mcp.ok("get_card", json!({ "at": lonely }))["term_role"],
        json!("unfiled")
    );
    // A tree with no Term list says so rather than answering an empty structure.
    let error = mcp.err("get_terms", json!({ "at": &topic }));
    assert!(error.contains("has no Term list"), "{error}");
}

#[test]
fn a_placement_chain_past_the_depth_limit_fails_visibly() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (_anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let _eq = head(&mut mcp, &list, "均衡");
    // A hand-made placement chain deeper than the read's bound: each Card continues the one
    // before it, and every Card is a valid Location, so only the bound refuses the read.
    let mut address = format!("{list}|a");
    let db = home.db();
    for _ in 0..130 {
        address.push_str("|1");
        db.execute(
            "INSERT INTO cards(location, is_topic, is_lit, text, workspace, card_type, term_name)
             VALUES(?1, 0, 0, ?2, 'agent', 'term', '均衡')",
            rusqlite::params![address, "# 均衡\n\n<!-- zt:reverse-links -->\n"],
        )
        .expect("seed one deep continuation");
    }
    let error = mcp.call("get_terms", json!({ "at": &list }));
    assert!(error.1, "the read must fail rather than cut the chain: {}", error.0);
    assert_eq!(error.0["code"], json!("term_placement"), "{}", error.0);
    assert_eq!(error.0["reason"], json!("depth"), "{}", error.0);
    assert!(
        error.0["error"]
            .as_str()
            .unwrap()
            .contains("longer than 128 steps"),
        "{}",
        error.0
    );
    // The head itself still reads: only a Card whose chain crosses the bound is refused.
    let eq = mcp.ok("get_card", json!({ "at": "Test2026/1|a|a" }));
    assert_eq!(eq["term_role"], json!("card"));
}

#[test]
fn get_terms_reports_the_seven_structure_faults() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (anchor, list) = book_with_a_term_list(&home, &mut mcp);
    // Two heads for one row; a head in no row; a continuation part in the right place; and an
    // unfiled Side successor of a head.
    let eq1 = head(&mut mcp, &list, "均衡");
    let eq2 = head(&mut mcp, &list, "均衡");
    let rate = head(&mut mcp, &list, "利率");
    let eq_part = part(&mut mcp, &eq1, "均衡");
    let rate_part = part(&mut mcp, &rate, "利率差");
    let unfiled = address(mcp.ok(
        "create_successor",
        json!({ "at": eq1, "kind": "side", "title": "均衡", "type": "term", "term": "均衡" }),
    ));
    // Stored Term uses: one good, one label mismatch, one part target, one missing target, and
    // one from outside this book, which counts as a use and is no fault of this list.
    let source_a = address(mcp.ok(
        "create_successor",
        json!({ "at": anchor, "title": "使用者甲", "body": "正文。" }),
    ));
    let source_b = address(mcp.ok(
        "create_successor",
        json!({ "at": anchor, "kind": "side", "title": "使用者乙", "body": "正文。" }),
    ));
    seed_term_uses(
        &home,
        &source_a,
        &[("均衡", &eq1), ("均衡", &eq_part), ("幽灵", "Test2026/1|a|zz")],
    );
    seed_term_uses(&home, &source_b, &[("平衡", &eq1)]);
    let topic = address(mcp.ok("create_topic", json!({ "title": "别处" })));
    let foreign = address(mcp.ok(
        "create_successor",
        json!({ "at": topic, "title": "外部使用者", "body": "正文。" }),
    ));
    // A use from another tree counts, and a wrong label is reported whoever writes it.
    seed_term_uses(&home, &foreign, &[("平衡", &eq1)]);

    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    let faults = terms["faults"].as_array().expect("faults");
    let names = faults
        .iter()
        .map(|fault| fault["fault"].as_str().unwrap())
        .collect::<Vec<_>>();
    let of = |name: &str| {
        faults
            .iter()
            .filter(|fault| fault["fault"] == name)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names,
        [
            "term_unfiled",
            "term_duplicate",
            "term_row_without_card",
            "term_card_without_row",
            "term_part_name",
            "term_label_mismatch",
            "term_label_mismatch",
            "term_link_target",
            "term_link_target",
        ],
        "{faults:#?}"
    );
    assert_eq!(of("term_unfiled")[0]["address"], json!(unfiled));
    assert_eq!(of("term_duplicate")[0]["term"], json!("均衡"));
    assert_eq!(of("term_duplicate")[0]["heads"], json!([eq1, eq2]));
    assert_eq!(of("term_row_without_card")[0]["row"], json!(2));
    assert_eq!(of("term_row_without_card")[0]["term"], json!("弹性"));
    assert_eq!(of("term_card_without_row")[0]["address"], json!(rate));
    assert_eq!(of("term_card_without_row")[0]["term"], json!("利率"));
    assert_eq!(of("term_part_name")[0]["address"], json!(rate_part));
    assert_eq!(of("term_part_name")[0]["term"], json!("利率差"));
    assert_eq!(of("term_part_name")[0]["expected"], json!("利率"));
    let mismatches = of("term_label_mismatch");
    let mut mismatch_sources = mismatches
        .iter()
        .map(|fault| fault["source"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    mismatch_sources.sort();
    let mut expected_sources = vec![source_b.clone(), foreign.clone()];
    expected_sources.sort();
    assert_eq!(mismatch_sources, expected_sources, "{mismatches:#?}");
    for mismatch in &mismatches {
        assert_eq!(mismatch["label"], json!("平衡"));
        assert_eq!(mismatch["term"], json!("均衡"));
    }
    let targets = of("term_link_target");
    assert_eq!(targets[0]["source"], json!(source_a));
    assert_eq!(targets[0]["target"], json!(eq_part));
    assert_eq!(targets[0]["reason"], json!("not_head"));
    assert_eq!(targets[1]["target"], json!("Test2026/1|a|zz"));
    assert_eq!(targets[1]["reason"], json!("missing"));

    // The uses themselves: every head's uses are counted, whoever writes them.
    assert_eq!(terms["coverage"]["uses"], json!(3));
    let uses = terms["uses"].as_array().expect("uses");
    let pairs = uses
        .iter()
        .map(|row| {
            (
                row["source"].as_str().unwrap().to_string(),
                row["label"].clone(),
                row["count"].clone(),
            )
        })
        .collect::<Vec<_>>();
    assert!(pairs.contains(&(source_a.clone(), json!("均衡"), json!(1))));
    assert!(pairs.contains(&(source_b.clone(), json!("平衡"), json!(1))));
    assert!(pairs.contains(&(foreign.clone(), json!("平衡"), json!(1))));
    let heads = terms["heads"].as_array().expect("heads");
    let eq_row = heads
        .iter()
        .find(|head| head["address"] == json!(eq1))
        .expect("the head row");
    assert_eq!(eq_row["uses"], json!(3));
    assert_eq!(eq_row["parts"], json!([eq_part]));

    // A valid head needs no other row to exist (S08-AC07): the 弹性 row is missing, yet both
    // 均衡 heads and the 利率 head are read as valid.
    assert_eq!(terms["coverage"]["published"], json!(1));
    assert_eq!(terms["coverage"]["missing"], json!(1));
    assert_eq!(
        terms["unfiled"].as_array().unwrap()[0]["address"],
        json!(unfiled)
    );
}

#[test]
fn get_terms_reads_the_rows_and_heads_of_one_list() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let (_anchor, list) = book_with_a_term_list(&home, &mut mcp);
    let eq = head(&mut mcp, &list, "均衡");

    let terms = mcp.ok("get_terms", json!({ "at": list }));
    assert_eq!(terms["list"], json!(list));
    assert_eq!(terms["title"], json!("术语表：测试"));
    assert_eq!(terms["workspace"], json!("agent"));
    assert_eq!(terms["tree"], json!("Test2026"));
    assert_eq!(terms["complete"], json!(true));
    assert!(terms["archive_revision"].as_u64().is_some());

    // Rows keep the table's source order, all four columns, and which head publishes each row.
    let rows = terms["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["row"], json!(1));
    assert_eq!(rows[0]["term"], json!("均衡"));
    assert_eq!(rows[0]["avoid"], json!(["平衡"]));
    assert_eq!(rows[0]["meaning"], json!("双方都不改变策略"));
    assert_eq!(rows[0]["defined"], json!("§1.1"));
    assert_eq!(rows[0]["heads"], json!([eq]));
    assert_eq!(rows[0]["published"], json!(true));
    assert_eq!(rows[1]["term"], json!("弹性"));
    assert_eq!(rows[1]["heads"], json!([]));
    assert_eq!(rows[1]["published"], json!(false));

    // A head is the wire role `card`; a valid head needs no other row to exist (S08-AC07).
    let heads = terms["heads"].as_array().expect("heads");
    assert_eq!(heads.len(), 1);
    assert_eq!(heads[0]["address"], json!(eq));
    assert_eq!(heads[0]["term"], json!("均衡"));
    assert_eq!(heads[0]["title"], json!("均衡"));
    assert_eq!(heads[0]["role"], json!("card"));
    assert_eq!(heads[0]["parts"], json!([]));
    assert_eq!(heads[0]["uses"], json!(0));
    assert_eq!(terms["parts"], json!([]));
    assert_eq!(terms["unfiled"], json!([]));
    assert_eq!(terms["uses"], json!([]));
    assert_eq!(terms["row_duplicates"], json!([]));

    // The one coverage fault: the second row has no head. The head's own row is complete.
    let faults = terms["faults"].as_array().expect("faults");
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert_eq!(faults[0]["fault"], json!("term_row_without_card"));
    assert_eq!(faults[0]["row"], json!(2));
    assert_eq!(faults[0]["term"], json!("弹性"));

    assert_eq!(
        terms["coverage"],
        json!({
            "rows": 2,
            "raw_rows": 2,
            "published": 1,
            "missing": 1,
            "heads": 1,
            "parts": 0,
            "unfiled": 0,
            "uses": 0,
        })
    );
    assert!(
        terms["archive_revision"].as_u64().unwrap() > 0,
        "the read names a real archive revision: {terms}"
    );
}
