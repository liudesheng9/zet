//! Tree Lock S01 (#124): one owner promise about one whole tree.
//!
//! These tests use the real token-guarded GUI routes, the real stdio MCP server, the real shell
//! command and the real terminal Session, on scratch archives (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`).
//! They compare stored facts — the lock tables, member fingerprints, seals and Card rows — not only
//! the returned messages.

#![allow(dead_code)]

mod support;

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

/// The tree id of one address: the text before the first `/`.
fn tree_of(address: &str) -> String {
    address.split_once('/').map(|(tree, _)| tree).unwrap_or(address).to_string()
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
    let stale_lines = stale["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(
        stale_lines.iter().any(|line| line.contains("third note")),
        "the renewed lines describe the renewed plan: {stale_lines:?}"
    );
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
    assert!(
        stale["lines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line.as_str().unwrap().contains("releases 2 cards")),
        "the renewed lines describe the renewed unlock plan: {stale}"
    );
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
    // Each fault carries the facts that explain it, so the panel can answer "expected vs found".
    let changed = faults
        .iter()
        .find(|fault| fault["kind"] == "fingerprint-changed")
        .expect("fingerprint fault");
    assert_eq!(changed["lock_id"], json!(lock_id));
    assert_eq!(changed["card"], json!(first));
    assert!(changed["expected"].as_str().is_some_and(|text| text.len() == 64));
    assert!(changed["actual"].as_str().is_some_and(|text| text.len() == 64));
    // The active Lock carries its own faults and a broken seal beside the seal it reviewed.
    let listed = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == lock_id)
        .expect("the active Lock");
    assert_eq!(listed["seal_intact"], false);
    assert_eq!(listed["seal"], lock["seal"], "the seal is never silently updated");
    assert!(listed["faults"].as_array().unwrap().len() >= 2, "{listed}");
    let first_events = listed["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["action"] == "integrity-fault")
        .count();
    assert_eq!(first_events, 2, "the member fault and the lost trigger, once each");

    // A second scan of the same faults adds no event, and the seal is not silently repaired.
    let panel = gui.panel();
    let listed = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == lock_id)
        .unwrap();
    let again = listed["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["action"] == "integrity-fault")
        .count();
    assert_eq!(again, 2);
    assert_eq!(panel["locks"][0]["seal"], lock["seal"]);
    assert_eq!(
        panel["integrity"]["latched"].as_array().unwrap().len(),
        2,
        "the recorded faults stay latched on the active Lock: {panel}"
    );
    assert!(panel["integrity"]["summary"]
        .as_str()
        .is_some_and(|line| line.starts_with("integrity faulted:")));

    // The owner can still release a faulted Lock from its retained record. The missing trigger is
    // still reported, so the archive is not called healthy; the next service start reinstalls it.
    let ended = gui.unlock(lock_id);
    assert!(ended["unlocked_at"].is_string());
    assert!(gui.active_lock().is_null());
    let panel = gui.panel();
    assert_eq!(panel["integrity"]["ok"], false, "{panel}");
    assert_eq!(
        panel["integrity"]["latched"],
        json!([]),
        "an ended Lock is history, not an active promise"
    );
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

// ---------------------------------------------------------------------------------------------
// S02 (#125): keep locked Cards readable and linkable while preserving external pins.
// ---------------------------------------------------------------------------------------------


/// Delete a Card through both steps of its confirmation.
fn delete(mcp: &mut Mcp, at: &str) {
    let plan = mcp.ok("delete_card", json!({ "at": at }));
    let word = plan["confirm"].as_str().expect("delete confirmation word").to_string();
    let done = mcp.ok("delete_card", json!({ "at": at, "confirm": word }));
    assert_eq!(done["applied"], true, "delete {at}: {done}");
}

/// Move a Card to `to` through both steps of its confirmation.
fn move_card(mcp: &mut Mcp, at: &str, to: &str) {
    let plan = mcp.ok("move_card", json!({ "at": at, "to": to }));
    let word = plan["confirm"].as_str().expect("move confirmation word").to_string();
    let done = mcp.ok("move_card", json!({ "at": at, "to": to, "confirm": word }));
    assert_eq!(done["applied"], true, "move {at}: {done}");
}
/// The member facts a Lock promised: address, row identity and authored fingerprint.
fn member_facts(panel: &Value) -> Vec<(String, i64, String)> {
    let mut facts = panel["locks"]
        .as_array()
        .expect("locks")
        .iter()
        .find(|lock| lock["unlocked_at"].is_null())
        .expect("one active Lock")["members"]
        .as_array()
        .expect("members")
        .iter()
        .map(|member| {
            (
                member["address"].as_str().expect("address").to_string(),
                member["row_id"].as_i64().expect("row id"),
                member["fingerprint"].as_str().expect("fingerprint").to_string(),
            )
        })
        .collect::<Vec<_>>();
    facts.sort();
    facts
}

/// T05 / AC03: a pinned target keeps its authored meaning editable; only its address and row
/// identity are frozen, so a Citation-key rename is the one edit that is refused.
#[test]
fn a_pinned_target_stays_editable_without_renaming_it() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let side = side_card(&mut mcp, &first, "Side", "keeps the reference");
    let outside = topic(&mut mcp, "Outside");
    let pinned = card(&mut mcp, &outside, "Pinned", "pinned body");
    let part = card(&mut mcp, &pinned, "Part", "part body");
    let book = address(mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Smith2024, title={Book}}", "body": "该书。" }),
    ));
    mcp.ok(
        "edit_card",
        json!({ "at": side, "body": format!("links [[{pinned}]] and [[{part}]] and [[{book}]]") }),
    );
    let lock = gui.lock(&root, None);
    let seal = lock["seal"].clone();
    let before = member_facts(&gui.panel());

    // Body, title and every authored type field of the pinned target stay editable.
    mcp.ok(
        "edit_card",
        json!({ "at": pinned, "body": "the meaning may change while the address does not" }),
    );
    mcp.ok("edit_card", json!({ "at": pinned, "title": "A better title" }));
    mcp.ok("edit_card", json!({ "at": pinned, "type": "event" }));
    mcp.ok(
        "edit_card",
        json!({ "at": part, "type": "model", "facet": "setting", "emphasis": "core" }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": pinned, "type": "model", "level": "formal" }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": pinned, "type": "model", "form": "theorem" }),
    );
    // BibTeX without a Citation-key rename is an authored edit of the pinned target.
    mcp.ok(
        "edit_literature_metadata",
        json!({ "at": book, "bibtex": "@book{Smith2024, title={Book, second edition}}" }),
    );
    let edited = mcp.ok("get_card", json!({ "at": book }));
    assert!(
        edited["bibtex"].as_str().unwrap_or_default().contains("second edition"),
        "{edited}"
    );

    // The one pinned edit that moves a Card: a Citation-key rename.
    let refusal = mcp.refused(
        "edit_literature_metadata",
        json!({ "at": book, "bibtex": "@book{Smith2025, title={Book}}" }),
    );
    assert_eq!(refusal["code"], "pinned", "{refusal}");
    assert_eq!(refusal["pinned_by"][0]["tree"], root);

    // The locked source's promise did not move: same seal, same member fingerprints.
    let panel = gui.panel();
    assert_eq!(member_facts(&panel), before);
    let active = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lock| lock["unlocked_at"].is_null())
        .expect("the Lock");
    assert_eq!(active["seal"], seal);
    assert_eq!(panel["integrity"]["ok"], true);
}

/// T07 / AC04: two active Locks can pin one target. Releasing one keeps the other protection;
/// the target is free only after the last pinning Lock ends.
#[test]
fn two_locks_pin_one_target_and_release_one_at_a_time() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let first_tree = topic(&mut mcp, "FirstPromise");
    let a = card(&mut mcp, &first_tree, "A", "first promise");
    let second_tree = topic(&mut mcp, "SecondPromise");
    let b = card(&mut mcp, &second_tree, "B", "second promise");
    let targets = topic(&mut mcp, "Targets");
    let shared = card(&mut mcp, &targets, "Shared", "one target, two promises");
    mcp.ok(
        "edit_card",
        json!({ "at": a, "body": format!("promises [[{shared}]]") }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": b, "body": format!("promises [[{shared}]]") }),
    );

    let lock_a = gui.lock(&first_tree, Some("first"));
    let lock_b = gui.lock(&second_tree, Some("second"));
    let id_a = lock_a["lock_id"].as_i64().expect("lock a");
    let id_b = lock_b["lock_id"].as_i64().expect("lock b");
    assert_ne!(id_a, id_b);
    let pins: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM lock_pins WHERE target_address = ?1",
            [&shared],
            |row| row.get(0),
        )
        .expect("pin rows");
    assert_eq!(pins, 2, "one pin row for each Lock");

    // Both promises protect the shared target.
    let refusal = mcp.refused(
        "move_card",
        json!({ "at": shared, "to": format!("{a}|a") }),
    );
    assert_eq!(refusal["code"], "pinned", "{refusal}");
    let trees = refusal["pinned_by"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pin| pin["lock_id"].as_i64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(trees, vec![id_a, id_b], "both Locks explain the refusal");

    // Releasing one Lock releases exactly its own pins.
    gui.unlock(id_a);
    let refusal = mcp.refused(
        "move_card",
        json!({ "at": shared, "to": format!("{a}|a") }),
    );
    assert_eq!(refusal["code"], "pinned", "{refusal}");
    let remaining = refusal["pinned_by"].as_array().unwrap();
    assert_eq!(remaining.len(), 1, "{refusal}");
    assert_eq!(remaining[0]["lock_id"].as_i64(), Some(id_b));
    assert_eq!(remaining[0]["sources"][0], b);
    let left: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM lock_pins WHERE target_address = ?1",
            [&shared],
            |row| row.get(0),
        )
        .expect("pin rows");
    assert_eq!(left, 1, "the ended Lock removed only its own pin");

    // The last release frees the target again, and the derived index still matches every body.
    gui.unlock(id_b);
    move_card(&mut mcp, &shared, &format!("{a}|a"));
    let checked = home
        .cmd()
        .args(["status", "--check-links"])
        .output()
        .expect("zt status --check-links");
    let report = String::from_utf8_lossy(&checked.stdout);
    assert!(checked.status.success(), "{report}");
    assert!(report.contains("\"ok\":true"), "{report}");
    assert_eq!(gui.panel()["integrity"]["ok"], true);
}

/// T08 / AC02: an unlocked source may add or remove an incoming Link, and a Reply may answer a
/// locked agent Card. Only the generated Reverse section moves; every authored fingerprint and the
/// seal stay, and the derived Link index still matches the bodies.
#[test]
fn an_incoming_link_and_a_reply_keep_a_locked_tree_untouched() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let lock = gui.lock(&root, None);
    let seal = lock["seal"].clone();
    let before = member_facts(&gui.panel());
    let member = gui.get(&format!("/api/card?at={first}"));
    let member_fingerprint = member["authored_fingerprint"]
        .as_str()
        .expect("member fingerprint")
        .to_string();

    // An unlocked source adds an incoming Link.
    let outside = topic(&mut mcp, "Outside");
    let source = card(&mut mcp, &outside, "Source", "no link yet");
    mcp.ok(
        "edit_card",
        json!({ "at": source, "body": format!("now cites [[{first}]]") }),
    );
    let cited = gui.get(&format!("/api/card?at={first}"));
    assert_eq!(
        cited["authored_fingerprint"].as_str(),
        Some(member_fingerprint.as_str()),
        "an incoming Link changes no authored identity"
    );
    assert!(
        cited["text"]
            .as_str()
            .expect("stored text")
            .contains(&source),
        "the generated Reverse section names the unlocked source: {}",
        cited["text"]
    );
    let indexed: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM links WHERE source = ?1 AND target = ?2",
            [&source, &first],
            |row| row.get(0),
        )
        .expect("links row");
    assert_eq!(indexed, 1, "the derived Link row is written");

    // The same unlocked source removes the Link; the generated section shrinks again.
    mcp.ok("edit_card", json!({ "at": source, "body": "no link now" }));
    let uncited = gui.get(&format!("/api/card?at={first}"));
    assert_eq!(
        uncited["authored_fingerprint"].as_str(),
        Some(member_fingerprint.as_str())
    );
    assert!(
        !uncited["text"].as_str().expect("stored text").contains(&source),
        "the Reverse section no longer names the source: {}",
        uncited["text"]
    );

    // The other side of the same promise: a locked source authors no new Link, and a request that
    // would change nothing is refused like any other (R082).
    let refusal = mcp.refused(
        "edit_card",
        json!({ "at": first, "body": format!("now cites [[{source}]]") }),
    );
    assert_eq!(refusal["code"], "locked", "{refusal}");
    let refusal = mcp.refused("edit_card", json!({ "at": first, "body": "first body" }));
    assert_eq!(refusal["code"], "locked", "{refusal}");

    // A Reply writes one human Card and updates only the locked target's generated section.
    let reply = gui.post(
        "/api/reply",
        &json!({
            "at": first,
            "base": member_fingerprint,
            "text": format!("# 回应：条件与边界\n\n印证 [[{first}]]：这条回应补充了条件与适用边界。\n"),
        }),
    );
    assert_eq!(reply["saved"], true, "{reply}");
    assert_eq!(reply["target"]["workspace"], "agent");
    let reply_at = reply["address"].as_str().expect("reply address").to_string();
    let answered = gui.get(&format!("/api/card?at={first}"));
    assert_eq!(
        answered["authored_fingerprint"].as_str(),
        Some(member_fingerprint.as_str()),
        "a Reply changes no authored identity of its target"
    );
    assert!(
        answered["text"].as_str().expect("stored text").contains(&reply_at),
        "the Reply is visible in the locked Card's Reverse section: {}",
        answered["text"]
    );
    let reply_indexed: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM links WHERE source = ?1 AND target = ?2",
            [&reply_at, &first],
            |row| row.get(0),
        )
        .expect("reply link row");
    assert_eq!(reply_indexed, 1);

    // The whole promise is unmoved, and the derived index still matches every body.
    let panel = gui.panel();
    assert_eq!(member_facts(&panel), before);
    assert_eq!(panel["integrity"]["ok"], true);
    let active = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lock| lock["unlocked_at"].is_null())
        .expect("the Lock");
    assert_eq!(active["seal"], seal);
    let checked = home
        .cmd()
        .args(["status", "--check-links"])
        .output()
        .expect("zt status --check-links");
    let report = String::from_utf8_lossy(&checked.stdout);
    assert!(checked.status.success(), "{report}");
    assert!(report.contains("\"ok\":true"), "{report}");
}

