//! Entry-first search: one ranking contract, two transports, server.
//!
//! `src/entry.rs::search` ranks an exact address, a matching register term and its entry Cards, a
//! matching hub member line and the Card it opens, a matching model head, a matching Card title,
//! then a matching authored body (§9.2, D199-D206). The MCP server's `search_cards mode: entry`
//! and the GUI's `/api/search mode=entry` return the same rows, order, counts, and evidence for
//! the same archive and query; an omitted mode keeps the existing plain order and shape. The
//! archive is a scratch one (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`).

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

/// A query value for `/api/search`, percent-encoded byte by byte.
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

fn rows(value: &Value) -> Vec<Value> {
    value["results"]
        .as_array()
        .expect("results")
        .iter()
        .cloned()
        .collect()
}

fn addresses(value: &Value) -> Vec<String> {
    rows(value)
        .iter()
        .map(|row| row["address"].as_str().expect("address").to_string())
        .collect()
}

fn position(value: &Value, address: &str) -> usize {
    addresses(value)
        .iter()
        .position(|at| at == address)
        .unwrap_or_else(|| panic!("{address} is missing from {:?}", addresses(value)))
}

#[test]
fn entry_points_precede_body_hits_and_both_transports_agree() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "增长与政策", 3);
    let one = format!("{tree}/1");
    let two = format!("{tree}/2");
    let three = format!("{tree}/3");
    // The entry target: neither its title nor its body holds the term.
    mcp.ok(
        "edit_card",
        json!({ "at": one, "title": "干中学提高生产率", "body": "累计产量提高时单位成本下降。" }),
    );
    // A body-only hit and a title hit that are not register entries.
    mcp.ok(
        "edit_card",
        json!({ "at": two, "title": "经验积累", "body": "人们在生产过程中学习。" }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": three, "title": "学习的外部性", "body": "知识溢出。" }),
    );
    register(&mut mcp, &format!("## 词条\n- 学习：[[{one}]]\n"));

    let entry = mcp.ok("search_cards", json!({ "query": "学习", "mode": "entry", "limit": 80 }));
    assert_eq!(entry["mode"], json!("entry"));
    assert_eq!(entry["register_missing"], json!(false));
    assert_eq!(entry["registers"][1]["workspace"], json!("agent"));
    assert_eq!(entry["registers"][1]["missing"], json!(false));
    assert_eq!(entry["registers"][1]["complete"], json!(true));
    assert_eq!(entry["register_faults"], json!([]));
    assert_eq!(entry["bounds"]["results_limit_max"], json!(200));
    assert!(entry["scanned"]["terms"].as_u64().is_some_and(|n| n > 0));
    assert_eq!(entry["scanned"]["terms_truncated"], json!(false));
    assert_eq!(entry["scanned"]["hubs_truncated"], json!(false));
    assert_eq!(
        entry["archive_fingerprint"].as_str().map(str::len),
        Some(24),
        "the reply names the read it was made from: {entry}"
    );

    // The register entry comes first, then the title hit, then the body hit.
    assert_eq!(entry["results"][0]["address"], json!(one));
    assert_eq!(entry["results"][0]["role"], json!("entry"));
    assert_eq!(entry["results"][0]["rank_tier"], json!(2));
    assert_eq!(entry["results"][0]["matched_term"], json!("学习"));
    assert_eq!(entry["results"][0]["entry_targets"], json!([one]));
    assert_eq!(entry["results"][0]["origin"]["card"], entry["registers"][1]["card"]);
    assert!(
        entry["results"][0]["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("学习")),
        "{}",
        entry["results"][0]
    );
    let title = &entry["results"][position(&entry, &three)];
    assert_eq!(title["role"], json!("hit"));
    assert_eq!(title["rank_tier"], json!(5));
    let body = &entry["results"][position(&entry, &two)];
    assert_eq!(body["role"], json!("hit"));
    assert_eq!(body["rank_tier"], json!(6));
    assert!(position(&entry, &one) < position(&entry, &three));
    assert!(position(&entry, &three) < position(&entry, &two));

    // Every address appears once: a term is not a second Card.
    let mut unique = addresses(&entry);
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), addresses(&entry).len());

    // The GUI answers the same query with the same rows, order, counts, and evidence.
    let gui = home.gui();
    let route = gui.get(&format!("/api/search?q={}&mode=entry", enc("学习")));
    assert_eq!(route, entry, "one contract, two transports");
}

