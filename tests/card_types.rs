//! Card types: Event, Finding, and Model Cards are Regular Cards whose type is stored,
//! shown, searchable, kept through moves and renames, and dumped; Topic and
//! Literature Cards stay Idea Cards.

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command as StdCommand, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[path = "support/host_tools.rs"]
mod host_tools;

const ROOT_REFUSAL: &str = "only Regular Cards can be Event, Finding, Model, or Hub Cards";

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
        Self {
            _tmp: tmp,
            config_dir,
            archive_root,
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    fn cmd_with_editor(&self, text: &str) -> Command {
        let script = self.config_dir.join("fake-editor.ps1");
        fs::create_dir_all(&self.config_dir).expect("config dir");
        fs::write(
            &script,
            "[IO.File]::WriteAllText($args[0], $env:ZT_EDITOR_TEXT)\n",
        )
        .expect("editor script");
        let mut cmd = self.cmd();
        cmd.env(
            "EDITOR",
            format!(
                "powershell -NoProfile -ExecutionPolicy Bypass -File {}",
                script.display()
            ),
        );
        cmd.env("ZT_EDITOR_TEXT", text);
        cmd
    }

    fn configure(&self) {
        self.cmd()
            .args([
                "config",
                "set",
                "archive_root",
                self.archive_root.to_str().unwrap(),
            ])
            .assert()
            .success();
    }

    fn configure_and_up(&self) {
        self.configure();
        self.cmd().arg("up").assert().success();
    }

    fn db(&self) -> Connection {
        Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db")
    }

    fn text(&self, address: &str) -> String {
        self.db()
            .query_row(
                "SELECT text FROM cards WHERE location = ?1 OR citation_key = ?1",
                [address],
                |row| row.get(0),
            )
            .expect("card text")
    }

    fn card_type(&self, address: &str) -> String {
        self.db()
            .query_row(
                "SELECT card_type FROM cards WHERE location = ?1 OR citation_key = ?1",
                [address],
                |row| row.get(0),
            )
            .expect("card type")
    }

    /// A human Topic `0/0` with one Card `0/1`, made with shell commands.
    fn human_topic(&self) {
        self.cmd_with_editor("# Human Topic\n\nmine\n")
            .args(["t", "Human Topic"])
            .assert()
            .success()
            .stdout("0/0\n");
        self.cmd_with_editor("# Human Card\n\nmy idea\n")
            .args(["n", "--at", "0/0"])
            .assert()
            .success()
            .stdout("0/1\n");
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
        let init = mcp.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "zt-tests", "version": "1" },
            }),
        );
        assert_eq!(init["result"]["serverInfo"]["name"], "zt");
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

    /// Call a tool; return its structured result and whether it failed.
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
        let _ = self.child.wait();
    }
}

/// Addresses and types of a list of Card results.
fn typed(results: &Value) -> Vec<(String, String)> {
    results
        .as_array()
        .expect("result list")
        .iter()
        .map(|card| {
            (
                card["address"].as_str().unwrap().to_string(),
                card["type"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(address, card_type)| (address.to_string(), card_type.to_string()))
        .collect()
}

#[test]
fn up_adds_card_types_to_an_existing_archive_with_every_card_an_idea_card() {
    let home = TestHome::new();
    home.configure();
    fs::create_dir_all(&home.archive_root).expect("archive root");
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("0.4 db");
    conn.execute_batch(
        "CREATE TABLE cards (
            row_id INTEGER PRIMARY KEY,
            location TEXT UNIQUE,
            citation_key TEXT UNIQUE,
            is_topic INTEGER NOT NULL CHECK(is_topic IN (0, 1)),
            is_lit INTEGER NOT NULL CHECK(is_lit IN (0, 1)),
            bibtex TEXT,
            text TEXT NOT NULL,
            workspace TEXT NOT NULL DEFAULT 'human' CHECK(workspace IN ('human', 'agent'))
         );
         CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO metadata VALUES('card_format', 'markdown');
         INSERT INTO metadata VALUES('next_topic_id', '1');
         INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace)
           VALUES('0/0', NULL, 1, 0, NULL, '# Old Topic\n\n<!-- zt:reverse-links -->\n', 'human');
         INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace)
           VALUES('0/1', NULL, 0, 0, NULL, '# Old Card\n\n<!-- zt:reverse-links -->\n', 'agent');
         INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace)
           VALUES(NULL, 'Old2020', 0, 1, '@book{Old2020, title={Old}}', '# Old\n\n<!-- zt:reverse-links -->\n', 'human');",
    )
    .expect("0.4 archive");
    drop(conn);

    home.cmd().arg("up").assert().success();
    for address in ["0/0", "0/1", "Old2020"] {
        assert_eq!(home.card_type(address), "idea");
    }
    assert!(
        fs::read_to_string(home.archive_root.join("zt.log"))
            .unwrap()
            .contains("added card types; existing cards are idea cards")
    );
    assert!(
        home.db()
            .execute(
                "UPDATE cards SET card_type = 'fact' WHERE location = '0/1'",
                []
            )
            .is_err(),
        "the schema accepts only the three Card types"
    );
    home.db()
        .execute(
            "UPDATE cards SET card_type = 'event' WHERE location = '0/1'",
            [],
        )
        .unwrap();
    home.cmd().arg("down").assert().success();
    home.cmd().arg("up").assert().success();
    assert_eq!(home.card_type("0/1"), "event", "reopening keeps the types");
    assert_eq!(
        fs::read_to_string(home.archive_root.join("zt.log"))
            .unwrap()
            .matches("added card types")
            .count(),
        1,
        "the column is added once"
    );
}