/// AC05: only an authored Link macro recognized by the existing parser pins a Card. Fenced and
/// inline code, a chart's `data-zt` navigation, a Document link and generated Reverse text do not.
#[test]
fn only_authored_link_macros_pin_a_target() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let targets = topic(&mut mcp, "Targets");
    let real = card(&mut mcp, &targets, "Real", "truly linked");
    let fenced_topic = topic(&mut mcp, "FencedTopic");
    let fenced = card(&mut mcp, &fenced_topic, "Fenced", "named in a code fence");
    let inline_topic = topic(&mut mcp, "InlineTopic");
    let inline = card(&mut mcp, &inline_topic, "Inline", "named in inline code");
    let chart_topic = topic(&mut mcp, "ChartTopic");
    let chart = card(&mut mcp, &chart_topic, "Chart", "named by a chart");
    let doc_topic = topic(&mut mcp, "DocTopic");
    let doc = card(&mut mcp, &doc_topic, "Doc", "named by a Document link");
    let body = format!(
        "real: [[{real}]]\n\n```text\n[[{fenced}]]\n```\n\ninline `[[{inline}]]` here\n\n\
         ```chart\n<svg data-zt=\"{chart}\"></svg>\n```\n\n\
         <figure data-zt=\"{chart}\"></figure>\n\n[the document](scholar:12)"
    );
    let member = card(&mut mcp, &root, "Member", &body);

    let plan = gui.post("/api/lock", &json!({ "root": root }));
    assert_eq!(plan["plan"]["pin_count"], 1, "{plan}");
    assert_eq!(plan["plan"]["pins"][0]["target"], real);
    assert_eq!(
        plan["plan"]["pins"][0]["sources"][0], member,
        "the member is the one authored source"
    );
    assert_eq!(
        plan["plan"]["warnings"].as_array().map(Vec::len),
        Some(0),
        "a fenced or inline example is no authored Link at all: {plan}"
    );
    let applied = gui.post(
        "/api/lock",
        &json!({ "root": root, "confirm": "lock", "plan_hash": plan["plan"]["plan_hash"] }),
    );
    assert_eq!(applied["applied"], true, "{applied}");

    // The one authored Link is pinned.
    let refusal = mcp.refused("delete_card", json!({ "at": real }));
    assert_eq!(refusal["code"], "pinned", "{refusal}");
    assert_eq!(refusal["pinned_by"][0]["sources"][0], member);

    // Everything the parser does not read as a Link stays free.
    for target in [&fenced, &inline, &chart, &doc] {
        delete(&mut mcp, target);
    }
    let remaining: i64 = home
        .db()
        .query_row("SELECT COUNT(*) FROM lock_pins", [], |row| row.get(0))
        .expect("pin rows");
    assert_eq!(remaining, 1, "only the authored macro made a pin");
    assert_eq!(gui.panel()["integrity"]["ok"], true);
}

/// T19 / AC06: a Broken Link present at lock time is retained as a warning with its source and
/// target, creates no pin, and joins the promise only when the owner unlocks and locks again.
#[test]
fn a_broken_link_is_a_warning_that_a_relock_captures() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let targets = topic(&mut mcp, "Targets");
    let doomed = card(&mut mcp, &targets, "Doomed", "the target that goes away");
    let member = card(&mut mcp, &root, "Member", &format!("refers to [[{doomed}]]"));
    delete(&mut mcp, &doomed);
    let broken: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE COALESCE(location,citation_key) = ?1",
            [&doomed],
            |row| row.get(0),
        )
        .expect("doomed row");
    assert_eq!(broken, 0, "the Link is broken before the Lock");

    // The plan shows the Broken Link with both addresses, and explains the limits of the promise.
    let plan = gui.post("/api/lock", &json!({ "root": root }));
    let warnings = plan["plan"]["warnings"].as_array().expect("warnings");
    assert_eq!(warnings.len(), 1, "{plan}");
    assert_eq!(warnings[0]["source"], member);
    assert_eq!(warnings[0]["target"], doomed);
    assert_eq!(plan["plan"]["pin_count"], 0, "a Broken Link pins nothing");
    let lines = plan["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|line| line.as_str().unwrap().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        lines.contains("unlock and lock again"),
        "the exception is explained: {lines}"
    );
    assert!(
        lines.contains("outside the seal promise"),
        "the promise excludes target meaning and external availability: {lines}"
    );

    let applied = gui.post(
        "/api/lock",
        &json!({ "root": root, "confirm": "lock", "plan_hash": plan["plan"]["plan_hash"] }),
    );
    assert_eq!(applied["applied"], true, "{applied}");
    let lock_id = applied["lock"]["lock_id"].as_i64().expect("lock id");

    // The retained record still names the warning's source and target.
    let panel = gui.panel();
    let active = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lock| lock["lock_id"] == lock_id)
        .expect("the Lock");
    let retained = active["warnings"].as_array().expect("retained warnings");
    assert_eq!(retained.len(), 1, "{active}");
    assert_eq!(retained[0]["source"], member);
    assert_eq!(retained[0]["target"], doomed);
    assert_eq!(
        active["events"][0]["detail"]["warnings"][0]["target"],
        doomed
    );

    // The target appears later at the same address: the baseline does not silently grow a pin.
    let revived = card(&mut mcp, &targets, "Revived", "the old address, a new target");
    assert_eq!(revived, doomed, "the same address is live again");
    mcp.ok("edit_card", json!({ "at": revived, "body": "edited freely" }));
    assert_eq!(gui.panel()["integrity"]["ok"], true);
    let pins: i64 = home
        .db()
        .query_row("SELECT COUNT(*) FROM lock_pins", [], |row| row.get(0))
        .expect("pin rows");
    assert_eq!(pins, 0, "no pin was added");
    // It is free like any unpinned Card: it can be edited and deleted, then the address is reused.
    delete(&mut mcp, &revived);
    let revived = card(&mut mcp, &targets, "Revived again", "live when the owner relocks");
    assert_eq!(revived, doomed, "the same address is live again");

    // Unlocking and locking again captures the now-resolving Link.
    gui.unlock(lock_id);
    let plan = gui.post("/api/lock", &json!({ "root": root }));
    assert_eq!(plan["plan"]["pin_count"], 1, "{plan}");
    assert_eq!(plan["plan"]["pins"][0]["target"], doomed);
    assert_eq!(plan["plan"]["warnings"].as_array().map(Vec::len), Some(0));
    let relocked = gui.lock(&root, None);
    assert_ne!(relocked["lock_id"].as_i64(), Some(lock_id));
    let refusal = mcp.refused("delete_card", json!({ "at": revived }));
    assert_eq!(refusal["code"], "pinned", "{refusal}");
    assert_eq!(gui.panel()["integrity"]["ok"], true);
}

/// The whole set of stored addresses, so a refused write can prove it left no Card behind.
fn stored_addresses(home: &TestHome) -> Vec<String> {
    let conn = home.db();
    let mut stmt = conn
        .prepare("SELECT COALESCE(location,citation_key) FROM cards ORDER BY 1")
        .expect("cards query");
    stmt.query_map([], |row| row.get::<_, String>(0))
        .expect("cards")
        .collect::<std::result::Result<Vec<_>, _>>()
        .expect("cards")
}

/// T09 / AC07: a Reply or a hub/Spread draft aimed at a locked destination refuses atomically and
/// keeps its draft, with no fallback Topic and no partial Card left behind.
#[test]
fn a_draft_into_a_locked_destination_leaves_no_partial_card() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();

    // An agent Card to answer, and an agent Topic that the owner locks.
    let agent_topic = topic(&mut mcp, "AgentTree");
    let agent_card = card(&mut mcp, &agent_topic, "AgentCard", "the agent's statement");
    let agent_member = card(&mut mcp, &agent_card, "AgentMember", "agent body");

    // A locked human Topic as the reply destination, and an open one beside it.
    let human = address(gui.post(
        "/api/create",
        &json!({ "kind": "topic", "text": "# HumanHome\n\nhuman body\n" }),
    ));
    let locked_member = address(gui.post(
        "/api/create",
        &json!({ "kind": "direct", "at": human, "text": "# HumanFirst\n\nfirst human card\n" }),
    ));
    let open_topic = address(gui.post(
        "/api/create",
        &json!({ "kind": "topic", "text": "# OpenHome\n\nopen body\n" }),
    ));
    let human_lock = gui.lock(&human, None);
    let agent_lock = gui.lock(&agent_topic, None);
    assert_ne!(human_lock["lock_id"], agent_lock["lock_id"]);
    let seal = agent_lock["seal"].clone();

    let base = gui.get(&format!("/api/card?at={agent_card}"))["authored_fingerprint"]
        .as_str()
        .expect("fingerprint")
        .to_string();
    let before = stored_addresses(&home);

    // A Reply aimed at the locked human Topic: a refusal that names the Lock, not an error.
    let (status, refused) = gui.post_with(
        "/api/reply",
        &gui.token,
        &json!({
            "at": agent_card,
            "base": base,
            "topic": human,
            "text": format!("# 回应：条件\n\n印证 [[{agent_card}]]：这条回应落在被锁的目的地里。\n"),
        }),
    );
    assert_eq!(status, 200, "a locked destination is a kept refusal: {refused}");
    assert_eq!(refused["saved"], false, "{refused}");
    assert_eq!(refused["reason"], "locked_destination", "{refused}");
    assert!(
        refused["detail"]
            .as_str()
            .unwrap()
            .contains(&human),
        "the refusal names the locked destination: {refused}"
    );
    assert_eq!(refused["refusal"]["code"], "locked_tree", "{refused}");
    assert_eq!(refused["target"]["address"], agent_card);

    // Nothing was written: no partial Reply Card and no fallback Topic.
    assert_eq!(
        stored_addresses(&home),
        before,
        "a refused draft writes no Card at all"
    );
    let reply_topic: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE text LIKE '# 回应%'",
            [],
            |row| row.get(0),
        )
        .expect("reply topics");
    assert_eq!(reply_topic, 0, "no fallback Topic was created");

    // The same Reply into an unlocked human Topic saves, so only the destination blocked it.
    let saved = gui.post(
        "/api/reply",
        &json!({
            "at": agent_card,
            "base": base,
            "topic": open_topic,
            "text": format!("# 回应：条件\n\n印证 [[{agent_card}]]：这条回应落在开放的目的地里。\n"),
        }),
    );
    assert_eq!(saved["saved"], true, "{saved}");
    assert_eq!(saved["destination"]["topic"], open_topic);

    // The hub draft plan still reads Cards of a locked tree, and the writing route it files with
    // refuses to add a Card inside that tree.
    let cards_before = stored_addresses(&home);
    let plan = gui.post(
        "/api/hub/draft",
        &json!({ "cards": [agent_card, agent_member], "question": "how does it work", "title": "Draft" }),
    );
    assert!(plan["draft"]["text"].as_str().is_some(), "{plan}");
    let (status, refused) = gui.post_with(
        "/api/create",
        &gui.token,
        &json!({
            "kind": "side",
            "at": agent_member,
            "type": "hub",
            "text": format!("# Draft hub\n\n- [[{agent_card}]]\n"),
        }),
    );
    assert_eq!(status, 409, "a locked tree takes no filed draft: {refused}");
    assert_eq!(refused["refusal"]["code"], "locked_tree", "{refused}");
    assert_eq!(
        stored_addresses(&home),
        cards_before,
        "the refused filing left no Card"
    );
    let (status, refused) = gui.post_with(
        "/api/create",
        &gui.token,
        &json!({ "kind": "side", "at": locked_member, "text": "# Draft inside\n\nbody\n" }),
    );
    assert_eq!(status, 409, "the locked human tree takes no Card: {refused}");
    assert_eq!(refused["refusal"]["code"], "locked_tree", "{refused}");
    assert_eq!(stored_addresses(&home), cards_before);

    // A Spread is not an authored Card change, so keeping one over locked Cards is allowed.
    let spreads = gui.post(
        "/api/spreads",
        &json!({ "current": "s1", "spreads": [{
            "id": "s1",
            "name": "Locked set",
            "cards": [
                { "at": agent_card, "x": 0, "y": 0 },
                { "at": agent_member, "x": 40, "y": 20 }
            ]
        }] }),
    );
    assert!(
        spreads["spreads"].as_array().map(Vec::len) == Some(1),
        "{spreads}"
    );

    // Only the human destination refused; the locked agent source is readable and replied to above.
    assert_eq!(gui.active_lock()["lock_id"], human_lock["lock_id"]);
    let agent_panel = gui.panel();
    let active = agent_panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|lock| lock["lock_id"] == agent_lock["lock_id"])
        .expect("the agent Lock");
    assert_eq!(active["seal"], seal);
    assert_eq!(agent_panel["integrity"]["ok"], true);
}

