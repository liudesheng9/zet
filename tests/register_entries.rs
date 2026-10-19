//! The concept register's entries: one shared projection, read through `zt mcp` and the GUI.
//!
//! The register is an ordinary Hub Card of level `register`. Its `词条` rows are one term before a
//! colon and one to three Links to Cards that carry the term's substance. `get_register` and
//! `/api/register` expose the same flattened entry view: every target of every term (not only the
//! first), the bounded authored body of each target, origins, declared parts, faults and the
//! archive identity/revision the view was read at. See `src/entry.rs` and
//! `docs/luhmann-comparison-implementation-design.md` §4.1 and §8.3-8.5.

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

    fn err(&mut self, name: &str, arguments: Value) -> String {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(is_error, "{name}({arguments}) should fail: {value}");
        value["error"].as_str().expect("error message").to_string()
    }
}

struct Gui {
    port: u16,
    token: String,
}

impl Gui {
    /// One authenticated API call; the JSON body of the answer.
    fn get(&self, path: &str) -> Value {
        use std::net::TcpStream;
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let port = self.port;
        let request = format!(
            "GET {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: {}\r\nContent-Length: 0\r\n\r\n",
            self.token
        );
        stream.write_all(request.as_bytes()).expect("send request");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        let start = response.find("\r\n\r\n").expect("header end") + 4;
        serde_json::from_str(&response[start..]).expect("JSON response")
    }

    /// One authenticated API call with a JSON body. The owner's GUI may write the human
    /// Workspace; the MCP server cannot, so a human Card fixture comes through here.
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

/// A Topic its number; `n` Cards are its main line.
fn topic(mcp: &mut Mcp, title: &str, n: usize) -> String {
    let root = address(mcp.ok("create_topic", json!({ "title": title, "body": "问题。" })));
    let tree = root.trim_end_matches("/0").to_string();
    let mut at = root;
    for i in 1..=n {
        at = address(mcp.ok(
            "create_successor",
            json!({ "at": at, "title": format!("{title} 第 {i} 张卡") }),
        ));
    }
    tree
}

/// Publish one register Hub Card, the first Card of the Topic `索引`.
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

fn terms(view: &Value) -> Vec<(String, Vec<String>)> {
    view["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|entry| {
            let term = entry["term"].as_str().expect("term").to_string();
            let targets = entry["targets"]
                .as_array()
                .expect("targets")
                .iter()
                .map(|target| target["address"].as_str().expect("target address").to_string())
                .collect();
            (term, targets)
        })
        .collect()
}