#[test]
fn mcp_creates_edits_reads_and_lists_cards_by_type() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    let mut mcp = home.mcp();

    let tools = mcp.request("tools/list", json!({}));
    let tool = |name: &str| {
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .cloned()
            .unwrap()
    };
    for name in ["create_successor", "edit_card", "search_cards"] {
        assert_eq!(
            tool(name)["inputSchema"]["properties"]["type"]["enum"],
            json!(["idea", "event", "finding", "model", "hub"])
        );
    }
    assert_eq!(tool("search_cards")["inputSchema"]["required"], json!([]));

    mcp.ok("create_topic", json!({ "title": "Firm boundaries" }));
    let model = mcp.ok(
        "create_successor",
        json!({ "at": "1/0", "title": "GHM property-rights model", "body": "Setup.", "type": "model" }),
    );
    assert_eq!(
        (model["address"].as_str(), model["type"].as_str()),
        (Some("1/1"), Some("model"))
    );
    let event = mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "kind": "side", "title": "1926 GM buys Fisher Body", "type": "event" }),
    );
    assert_eq!(event["address"], "1/1|a");
    let idea = mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "title": "Ownership shapes investment", "body": "See [[1/1]] and [[1/1|a]]." }),
    );
    assert_eq!(
        (idea["address"].as_str(), idea["type"].as_str()),
        (Some("1/2"), Some("idea"))
    );
    assert_eq!(home.card_type("1/1"), "model");
    assert_eq!(home.card_type("1/1|a"), "event");
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "1/2", "title": "x", "type": "fact" })
        )
        .contains("unknown card type `fact`; expected idea, event, finding, model, or hub")
    );

    // Reads report the type.
    assert_eq!(mcp.ok("get_card", json!({ "at": "1/1" }))["type"], "model");
    assert_eq!(mcp.ok("get_card", json!({ "at": "0/1" }))["type"], "idea");
    assert_eq!(
        typed(&mcp.ok("get_tree", json!({ "tree": "1" }))["cards"]),
        pairs(&[
            ("1/0", "idea"),
            ("1/1", "model"),
            ("1/1|a", "event"),
            ("1/2", "idea")
        ])
    );

    // With a type, search needs no query.
    assert_eq!(
        typed(&mcp.ok("search_cards", json!({ "type": "event" }))["results"]),
        pairs(&[("1/1|a", "event")])
    );
    assert_eq!(
        typed(&mcp.ok("search_cards", json!({ "type": "model", "query": "" }))["results"]),
        pairs(&[("1/1", "model")])
    );
    assert_eq!(
        typed(
            &mcp.ok(
                "search_cards",
                json!({ "type": "idea", "workspace": "human" })
            )["results"]
        ),
        pairs(&[("0/0", "idea"), ("0/1", "idea")])
    );
    assert_eq!(
        mcp.ok("search_cards", json!({ "type": "idea", "limit": 1 }))["results"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        typed(&mcp.ok("search_cards", json!({ "query": "fisher" }))["results"]),
        pairs(&[("1/1|a", "event")])
    );
    assert!(
        mcp.ok(
            "search_cards",
            json!({ "query": "fisher", "type": "model" })
        )["results"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        mcp.err("search_cards", json!({}))
            .contains("missing string argument `query`")
    );

    // edit_card changes the type alone or with the text.
    let before = home.text("1/2");
    mcp.ok("edit_card", json!({ "at": "1/2", "type": "event" }));
    assert_eq!(home.card_type("1/2"), "event");
    assert_eq!(home.text("1/2"), before, "a type alone leaves the text");
    mcp.ok(
        "edit_card",
        json!({ "at": "1/2", "title": "Ownership drives investment", "type": "idea" }),
    );
    assert_eq!(home.card_type("1/2"), "idea");
    assert!(
        home.text("1/2")
            .starts_with("# Ownership drives investment\n\nSee [[1/1]] and [[1/1|a]].\n")
    );
    assert!(
        mcp.err("edit_card", json!({ "at": "1/0", "type": "event" }))
            .contains(ROOT_REFUSAL)
    );
    assert!(
        mcp.err("edit_card", json!({ "at": "0/1", "type": "event" }))
            .contains("the human workspace is read-only for AI agents")
    );
    assert_eq!(home.card_type("0/1"), "idea");

    // Moves and Citation key renames keep the type.
    mcp.ok(
        "move_card",
        json!({ "at": "1/1|a", "to": "1/3", "confirm": "move" }),
    );
    assert_eq!(mcp.ok("get_card", json!({ "at": "1/3" }))["type"], "event");
    assert!(home.text("1/2").contains("[[1/3]]"));
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@article{Klein1978, title = {Vertical Integration}}" }),
    );
    assert!(
        mcp.err("edit_card", json!({ "at": "Klein1978", "type": "model" }))
            .contains(ROOT_REFUSAL)
    );
    mcp.ok(
        "create_successor",
        json!({ "at": "Klein1978", "title": "Fisher Body as told by Klein", "type": "event" }),
    );
    mcp.ok(
        "edit_literature_metadata",
        json!({
            "at": "Klein1978",
            "bibtex": "@article{KleinEtAl1978, title = {Vertical Integration}}",
            "confirm": "move",
        }),
    );
    assert_eq!(home.card_type("KleinEtAl1978/1"), "event");
    assert_eq!(home.card_type("KleinEtAl1978"), "idea");
}