/// AC04: a pin crosses Workspaces, and it does not propagate: the locked tree's own Links pin their
/// immediate targets only.
#[test]
fn a_pin_is_cross_workspace_and_does_not_propagate() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();

    // A human Card, named by a Link authored in the agent Workspace.
    let human_topic = address(gui.post(
        "/api/create",
        &json!({ "kind": "topic", "text": "# HumanTopic\n\nhuman body\n" }),
    ));
    let human_card = address(gui.post(
        "/api/create",
        &json!({ "kind": "direct", "at": human_topic, "text": "# HumanCard\n\nhuman thought\n" }),
    ));

    let root = topic(&mut mcp, "Frozen");
    let member = card(&mut mcp, &root, "Member", &format!("cites [[{human_card}]]"));
    // The pinned target links onward, and that second target is not itself promised.
    let bridge_topic = topic(&mut mcp, "Bridge");
    let bridge = card(&mut mcp, &bridge_topic, "Bridge", "the pinned target");
    let farther_topic = topic(&mut mcp, "Farther");
    let farther = card(&mut mcp, &farther_topic, "Farther", "one Link beyond the promise");
    mcp.ok(
        "edit_card",
        json!({ "at": bridge, "body": format!("continues to [[{farther}]]") }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": member, "body": format!("cites [[{human_card}]] and [[{bridge}]]") }),
    );

    let plan = gui.post("/api/lock", &json!({ "root": root }));
    let targets = plan["plan"]["pins"]
        .as_array()
        .expect("pins")
        .iter()
        .map(|pin| pin["target"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(targets, vec![human_card.clone(), bridge.clone()], "{plan}");
    let lock = gui.lock(&root, None);
    let seal = lock["seal"].clone();

    // The human Card is pinned even though the Lock lives in the agent Workspace.
    let (status, refused) = gui.post_with(
        "/api/delete",
        &gui.token,
        &json!({ "at": human_card }),
    );
    assert_eq!(status, 409, "a pinned human Card cannot be deleted: {refused}");
    assert_eq!(refused["refusal"]["code"], "pinned", "{refused}");
    assert_eq!(refused["refusal"]["pinned_by"][0]["tree"], root);
    assert_eq!(refused["refusal"]["pinned_by"][0]["sources"][0], member);
    // Its authored meaning still changes freely.
    gui.post(
        "/api/edit",
        &json!({ "at": human_card, "text": "# HumanCard\n\nthe human thought may change\n" }),
    );

    // One Link beyond the promise: the farther Card is not pinned.
    mcp.ok("edit_card", json!({ "at": farther, "body": "free to change" }));
    delete(&mut mcp, &farther);

    // The bridge itself stays pinned.
    let refusal = mcp.refused("delete_card", json!({ "at": bridge }));
    assert_eq!(refusal["code"], "pinned", "{refusal}");
    assert_eq!(
        gui.panel()["locks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["unlocked_at"].is_null())
            .expect("the Lock")["seal"],
        seal
    );
    assert_eq!(gui.panel()["integrity"]["ok"], true);
}

/// Every Card address a read payload names, in document order, from its own `address`/`at` fields.
fn read_addresses(value: &Value) -> Vec<String> {
    fn walk(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, item) in map {
                    if (key == "address" || key == "at")
                        && let Some(text) = item.as_str()
                    {
                        out.push(text.to_string());
                    }
                    walk(item, out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(value, &mut out);
    out
}

/// S02-AC01: every read path keeps a locked tree, with the same selection and ranking semantics.
#[test]
fn every_read_path_still_serves_locked_cards() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "the quokka equilibrium");
    let model = side_card(&mut mcp, &first, "Quokka model", "a quokka model");
    mcp.ok(
        "edit_card",
        json!({ "at": model, "type": "model", "level": "concept" }),
    );
    let hub = side_card(&mut mcp, &model, "Quokka hub", "hubs the quokka cards");
    mcp.ok(
        "edit_card",
        json!({ "at": hub, "type": "hub", "level": "line" }),
    );
    let outside = topic(&mut mcp, "Outside");
    let other = card(&mut mcp, &outside, "Other", "quokka appears here too");
    let tree = root.split_once('/').unwrap().0.to_string();

    // One Spread keeps the Table's placement over Cards of both trees.
    let spread = json!({ "current": "s1", "spreads": [{
        "id": "s1",
        "name": "Quokka set",
        "cards": [
            { "at": first, "x": 0, "y": 0 },
            { "at": model, "x": 30, "y": 10 },
            { "at": other, "x": 60, "y": 20 }
        ]
    }] });
    gui.post("/api/spreads", &spread);

    // Reads before the Lock.
    let before = json!({
        "search_plain": read_addresses(&gui.get("/api/search?ws=agent&q=quokka")),
        "search_entry": read_addresses(&gui.get("/api/search?ws=agent&q=quokka&mode=entry")),
        "tree": read_addresses(&gui.get(&format!("/api/tree?id={tree}"))),
        "graph": read_addresses(&gui.get("/api/graph")),
        "ask": read_addresses(&gui.post("/api/ask", &json!({ "question": "quokka" }))),
        "register": read_addresses(&gui.get("/api/register?ws=agent")),
        "overview": gui.get("/api/overview")["stats"].clone(),
        "card": gui.get(&format!("/api/card?at={first}"))["body"].clone(),
        "mcp_search": read_addresses(&mcp.ok("search_cards", json!({ "query": "quokka", "workspace": "agent" }))),
        "mcp_search_entry": read_addresses(&mcp.ok("search_cards", json!({ "query": "quokka", "workspace": "agent", "mode": "entry" }))),
        "mcp_tree": read_addresses(&mcp.ok("get_tree", json!({ "tree": tree, "bodies": true }))),
        "mcp_model": read_addresses(&mcp.ok("get_model", json!({ "at": model }))),
        "mcp_hub": read_addresses(&mcp.ok("get_hub", json!({ "at": hub }))),
        "mcp_hubs": read_addresses(&mcp.ok("list_hubs", json!({ "workspace": "agent" }))),
        "mcp_register": read_addresses(&mcp.ok("get_register", json!({ "workspace": "agent" }))),
        "mcp_ask": read_addresses(&mcp.ok("ask_box", json!({ "question": "quokka", "workspace": "agent" }))),
        "mcp_titles": read_addresses(&mcp.ok("export_titles", json!({ "workspace": "agent" }))),
        "spreads": gui.get("/api/spreads")["spreads"].clone(),
    });
    let plain = before["search_plain"].as_array().expect("search rows");
    assert!(plain.len() >= 3, "all three quokka Cards are found: {before}");
    assert!(
        before["ask"].as_array().map(Vec::len).unwrap_or(0) >= 1,
        "the question still proposes a Card: {before}"
    );
    assert!(
        before["mcp_model"].as_array().map(Vec::len).unwrap_or(0) >= 1,
        "the model report names its Cards: {before}"
    );

    let lock = gui.lock(&root, None);
    let seal = lock["seal"].clone();

    // The same reads after the Lock: the same Cards, in the same order, with unchanged facts.
    let after = json!({
        "search_plain": read_addresses(&gui.get("/api/search?ws=agent&q=quokka")),
        "search_entry": read_addresses(&gui.get("/api/search?ws=agent&q=quokka&mode=entry")),
        "tree": read_addresses(&gui.get(&format!("/api/tree?id={tree}"))),
        "graph": read_addresses(&gui.get("/api/graph")),
        "ask": read_addresses(&gui.post("/api/ask", &json!({ "question": "quokka" }))),
        "register": read_addresses(&gui.get("/api/register?ws=agent")),
        "overview": gui.get("/api/overview")["stats"].clone(),
        "card": gui.get(&format!("/api/card?at={first}"))["body"].clone(),
        "mcp_search": read_addresses(&mcp.ok("search_cards", json!({ "query": "quokka", "workspace": "agent" }))),
        "mcp_search_entry": read_addresses(&mcp.ok("search_cards", json!({ "query": "quokka", "workspace": "agent", "mode": "entry" }))),
        "mcp_tree": read_addresses(&mcp.ok("get_tree", json!({ "tree": tree, "bodies": true }))),
        "mcp_model": read_addresses(&mcp.ok("get_model", json!({ "at": model }))),
        "mcp_hub": read_addresses(&mcp.ok("get_hub", json!({ "at": hub }))),
        "mcp_hubs": read_addresses(&mcp.ok("list_hubs", json!({ "workspace": "agent" }))),
        "mcp_register": read_addresses(&mcp.ok("get_register", json!({ "workspace": "agent" }))),
        "mcp_ask": read_addresses(&mcp.ok("ask_box", json!({ "question": "quokka", "workspace": "agent" }))),
        "mcp_titles": read_addresses(&mcp.ok("export_titles", json!({ "workspace": "agent" }))),
        "spreads": gui.get("/api/spreads")["spreads"].clone(),
    });
    for field in [
        "search_plain",
        "search_entry",
        "tree",
        "graph",
        "ask",
        "register",
        "overview",
        "card",
        "mcp_search",
        "mcp_search_entry",
        "mcp_tree",
        "mcp_model",
        "mcp_hub",
        "mcp_hubs",
        "mcp_register",
        "mcp_ask",
        "mcp_titles",
        "spreads",
    ] {
        assert_eq!(after[field], before[field], "`{field}` changed when the tree locked");
    }
    // The locked Cards are still selectable, readable and on the Table.
    assert!(after["search_plain"].as_array().unwrap().iter().any(|at| at == &first));
    assert_eq!(after["card"], before["card"]);
    assert_eq!(
        after["spreads"][0]["cards"].as_array().map(Vec::len),
        Some(3)
    );
    assert_eq!(gui.active_lock()["seal"], seal);
    assert_eq!(gui.panel()["integrity"]["ok"], true);
}

/// R084 / AC03: a target that also belongs to a locked tree obeys that stronger Lock, and the pin
/// still protects its address from the other tree's promise.
#[test]
fn a_target_in_another_locked_tree_obeys_its_own_lock() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let first_tree = topic(&mut mcp, "FirstPromise");
    let member = card(&mut mcp, &first_tree, "Member", "promises an outside target");
    let second_tree = topic(&mut mcp, "SecondPromise");
    let target = card(&mut mcp, &second_tree, "Target", "a locked target");
    mcp.ok(
        "edit_card",
        json!({ "at": member, "body": format!("promises [[{target}]]") }),
    );
    let first_lock = gui.lock(&first_tree, None);
    let second_lock = gui.lock(&second_tree, None);

    // The target's own Lock is the stronger rule: no authored edit of its own.
    let refusal = mcp.refused(
        "edit_card",
        json!({ "at": target, "body": "the second promise refuses this" }),
    );
    assert_eq!(refusal["code"], "locked", "{refusal}");
    assert_eq!(refusal["locked_tree"], second_tree);
    assert_eq!(gui.panel()["integrity"]["ok"], true);

    // Once its own Lock ends, the pin still keeps the address and row: edits pass, moves do not.
    gui.unlock(second_lock["lock_id"].as_i64().expect("lock id"));
    mcp.ok("edit_card", json!({ "at": target, "body": "free to mean something else" }));
    let refusal = mcp.refused("delete_card", json!({ "at": target }));
    assert_eq!(refusal["code"], "pinned", "{refusal}");
    assert_eq!(refusal["pinned_by"][0]["lock_id"], first_lock["lock_id"]);
    assert_eq!(gui.panel()["integrity"]["ok"], true);
}

/// AC08: the derived Link index and every generated Reverse section are still exact after an
/// allowed write inside a locked archive and after a rebuild the owner runs while it is locked.
#[test]
fn a_locked_archive_keeps_its_link_index_and_reverse_sections_exact() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let source_topic = topic(&mut mcp, "Outside");
    let source = card(&mut mcp, &source_topic, "Source", "no link yet");
    let lock = gui.lock(&root, None);
    let seal = lock["seal"].clone();
    let before = member_facts(&gui.panel());

    // The allowed incoming Link, then a rebuild while the tree is locked.
    mcp.ok(
        "edit_card",
        json!({ "at": source, "body": format!("cites [[{first}]]") }),
    );
    let reindexed = home.cmd().arg("reindex").output().expect("zt reindex");
    let report = String::from_utf8_lossy(&reindexed.stdout);
    assert!(reindexed.status.success(), "{report}");
    assert!(report.contains("\"ok\":true"), "{report}");
    let checked = home
        .cmd()
        .args(["status", "--check-links"])
        .output()
        .expect("zt status --check-links");
    assert!(checked.status.success());
    let report = String::from_utf8_lossy(&checked.stdout);
    assert!(report.contains("\"ok\":true"), "{report}");

    // The authored promise is untouched by the generated refresh.
    let panel = gui.panel();
    assert_eq!(member_facts(&panel), before);
    let active = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["unlocked_at"].is_null())
        .expect("the Lock");
    assert_eq!(active["seal"], seal);
    assert_eq!(panel["integrity"]["ok"], true);
    let reverse: String = home
        .db()
        .query_row(
            "SELECT text FROM cards WHERE location = ?1",
            [&first],
            |row| row.get(0),
        )
        .expect("stored text");
    assert!(
        reverse.contains(&source),
        "the generated Reverse section names its source: {reverse}"
    );
}

/// The row of one address from any read payload's array, by its own `address` field.
fn row_at<'a>(rows: &'a Value, address: &str) -> &'a Value {
    rows.as_array()
        .into_iter()
        .flatten()
        .find(|row| row["address"] == json!(address))
        .unwrap_or_else(|| panic!("no row for `{address}` in {rows}"))
}