#[test]
fn accepted_spellings_name_the_same_targets_once() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "概念", 2);
    let one = format!("{tree}/1");
    let two = format!("{tree}/2");
    register(
        &mut mcp,
        &format!("## 词条\n- 多重均衡：[[{one}]] [[{two}]]\n- 均衡的多重性：[[{one}]] [[{two}]]\n"),
    );
    let found = mcp.ok("search_cards", json!({ "query": "均衡", "mode": "entry" }));
    let entry_rows = rows(&found)
        .into_iter()
        .filter(|row| row["role"] == json!("entry"))
        .collect::<Vec<_>>();
    assert_eq!(
        entry_rows.len(),
        2,
        "two spellings still open two Cards, each once: {found}"
    );
    assert_eq!(
        entry_rows
            .iter()
            .map(|row| row["address"].as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        vec![one.clone(), two.clone()]
    );
    for row in &entry_rows {
        assert_eq!(row["rank_tier"], json!(2));
        // The prefix spelling beats the partial one, and either names both destinations.
        assert_eq!(row["matched_term"], json!("均衡的多重性"));
        assert_eq!(row["entry_targets"], json!([one, two]));
    }
}

#[test]
fn a_register_target_that_is_also_a_body_hit_appears_once_as_an_entry() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "政策", 1);
    let target = format!("{tree}/1");
    let body = "学习在干中发生。".repeat(5);
    mcp.ok("edit_card", json!({ "at": target, "title": "干中学", "body": body }));
    register(&mut mcp, &format!("## 词条\n- 学习曲线：[[{target}]]\n"));
    let found = mcp.ok(
        "search_cards",
        json!({ "query": "学习", "mode": "entry" }),
    );
    let matches = rows(&found)
        .into_iter()
        .filter(|row| row["address"] == json!(target))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "the target appears once: {found}");
    assert_eq!(matches[0]["role"], json!("entry"));
    assert_eq!(matches[0]["rank_tier"], json!(2));
    assert_eq!(matches[0]["matched_term"], json!("学习曲线"));
}

#[test]
fn hub_member_lines_and_model_heads_are_ways_in() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "成本", 2);
    let one = format!("{tree}/1");
    let two = format!("{tree}/2");
    mcp.ok("edit_card", json!({ "at": one, "title": "干中学提高生产率", "body": "累计产量。" }));
    mcp.ok("edit_card", json!({ "at": two, "title": "经验积累", "body": "人们在生产过程中学习。" }));
    // A line hub whose member line names the subject of a Card that is not titled with it.
    let hub = address(mcp.ok(
        "create_successor",
        json!({
            "at": one,
            "kind": "side",
            "type": "hub",
            "level": "line",
            "title": "成本这一支的入口",
            "body": format!("## 邻近\n- [[{two}]]：讨论干中学的成本下降。\n"),
        }),
    ));
    let member = mcp.ok("search_cards", json!({ "query": "干中学", "mode": "entry" }));
    let row = &member["results"][position(&member, &two)];
    assert_eq!(row["role"], json!("member"));
    assert_eq!(row["rank_tier"], json!(3));
    assert_eq!(row["origin"]["card"], json!(hub));
    assert_eq!(row["origin"]["line"], json!(2), "the body line of the member row");
    assert!(
        row["matched_line"]
            .as_str()
            .is_some_and(|line| line.contains("讨论干中学")),
        "{row}"
    );
    assert!(
        position(&member, &two) < position(&member, &one),
        "the hub way in precedes the ordinary title hit: {}",
        addresses(&member).join(", ")
    );

    // A model head is a way in ahead of an ordinary title hit.
    let models = topic(&mut mcp, "模型入口", 2);
    let model = format!("{models}/1");
    let note = format!("{models}/2");
    mcp.ok(
        "edit_card",
        json!({ "at": model, "title": "学习模型", "type": "model" }),
    );
    mcp.ok("edit_card", json!({ "at": note, "title": "学习模型的笔记" }));
    let found = mcp.ok("search_cards", json!({ "query": "学习模型", "mode": "entry" }));
    let head = &found["results"][position(&found, &model)];
    assert_eq!(head["role"], json!("hit"));
    assert_eq!(head["rank_tier"], json!(4));
    assert_eq!(found["results"][position(&found, &note)]["rank_tier"], json!(5));
    assert!(position(&found, &model) < position(&found, &note));
}