#[test]
fn a_register_projects_one_two_and_three_targets_with_bounded_bodies() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "学习与增长", 2);
    let second = format!("{tree}/2");
    let side = address(mcp.ok(
        "create_successor",
        json!({ "at": second, "kind": "side", "title": "学习曲线是干中学的图形", "body": "累计产量提高时单位成本下降。" }),
    ));
    // A long authored body: the projection cuts it at the documented bound and says so.
    let long = "这一段很长。".repeat(400);
    let first = format!("{tree}/1");
    mcp.ok("edit_card", json!({ "at": first, "title": "学习提高产业生产率", "body": long }));

    let reg = register(
        &mut mcp,
        &format!(
            "范围：代理工作区的入口。\n\n## 词条\n- 学习：[[{first}]]\n- 干中学：[[{first}]] [[{second}]]\n- 学习曲线：[[{first}]] [[{second}]] [[{side}]]\n"
        ),
    );

    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(view["register"], json!(reg));
    assert_eq!(view["register_missing"], json!(false));
    assert_eq!(view["complete"], json!(true), "the rows are valid: {view}");
    assert_eq!(
        terms(&view),
        vec![
            ("学习".to_string(), vec![first.clone()]),
            ("干中学".to_string(), vec![first.clone(), second.clone()]),
            (
                "学习曲线".to_string(),
                vec![first.clone(), second.clone(), side.clone()]
            ),
        ],
        "every target of every term is projected, in body order"
    );
    assert_eq!(view["entry_total"], json!(3));
    assert_eq!(view["entry_offset"], json!(0));
    assert_eq!(view["entry_limit"], json!(200));
    assert_eq!(view["entry_next_offset"], Value::Null);
    assert!(
        view["archive_revision"].as_u64().is_some_and(|n| n > 0),
        "the view names its revision: {view}"
    );
    assert_eq!(
        view["archive_identity"]["id"].as_str().map(str::len),
        Some(32)
    );
    assert!(view["archive_identity"]["path"]
        .as_str()
        .is_some_and(|path| path.ends_with("archive")));

    // The third target of the third term is a real Card with its own title and body.
    let third = &view["entries"][2]["targets"][2];
    assert_eq!(third["address"], json!(side));
    assert_eq!(third["title"], json!("学习曲线是干中学的图形"));
    assert_eq!(third["exists"], json!(true));
    assert!(
        third["body"].as_str().is_some_and(|body| !body.is_empty()),
        "a target carries its authored body: {third}"
    );

    // The documented bound: 1,200 characters, and the cut is named with the authored length.
    let cut = &view["entries"][0]["targets"][0];
    assert_eq!(cut["body_chars"], json!(long.chars().count()));
    assert_eq!(cut["body_truncated"], json!(true));
    assert!(
        cut["body"].as_str().unwrap().chars().count() <= 1200,
        "the body is cut at the bound: {}",
        cut["body"].as_str().unwrap().chars().count()
    );
    assert_eq!(view["bounds"]["body_max_chars"], json!(1200));
    assert!(
        view["bounds"]["body_max_lines"].as_u64().is_some_and(|n| n > 0),
        "the body line bound is named: {view}"
    );

    // terms_by_card: the explicit targets of each term, in register order.
    assert_eq!(
        view["terms_by_card"][&first],
        json!(["学习", "干中学", "学习曲线"])
    );

    // One page at a time; `entry_next_offset` names the next page, null the last.
    let page = mcp.ok(
        "get_register",
        json!({ "workspace": "agent", "entry_offset": 1, "entry_limit": 1 }),
    );
    assert_eq!(page["entry_total"], json!(3));
    assert_eq!(page["entry_offset"], json!(1));
    assert_eq!(page["entry_next_offset"], json!(2));
    assert_eq!(terms(&page), vec![("干中学".to_string(), vec![first.clone(), second.clone()])]);
    let last = mcp.ok(
        "get_register",
        json!({ "workspace": "agent", "entry_offset": 2, "entry_limit": 1 }),
    );
    assert_eq!(last["entry_next_offset"], Value::Null);
    // A limit above the maximum is clamped and echoed, not silently ignored.
    let clamped = mcp.ok(
        "get_register",
        json!({ "workspace": "agent", "entry_limit": 5000 }),
    );
    assert_eq!(clamped["entry_limit"], json!(1000));
    assert_eq!(clamped["entry_total"], json!(3));

    // get_hub on a register keeps its report and adds the same projection.
    let hub = mcp.ok("get_hub", json!({ "at": reg }));
    assert_eq!(hub["level"], json!("register"));
    assert!(hub["groups"].as_array().is_some_and(|g| !g.is_empty()), "{hub}");
    assert!(hub["lines"].as_array().is_some_and(|l| l.len() == 3), "{hub}");
    assert_eq!(terms(&hub), terms(&view));
    assert_eq!(hub["entry_total"], json!(3));
    assert_eq!(hub["parts"], json!([]));
    assert_eq!(hub["archive_revision"], view["archive_revision"]);
    assert_eq!(hub["archive_identity"], view["archive_identity"]);

    // The GUI route answers the same view: same addresses, same order, same evidence.
    let gui = home.gui();
    let route = gui.get("/api/register?ws=agent");
    assert_eq!(terms(&route), terms(&view));
    assert_eq!(route["entry_total"], view["entry_total"]);
    assert_eq!(route["archive_revision"], view["archive_revision"]);
    assert_eq!(route["archive_identity"], view["archive_identity"]);
    assert_eq!(route["register"], json!(reg));

    // A workspace must be named.
    mcp.err("get_register", json!({}));
}