/// Every stored Card text, in row order: the independent oracle for "no Card changed".
fn stored_texts(home: &TestHome) -> Vec<(String, String)> {
    let conn = home.db();
    let mut statement = conn
        .prepare("SELECT COALESCE(location,citation_key), text FROM cards ORDER BY row_id")
        .expect("query");
    statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("rows")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("texts")
}

/// S03-AC01..AC07: every read projection names the same Lock facts at one archive revision, a lean
/// read answers what a full read answers, and a Lock-only change reaches the open GUI.
#[test]
fn every_read_projection_names_the_same_lock_facts_at_one_revision() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "the quokka equilibrium");
    let side = side_card(&mut mcp, &first, "Side", "the quokka side");
    let outside = topic(&mut mcp, "Outside");
    let target = card(&mut mcp, &outside, "Target", "the quokka target");
    // The first tree promises the outside Card with an authored Link, so its Lock pins that Card.
    mcp.ok(
        "edit_card",
        json!({ "at": side, "body": format!("the quokka side cites [[{target}]]") }),
    );
    let tree = root.split_once('/').expect("tree id").0.to_string();

    let lock = gui.lock(&root, Some("frozen for review"));
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    assert_eq!(gui.active_lock()["seal"], lock["seal"]);

    // The stored facts, read straight from SQLite, are the independent oracle.
    let (stored_lock, stored_root): (i64, String) = home
        .db()
        .query_row(
            "SELECT lock_id, root_address FROM locks WHERE unlocked_at IS NULL",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the active Lock row");
    assert_eq!(stored_lock, lock_id);
    assert_eq!(stored_root, root);
    let stored_pin: String = home
        .db()
        .query_row("SELECT target_address FROM lock_pins", [], |row| {
            row.get(0)
        })
        .expect("the pin row");
    assert_eq!(stored_pin, target);

    // get_card: a member carries the Lock, the pinned target carries the pin, neither changes the
    // existing Card `tree` field.
    let member = mcp.ok("get_card", json!({ "at": side }));
    assert_eq!(member["lock"]["lock_id"], json!(lock_id), "{member}");
    assert_eq!(
        member["lock"]["tree"],
        json!(root),
        "the annotation names the Locked tree's root address"
    );
    assert!(member["lock"]["since"].is_string(), "{member}");
    assert_eq!(member["pinned_by"], json!([]));
    assert_eq!(member["writable"], json!(false), "an active Lock is not writable");
    assert_eq!(member["tree"], json!(tree), "the Card tree id is unchanged");

    let pinned = mcp.ok("get_card", json!({ "at": target }));
    assert!(pinned["lock"].is_null(), "{pinned}");
    assert_eq!(pinned["pinned_by"], json!([root]));
    assert_eq!(
        pinned["writable"],
        json!(true),
        "a pin alone leaves authored editing possible"
    );

    // Both bulk views agree with get_card and with each other, at one revision.
    let addresses = json!([root, first, side, target, outside]);
    let full = mcp.ok("get_cards", json!({ "addresses": addresses, "view": "full" }));
    let lean = mcp.ok("get_cards", json!({ "addresses": addresses, "view": "text" }));
    assert_eq!(full["archive_revision"], lean["archive_revision"]);
    for at in [&root, &first, &side, &target, &outside] {
        let full_row = row_at(&full["cards"], at);
        let lean_row = row_at(&lean["cards"], at);
        let single = mcp.ok("get_card", json!({ "at": at }));
        for field in ["lock", "pinned_by", "writable"] {
            assert_eq!(full_row[field], single[field], "{field} of `{at}` in the full view");
            assert_eq!(lean_row[field], single[field], "{field} of `{at}` in the lean view");
        }
    }

    // get_tree, with and without bodies, carries the annotations on every member.
    let bodies = mcp.ok("get_tree", json!({ "tree": tree, "bodies": true }));
    let summaries = mcp.ok("get_tree", json!({ "tree": tree }));
    for value in [&bodies, &summaries] {
        let row = row_at(&value["cards"], &side);
        assert_eq!(row["lock"]["lock_id"], json!(lock_id), "{value}");
        assert_eq!(row["pinned_by"], json!([]));
    }

    // list_trees and the GUI's own tree list add `locked`.
    let trees = mcp.ok("list_trees", json!({ "workspace": "agent" }));
    for row in trees["trees"].as_array().expect("tree rows") {
        let expected = row["root"] == json!(root);
        assert_eq!(row["locked"], json!(expected), "{row}");
    }
    let overview = gui.get("/api/overview");
    for row in overview["trees"].as_array().expect("gui tree rows") {
        let expected = row["root"] == json!(root);
        assert_eq!(row["locked"], json!(expected), "{row}");
    }
    assert_eq!(
        overview["revision"],
        full["archive_revision"],
        "the GUI and the MCP server read one revision"
    );

    // Search rows (plain and entry) and the exported title rows carry the same facts.
    for mode in ["plain", "entry"] {
        let found = mcp.ok(
            "search_cards",
            json!({ "query": "quokka", "workspace": "agent", "mode": mode }),
        );
        let row = row_at(&found["results"], &side);
        assert_eq!(row["lock"]["lock_id"], json!(lock_id), "{mode}: {found}");
        assert_eq!(row_at(&found["results"], &target)["pinned_by"], json!([root]));
    }
    let titles = mcp.ok("export_titles", json!({ "workspace": "agent" }));
    assert_eq!(row_at(&titles["titles"], &side)["lock"]["lock_id"], json!(lock_id));
    assert_eq!(row_at(&titles["titles"], &target)["pinned_by"], json!([root]));
    assert_eq!(
        titles["archive_revision"],
        full["archive_revision"],
        "the title rows are read at the same revision"
    );

    // The GUI's summary, detail and navigation rows say the same.
    let gui_tree = gui.get(&format!("/api/tree?id={tree}"));
    assert_eq!(row_at(&gui_tree["cards"], &side)["lock"]["lock_id"], json!(lock_id));
    let graph = gui.get("/api/graph");
    assert_eq!(row_at(&graph["nodes"], &target)["pinned_by"], json!([root]));
    let detail = gui.get(&format!("/api/card?at={side}"));
    assert_eq!(detail["lock"]["lock_id"], json!(lock_id));
    assert_eq!(detail["pinned_by"], json!([]));
    assert_eq!(detail["writable"], json!(false));
    assert_eq!(
        detail["parent"]["lock"]["lock_id"],
        json!(lock_id),
        "the navigation rows carry the facts of the Card they open"
    );

    // A Lock-only change: no Card text moves, yet the browser's heartbeat sees a new fingerprint.
    let texts = stored_texts(&home);
    let plan = gui.post("/api/lock/note", &json!({ "lock_id": lock_id, "note": "still frozen" }));
    let applied = gui.post(
        "/api/lock/note",
        &json!({
            "lock_id": lock_id,
            "note": "still frozen",
            "confirm": "note",
            "plan_hash": plan["plan"]["plan_hash"],
        }),
    );
    assert_eq!(applied["applied"], json!(true), "{applied}");
    let noted = gui.get("/api/overview");
    assert_eq!(stored_texts(&home), texts, "a changed note writes no Card");
    assert_ne!(
        noted["fingerprint"], overview["fingerprint"],
        "the existing heartbeat sees a metadata-only change"
    );
    assert_eq!(
        noted["revision"].as_u64(),
        overview["revision"].as_u64().map(|revision| revision + 1),
        "one note advances the revision exactly once"
    );

    // Releasing the Lock takes the annotations off every projection again.
    gui.unlock(lock_id);
    let released = mcp.ok("get_card", json!({ "at": side }));
    assert!(released["lock"].is_null(), "{released}");
    assert_eq!(released["writable"], json!(true));
    let unfettered = mcp.ok("get_card", json!({ "at": target }));
    assert_eq!(unfettered["pinned_by"], json!([]));
    let after = gui.get("/api/overview");
    assert_ne!(after["fingerprint"], noted["fingerprint"]);
    for row in after["trees"].as_array().expect("gui tree rows") {
        assert_eq!(row["locked"], json!(false), "{row}");
    }
    assert_eq!(gui.active_lock(), Value::Null);
    assert_eq!(gui.panel()["integrity"]["ok"], true);
}

// ---------------------------------------------------------------------------------------------
// S05 (#128): the owner-facing interaction around plans, refusals, drafts and pins.
// ---------------------------------------------------------------------------------------------

/// R217 / S05-AC04 / S05-AC07: a refusal names the exact Card it refused, its Locked tree and its
/// Lock. The reading desk's disabled-control explanation and a refused Save both name that Card, so
/// a refusal about one regular Card never reports the tree's root or another member.
#[test]
fn a_refusal_names_the_card_it_refused_and_its_locked_tree() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let alpha = card(&mut mcp, &root, "Alpha", "alpha body");
    let beta = card(&mut mcp, &alpha, "Beta", "beta body");
    let lock = gui.lock(&root, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");

    // The last regular Card of the tree, through the MCP writer.
    let refusal = mcp.refused("edit_card", json!({ "at": beta, "body": "changed beta" }));
    assert_eq!(refusal["code"], "locked", "{refusal}");
    assert_eq!(refusal["card"], json!(beta), "the refusal names the Card it refused: {refusal}");
    assert_eq!(refusal["locked_tree"], json!(root));
    assert_eq!(refusal["lock_id"], json!(lock_id));
    assert!(
        refusal["error"].as_str().unwrap_or_default().contains(&beta),
        "the message names the Card it refused: {refusal}"
    );

    // The owner's own route answers the same facts under `refusal`.
    let (status, refused) = gui.post_with(
        "/api/edit",
        &gui.token,
        &json!({ "at": beta, "text": "# Beta\n\nchanged\n" }),
    );
    assert_eq!(status, 409, "a Lock refusal is a structured 409: {refused}");
    assert_eq!(refused["refusal"]["card"], json!(beta), "{refused}");
    assert_eq!(refused["refusal"]["locked_tree"], json!(root));
    assert_eq!(refused["refusal"]["lock_id"], json!(lock_id));
    assert!(
        refused["error"].as_str().unwrap_or_default().contains(&beta),
        "the desk reads one message that names the Card: {refused}"
    );

    // An earlier Card of the same tree names itself, not the root and not another member.
    let refusal = mcp.refused("edit_card", json!({ "at": alpha, "type": "event" }));
    assert_eq!(refusal["card"], json!(alpha), "{refusal}");
    // A Literature root inside its own Locked tree names itself too.
    let book = address(mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Smith2024, title={Book}}", "body": "该书。" }),
    ));
    gui.lock(&book, None);
    let (status, refused) = gui.post_with(
        "/api/metadata",
        &gui.token,
        &json!({ "at": book, "bibtex": "@book{Smith2024, title={Book, second edition}}" }),
    );
    assert_eq!(status, 409, "{refused}");
    assert_eq!(refused["refusal"]["card"], json!(book), "{refused}");
    assert_eq!(refused["refusal"]["locked_tree"], json!(book));

    // Nothing was written, and the Lock is intact.
    let stored = mcp.ok("get_card", json!({ "at": beta }));
    assert!(stored["body"].as_str().unwrap_or_default().contains("beta body"), "{stored}");
    assert_eq!(gui.panel()["integrity"]["ok"], true);
}