#[test]
fn an_exact_address_opens_that_card_first() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "地址", 2);
    let one = format!("{tree}/2");
    mcp.ok("edit_card", json!({ "at": one, "title": "第二张", "body": "正文。" }));
    let found = mcp.ok("search_cards", json!({ "query": one, "mode": "entry" }));
    assert_eq!(found["results"][0]["address"], json!(one));
    assert_eq!(found["results"][0]["rank_tier"], json!(1));
    assert_eq!(found["results"][0]["role"], json!("hit"));
}

#[test]
fn filters_apply_to_targets_before_rows_and_counts() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "政策", 1);
    let idea = format!("{tree}/1");
    mcp.ok(
        "edit_card",
        json!({ "at": idea, "title": "学习提高生产率", "body": "累计产量提高。" }),
    );
    let model = address(mcp.ok(
        "create_successor",
        json!({ "at": idea, "kind": "side", "title": "学习模型", "type": "model" }),
    ));
    register(&mut mcp, &format!("## 词条\n- 学习：[[{idea}]] [[{model}]]\n"));

    // An Idea-only search cannot receive the Model through the register row (D208).
    let ideas = mcp.ok(
        "search_cards",
        json!({ "query": "学习", "mode": "entry", "type": "idea" }),
    );
    assert_eq!(addresses(&ideas), vec![idea.clone()]);
    assert_eq!(ideas["results"][0]["role"], json!("entry"));
    assert_eq!(
        ideas["results"][0]["entry_targets"],
        json!([idea]),
        "the row reports the destinations that survive the filter"
    );
    assert_eq!(
        ideas["types"],
        json!({ "hub": 1, "idea": 1, "model": 1 }),
        "the type chips count every matching Card, as the plain search does, before the filter"
    );
    assert_eq!(ideas["total"], json!(1));

    // A Model-only search keeps the entry row: the term's second destination survives.
    let models = mcp.ok(
        "search_cards",
        json!({ "query": "学习", "mode": "entry", "type": "model" }),
    );
    assert_eq!(addresses(&models), vec![model.clone()]);
    assert_eq!(models["results"][0]["role"], json!("entry"));
    assert_eq!(models["results"][0]["entry_targets"], json!([model]));

    // A Workspace with neither target returns no row, but the chips keep saying what they would.
    let human = mcp.ok(
        "search_cards",
        json!({ "query": "学习", "mode": "entry", "workspace": "human" }),
    );
    assert_eq!(human["results"], json!([]));
    assert_eq!(human["total"], json!(0));
    assert_eq!(human["workspaces"], json!({ "agent": 3 }));
    assert_eq!(human["types"], json!({}));
}

#[test]
fn an_archive_without_a_register_falls_back_honestly() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "没有登记簿", 1);
    mcp.ok(
        "edit_card",
        json!({ "at": format!("{tree}/1"), "title": "一张卡", "body": "这里讨论学习。" }),
    );
    let fallback = mcp.ok("search_cards", json!({ "query": "学习", "mode": "entry" }));
    assert_eq!(fallback["mode"], json!("entry"));
    assert_eq!(fallback["register_missing"], json!(true));
    assert_eq!(fallback["registers"][0]["missing"], json!(true));
    assert_eq!(fallback["registers"][1]["missing"], json!(true));
    assert_eq!(fallback["register_faults"], json!([]));
    assert_eq!(fallback["total"], json!(1));
    assert_eq!(fallback["results"][0]["role"], json!("hit"));
    assert_eq!(fallback["results"][0]["rank_tier"], json!(6));
    // The GUI answers the same fallback, labelled the same way.
    let gui = home.gui();
    assert_eq!(
        gui.get(&format!("/api/search?q={}&mode=entry", enc("学习"))),
        fallback
    );
}