#[test]
fn several_accepted_spellings_are_separate_lines_with_the_same_entries() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "多重均衡", 2);
    let one = format!("{tree}/1");
    let two = format!("{tree}/2");
    register(
        &mut mcp,
        &format!(
            "## 词条\n- 多重均衡：[[{one}]] [[{two}]]\n- 均衡的多重性：[[{one}]] [[{two}]]\n"
        ),
    );
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(
        terms(&view),
        vec![
            ("多重均衡".to_string(), vec![one.clone(), two.clone()]),
            ("均衡的多重性".to_string(), vec![one.clone(), two.clone()]),
        ],
        "two accepted spellings stay two register lines with the same entry Cards"
    );
    assert_eq!(
        view["terms_by_card"][&one],
        json!(["多重均衡", "均衡的多重性"]),
        "the entry Cards carry both spellings, so search needs no alias table"
    );
    // A second target is a hub member too: `hubs_of` reports the term and the target index.
    let card = mcp.ok("get_card", json!({ "at": two }));
    let listed = card["hubs"]
        .as_array()
        .expect("hubs")
        .iter()
        .filter(|hub| hub["level"] == json!("register"))
        .collect::<Vec<_>>();
    assert!(
        listed.len() == 2
            && listed.iter().all(|hub| hub["target_index"] == json!(1))
            && listed.iter().any(|hub| hub["term"] == json!("均衡的多重性")),
        "a term's second target is a member of the register: {listed:?}"
    );
}

