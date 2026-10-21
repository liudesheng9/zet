//! Tree Lock S01 (#124): one owner promise about one whole tree.
//!
//! These tests use the real token-guarded GUI routes, the real stdio MCP server and the real shell
//! command, on scratch archives (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`). They compare stored facts — the
//! lock tables, member fingerprints, seals and Card rows — not only the returned messages.

#![allow(dead_code)]

use assert_cmd::Command;
use rusqlite::Connection;
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

    fn db(&self) -> Connection {
        Connection::open(self.archive_root.join("zt.sqlite3")).expect("open database")
    }

    fn restart(&self) {
        self.cmd().arg("down").assert().success();
        self.cmd().arg("up").assert().success();
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

    /// A refused call: the structured payload and its error flag.
    fn refused(&mut self, name: &str, arguments: Value) -> Value {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(is_error, "{name}({arguments}) was not refused: {value}");
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

    fn panel(&self) -> Value {
        self.get("/api/locks")
    }

    fn active_lock(&self) -> Value {
        let panel = self.panel();
        let locks = panel["locks"].as_array().cloned().unwrap_or_default();
        locks
            .into_iter()
            .find(|lock| lock["unlocked_at"].is_null())
            .unwrap_or(Value::Null)
    }

    /// Plan a Lock and confirm it: the ordinary owner action.
    fn lock(&self, root: &str, note: Option<&str>) -> Value {
        let mut request = json!({ "root": root });
        if let Some(note) = note {
            request["note"] = json!(note);
        }
        let plan = self.post("/api/lock", &request);
        assert_eq!(plan["applied"], false, "the first request only plans: {plan}");
        let plan_value = &plan["plan"];
        let mut confirmed = request.clone();
        confirmed["confirm"] = json!("lock");
        confirmed["plan_hash"] = plan_value["plan_hash"].clone();
        let applied = self.post("/api/lock", &confirmed);
        assert_eq!(applied["applied"], true, "the confirmation commits: {applied}");
        applied["lock"].clone()
    }

    fn unlock(&self, lock_id: i64) -> Value {
        let plan = self.post("/api/unlock", &json!({ "lock_id": lock_id }));
        assert_eq!(plan["applied"], false, "unlock plans first: {plan}");
        let applied = self.post(
            "/api/unlock",
            &json!({
                "lock_id": lock_id,
                "confirm": "unlock",
                "plan_hash": plan["plan"]["plan_hash"],
            }),
        );
        assert_eq!(applied["applied"], true, "unlock applies: {applied}");
        applied["lock"].clone()
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

/// One Topic tree root.
fn topic(mcp: &mut Mcp, title: &str) -> String {
    address(mcp.ok(
        "create_topic",
        json!({ "title": title, "body": format!("{title} body") }),
    ))
}

/// A Direct successor of `at`.
fn card(mcp: &mut Mcp, at: &str, title: &str, body: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "title": title, "body": body }),
    ))
}

/// A Side successor of `at`.
fn side_card(mcp: &mut Mcp, at: &str, title: &str, body: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "kind": "side", "title": title, "body": body }),
    ))
}

fn first_address(root: &str) -> String {
    let tree = root.split_once('/').map(|(tree, _)| tree).unwrap_or(root);
    format!("{tree}/1")
}