#[test]
fn a_broken_register_row_is_reported_and_left_out_of_retrieval() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "学习", 2);
    let one = format!("{tree}/1");
    let gone = address(mcp.ok(
        "create_successor",
        json!({ "at": one, "kind": "side", "title": "消失的卡" }),
    ));
    let register = register(
        &mut mcp,
        &format!("## 词条\n- 学习：[[{one}]]\n- 消失：[[{gone}]]\n"),
    );
    mcp.ok("delete_card", json!({ "at": gone, "confirm": "delete" }));
    let found = mcp.ok("search_cards", json!({ "query": "消失", "mode": "entry" }));
    assert!(
        rows(&found).iter().all(|row| row["role"] != json!("entry")),
        "the broken term is not an entry: {found}"
    );
    let faults = found["register_faults"].as_array().expect("register faults");
    assert!(
        faults.iter().any(|fault| fault["fault"] == json!("broken")
            && fault["detail"].as_str().is_some_and(|detail| detail.contains(&gone))),
        "the exact row is reported: {faults:?}"
    );
    assert!(faults.iter().all(|fault| fault["card"] == json!(register)));
    assert_eq!(found["registers"][1]["complete"], json!(false));
    // The valid row still works.
    let good = mcp.ok("search_cards", json!({ "query": "学习", "mode": "entry" }));
    assert_eq!(good["results"][0]["address"], json!(one));
    assert_eq!(good["results"][0]["role"], json!("entry"));
}

#[test]
fn plain_search_and_empty_query_listings_keep_their_contract() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "增长", 2);
    let one = format!("{tree}/1");
    mcp.ok("edit_card", json!({ "at": one, "title": "学习曲线", "body": "单位成本下降。" }));
    register(&mut mcp, &format!("## 词条\n- 学习：[[{one}]]\n"));

    // The omitted mode is plain, with the existing MCP order and the existing single key.
    let plain = mcp.ok("search_cards", json!({ "query": "学习" }));
    assert_eq!(
        plain
            .as_object()
            .map(|object| object.keys().cloned().collect::<Vec<_>>()),
        Some(vec!["results".to_string()]),
        "the new fields exist only in entry mode: {plain}"
    );
    let explicit = mcp.ok("search_cards", json!({ "query": "学习", "mode": "plain" }));
    assert_eq!(plain, explicit, "an explicit plain mode changes nothing");
    assert!(
        plain["results"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty()),
        "{plain}"
    );

    // An empty query is the existing type listing, in either mode.
    let listing = mcp.ok("search_cards", json!({ "type": "hub" }));
    assert_eq!(
        mcp.ok("search_cards", json!({ "type": "hub", "mode": "entry" })),
        listing,
        "an empty query keeps the existing listing"
    );
    assert_eq!(mcp.ok("search_cards", json!({ "type": "hub", "mode": "plain" })), listing);

    // The GUI's plain presentation is its own existing lexical order: every row carries its
    // score, and the set of Cards is the same as the MCP plain set (the two legacy orders
    // already differ; entry mode is the one shared order).
    let gui = home.gui();
    let route = gui.get(&format!("/api/search?q={}&mode=plain", enc("学习")));
    assert_eq!(route["mode"], json!("plain"));
    let scores = rows(&route)
        .iter()
        .map(|row| row["score"].as_i64().expect("a plain GUI row keeps its lexical score"))
        .collect::<Vec<_>>();
    assert!(
        scores.windows(2).all(|pair| pair[0] >= pair[1]),
        "the GUI plain order keeps its lexical score: {scores:?}"
    );
    let mut mcp_set = plain["results"]
        .as_array()
        .expect("results")
        .iter()
        .map(|row| row["address"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let mut gui_set = addresses(&route);
    mcp_set.sort();
    gui_set.sort();
    assert_eq!(mcp_set, gui_set);
    assert_eq!(route["total"], json!(scores.len()));

    // An unknown mode is refused, in both transports.
    let (value, is_error) = mcp.call("search_cards", json!({ "query": "学习", "mode": "fuzzy" }));
    assert!(is_error, "{value}");
    let refusal = gui.get("/api/search?q=x&mode=fuzzy");
    assert!(
        refusal["error"]
            .as_str()
            .is_some_and(|error| error.contains("unknown search mode")),
        "{refusal}"
    );
}

#[test]
fn the_entry_order_is_deterministic_and_bounded() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "排序", 2);
    for at in [format!("{tree}/1"), format!("{tree}/2")] {
        mcp.ok("edit_card", json!({ "at": at, "title": "并列的结果", "body": "正文相同。" }));
    }
    let first = mcp.ok("search_cards", json!({ "query": "并列", "mode": "entry" }));
    let second = mcp.ok("search_cards", json!({ "query": "并列", "mode": "entry" }));
    assert_eq!(first, second, "the same read and query give the same rows");
    assert_eq!(
        addresses(&first),
        vec![format!("{tree}/1"), format!("{tree}/2")],
        "natural address order breaks the final tie"
    );
    // A page is bounded, and the unbounded count is reported separately.
    let page = mcp.ok(
        "search_cards",
        json!({ "query": "并列", "mode": "entry", "limit": 1 }),
    );
    assert_eq!(page["shown"], json!(1));
    assert_eq!(addresses(&page).len(), 1);
    assert_eq!(page["total"], json!(2));
    assert_eq!(page["row_total"], json!(2));
    let clamped = mcp.ok(
        "search_cards",
        json!({ "query": "并列", "mode": "entry", "limit": 5000 }),
    );
    assert!(rows(&clamped).len() <= 200);
    assert_eq!(clamped["bounds"]["results_limit_max"], json!(200));
    assert_eq!(clamped["bounds"]["matched_line_max_chars"], json!(200));
}