#[test]
fn a_missing_target_is_named_and_the_entry_stays_incomplete() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "学习", 2);
    let one = format!("{tree}/1");
    let two = format!("{tree}/2");
    // A Side successor is deleted after the register names it, so the row keeps its address.
    let gone = address(mcp.ok(
        "create_successor",
        json!({ "at": one, "kind": "side", "title": "消失的卡" }),
    ));
    let another = address(mcp.ok(
        "create_successor",
        json!({ "at": one, "kind": "side", "title": "留下的卡" }),
    ));
    register(
        &mut mcp,
        &format!(
            "## 词条\n- 学习：[[{one}]] [[{gone}]]\n- 空词条：\n- 太多：[[{one}]] [[{two}]] [[{gone}]] [[{another}]]\n- 重复：[[{one}]] [[{one}]]\n"
        ),
    );
    mcp.ok("delete_card", json!({ "at": gone, "confirm": "delete" }));
    let missing = gone;
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    // The broken row stays visible, marked incomplete, with its missing target named.
    let broken = view["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|entry| entry["term"] == json!("学习"))
        .expect("the broken term is still an entry");
    assert_eq!(broken["complete"], json!(false));
    assert_eq!(terms(&view)[0], ("学习".to_string(), vec![
        one.clone(),
        missing.clone()
    ]));
    let gap = &broken["targets"][1];
    assert_eq!(gap["address"], json!(missing));
    assert_eq!(gap["exists"], json!(false));
    assert!(gap["title"].is_null() && gap["body"].is_null(), "{gap}");
    assert!(
        broken["faults"]
            .as_array()
            .is_some_and(|faults| faults.iter().any(|fault| fault["fault"] == json!("broken"))),
        "the broken target is a fault of the entry: {broken}"
    );
    // The duplicate row is an entry too, deduplicated and incomplete.
    let duplicate = view["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["term"] == json!("重复"))
        .expect("the duplicate row is an entry");
    assert_eq!(duplicate["complete"], json!(false));
    assert_eq!(duplicate["targets"].as_array().unwrap().len(), 1);
    // A row that names no Link and a row that names more than three are reported, not entries.
    assert_eq!(view["complete"], json!(false));
    let faults = view["faults"].as_array().expect("faults");
    assert!(
        faults.iter().any(|fault| fault["fault"] == json!("empty") && fault["term"] == json!("空词条")),
        "a term row without a Link is an empty row: {faults:?}"
    );
    assert!(
        faults.iter().any(|fault| fault["fault"] == json!("oversize") && fault["term"] == json!("太多")),
        "a term row with four targets is oversize: {faults:?}"
    );
    // The same rows are faults of the Hub Card report, and every target is validated.
    let hub = mcp.ok("get_hub", json!({ "at": view["register"].clone() }));
    let names = hub["faults"]
        .as_array()
        .expect("hub faults")
        .iter()
        .map(|fault| fault["fault"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    for name in ["broken", "empty", "oversize", "duplicate-target"] {
        assert!(names.contains(&name.to_string()), "{name} missing: {names:?}");
    }
    assert!(
        hub["faults"].as_array().unwrap().iter().any(|fault| {
            fault["fault"] == json!("broken")
                && fault["detail"]
                    .as_str()
                    .is_some_and(|detail| detail.contains(&missing))
        }),
        "the broken detail names the second target: {hub}"
    );
}

#[test]
fn a_declared_part_is_projected_with_its_own_origin() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "学习", 2);
    let one = format!("{tree}/1");
    let two = format!("{tree}/2");
    let reg = register(&mut mcp, &format!("## 词条\n- 学习：[[{one}]]\n"));
    // A part is an ordinary Idea Card, a Side successor of the register, declared under 词条分册.
    let part = address(mcp.ok(
        "create_successor",
        json!({
            "at": reg,
            "kind": "side",
            "title": "词条分册：干中学",
            "body": format!("## 词条\n- 干中学：[[{two}]]\n"),
        }),
    ));
    mcp.ok(
        "edit_card",
        json!({ "at": reg, "body": format!("## 词条\n- 学习：[[{one}]]\n\n## 词条分册\n- [[{part}]] A–F\n") }),
    );
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(
        terms(&view),
        vec![
            ("学习".to_string(), vec![one.clone()]),
            ("干中学".to_string(), vec![two.clone()]),
        ],
        "a declared part's rows are flattened after the root's rows"
    );
    assert_eq!(view["entries"][1]["origin"]["part"], json!(part));
    assert_eq!(view["entries"][0]["origin"]["part"], Value::Null);
    assert_eq!(view["entries"][1]["origin"]["card"], json!(part));
    assert_eq!(view["parts"][0]["address"], json!(part));
    assert_eq!(view["parts"][0]["place"], json!("ok"));
    assert_eq!(view["entry_total"], json!(2));
    // A part that is not an immediate Side successor of the register is reported, not accepted.
    let moved = address(mcp.ok(
        "create_successor",
        json!({ "at": two, "kind": "side", "title": "词条分册：误放", "body": format!("## 词条\n- 误放：[[{one}]]\n") }),
    ));
    mcp.ok(
        "edit_card",
        json!({ "at": reg, "body": format!("## 词条\n- 学习：[[{one}]]\n\n## 词条分册\n- [[{part}]] A–F\n- [[{moved}]] 误放\n") }),
    );
    let moved_view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(
        moved_view["complete"],
        json!(false),
        "an unplaced part makes the view incomplete"
    );
    assert!(
        moved_view["faults"].as_array().is_some_and(|faults| faults.iter().any(|fault| {
            fault["fault"] == json!("part-place")
                && fault["detail"].as_str().is_some_and(|detail| detail.contains(&moved))
        })),
        "the unplaced part is named: {}",
        moved_view["faults"]
    );
    assert!(
        terms(&moved_view).iter().all(|(term, _)| term != "误放"),
        "an unplaced part's rows are not accepted"
    );
}

#[test]
fn a_register_written_before_the_term_heading_is_still_read() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "旧词", 2);
    let one = format!("{tree}/1");
    let two = format!("{tree}/2");
    // Registers written before the `## 词条` heading (or by the older compiler under 关键词)
    // keep their rows readable; the grammar is the row, not the heading.
    register(&mut mcp, &format!("范围：旧格式。\n\n- 旧词：[[{one}]] [[{two}]]\n"));
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(
        terms(&view),
        vec![("旧词".to_string(), vec![one.clone(), two.clone()])]
    );
    let hub = mcp.ok("get_hub", json!({ "at": view["register"].clone() }));
    assert_eq!(
        hub["faults"],
        json!([]),
        "a bare term row is valid: {hub}"
    );
}