#[test]
fn line_session_shows_and_sets_the_type_of_the_current_card() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    let output = home
        .cmd()
        .write_stdin("go 0/1\nty event\nty model\ngo 0/0\nty event\nhelp\nq\n".as_bytes())
        .output()
        .expect("line session");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("location: 0/1\n# Human Card"));
    assert!(stdout.contains("location: 0/1 [event]\n# Human Card"));
    assert!(stdout.contains("location: 0/1 [model]\n# Human Card"));
    assert!(stdout.contains(&format!("{ROOT_REFUSAL}\n")));
    assert!(stdout.contains("location: 0/0\n# Human Topic"));
    assert_eq!(home.card_type("0/1"), "model");
    assert_eq!(home.card_type("0/0"), "idea");
}

fn http_request(port: u16, method: &str, path: &str, token: &str, body: Option<&str>) -> Value {
    use std::io::Read;
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect to gui");
    let body = body.unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("send request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    let start = response.find("\r\n\r\n").expect("header end") + 4;
    serde_json::from_str(&response[start..]).expect("JSON response")
}

#[test]
fn gui_creates_and_edits_typed_cards_and_reports_types() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut gui = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .arg("gui")
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start zt gui");
    let mut stdout = BufReader::new(gui.stdout.take().expect("gui stdout"));
    let mut first_line = String::new();
    stdout.read_line(&mut first_line).expect("gui url line");
    let port: u16 = first_line
        .trim()
        .strip_prefix("zt gui: http://127.0.0.1:")
        .and_then(|rest| rest.strip_suffix('/'))
        .expect("gui prints its URL")
        .parse()
        .expect("port");
    let page = {
        use std::io::Read;
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut page = String::new();
        stream.read_to_string(&mut page).unwrap();
        page
    };
    let token = page
        .split("name=\"zt-token\" content=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("token")
        .to_string();
    let api = |method: &str, path: &str, body: Option<&str>| {
        http_request(port, method, path, &token, body)
    };

    api(
        "POST",
        "/api/create",
        Some(r##"{"kind":"topic","text":"# Firms\n"}"##),
    );
    assert!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"topic","type":"event","text":"# Nope\n"}"##)
        )["error"]
            .as_str()
            .unwrap()
            .contains(ROOT_REFUSAL)
    );
    assert_eq!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"direct","at":"0/0","type":"event","text":"# Fisher Body\n"}"##)
        )["address"],
        "0/1"
    );
    assert_eq!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"side","at":"0/1","text":"# Plain idea\n"}"##)
        )["address"],
        "0/1|a"
    );
    assert_eq!(home.card_type("0/1"), "event");
    assert_eq!(home.card_type("0/1|a"), "idea");

    assert_eq!(api("GET", "/api/card?at=0%2F1", None)["type"], "event");
    assert_eq!(
        typed(&api("GET", "/api/tree?id=0", None)["cards"]),
        pairs(&[("0/0", "idea"), ("0/1", "event"), ("0/1|a", "idea")])
    );
    assert_eq!(
        typed(&api("GET", "/api/search?q=fisher", None)["results"]),
        pairs(&[("0/1", "event")])
    );
    let overview = api("GET", "/api/overview", None);
    assert_eq!(
        (
            &overview["trees"][0]["events"],
            &overview["trees"][0]["models"]
        ),
        (&json!(1), &json!(0))
    );

    // The editor saves text and type together; `ty` sends the type alone.
    let text = home.text("0/1|a");
    let body = json!({ "at": "0/1|a", "text": text.replace("# Plain idea", "# A model"), "base": text, "type": "model" });
    assert_eq!(
        api("POST", "/api/edit", Some(&body.to_string()))["address"],
        "0/1|a"
    );
    assert_eq!(home.card_type("0/1|a"), "model");
    assert!(home.text("0/1|a").starts_with("# A model\n"));
    api("POST", "/api/edit", Some(r#"{"at":"0/1|a","type":"idea"}"#));
    assert_eq!(home.card_type("0/1|a"), "idea");
    assert!(
        api("POST", "/api/edit", Some(r#"{"at":"0/0","type":"model"}"#))["error"]
            .as_str()
            .unwrap()
            .contains(ROOT_REFUSAL)
    );

    assert!(gui.wait().expect("gui exits").success());
}

#[cfg(windows)]
fn zip_backend_available() -> bool {
    StdCommand::new(host_tools::host_tar())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(windows))]
fn zip_backend_available() -> bool {
    StdCommand::new("zip")
        .arg("-v")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[test]
fn dump_records_event_and_model_cards_after_the_workspaces() {
    if !zip_backend_available() {
        return;
    }
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    {
        let mut mcp = home.mcp();
        mcp.ok("create_topic", json!({ "title": "Agent Topic" }));
        mcp.ok(
            "create_successor",
            json!({ "at": "1/0", "title": "A model", "type": "model", "level": "formal" }),
        );
        mcp.ok(
            "create_successor",
            json!({ "at": "1/1", "kind": "side", "title": "An event", "type": "event" }),
        );
    }

    home.cmd().arg("dp").write_stdin("1\n").assert().success();
    let dump_dir = home.archive_root.join("dump");
    let archive_path = fs::read_dir(&dump_dir)
        .expect("dump directory")
        .map(|entry| entry.expect("dump entry").path())
        .find(|path| path.extension().is_some_and(|ext| ext == "zip"))
        .expect("zip dump");
    let folder = archive_path
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let mut archive = zip::ZipArchive::new(fs::File::open(&archive_path).unwrap()).unwrap();
    let mut manifest = String::new();
    std::io::Read::read_to_string(
        &mut archive
            .by_name(&format!("{folder}/archive.json"))
            .expect("archive.json"),
        &mut manifest,
    )
    .unwrap();
    assert_eq!(
        manifest,
        r#"{"next_topic_id":2,"workspaces":{"0/0":"human","0/1":"human","1/0":"agent","1/1":"agent","1/1|a":"agent"},"types":{"1/1":"model","1/1|a":"event"},"facets":{},"emphasis":{},"levels":{"1/1":"formal"},"forms":{}}"#
    );
}

#[test]
fn mcp_creates_lists_counts_and_retypes_finding_cards() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({ "title": "Missing markets" }));
    let finding = mcp.ok(
        "create_successor",
        json!({
            "at": "1/0",
            "title": "31 of 48 firms delayed investment for want of insurance",
            "body": "测量：投资时点。\n数据：48 家企业。\n方法：访谈。\n结果：31 家推迟投资。\n局限：自报原因。\n来源：[[1/0]]",
            "type": "finding",
        }),
    );
    assert_eq!(
        (finding["address"].as_str(), finding["type"].as_str()),
        (Some("1/1"), Some("finding"))
    );
    assert_eq!(home.card_type("1/1"), "finding");
    assert_eq!(
        mcp.ok("get_card", json!({ "at": "1/1" }))["type"],
        "finding"
    );
    mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "title": "Missing insurance lowers investment", "body": "印证于 [[1/1]] 的访谈结果。" }),
    );
    assert_eq!(
        typed(&mcp.ok("search_cards", json!({ "type": "finding" }))["results"]),
        pairs(&[("1/1", "finding")])
    );
    assert_eq!(
        typed(&mcp.ok("get_tree", json!({ "tree": "1" }))["cards"]),
        pairs(&[("1/0", "idea"), ("1/1", "finding"), ("1/2", "idea")])
    );
    let trees = mcp.ok("list_trees", json!({ "workspace": "agent" }));
    assert_eq!(
        (&trees["trees"][0]["findings"], &trees["trees"][0]["events"]),
        (&json!(1), &json!(0))
    );

    // edit_card and insert_card take the type too; a tree root is refused.
    mcp.ok("edit_card", json!({ "at": "1/2", "type": "finding" }));
    assert_eq!(home.card_type("1/2"), "finding");
    mcp.ok("edit_card", json!({ "at": "1/2", "type": "idea" }));
    assert_eq!(home.card_type("1/2"), "idea");
    let inserted = mcp.ok(
        "insert_card",
        json!({ "before": "1/2", "title": "12 of 48 firms cancelled investment", "type": "finding", "confirm": "insert" }),
    );
    assert_eq!(inserted["address"], "1/2");
    assert_eq!(home.card_type("1/2"), "finding");
    assert_eq!(home.card_type("1/3"), "idea", "the old 1/2 moves down");
    assert!(
        mcp.err("edit_card", json!({ "at": "1/0", "type": "finding" }))
            .contains(ROOT_REFUSAL)
    );
    assert!(
        mcp.err("edit_card", json!({ "at": "0/1", "type": "finding" }))
            .contains("the human workspace is read-only for AI agents")
    );
}