#[test]
fn an_owner_lock_freezes_a_topic_tree_for_every_writer() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let side = side_card(&mut mcp, &first, "Side", &format!("continues [[{first}]]"));
    let other_root = topic(&mut mcp, "Outside");
    let pinned = card(&mut mcp, &other_root, "Pinned", "pinned body");
    let third_root = topic(&mut mcp, "Targets");
    card(&mut mcp, &third_root, "Target", "target body");
    mcp.ok(
        "edit_card",
        json!({ "at": side, "body": format!("links [[{first}]] and [[{pinned}]]") }),
    );

    // An empty panel, one token guard, and the plan before any write.
    assert_eq!(gui.panel()["locks"].as_array().map(Vec::len), Some(0));
    let (status, refused) = gui.get_with("/api/locks", "not-the-token");
    assert_eq!(status, 403, "the token guard stays in front of the route");
    assert!(refused["error"].as_str().unwrap().contains("token"));
    let plan = gui.post("/api/lock", &json!({ "root": root }));
    assert_eq!(plan["applied"], false);
    let plan_facts = &plan["plan"];
    assert_eq!(plan_facts["root"], root);
    assert_eq!(plan_facts["confirm"], "lock");
    assert_eq!(plan_facts["zone"], "topic");
    assert_eq!(plan_facts["member_count"], 3, "root and two regular Cards");
    assert_eq!(plan_facts["regular_count"], 2);
    assert_eq!(plan_facts["pin_count"], 1, "one distinct external target");
    assert_eq!(plan_facts["pins"][0]["target"], pinned);
    assert_eq!(plan_facts["pins"][0]["sources"][0], side);
    assert_eq!(plan_facts["seal"].as_str().unwrap().len(), 64);
    assert_eq!(plan_facts["seal_display"].as_str().unwrap().len(), 12);
    assert!(
        plan["lines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line.as_str().unwrap().contains(&pinned))
    );

    // A wrong confirmation word and a missing hash never commit.
    let (status, answer) = gui.post_with(
        "/api/lock",
        &gui.token,
        &json!({ "root": root, "confirm": "unlock", "plan_hash": plan_facts["plan_hash"] }),
    );
    assert_eq!(status, 400, "wrong word is invalid: {answer}");
    let (status, answer) = gui.post_with(
        "/api/lock",
        &gui.token,
        &json!({ "root": root, "confirm": "lock" }),
    );
    assert_eq!(status, 400, "a confirmed request needs its hash: {answer}");
    assert_eq!(gui.panel()["locks"].as_array().map(Vec::len), Some(0));

    // The real commit.
    let lock = gui.lock(&root, Some("reviewed"));
    assert_eq!(lock["root"], root);
    assert_eq!(lock["member_count"], 3);
    assert_eq!(lock["seal"], plan_facts["seal"]);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let panel = gui.panel();
    let active = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == lock_id)
        .expect("the panel shows the committed Lock");
    assert_eq!(active["note"], "reviewed");
    assert_eq!(active["members"].as_array().unwrap().len(), 3);
    assert_eq!(active["events"][0]["action"], "lock");
    assert_eq!(active["events"][0]["detail"]["seal"], lock["seal"]);
    assert_eq!(panel["integrity"]["ok"], true);

    // No writer path gets through, and the refusal names the Locked tree.
    let refusal = mcp.refused(
        "edit_card",
        json!({ "at": first, "body": "changed by the agent" }),
    );
    assert_eq!(refusal["code"], "locked", "{refusal}");
    assert_eq!(refusal["locked_tree"], root);
    let refusal = mcp.refused(
        "create_successor",
        json!({ "at": first, "title": "New", "body": "new body" }),
    );
    assert!(
        refusal["code"] == "locked_tree" || refusal["code"] == "locked",
        "{refusal}"
    );
    let refusal = mcp.refused(
        "insert_card",
        json!({ "before": first, "title": "New", "body": "new body" }),
    );
    assert!(
        refusal["code"] == "locked_tree" || refusal["code"] == "locked",
        "{refusal}"
    );
    let refusal = mcp.refused("delete_card", json!({ "at": first }));
    assert_eq!(refusal["code"], "locked", "{refusal}");
    let refusal = mcp.refused(
        "move_card",
        json!({ "at": first, "to": format!("{}/2", other_root.split_once('/').unwrap().0) }),
    );
    assert_eq!(refusal["code"], "locked", "{refusal}");

    // The owner's own route is bound too: no Owner exemption, and the refusal is structured.
    let (status, refused) = gui.post_with(
        "/api/edit",
        &gui.token,
        &json!({ "at": first, "text": "# First\n\nchanged\n" }),
    );
    assert_eq!(status, 409, "a Lock refusal is a structured 409: {refused}");
    assert_eq!(refused["refusal"]["code"], "locked");
    assert_eq!(refused["refusal"]["locked_tree"], root);

    // The pinned target keeps its body editable, but may not move or be deleted.
    mcp.ok(
        "edit_card",
        json!({ "at": pinned, "body": "allowed body edit" }),
    );
    let refusal = mcp.refused(
        "move_card",
        json!({ "at": pinned, "to": format!("{}/2", third_root.split_once('/').unwrap().0) }),
    );
    assert_eq!(refusal["code"], "pinned", "{refusal}");
    assert_eq!(refusal["pinned_by"][0]["tree"], root);
    assert_eq!(refusal["pinned_by"][0]["sources"][0], side);
    let refusal = mcp.refused("delete_card", json!({ "at": pinned }));
    assert_eq!(refusal["code"], "pinned", "{refusal}");

    // An incoming Link from an unlocked source is allowed and does not change the seal.
    let outside = side_card(&mut mcp, &pinned, "Outside", "outside body");
    mcp.ok(
        "edit_card",
        json!({ "at": outside, "body": format!("now links [[{first}]]") }),
    );
    let after = gui.active_lock();
    assert_eq!(after["seal"], lock["seal"], "a generated change keeps the seal");

    // The raw SQLite backstop. An external connection has no `zt_authored_fp` wrapper, so the
    // statement fails instead of changing the busy author's text (T11); either way it cannot write.
    let external = home.db();
    let error = external
        .execute(
            "UPDATE cards SET text = '# First\n\nraw\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
            [&first],
        )
        .expect_err("the trigger refuses the statement");
    let message = error.to_string();
    assert!(
        message.contains("locked tree") || message.contains("no such function: zt_authored_fp"),
        "the trigger is the backstop: {message}"
    );
    let stored: String = external
        .query_row("SELECT text FROM cards WHERE location = ?1", [&first], |row| {
            row.get(0)
        })
        .expect("stored text");
    assert!(!stored.contains("raw"), "no authored text changed");

    // Reading the stored facts: the seal and every member fingerprint are unchanged.
    let panel = gui.panel();
    let active = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == lock_id)
        .expect("same Lock");
    assert_eq!(active["seal"], lock["seal"]);
    assert_eq!(panel["integrity"]["ok"], true);
}