#[test]
fn an_accepted_move_keeps_the_register_entry_current() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "学习", 2);
    // The entry is a Side successor, so the move has a free location to go to.
    let target = address(mcp.ok(
        "create_successor",
        json!({ "at": format!("{tree}/1"), "kind": "side", "title": "学习曲线", "body": "累计产量提高时成本下降。" }),
    ));
    let reg = register(&mut mcp, &format!("## 词条\n- 学习曲线：[[{target}]]\n"));
    // A normal move rewrites the Link in the register body: no concept id has to change.
    let done = mcp.ok(
        "move_card",
        json!({ "at": target, "to": format!("{tree}/2|a"), "confirm": "move" }),
    );
    let moved = done["moved"]
        .as_object()
        .expect("the move names its mapping")
        .get(&target)
        .and_then(Value::as_str)
        .expect("the target moved")
        .to_string();
    assert_ne!(moved, target);
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(
        terms(&view),
        vec![("学习曲线".to_string(), vec![moved.clone()])],
        "the entry follows the moved Card"
    );
    assert_eq!(view["entries"][0]["targets"][0]["title"], json!("学习曲线"));
    assert!(
        view["terms_by_card"].get(&target).is_none(),
        "the old address no longer names the Card"
    );
    let body = mcp.ok("get_card", json!({ "at": reg }))["body"]
        .as_str()
        .expect("the register body")
        .to_string();
    assert!(body.contains(&format!("[[{moved}]]")), "{body}");
}

#[test]
fn a_human_card_is_a_read_only_target_of_an_agent_register() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "学习", 1);
    let agent_card = format!("{tree}/1");
    // The person's own Card, made through the owner's GUI: only that route writes the human
    // Workspace (the MCP server reads it and never changes it).
    let gui = home.gui();
    let human_root = address(gui.post(
        "/api/create",
        &json!({
            "kind": "topic",
            "workspace": "human",
            "text": "# 人的学习\n\n人自己的问题。\n",
        }),
    ));
    let human_card = address(gui.post(
        "/api/create",
        &json!({
            "kind": "direct",
            "at": human_root,
            "text": "# 学习是个体行为\n\n重复练习改变个体技能。\n",
        }),
    ));
    let authored = mcp.ok("get_card", json!({ "at": human_card }));
    assert_eq!(authored["workspace"], json!("human"));

    // An agent register may name the human Card as a target: agents read both Workspaces.
    let reg = register(
        &mut mcp,
        &format!("## 词条\n- 学习：[[{agent_card}]] [[{human_card}]]\n"),
    );
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(
        terms(&view),
        vec![(
            "学习".to_string(),
            vec![agent_card.clone(), human_card.clone()]
        )],
        "the human Card is the term's second target"
    );
    assert_eq!(view["complete"], json!(true));
    let human = &view["entries"][0]["targets"][1];
    assert_eq!(human["address"], json!(human_card));
    assert_eq!(human["exists"], json!(true));
    assert_eq!(human["workspace"], json!("human"));
    assert_eq!(human["title"], json!("学习是个体行为"));
    assert!(
        human["body"]
            .as_str()
            .is_some_and(|body| body.contains("重复练习")),
        "the human target carries its authored body: {human}"
    );

    // Read-only: the human Card's authored title and body never change. Only its generated
    // Reverse section follows the new Link (`workspace.rs`: that text is not an edit).
    let after = mcp.ok("get_card", json!({ "at": human_card }));
    assert_eq!(after["title"], authored["title"]);
    assert_eq!(after["body"], authored["body"]);
    assert_eq!(after["workspace"], json!("human"));
    // The register is reported as a hub of the human Card at its second target (D051).
    assert!(
        after["hubs"].as_array().is_some_and(|hubs| hubs.iter().any(|hub| {
            hub["address"] == json!(reg)
                && hub["term"] == json!("学习")
                && hub["target_index"] == json!(1)
        })),
        "the human Card reports the agent register: {}",
        after["hubs"]
    );
    // The view is the agent register's; the human Workspace has no register of its own.
    let human_view = mcp.ok("get_register", json!({ "workspace": "human" }));
    assert_eq!(human_view["register_missing"], json!(true));
}

