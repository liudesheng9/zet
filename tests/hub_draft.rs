//! Filing a kept Ask Spread as an agent Hub Card draft (S17, #118; design §9.6, D235-D237).
//!
//! `POST /api/hub/draft` reads one Spread's Cards, the question it was kept from and the reasons
//! the answer gave each, and answers where the hub would be filed, at what level, the Markdown the
//! desk opens with, every legal destination, and the precheck of that draft. The Save is the one
//! existing write route: `/api/create` as a side successor of the destination, or `/api/edit` of
//! the hub a rewrite started from. The archive is a scratch one (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`).

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
            .env("ZT_GUI_PORT", "0")
            .env("ZT_DIGEST_HELPER", "Z:\\nowhere\\zt-digest.exe");
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

    fn post(&self, path: &str, body: &Value) -> Value {
        let (status, value) = self.post_with(path, &self.token, body);
        assert_eq!(status, 200, "{path} answered {status}: {value}");
        value
    }

    fn draft(&self, body: &Value) -> Value {
        self.post("/api/hub/draft", body)
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

/// Two books that share one accepted term, and the register that accepts it.
fn two_books(home: &TestHome) -> (Gui, Value) {
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
    register(
        &mut mcp,
        "## 词条\n- 多重均衡：[[bookA/1]]\n- 策略互补：[[bookB/1]]\n",
    );
    let gui = home.gui();
    (gui, json!({ "focus": a, "other": b }))
}

/// The Spread document the page's Keep posts (`ZtAsk.askKeep`): every Spread the page holds, then
/// this question's own, as current.
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

/// The reasons the answer gave its Cards, as the page passes them to the draft.
fn reasons_of(proposal: &Value) -> Value {
    let mut reasons = serde_json::Map::new();
    for card in proposal["cards"].as_array().expect("cards") {
        reasons.insert(card["at"].as_str().expect("at").to_string(), card["reason"].clone());
    }
    Value::Object(reasons)
}

fn faults(value: &Value) -> Vec<String> {
    value["faults"]
        .as_array()
        .expect("faults")
        .iter()
        .map(|fault| fault["fault"].as_str().expect("fault id").to_string())
        .collect()
}

/// S17 (#118, S17-AC01, S17-AC02, S17-AC04, S17-AC05): a kept Ask Spread that spans both books is
/// filed as one agent Workspace field hub under the agent register, through the existing write
/// route. The plan writes nothing; the members keep their authored text; the draft passes the
/// precheck before Save.
#[test]
fn a_cross_tree_ask_spread_files_as_one_agent_field_hub() {
    let home = TestHome::new();
    let (gui, cards) = two_books(&home);
    let question = "多重均衡与策略互补有什么关系？";
    let proposal = gui.post("/api/ask", &json!({ "question": question }));
    assert_eq!(proposal["held"], cards["focus"], "{proposal}");
    let kept = gui.post(
        "/api/spreads",
        &keep_document(&gui.get("/api/spreads"), &proposal, "kept-1"),
    );
    assert_eq!(kept["current"], json!("kept-1"));
    let before = gui.get("/api/overview");
    let members = proposal["spread"]["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|card| card["at"].as_str().expect("at").to_string())
        .collect::<Vec<_>>();
    assert_eq!(members, ["bookA/1", "bookB/1"]);

    // The plan: where the draft is filed, its level, and the Markdown the desk opens with.
    let plan = gui.draft(&json!({
        "cards": members,
        "question": question,
        "title": kept["spreads"][0]["name"],
        "reasons": reasons_of(&proposal),
    }));
    assert_eq!(plan["workspace"], json!("agent"), "{plan}");
    assert_eq!(plan["level"], json!("field"), "{plan}");
    assert_eq!(plan["missing"], Value::Null, "{plan}");
    let register = plan["destination"]["at"].as_str().expect("destination");
    assert_eq!(plan["destination"]["how"], json!("side"), "{plan}");
    assert_eq!(plan["destination"]["reuse"], json!(false));
    let destination = gui.get(&format!("/api/card?at={}", enc(register)));
    assert_eq!(destination["workspace"], json!("agent"), "{destination}");
    assert_eq!(destination["level"], json!("register"), "{destination}");
    assert_eq!(
        plan["trees"].as_array().expect("trees").len(),
        2,
        "one group per Tree: {plan}"
    );
    let text = plan["draft"]["text"].as_str().expect("draft text");
    assert!(text.starts_with(&format!("# {question}\n")), "{text}");
    assert!(text.contains("范围：") && text.contains(question), "{text}");
    assert!(text.contains("## Book A") && text.contains("## Book B"), "{text}");
    for member in &members {
        assert!(text.contains(&format!("[[{member}]]")), "{member}: {text}");
    }
    assert_eq!(plan["check"]["ok"], json!(true), "the draft passes the precheck: {plan}");
    assert_eq!(plan["check"]["faults"], json!([]), "{plan}");
    // Planning wrote nothing: the archive is exactly as it was.
    assert_eq!(gui.get("/api/overview"), before, "a hub draft writes no Card");
    assert_eq!(gui.get("/api/spreads"), kept);

    // Save: the one existing create route, as a side successor of the destination.
    let mut mcp = home.mcp();
    let saved = gui.post(
        "/api/create",
        &json!({
            "kind": plan["destination"]["how"],
            "at": register,
            "text": text,
            "type": "hub",
            "level": plan["level"],
        }),
    );
    let hub = saved["address"].as_str().expect("address").to_string();
    assert!(hub.starts_with(&format!("{register}|")), "filed under the register: {hub}");
    let card = gui.get(&format!("/api/card?at={}", enc(&hub)));
    assert_eq!(card["type"], json!("hub"), "{card}");
    assert_eq!(card["level"], json!("field"), "{card}");
    assert_eq!(card["workspace"], json!("agent"), "{card}");
    assert_eq!(card["parent"]["address"], json!(register), "{card}");
    assert_eq!(card["context"], json!("regular"));
    let body = card["body"].as_str().expect("body");
    assert!(body.contains("## Book A") && body.contains("## Book B"), "{body}");
    // The members are untouched: their authored text is exactly what it was.
    for member in &members {
        let after = gui.get(&format!("/api/card?at={}", enc(member)));
        let proposal_card = proposal["cards"]
            .as_array()
            .expect("cards")
            .iter()
            .find(|card| card["at"] == json!(member))
            .expect("the proposed Card");
        assert_eq!(
            after["authored_fingerprint"], proposal_card["authored_fingerprint"],
            "filing a hub changes no member Card: {after}"
        );
    }
    // The hub report names the two members and reports no body fault.
    let report = mcp.ok("get_hub", json!({ "at": hub }));
    assert_eq!(report["level"], json!("field"), "{report}");
    assert_eq!(report["lines"].as_array().expect("lines").len(), 2, "{report}");
    let reported = report["faults"]
        .as_array()
        .expect("faults")
        .iter()
        .map(|fault| fault["fault"].as_str().expect("fault").to_string())
        .collect::<Vec<_>>();
    for named in ["bare", "broken", "anchor-title", "upward", "oversize", "flat", "level"] {
        assert!(
            !reported.contains(&named.to_string()),
            "the filed hub inherits no `{named}` fault: {report}"
        );
    }
    // Boundary of this slice: the register's `## 领域` line that anchors the field hub is the hub
    // stage's publication, not the composer's write, so the report may still say `unanchored`.
    assert!(
        reported.is_empty() || reported == ["unanchored".to_string()],
        "only the anchor line may be outstanding: {report}"
    );
}

/// S17 (#118, S17-AC02, S17-AC06): a Spread inside one agent Topic rewrites the topic hub it
/// already has, keeping the hub's authored lines; a Save against a hub that changed since the desk
/// opened it is refused with its fresh context and writes nothing.
#[test]
fn a_single_topic_ask_spread_rewrites_the_trees_topic_hub() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let topic = address(mcp.ok("create_topic", json!({ "title": "学习", "body": "本主题收放学习的卡。" })));
    let first = address(mcp.ok(
        "create_successor",
        json!({
            "at": topic,
            "title": "学习的第一张卡",
            "body": "学习会改变信念。",
        }),
    ));
    for (title, body) in [
        ("学习的个体行为", "学习会改变个体的信念。"),
        ("学习的产业生产率", "学习会提高产业的产出。"),
    ] {
        mcp.ok(
            "create_successor",
            json!({ "at": first, "kind": "side", "title": title, "body": body }),
        );
    }
    register(&mut mcp, &format!("## 词条\n- 学习：[[{first}|a]] [[{first}|b]]\n"));
    let hub = address(mcp.ok(
        "create_successor",
        json!({
            "at": first,
            "kind": "side",
            "type": "hub",
            "level": "topic",
            "title": "学习的入口",
            "body": format!("## 个体学习\n- [[{first}|a]]：学习会改变个体的信念。\n"),
        }),
    ));
    let gui = home.gui();
    let question = "学习怎样影响产出？";
    let proposal = gui.post("/api/ask", &json!({ "question": question }));
    assert_eq!(proposal["held"], json!(format!("{first}|a")), "{proposal}");
    let kept = gui.post(
        "/api/spreads",
        &keep_document(&gui.get("/api/spreads"), &proposal, "kept-1"),
    );
    let members = proposal["spread"]["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|card| card["at"].as_str().expect("at").to_string())
        .collect::<Vec<_>>();
    assert_eq!(members.len(), 2, "{proposal}");
    let before = gui.get("/api/overview");

    let plan = gui.draft(&json!({
        "cards": members,
        "question": question,
        "title": kept["spreads"][0]["name"],
        "reasons": reasons_of(&proposal),
    }));
    assert_eq!(plan["level"], json!("topic"), "{plan}");
    assert_eq!(plan["destination"]["at"], json!(hub), "{plan}");
    assert_eq!(plan["destination"]["how"], json!("edit"));
    assert_eq!(plan["destination"]["reuse"], json!(true));
    let base = plan["destination"]["base"].as_str().expect("guarded by the hub's text");
    assert_eq!(
        plan["check"]["ok"],
        json!(true),
        "the merged draft passes the precheck: {plan}"
    );
    let merged = plan["draft"]["text"].as_str().expect("text");
    assert!(
        merged.starts_with("# 学习的入口\n\n## 个体学习\n- "),
        "the hub keeps its own title and lines: {merged}"
    );
    assert!(merged.contains(&format!("## {question}")), "{merged}");
    assert_eq!(gui.get("/api/overview"), before, "a plan writes nothing");

    // The rewrite is one edit of the hub, guarded by the text the desk opened.
    let saved = gui.post("/api/edit", &json!({ "at": hub, "text": merged, "base": base }));
    assert_eq!(saved["address"], json!(hub));
    let card = gui.get(&format!("/api/card?at={}", enc(&hub)));
    assert_eq!(card["type"], json!("hub"));
    assert_eq!(card["level"], json!("topic"));
    assert_eq!(card["workspace"], json!("agent"));
    let body = card["body"].as_str().expect("body");
    assert!(body.contains("## 个体学习"), "the authored line survives: {body}");
    assert!(body.contains(&format!("## {question}")), "{body}");
    for member in &members {
        assert!(body.contains(&format!("[[{member}]]")), "{member}: {body}");
    }
    // A member Card is untouched by the rewrite.
    let member = gui.get(&format!("/api/card?at={}", enc(&members[0])));
    let proposed = proposal["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|card| card["at"] == json!(members[0]))
        .expect("card");
    assert_eq!(member["authored_fingerprint"], proposed["authored_fingerprint"]);
    // A second Save with the same base is a conflict: the hub changed since it was opened.
    mcp.ok(
        "edit_card",
        json!({
            "at": hub,
            "body": format!("## 个体学习\n- [[{first}|a]]：学习会改变个体的信念。\n\n## 另一条\n- [[{first}|b]]：这里是另一条线索。"),
        }),
    );
    let after = gui.get(&format!("/api/card?at={}", enc(&hub)));
    let (status, refused) = gui.post_with(
        "/api/edit",
        &gui.token,
        &json!({ "at": hub, "text": merged, "base": base }),
    );
    assert_eq!(status, 400, "{refused}");
    assert!(
        refused["error"]
            .as_str()
            .is_some_and(|error| error.contains("changed since it was opened")),
        "the conflict names the changed Card: {refused}"
    );
    assert_eq!(
        gui.get(&format!("/api/card?at={}", enc(&hub)))["text"],
        after["text"],
        "a refused rewrite writes nothing"
    );
}

/// S17 (#118, S17-AC03): a Spread with no agent register asks for a destination instead, and the
/// new read routes are guarded and validated exactly like the rest of the API.
#[test]
fn a_spread_without_a_register_asks_for_a_destination() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let topic = address(mcp.ok("create_topic", json!({ "title": "学习", "body": "本主题收放学习的卡。" })));
    let first = address(mcp.ok(
        "create_successor",
        json!({ "at": topic, "title": "学习的第一张卡", "body": "学习会改变信念。" }),
    ));
    let second = address(mcp.ok(
        "create_successor",
        json!({ "at": first, "kind": "side", "title": "产业学习", "body": "学习会提高产业的产出。" }),
    ));
    let book = book(&mut mcp, "bookA", "Book A", "策略互补", "策略互补解释多重均衡。\n\n来源：[[bookA]]");
    let gui = home.gui();
    let before = gui.get("/api/overview");

    let plan = gui.draft(&json!({
        "cards": [second, book],
        "question": "学习与策略互补怎样相连？",
        "reasons": {
            second: "The Card states the production reading of 学习.",
            book: "Adds Book A's treatment of complementarity.",
        },
    }));
    assert_eq!(plan["missing"], json!("register"), "{plan}");
    assert_eq!(plan["destination"], Value::Null, "{plan}");
    assert_eq!(plan["workspace"], json!("agent"));
    assert!(
        plan["note"].as_str().is_some_and(|note| note.contains("no register")),
        "the desk asks for a destination instead of filing into a viewed tree: {plan}"
    );
    let choices = plan["choices"].as_array().expect("choices");
    assert!(
        choices.iter().all(|choice| choice["level"] != json!("field")),
        "no register means no field destination: {plan}"
    );
    assert!(
        choices.iter().any(|choice| choice["at"] == json!(first) && choice["level"] == json!("topic")),
        "the agent Topic is still a destination: {plan}"
    );
    assert_eq!(plan["check"]["ok"], json!(false), "{plan}");
    assert_eq!(faults(&plan["check"]), ["destination"]);
    // Nothing was written, and the human Workspace never became a destination.
    assert_eq!(gui.get("/api/overview"), before);
    assert_eq!(gui.get("/api/spreads"), json!({ "current": Value::Null, "spreads": [] }));

    // The token guard stands in front of both new routes.
    for path in ["/api/hub/draft", "/api/hub/check"] {
        let (status, refused) = gui.post_with(path, "not-the-token", &json!({ "cards": [] }));
        assert_eq!(status, 403, "{path}: {refused}");
        assert_eq!(refused["error"], json!("missing or invalid GUI token"));
    }
    // Malformed requests are 400s with the reason.
    for (body, why) in [
        (json!({}), "no cards"),
        (json!({ "cards": [] }), "an empty Spread"),
        (json!({ "cards": ["noSuchCard"] }), "a Card that does not exist"),
    ] {
        let (status, refused) = gui.post_with("/api/hub/draft", &gui.token, &body);
        assert_eq!(status, 400, "{why}: {refused}");
        assert!(
            refused["error"].as_str().is_some_and(|error| !error.is_empty()),
            "{why} needs a reason: {refused}"
        );
    }
    // A draft is checked as typed; a refused level or a wrong placement is a fault, and a request
    // without its text, destination, or a legal level is a 400.
    for (body, why, named) in [
        (json!({ "text": "# draft\n", "level": "register", "at": first }), "the register level", "level"),
        (json!({ "text": "# draft\n", "level": "field", "at": first }), "no register for a field hub", "placement"),
    ] {
        let answer = gui.post("/api/hub/check", &body);
        assert_eq!(answer["ok"], json!(false), "{why}: {answer}");
        assert!(faults(&answer).contains(&named.to_string()), "{why}: {answer}");
    }
    for (body, why) in [
        (json!({ "text": "# draft\n", "level": "field" }), "a missing `at`"),
        (json!({ "level": "topic", "at": first }), "a missing draft text"),
        (json!({ "text": "# draft\n", "level": "concept", "at": first }), "an unknown level"),
    ] {
        let (status, refused) = gui.post_with("/api/hub/check", &gui.token, &body);
        assert_eq!(status, 400, "{why}: {refused}");
        assert!(
            refused["error"].as_str().is_some_and(|error| !error.is_empty()),
            "{why} needs a reason: {refused}"
        );
    }
}

fn enc(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('/', "%2F")
        .replace('|', "%7C")
        .replace('+', "%2B")
        .replace(' ', "%20")
}
