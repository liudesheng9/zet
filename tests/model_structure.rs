//! Model structure: a Model Card and the Model parts filed under it, one per
//! facet, read and written over MCP and the GUI; and the GUI's rendering of
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
        cmd.env("ZT_CONFIG_DIR", &self.config_dir);
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
    let facets = json!([
        "setting",
        "assumptions",
        "timing",
        "dynamics",
        "solution",
        "results",
        "intuition",
        "limits",
        ""
    ]);
    for name in ["create_successor", "edit_card"] {
        assert_eq!(
            tool(name)["inputSchema"]["properties"]["facet"]["enum"],
            facets
        );
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
    mcp.ok("create_topic", json!({ "title": "Elsewhere" }));
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "1/0", "title": "Stray part", "facet": "results" })
        )
        .contains("must be filed under its Model Card")
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
            json!({ "at": "0/1", "kind": "side", "title": "x", "facet": "proof" })
        )
        .contains("unknown model facet `proof`")
    );

    let card = mcp.ok("get_card", json!({ "at": "0/1|a|1" }));
    assert_eq!(card["facet"], "timing");
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
            (json!("0/1|a|1"), json!("timing"))
        ]
    );
    assert!(parts[1]["body"].as_str().unwrap().contains("```chart"));
    assert_eq!(parts[1]["links"], json!(["0/1", "0/1|a"]));
    assert_eq!(model["issues"].as_array().unwrap().len(), 2);
    assert_eq!(model["facets"].as_array().unwrap().len(), 8);
    assert!(
        mcp.err("get_model", json!({ "at": "0/0" }))
            .contains("is not a Model Card or Model part")
    );

    // The Model Card lists its parts; then the model is consistent.
    mcp.ok(
        "edit_card",
        json!({ "at": "0/1", "body": "Question.\n\n- [[0/1|a]] setting\n- [[0/1|a|1]] timing" }),
    );
    assert_eq!(
        mcp.ok("get_model", json!({ "at": "0/1" }))["issues"],
        json!([])
    );
    // A facet alone retypes; an empty facet makes the part a Model Card of its own.
    mcp.ok("edit_card", json!({ "at": "0/1|a|1", "facet": "dynamics" }));
    assert_eq!(home.facet("0/1|a|1").as_deref(), Some("dynamics"));
    assert!(
        mcp.err("edit_card", json!({ "at": "0/1", "type": "idea" }))
            .contains("is the Model Card of 2 Model part(s)")
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
            .args(["gui", "--no-open", "--port", "0"])
            .env("ZT_CONFIG_DIR", &home.config_dir)
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
    // The part links its Model Card, which does not yet list the part.
    assert_eq!(
        card["model"]["issues"],
        json!(["the Model Card `0/1` does not link its part `0/1|a`"])
    );
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

    gui.api("POST", "/api/quit", json!({}));
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
        json!({ "at": "0/1", "title": "Setting", "facet": "setting" }),
    );
    drop(mcp);
    let output = home
        .cmd()
        .write_stdin("go 0/2\nty model results\ngo 0/2\nq\n")
        .output()
        .expect("run session");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("location: 0/2 [model · setting]"),
        "{stdout}"
    );
    assert!(
        stdout.contains("location: 0/2 [model · results]"),
        "{stdout}"
    );
    assert_eq!(home.facet("0/2").as_deref(), Some("results"));
}