#[test]
fn a_confirmed_restatement_is_projected_and_a_malformed_one_is_a_fault() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "斯卢茨基", 2);
    let entry = format!("{tree}/1");
    let substantive = format!("{tree}/2");
    mcp.ok(
        "edit_card",
        json!({ "at": substantive, "title": "斯卢茨基分解" }),
    );
    // The entry states its confirmed restatement: exactly one substantive Card (§4.5).
    mcp.ok(
        "edit_card",
        json!({
            "at": entry,
            "title": "斯卢茨基分解的陈述入口",
            "body": format!("陈述。\n\n## 同述入口\n\n- [[{substantive}]] 给出完整定义。\n"),
        }),
    );
    register(
        &mut mcp,
        &format!("## 词条\n- 斯卢茨基分解：[[{entry}]]\n"),
    );
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(view["complete"], json!(true));
    assert_eq!(
        view["restatements"],
        json!([{
            "entry": entry,
            "substantive": substantive,
            "title": "斯卢茨基分解",
        }]),
        "the confirmed restatement is part of the view"
    );

    // Two Links in the section is a malformed restatement: reported, and the entry is not
    // recorded as confirmed.
    mcp.ok(
        "edit_card",
        json!({
            "at": entry,
            "body": format!("陈述。\n\n## 同述入口\n\n- [[{substantive}]] 一处\n- [[{entry}]] 两处\n"),
        }),
    );
    let malformed = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(malformed["complete"], json!(false));
    assert_eq!(malformed["restatements"], json!([]));
    assert!(
        malformed["faults"].as_array().is_some_and(|faults| faults.iter().any(|fault| {
            fault["fault"] == json!("restatement")
                && fault["card"] == json!(entry)
                && fault["detail"].as_str().is_some_and(|detail| detail.contains("names 2 Cards"))
        })),
        "the section names two Cards: {}",
        malformed["faults"]
    );

    // A section that names its own Card is malformed too.
    mcp.ok(
        "edit_card",
        json!({
            "at": entry,
            "body": format!("陈述。\n\n## 同述入口\n\n- [[{entry}]] 指向自己\n"),
        }),
    );
    let itself = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(itself["complete"], json!(false));
    assert!(
        itself["faults"].as_array().is_some_and(|faults| faults.iter().any(|fault| {
            fault["fault"] == json!("restatement")
                && fault["detail"].as_str().is_some_and(|detail| detail.contains("itself"))
        })),
        "the section points at itself: {}",
        itself["faults"]
    );
}