#[test]
fn up_rebuilds_an_archive_from_before_finding_cards_once() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    home.cmd().arg("down").assert().success();
    for _ in 0..150 {
        if !home.archive_root.join("zt.pid").exists() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }

    // The schema of zt 0.17: every column of today, but no `finding` type.
    let db = home.db();
    let definition: String = db
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let old = definition
        .replace("'event', 'finding', 'model'", "'event', 'model'")
        .replacen("CREATE TABLE cards", "CREATE TABLE cards_old", 1);
    assert!(!old.contains("'finding'"));
    db.execute_batch(&format!(
        "BEGIN;
         {old};
         INSERT INTO cards_old SELECT * FROM cards;
         DROP TABLE cards;
         ALTER TABLE cards_old RENAME TO cards;
         CREATE UNIQUE INDEX cards_citation_key_nocase ON cards(citation_key COLLATE NOCASE)
             WHERE citation_key IS NOT NULL;
         COMMIT;"
    ))
    .unwrap();
    assert!(
        db.execute(
            "UPDATE cards SET card_type = 'finding' WHERE location = '0/1'",
            []
        )
        .is_err(),
        "the old schema refuses Finding Cards"
    );
    drop(db);
    let before = home.text("0/1");

    home.cmd().arg("up").assert().success();
    assert_eq!(home.text("0/1"), before, "the rebuild keeps every Card");
    assert_eq!(home.card_type("0/1"), "idea");
    home.db()
        .execute(
            "UPDATE cards SET card_type = 'finding' WHERE location = '0/1'",
            [],
        )
        .expect("the rebuilt schema takes Finding Cards");
    let backups = fs::read_dir(&home.archive_root)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("zt.sqlite3.pre-finding-cards-")
        })
        .count();
    assert_eq!(backups, 1, "the archive is backed up before the rebuild");

    home.cmd().arg("down").assert().success();
    home.cmd().arg("up").assert().success();
    assert_eq!(home.card_type("0/1"), "finding", "reopening keeps the type");
    assert_eq!(
        fs::read_to_string(home.archive_root.join("zt.log"))
            .unwrap()
            .matches("added finding cards")
            .count(),
        1,
        "the table is rebuilt once"
    );
}
