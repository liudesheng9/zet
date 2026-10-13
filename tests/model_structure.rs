//! Model structure: a Model Card and the Model parts that belong to it, by
//! filing or by Link, read and written over MCP and the GUI; and the GUI's rendering of
//! TeX math and HTML charts in Card bodies.

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
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

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_builds_a_model_from_its_model_card_and_facet_parts() {
    let home = TestHome::new();
    let mut mcp = home.mcp();

    let tools = mcp.request("tools/list", json!({}));
    let tool = |name: &str| {
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("tool {name}"))
    };
    // A facet is any short label; an emphasis is core, aside, or none.
    for name in ["create_successor", "edit_card"] {
        let properties = &tool(name)["inputSchema"]["properties"];
        assert_eq!(properties["facet"]["enum"], Value::Null);
        assert_eq!(properties["facet"]["maxLength"], 32);
        assert_eq!(properties["emphasis"]["enum"], json!(["core", "aside", ""]));
    }
    assert_eq!(tool("get_model")["inputSchema"]["required"], json!(["at"]));

    mcp.ok("create_topic", json!({ "title": "Firms" }));
    let head = mcp.ok(
        "create_successor",
        json!({ "at": "0/0", "title": "GHM model: ownership shapes investment", "body": "Question.", "type": "model" }),
    );
    assert_eq!(head["address"], "0/1");
    // A facet makes a Model part, filed under the Model Card or one of its parts.
    let setting = mcp.ok(
        "create_successor",
        json!({ "at": "0/1", "kind": "side", "title": "Setting: two managers, two assets", "body": "Part of [[0/1]].", "facet": "setting" }),
    );
    assert_eq!(
        (&setting["address"], &setting["type"], &setting["facet"]),
        (&json!("0/1|a"), &json!("model"), &json!("setting"))
    );
    mcp.ok(
        "create_successor",
        json!({ "at": "0/1|a", "title": "Timing: invest, then bargain", "body": "Part of [[0/1]]; uses [[0/1|a]].\n\n```chart\n<svg viewBox=\"0 0 10 10\"></svg>\n```", "facet": "timing" }),
    );
    assert_eq!(home.facet("0/1|a|1").as_deref(), Some("timing"));
    // A part may be filed anywhere; it belongs to no model until a Model
    // Card links it or it is filed under one.
    mcp.ok("create_topic", json!({ "title": "Elsewhere" }));
    let stray = mcp.ok(
        "create_successor",
        json!({ "at": "1/0", "title": "Results: underinvestment", "body": "Rests on [[0/1|a|1]].", "facet": "results" }),
    );
    assert_eq!(stray["address"], "1/1");
    assert!(
        mcp.ok("get_model", json!({ "at": "1/1" }))["issues"][0]
            .as_str()
            .unwrap()
            .contains("belongs to no model")
    );
    assert_eq!(
        mcp.err(
            "create_successor",
            json!({ "at": "0/1", "kind": "side", "title": "x", "type": "event", "facet": "results" })
        ),
        "only Model Cards can have a Model facet"
    );
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "0/1", "kind": "side", "title": "x", "facet": "a facet that is far too long to be a label" })
        )
        .contains("at most 32 characters")
    );
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "0/1", "kind": "side", "title": "x", "facet": "proof", "emphasis": "loud" })
        )
        .contains("unknown emphasis `loud`")
    );
    // The author chooses the facets, so a part may present anything the model needs.
    let proof = mcp.ok(
        "create_successor",
        json!({ "at": "0/1", "kind": "side", "title": "Proof of Proposition 1", "body": "Part of [[0/1]].", "facet": "Proof", "emphasis": "aside" }),
    );
    assert_eq!(
        (&proof["address"], &proof["facet"], &proof["emphasis"]),
        (&json!("0/1|b"), &json!("Proof"), &json!("aside"))
    );

    let card = mcp.ok("get_card", json!({ "at": "0/1|a|1" }));
    assert_eq!(card["facet"], "timing");
    assert_eq!(card["emphasis"], Value::Null);
    assert_eq!(card["model"]["head"]["address"], "0/1");
    let model = mcp.ok("get_model", json!({ "at": "0/1|a" }));
    assert_eq!(model["head"]["body"], "Question.");
    let parts = model["parts"].as_array().unwrap();
    assert_eq!(
        parts
            .iter()
            .map(|part| (part["address"].clone(), part["facet"].clone()))
            .collect::<Vec<_>>(),
        [
            (json!("0/1|a"), json!("setting")),
            (json!("0/1|a|1"), json!("timing")),
            (json!("0/1|b"), json!("Proof"))
        ]
    );
    assert!(parts[1]["body"].as_str().unwrap().contains("```chart"));
    assert_eq!(parts[1]["links"], json!(["0/1", "0/1|a"]));
    // Each relation says what it is, by the words before its Link.
    assert_eq!(
        model["relations"],
        json!([{ "from": "0/1|a|1", "to": "0/1|a", "kind": "rests-on", "note": "uses 「setting」." }])
    );
    assert_eq!(model["loose"], json!(["0/1|b"]));
    // Neither the Model Card nor its parts must link each other.
    assert_eq!(model["issues"], json!([]));
    assert_eq!(model["common_facets"], Value::Null);
    assert!(
        mcp.err("get_model", json!({ "at": "0/0" }))
            .contains("is not a Model Card or Model part")
    );

    // The Model Card names its parts in its own order, and its Link takes in
    // the part filed elsewhere.
    mcp.ok(
        "edit_card",
        json!({ "at": "0/1", "body": "Question.\n\n- [[0/1|a]] setting\n- [[0/1|a|1]] timing\n- [[1/1]] results\n- [[0/1|b]] proof" }),
    );
    let model = mcp.ok("get_model", json!({ "at": "1/1" }));
    assert_eq!(model["head"]["address"], "0/1");
    assert_eq!(
        model["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|part| (part["address"].clone(), part["filed"].clone()))
            .collect::<Vec<_>>(),
        [
            (json!("0/1|a"), json!(true)),
            (json!("0/1|a|1"), json!(true)),
            (json!("1/1"), json!(false)),
            (json!("0/1|b"), json!(true))
        ]
    );
    assert_eq!(
        model["relations"][1],
        json!({ "from": "1/1", "to": "0/1|a|1", "kind": "rests-on", "note": "Rests on 「timing」." })
    );
    // A second Model Card that links a part shares it; Cards outside the
    // model that link it, or that it links, are its neighbours, with kinds.
    let other = mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "kind": "side", "title": "Hold-up model", "body": "Shares [[0/1|a]].", "type": "model" }),
    );
    let critique = mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "title": "Contracts can fix it", "body": "This relaxes [[0/1|a]]: write the contract first." }),
    );
    let model = mcp.ok("get_model", json!({ "at": "0/1|a" }));
    assert_eq!(model["models"][0]["address"], "0/1");
    assert_eq!(model["models"][1]["address"], other["address"]);
    assert_eq!(model["parts"][0]["also"][0]["address"], other["address"]);
    let neighbours = model["neighbours"].as_array().unwrap();
    let relaxing = neighbours
        .iter()
        .find(|n| n["from"] == critique["address"])
        .expect("the critique is a neighbour");
    assert_eq!(
        (&relaxing["way"], &relaxing["to"], &relaxing["kind"]),
        (&json!("in"), &json!("0/1|a"), &json!("relaxes"))
    );
    assert_eq!(relaxing["card"]["title"], "Contracts can fix it");
    assert!(neighbours.iter().all(|n| n["card"]["address"] != "1/0"));
    mcp.ok("edit_card", json!({ "at": "0/1", "body": "Question.\n\n- [[0/1|a]] setting\n- [[0/1|a|1]] timing\n- [[0/1|b]] proof" }));
    mcp.ok("edit_card", json!({ "at": "1/1", "type": "idea" }));
    // A Model Card that links a part cannot stop being a Model Card.
    let other = other["address"].as_str().unwrap();
    assert!(
        mcp.err("edit_card", json!({ "at": other, "type": "idea" }))
            .contains("is the Model Card of 1 Model part(s) (0/1|a)")
    );
    mcp.ok("edit_card", json!({ "at": other, "body": "No parts." }));
    mcp.ok("edit_card", json!({ "at": other, "type": "idea" }));
    // A facet alone retypes; an emphasis alone stresses a part; an empty
    // facet makes the part a Model Card of its own.
    mcp.ok("edit_card", json!({ "at": "0/1|a|1", "facet": "dynamics" }));
    assert_eq!(home.facet("0/1|a|1").as_deref(), Some("dynamics"));
    mcp.ok("edit_card", json!({ "at": "0/1|a|1", "emphasis": "core" }));
    assert_eq!(
        mcp.ok("get_model", json!({ "at": "0/1" }))["parts"][1]["emphasis"],
        "core"
    );
    assert!(
        mcp.err("edit_card", json!({ "at": "0/1", "emphasis": "core" }))
            .contains("only Model parts can have an emphasis")
    );
    assert!(
        mcp.err("edit_card", json!({ "at": "0/1", "type": "idea" }))
            .contains("is the Model Card of 3 Model part(s)")
    );
    mcp.ok("edit_card", json!({ "at": "0/1|a|1", "facet": "" }));
    assert_eq!(home.facet("0/1|a|1"), None);

    let found = mcp.ok("search_cards", json!({ "type": "model" }));
    assert_eq!(found["results"][1]["facet"], "setting");
    let tree = mcp.ok("get_tree", json!({ "tree": "0" }));
    assert_eq!(tree["cards"][2]["head"], "0/1");
}