#[test]
fn a_register_part_is_evidence_not_a_competitor() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "政策", 2);
    let one = format!("{tree}/1");
    let two = format!("{tree}/2");
    mcp.ok(
        "edit_card",
        json!({ "at": one, "title": "干中学提高生产率", "body": "累计产量提高。" }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": two, "title": "经验积累", "body": "人们在生产过程中学习。" }),
    );
    // The register keeps no rows itself: its declared part holds them, so the part's body repeats
    // the term far more often than either target does.
    let index = address(mcp.ok(
        "create_topic",
        json!({ "title": "索引", "body": "本主题收放代理工作区的入口。" }),
    ));
    let register = address(mcp.ok(
        "create_successor",
        json!({
            "at": index,
            "type": "hub",
            "level": "register",
            "title": "索引：代理工作区的入口",
            "body": "## 词条分册\n",
        }),
    ));
    let part = address(mcp.ok(
        "create_successor",
        json!({
            "at": register,
            "kind": "side",
            "title": "词条 A–F",
            "body": format!("## 词条\n- 学习：[[{one}]]\n- 学习曲线：[[{two}]]\n"),
        }),
    ));
    mcp.ok(
        "edit_card",
        json!({ "at": register, "body": format!("## 词条分册\n- [[{part}]] A–F\n") }),
    );

    let found = mcp.ok("search_cards", json!({ "query": "学习", "mode": "entry" }));
    assert_eq!(found["registers"][1]["entry_total"], json!(2));
    let row = &found["results"][position(&found, &one)];
    assert_eq!(row["role"], json!("entry"));
    assert_eq!(row["rank_tier"], json!(2));
    assert_eq!(row["origin"]["card"], json!(part));
    assert_eq!(row["origin"]["part"], json!(part));
    assert_eq!(row["entry_targets"], json!([one]));
    // The part opens nothing itself: its repeating rows never outrank a real destination.
    let at_part = position(&found, &part);
    assert_eq!(found["results"][at_part]["rank_tier"], json!(6));
    assert!(position(&found, &one) < at_part);
    assert!(position(&found, &two) < at_part);
    // The declared part is never a target and never a term.
    assert!(
        rows(&found).iter().all(|row| row["role"] != json!("entry") || row["address"] != json!(part)),
        "{found}"
    );
}

#[test]
fn the_form_filter_applies_to_targets_before_rows_and_counts() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "形式", 1);
    let idea = format!("{tree}/1");
    mcp.ok(
        "edit_card",
        json!({ "at": idea, "title": "学习提高生产率", "body": "累计产量提高。" }),
    );
    // A unit states the same term with a Math form; the term names both destinations.
    let unit = address(mcp.ok(
        "create_successor",
        json!({
            "at": idea,
            "kind": "side",
            "title": "学习定理",
            "type": "model",
            "form": "theorem",
        }),
    ));
    register(&mut mcp, &format!("## 词条\n- 学习：[[{idea}]] [[{unit}]]\n"));

    let theorem = mcp.ok(
        "search_cards",
        json!({ "query": "学习", "mode": "entry", "form": "theorem" }),
    );
    assert_eq!(addresses(&theorem), vec![unit.clone()]);
    assert_eq!(theorem["results"][0]["role"], json!("entry"));
    assert_eq!(theorem["results"][0]["rank_tier"], json!(2));
    assert_eq!(
        theorem["results"][0]["entry_targets"],
        json!([unit]),
        "the row names only the destination that survives the form filter"
    );
    assert_eq!(theorem["total"], json!(1));

    // A form no target states returns no row: a term with no surviving destination is not usable.
    let theory = mcp.ok(
        "search_cards",
        json!({ "query": "学习", "mode": "entry", "form": "theory" }),
    );
    assert_eq!(theory["results"], json!([]));
    assert_eq!(theory["total"], json!(0));
    assert_eq!(theory["types"], json!({}));
}

