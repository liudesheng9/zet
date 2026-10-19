//! Hub Cards: a way into one subject, with a hub level, over MCP.

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

    fn facet(&self, address: &str) -> Option<String> {
        Connection::open(self.archive_root.join("zt.sqlite3"))
            .expect("open db")
            .query_row(
                "SELECT model_facet FROM cards WHERE location = ?1",
                [address],
                |row| row.get(0),
            )
            .expect("card facet")
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

    fn err(&mut self, name: &str, arguments: Value) -> String {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(is_error, "{name}({arguments}) should fail: {value}");
        value["error"].as_str().expect("error message").to_string()
    }
}

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

fn hub_level(home: &TestHome, address: &str) -> Option<String> {
    Connection::open(home.archive_root.join("zt.sqlite3"))
        .expect("open db")
        .query_row(
            "SELECT hub_level FROM cards WHERE location = ?1",
            [address],
            |row| row.get(0),
        )
        .expect("card hub level")
}

fn faults(report: &Value) -> Vec<String> {
    report["faults"]
        .as_array()
        .expect("faults")
        .iter()
        .map(|fault| fault["fault"].as_str().unwrap().to_string())
        .collect()
}

/// A Topic with a main line of `n` Cards; returns the Topic number.
fn topic(mcp: &mut Mcp, title: &str, n: usize) -> String {
    let root = address(mcp.ok("create_topic", json!({ "title": title, "body": "问题。" })));
    let tree = root.trim_end_matches("/0").to_string();
    let mut at = root.clone();
    for i in 1..=n {
        at = address(mcp.ok(
            "create_successor",
            json!({ "at": at, "title": format!("{title} 第 {i} 张卡") }),
        ));
    }
    tree
}

#[test]
fn a_hub_card_carries_a_hub_level_and_only_a_hub_card_does() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "信息不对称", 3);
    let hub = mcp.ok(
        "create_successor",
        json!({ "at": format!("{tree}/1"), "kind": "side", "type": "hub", "level": "topic", "title": "信息不对称的入口", "body": format!("## 后果\n- [[{tree}/2]]：逆向选择让市场萎缩。\n") }),
    );
    assert_eq!(hub["type"], "hub");
    assert_eq!(hub["level"], "topic");
    let hub = address(hub);
    assert_eq!(hub_level(&home, &hub).as_deref(), Some("topic"));
    assert_eq!(mcp.ok("get_card", json!({ "at": hub }))["level"], "topic");

    // A hub level needs a Hub Card, and an elaboration level a Model Card.
    let refusal = mcp.err(
        "edit_card",
        json!({ "at": format!("{tree}/2"), "level": "line" }),
    );
    assert!(
        refusal.contains("only a Hub Card has a hub level"),
        "{refusal}"
    );
    let refusal = mcp.err("edit_card", json!({ "at": hub, "level": "formal" }));
    assert!(refusal.contains("elaboration level"), "{refusal}");

    // The Chinese name works, and a Card that stops being a hub loses its level.
    mcp.ok("edit_card", json!({ "at": hub, "level": "线索" }));
    assert_eq!(hub_level(&home, &hub).as_deref(), Some("line"));
    mcp.ok("edit_card", json!({ "at": hub, "type": "idea" }));
    assert_eq!(hub_level(&home, &hub), None);
    let found = mcp.ok("search_cards", json!({ "type": "hub" }));
    assert_eq!(found["results"].as_array().unwrap().len(), 0);
}