struct Gui {
    child: Child,
    port: u16,
    token: String,
}

impl Gui {
    fn start(home: &TestHome) -> Self {
        let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
            .arg("gui")
            .env("ZT_CONFIG_DIR", &home.config_dir)
            .env("ZT_GUI_PORT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start zt gui");
        let mut stdout = BufReader::new(child.stdout.take().expect("gui stdout"));
        let mut first_line = String::new();
        stdout.read_line(&mut first_line).expect("gui url line");
        let port: u16 = first_line
            .trim()
            .strip_prefix("zt gui: http://127.0.0.1:")
            .and_then(|rest| rest.strip_suffix('/'))
            .expect("gui prints its URL")
            .parse()
            .expect("port");
        let mut gui = Self {
            child,
            port,
            token: String::new(),
        };
        let (_, _, page) = gui.raw("GET", "/", None);
        gui.token = page
            .split("name=\"zt-token\" content=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("token")
            .to_string();
        gui
    }

    /// Status code, headers, and body of one request.
    fn raw(&self, method: &str, path: &str, body: Option<&str>) -> (u16, String, String) {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        let body = body.unwrap_or_default();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            self.port,
            self.token,
            body.len()
        )
        .expect("send request");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        let response = String::from_utf8_lossy(&response).to_string();
        let (head, body) = response.split_once("\r\n\r\n").expect("header end");
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .expect("status");
        (status, head.to_ascii_lowercase(), body.to_string())
    }