/// R367 / T13 / S05-AC02: an unlock dialog that saw one Lock, one revision or one archive state
/// cannot release a later Lock. The plan hash binds the Lock id, its seal, the root and the current
/// revision, so every stale dialog answers `plan_changed` and writes nothing.
#[test]
fn a_stale_unlock_dialog_cannot_release_a_later_lock() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let first_lock = gui.lock(&root, Some("keep this"));
    let first_id = first_lock["lock_id"].as_i64().expect("first lock id");
    let seal = first_lock["seal"].clone();

    // A dialog opened on the first Lock. The owner then releases that Lock and locks the tree again;
    // the second promise is a new Lock id over the same untouched content, so its seal is the same.
    let old_plan = gui.post("/api/unlock", &json!({ "lock_id": first_id }));
    let old_hash = old_plan["plan"]["plan_hash"]
        .as_str()
        .expect("old plan hash")
        .to_string();
    gui.unlock(first_id);
    let second = gui.lock(&root, Some("keep this"));
    let second_id = second["lock_id"].as_i64().expect("second lock id");
    assert_ne!(second_id, first_id);
    assert_eq!(second["seal"], seal, "the same content promises the same seal");

    // The old dialog names the new Lock: the hashes differ by Lock id and revision, nothing is written.
    let (status, stale) = gui.post_with(
        "/api/unlock",
        &gui.token,
        &json!({
            "lock_id": second_id,
            "tree": root,
            "confirm": "unlock",
            "plan_hash": old_hash,
        }),
    );
    assert_eq!(status, 200, "{stale}");
    assert_eq!(stale["applied"], false, "{stale}");
    assert_eq!(stale["plan_changed"], true, "{stale}");
    assert_eq!(stale["plan"]["lock_id"], json!(second_id));
    assert_eq!(
        gui.active_lock()["lock_id"],
        json!(second_id),
        "the stale dialog released nothing"
    );

    // The old dialog names the ended Lock: an ended Lock cannot be changed at all.
    let (status, ended) = gui.post_with(
        "/api/unlock",
        &gui.token,
        &json!({
            "lock_id": first_id,
            "tree": root,
            "confirm": "unlock",
            "plan_hash": old_hash,
        }),
    );
    assert_eq!(status, 400, "{ended}");
    assert!(
        ended["error"]
            .as_str()
            .unwrap_or_default()
            .contains("already ended"),
        "{ended}"
    );

    // A changed archive revision renews the plan too: a write outside the tree moves the revision.
    let plan = gui.post("/api/unlock", &json!({ "lock_id": second_id, "tree": root }));
    let fresh_hash = plan["plan"]["plan_hash"]
        .as_str()
        .expect("fresh plan hash")
        .to_string();
    let outside = topic(&mut mcp, "Outside");
    card(&mut mcp, &outside, "Later", "added after the plan");
    let (status, stale) = gui.post_with(
        "/api/unlock",
        &gui.token,
        &json!({
            "lock_id": second_id,
            "tree": root,
            "confirm": "unlock",
            "plan_hash": fresh_hash,
        }),
    );
    assert_eq!(status, 200, "{stale}");
    assert_eq!(stale["plan_changed"], true, "a new revision renews the plan: {stale}");
    assert!(
        stale["plan"]["revision"].as_u64() > plan["plan"]["revision"].as_u64(),
        "the renewed plan reads the newer revision: {stale}"
    );
    assert_eq!(gui.active_lock()["lock_id"], json!(second_id));

    // A dialog naming another tree is refused before it can act on this Lock.
    let (status, wrong) = gui.post_with(
        "/api/unlock",
        &gui.token,
        &json!({ "lock_id": second_id, "tree": outside }),
    );
    assert_eq!(status, 400, "{wrong}");
    assert!(
        wrong["error"].as_str().unwrap_or_default().contains("belongs to tree"),
        "{wrong}"
    );
    assert_eq!(gui.active_lock()["lock_id"], json!(second_id));

    // A confirmed unlock without its hash is invalid, and the renewed plan releases exactly this Lock.
    let (status, bare) = gui.post_with(
        "/api/unlock",
        &gui.token,
        &json!({ "lock_id": second_id, "tree": root, "confirm": "unlock" }),
    );
    assert_eq!(status, 400, "{bare}");
    assert!(
        bare["error"].as_str().unwrap_or_default().contains("plan_hash"),
        "{bare}"
    );
    let plan = gui.post("/api/unlock", &json!({ "lock_id": second_id, "tree": root }));
    let applied = gui.post(
        "/api/unlock",
        &json!({
            "lock_id": second_id,
            "tree": root,
            "confirm": "unlock",
            "plan_hash": plan["plan"]["plan_hash"],
        }),
    );
    assert_eq!(applied["applied"], true, "{applied}");
    assert_eq!(applied["lock"]["lock_id"], json!(second_id));
    assert!(applied["lock"]["unlocked_at"].is_string());
    let stored = mcp.ok("get_card", json!({ "at": first }));
    assert!(stored["lock"].is_null(), "the release reaches every read: {stored}");
}

/// R217 / S05-AC02: the server recomputes every plan fact. A browser that sends its own counts,
/// seal, member array or pins changes nothing, because only the plan hash and the server's own plan
/// decide what commits.
#[test]
fn a_lock_plan_recomputes_every_fact_the_browser_sends() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    card(&mut mcp, &first, "Second", "second body");
    let forged_members = json!([{ "address": root, "row_id": 1, "fingerprint": "0", "root": true }]);

    // A plan request that names a forged member count, seal and member list still plans the tree.
    let forged = gui.post(
        "/api/lock",
        &json!({
            "root": root,
            "member_count": 1,
            "regular_count": 0,
            "seal": "0".repeat(64),
            "members": forged_members,
            "pins": [],
            "revision": 1,
        }),
    );
    assert_eq!(forged["applied"], false, "{forged}");
    let plan = &forged["plan"];
    assert_eq!(plan["member_count"], 3, "the server counts the tree: {forged}");
    assert_eq!(plan["regular_count"], 2);
    assert_ne!(plan["seal"], json!("0".repeat(64)));
    assert_eq!(plan["members"].as_array().map(Vec::len), Some(3));
    assert!(plan["revision"].as_u64() > Some(1));

    // The confirmation carries the same forged facts: only the hash decides, and the stored member
    // snapshot is the server's.
    let applied = gui.post(
        "/api/lock",
        &json!({
            "root": root,
            "confirm": "lock",
            "plan_hash": plan["plan_hash"],
            "member_count": 1,
            "seal": "0".repeat(64),
            "members": forged_members,
        }),
    );
    assert_eq!(applied["applied"], true, "{applied}");
    let lock = &applied["lock"];
    assert_eq!(lock["member_count"], 3);
    assert_eq!(lock["seal"], plan["seal"]);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let members: i64 = home
        .db()
        .query_row(
            "SELECT COUNT(*) FROM lock_members WHERE lock_id = ?1",
            [lock_id],
            |row| row.get(0),
        )
        .expect("member rows");
    assert_eq!(members, 3, "the stored snapshot is the server's");
    let panel = gui.panel();
    let active = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == json!(lock_id))
        .expect("the Lock");
    assert_eq!(active["seal"], plan["seal"]);
    assert_eq!(active["member_count"], 3);
}

/// R226 / S05-AC05: a pin is not a whole-tree Lock. A successor filed under a pinned target moves no
/// promised address, so the server allows it; only an actual mapping that moves a pinned Card, such
/// as an insertion before it, is refused.
#[test]
fn a_pinned_target_takes_new_successors_but_cannot_be_moved_into_place() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let side = side_card(&mut mcp, &first, "Side", "the reference");
    let outside = topic(&mut mcp, "Outside");
    let pinned = card(&mut mcp, &outside, "Pinned", "pinned body");
    mcp.ok(
        "edit_card",
        json!({ "at": side, "body": format!("links [[{pinned}]]") }),
    );
    gui.lock(&root, None);

    // Continuations of the pinned target change no promised address: they are ordinary writes.
    let tree = tree_of(&pinned);
    let child = card(&mut mcp, &pinned, "Child", "a harmless continuation");
    let branch = side_card(&mut mcp, &pinned, "Branch", "another harmless continuation");
    assert_eq!(tree_of(&child), tree, "a direct successor continues the line: {child}");
    assert_eq!(tree_of(&branch), tree, "a side successor branches: {branch}");
    // So is a new continuation filed under that child: the pin covers the target's own address.
    card(&mut mcp, &child, "Grandchild", "one step further");

    // An insertion before the pinned target would move its address, so the mapping is refused before
    // any temporary Location is written.
    let before = home
        .db()
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get::<_, i64>(0))
        .expect("cards");
    let refusal = mcp.refused(
        "insert_card",
        json!({ "before": pinned, "title": "Before", "body": "body" }),
    );
    assert_eq!(refusal["code"], "pinned", "{refusal}");
    assert_eq!(refusal["pinned_by"][0]["tree"], root);
    assert_eq!(refusal["pinned_by"][0]["sources"][0], side);
    assert!(
        refusal["error"].as_str().unwrap_or_default().contains(&side),
        "the refusal names the locked source Card: {refusal}"
    );
    let after = home
        .db()
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get::<_, i64>(0))
        .expect("cards");
    assert_eq!(after, before, "a refused insertion writes no Card");

    // Moving the pinned target itself stays refused, through the GUI route too.
    let targets = topic(&mut mcp, "Targets");
    let (status, moved) = gui.post_with(
        "/api/move",
        &gui.token,
        &json!({ "at": pinned, "to": format!("{}/1", tree_of(&targets)) }),
    );
    assert_eq!(status, 409, "{moved}");
    assert_eq!(moved["refusal"]["code"], "pinned", "{moved}");
    assert_eq!(moved["refusal"]["pinned_by"][0]["tree"], root);
}

/// R212 / R230 / S05-AC08: the Lock has GUI routes only. The MCP server offers no Lock-control tool,
/// and every Lock route keeps the existing GUI token check.
#[test]
fn the_lock_has_gui_routes_only_and_no_mcp_control_tool() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");

    // No MCP tool names a Lock action; the read projections remain.
    let listed = mcp.request("tools/list", json!({}));
    let names = listed["result"]["tools"]
        .as_array()
        .expect("the MCP server lists its tools")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name").to_string())
        .collect::<Vec<_>>();
    assert!(names.iter().any(|name| name == "get_card"), "{names:?}");
    assert!(names.iter().any(|name| name == "list_trees"), "{names:?}");
    for name in &names {
        let lower = name.to_lowercase();
        assert!(
            !lower.contains("lock") && !lower.contains("unlock"),
            "a Lock action is GUI only (R212): {name}"
        );
    }

    // The four routes keep the ordinary token check: without it they answer 403 and change nothing.
    let open = gui.post("/api/lock", &json!({ "root": root }));
    assert_eq!(open["applied"], false);
    for (path, body) in [
        ("/api/lock", json!({ "root": root })),
        ("/api/unlock", json!({ "lock_id": 1 })),
        ("/api/lock/note", json!({ "lock_id": 1, "note": "x" })),
    ] {
        let (status, refused) = gui.post_with(path, "not-the-token", &body);
        assert_eq!(status, 403, "{path}: {refused}");
        assert!(
            refused["error"].as_str().unwrap_or_default().contains("token"),
            "{path}: {refused}"
        );
    }
    let (status, refused) = gui.get_with("/api/locks", "not-the-token");
    assert_eq!(status, 403, "{refused}");
    assert_eq!(gui.panel()["locks"].as_array().map(Vec::len), Some(0));
}

// ---------------------------------------------------------------------------------------------
// S06 (#129): detect exact integrity faults and keep owner recovery reachable.
// ---------------------------------------------------------------------------------------------

/// The integrity-fault events a Lock retains, newest last.
fn fault_events(panel: &Value, lock_id: i64) -> Vec<Value> {
    panel["locks"]
        .as_array()
        .expect("locks")
        .iter()
        .find(|lock| lock["lock_id"] == json!(lock_id))
        .expect("the Lock")["events"]
        .as_array()
        .expect("events")
        .iter()
        .filter(|event| event["action"] == "integrity-fault")
        .cloned()
        .collect()
}

fn fault_kinds(panel: &Value) -> Vec<String> {
    panel["integrity"]["faults"]
        .as_array()
        .expect("faults")
        .iter()
        .map(|fault| fault["kind"].as_str().expect("kind").to_string())
        .collect()
}

/// R027/R289/S06-AC04: an intact archive writes nothing when the panel opens, and a tampered one
/// never gains a second event for the same fault.
#[test]
fn a_panel_open_records_nothing_while_the_promise_holds() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let lock = gui.lock(&root, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let revision: i64 = home
        .db()
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM metadata WHERE key = 'archive_revision'",
            [],
            |row| row.get(0),
        )
        .expect("revision");

    for _ in 0..3 {
        let panel = gui.panel();
        assert_eq!(panel["integrity"]["ok"], true, "{panel}");
        assert_eq!(panel["integrity"]["latched"], json!([]));
        assert_eq!(fault_events(&panel, lock_id), Vec::<Value>::new());
    }
    // An ordinary read and refresh is not a check point either.
    mcp.ok("get_card", json!({ "at": first }));
    gui.get("/api/overview");
    let panel = gui.panel();
    assert_eq!(fault_events(&panel, lock_id), Vec::<Value>::new());
    let after: i64 = home
        .db()
        .query_row(
            "SELECT CAST(value AS INTEGER) FROM metadata WHERE key = 'archive_revision'",
            [],
            |row| row.get(0),
        )
        .expect("revision");
    assert_eq!(
        after, revision,
        "reading, refreshing or opening a healthy panel changes no revision"
    );
}

/// R229/R377/T20/T23, S06-AC08/S06-AC09: with the locked root gone, the panel reports the exact
/// fault, keeps the reviewed seal, and still lets the owner release the retained Lock by id; the
/// old evidence stays in history and a later Lock gets a new id.
#[test]
fn an_orphaned_faulted_lock_is_reported_and_released_from_the_panel() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    card(&mut mcp, &root, "First", "first body");
    let lock = gui.lock(&root, Some("keep the evidence"));
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let seal = lock["seal"].clone();

    // Outside tampering: the protection triggers go first, then the whole tree, so the Lock keeps
    // its record while its root Card is gone.
    let raw = home.db();
    raw.execute_batch(
        "DROP TRIGGER lock_no_delete; DROP TRIGGER lock_no_change; DROP TRIGGER lock_no_insert;
         DROP TRIGGER lock_no_move_in; DROP TRIGGER lock_pin_no_delete; DROP TRIGGER lock_pin_no_move;",
    )
    .expect("drop the protection triggers");
    raw.execute("DELETE FROM cards WHERE location = ?1", [&root])
        .expect("delete the root");

    let panel = gui.panel();
    assert_eq!(panel["integrity"]["ok"], false);
    let facts = panel["integrity"]["faults"].as_array().expect("faults");
    let root_missing = facts
        .iter()
        .find(|fault| fault["kind"] == "root-missing")
        .unwrap_or_else(|| panic!("no root-missing fault: {panel}"));
    assert_eq!(root_missing["lock_id"], json!(lock_id));
    assert_eq!(root_missing["card"], json!(root));
    assert!(fault_kinds(&panel).contains(&"trigger-missing".to_string()));
    let listed = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == json!(lock_id))
        .expect("the retained Lock");
    assert_eq!(listed["root"], json!(root));
    assert_eq!(listed["seal"], seal, "a fault never rewrites the reviewed seal");
    assert_eq!(listed["seal_intact"], false);
    assert_eq!(listed["note"], "keep the evidence");
    assert!(
        !fault_events(&panel, lock_id).is_empty(),
        "the faults are recorded once"
    );
    // Reading is untouched: the surviving Cards of the archive are still served.
    let overview = gui.get("/api/overview");
    assert!(overview["stats"]["total"].as_i64().is_some_and(|total| total >= 0), "{overview}");

    // Unlock is planned and committed from the retained record, with no root Card to open.
    let ended = gui.unlock(lock_id);
    assert!(ended["unlocked_at"].is_string());
    assert!(gui.active_lock().is_null());
    let panel = gui.panel();
    let history = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == json!(lock_id))
        .expect("the ended Lock keeps its history");
    assert!(history["unlocked_at"].is_string());
    assert_eq!(history["seal"], seal);
    let actions = history["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| event["action"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(actions.contains(&"lock".to_string()), "{actions:?}");
    assert!(actions.contains(&"unlock".to_string()), "{actions:?}");
    assert!(
        actions.iter().filter(|action| *action == "integrity-fault").count() >= 2,
        "the old fault evidence survives the release: {actions:?}"
    );
    assert_eq!(
        panel["integrity"]["latched"],
        json!([]),
        "an ended Lock is historical evidence, not an active promise"
    );

    // The protection comes back at the next start; a new promise on another tree gets a new id.
    home.restart();
    let gui = home.gui();
    let panel = gui.panel();
    assert_eq!(
        panel["integrity"]["faults"],
        json!([]),
        "the guarded startup restored every protection trigger: {panel}"
    );
    assert_eq!(panel["integrity"]["ok"], true);
    home.db()
        .execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type)
             VALUES('5/0', NULL, 1, 0, NULL, ?1, 'human', 'idea')",
            ["# Fresh\n\nfresh root\n\n<!-- zt:reverse-links -->\n"],
        )
    .expect("a fresh root for the next promise");
    let second = gui.lock("5/0", None);
    assert_ne!(second["lock_id"], json!(lock_id), "a new Lock id");
    let panel = gui.panel();
    assert_eq!(panel["integrity"]["ok"], true, "a new promise starts clean");
    let old = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == json!(lock_id))
        .expect("the first Lock stays in history");
    assert_eq!(old["seal"], seal);
    assert!(fault_events(&panel, lock_id).len() >= 2);
}