#[test]
fn every_public_writer_path_is_refused_before_it_writes() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    // The locked tree is created through the identified batch writer, so the member has a receipt.
    let created = mcp.ok(
        "apply_batch",
        json!({ "items": [
            { "op_key": "lock-test/1/1/create/topic", "client_ref": "root",
              "tool": "create_topic", "arguments": { "title": "Frozen" } },
            { "op_key": "lock-test/1/1/create/first", "client_ref": "first",
              "tool": "create_successor",
              "arguments": { "at": "@@root", "title": "First", "body": "first body" } },
        ] }),
    );
    let root = created["results"][0]["result"]["address"]
        .as_str()
        .expect("topic address")
        .to_string();
    let first = created["results"][1]["result"]["address"]
        .as_str()
        .expect("card address")
        .to_string();
    let other_root = topic(&mut mcp, "Outside");
    let target = card(&mut mcp, &other_root, "Target", "target body");
    gui.lock(&root, None);
    let revision = |home: &TestHome| -> i64 {
        home.db()
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM metadata WHERE key='archive_revision'",
                [],
                |row| row.get(0),
            )
            .expect("revision")
    };
    let before = revision(&home);
    let member_count = |home: &TestHome| -> i64 {
        home.db()
            .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
            .expect("cards")
    };
    let cards_before = member_count(&home);

    // Every authored metadata field, through the MCP writer.
    for change in [
        json!({ "at": first, "type": "event" }),
        json!({ "at": first, "type": "model", "level": "formal" }),
        json!({ "at": first, "type": "model", "facet": "setting", "emphasis": "core" }),
        json!({ "at": first, "type": "hub", "level": "line" }),
        json!({ "at": first, "type": "model", "form": "theorem" }),
        json!({ "at": first, "body": "changed body" }),
    ] {
        let refusal = mcp.refused("edit_card", change.clone());
        assert_eq!(refusal["code"], "locked", "{change}: {refusal}");
    }
    // An owned paragraph through the identified batch writer, with a receipt that still matches.
    let refusal = mcp.refused(
        "apply_batch",
        json!({ "items": [{
            "op_key": "lock-test/1/1/edit/paragraph",
            "client_ref": "member",
            "tool": "edit_card",
            "arguments": {
                "at": first,
                "base_receipt": "lock-test/1/1/create/first",
                "owned_paragraph": { "text": "an appended paragraph" },
            },
        }] }),
    );
    assert_eq!(refusal["code"], "locked", "{refusal}");
    // A historical receipt still replays its earlier result without a Card write.
    let replayed = mcp.ok(
        "apply_batch",
        json!({ "items": [
            { "op_key": "lock-test/1/1/create/topic", "client_ref": "root",
              "tool": "create_topic", "arguments": { "title": "Frozen" } },
            { "op_key": "lock-test/1/1/create/first", "client_ref": "first",
              "tool": "create_successor",
              "arguments": { "at": "@@root", "title": "First", "body": "first body" } },
        ] }),
    );
    assert_eq!(replayed["results"][0]["replayed"], true, "{replayed}");
    assert_eq!(replayed["results"][1]["replayed"], true, "{replayed}");
    // Creation, insertion, movement and deletion.
    let refusal = mcp.refused(
        "create_successor",
        json!({ "at": first, "title": "Follower", "body": "body" }),
    );
    assert!(refusal["code"] == "locked" || refusal["code"] == "locked_tree");
    let refusal = mcp.refused(
        "create_successor",
        json!({ "at": first, "kind": "side", "title": "Side", "body": "body" }),
    );
    assert!(refusal["code"] == "locked" || refusal["code"] == "locked_tree");
    let refusal = mcp.refused(
        "insert_card",
        json!({ "before": first, "title": "Inserted", "body": "body" }),
    );
    assert!(refusal["code"] == "locked" || refusal["code"] == "locked_tree");
    let refusal = mcp.refused("move_card", json!({ "at": first, "to": format!("{target}|a") }));
    assert_eq!(refusal["code"], "locked", "{refusal}");
    let refusal = mcp.refused("delete_card", json!({ "at": first }));
    assert_eq!(refusal["code"], "locked", "{refusal}");

    // The same paths through the GUI routes.
    for (path, body) in [
        ("/api/edit", json!({ "at": first, "text": "# First\n\nchanged\n" })),
        ("/api/create", json!({ "kind": "direct", "at": first, "text": "# New\n\nnew\n" })),
        ("/api/create", json!({ "kind": "side", "at": first, "text": "# New\n\nnew\n" })),
        ("/api/insert", json!({ "before": first, "text": "# New\n\nnew\n" })),
        ("/api/move", json!({ "at": first, "to": format!("{target}|a") })),
        ("/api/delete", json!({ "at": first })),
    ] {
        let (status, refused) = gui.post_with(path, &gui.token, &body);
        assert_eq!(status, 409, "{path} {body}: {refused}");
        assert!(
            refused["refusal"]["code"] == "locked" || refused["refusal"]["code"] == "locked_tree",
            "{path}: {refused}"
        );
    }

    // The shell commands refuse before an editor, a prompt or a write.
    let edit = home.cmd().args(["e", "--at", &first]).output().expect("zt e");
    assert!(!edit.status.success());
    assert!(
        String::from_utf8_lossy(&edit.stderr).contains("locked tree"),
        "no editor opened: {edit:?}"
    );
    let direct = home
        .cmd()
        .args(["n", "--at", &first])
        .output()
        .expect("zt n");
    assert!(!direct.status.success());
    assert!(String::from_utf8_lossy(&direct.stderr).contains("locked tree"));
    let side = home
        .cmd()
        .args(["b", "--at", &first])
        .output()
        .expect("zt b");
    assert!(!side.status.success());
    assert!(String::from_utf8_lossy(&side.stderr).contains("locked tree"));
    let insert = home
        .cmd()
        .args(["ins", "--at", &first])
        .output()
        .expect("zt ins");
    assert!(!insert.status.success());
    assert!(String::from_utf8_lossy(&insert.stderr).contains("locked tree"));
    let moved = home
        .cmd()
        .args(["mv", "--at", &first, &format!("{target}|a")])
        .output()
        .expect("zt mv");
    assert!(!moved.status.success());
    assert!(String::from_utf8_lossy(&moved.stderr).contains("locked tree"));
    let deleted = home
        .cmd()
        .args(["del", "--at", &first])
        .write_stdin("delete\n")
        .output()
        .expect("zt del");
    assert!(!deleted.status.success());
    assert!(String::from_utf8_lossy(&deleted.stderr).contains("locked tree"));

    // Nothing was written: same Card count, same revision, same fingerprints and seal.
    assert_eq!(member_count(&home), cards_before);
    assert_eq!(revision(&home), before, "a refused request writes nothing");
    let panel = gui.panel();
    let active = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lock| lock["unlocked_at"].is_null())
        .expect("the Lock");
    let fingerprints = active["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|member| {
            (
                member["address"].as_str().unwrap().to_string(),
                member["fingerprint"].as_str().unwrap().to_string(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(fingerprints.len(), 2);
    assert_eq!(panel["integrity"]["ok"], true);
    let (status, replan) = gui.post_with("/api/lock", &gui.token, &json!({ "root": root }));
    assert_eq!(status, 400, "a locked tree cannot be replanned: {replan}");
    assert!(replan["error"].as_str().unwrap().contains("already locked"));
}

#[test]
fn a_literature_tree_and_a_root_only_tree_anchor_locks() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let key = address(mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Smith2024, title={Book}}", "body": "该书。" }),
    ));
    let chapter = card(&mut mcp, &key, "Chapter", &format!("chapter of [[{key}]]"));

    let lock = gui.lock(&key, None);
    assert_eq!(lock["root"], key);
    assert_eq!(lock["workspace"], "agent");
    assert_eq!(lock["member_count"], 2);
    let refusal = mcp.refused(
        "edit_card",
        json!({ "at": chapter, "body": "changed chapter" }),
    );
    assert_eq!(refusal["code"], "locked", "{refusal}");
    let refusal = mcp.refused(
        "edit_literature_metadata",
        json!({ "at": key, "bibtex": "@book{Renamed2024, title={Book}}" }),
    );
    assert!(
        refusal["code"] == "locked" || refusal["error"].as_str().unwrap().contains("locked"),
        "{refusal}"
    );

    // A root-only Topic tree is a real tree and locks with one member.
    let empty = topic(&mut mcp, "EmptyTopic");
    let plan = gui.post("/api/lock", &json!({ "root": empty }));
    assert_eq!(plan["plan"]["member_count"], 1);
    let lock = gui.lock(&empty, None);
    assert_eq!(lock["member_count"], 1);
    let refusal = mcp.refused(
        "create_successor",
        json!({ "at": empty, "title": "First", "body": "body" }),
    );
    assert_eq!(refusal["code"], "locked_tree", "{refusal}");

    // A Regular Card is never an anchor, and a second Lock on one tree is refused.
    let (status, refused) = gui.post_with(
        "/api/lock",
        &gui.token,
        &json!({ "root": chapter }),
    );
    assert_eq!(status, 400);
    assert!(
        refused["error"].as_str().unwrap().contains("Regular Card"),
        "{refused}"
    );
    let (status, refused) = gui.post_with("/api/lock", &gui.token, &json!({ "root": key }));
    assert_eq!(status, 400);
    assert!(
        refused["error"].as_str().unwrap().contains("already locked"),
        "{refused}"
    );
    assert!(
        refused["error"].as_str().unwrap().contains("seal"),
        "the refusal names the existing seal: {refused}"
    );
}