#[test]
fn a_register_with_declared_parts_needs_no_topic_hub_of_its_own() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "被分册的领域", 2);
    let one = format!("{tree}/1");
    // A register with eight declared parts: more non-hub Cards than `topic_min` in the 索引 tree.
    // Each part is an immediate Side successor holding `词条` rows (S11, #104, D188).
    let index = address(mcp.ok(
        "create_topic",
        json!({ "title": "索引", "body": "本主题收放代理工作区的入口。" }),
    ));
    let reg = address(mcp.ok(
        "create_successor",
        json!({ "at": index, "type": "hub", "level": "register", "title": "索引：代理工作区的入口", "body": "范围：代理工作区的入口。" }),
    ));
    let mut declarations = String::new();
    for i in 0..8 {
        let part = address(mcp.ok(
            "create_successor",
            json!({ "at": reg, "kind": "side", "title": format!("词条 其他{i}"), "body": format!("## 词条\n- 词条{i}：[[{one}]]\n") }),
        ));
        declarations.push_str(&format!("- [[{part}]] 其他\n"));
    }
    mcp.ok(
        "edit_card",
        json!({ "at": reg, "body": format!("范围：代理工作区的入口。\n\n## 词条分册\n{declarations}") }),
    );
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(view["entry_total"], json!(8), "{view}");
    assert_eq!(view["complete"], json!(true), "{view}");
    // The parts are navigation: the 索引 tree still needs no topic hub of its own, and a part is
    // not a branch that would need a line hub either.
    let graph = mcp.ok("list_hubs", json!({ "workspace": "agent", "topic_min": 3 }));
    let missing = graph["missing"].as_array().expect("missing hubs");
    let index_tree = index.split('/').next().expect("tree id");
    assert!(
        !missing
            .iter()
            .any(|row| row["tree"].as_str() == Some(index_tree)),
        "the register tree requires no topic hub: {missing:?}"
    );
    assert!(
        !missing.iter().any(|row| row["level"] == "line"),
        "a declared part is no branch: {missing:?}"
    );
}

#[test]
fn get_hub_reports_faults_and_list_hubs_finds_the_missing_hubs() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let a = topic(&mut mcp, "逆向选择", 8);
    let b = topic(&mut mcp, "信号传递", 3);
    // Tree b links tree a three times: they are neighbours.
    for i in 1..=3 {
        mcp.ok(
            "edit_card",
            json!({ "at": format!("{b}/{i}"), "body": format!("对照 [[{a}/{i}]]：同一个市场。") }),
        );
    }
    let missing = mcp.ok("list_hubs", json!({ "workspace": "agent" }));
    let missing = missing["missing"].as_array().unwrap();
    assert!(
        missing
            .iter()
            .any(|m| m["level"] == "topic" && m["tree"] == a.as_str()),
        "{missing:?}"
    );
    assert!(!missing.iter().any(|m| m["tree"] == b.as_str()));
    // The run's own limits change what is required.
    let strict = mcp.ok("list_hubs", json!({ "workspace": "agent", "topic_min": 3 }));
    assert!(
        strict["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["tree"] == b.as_str())
    );

    // A topic hub that copies its own tree in address order.
    let lines = (1..=5)
        .map(|i| format!("- [[{a}/{i}]]：第 {i} 张卡说明逆向选择的一步。"))
        .collect::<Vec<_>>()
        .join("\n");
    let hub = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{a}/1"), "kind": "side", "type": "hub", "level": "topic", "title": "逆向选择的入口", "body": format!("## 主线\n{lines}\n- [[{a}/6]]：短\n") }),
    ));
    let report = mcp.ok("get_hub", json!({ "at": hub }));
    assert_eq!(report["level"], "topic");
    assert_eq!(report["lines"].as_array().unwrap().len(), 6);
    assert_eq!(report["mirror"], true);
    let found = faults(&report);
    for expected in ["bare", "unanchored", "isolated"] {
        assert!(
            found.contains(&expected.to_string()),
            "{expected}: {found:?}"
        );
    }
    assert!(
        report["unreached"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a_card| a_card == &json!(format!("{a}/7")))
    );

    // The Topic body links its hub, the hub names its neighbour, and a register lists it.
    mcp.ok(
        "edit_card",
        json!({ "at": format!("{a}/0"), "body": format!("问题。\n\n入口见 [[{hub}]]。") }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": hub, "body": format!("## 主线\n{lines}\n- [[{a}/6]]：第六张卡给出均衡。\n\n## 邻近\n- 对照 [[{b}/1]]：信号传递是知情一方的应对。\n") }),
    );
    let index = topic(&mut mcp, "索引", 0);
    let register = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{index}/0"), "type": "hub", "level": "register", "title": "索引", "body": format!("- 逆向选择：[[{hub}]]\n") }),
    ));
    let report = mcp.ok("get_hub", json!({ "at": hub }));
    assert_eq!(faults(&report), Vec::<String>::new(), "{report}");
    assert_eq!(report["mirror"], false);
    assert_eq!(report["upper"][0]["address"], register.as_str());
    assert_eq!(
        faults(&mcp.ok("get_hub", json!({ "at": register }))),
        Vec::<String>::new()
    );

    // A Card names the hubs that list it.
    let card = mcp.ok("get_card", json!({ "at": format!("{a}/2") }));
    assert_eq!(card["hubs"][0]["address"], hub.as_str());
    assert_eq!(card["hubs"][0]["level"], "topic");

    // A register lists only lower hubs; a hub that lists a higher one is upward.
    mcp.ok(
        "edit_card",
        json!({ "at": hub, "body": format!("## 主线\n{lines}\n- [[{a}/6]]：第六张卡给出均衡。\n\n## 邻近\n- 对照 [[{b}/1]]：信号传递是知情一方的应对。\n- [[{register}]]：全部入口的索引。\n") }),
    );
    let found = faults(&mcp.ok("get_hub", json!({ "at": hub })));
    assert!(found.contains(&"upward".to_string()), "{found:?}");
    assert!(found.contains(&"cycle".to_string()), "{found:?}");
    let graph = mcp.ok("list_hubs", json!({ "workspace": "agent" }));
    assert_eq!(graph["counts"]["topic"], 1);
    assert_eq!(graph["counts"]["register"], 1);
}