/// R291/R295/S06-AC06: `zt status --check-locks` runs the full comparison, records its evidence
/// once, and exits 1 on a current or latched fault.
#[test]
fn the_check_locks_command_records_evidence_and_exits_on_a_fault() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    card(&mut mcp, &root, "First", "first body");
    let lock = gui.lock(&root, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");

    let healthy = home.cmd().args(["status", "--check-locks"]).output().unwrap();
    assert!(
        healthy.status.success(),
        "a healthy archive exits 0: {}",
        String::from_utf8_lossy(&healthy.stdout)
    );
    let said = String::from_utf8_lossy(&healthy.stdout);
    assert!(said.contains("integrity: ok"), "{said}");
    assert!(said.contains("recorded: 0 new fault event(s)"), "{said}");

    // Outside tampering: a lost protection trigger is a current fault that the command records.
    home.db()
        .execute_batch("DROP TRIGGER lock_no_move_in")
        .expect("drop the trigger");
    let faulted = home.cmd().args(["status", "--check-locks"]).output().unwrap();
    assert_eq!(
        faulted.status.code(),
        Some(1),
        "a fault exits 1: {}",
        String::from_utf8_lossy(&faulted.stdout)
    );
    let said = String::from_utf8_lossy(&faulted.stdout);
    assert!(said.contains("integrity: faulted"), "{said}");
    assert!(said.contains("fault: schema trigger-missing lock_no_move_in"), "{said}");
    assert!(said.contains("recorded: 1 new fault event(s)"), "{said}");
    assert_eq!(
        fault_events(&gui.panel(), lock_id).len(),
        1,
        "the command's evidence is the panel's evidence"
    );
    // The same fault is never recorded twice; the latch alone still exits 1.
    home.db()
        .execute_batch(
            "CREATE TRIGGER lock_no_move_in BEFORE UPDATE OF location, citation_key ON cards
             WHEN (NEW.location IS NOT OLD.location OR NEW.citation_key IS NOT OLD.citation_key)
              AND EXISTS (
                 SELECT 1 FROM locks l WHERE l.unlocked_at IS NULL
                   AND l.tree = CASE WHEN instr(COALESCE(NEW.location, NEW.citation_key), '/') = 0
                                     THEN COALESCE(NEW.location, NEW.citation_key)
                                     ELSE substr(COALESCE(NEW.location, NEW.citation_key), 1,
                                                 instr(COALESCE(NEW.location, NEW.citation_key), '/') - 1)
                                END
             )
             BEGIN SELECT RAISE(ABORT, 'zt: locked tree takes no new cards'); END;",
        )
        .expect("restore a canonical trigger by hand");
    let latched = home.cmd().args(["status", "--check-locks"]).output().unwrap();
    assert_eq!(latched.status.code(), Some(1), "the latch keeps exiting 1");
    let said = String::from_utf8_lossy(&latched.stdout);
    assert!(
        said.contains("integrity: faulted"),
        "the latch still faults the Lock: {said}"
    );
    assert!(said.contains("recorded: 0 new fault event(s)"), "{said}");
    assert!(
        said.contains(&format!("latched: lock {lock_id} trigger-missing lock_no_move_in")),
        "{said}"
    );
}

/// R232/S08-AC02: a Literature tree is a tree too. The Session refuses its metadata and authored
/// edits before the part choice, so no editor opens over frozen bibliographic data.
#[test]
fn the_session_refuses_a_locked_literature_edit_before_the_part_choice() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let key = address(mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Smith2024, title={Book}}", "body": "该书。" }),
    ));
    let chapter = card(&mut mcp, &key, "Chapter", &format!("chapter of [[{key}]]"));
    let lock = gui.lock(&key, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let before = stored_texts(&home);
    let output = home
        .cmd()
        .write_stdin(format!("go {key}\ne\ngo {chapter}\ne\nq\n"))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let said = String::from_utf8(output).expect("session output is UTF-8");
    // The Literature Card's title row is the Citation key row, and it carries the same mark.
    assert!(said.contains(&format!("citation key: {key} [locked]")), "{said}");
    assert_eq!(
        said.matches(&format!(
            "card `{key}` is in locked tree `{key}` (lock {lock_id}); unlock the tree in the GUI first"
        ))
        .count(),
        1,
        "{said}"
    );
    assert_eq!(
        said.matches(&format!(
            "card `{chapter}` is in locked tree `{key}` (lock {lock_id}); unlock the tree in the GUI first"
        ))
        .count(),
        1,
        "{said}"
    );
    // Neither the part choice nor a metadata or text editor was offered.
    assert!(!said.contains("invalid edit option"), "{said}");
    assert!(!said.contains("metadata edit"), "{said}");
    assert!(!said.contains("edit mode"), "{said}");
    assert_eq!(stored_texts(&home), before);
}

/// R034/R044/S08-AC03: the shell and the Session offer no Lock control. A terminal user sees the
/// marks, the refusal hint and the status counts, and the only Lock command anywhere is the GUI.
#[test]
fn the_shell_and_the_session_offer_no_lock_command() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    gui.lock(&root, None);

    // The shell help names no Lock action; `--check-locks` reports the promise, it cannot change it.
    let shell = home.cmd().arg("help").output().expect("zt help");
    let said = String::from_utf8_lossy(&shell.stdout);
    assert!(said.contains("zt status --check-locks"), "{said}");
    let without_check = said.replace("--check-locks", "");
    assert!(
        !without_check.to_lowercase().contains("lock"),
        "the shell offers no Lock control: {said}"
    );
    // So does the Session's help, at ROOT and on a Card, and a lock command is unknown.
    let output = home
        .cmd()
        .write_stdin(format!(
            "help\ngo {first}\nhelp\nlock {root}\nunlock {root}\nq\n"
        ))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let said = String::from_utf8(output).expect("session output is UTF-8");
    for line in said
        .lines()
        .filter(|line| line.starts_with("  ") || line.contains(" | "))
    {
        assert!(
            !line.to_lowercase().contains("lock"),
            "the Session offers no Lock command: {line}"
        );
    }
    assert!(said.contains("unknown session command: lock"), "{said}");
    assert!(said.contains("unknown session command: unlock"), "{said}");
    // The status counts are the shell's view of the promise, not a control over it.
    assert!(
        String::from_utf8_lossy(&home.cmd().arg("status").output().unwrap().stdout)
            .contains("locks: 1 tree, 2 cards, 0 pinned targets; integrity ok"),
        "status reports the promise"
    );
}

/// R233/R234/R235/S08-AC04: `zt status` reports the active Locked trees, their member snapshot rows
/// and the DISTINCT pinned target rows, prints `integrity ok` only for a healthy comparison, and
/// keeps its exit code 0: only `--check-locks` fails on a fault.
#[test]
fn ordinary_status_reports_the_lock_scope_and_its_integrity() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let side = side_card(&mut mcp, &first, "Side", "side body");
    let outside_root = topic(&mut mcp, "Outside");
    let target = card(&mut mcp, &outside_root, "Target", "target body");
    // Two members of the Locked tree name the same outside Card: two pin rows, one target row.
    mcp.ok(
        "edit_card",
        json!({ "at": first, "body": format!("links [[{target}]]") }),
    );
    mcp.ok(
        "edit_card",
        json!({ "at": side, "body": format!("links [[{target}]] too") }),
    );
    let lock = gui.lock(&root, Some("status scope"));

    let status = home.cmd().arg("status").output().expect("zt status");
    assert!(
        status.status.success(),
        "ordinary status reports; it does not fail: {}",
        String::from_utf8_lossy(&status.stdout)
    );
    let said = String::from_utf8_lossy(&status.stdout);
    assert!(
        said.contains("locks: 1 tree, 3 cards, 1 pinned target; integrity ok"),
        "{said}"
    );
    // The checked command names the same scope on its own line and still exits 0 while healthy.
    let checked = home
        .cmd()
        .args(["status", "--check-locks"])
        .output()
        .expect("zt status --check-locks");
    assert_eq!(checked.status.code(), Some(0));
    let said = String::from_utf8_lossy(&checked.stdout);
    assert!(
        said.contains("locks: 1 tree, 3 cards, 1 pinned target"),
        "{said}"
    );
    // R236/S08-AC06: the counts come from the Lock book itself. No persisted check result and no new
    // polling table was added to answer status.
    let tables: Vec<String> = home
        .db()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE '%lock%' ORDER BY name")
        .expect("tables")
        .query_map([], |row| row.get(0))
        .expect("tables")
        .collect::<std::result::Result<Vec<_>, _>>()
        .expect("tables");
    assert_eq!(
        tables,
        ["lock_events", "lock_members", "lock_pins", "locks"],
        "the Lock schema is still the four evidence tables"
    );

    // An ended Lock counts nowhere: its members are history, not an active promise.
    gui.unlock(lock["lock_id"].as_i64().expect("lock id"));
    let said = String::from_utf8_lossy(&home.cmd().arg("status").output().unwrap().stdout).to_string();
    assert!(
        said.contains("locks: 0 trees, 0 cards, 0 pinned targets; integrity ok"),
        "{said}"
    );

    // A second promise with no Links out counts its own tree only.
    let second = gui.lock(&outside_root, None);
    let said = String::from_utf8_lossy(&home.cmd().arg("status").output().unwrap().stdout).to_string();
    assert!(
        said.contains("locks: 1 tree, 2 cards, 0 pinned targets; integrity ok"),
        "{said}"
    );
    gui.unlock(second["lock_id"].as_i64().expect("lock id"));
}

/// R235/S08-AC04: a fault prints the faulted roots and a concise diagnostic instead of a health
/// claim, while ordinary status still reports rather than failing.
#[test]
fn an_ordinary_status_reports_a_lock_fault_instead_of_health() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    gui.lock(&root, None);
    // Outside tampering: the member's authored text changes while its trigger is gone.
    home.db()
        .execute_batch("DROP TRIGGER lock_no_change")
        .expect("drop the trigger");
    home.db()
        .execute(
            "UPDATE cards SET text = '# First\n\nchanged body\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
            [&first],
        )
        .expect("tamper");
    let status = home.cmd().arg("status").output().expect("zt status");
    assert!(
        status.status.success(),
        "ordinary status reports a fault without failing: {}",
        String::from_utf8_lossy(&status.stdout)
    );
    let said = String::from_utf8_lossy(&status.stdout);
    assert!(
        said.contains("locks: 1 tree, 2 cards, 0 pinned targets; integrity faulted"),
        "{said}"
    );
    assert!(said.contains(&format!("faulted roots: {root}")), "{said}");
    assert!(!said.contains("integrity ok"), "{said}");
    // The check command is the one that fails, and it agrees about the scope and the fault.
    let checked = home.cmd().args(["status", "--check-locks"]).output().unwrap();
    assert_eq!(checked.status.code(), Some(1));
    let said = String::from_utf8_lossy(&checked.stdout);
    assert!(
        said.contains("locks: 1 tree, 2 cards, 0 pinned targets"),
        "{said}"
    );
}