#[test]
fn a_stale_confirmation_never_commits_and_a_lost_answer_is_read_back() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first");
    let plan = gui.post("/api/lock", &json!({ "root": root }));
    let hash = plan["plan"]["plan_hash"].as_str().unwrap().to_string();

    // The archive moves on while the owner reads the dialog: the revision changes, so does the plan.
    let revision_before = home
        .db()
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM metadata WHERE key='archive_revision'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("revision");
    card(&mut mcp, &first, "Later", "added after the plan");
    let (status, stale) = gui.post_with(
        "/api/lock",
        &gui.token,
        &json!({ "root": root, "confirm": "lock", "plan_hash": hash }),
    );
    assert_eq!(status, 200, "{stale}");
    assert_eq!(stale["applied"], false);
    assert_eq!(stale["plan_changed"], true, "{stale}");
    assert_ne!(stale["plan"]["plan_hash"], hash);
    assert_eq!(stale["plan"]["member_count"], 3, "the fresh plan sees the new Card");
    assert_eq!(gui.panel()["locks"].as_array().map(Vec::len), Some(0));

    // The fresh plan commits, and a repeated request resolves by reading instead of locking twice.
    let confirmed = json!({
        "root": root,
        "confirm": "lock",
        "plan_hash": stale["plan"]["plan_hash"],
    });
    let applied = gui.post("/api/lock", &confirmed);
    assert_eq!(applied["applied"], true, "{applied}");
    let lock_id = applied["lock"]["lock_id"].as_i64().expect("lock id");
    let (status, repeated) = gui.post_with("/api/lock", &gui.token, &confirmed);
    assert_eq!(status, 400, "a lost answer is resolved by reading: {repeated}");
    assert!(
        repeated["error"].as_str().unwrap().contains("already locked"),
        "{repeated}"
    );
    assert_eq!(
        repeated["error"]
            .as_str()
            .unwrap()
            .contains(&format!("lock {lock_id}")),
        true,
        "{repeated}"
    );
    let locks: i64 = home
        .db()
        .query_row("SELECT COUNT(*) FROM locks", [], |row| row.get(0))
        .expect("lock rows");
    assert_eq!(locks, 1, "never a second active Lock");
    assert!(
        home.db()
            .query_row("SELECT CAST(value AS INTEGER) FROM metadata WHERE key='archive_revision'", [], |row| row.get::<_, i64>(0))
            .expect("revision")
            > revision_before
    );
}