    fn api(&self, method: &str, path: &str, body: Value) -> Value {
        let body = (method == "POST").then(|| body.to_string());
        let (_, _, text) = self.raw(method, path, body.as_deref());
        serde_json::from_str(&text).expect("JSON response")
    }
}

impl Drop for Gui {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn gui_renders_math_and_sandboxed_charts_and_edits_model_parts() {
    let home = TestHome::new();
    let gui = Gui::start(&home);

    // KaTeX is served from the binary, so math renders offline.
    let (status, head, script) = gui.raw("GET", "/vendor/katex/katex.min.js", None);
    assert_eq!(status, 200);
    assert!(head.contains("text/javascript") && script.contains("katex"));
    let (status, head, _) = gui.raw("GET", "/vendor/katex/fonts/KaTeX_Main-Regular.woff2", None);
    assert_eq!(status, 200);
    assert!(head.contains("font/woff2"), "{head}");
    assert_eq!(
        gui.raw("GET", "/vendor/katex/fonts/nope.woff2", None).0,
        404
    );
    assert_eq!(gui.raw("GET", "/chart/0123", None).0, 404);

    gui.api(
        "POST",
        "/api/create",
        json!({ "kind": "topic", "text": "# Firms\n" }),
    );
    let head = gui.api(
        "POST",
        "/api/create",
        json!({ "kind": "direct", "at": "0/0", "type": "model", "text": "# GHM $i<i^*$\n\nInvestment $R'(i)/2 = 1$.\n\n<div class=\"flow\">\n<p>[[0/0]]</p>\n\n<p>two $x^2$</p>\n</div>\n" }),
    );
    assert_eq!(head["address"], "0/1");
    let part = gui.api(
        "POST",
        "/api/create",
        json!({ "kind": "side", "at": "0/1", "facet": "assumptions", "text": "# Assumptions\n\nOf [[0/1]].\n" }),
    );
    assert_eq!(part["address"], "0/1|a");
    assert_eq!(home.facet("0/1|a").as_deref(), Some("assumptions"));