/// R235/S08-AC05: a database the command cannot read is reported as unavailable, never converted
/// into a healthy zero-Lock result, and the unconfigured archive keeps its down/unset output.
#[test]
fn an_unreadable_database_reports_the_lock_scope_as_unavailable() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    card(&mut mcp, &root, "First", "first body");
    drop(mcp);
    home.cmd().arg("down").assert().success();
    // A file that is not a SQLite database: the read fails instead of answering "no Locks".
    fs::write(home.archive_root.join("zt.sqlite3"), "not a database").expect("corrupt the database");
    let status = home.cmd().arg("status").output().expect("zt status");
    let said = String::from_utf8_lossy(&status.stdout);
    assert!(said.contains("locks: unavailable ("), "{said}");
    assert!(
        !said.contains("0 trees, 0 cards, 0 pinned targets"),
        "an unreadable database is not a healthy zero: {said}"
    );
    let checked = home.cmd().args(["status", "--check-locks"]).output().unwrap();
    assert_eq!(checked.status.code(), Some(1));
    let said = String::from_utf8_lossy(&checked.stdout);
    assert!(said.contains("integrity: unavailable ("), "{said}");
    assert!(!said.contains("integrity: ok"), "{said}");
    // An archive that is not configured keeps today's down/unset output and adds no lock claim.
    let unset = tempfile::tempdir().expect("tempdir");
    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin("zt"))
        .arg("status")
        .env("ZT_CONFIG_DIR", unset.path())
        .env("ZT_GUI_PORT", "0")
        .output()
        .expect("zt status without an archive");
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(said.contains("state: down"), "{said}");
    assert!(said.contains("archive_root: <unset>"), "{said}");
    assert!(!said.contains("locks:"), "{said}");
    let checked = std::process::Command::new(assert_cmd::cargo::cargo_bin("zt"))
        .args(["status", "--check-locks"])
        .env("ZT_CONFIG_DIR", unset.path())
        .env("ZT_GUI_PORT", "0")
        .output()
        .expect("zt status --check-locks without an archive");
    assert_eq!(checked.status.code(), Some(1));
    let said = String::from_utf8_lossy(&checked.stdout);
    assert!(said.contains("archive_root: <unset>"), "{said}");
    assert!(!said.contains("locks:"), "{said}");
    assert!(!said.contains("integrity: ok"), "{said}");
}

/// R231/S08-AC01: the line Session's Card view marks the held Card with `[locked]` and/or
/// `[pinned]`, the marks clear when navigation leaves that state, and ordinary navigation stays
/// available. The marks name the held Card, never the tree it came from.
#[test]
fn the_line_session_marks_the_held_card_locked_or_pinned() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let outside_root = topic(&mut mcp, "Outside");
    let target = card(&mut mcp, &outside_root, "Target", "target body");
    mcp.ok(
        "edit_card",
        json!({ "at": first, "body": format!("links [[{target}]]") }),
    );
    gui.lock(&root, None);
    let output = home
        .cmd()
        .write_stdin(format!(
            "go {root}\ngo {first}\ngo {target}\ngo {outside_root}\nup\nstatus\nq\n"
        ))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let said = String::from_utf8(output).expect("session output is UTF-8");
    assert!(
        said.contains(&format!("location: {root} [locked]")),
        "the Locked tree's root is marked: {said}"
    );
    assert!(
        said.contains(&format!("location: {first} [locked]")),
        "a member is marked: {said}"
    );
    assert!(
        said.contains(&format!("location: {target} [pinned]")),
        "a pinned target is marked, and is not called locked: {said}"
    );
    // Navigating outside the promise clears the marks, so they describe the held Card only.
    assert!(
        said.contains(&format!("location: {outside_root}\n")),
        "an unlocked Card carries no mark: {said}"
    );
    assert!(
        !said.contains(&format!("location: {outside_root} [")),
        "no mark survives navigation: {said}"
    );
    // The Session's own status names the same promise the shell reports (R044, S08-AC04).
    assert!(
        said.contains("locks: 1 tree, 2 cards, 1 pinned target; integrity ok"),
        "the Session status reports the promise: {said}"
    );
}

/// R231/S08-AC01: the TUI renders the same facts, through the same Card view and against the real
/// terminal protocol.
#[test]
fn the_tui_marks_the_held_card_locked_or_pinned() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    gui.lock(&root, None);
    let program = assert_cmd::cargo::cargo_bin("zt");
    let mut session = support::pty::PtySession::spawn(&program, &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text(&format!("go {first}"));
    session.send_enter();
    session.wait_for_text(&format!("location: {first} [locked]"));
    session.send_text(&format!("go {root}"));
    session.send_enter();
    session.wait_for_text(&format!("location: {root} [locked]"));
    session.send_text("q");
    session.send_enter();
    session.wait_for_exit();
}

/// R232/S08-AC02/S08-AC03: the Session refuses every locked write before it opens an editor or asks
/// for confirmation, names the Locked tree and the GUI unlock, and leaves the archive as it was. The
/// metadata setters write with no editor, so their refusal is the transaction-time guard itself.
#[test]
fn the_session_refuses_locked_writes_before_any_editor_or_confirmation() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let outside_root = topic(&mut mcp, "Outside");
    let movable = card(&mut mcp, &outside_root, "Movable", "movable body");
    let lock = gui.lock(&root, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let before = stored_texts(&home);
    let members_before = {
        let conn = home.db();
        member_rows(&conn, lock_id)
    };

    let output = home
        .cmd()
        .write_stdin(format!(
            "go {first}\ne\nn\nb\ndel\nins\nmv before {movable}\nty event\nlv concept\nmf theorem\nq\n"
        ))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let said = String::from_utf8(output).expect("session output is UTF-8");
    // The owner's refusal names the exact Card, its Locked tree and the only way out (R217, R232).
    let refusal = format!("card `{first}` is in locked tree `{root}` (lock {lock_id}); unlock the tree in the GUI first");
    assert_eq!(said.matches(&refusal).count(), 7, "{said}");
    // A new Card would land inside the Locked tree, and a Locked tree takes none.
    let next_direct = format!("{}/2", tree_of(&first));
    assert!(
        said.contains(&format!(
            "`{next_direct}` is inside locked tree `{root}` (lock {lock_id})"
        )),
        "{said}"
    );
    assert!(
        said.contains(&format!("`{first}|a` is inside locked tree `{root}`")),
        "{said}"
    );
    // No editor opened and no confirmation was asked, so no work was typed away and lost.
    assert!(!said.contains("edit mode"), "no editor opened: {said}");
    assert!(!said.contains("to confirm"), "no prompt opened: {said}");
    // Nothing was written: every Card text and every member baseline is unchanged.
    assert_eq!(stored_texts(&home), before);
    let conn = home.db();
    assert_eq!(member_rows(&conn, lock_id), members_before);
    drop(conn);

    // After the owner unlocks in the GUI the same work is available again.
    gui.unlock(lock_id);
    let output = home
        .cmd()
        .write_stdin(format!(
            "go {first}\nn\n# New\n\nnew body\n\n<!-- zt:reverse-links -->\n\x13\nq\n"
        ))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let said = String::from_utf8(output).expect("session output is UTF-8");
    assert!(!said.contains("locked tree"), "{said}");
    assert!(said.contains("edit mode"), "the editor opened: {said}");
    assert!(
        said.contains(&format!("location: {next_direct}")),
        "the new Card was created: {said}"
    );
}

/// R232/S08-AC02: a Session whose read view was drawn before a Lock still refuses the later write, so
/// a stale view cannot carry an edit into a frozen tree. The transaction-time guard covers the
/// commands that write with no editor at all.
#[test]
fn a_session_view_drawn_before_a_lock_refuses_the_later_write() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let before = stored_texts(&home);
    let program = assert_cmd::cargo::cargo_bin("zt");
    let mut session = support::pty::PtySession::spawn(&program, &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text(&format!("go {first}"));
    session.send_enter();
    // The view names the Card and no mark: the tree is not locked yet.
    session.wait_for_text(&format!("location: {first}\n"));
    let lock = gui.lock(&root, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    // The editor never opens over the Card the later Lock froze.
    session.send_text("e");
    session.send_enter();
    session.wait_for_text(&format!("is in locked tree `{root}` (lock {lock_id})"));
    // A setter that needs no editor is refused by the transaction-time guard itself.
    session.send_text("ty event");
    session.send_enter();
    session.wait_for_text_count(&format!("is in locked tree `{root}` (lock {lock_id})"), 2);
    session.send_text("q");
    session.send_enter();
    session.wait_for_exit();
    let said = session.plain_output();
    assert!(
        !said.contains("edit mode"),
        "the editor never opened over a Locked Card: {said}"
    );
    assert_eq!(stored_texts(&home), before, "the refused commands changed no Card");
}

/// R295/S06-AC06: a run that cannot persist its evidence says so and exits 1; it never reports a
/// healthy archive it could not record.
#[cfg(windows)]
#[test]
fn a_check_locks_run_that_cannot_record_says_so() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    card(&mut mcp, &root, "First", "first body");
    gui.lock(&root, None);
    home.db()
        .execute_batch("DROP TRIGGER lock_no_delete")
        .expect("drop the trigger");
    // A live edit-lock holder: the command cannot take the writer lock, so it cannot record.
    let path = home.archive_root.join("zt.edit.lock");
    fs::write(&path, "zt edit lock\npid 999999\n").expect("write the lock file");
    let holder = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .expect("open the live lock file");
    holder.lock().expect("hold the live lock");
    let output = home.cmd().args(["status", "--check-locks"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(said.contains("recording: failed"), "{said}");
    assert!(!said.contains("integrity: ok"), "{said}");
    holder.unlock().expect("release the live lock");
    drop(holder);
    assert!(path.exists(), "a live holder's lock file is kept");
}

/// R290/R296/R297/S06-AC07: a protection trigger lost before startup is recorded and latched
/// before the canonical one is reinstalled, so the evidence survives the repair.
#[test]
fn a_service_start_records_a_lost_trigger_before_repairing_it() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    card(&mut mcp, &root, "First", "first body");
    let lock = gui.lock(&root, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let canonical: String = home
        .db()
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'lock_no_change'",
            [],
            |row| row.get(0),
        )
        .expect("the canonical trigger");
    home.db()
        .execute_batch("DROP TRIGGER lock_no_change")
        .expect("lose the trigger");

    home.restart();

    let gui = home.gui();
    let panel = gui.panel();
    assert_eq!(
        panel["integrity"]["faults"],
        json!([]),
        "the guarded startup restored the backstop: {panel}"
    );
    let latched = panel["integrity"]["latched"].as_array().expect("latched");
    assert_eq!(latched.len(), 1, "{panel}");
    assert_eq!(latched[0]["lock_id"], json!(lock_id));
    assert_eq!(latched[0]["kind"], "trigger-missing");
    assert_eq!(latched[0]["card"], json!("lock_no_change"));
    assert!(
        latched[0]["at"].as_str().is_some_and(|at| at.contains('T')),
        "the latch carries when it was observed"
    );
    let listed = panel["locks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["lock_id"] == json!(lock_id))
        .expect("the Lock");
    assert_eq!(listed["seal_intact"], false, "a latched fault breaks the seal");
    assert_eq!(listed["seal"], lock["seal"]);
    assert_eq!(
        fault_events(&panel, lock_id).len(),
        1,
        "the loss was recorded once, by the startup scan"
    );
    let restored: String = home
        .db()
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'lock_no_change'",
            [],
            |row| row.get(0),
        )
        .expect("the restored trigger");
    assert_eq!(
        restored, canonical,
        "the canonical definition is back, not an approximation"
    );
    // The restored backstop really refuses a write again.
    let refused = home
        .db()
        .execute(
            "UPDATE cards SET text = '# First\n\nrewritten\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
            [first_address(&root)],
        )
        .expect_err("the trigger protects again");
    assert!(refused.to_string().contains("locked tree"), "{refused}");
    // The restarted service still refuses through its own path, so the latch is not the only guard.
    let mut mcp = home.mcp();
    let refused = mcp.refused(
        "edit_card",
        json!({ "at": first_address(&root), "body": "x" }),
    );
    assert_eq!(refused["code"], "locked", "{refused}");
    // The startup line is in the log, so an operator sees what happened.
    let log = fs::read_to_string(home.archive_root.join("zt.log")).unwrap_or_default();
    assert!(log.contains("tree lock startup:"), "{log}");
    assert!(log.contains("lock_no_change"), "{log}");
}

/// R298/S06-AC07: a damaged baseline is reported and refuses writable startup; the product never
/// fabricates an empty snapshot or a new seal to hide it.
#[test]
fn a_damaged_lock_baseline_refuses_writable_startup() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    card(&mut mcp, &root, "First", "first body");
    let lock = gui.lock(&root, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    home.cmd().arg("down").assert().success();
    // Outside tampering: the baseline loses the column that holds each member's fingerprint.
    home.db()
        .execute_batch("ALTER TABLE lock_members DROP COLUMN authored_fp")
        .expect("damage the baseline");
    let up = home.cmd().arg("up").output().expect("run zt up");
    assert!(
        !up.status.success(),
        "a damaged baseline refuses writable startup: {}",
        String::from_utf8_lossy(&up.stdout)
    );
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&up.stdout),
        String::from_utf8_lossy(&up.stderr)
    );
    assert!(said.contains("lock_members"), "{said}");
    assert!(said.contains("unsupported column set"), "{said}");
    assert!(said.contains("archive was not modified"), "{said}");
    // Nothing fabricated a snapshot or a new seal, and the retained Lock record still names its
    // promise, so an owner can read it and restore a consistent backup.
    let (members, seal): (i64, String) = home
        .db()
        .query_row(
            "SELECT (SELECT COUNT(*) FROM lock_members), range_fingerprint FROM locks WHERE lock_id = ?1",
            [lock_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("the retained Lock");
    assert_eq!(members, 2, "the snapshot rows are still there");
    assert_eq!(json!(seal), lock["seal"], "the reviewed seal is unchanged");
}

// --- S07 (#130): first installation, generation validation and Card rebuilds -----------------

/// The six canonical trigger statements and the `cards` unique index, so a hand rebuild can put the
/// archive back the way the previous release left it.
fn protection_sql(conn: &Connection) -> (Vec<String>, String) {
    let mut triggers = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name IN
             ('lock_no_delete','lock_no_change','lock_no_insert','lock_no_move_in',
              'lock_pin_no_delete','lock_pin_no_move') ORDER BY name",
        )
        .expect("trigger catalog");
    for sql in stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("trigger rows")
    {
        triggers.push(sql.expect("trigger sql"));
    }
    assert_eq!(triggers.len(), 6, "every canonical trigger exists");
    let index: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'index'
               AND name = 'cards_citation_key_nocase'",
            [],
            |row| row.get(0),
        )
        .expect("the citation index");
    (triggers, index)
}