#[test]
fn a_lock_note_adds_one_history_event_and_never_reseals() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    card(&mut mcp, &root, "First", "first");
    let lock = gui.lock(&root, Some("first note"));
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let seal = lock["seal"].clone();

    let plan = gui.post(
        "/api/lock/note",
        &json!({ "lock_id": lock_id, "note": "second note" }),
    );
    assert_eq!(plan["applied"], false);
    assert_eq!(plan["plan"]["previous_note"], "first note");
    assert_eq!(plan["plan"]["note"], "second note");
    let applied = gui.post(
        "/api/lock/note",
        &json!({
            "lock_id": lock_id,
            "note": "second note",
            "confirm": "note",
            "plan_hash": plan["plan"]["plan_hash"],
        }),
    );
    assert_eq!(applied["applied"], true, "{applied}");
    assert_eq!(applied["lock"]["note"], "second note");
    assert_eq!(applied["lock"]["seal"], seal, "a note never reseals the tree");

    // The same note again writes no second event.
    let same = gui.post(
        "/api/lock/note",
        &json!({ "lock_id": lock_id, "note": "second note" }),
    );
    let applied = gui.post(
        "/api/lock/note",
        &json!({
            "lock_id": lock_id,
            "note": "second note",
            "confirm": "note",
            "plan_hash": same["plan"]["plan_hash"],
        }),
    );
    assert_eq!(applied["applied"], true);
    let panel = gui.panel();
    let active = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == lock_id)
        .expect("the Lock");
    let actions = active["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["action"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actions, ["lock", "note"], "{actions:?}");

    // A stale note plan does not commit.
    let (status, stale) = gui.post_with(
        "/api/lock/note",
        &gui.token,
        &json!({
            "lock_id": lock_id,
            "note": "third note",
            "confirm": "note",
            "plan_hash": plan["plan"]["plan_hash"],
        }),
    );
    assert_eq!(status, 200);
    assert_eq!(stale["plan_changed"], true, "{stale}");
    assert_eq!(gui.active_lock()["note"], "second note");
}