/// The row `list_hubs` reports for one Topic tree's `## 入口` section, if it reads one.
fn entry_section(graph: &Value, tree: &str) -> Option<Value> {
    graph["entries"]
        .as_array()?
        .iter()
        .find(|row| row["tree"].as_str() == Some(tree))
        .cloned()
}

fn missing_tree(graph: &Value, tree: &str) -> bool {
    graph["missing"]
        .as_array()
        .is_some_and(|rows| rows.iter().any(|row| row["tree"].as_str() == Some(tree)))
}

#[test]
fn a_topic_body_entry_section_satisfies_the_topic_hub_requirement() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let a = topic(&mut mcp, "逆向选择", 8);
    // Eight Cards: the tree requires a topic hub.
    let graph = mcp.ok("list_hubs", json!({ "workspace": "agent" }));
    assert!(missing_tree(&graph, &a), "{}", graph["missing"]);
    assert!(entry_section(&graph, &a).is_none(), "{graph}");

    // Four entries are more than the documented one to three: no exception.
    let four = (1..=4)
        .map(|i| format!("- [[{a}/{i}]]：第 {i} 张卡给出逆向选择的一步。"))
        .collect::<Vec<_>>()
        .join("\n");
    mcp.ok(
        "edit_card",
        json!({ "at": format!("{a}/0"), "body": format!("问题。\n\n## 入口\n{four}") }),
    );
    let graph = mcp.ok("list_hubs", json!({ "workspace": "agent" }));
    assert!(missing_tree(&graph, &a), "{}", graph["missing"]);
    let section = entry_section(&graph, &a).expect("a section is read");
    assert_eq!(section["valid"], json!(false));
    assert_eq!(section["fault"], json!("entry-shape"));
    // The missing row says why the entry requirement is still unmet.
    let row = graph["missing"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["tree"].as_str() == Some(a.as_str()))
        .cloned()
        .unwrap();
    assert_eq!(row["entries"]["fault"], json!("entry-shape"));

    // One to three entry Links in the Topic body satisfy the requirement without a hub.
    let three = (1..=3)
        .map(|i| format!("- [[{a}/{i}]]：第 {i} 张卡给出逆向选择的一步。"))
        .collect::<Vec<_>>()
        .join("\n");
    mcp.ok(
        "edit_card",
        json!({ "at": format!("{a}/0"), "body": format!("问题。\n\n## 入口\n{three}") }),
    );
    let graph = mcp.ok("list_hubs", json!({ "workspace": "agent" }));
    assert!(!missing_tree(&graph, &a), "{}", graph["missing"]);
    let section = entry_section(&graph, &a).expect("a section is read");
    assert_eq!(section["valid"], json!(true));
    assert_eq!(
        section["entries"],
        json!([format!("{a}/1"), format!("{a}/2"), format!("{a}/3")])
    );

    // A broken entry restores the advice: a Card the section names is deleted after the fact.
    let doomed = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{a}/3"), "kind": "side", "title": "会消失的入口" }),
    ));
    let two = (1..=2)
        .map(|i| format!("- [[{a}/{i}]]：第 {i} 张卡给出逆向选择的一步。"))
        .collect::<Vec<_>>()
        .join("\n");
    mcp.ok(
        "edit_card",
        json!({ "at": format!("{a}/0"), "body": format!("问题。\n\n## 入口\n{two}\n- [[{doomed}]]：第三张入口卡。") }),
    );
    mcp.ok("delete_card", json!({ "at": doomed, "confirm": "delete" }));
    let graph = mcp.ok("list_hubs", json!({ "workspace": "agent" }));
    assert!(missing_tree(&graph, &a), "{}", graph["missing"]);
    let section = entry_section(&graph, &a).unwrap();
    assert_eq!(section["fault"], json!("entry-target"));
    assert!(
        section["detail"].as_str().unwrap().contains(&doomed),
        "{section}"
    );

    // An entry must be a Card of the tree, and it must say what it adds.
    let outside = topic(&mut mcp, "信号传递", 2);
    mcp.ok(
        "edit_card",
        json!({ "at": format!("{a}/0"), "body": format!("问题。\n\n## 入口\n- [[{outside}/1]]：另一棵树里的卡。") }),
    );
    let graph = mcp.ok("list_hubs", json!({ "workspace": "agent" }));
    assert_eq!(entry_section(&graph, &a).unwrap()["fault"], json!("entry-target"));
    mcp.ok(
        "edit_card",
        json!({ "at": format!("{a}/0"), "body": format!("问题。\n\n## 入口\n- [[{a}/1]]：") }),
    );
    let graph = mcp.ok("list_hubs", json!({ "workspace": "agent" }));
    assert_eq!(entry_section(&graph, &a).unwrap()["fault"], json!("entry-bare"));

    // Removing the section restores the missing-entry advice.
    mcp.ok(
        "edit_card",
        json!({ "at": format!("{a}/0"), "body": "问题。" }),
    );
    let graph = mcp.ok("list_hubs", json!({ "workspace": "agent" }));
    assert!(missing_tree(&graph, &a), "{}", graph["missing"]);
    assert!(entry_section(&graph, &a).is_none(), "{graph}");
}

