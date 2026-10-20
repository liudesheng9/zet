//! The integrated release contract (S24, #120): one scratch archive, every delivered seam.
//!
//! The earlier slices each proved one feature at its own public seam. This suite proves that the
//! features still hold together in one archive and one running service: the MCP server and the GUI
//! answer one entry-first search, one Ask proposal and one register projection (§9.5, D206); the
//! reads write nothing; the Owner writes (a kept Ask Spread, a Reply, a filed hub draft) change only
//! the Cards they address and keep every authored fingerprint else; the saved state survives a
//! restart; and the ordinary move, delete and Citation-key paths keep the saved Spreads, the
//! register entries and the Link index current (§12.3, D300/D303). The archive is a scratch one
//! (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`); the live archive is never touched.

#![allow(dead_code)]

use assert_cmd::Command;
use serde_json::{Value, json};
use std::collections::BTreeMap;
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
        // A helper that cannot run makes any accidental model or digest call fail the test.
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
            .env("ZT_GUI_PORT", "0")
            .env("ZT_DIGEST_HELPER", "Z:\\nowhere\\zt-digest.exe");
        cmd
    }

    fn mcp(&self) -> Mcp {
        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("zt"))
            .arg("mcp")
            .env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0")
            .env("ZT_DIGEST_HELPER", "Z:\\nowhere\\zt-digest.exe")
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

    fn restart(&self) -> Gui {
        self.cmd().arg("down").assert().success();
        self.cmd().arg("up").assert().success();
        self.gui()
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
        let reply = self.request("tools/call", json!({ "name": name, "arguments": arguments }));
        (
            reply["result"]["structuredContent"].clone(),
            reply["result"]["isError"].as_bool().expect("isError flag"),
        )
    }

    fn ok(&mut self, name: &str, arguments: Value) -> Value {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(!is_error, "{name}({arguments}) failed: {value}");
        value
    }

    /// The archive revision one read reports, from an empty bulk read.
    fn revision(&mut self) -> Value {
        self.ok("get_cards", json!({ "addresses": [] }))["archive_revision"].clone()
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
    /// One authenticated GET; the JSON body of the answer, asserting a 200.
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

    /// One authenticated POST of a JSON body; the status and the JSON answer.
    fn post_with(&self, path: &str, token: &str, body: &Value) -> (u16, Value) {
        use std::net::TcpStream;
        let payload = serde_json::to_string(body).expect("JSON body");
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let request = format!(
            "POST {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            self.port,
            payload.len()
        );
        stream.write_all(request.as_bytes()).expect("send request");
        read_reply(stream)
    }

    /// One authenticated POST; the JSON answer, asserting a successful read.
    fn post(&self, path: &str, body: &Value) -> Value {
        let (status, value) = self.post_with(path, &self.token, body);
        assert_eq!(status, 200, "{path} answered {status}: {value}");
        value
    }

    /// The Card the page sees: its authored fingerprint, title, type and Workspace.
    fn card(&self, at: &str) -> Value {
        self.get(&format!("/api/card?at={}", enc(at)))
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

/// A query value for the GUI routes, percent-encoded byte by byte.
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

fn address(value: &Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

/// Every Card's authored fingerprint, keyed by address: the identity an accepted edit must keep.
fn fingerprints(gui: &Gui) -> BTreeMap<String, String> {
    let graph = gui.get("/api/graph");
    let mut prints = BTreeMap::new();
    for node in graph["nodes"].as_array().expect("nodes") {
        let at = node["address"].as_str().expect("node address").to_string();
        let card = gui.card(&at);
        let print = card["authored_fingerprint"]
            .as_str()
            .unwrap_or_else(|| panic!("{at} carries an authored fingerprint: {card}"))
            .to_string();
        prints.insert(at, print);
    }
    prints
}

/// The archive state a read must leave exactly as it found it.
fn state(gui: &Gui) -> Value {
    json!({
        "overview": gui.get("/api/overview"),
        "graph": gui.get("/api/graph"),
        "spreads": gui.get("/api/spreads"),
        "register_agent": gui.get("/api/register?ws=agent"),
        "register_human": gui.get("/api/register?ws=human"),
        "broken": gui.get("/api/broken"),
    })
}

/// One Literature tree with its first Card: a book and a Card filed in it.
fn book(mcp: &mut Mcp, key: &str, title: &str, card_title: &str, body: &str) -> String {
    let at = address(&mcp.ok(
        "create_literature",
        json!({ "bibtex": format!("@book{{{key}, title={{{title}}}}}"), "body": "该书。" }),
    ));
    assert_eq!(at, key, "the Citation key is the tree");
    address(&mcp.ok(
        "create_successor",
        json!({ "at": key, "title": card_title, "body": body }),
    ))
}

/// The concept register: a Hub Card of level `register`, the first Card of the Topic `索引`.
fn register(mcp: &mut Mcp, body: &str) -> String {
    let index = address(&mcp.ok(
        "create_topic",
        json!({ "title": "索引", "body": "本主题收放代理工作区的入口。" }),
    ));
    address(&mcp.ok(
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

struct Fixture {
    /// The first Card of book A, the entry target of the accepted term.
    book_a: String,
    /// The first Card of book B, the supported treatment of the same term.
    book_b: String,
    /// book B's Literature Card: the only Card of the tree that carries BibTeX.
    book_b_root: String,
    /// The Idea Card one Topic tree holds, a register target and a hub member.
    idea: String,
    /// An Idea Card filed beside it as a Side successor, the third register target.
    other_idea: String,
    /// The Model Card and its one part.
    model: String,
    part: String,
    /// A line Hub Card whose member lines name the Idea Card and book B's Card.
    hub: String,
    /// The register Hub Card.
    register: String,
    /// The Topic root all the Idea Cards are filed under.
    topic_root: String,
}

/// One scratch archive holding every feature a reader and the two transports share: two books that
/// cross-link, an Idea Topic with a model, a line hub, the concept register with a three-target
/// term, and an empty human Workspace (a Reply creates its default Topic).
fn fixture(home: &TestHome) -> Fixture {
    let mut mcp = home.mcp();
    let book_a = book(
        &mut mcp,
        "bookA",
        "Book A",
        "多重均衡的陈述",
        "多重均衡来自策略互补。\n\n来源：[[bookA]]",
    );
    let book_b = book(
        &mut mcp,
        "bookB",
        "Book B",
        "策略互补",
        "策略互补解释多重均衡。\n\n来源：[[bookB]]",
    );
    assert_eq!((book_a.as_str(), book_b.as_str()), ("bookA/1", "bookB/1"));
    // The two books cross-link once both Cards exist, so each Link has its target.
    mcp.ok(
        "edit_card",
        json!({ "at": book_a, "body": format!("多重均衡来自策略互补。对应 [[{book_b}]]。\n\n来源：[[bookA]]") }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": book_b, "body": format!("策略互补解释多重均衡。对照 [[{book_a}]]。\n\n来源：[[bookB]]") }),
    );
    let topic_root = address(&mcp.ok(
        "create_topic",
        json!({ "title": "概念", "body": "本主题收放跨书的同一机制。" }),
    ));
    let tree = topic_root.trim_end_matches("/0").to_string();
    let idea = address(&mcp.ok(
        "create_successor",
        json!({
            "at": topic_root,
            "title": "干中学提高生产率",
            "body": "累计产量提高时单位成本下降。\n\n来源：[[bookA]]",
        }),
    ));
    assert_eq!(idea, format!("{tree}/1"));
    // A Side successor: no other Card stands after it, so a later delete frees its address instead
    // of handing it to the next occupant.
    let other_idea = address(&mcp.ok(
        "create_successor",
        json!({
            "at": idea,
            "kind": "side",
            "title": "规模报酬也产生多重均衡",
            "body": "规模报酬递增时，厂商的最优选择不唯一。",
        }),
    ));
    let model = address(&mcp.ok(
        "create_successor",
        json!({
            "at": idea,
            "type": "model",
            "level": "concept",
            "title": "策略互补模型",
            "body": "本模型追问：策略互补为什么产生多重均衡？",
        }),
    ));
    let part = address(&mcp.ok(
        "create_successor",
        json!({
            "at": model,
            "type": "model",
            "facet": "设定",
            "emphasis": "core",
            "title": "策略互补的设定",
            "body": format!("依赖 [[{book_a}]]：多重均衡来自策略互补。"),
        }),
    ));
    let hub = address(&mcp.ok(
        "create_successor",
        json!({
            "at": model,
            "kind": "side",
            "type": "hub",
            "level": "line",
            "title": "策略互补这条线",
            "body": format!("## 主线\n- [[{idea}]]：干中学提高生产率。\n- [[{book_b}]]：策略互补解释多重均衡。"),
        }),
    ));
    let register = register(
        &mut mcp,
        &format!("## 词条\n- 多重均衡：[[{book_a}]] [[{book_b}]] [[{other_idea}]]\n- 学习：[[{idea}]] [[{book_b}]]\n"),
    );
    Fixture {
        book_a,
        book_b: book_b.clone(),
        book_b_root: book_b.trim_end_matches("/1").to_string(),
        idea,
        other_idea,
        model,
        part,
        hub,
        register,
        topic_root,
    }
}

/// The address a plan's mapping gives a Card: its new one, or its own when it did not move.
fn translate(mapping: &Value, at: &str) -> String {
    mapping
        .get(at)
        .and_then(Value::as_str)
        .unwrap_or(at)
        .to_string()
}

/// The Spread document the page posts to `/api/spreads`.
fn spread_document(spreads: &Value, id: &str, name: &str, question: &str, cards: &[(&str, f64)]) -> Value {
    let mut doc = spreads.clone();
    doc["spreads"].as_array_mut().expect("spreads list").push(json!({
        "id": id,
        "name": name,
        "question": question,
        "cards": cards.iter().map(|(at, x)| json!({ "at": at, "x": x, "y": 0.0 })).collect::<Vec<_>>(),
    }));
    doc["current"] = json!(id);
    doc
}

/// The Spread document the page's Keep posts for a proposal (`ZtAsk.askKeep`).
fn keep_document(spreads: &Value, proposal: &Value, id: &str) -> Value {
    let mut doc = spreads.clone();
    doc["spreads"]
        .as_array_mut()
        .expect("spreads list")
        .push(json!({
            "id": id,
            "name": proposal["spread"]["name"],
            "question": proposal["spread"]["question"],
            "cards": proposal["spread"]["cards"],
        }));
    doc["current"] = json!(id);
    doc
}

fn kept_spread(spreads: &Value, id: &str) -> Value {
    spreads["spreads"]
        .as_array()
        .expect("spreads list")
        .iter()
        .find(|spread| spread["id"] == json!(id))
        .cloned()
        .unwrap_or_else(|| panic!("no Spread {id} in {spreads}"))
}

fn spread_addresses(spread: &Value) -> Vec<String> {
    spread["cards"]
        .as_array()
        .expect("spread cards")
        .iter()
        .map(|place| place["at"].as_str().expect("spread place").to_string())
        .collect()
}

/// The entry rows of one term in a register projection.
fn term_rows<'a>(register: &'a Value, term: &str) -> Vec<&'a Value> {
    register["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .filter(|entry| entry["term"] == json!(term))
        .collect()
}

/// AC03/AC04: entry-first search, the Ask proposal, the register projection and the plain listing
/// answer one value over both transports, and none of the reads writes a Card, a Link or a Spread.
#[test]
fn every_read_agrees_across_the_two_transports_and_writes_nothing() {
    let home = TestHome::new();
    let f = fixture(&home);
    let gui = home.gui();
    let mut mcp = home.mcp();
    let before = state(&gui);
    let prints = fingerprints(&gui);
    let revision = mcp.revision();

    // Entry mode: one ranking, two transports, byte-identical (D206, §9.5).
    let entry_mcp = mcp.ok(
        "search_cards",
        json!({ "query": "多重均衡", "mode": "entry", "limit": 80 }),
    );
    let entry_gui = gui.get(&format!("/api/search?q={}&mode=entry", enc("多重均衡")));
    assert_eq!(entry_gui, entry_mcp, "one contract, two transports");
    assert_eq!(entry_mcp["mode"], json!("entry"));
    assert_eq!(entry_mcp["register_missing"], json!(false));
    assert_eq!(entry_mcp["register_faults"], json!([]));
    let rows = entry_mcp["results"].as_array().expect("results");
    let entry_rows = rows
        .iter()
        .filter(|row| row["role"] == json!("entry"))
        .collect::<Vec<_>>();
    assert_eq!(
        entry_rows
            .iter()
            .map(|row| row["address"].as_str().unwrap().to_string())
            .collect::<std::collections::BTreeSet<_>>(),
        [f.book_a.clone(), f.book_b.clone(), f.other_idea.clone()]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        "the accepted term opens exactly its three targets: {entry_mcp}"
    );
    assert!(
        entry_rows
            .iter()
            .all(|row| row["entry_targets"] == json!([f.book_a, f.book_b, f.other_idea])),
        "every row names the register row's three targets: {entry_mcp}"
    );
    assert!(entry_rows.iter().all(|row| row["matched_term"] == json!("多重均衡")));
    assert_eq!(
        rows.iter()
            .filter(|row| row["role"] == json!("entry"))
            .count(),
        3,
        "a term opens its targets once each: {entry_mcp}"
    );

    // The omitted mode keeps each transport's legacy plain shape, and both rank the same Cards.
    let plain_mcp = mcp.ok("search_cards", json!({ "query": "多重均衡" }));
    let plain_gui = gui.get(&format!("/api/search?q={}", enc("多重均衡")));
    assert!(
        plain_mcp.get("mode").is_none() && plain_mcp.get("total").is_none(),
        "the MCP plain answer keeps its legacy shape: {plain_mcp}"
    );
    assert_eq!(plain_gui["mode"], json!("plain"));
    assert_eq!(plain_gui["total"], json!(plain_gui["results"].as_array().unwrap().len()));
    assert!(
        plain_gui["results"][0].get("field").is_some()
            && plain_gui["results"][0].get("score").is_some(),
        "the GUI plain answer keeps its presentation fields: {plain_gui}"
    );
    let plain_addresses = |value: &Value| {
        value["results"]
            .as_array()
            .expect("results")
            .iter()
            .map(|row| row["address"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    // Each legacy plain order is stable on its own transport (it is a presentation, not the shared
    // ranking): the same call twice answers the same list.
    let plain_again = mcp.ok("search_cards", json!({ "query": "多重均衡" }));
    assert_eq!(plain_again, plain_mcp, "the MCP plain order is stable");
    assert_eq!(
        gui.get(&format!("/api/search?q={}", enc("多重均衡"))),
        plain_gui,
        "the GUI plain order is stable"
    );
    let plain_mcp_addresses = plain_addresses(&plain_mcp);
    for at in [&f.book_a, &f.book_b, &f.other_idea] {
        assert!(
            plain_mcp_addresses.contains(at),
            "the plain answer names {at}: {plain_mcp}"
        );
    }
    assert!(
        plain_mcp_addresses
            .iter()
            .all(|at| plain_gui["results"].as_array().unwrap().iter().any(|row| row["address"] == json!(at))),
        "every plain Card the MCP server names is in the GUI's plain answer too"
    );

    // An empty query on the MCP server stays the type listing in front of entry mode (D198, D341).
    let listing_mcp = mcp.ok("search_cards", json!({ "type": "hub" }));
    let listing_gui = gui.get("/api/search?q=&type=hub");
    assert_eq!(listing_gui["mode"], json!("plain"));
    let listing_entry = mcp.ok("search_cards", json!({ "type": "hub", "mode": "entry" }));
    assert_eq!(
        listing_entry, listing_mcp,
        "an empty query keeps the plain listing whatever mode was asked for"
    );
    let hub_listing = listing_mcp["results"].as_array().expect("hub listing");
    assert_eq!(hub_listing.len(), 2, "{listing_mcp}");
    assert!(
        hub_listing
            .iter()
            .any(|row| row["address"] == json!(f.register))
            && hub_listing.iter().any(|row| row["address"] == json!(f.hub))
    );
    // A type filter narrows the shared entry ranking on both transports.
    let typed_mcp = mcp.ok(
        "search_cards",
        json!({ "query": "多重均衡", "mode": "entry", "type": "hub", "limit": 80 }),
    );
    let typed_gui = gui.get(&format!(
        "/api/search?q={}&mode=entry&type=hub",
        enc("多重均衡")
    ));
    assert_eq!(typed_gui, typed_mcp, "a type filter keeps one value");
    assert!(
        typed_mcp["results"]
            .as_array()
            .expect("results")
            .iter()
            .all(|row| row["type"] == json!("hub")),
        "{typed_mcp}"
    );

    // One Ask proposal over the same archive, byte-identical, and it writes nothing (D211-D231).
    let question = "为什么策略互补会产生多重均衡？";
    let ask_mcp = mcp.ok("ask_box", json!({ "question": question }));
    let ask_gui = gui.post("/api/ask", &json!({ "question": question }));
    assert_eq!(ask_gui, ask_mcp, "one proposal, two transports");
    assert_eq!(ask_mcp["held"], json!(f.book_a));
    assert!(
        ask_mcp["matched_terms"]
            .as_array()
            .is_some_and(|terms| terms.contains(&json!("多重均衡")))
    );
    let proposed = ask_mcp["spread"]["cards"].as_array().expect("proposal cards");
    assert!(
        proposed.iter().any(|place| place["at"] == json!(f.book_a))
            && proposed.iter().any(|place| place["at"] == json!(f.book_b)),
        "the proposal holds both books' treatments: {ask_mcp}"
    );

    // One register projection, byte-identical, with the three targets and the accepted senses.
    let register_mcp = mcp.ok("get_register", json!({ "workspace": "agent" }));
    let register_gui = gui.get("/api/register?ws=agent");
    assert_eq!(register_gui, register_mcp, "one register, two transports");
    assert_eq!(register_mcp["complete"], json!(true));
    assert_eq!(register_mcp["register_missing"], json!(false));
    assert_eq!(register_mcp["entry_total"], json!(2));
    let multi = term_rows(&register_mcp, "多重均衡");
    assert_eq!(multi.len(), 1, "one row, one sense: {register_mcp}");
    assert_eq!(
        multi[0]["targets"]
            .as_array()
            .expect("targets")
            .iter()
            .map(|target| target["address"].as_str().unwrap().to_string())
            .collect::<Vec<_>>(),
        vec![f.book_a.clone(), f.book_b.clone(), f.other_idea.clone()]
    );
    assert!(multi[0]["targets"].as_array().unwrap().iter().all(|target| {
        target["exists"] == json!(true) && target["title"].is_string() && target["body"].is_string()
    }));

    // The model and hub reports name the parts and members the fixture filed.
    let model = mcp.ok("get_model", json!({ "at": f.model }));
    assert_eq!(model["level"], json!("concept"));
    assert!(
        model["parts"]
            .as_array()
            .is_some_and(|parts| parts.iter().any(|part| part["facet"] == json!("设定"))),
        "{model}"
    );
    let hub = mcp.ok("get_hub", json!({ "at": f.hub }));
    assert_eq!(hub["level"], json!("line"));
    let lines = hub["lines"].as_array().expect("hub lines");
    assert_eq!(lines.len(), 2, "{hub}");
    assert_eq!(lines[0]["address"], json!(f.idea));
    assert_eq!(lines[1]["address"], json!(f.book_b));
    assert!(
        hub["faults"].as_array().is_some_and(|faults| !faults
            .iter()
            .any(|fault| fault["fault"] == json!("anchor-title"))),
        "the hub title does not copy its anchor: {hub}"
    );
    assert_eq!(mcp.ok("list_broken_links", json!({})), json!({ "broken": [] }));

    // One distant Card: one candidate from another supported book, and opening it writes nothing.
    // The Idea Card already links book A's Card, so the candidate is book B's treatment of the term
    // it carries, not the Card it already reaches.
    let distant = gui.get(&format!("/api/distant?at={}", enc(&f.idea)));
    assert_eq!(distant["at"], json!(f.idea));
    assert_eq!(
        distant["candidate"]["address"],
        json!(f.book_b),
        "the one distant Card is the other book's treatment: {distant}"
    );
    assert_eq!(distant["candidate"]["source"], json!("register_term"));
    assert_eq!(distant["candidate"]["term"], json!("学习"));
    assert_eq!(
        distant["candidate"]["evidence"]["focus_sources"],
        json!([f.book_a.clone()[..5].to_string()])
    );
    assert_eq!(distant["candidate"]["evidence"]["sources"], json!(["bookB"]));
    assert_eq!(distant["candidate"]["evidence"]["mixed"], json!(false));
    assert_eq!(
        distant["candidate"]["origin"]["card"],
        json!(f.register),
        "{distant}"
    );
    assert_eq!(
        distant["excluded"]["linked"],
        json!(3),
        "the Card it already links is never offered: {distant}"
    );
    assert!(distant["source"].is_string());

    // The reads changed nothing: no authored text, no Spread, no register, no revision.
    assert_eq!(fingerprints(&gui), prints, "a read rewrites no authored Card");
    assert_eq!(state(&gui), before, "a read changes no visible state");
    assert_eq!(mcp.revision(), revision, "a read publishes no revision");
}

/// AC04/AC06: the Owner's three writes each add exactly the Cards they address, keep every other
/// authored fingerprint, open one distant Card at most, and survive a service restart.
#[test]
fn the_owner_writes_change_only_what_they_write_and_survive_a_restart() {
    let home = TestHome::new();
    let f = fixture(&home);
    let gui = home.gui();
    let mut mcp = home.mcp();
    let prints = fingerprints(&gui);
    let revision = mcp.revision();

    // Keep the Ask proposal: one Spread with the whole question, no Card changed (S16, D233).
    let question = "为什么策略互补会产生多重均衡？";
    let ask = gui.post("/api/ask", &json!({ "question": question }));
    let kept = gui.post(
        "/api/spreads",
        &keep_document(&gui.get("/api/spreads"), &ask, "kept-release"),
    );
    let spread = kept_spread(&kept, "kept-release");
    assert_eq!(spread["question"], json!(question));
    assert_eq!(spread["name"], json!(question), "a short question is no cut name");
    assert_eq!(spread_addresses(&spread), spread_addresses(&ask["spread"]));
    assert_eq!(fingerprints(&gui), prints, "Keep writes no Card");
    assert_eq!(mcp.revision(), revision, "Keep publishes no revision");

    // Reply: one human Card, the default Topic it needs, never a change to the agent Card.
    let target = gui.card(&f.book_b);
    let text = format!(
        "# 同一机制的另一种讲法\n\n印证 [[{}]]：策略互补在另一本书里也解释多重均衡。\n",
        f.book_b
    );
    let reply = gui.post(
        "/api/reply",
        &json!({ "at": f.book_b, "base": target["authored_fingerprint"], "text": text }),
    );
    assert_eq!(reply["saved"], json!(true), "{reply}");
    assert_eq!(reply["kind"], json!("illustrates"));
    assert_eq!(reply["target"]["address"], json!(f.book_b));
    assert_eq!(
        reply["target"]["authored_fingerprint"], target["authored_fingerprint"],
        "the agent Card's authored identity is untouched"
    );
    assert_eq!(
        reply["destination"]["created_topic"],
        json!(true),
        "an empty human archive gets its default Topic: {reply}"
    );
    let reply_at = address(&reply);
    let after_reply = fingerprints(&gui);
    assert_eq!(
        after_reply.len(),
        prints.len() + 2,
        "only the default Topic and the Reply are new"
    );
    for (at, print) in &prints {
        assert_eq!(
            after_reply.get(at),
            Some(print),
            "{at} keeps its authored text through the Reply"
        );
    }
    let human = gui.card(&reply_at);
    assert_eq!(human["workspace"], json!("human"));
    assert_eq!(human["type"], json!("idea"));
    assert!(human["body"].as_str().is_some_and(|body| body.contains(&f.book_b)));
    let target_after = gui.card(&f.book_b);
    assert!(
        target_after["inbound"]
            .as_array()
            .expect("inbound")
            .iter()
            .any(|link| link["address"] == json!(reply_at)),
        "the generated Reverse link carries the connection: {target_after}"
    );

    // A changed target keeps the draft: the stale base is refused and nothing is written.
    let edited = mcp.ok(
        "edit_card",
        json!({ "at": f.book_b, "body": "策略互补解释多重均衡。一个市场可以有两个稳定价格。\n\n来源：[[bookB]]" }),
    );
    assert_eq!(edited["address"], json!(f.book_b));
    let before_stale = gui.get("/api/graph");
    let stale = gui.post(
        "/api/reply",
        &json!({ "at": f.book_b, "base": target["authored_fingerprint"], "text": text }),
    );
    assert_eq!(stale["saved"], json!(false), "{stale}");
    assert_eq!(stale["reason"], json!("target_changed"));
    assert_eq!(
        stale["target"]["authored_fingerprint"],
        gui.card(&f.book_b)["authored_fingerprint"],
        "the refusal hands back the fresh target"
    );
    assert_eq!(
        gui.get("/api/graph"),
        before_stale,
        "a refused Reply writes nothing"
    );

    // File the kept Spread as a field hub draft: the plan writes nothing, Save writes one Card.
    let kept_question = kept_spread(&gui.get("/api/spreads"), "kept-release");
    let question = kept_question["question"].as_str().expect("question");
    let before_draft = fingerprints(&gui);
    let draft = gui.post(
        "/api/hub/draft",
        &json!({
            "cards": [f.idea, f.book_b],
            "question": question,
            "title": "策略互补与干中学",
            "reasons": {
                f.idea.clone(): "把干中学放进这条线。",
                f.book_b.clone(): "另一本书的同一机制。",
            },
        }),
    );
    assert_eq!(draft["missing"], Value::Null, "{draft}");
    assert_eq!(draft["level"], json!("field"));
    assert_eq!(draft["destination"]["at"], json!(f.register));
    assert_eq!(draft["check"]["ok"], json!(true), "{draft}");
    assert_eq!(
        fingerprints(&gui),
        before_draft,
        "the draft plan writes no Card"
    );
    let before_save = fingerprints(&gui);
    let saved = gui.post(
        "/api/create",
        &json!({
            "kind": draft["destination"]["how"],
            "at": draft["destination"]["at"],
            "text": draft["draft"]["text"],
            "type": "hub",
            "level": draft["level"],
        }),
    );
    let filed = address(&saved);
    assert!(
        filed.starts_with(&format!("{}|", f.register)),
        "filed under the register: {filed}"
    );
    let after_save = fingerprints(&gui);
    assert_eq!(after_save.len(), before_save.len() + 1);
    for (at, print) in &before_save {
        assert_eq!(
            after_save.get(at),
            Some(print),
            "{at} keeps its authored text through the filing"
        );
    }
    let filed_card = gui.card(&filed);
    assert_eq!(filed_card["type"], json!("hub"));
    assert_eq!(filed_card["level"], json!("field"));
    assert_eq!(filed_card["workspace"], json!("agent"));
    let filed_hub = mcp.ok("get_hub", json!({ "at": filed }));
    let members = filed_hub["lines"].as_array().expect("lines");
    assert!(
        members.iter().any(|line| line["address"] == json!(f.idea))
            && members.iter().any(|line| line["address"] == json!(f.book_b)),
        "{filed_hub}"
    );

    // A restart keeps every saved state: the Spreads, the register, the humans, the search.
    let spreads = gui.get("/api/spreads");
    let register = gui.get("/api/register?ws=agent");
    let search = gui.get(&format!("/api/search?q={}&mode=entry", enc("多重均衡")));
    let graph = gui.get("/api/graph");
    let revision = mcp.revision();
    let gui = home.restart();
    assert_eq!(gui.get("/api/spreads"), spreads, "the kept Spread survives");
    assert_eq!(
        gui.get("/api/register?ws=agent"),
        register,
        "the register survives"
    );
    assert_eq!(
        gui.get(&format!("/api/search?q={}&mode=entry", enc("多重均衡"))),
        search,
        "the ranking survives"
    );
    assert_eq!(gui.get("/api/graph"), graph, "every Card survives");
    assert_eq!(fingerprints(&gui), after_save, "no restart rewrites a Card");
    let mut mcp = home.mcp();
    assert_eq!(mcp.revision(), revision, "a restart publishes no revision");
    assert_eq!(gui.card(&reply_at)["workspace"], json!("human"));
}

/// AC04/AC07: the ordinary move, Citation-key change and delete paths keep the saved Spread, the
/// register entries and the Link index current, and rewrite no authored text but the Links to the
/// Cards whose addresses changed.
#[test]
fn moves_a_citation_change_and_a_delete_keep_the_saved_state_current() {
    let home = TestHome::new();
    let f = fixture(&home);
    let gui = home.gui();
    let mut mcp = home.mcp();
    // One kept Spread holding the Idea Card a move renumbers and the book Card a rename renames.
    let doc = spread_document(
        &gui.get("/api/spreads"),
        "release",
        "Release set",
        "这套卡说明什么？",
        &[(f.idea.as_str(), 0.0), (f.book_b.as_str(), 236.0)],
    );
    let saved = gui.post("/api/spreads", &doc);
    assert_eq!(
        spread_addresses(&kept_spread(&saved, "release")),
        vec![f.idea.clone(), f.book_b.clone()]
    );
    let prints = fingerprints(&gui);

    // Insert before the Idea Card: the new Card takes its place, the Card moves down one place, and
    // every authored Link to it follows.
    let inserted_text = "# 规模报酬递增的来源\n\n规模报酬递增时，厂商的最优选择不唯一。\n";
    let plan = gui.post("/api/insert", &json!({ "before": f.idea, "text": inserted_text }));
    assert_eq!(plan["confirm"], json!("insert"));
    assert!(fingerprints(&gui) == prints, "the plan writes nothing");
    let done = gui.post(
        "/api/insert",
        &json!({ "before": f.idea, "text": inserted_text, "confirm": "insert" }),
    );
    let inserted = address(&done);
    assert_eq!(
        inserted, f.idea,
        "the new Card takes the addressed place: {done}"
    );
    let moved = done["moved"].clone();
    let idea_now = translate(&moved, &f.idea);
    let other_now = translate(&moved, &f.other_idea);
    let hub_now = translate(&moved, &f.hub);
    assert_ne!(idea_now, f.idea, "{done}");
    assert_eq!(
        gui.card(&idea_now)["title"],
        json!("干中学提高生产率"),
        "the moved Card kept its title"
    );
    let spreads = gui.get("/api/spreads");
    assert!(
        spread_addresses(&kept_spread(&spreads, "release")).contains(&idea_now),
        "the saved Spread follows the moved Card: {spreads}"
    );
    let register_now = gui.get("/api/register?ws=agent");
    let learning = term_rows(&register_now, "学习");
    assert_eq!(learning.len(), 1);
    assert_eq!(
        learning[0]["targets"][0]["address"],
        json!(idea_now),
        "the register row follows the move"
    );
    let hub_report = mcp.ok("get_hub", json!({ "at": hub_now }));
    assert!(
        hub_report["lines"]
            .as_array()
            .expect("lines")
            .iter()
            .any(|line| line["address"] == json!(idea_now)),
        "the hub member line follows the move: {hub_report}"
    );
    // Only the two Cards that named the moved address have new authored text.
    let after_insert = fingerprints(&gui);
    let mut changed = Vec::new();
    for (at, print) in &prints {
        if after_insert.get(&translate(&moved, at)) != Some(print) {
            changed.push(at.clone());
        }
    }
    changed.sort();
    let mut expected = vec![f.hub.clone(), f.register.clone()];
    expected.sort();
    assert_eq!(
        changed, expected,
        "only the hub body and the register row named the moved address: {after_insert:?}"
    );
    for (at, print) in &prints {
        if at == &f.hub || at == &f.register {
            continue;
        }
        assert_eq!(
            after_insert.get(&translate(&moved, at)),
            Some(print),
            "{at} keeps its authored text through the insert"
        );
    }

    // A Citation-key change: the whole Literature tree moves, every Link to it is rewritten, and the
    // saved Spread follows.
    let bibtex = "@book{bookC, title={Book C}}";
    let plan = gui.post(
        "/api/metadata",
        &json!({ "at": f.book_b_root, "bibtex": bibtex }),
    );
    assert_eq!(plan["confirm"], json!("move"), "a rename needs confirmation");
    assert!(
        fingerprints(&gui) == after_insert,
        "the rename plan writes nothing"
    );
    let renamed = gui.post(
        "/api/metadata",
        &json!({ "at": f.book_b_root, "bibtex": bibtex, "confirm": "move" }),
    );
    assert_eq!(renamed["address"], json!("bookC"), "{renamed}");
    let spreads = gui.get("/api/spreads");
    assert!(
        spread_addresses(&kept_spread(&spreads, "release")).contains(&"bookC/1".to_string()),
        "the saved Spread follows the Citation-key change: {spreads}"
    );
    let register_now = gui.get("/api/register?ws=agent");
    let multi = term_rows(&register_now, "多重均衡");
    assert_eq!(
        multi[0]["targets"][1]["address"],
        json!("bookC/1"),
        "the register row follows the rename"
    );
    assert!(
        gui.card(&f.book_a)["body"]
            .as_str()
            .is_some_and(|body| body.contains("[[bookC/1]]")),
        "the other book's authored Link follows the rename"
    );
    let hub_report = mcp.ok("get_hub", json!({ "at": hub_now }));
    assert!(
        hub_report["lines"]
            .as_array()
            .expect("lines")
            .iter()
            .any(|line| line["address"] == json!("bookC/1")),
        "{hub_report}"
    );
    assert_eq!(
        mcp.ok("list_broken_links", json!({})),
        json!({ "broken": [] }),
        "a rename breaks no Link"
    );
    // The Cards that named neither renamed address keep their authored text exactly.
    let after_rename = fingerprints(&gui);
    for at in [&idea_now, &other_now, &f.model, &f.part] {
        assert_eq!(
            after_rename.get(at),
            after_insert.get(at),
            "{at} keeps its authored text through the rename"
        );
    }

    // A delete of a register target: the entry names the missing target, the other entries stay, and
    // no remaining authored text changes.
    let before_delete = fingerprints(&gui);
    let plan = gui.post("/api/delete", &json!({ "at": other_now }));
    let confirmation = plan["confirm"].as_str().expect("the plan asks for confirmation");
    assert_eq!(confirmation, "delete", "{plan}");
    assert!(
        fingerprints(&gui) == before_delete,
        "the delete plan writes nothing"
    );
    let done = gui.post(
        "/api/delete",
        &json!({ "at": other_now, "confirm": confirmation }),
    );
    assert_eq!(done["deleted"], json!(1), "{done}");
    let compaction = done["moved"].clone();
    assert_eq!(compaction, json!({}), "a lone Side successor leaves no gap: {done}");
    let register_now = gui.get("/api/register?ws=agent");
    let multi = term_rows(&register_now, "多重均衡");
    assert_eq!(
        multi.len(),
        1,
        "the row stays and names its gap: {register_now}"
    );
    assert_eq!(multi[0]["complete"], json!(false));
    assert!(
        multi[0]["targets"]
            .as_array()
            .expect("targets")
            .iter()
            .any(|target| target["address"] == json!(other_now)
                && target["exists"] == json!(false)),
        "the deleted target is named, never dropped: {register_now}"
    );
    let learning = term_rows(&register_now, "学习");
    assert_eq!(
        learning[0]["targets"][0]["address"],
        json!(idea_now),
        "the other entry stays current"
    );
    assert_eq!(
        gui.get("/api/register?ws=agent"),
        mcp.ok("get_register", json!({ "workspace": "agent" })),
        "the register keeps one value over both transports"
    );
    let after_delete = fingerprints(&gui);
    assert_eq!(after_delete.len(), before_delete.len() - 1);
    for (at, print) in &before_delete {
        if at == &other_now {
            continue;
        }
        assert_eq!(
            after_delete.get(&translate(&compaction, at)),
            Some(print),
            "a delete changes no authored text of {at}"
        );
    }

    // Every stored Reverse section still matches the Link index, and the search still answers. The
    // broken row is reported and left out of retrieval, and the other row still opens its target.
    home.cmd()
        .arg("status")
        .arg("--check-links")
        .assert()
        .success();
    let found = gui.get(&format!("/api/search?q={}&mode=entry", enc("多重均衡")));
    assert_eq!(found["register_missing"], json!(false));
    assert!(
        found["register_faults"].as_array().is_some_and(|faults| faults
            .iter()
            .any(|fault| fault["fault"] == json!("broken")
                && fault["card"] == json!(f.register)
                && fault["detail"]
                    .as_str()
                    .is_some_and(|detail| detail.contains(&other_now)))),
        "the broken row names its Card, line and target: {found}"
    );
    assert!(
        !found["results"]
            .as_array()
            .expect("results")
            .iter()
            .any(|row| row["role"] == json!("entry")),
        "a broken row opens no target: {found}"
    );
    let other = gui.get(&format!("/api/search?q={}&mode=entry", enc("学习")));
    let rows = other["results"].as_array().expect("results");
    assert!(
        rows.iter()
            .any(|row| row["address"] == json!(idea_now) && row["role"] == json!("entry")),
        "the other accepted term still opens its target: {other}"
    );
    assert!(
        !rows.iter().any(|row| row["address"] == json!(other_now)),
        "a deleted Card is gone from the ranking: {other}"
    );
}