#[test]
fn unlock_releases_exactly_one_lock_and_keeps_its_history() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first");
    let lock = gui.lock(&root, Some("keep this"));
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let seal = lock["seal"].clone();

    let plan = gui.post("/api/unlock", &json!({ "lock_id": lock_id }));
    assert_eq!(plan["applied"], false);
    assert_eq!(plan["plan"]["member_count"], 2);
    assert!(
        plan["lines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line.as_str().unwrap().contains("history stays"))
    );
    let (status, stale) = gui.post_with(
        "/api/unlock",
        &gui.token,
        &json!({ "lock_id": lock_id, "confirm": "unlock", "plan_hash": "0".repeat(64) }),
    );
    assert_eq!(status, 200, "{stale}");
    assert_eq!(stale["plan_changed"], true);
    assert!(gui.active_lock()["lock_id"].is_number(), "still locked");

    let ended = gui.unlock(lock_id);
    assert!(ended["unlocked_at"].is_string());
    assert_eq!(ended["seal"], seal);
    assert_eq!(ended["note"], "keep this");
    let members: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM lock_members WHERE lock_id = ?1",
            [lock_id],
            |row| row.get(0),
        )
        .expect("member rows");
    assert_eq!(members, 0, "the live snapshot is removed");
    let pins: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM lock_pins WHERE lock_id = ?1",
            [lock_id],
            |row| row.get(0),
        )
        .expect("pin rows");
    assert_eq!(pins, 0);
    let panel = gui.panel();
    let history = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == lock_id)
        .expect("the ended Lock keeps its history");
    let actions = history["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["action"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actions, ["lock", "unlock"]);
    assert_eq!(history["events"][1]["detail"]["member_count"], 2);

    // Writes are allowed again, and a new Lock is a new id about the content then present.
    mcp.ok("edit_card", json!({ "at": first, "body": "allowed again" }));
    let relock = gui.lock(&root, None);
    assert_ne!(relock["lock_id"].as_i64(), Some(lock_id));
    assert_ne!(
        relock["seal"], seal,
        "the edited member makes a new promise with a new seal"
    );
    assert_eq!(relock["member_count"], 2);
}