/// A Literature Card (a book) with `n` Cards under it; returns the Citation key.
fn book(mcp: &mut Mcp, key: &str, title: &str, n: usize) -> String {
    mcp.ok(
        "create_literature",
        json!({ "bibtex": format!("@book{{{key}, title = {{{title}}}}}") }),
    );
    let mut at = key.to_string();
    for i in 1..=n {
        at = address(mcp.ok(
            "create_successor",
            json!({ "at": at, "title": format!("{title} 第 {i} 节") }),
        ));
    }
    key.to_string()
}

#[test]
fn one_book_advice_names_a_supported_treatment_from_another_book() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let a = topic(&mut mcp, "逆向选择", 6);
    let b = book(&mut mcp, "stiglitz2014learning", "学习", 3);
    let other = book(&mut mcp, "acemoglu2019", "技术", 3);
    // Every Card of tree a cites the same book: the subject carries one book's evidence.
    for i in 1..=6 {
        mcp.ok(
            "edit_card",
            json!({ "at": format!("{a}/{i}"), "body": format!("逆向选择让市场萎缩。\n\n来源：[[{b}]] §{i}") }),
        );
    }
    let hub = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{a}/1"), "kind": "side", "type": "hub", "level": "topic",
                "title": "信息不对称市场的入口",
                "body": format!("## 后果\n- [[{a}/2]]：逆向选择让市场萎缩。\n- [[{a}/3]]：均衡可能完全瓦解。\n- [[{a}/4]]：约束帕累托最优是评价标准。\n- [[{a}/5]]：干预只在均衡非约束最优时有用。\n- [[{a}/6]]：多重均衡让干预更难。\n") }),
    ));
    mcp.ok("edit_card", json!({ "at": format!("{a}/0"), "body": format!("问题。\n\n入口见 [[{hub}]]。") }));
    // An accepted register term links the tree's Card to two treatments of one term.
    let index = topic(&mut mcp, "索引", 0);
    let register = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{index}/0"), "type": "hub", "level": "register", "title": "索引：代理工作区的入口",
                "body": format!("范围：代理工作区的入口。\n\n## 词条\n- 逆向选择：[[{a}/2]] [[{other}/1]]\n- 多重均衡：[[{a}/6]] [[{b}/2]]\n") }),
    ));
    // Before the other book's treatment is in 邻近: one-book advice names it, with its source.
    let report = mcp.ok("get_hub", json!({ "at": hub }));
    let one = &report["one_book"];
    assert_eq!(one["sources"], json!([b]), "{report}");
    let candidates = one["candidates"].as_array().expect("candidates");
    assert_eq!(candidates.len(), 1, "{report}");
    let candidate = candidates[0].to_string();
    assert!(candidate.contains(&other), "{candidate}");
    assert_eq!(candidates[0]["address"], json!(format!("{other}/1")));
    assert_eq!(candidates[0]["source"], json!(other));
    assert_eq!(candidates[0]["term"], json!("逆向选择"));
    assert_eq!(candidates[0]["member"], json!(format!("{a}/2")));

    // A treatment of the subject's own book is no cross-book candidate.
    assert_eq!(
        one["unknown"].as_array().expect("unknown").len(),
        0,
        "{report}"
    );

    // Once the hub's 邻近 names the other book's treatment, the advice is gone.
    mcp.ok(
        "edit_card",
        json!({ "at": hub, "body": format!("## 后果\n- [[{a}/2]]：逆向选择让市场萎缩。\n- [[{a}/3]]：均衡可能完全瓦解。\n- [[{a}/4]]：约束帕累托最优是评价标准。\n- [[{a}/5]]：干预只在均衡非约束最优时有用。\n- [[{a}/6]]：多重均衡让干预更难。\n\n## 邻近\n- 对照 [[{other}/1]]：技术选择也有逆向选择。\n") }),
    );
    let report = mcp.ok("get_hub", json!({ "at": hub }));
    assert_eq!(
        report["one_book"]["candidates"],
        json!([]),
        "{}",
        report["one_book"]
    );

    // A treatment with unknown source evidence is shown separately, never as an advice.
    let unknown = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{a}/1"), "kind": "side", "title": "没有来源的入口" }),
    ));
    mcp.ok(
        "edit_card",
        json!({ "at": register, "body": format!("范围：代理工作区的入口。\n\n## 词条\n- 逆向选择：[[{a}/2]] [[{unknown}]]\n") }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": hub, "body": format!("## 后果\n- [[{a}/2]]：逆向选择让市场萎缩。\n- [[{a}/3]]：均衡可能完全瓦解。\n- [[{a}/4]]：约束帕累托最优是评价标准。\n- [[{a}/5]]：干预只在均衡非约束最优时有用。\n- [[{a}/6]]：多重均衡让干预更难。\n") }),
    );
    let report = mcp.ok("get_hub", json!({ "at": hub }));
    let one = &report["one_book"];
    assert_eq!(one["candidates"], json!([]), "{one}");
    let unknown_rows = one["unknown"].as_array().expect("unknown");
    assert_eq!(unknown_rows.len(), 1, "{one}");
    assert_eq!(unknown_rows[0]["address"], json!(unknown));
    assert_eq!(unknown_rows[0]["term"], json!("逆向选择"));
}