#[test]
fn the_cached_gui_read_follows_a_register_row_and_a_title_edit() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "政策", 2);
    let one = format!("{tree}/1");
    let two = format!("{tree}/2");
    mcp.ok("edit_card", json!({ "at": two, "title": "别的题目", "body": "与它无关。" }));
    let register = register(&mut mcp, &format!("## 词条\n- 学习：[[{one}]]\n"));
    let gui = home.gui();
    let route = format!("/api/search?q={}&mode=entry", enc("学习"));
    // The first answer fills the GUI's cached Snapshot.
    let first = gui.get(&route);
    assert_eq!(first["results"][0]["address"], json!(one));
    assert_eq!(first["results"][0]["role"], json!("entry"));
    let fingerprint = first["archive_fingerprint"].clone();
    assert!(fingerprint.as_str().is_some_and(|value| value.len() == 24));

    // A register row edit retargets the term; the next read must not serve the old ranking.
    mcp.ok(
        "edit_card",
        json!({ "at": register, "body": format!("## 词条\n- 学习：[[{two}]]\n") }),
    );
    let retargeted = gui.get(&route);
    assert_eq!(retargeted["results"][0]["address"], json!(two));
    assert_eq!(retargeted["results"][0]["role"], json!("entry"));
    assert_ne!(
        retargeted["archive_fingerprint"], fingerprint,
        "the reply names the read it was made from"
    );

    // A title edit reaches the ranking too: the Card becomes a title hit, not a body hit.
    mcp.ok("edit_card", json!({ "at": two, "title": "学习曲线" }));
    let retitled = gui.get(&route);
    let row = &retitled["results"][position(&retitled, &two)];
    assert_eq!(row["role"], json!("entry"), "the term row outranks the title: {row}");
    assert_eq!(row["rank_tier"], json!(2));
    // A Card that only its body names is an ordinary body hit, below every entry.
    mcp.ok("edit_card", json!({ "at": one, "title": "甲", "body": "这里讨论学习。" }));
    let body = gui.get(&route);
    let row = &body["results"][position(&body, &one)];
    assert_eq!(row["role"], json!("hit"));
    assert_eq!(row["rank_tier"], json!(6));
    assert!(position(&body, &two) < position(&body, &one));
}

#[test]
fn the_cached_gui_read_follows_a_part_and_a_hub_member_line() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "成本", 2);
    let one = format!("{tree}/1");
    let two = format!("{tree}/2");
    let gui = home.gui();
    let route = format!("/api/search?q={}&mode=entry", enc("学习"));
    // A Card with no hub way in yet: only its own body names the term.
    mcp.ok("edit_card", json!({ "at": one, "title": "干中学", "body": "学习在生产中发生。" }));
    let before = gui.get(&route);
    assert_eq!(before["results"][position(&before, &one)]["role"], json!("hit"));

    // A hub member line added to the open Card's hub makes the Card a way in (tier 3), and the
    // GUI's cached Snapshot must follow the Link edit.
    let hub = address(mcp.ok(
        "create_successor",
        json!({
            "at": one,
            "kind": "side",
            "type": "hub",
            "level": "line",
            "title": "这一支的入口",
        }),
    ));
    mcp.ok(
        "edit_card",
        json!({ "at": hub, "body": format!("## 邻近\n- [[{two}]] 讨论学习的外部性。\n") }),
    );
    mcp.ok("edit_card", json!({ "at": two, "title": "经验积累" }));
    let way_in = gui.get(&route);
    let row = &way_in["results"][position(&way_in, &two)];
    assert_eq!(row["role"], json!("member"));
    assert_eq!(row["rank_tier"], json!(3));
    assert_eq!(row["origin"]["card"], json!(hub));

    // A declared part of the register moves the row's origin without changing the row's target.
    let index = address(mcp.ok(
        "create_topic",
        json!({ "title": "索引", "body": "本主题收放代理工作区的入口。" }),
    ));
    let register = address(mcp.ok(
        "create_successor",
        json!({
            "at": index,
            "type": "hub",
            "level": "register",
            "title": "索引：代理工作区的入口",
            "body": "## 词条分册\n",
        }),
    ));
    let part = address(mcp.ok(
        "create_successor",
        json!({
            "at": register,
            "kind": "side",
            "title": "词条 A–F",
            "body": format!("## 词条\n- 学习：[[{one}]]\n"),
        }),
    ));
    mcp.ok(
        "edit_card",
        json!({ "at": register, "body": format!("## 词条分册\n- [[{part}]] A–F\n") }),
    );
    let declared = gui.get(&route);
    let row = &declared["results"][position(&declared, &one)];
    assert_eq!(row["role"], json!("entry"));
    assert_eq!(row["rank_tier"], json!(2));
    assert_eq!(row["origin"]["card"], json!(part));
    assert_eq!(row["origin"]["part"], json!(part));
}