#[test]
fn every_entry_page_is_read_at_one_revision_and_a_write_moves_the_register() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "分页", 4);
    let cards = (1..=4)
        .map(|n| format!("{tree}/{n}"))
        .collect::<Vec<_>>();
    register(
        &mut mcp,
        &format!(
            "## 词条\n- 词一：[[{}]]\n- 词二：[[{}]]\n- 词三：[[{}]]\n- 词四：[[{}]]\n",
            cards[0], cards[1], cards[2], cards[3]
        ),
    );
    let full = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(full["entry_total"], json!(4));
    let first = mcp.ok(
        "get_register",
        json!({ "workspace": "agent", "entry_offset": 0, "entry_limit": 2 }),
    );
    let second = mcp.ok(
        "get_register",
        json!({ "workspace": "agent", "entry_offset": 2, "entry_limit": 2 }),
    );
    assert_eq!(first["entry_next_offset"], json!(2));
    assert_eq!(second["entry_next_offset"], Value::Null);
    for page in [&first, &second] {
        assert_eq!(page["entry_total"], json!(4));
        assert_eq!(page["archive_revision"], full["archive_revision"]);
        assert_eq!(page["archive_identity"], full["archive_identity"]);
    }
    assert_eq!(
        terms(&first)
            .into_iter()
            .chain(terms(&second))
            .collect::<Vec<_>>(),
        terms(&full),
        "the pages are the whole catalog in page order"
    );
    // The same page read twice is the same page at the same revision: pagination is stable.
    let again = mcp.ok(
        "get_register",
        json!({ "workspace": "agent", "entry_offset": 0, "entry_limit": 2 }),
    );
    assert_eq!(again["archive_revision"], first["archive_revision"]);
    assert_eq!(again["entry_total"], first["entry_total"]);
    assert_eq!(terms(&again), terms(&first));

    // A write to an entry's body advances the revision, and the page shows the new body.
    let long = "被改写的正文。".repeat(200);
    mcp.ok(
        "edit_card",
        json!({ "at": cards[0], "title": "词一的定义", "body": long }),
    );
    let changed = mcp.ok(
        "get_register",
        json!({ "workspace": "agent", "entry_offset": 0, "entry_limit": 2 }),
    );
    assert!(
        changed["archive_revision"].as_u64() > full["archive_revision"].as_u64(),
        "a write advances the revision: {} then {}",
        full["archive_revision"],
        changed["archive_revision"]
    );
    assert_eq!(changed["entry_total"], json!(4));
    assert_eq!(changed["entry_next_offset"], json!(2));
    let target = &changed["entries"][0]["targets"][0];
    assert_eq!(target["title"], json!("词一的定义"));
    assert_eq!(target["body_chars"], json!(long.chars().count()));
    assert_eq!(target["body_truncated"], json!(true));
}