#[test]
fn a_field_hub_crosses_two_topics_and_names_the_literature_cards_it_needs() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let a = topic(&mut mcp, "逆向选择", 3);
    let b = topic(&mut mcp, "信号传递", 3);
    let other = book(&mut mcp, "acemoglu2019", "技术", 2);
    let index = topic(&mut mcp, "索引", 0);
    let register = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{index}/0"), "type": "hub", "level": "register", "title": "索引：代理工作区的入口",
                "body": "范围：代理工作区的入口。" }),
    ));
    // A field hub under the register: selected treatments of two Topic trees and one
    // substantive Literature-tree Card that explains how the two books state the concept.
    let field = address(mcp.ok(
        "create_successor",
        json!({ "at": register, "kind": "side", "type": "hub", "level": "field",
                "title": "私人信息改变市场均衡的各个入口",
                "body": format!("## 定义\n- [[{a}/1]]：逆向选择让高质量卖者退出市场。\n- [[{b}/1]]：信号让知情者的类型变得可见。\n\n## 需要的一手材料\n- [[{other}/2]]：技术选择也面对同一信息问题。\n") }),
    ));
    mcp.ok(
        "edit_card",
        json!({ "at": register, "body": format!("范围：代理工作区的入口。\n\n## 领域\n- 信息：[[{field}]]\n") }),
    );
    let report = mcp.ok("get_hub", json!({ "at": field }));
    assert_eq!(report["level"], "field");
    // The two Topic trees are the range; the Literature-tree Card is a member, not a whole tree.
    assert_eq!(report["range"], json!(6), "{report}");
    assert!(
        report["unreached"]
            .as_array()
            .unwrap()
            .iter()
            .all(|address| !address.as_str().unwrap().starts_with("acemoglu2019")),
        "{report}"
    );
    // The crossing is one member line of the field hub, and the Card knows the hub lists it.
    let named = report["lines"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line["address"] == json!(format!("{other}/2")));
    assert!(named, "{report}");
    let card = mcp.ok("get_card", json!({ "at": format!("{other}/2") }));
    assert_eq!(card["hubs"][0]["address"], json!(field));
}