    let card = gui.api("GET", "/api/card?at=0%2F1", json!(null));
    let html = card["html"].as_str().unwrap();
    assert!(
        html.contains(r#"<span class="math math-inline">R&#39;(i)/2 = 1</span>"#),
        "{html}"
    );
    assert_eq!(html.matches("zt-chart-frame").count(), 1, "{html}");
    assert_eq!(card["model"]["parts"][0]["facet"], "assumptions");
    // The Model Card need not list a part filed under it.
    assert_eq!(card["model"]["issues"], json!([]));
    let id = html
        .split("data-chart=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("chart id");
    let (status, head, page) = gui.raw("GET", &format!("/chart/{id}?theme=dark"), None);
    assert_eq!(status, 200);
    assert!(page.contains("data-theme=\"dark\""), "{page}");
    // The page is self-contained: its frame script and KaTeX, fonts included, are inlined.
    assert!(page.contains("<p>two $x^2$</p>") && page.contains("postMessage"));
    assert!(page.contains("katex") && page.contains("url(data:font/woff2;base64,"));
    assert!(
        page.contains(r#""0/0":{"exists":true,"title":"Firms"}"#),
        "{page}"
    );
    assert!(
        head.contains("content-security-policy: sandbox allow-scripts; default-src 'none'"),
        "{head}"
    );
    assert!(head.contains("frame-ancestors http://127.0.0.1:"), "{head}");

    // The editor sends a facet with the type; `ty model` with a facet sets it alone.
    gui.api(
        "POST",
        "/api/edit",
        json!({ "at": "0/1|a", "type": "model", "facet": "limits" }),
    );
    assert_eq!(home.facet("0/1|a").as_deref(), Some("limits"));
    // The editor names any facet and an emphasis; the reader gets both back.
    gui.api(
        "POST",
        "/api/edit",
        json!({ "at": "0/1|a", "type": "model", "facet": "反事实", "emphasis": "core" }),
    );
    let part = gui.api("GET", "/api/card?at=0%2F1%7Ca", json!(null));
    assert_eq!(
        (
            &part["facet"],
            &part["emphasis"],
            &part["model"]["parts"][0]["emphasis"]
        ),
        (&json!("反事实"), &json!("core"), &json!("core"))
    );
    gui.api(
        "POST",
        "/api/edit",
        json!({ "at": "0/1|a", "type": "model", "facet": "limits", "emphasis": "" }),
    );
    let refused = gui.api("POST", "/api/edit", json!({ "at": "0/1", "type": "event" }));
    assert!(
        refused["error"]
            .as_str()
            .unwrap()
            .contains("is the Model Card of 1 Model part(s)"),
        "{refused}"
    );
    let nodes = gui.api("GET", "/api/graph", json!(null));
    let part_node = nodes["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["address"] == "0/1|a")
        .cloned()
        .unwrap();
    assert_eq!(
        (&part_node["facet"], &part_node["head"]),
        (&json!("limits"), &json!("0/1"))
    );
    let overview = gui.api("GET", "/api/overview", json!(null));
    assert_eq!(overview["trees"][0]["models"], 1);
}

#[test]
fn session_names_a_model_part_and_ty_sets_its_facet() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({ "title": "Firms" }));
    mcp.ok(
        "create_successor",
        json!({ "at": "0/0", "title": "GHM", "type": "model" }),
    );
    mcp.ok(
        "create_successor",
        json!({ "at": "0/1", "title": "Setting", "facet": "setting", "emphasis": "core" }),
    );
    drop(mcp);
    // A facet is the rest of the line; the part keeps its emphasis.
    let output = home
        .cmd()
        .write_stdin("go 0/2\nty model 均衡 与 比较静态\ngo 0/2\nq\n")
        .output()
        .expect("run session");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("location: 0/2 [model · setting · core]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("location: 0/2 [model · 均衡 与 比较静态 · core]"),
        "{stdout}"
    );
    assert_eq!(home.facet("0/2").as_deref(), Some("均衡 与 比较静态"));
}

#[test]
fn up_opens_the_facets_of_an_archive_written_by_zt_0_6() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = TestHome {
        config_dir: tmp.path().join("config"),
        archive_root: tmp.path().join("archive"),
        _tmp: tmp,
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
    fs::create_dir_all(&home.archive_root).expect("archive root");
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("0.6 db");
    conn.execute_batch(
        "CREATE TABLE cards (
            row_id INTEGER PRIMARY KEY,
            location TEXT UNIQUE,
            citation_key TEXT UNIQUE,
            is_topic INTEGER NOT NULL CHECK(is_topic IN (0, 1)),
            is_lit INTEGER NOT NULL CHECK(is_lit IN (0, 1)),
            bibtex TEXT,
            text TEXT NOT NULL,
            workspace TEXT NOT NULL DEFAULT 'human' CHECK(workspace IN ('human', 'agent')),
            card_type TEXT NOT NULL DEFAULT 'idea' CHECK(card_type IN ('idea', 'event', 'model')),
            model_facet TEXT CHECK(model_facet IS NULL OR model_facet IN (
                'setting', 'assumptions', 'timing', 'dynamics',
                'solution', 'results', 'intuition', 'limits')),
            CHECK(
                (is_lit = 0 AND location IS NOT NULL AND citation_key IS NULL AND bibtex IS NULL)
                OR
                (is_lit = 1 AND is_topic = 0 AND location IS NULL AND citation_key IS NOT NULL AND bibtex IS NOT NULL)
            )
         );
         CREATE UNIQUE INDEX cards_citation_key_nocase ON cards(citation_key COLLATE NOCASE)
             WHERE citation_key IS NOT NULL;
         CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO metadata VALUES('card_format', 'markdown');
         INSERT INTO metadata VALUES('next_topic_id', '1');
         INSERT INTO cards(row_id, location, is_topic, is_lit, text, workspace, card_type, model_facet)
           VALUES(7, '0/0', 1, 0, '# Firms\n\n<!-- zt:reverse-links -->\n', 'agent', 'idea', NULL);
         INSERT INTO cards(row_id, location, is_topic, is_lit, text, workspace, card_type, model_facet)
           VALUES(9, '0/1', 0, 0, '# GHM\n\n<!-- zt:reverse-links -->\n', 'agent', 'model', NULL);
         INSERT INTO cards(row_id, location, is_topic, is_lit, text, workspace, card_type, model_facet)
           VALUES(12, '0/1|a', 0, 0, '# Timing\n\n<!-- zt:reverse-links -->\n', 'agent', 'model', 'timing');
         INSERT INTO cards(row_id, citation_key, is_topic, is_lit, bibtex, text, workspace)
           VALUES(13, 'Old2020', 0, 1, '@book{Old2020, title={Old}}', '# Old\n\n<!-- zt:reverse-links -->\n', 'human');",
    )
    .expect("0.6 archive");
    drop(conn);

    home.cmd().arg("up").assert().success();
    let db = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    let rows = db
        .prepare("SELECT row_id, COALESCE(location, citation_key), model_facet, model_emphasis FROM cards ORDER BY row_id")
        .unwrap()
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(
        rows,
        [
            (7, "0/0".into(), None, None),
            (9, "0/1".into(), None, None),
            (12, "0/1|a".into(), Some("timing".into()), None),
            (13, "Old2020".into(), None, None),
        ],
        "every Card keeps its row id and facet"
    );
    // Any short label now fits, an emphasis needs a facet, and the Citation key
    // index survives.
    db.execute(
        "UPDATE cards SET model_facet = '议价', model_emphasis = 'core' WHERE location = '0/1|a'",
        [],
    )
    .unwrap();
    assert!(
        db.execute(
            "UPDATE cards SET model_emphasis = 'core' WHERE location = '0/1'",
            []
        )
        .is_err()
    );
    assert!(
        db.execute(
            "INSERT INTO cards(citation_key, is_topic, is_lit, bibtex, text) VALUES('OLD2020', 0, 1, '@x', 'x')",
            []
        )
        .is_err()
    );
    drop(db);
    let backups = fs::read_dir(&home.archive_root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("zt.sqlite3.pre-open-models-")
        })
        .count();
    assert_eq!(backups, 1, "the archive is backed up before the rebuild");
    let log = fs::read_to_string(home.archive_root.join("zt.log")).unwrap();
    assert_eq!(log.matches("model facets are free labels").count(), 1);
    home.cmd().arg("down").assert().success();
    home.cmd().arg("up").assert().success();
    let log = fs::read_to_string(home.archive_root.join("zt.log")).unwrap();
    assert_eq!(
        log.matches("model facets are free labels").count(),
        1,
        "the table is rebuilt once"
    );
}