#[test]
fn a_scoped_term_stays_its_label_and_a_second_destination_is_an_ambiguity_fault() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "学习", 3);
    let individual = format!("{tree}/1");
    let industry = format!("{tree}/2");
    let curve = format!("{tree}/3");
    // Scope belongs in the label (§8.4): one bare word with two senses is two scoped labels.
    let reg = register(
        &mut mcp,
        &format!(
            "## 词条\n- 学习（个体行为）：[[{individual}]]\n- 学习（产业生产率）：[[{industry}]]\n"
        ),
    );
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(
        terms(&view),
        vec![
            ("学习（个体行为）".to_string(), vec![individual.clone()]),
            ("学习（产业生产率）".to_string(), vec![industry.clone()]),
        ],
        "a scoped term is read as its whole label, not reduced to the bare word"
    );
    assert_eq!(view["complete"], json!(true));
    assert_eq!(
        view["terms_by_card"][&individual],
        json!(["学习（个体行为）"])
    );

    // The same label written twice with different destinations is one ambiguous term: both
    // destinations stay visible and the conflict is reported; nothing is merged or chosen.
    mcp.ok(
        "edit_card",
        json!({
            "at": reg,
            "body": format!(
                "## 词条\n- 学习（个体行为）：[[{individual}]]\n- 学习（产业生产率）：[[{industry}]]\n- 学习（个体行为）：[[{curve}]]\n"
            ),
        }),
    );
    let ambiguous = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(
        ambiguous["entry_total"],
        json!(3),
        "every authored row stays an entry: {ambiguous}"
    );
    assert_eq!(ambiguous["complete"], json!(false));
    let faults = ambiguous["faults"].as_array().expect("faults");
    let fault = faults
        .iter()
        .find(|fault| fault["fault"] == json!("ambiguous"))
        .unwrap_or_else(|| panic!("the identical label is an ambiguity fault: {faults:?}"));
    assert_eq!(fault["term"], json!("学习（个体行为）"));
    let rows = fault["rows"].as_array().expect("the conflicting rows");
    assert_eq!(rows.len(), 2, "both rows are named: {fault}");
    assert!(
        rows.iter()
            .any(|row| row["targets"] == json!([individual.clone()])),
        "{fault}"
    );
    assert!(
        rows.iter()
            .any(|row| row["targets"] == json!([curve.clone()])),
        "{fault}"
    );
    let same_term = ambiguous["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["term"] == json!("学习（个体行为）"))
        .map(|entry| entry["targets"][0]["address"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        same_term,
        vec![json!(individual), json!(curve)],
        "the separate destinations stay in body order"
    );
    // get_hub on the register keeps the same explanation.
    let hub = mcp.ok("get_hub", json!({ "at": reg }));
    assert!(
        hub["faults"]
            .as_array()
            .unwrap()
            .iter()
            .any(|fault| fault["fault"] == json!("ambiguous")),
        "{hub}"
    );
    assert_eq!(hub["complete"], json!(false));
}

#[test]
fn an_archive_without_a_register_answers_an_honest_empty() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    topic(&mut mcp, "没有登记簿", 2);
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(view["register"], Value::Null);
    assert_eq!(view["register_missing"], json!(true));
    assert_eq!(view["entries"], json!([]));
    assert_eq!(view["entry_total"], json!(0));
    assert_eq!(view["entry_next_offset"], Value::Null);
    assert_eq!(
        view["complete"],
        json!(false),
        "an absent register is no claim of coverage: {view}"
    );
    assert!(
        view["faults"].as_array().is_some_and(|faults| faults
            .iter()
            .any(|fault| fault["fault"] == json!("register_missing"))),
        "the absent register is named: {view}"
    );
    assert!(
        view["archive_revision"].as_u64().is_some_and(|n| n > 0),
        "the empty answer still names the revision it read: {view}"
    );
    // The GUI answers the same empty, not an error.
    let gui = home.gui();
    let route = gui.get("/api/register?ws=agent");
    assert_eq!(route["register"], Value::Null);
    assert_eq!(route["register_missing"], json!(true));
    assert_eq!(route["entries"], json!([]));
    assert_eq!(route["entry_total"], json!(0));
    assert_eq!(route["archive_revision"], view["archive_revision"]);
    // The other Workspace has no register either.
    let human = mcp.ok("get_register", json!({ "workspace": "human" }));
    assert_eq!(human["register_missing"], json!(true));
    // An unknown Workspace is an error, not an empty answer.
    let refusal = mcp.err("get_register", json!({ "workspace": "all" }));
    assert!(refusal.contains("workspace"), "{refusal}");
}

#[test]
fn a_register_hub_report_keeps_its_own_checks_beside_the_entry_faults() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "枢纽检查", 1);
    let one = format!("{tree}/1");
    // The 领域 navigation line has no reason: the hub policy's own `bare` fault. The term rows
    // are valid, so the entry view has no fault; the legacy report check must not disappear.
    let reg = register(
        &mut mcp,
        &format!("## 词条\n- 术语：[[{one}]]\n\n## 领域\n- [[{one}]]\n"),
    );
    let view = mcp.ok("get_register", json!({ "workspace": "agent" }));
    assert_eq!(view["faults"], json!([]), "the term rows are valid: {view}");
    let hub = mcp.ok("get_hub", json!({ "at": reg }));
    let names = hub["faults"]
        .as_array()
        .expect("hub faults")
        .iter()
        .map(|fault| fault["fault"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(
        names.contains(&"bare".to_string()),
        "the register's own hub line check stays visible beside the entry faults: {names:?} {hub}"
    );
}