#[test]
fn a_hub_that_copies_its_anchor_title_is_named() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let a = topic(&mut mcp, "逆向选择", 4);
    let anchor = mcp.ok("get_card", json!({ "at": format!("{a}/1") }));
    let line = format!("- [[{a}/2]]：第二张卡说明机制。");
    // The title repeats the anchor's title; whitespace is the only difference.
    let copied = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{a}/1"), "kind": "side", "type": "hub", "level": "line",
                "title": "逆向选择  第 1 张卡", "body": format!("## 主线\n{line}") }),
    ));
    let found = faults(&mcp.ok("get_hub", json!({ "at": copied })));
    assert!(found.contains(&"anchor-title".to_string()), "{found:?}");
    assert_eq!(anchor["title"], "逆向选择 第 1 张卡");
    // A title that names the navigation role is no fault.
    mcp.ok("edit_card", json!({ "at": copied, "title": "逆向选择 第 1 张卡的分解入口" }));
    let found = faults(&mcp.ok("get_hub", json!({ "at": copied })));
    assert!(!found.contains(&"anchor-title".to_string()), "{found:?}");
    // The report is read-only: the anchor Card itself is unchanged.
    assert_eq!(mcp.ok("get_card", json!({ "at": format!("{a}/1") }))["title"], anchor["title"]);
}

