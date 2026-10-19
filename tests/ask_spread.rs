//! Ask the box: one bounded Spread proposal from a question, two transports, no write on asking,
//! and one ordinary Spread on Keep.
//!
//! `src/entry.rs::gather` reads one question over the supplied Cards: it matches accepted register
//! terms, walks at most two authored semantic steps from the entry seeds, and proposes up to nine
//! explained Cards that hold the best entry Card (§9.3-§9.5, D211-D231). The MCP server's
//! `ask_box` and the GUI's authenticated `POST /api/ask` return that one proposal for the same
//! archive and question, and neither writes a Card, a Link, or a Spread. S16 (#116) adds the
//! reader's Keep: the page posts the ordinary `/api/spreads` document, whose new Spread carries the
//! whole optional question (D233) while its display name stays within 80 characters. The archive is
//! a scratch one (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`).

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
            env: env
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
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

    /// Every tool the server advertises, so a test can see the ask tool and its schema.
    fn tools(&mut self) -> Vec<Value> {
        let reply = self.request("tools/list", json!({}));
        reply["result"]["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default()
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
    /// One authenticated GET; the JSON body of the answer.
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

    fn ask(&self, body: &Value) -> Value {
        self.post("/api/ask", body)
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

/// Two books that share one accepted term and link to each other, and the register that accepts it.
fn two_books(home: &TestHome) -> (Gui, Value) {
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
        "策略互补",
        "策略互补解释多重均衡。\n\n来源：[[bookB]]",
    );
    mcp.ok(
        "edit_card",
        json!({ "at": focus, "body": "多重均衡来自策略互补。对应 [[bookB/1]]。\n\n来源：[[bookA]]" }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": other, "body": "策略互补解释多重均衡。对照 [[bookA/1]]。\n\n来源：[[bookB]]" }),
    );
    register(
        &mut mcp,
        "## 词条\n- 多重均衡：[[bookA/1]]\n- 策略互补：[[bookB/1]]\n",
    );
    let gui = home.gui();
    (gui, json!({ "focus": focus, "other": other }))
}

/// The archive state a question must leave exactly as it found it.
fn state(gui: &Gui) -> Value {
    json!({
        "overview": gui.get("/api/overview"),
        "graph": gui.get("/api/graph"),
        "spreads": gui.get("/api/spreads"),
        "register": gui.get("/api/register?ws=agent"),
        "broken": gui.get("/api/broken"),
    })
}

/// The Spread document the page's Keep posts to `/api/spreads` (`ZtAsk.askKeep`): every Spread the
/// page holds, then this question's own, as current. The reasons and fingerprints stay in the page.
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

#[test]
fn one_question_proposal_serves_both_transports_without_a_write() {
    // No model or helper may be started: a helper that cannot run makes an accidental call fail.
    let home = TestHome::new_with(&[("ZT_DIGEST_HELPER", "Z:\\nowhere\\zt-digest.exe")]);
    let (gui, cards) = two_books(&home);
    assert_eq!(cards["focus"], json!("bookA/1"));
    assert_eq!(cards["other"], json!("bookB/1"));
    let question = json!({ "question": "为什么多重均衡可能出现？" });
    let before = state(&gui);

    let mut mcp = home.mcp();
    let tools = mcp.tools();
    let ask = tools
        .iter()
        .find(|tool| tool["name"] == json!("ask_box"))
        .expect("the server advertises ask_box");
    assert_eq!(ask["annotations"]["readOnlyHint"], json!(true));
    assert_eq!(
        ask["inputSchema"]["required"],
        json!(["question"]),
        "the question is the only required argument"
    );
    assert_eq!(ask["inputSchema"]["properties"]["limit"]["minimum"], json!(5));
    assert_eq!(ask["inputSchema"]["properties"]["limit"]["maximum"], json!(9));

    let proposal = mcp.ok("ask_box", question.clone());
    let value = gui.ask(&question);
    assert_eq!(value, proposal, "both transports return one proposal");

    assert_eq!(proposal["question"], json!("为什么多重均衡可能出现？"));
    assert_eq!(proposal["workspace"], json!("all"));
    assert_eq!(proposal["limit"], json!(7));
    assert_eq!(proposal["matched_terms"], json!(["多重均衡"]));
    assert_eq!(proposal["ambiguous_terms"], json!([]));
    assert_eq!(proposal["fallback"], Value::Null);
    assert_eq!(proposal["held"], json!("bookA/1"));
    assert_eq!(
        proposal["spread"]["name"],
        json!("为什么多重均衡可能出现？")
    );
    assert_eq!(
        proposal["spread"]["question"],
        json!("为什么多重均衡可能出现？")
    );
    assert_eq!(proposal["spread"]["cards"][0]["at"], json!("bookA/1"));
    assert_eq!(proposal["spread"]["cards"][1]["at"], json!("bookB/1"));
    let card = &proposal["cards"][0];
    assert_eq!(card["at"], json!("bookA/1"));
    assert_eq!(card["held"], json!(true));
    assert_eq!(card["term"], json!("多重均衡"));
    assert_eq!(card["depth"], json!(0));
    assert!(card["authored_fingerprint"].is_string());
    assert_eq!(proposal["cards"][1]["at"], json!("bookB/1"));
    assert_eq!(proposal["cards"][1]["depth"], json!(1));
    assert_eq!(
        proposal["cards"][1]["path"][0]["kind"], json!("corresponds"),
        "the step keeps its authored kind"
    );
    assert_eq!(proposal["cards"][1]["path"][0]["direction"], json!("out"));
    // Both Cards link to each other, so two authored threads join them.
    assert_eq!(proposal["threads"].as_array().expect("threads").len(), 2);
    assert_eq!(proposal["sparse"], json!(true), "two Cards is sparse");
    assert_eq!(proposal["truncated"], json!(false));
    assert_eq!(proposal["counts"]["visited"], json!(2));
    assert_eq!(proposal["bounds"]["terms"], json!(6));
    assert!(proposal["archive_revision"].as_u64().is_some());
    assert!(proposal["archive_identity"]["id"].is_string());
    assert_eq!(proposal["registers"][1]["workspace"], json!("agent"));
    assert_eq!(proposal["registers"][1]["complete"], json!(true));

    // Reading a proposal changes nothing in the archive.
    assert_eq!(state(&gui), before, "the question changed the archive");
    // The same read answers the same way.
    assert_eq!(
        mcp.ok("ask_box", question.clone()),
        proposal,
        "a repeated question gives the same proposal"
    );
}

#[test]
fn a_question_refuses_a_blank_or_oversize_input_on_both_transports() {
    let home = TestHome::new();
    let (gui, _cards) = two_books(&home);
    let mut mcp = home.mcp();

    for (body, why) in [
        (json!({}), "a missing question"),
        (json!({ "question": "  " }), "a blank question"),
        (json!({ "question": "学".repeat(513) }), "a question over 512 characters"),
    ] {
        let (value, is_error) = mcp.call("ask_box", body.clone());
        assert!(is_error, "{why} must be refused by the MCP tool: {value}");
        let (status, refused) = gui.post_with("/api/ask", &gui.token, &body);
        assert_eq!(status, 400, "{why} must be refused by the route: {refused}");
        assert!(
            refused["error"].as_str().is_some_and(|error| !error.is_empty()),
            "{why} needs a reason: {refused}"
        );
    }

    // The token guard stays in front of the route.
    let (status, refused) =
        gui.post_with("/api/ask", "not-the-token", &json!({ "question": "问题" }));
    assert_eq!(status, 403, "{refused}");
    assert_eq!(refused["error"], json!("missing or invalid GUI token"));

    // The bound counts characters, not bytes: 512 Chinese characters are accepted.
    let longest = json!({ "question": "学".repeat(512) });
    let (status, value) = gui.post_with("/api/ask", &gui.token, &longest);
    assert_eq!(status, 200, "{value}");
    assert_eq!(value["question"], longest["question"]);
    // A Workspace filter and the selection size are accepted and echoed.
    let value = gui.ask(&json!({ "question": "多重均衡", "workspace": "agent", "limit": 9 }));
    assert_eq!(value["workspace"], json!("agent"));
    assert_eq!(value["limit"], json!(9));
    let value = gui.ask(&json!({ "question": "多重均衡", "workspace": "human" }));
    assert_eq!(value["workspace"], json!("human"));
    assert_eq!(
        value["held"],
        Value::Null,
        "the human Workspace holds no matching entry: {value}"
    );
}

/// S16 (#116, D232, D233; S16-AC01, S16-AC02, S16-AC06): Keep posts the ordinary Spread document
/// through the one `/api/spreads` path. The kept Spread carries the whole question while only its
/// display name is cut to 80 characters, survives a restart, and asking again (MCP or GUI) still
/// writes nothing — a failed question leaves the saved Set exactly as it was.
#[test]
fn a_kept_question_survives_a_restart_and_a_failed_question_leaves_it_alone() {
    let home = TestHome::new();
    let (gui, _cards) = two_books(&home);
    let question = "为什么学习可能造成多重均衡？".repeat(9);
    assert!(question.chars().count() > 80 && question.chars().count() <= 512);
    assert_eq!(
        gui.get("/api/spreads"),
        json!({ "current": Value::Null, "spreads": [] })
    );

    let proposal = gui.ask(&json!({ "question": question }));
    assert_eq!(proposal["spread"]["question"], json!(question));
    assert_eq!(
        proposal["spread"]["name"].as_str().expect("name").chars().count(),
        80,
        "the display name is cut; the question is whole: {proposal}"
    );
    let cards: Vec<String> = proposal["spread"]["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|card| card["at"].as_str().expect("at").to_string())
        .collect();
    assert_eq!(cards, ["bookA/1", "bookB/1"]);

    // Keep: the page's own document, through the existing save route.
    let kept = gui.post("/api/spreads", &keep_document(&gui.get("/api/spreads"), &proposal, "kept-1"));
    assert_eq!(kept["current"], json!("kept-1"));
    assert_eq!(kept["spreads"].as_array().expect("spreads").len(), 1);
    let spread = kept_spread(&kept, "kept-1");
    assert_eq!(spread["question"], json!(question), "the whole question is kept");
    assert_eq!(spread["name"].as_str().expect("name").chars().count(), 80);
    let kept_cards = spread["cards"].as_array().expect("cards");
    let proposed_cards = proposal["spread"]["cards"].as_array().expect("cards");
    assert_eq!(kept_cards.len(), proposed_cards.len());
    for (kept_card, proposed_card) in kept_cards.iter().zip(proposed_cards) {
        assert_eq!(kept_card["at"], proposed_card["at"], "its Cards are kept");
        assert_eq!(kept_card["x"].as_f64(), proposed_card["x"].as_f64(), "its places are kept");
        assert_eq!(kept_card["y"].as_f64(), proposed_card["y"].as_f64());
    }
    assert_eq!(gui.get("/api/spreads"), kept);

    // Asking again writes nothing, from either transport, and answers the same proposal.
    let mut mcp = home.mcp();
    assert_eq!(mcp.ok("ask_box", json!({ "question": question })), proposal);
    assert_eq!(gui.get("/api/spreads"), kept);

    // A failed question keeps the saved set: blank, over 512 characters, and a missing token.
    for body in [
        json!({ "question": "   " }),
        json!({ "question": "学".repeat(513) }),
    ] {
        let (status, refused) = gui.post_with("/api/ask", &gui.token, &body);
        assert_eq!(status, 400, "{refused}");
        assert_eq!(gui.get("/api/spreads"), kept, "a failed question changes no Spread");
    }
    let (status, refused) = gui.post_with("/api/ask", "not-the-token", &json!({ "question": "多重均衡" }));
    assert_eq!(status, 403, "{refused}");
    assert_eq!(gui.get("/api/spreads"), kept);

    // The data the page's Keep check compares (D234): the proposal captured the endpoint's authored
    // fingerprint, an edit moves it, and a vanished Card no longer loads — never a new occupant
    // silently standing in for the selected Card.
    let before = gui.get("/api/card?at=bookB%2F1");
    assert_eq!(
        before["authored_fingerprint"], proposal["cards"][1]["authored_fingerprint"],
        "the proposal captured the endpoint's authored fingerprint"
    );
    mcp.ok(
        "edit_card",
        json!({ "at": "bookB/1", "body": "策略互补解释多重均衡。另见 [[bookA/1]]。\n\n来源：[[bookB]]" }),
    );
    let after = gui.get("/api/card?at=bookB%2F1");
    assert_ne!(
        after["authored_fingerprint"], before["authored_fingerprint"],
        "an edited endpoint's fingerprint moves, so Keep sees the change"
    );
    assert_eq!(gui.get("/api/spreads"), kept, "a Card edit changes no Spread");

    // A restart keeps the kept question: it is archived metadata, not page state.
    home.cmd().arg("down").assert().success();
    home.cmd().arg("up").assert().success();
    let gui = home.gui();
    assert_eq!(gui.get("/api/spreads"), kept, "the kept question survives a restart");

    // A deleted endpoint is gone rather than followed to whoever holds its address.
    let mut mcp = home.mcp();
    mcp.ok("delete_card", json!({ "at": "bookA/1", "confirm": "delete" }));
    let (status, gone) = gui.get_with("/api/card?at=bookA%2F1", &gui.token);
    assert_eq!(status, 400, "a vanished endpoint: {gone}");
    let followed = gui.get("/api/spreads");
    let spread = kept_spread(&followed, "kept-1");
    assert_eq!(spread["question"], json!(question), "the question stays");
    assert_eq!(
        spread["cards"],
        json!([{ "at": "bookB/1", "x": 266.0, "y": 0.0 }]),
        "the deleted Card leaves the Spread"
    );
}

/// S16 (#116, D233; S16-AC03, S16-AC05): a Spread saved before the field existed loads and saves
/// unchanged beside a kept question, and the ordinary move and delete paths keep the question while
/// they rewrite or drop addresses.
#[test]
fn legacy_spreads_load_beside_a_kept_question_and_follow_a_move_and_a_delete() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let a = book(
        &mut mcp,
        "bookA",
        "Book A",
        "多重均衡的陈述",
        "多重均衡来自策略互补。\n\n来源：[[bookA]]",
    );
    let b = book(
        &mut mcp,
        "bookB",
        "Book B",
        "策略互补",
        "策略互补解释多重均衡。\n\n来源：[[bookB]]",
    );
    assert_eq!((a.as_str(), b.as_str()), ("bookA/1", "bookB/1"));
    let gui = home.gui();

    // A Spread from before the field existed: saved as it is, and read back without a question.
    let legacy = json!({ "current": "old", "spreads": [{
        "id": "old", "name": "Work",
        "cards": [{ "at": "bookA/1", "x": 0.0, "y": 0.0 }, { "at": "bookB/1", "x": 236.0, "y": 0.0 }],
    }] });
    let saved = gui.post("/api/spreads", &legacy);
    assert_eq!(saved, legacy, "a legacy document loads and saves unchanged");
    assert_eq!(gui.get("/api/spreads"), legacy);

    // A kept question beside it.
    let question = "为什么学习可能造成多重均衡？".repeat(9);
    let consulted = json!({
        "current": "consulted",
        "spreads": [
            legacy["spreads"][0].clone(),
            {
                "id": "consulted",
                "name": question.chars().take(80).collect::<String>(),
                "question": question,
                "cards": [{ "at": "bookA/1", "x": 10.0, "y": 10.0 }, { "at": "bookB/1", "x": 300.0, "y": 10.0 }],
            },
        ],
    });
    let saved = gui.post("/api/spreads", &consulted);
    assert_eq!(saved, consulted);
    assert!(
        kept_spread(&saved, "old").get("question").is_none(),
        "a legacy Spread gains no question: {saved}"
    );
    assert_eq!(kept_spread(&saved, "consulted")["question"], json!(question));

    // A move rewrites the address in place: a second Card inserted before bookA/1 takes its place,
    // so the Card the Spread holds becomes bookA/2. The question stays with the Spread.
    let second = address(mcp.ok(
        "create_successor",
        json!({ "at": "bookA/1", "title": "第二个陈述", "body": "另一条陈述。\n\n来源：[[bookA]]" }),
    ));
    assert_eq!(second, "bookA/2");
    let plan = mcp.ok("move_card", json!({ "at": "bookA/2", "before": "bookA/1" }));
    assert_eq!(plan["confirm"], json!("move"), "{plan}");
    let moved = mcp.ok(
        "move_card",
        json!({ "at": "bookA/2", "before": "bookA/1", "confirm": "move" }),
    );
    assert_eq!(moved["moved"]["bookA/1"], json!("bookA/2"), "{moved}");
    let followed = gui.get("/api/spreads");
    let consulted_after = kept_spread(&followed, "consulted");
    assert_eq!(consulted_after["question"], json!(question));
    assert_eq!(
        consulted_after["cards"],
        json!([
            { "at": "bookA/2", "x": 10.0, "y": 10.0 },
            { "at": "bookB/1", "x": 300.0, "y": 10.0 },
        ])
    );
    assert_eq!(
        kept_spread(&followed, "old")["cards"][0]["at"],
        json!("bookA/2"),
        "the legacy Spread follows too, and gains no question"
    );

    // A delete drops the Card from every Spread; the kept question stays.
    mcp.ok("delete_card", json!({ "at": "bookA/2", "confirm": "delete" }));
    let followed = gui.get("/api/spreads");
    let consulted_after = kept_spread(&followed, "consulted");
    assert_eq!(
        consulted_after["cards"],
        json!([{ "at": "bookB/1", "x": 300.0, "y": 10.0 }]),
        "a deleted Card leaves the Spread"
    );
    assert_eq!(consulted_after["question"], json!(question));
    assert!(kept_spread(&followed, "old").get("question").is_none());
}