#[test]
fn an_exact_term_precedes_a_prefix_term_whatever_the_score() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "均衡", 2);
    let exact = format!("{tree}/1");
    let prefix = format!("{tree}/2");
    mcp.ok("edit_card", json!({ "at": exact, "title": "甲", "body": "无关。" }));
    // The prefix term's target repeats the query, so its lexical score is the higher one.
    mcp.ok(
        "edit_card",
        json!({ "at": prefix, "title": "乙", "body": "学习。学习。学习。学习。" }),
    );
    register(
        &mut mcp,
        &format!("## 词条\n- 学习：[[{exact}]]\n- 学习曲线：[[{prefix}]]\n"),
    );
    let found = mcp.ok("search_cards", json!({ "query": "学习", "mode": "entry" }));
    assert_eq!(found["results"][position(&found, &exact)]["matched_term"], json!("学习"));
    assert_eq!(
        found["results"][position(&found, &prefix)]["matched_term"],
        json!("学习曲线")
    );
    assert!(
        found["results"][position(&found, &exact)]["score"]
            .as_i64()
            .is_some_and(|score| found["results"][position(&found, &prefix)]["score"]
                .as_i64()
                .is_some_and(|other| score < other)),
        "the prefix term's target scores higher: {found}"
    );
    assert!(
        position(&found, &exact) < position(&found, &prefix),
        "the exact label precedes the prefix label before any score: {}",
        addresses(&found).join(", ")
    );
}

#[test]
fn within_a_tier_the_lexical_score_orders_the_ties() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let tree = topic(&mut mcp, "政策", 2);
    let once = format!("{tree}/1");
    let thrice = format!("{tree}/2");
    // Neither title holds the term, so both are body hits; the repeated body scores higher.
    mcp.ok("edit_card", json!({ "at": once, "title": "甲", "body": "这里讨论学习。" }));
    mcp.ok(
        "edit_card",
        json!({ "at": thrice, "title": "乙", "body": "学习。学习。学习。" }),
    );
    let found = mcp.ok("search_cards", json!({ "query": "学习", "mode": "entry" }));
    assert_eq!(found["results"][position(&found, &once)]["rank_tier"], json!(6));
    assert_eq!(found["results"][position(&found, &thrice)]["rank_tier"], json!(6));
    assert!(
        found["results"][position(&found, &thrice)]["score"]
            .as_i64()
            .is_some_and(|score| score > found["results"][position(&found, &once)]["score"].as_i64().unwrap_or(i64::MAX)),
        "{found}"
    );
    assert!(
        position(&found, &thrice) < position(&found, &once),
        "the higher lexical score orders a tie inside the tier: {}",
        addresses(&found).join(", ")
    );

    // One term with three destinations: every row names all three, and address order breaks the
    // remaining tie.
    let unit_tree = topic(&mut mcp, "均衡", 3);
    let one = format!("{unit_tree}/1");
    let two = format!("{unit_tree}/2");
    let three = format!("{unit_tree}/3");
    register(
        &mut mcp,
        &format!("## 词条\n- 均衡：[[{one}]] [[{two}]] [[{three}]]\n"),
    );
    let tied = mcp.ok("search_cards", json!({ "query": "均衡", "mode": "entry" }));
    let entry_rows = rows(&tied)
        .into_iter()
        .filter(|row| row["role"] == json!("entry"))
        .collect::<Vec<_>>();
    assert_eq!(
        entry_rows
            .iter()
            .map(|row| row["address"].as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        vec![one.clone(), two.clone(), three.clone()],
        "natural address order breaks the final tie: {tied}"
    );
    assert!(entry_rows.iter().all(|row| row["entry_targets"] == json!([one, two, three])));
}