#[test]
fn a_topic_hub_that_copies_its_topic_title_is_named() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let a = topic(&mut mcp, "逆向选择", 4);
    let line = format!("- [[{a}/2]]：第二张卡说明机制。");
    // A topic hub is filed under <n>/1; a title that repeats its tree's Topic Card says
    // nothing about the way in, and the hub compiler's precheck refuses the same draft.
    let hub = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{a}/1"), "kind": "side", "type": "hub", "level": "topic",
                "title": "逆向选择", "body": format!("## 主线\n{line}") }),
    ));
    let report = mcp.ok("get_hub", json!({ "at": hub }));
    assert!(faults(&report).contains(&"anchor-title".to_string()), "{:?}", faults(&report));
    // The report's anchor stays the Card it hangs under.
    assert_eq!(report["anchor"], format!("{a}/1"));
    // A title that names the navigation role is no fault.
    mcp.ok("edit_card", json!({ "at": hub, "title": "逆向选择市场的分解入口" }));
    let found = faults(&mcp.ok("get_hub", json!({ "at": hub })));
    assert!(!found.contains(&"anchor-title".to_string()), "{found:?}");
    // Copying the title of the Card it hangs under is still reported.
    mcp.ok("edit_card", json!({ "at": hub, "title": "逆向选择 第 1 张卡" }));
    let found = faults(&mcp.ok("get_hub", json!({ "at": hub })));
    assert!(found.contains(&"anchor-title".to_string()), "{found:?}");
    // The report is read-only: the Topic Card itself is unchanged.
    assert_eq!(mcp.ok("get_card", json!({ "at": format!("{a}/0") }))["title"], "逆向选择");
}

#[test]
fn a_line_hub_lists_the_line_hub_of_its_sub_branch_and_neighbours_do_not_count_toward_size() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let a = topic(&mut mcp, "逆向选择", 2);
    let branch = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{a}/2"), "kind": "side", "title": "多重均衡" }),
    ));
    let sub = address(mcp.ok(
        "create_successor",
        json!({ "at": branch, "kind": "side", "title": "约束最优" }),
    ));
    let inner = address(mcp.ok(
        "create_successor",
        json!({ "at": sub, "kind": "side", "type": "hub", "level": "line", "title": "约束最优这一支", "body": format!("## 判据\n- [[{sub}]]：权威看不到类型时的改进标准。\n") }),
    ));
    let outer = address(mcp.ok(
        "create_successor",
        json!({ "at": branch, "kind": "side", "type": "hub", "level": "line", "title": "多重均衡这一支",
                "body": format!("## 均衡\n- [[{branch}]]：逆向选择市场可以有多个均衡。\n- [[{inner}]]（枢纽）：约束最优这一支的入口。\n\n## 邻近\n- 对照 [[{a}/1]]：主线讲市场后果。\n- 对照 [[{a}/2]]：主线讲知情者的决定。\n") }),
    ));
    // Two lines outside 邻近 and one group: within max_lines 2 and max_groups 1.
    let report = mcp.ok(
        "get_hub",
        json!({ "at": outer, "max_lines": 2, "max_groups": 1 }),
    );
    let found = faults(&report);
    assert!(!found.contains(&"upward".to_string()), "{found:?}");
    assert!(!found.contains(&"oversize".to_string()), "{found:?}");
    assert!(!found.contains(&"unlisted".to_string()), "{found:?}");
    // The inner hub may not list the outer one.
    mcp.ok(
        "edit_card",
        json!({ "at": inner, "body": format!("## 判据\n- [[{sub}]]：权威看不到类型时的改进标准。\n- [[{outer}]]：外面那一支的入口。\n") }),
    );
    let found = faults(&mcp.ok("get_hub", json!({ "at": inner })));
    assert!(found.contains(&"upward".to_string()), "{found:?}");
}