/// Replace the `cards` table with the previous release's definition: the same columns and rules but
/// no Finding Card type. Every row keeps its row id, and the canonical protection is re-created the
/// way that release left it, so the next boot performs an authorized Card rebuild (R051, R304).
fn previous_release_cards(conn: &Connection) {
    let (triggers, index) = protection_sql(conn);
    let definition: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
            [],
            |row| row.get(0),
        )
        .expect("the cards table");
    let previous = definition
        .replacen("cards", "cards_previous", 1)
        .replace("'finding', ", "");
    assert!(!previous.contains("'finding'"), "{previous}");
    // The stored definition carries no trailing semicolon of its own.
    let previous = format!("{};", previous.trim_end_matches(';'));
    let columns = "row_id, location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis, model_level, hub_level, math_form";
    conn.execute_batch(&format!(
        "BEGIN IMMEDIATE;
         {previous}
         INSERT INTO cards_previous ({columns}) SELECT {columns} FROM cards;
         DROP TABLE cards;
         ALTER TABLE cards_previous RENAME TO cards;
         {index};
         COMMIT;"
    ))
    .expect("rebuild the cards table into the previous release's definition");
    for sql in &triggers {
        conn.execute_batch(sql).expect("re-create a canonical trigger");
    }
    let triggers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger'",
            [],
            |row| row.get(0),
        )
        .expect("triggers");
    assert_eq!(triggers, 6, "the previous release left its protection in place");
}

/// Erase the whole Tree Lock schema, the way the archive before this feature looked.
fn erase_lock_schema(conn: &Connection) {
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS lock_no_delete;
         DROP TRIGGER IF EXISTS lock_no_change;
         DROP TRIGGER IF EXISTS lock_no_insert;
         DROP TRIGGER IF EXISTS lock_no_move_in;
         DROP TRIGGER IF EXISTS lock_pin_no_delete;
         DROP TRIGGER IF EXISTS lock_pin_no_move;
         DROP INDEX IF EXISTS locks_active_tree;
         DROP INDEX IF EXISTS lock_members_by_row;
         DROP INDEX IF EXISTS lock_pins_by_target;
         DROP INDEX IF EXISTS lock_events_by_lock;
         DROP TABLE IF EXISTS lock_events;
         DROP TABLE IF EXISTS lock_pins;
         DROP TABLE IF EXISTS lock_members;
         DROP TABLE IF EXISTS locks;
         DELETE FROM metadata WHERE key = 'tree_lock_schema_version';",
    )
    .expect("erase the Tree Lock schema");
}

/// Every Card's row identity and address, in row order.
fn card_rows(conn: &Connection) -> Vec<(i64, String)> {
    let mut stmt = conn
        .prepare("SELECT row_id, COALESCE(location, citation_key) FROM cards ORDER BY row_id")
        .expect("cards");
    stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("rows")
        .collect::<std::result::Result<Vec<_>, _>>()
        .expect("card rows")
}

/// The stored member baselines of one Lock.
fn member_rows(conn: &Connection, lock_id: i64) -> Vec<(i64, String, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT row_id, address, authored_fp FROM lock_members
              WHERE lock_id = ?1 ORDER BY address",
        )
        .expect("members");
    stmt.query_map([lock_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })
    .expect("member rows")
    .collect::<std::result::Result<Vec<_>, _>>()
    .expect("members")
}

/// The `zt.sqlite3.pre-tree-lock-<s>.bak` files of one archive root.
fn tree_lock_backups(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut names = fs::read_dir(root)
        .expect("archive root")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("zt.sqlite3.pre-tree-lock-") && name.ends_with(".bak")
                })
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn lock_schema_version(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT value FROM metadata WHERE key = 'tree_lock_schema_version'",
        [],
        |row| row.get(0),
    )
    .ok()
}

/// R302/S07-AC01/AC09: the first upgrade of an archive that has no lock schema backs it up through
/// SQLite, installs version 1 with all six triggers, keeps every Card row identity, and installs
/// nothing on the next start.
#[test]
fn a_first_upgrade_backs_the_archive_up_and_installs_protection_once() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    card(&mut mcp, &root, "First", "first body");
    home.cmd().arg("down").assert().success();
    // The archive the previous release left: Cards and their history, no Tree Lock at all.
    let rows_before = {
        let conn = home.db();
        erase_lock_schema(&conn);
        assert_eq!(lock_schema_version(&conn), None);
        card_rows(&conn)
    };
    assert_eq!(rows_before.len(), 2, "the Topic root and one Card");

    home.cmd().arg("up").assert().success();

    let backups = tree_lock_backups(&home.archive_root);
    assert_eq!(backups.len(), 1, "one backup per first installation: {backups:?}");
    // The backup is the archive before the installation, taken through SQLite.
    let saved =
        Connection::open_with_flags(&backups[0], rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("open the backup read-only");
    let integrity: String = saved
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .expect("integrity check");
    assert_eq!(integrity, "ok", "the backup is a consistent SQLite database");
    assert_eq!(card_rows(&saved), rows_before, "the backup holds every prior Card");
    let saved_locks: bool = saved
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = 'locks')",
            [],
            |row| row.get(0),
        )
        .expect("lock table");
    assert!(!saved_locks, "the backup is the archive before the installation");
    // The archive itself is installed, with every row identity intact.
    let installed = home.db();
    assert_eq!(lock_schema_version(&installed).as_deref(), Some("1"));
    let triggers: i64 = installed
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name IN
             ('lock_no_delete','lock_no_change','lock_no_insert','lock_no_move_in',
              'lock_pin_no_delete','lock_pin_no_move')",
            [],
            |row| row.get(0),
        )
        .expect("triggers");
    assert_eq!(triggers, 6);
    assert_eq!(card_rows(&installed), rows_before, "the installation kept every row id");
    let log = fs::read_to_string(home.archive_root.join("zt.log")).unwrap_or_default();
    assert!(
        log.contains("installing tree lock protection; backup"),
        "the operator sees which backup the upgrade took: {log}"
    );
    // A second start installs nothing and takes no second backup.
    drop(installed);
    home.restart();
    assert_eq!(
        tree_lock_backups(&home.archive_root).len(),
        1,
        "only the first installation backs the archive up"
    );
    assert_eq!(card_rows(&home.db()), rows_before);
}

/// R303/S07-AC02/AC04: an archive that declares an unsupported lock generation refuses writable
/// startup before any Card migration, and keeps its promise, its rows and the marker as they are.
#[test]
fn an_unsupported_lock_generation_refuses_startup_before_a_card_migration() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let lock = gui.lock(&root, None);
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    home.cmd().arg("down").assert().success();
    let (rows_before, members_before) = {
        let conn = home.db();
        // The previous release's Card table, and a generation this binary does not know.
        previous_release_cards(&conn);
        conn.execute(
            "UPDATE metadata SET value = '2' WHERE key = 'tree_lock_schema_version'",
            [],
        )
        .expect("mark an unsupported generation");
        (card_rows(&conn), member_rows(&conn, lock_id))
    };
    assert!(!members_before.is_empty());

    let up = home.cmd().arg("up").output().expect("run zt up");
    assert!(
        !up.status.success(),
        "an unsupported generation refuses writable startup: {}",
        String::from_utf8_lossy(&up.stdout)
    );
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&up.stdout),
        String::from_utf8_lossy(&up.stderr)
    );
    assert!(
        said.contains("unsupported Tree Lock schema version `2`"),
        "{said}"
    );
    assert!(said.contains("archive was not modified"), "{said}");
    // No Card migration ran before the refusal, so the old Card table is still there.
    let conn = home.db();
    let definition: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
            [],
            |row| row.get(0),
        )
        .expect("the Card table");
    assert!(
        !definition.contains("'finding'"),
        "the Card migration must not run before the generation is accepted: {definition}"
    );
    assert_eq!(lock_schema_version(&conn).as_deref(), Some("2"));
    assert_eq!(card_rows(&conn), rows_before, "every Card row identity is untouched");
    assert_eq!(
        member_rows(&conn, lock_id),
        members_before,
        "the promise is retained, never repaired"
    );
    let seal: String = conn
        .query_row("SELECT range_fingerprint FROM locks WHERE lock_id = ?1", [lock_id], |row| {
            row.get(0)
        })
        .expect("the retained Lock");
    assert_eq!(json!(seal), lock["seal"], "the reviewed seal is unchanged");
    drop(conn);
    // No service is up, so every write path is refused.
    let status = home.cmd().arg("status").output().expect("zt status");
    assert!(
        String::from_utf8_lossy(&status.stdout).contains("state: down"),
        "a refused startup leaves no service: {}",
        String::from_utf8_lossy(&status.stdout)
    );
    let _ = first;
}

/// R051/R304/S07-AC05: an authorized Card rebuild keeps every row identity and all Lock evidence,
/// reinstalls the six canonical triggers without recording a fault, and leaves the tree protected.
#[test]
fn an_authorized_card_rebuild_keeps_the_promise_and_its_protection() {
    let home = TestHome::new();
    let gui = home.gui();
    let mut mcp = home.mcp();
    let root = topic(&mut mcp, "Frozen");
    let first = card(&mut mcp, &root, "First", "first body");
    let lock = gui.lock(&root, Some("survives a rebuild"));
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    home.cmd().arg("down").assert().success();
    let (rows_before, members_before) = {
        let conn = home.db();
        previous_release_cards(&conn);
        (card_rows(&conn), member_rows(&conn, lock_id))
    };
    let events_before = {
        let conn = home.db();
        let events: i64 = conn
            .query_row("SELECT COUNT(*) FROM lock_events WHERE lock_id = ?1", [lock_id], |row| {
                row.get(0)
            })
            .expect("events");
        events
    };

    home.cmd().arg("up").assert().success();

    let gui = home.gui();
    let panel = gui.panel();
    // The rebuild happened, and it is not tampering: the guarded comparison is healthy.
    let definition: String = home
        .db()
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
            [],
            |row| row.get(0),
        )
        .expect("the rebuilt table");
    assert!(definition.contains("'finding'"), "{definition}");
    assert_eq!(
        panel["integrity"]["faults"],
        json!([]),
        "an authorized rebuild records no current fault: {panel}"
    );
    assert_eq!(
        panel["integrity"]["latched"],
        json!([]),
        "the rebuild's own trigger loss is not evidence of tampering: {panel}"
    );
    assert_eq!(panel["integrity"]["ok"], true);
    let listed = panel["locks"]
        .as_array()
        .expect("locks")
        .iter()
        .find(|item| item["lock_id"] == json!(lock_id))
        .expect("the Lock");
    assert_eq!(listed["seal_intact"], true);
    assert_eq!(listed["seal"], lock["seal"], "the reviewed seal survives");
    assert_eq!(
        fault_events(&panel, lock_id).len(),
        0,
        "no integrity-fault event for an authorized rebuild"
    );
    // Every row identity, member baseline and history event survived, and the six triggers are back.
    let conn = home.db();
    assert_eq!(card_rows(&conn), rows_before, "every row id is preserved");
    assert_eq!(member_rows(&conn, lock_id), members_before);
    let triggers: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name IN
             ('lock_no_delete','lock_no_change','lock_no_insert','lock_no_move_in',
              'lock_pin_no_delete','lock_pin_no_move')",
            [],
            |row| row.get(0),
        )
        .expect("triggers");
    assert_eq!(triggers, 6, "the six canonical triggers are reinstalled");
    let events_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM lock_events WHERE lock_id = ?1", [lock_id], |row| {
            row.get(0)
        })
        .expect("events");
    assert_eq!(events_after, events_before, "the history is unchanged");
    // The reinstalled backstop protects the rebuilt table again.
    let refused = conn
        .execute(
            "UPDATE cards SET text = '# First\n\nrewritten\n\n<!-- zt:reverse-links -->\n' WHERE location = ?1",
            [&first],
        )
        .expect_err("the reinstalled trigger protects the rebuilt row");
    assert!(refused.to_string().contains("locked tree"), "{refused}");
    drop(conn);
    // The row ids really are the same Cards: the service still refuses the member's write.
    let mut mcp = home.mcp();
    let refusal = mcp.refused("edit_card", json!({ "at": first, "body": "x" }));
    assert_eq!(refusal["code"], "locked", "{refusal}");
    // `zt status --check-locks` agrees with the panel and exits 0.
    let checked = home
        .cmd()
        .args(["status", "--check-locks"])
        .output()
        .expect("zt status --check-locks");
    assert_eq!(checked.status.code(), Some(0));
    let said = String::from_utf8_lossy(&checked.stdout);
    assert!(said.contains("integrity: ok"), "{said}");
    assert!(!said.contains("latched:"), "{said}");
    // A second start rebuilds nothing and records nothing.
    home.restart();
    let gui = home.gui();
    let panel = gui.panel();
    assert_eq!(panel["integrity"]["ok"], true);
    assert_eq!(fault_events(&panel, lock_id).len(), 0);
    let conn = home.db();
    let definition_again: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
            [],
            |row| row.get(0),
        )
        .expect("the table");
    assert_eq!(definition_again, definition, "the second start rebuilds nothing");
    assert_eq!(card_rows(&conn), rows_before);
    gui.unlock(lock_id);
}