#[test]
fn the_all_archive_clear_refuses_while_a_tree_is_locked() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    card(&mut mcp, &root, "First", "first");
    let lock = gui.lock(&root, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");

    let refused = home
        .cmd()
        .arg("clear")
        .write_stdin("clear\n")
        .output()
        .expect("run clear");
    assert!(!refused.status.success(), "clear refuses: {refused:?}");
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("locked tree"), "{stderr}");
    let cards: i64 = home
        .db()
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
        .expect("cards");
    assert_eq!(cards, 2, "nothing was deleted");

    gui.unlock(lock_id);
    let cleared = home
        .cmd()
        .arg("clear")
        .write_stdin("clear\n")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&cleared.get_output().stdout).to_string();
    assert!(stdout.contains("cleared 2 cards"), "{stdout}");
}

#[test]
fn a_lock_survives_a_service_restart_and_installs_once() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first");
    let lock = gui.lock(&root, Some("survives"));
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let seal = lock["seal"].clone();

    home.restart();

    let gui = home.gui();
    let panel = gui.panel();
    let active = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == lock_id)
        .expect("the Lock survived the restart");
    assert_eq!(active["seal"], seal);
    assert_eq!(panel["integrity"]["ok"], true);
    let mut mcp = home.mcp();
    let refusal = mcp.refused(
        "edit_card",
        json!({ "at": first, "body": "after restart" }),
    );
    assert_eq!(refusal["code"], "locked", "{refusal}");
    let locks: i64 = home
        .db()
        .query_row("SELECT COUNT(*) FROM locks", [], |row| row.get(0))
        .expect("lock rows");
    assert_eq!(locks, 1);
    let version: String = home
        .db()
        .query_row(
            "SELECT value FROM metadata WHERE key = 'tree_lock_schema_version'",
            [],
            |row| row.get(0),
        )
        .expect("schema version");
    assert_eq!(version, "1");
    let triggers: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name IN
             ('lock_no_delete','lock_no_change','lock_no_insert','lock_no_move_in',
              'lock_pin_no_delete','lock_pin_no_move')",
            [],
            |row| row.get(0),
        )
        .expect("triggers");
    assert_eq!(triggers, 6);
    gui.unlock(lock_id);
}

#[test]
fn the_panel_records_a_new_integrity_fault_once() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first");
    let lock = gui.lock(&root, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");

    // Tampering outside the product: drop the statement trigger, then change authored text.
    let raw = home.db();
    raw.execute_batch("DROP TRIGGER lock_no_change")
        .expect("drop trigger");
    raw.execute(
        "UPDATE cards SET text = '# First\n\ntampered\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
        [&first],
    )
    .expect("tamper");

    let panel = gui.panel();
    assert_eq!(panel["integrity"]["ok"], false);
    let faults = panel["integrity"]["faults"].as_array().unwrap();
    let kinds = faults
        .iter()
        .map(|fault| fault["kind"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&"fingerprint-changed"), "{kinds:?}");
    assert!(kinds.contains(&"trigger-missing"), "{kinds:?}");
    let first_events = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == lock_id)
        .unwrap()["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["action"] == "integrity-fault")
        .count();
    assert_eq!(first_events, 1, "a new fault is recorded once");

    // A second scan of the same faults adds no event, and the seal is not silently repaired.
    let panel = gui.panel();
    let again = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == lock_id)
        .unwrap()["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["action"] == "integrity-fault")
        .count();
    assert_eq!(again, 1);
    assert_eq!(panel["locks"][0]["seal"], lock["seal"]);

    // The owner can still release a faulted Lock from its retained record. The missing trigger is
    // still reported, so the archive is not called healthy; the next service start reinstalls it.
    let ended = gui.unlock(lock_id);
    assert!(ended["unlocked_at"].is_string());
    assert!(gui.active_lock().is_null());
    let panel = gui.panel();
    assert_eq!(panel["integrity"]["ok"], false, "{panel}");
    let kinds = panel["integrity"]["faults"]
        .as_array()
        .unwrap()
        .iter()
        .map(|fault| fault["kind"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(kinds, ["trigger-missing"], "{kinds:?}");
    home.restart();
    let gui = home.gui();
    assert_eq!(gui.panel()["integrity"]["ok"], true);
}
