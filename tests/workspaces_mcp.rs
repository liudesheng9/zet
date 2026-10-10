//! Workspaces and the `zt mcp` server: two Workspaces with Topic and
//! Literature Zones, Links across them, no moves between them, and an MCP
//! client that may read both but change only the agent Workspace.

use assert_cmd::Command;
use predicates::prelude::*;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
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
        Self {
            _tmp: tmp,
            config_dir,
            archive_root,
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir);
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

    fn workspace(&self, address: &str) -> String {
        self.db()
            .query_row(
                "SELECT workspace FROM cards WHERE location = ?1 OR citation_key = ?1",
                [address],
                |row| row.get(0),
            )
            .expect("card workspace")
    }

    fn exists(&self, address: &str) -> bool {
        self.db()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM cards WHERE location = ?1 OR citation_key = ?1)",
                [address],
                |row| row.get::<_, i64>(0),
            )
            .expect("exists")
            == 1
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
        mcp.notify("notifications/initialized");
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

    fn notify(&mut self, method: &str) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method }));
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
        assert!(
            result["content"][0]["text"].is_string(),
            "tool results carry text content: {reply}"
        );
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

#[test]
fn shell_and_session_create_trees_in_the_named_workspace_and_root_shows_its_zones() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    home.cmd_with_editor("# Agent Topic\n\ntheirs\n")
        .args(["t", "--ws", "agent", "Agent Topic"])
        .assert()
        .success()
        .stdout("1/0\n");
    home.cmd_with_editor("# Agent Card\n\nsee [[0/1]]\n")
        .args(["n", "--at", "1/0"])
        .assert()
        .success()
        .stdout("1/1\n");
    home.cmd()
        .args(["t", "--ws", "shared", "Nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown workspace `shared`"));

    assert_eq!(home.workspace("0/0"), "human");
    assert_eq!(home.workspace("0/1"), "human");
    assert_eq!(home.workspace("1/0"), "agent");
    assert_eq!(
        home.workspace("1/1"),
        "agent",
        "successors join their tree's workspace"
    );
    assert!(
        home.text("0/1").contains("- [[1/1]] Agent Card"),
        "Links cross workspaces and generate Reverse links"
    );

    home.cmd().arg("ws").assert().success().stdout(
        "human (Human): 1 topic trees, 0 literature trees, 2 cards; MCP access: read-only\n\
         agent (AI agent): 1 topic trees, 0 literature trees, 2 cards; MCP access: read-write\n",
    );

    let output = home
        .cmd()
        .write_stdin(
            concat!(
                "ws agent\n",
                "ls\n",
                "t Session Agent Topic\n",
                "# Session Agent Topic\n\nfrom the session\n",
                "\x13",
                "go 0/1\n",
                "go\n",
                "ws\n",
                "q\n"
            )
            .as_bytes(),
        )
        .output()
        .expect("line session");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(
        "ROOT\nworkspace: human (switch with ws agent)\ntopics:\n0/0 Human Topic\nliterature:\n"
    ));
    assert!(stdout.contains(
        "ROOT\nworkspace: agent (switch with ws human)\ntopics:\n1/0 Agent Topic\nliterature:\n"
    ));
    assert!(stdout.contains("agent workspace topics and literature: 1\n1/0 Agent Topic\n"));
    assert!(
        stdout.contains("location: 0/1\n# Human Card"),
        "go follows Links into the other workspace"
    );
    assert!(
        stdout.contains(
            "ROOT\nworkspace: human (switch with ws agent)\ntopics:\n0/0 Human Topic\nliterature:\n"
        ),
        "ROOT follows the Pointer's workspace"
    );
    assert!(stdout.contains("* human (Human): 1 topic trees"));
    assert!(stdout.contains("  agent (AI agent): 2 topic trees"));
    assert_eq!(
        home.workspace("2/0"),
        "agent",
        "t creates in the current workspace"
    );
}

#[test]
fn cards_never_move_between_workspaces_even_for_the_owner() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    home.cmd_with_editor("# Agent Topic\n\ntheirs\n")
        .args(["t", "--ws", "agent", "Agent Topic"])
        .assert()
        .success();
    home.cmd_with_editor("# Agent Card\n\nbody\n")
        .args(["n", "--at", "1/0"])
        .assert()
        .success();
    home.cmd_with_editor("# Agent Card 2\n\nbody\n")
        .args(["n", "--at", "1/1"])
        .assert()
        .success();

    home.cmd()
        .args(["mv", "--at", "1/2", "0/2"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cards cannot move between workspaces: `1/2` is in the agent workspace and `0/2` is in the human workspace",
        ));
    home.cmd()
        .args(["mv", "--at", "0/1", "1/1|a"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cards cannot move between workspaces",
        ));
    assert!(home.exists("1/2") && home.exists("0/1"));

    home.cmd()
        .args(["mv", "--at", "1/2", "1/1|a"])
        .write_stdin("move\n")
        .assert()
        .success();
    assert!(home.exists("1/1|a"));
    assert_eq!(home.workspace("1/1|a"), "agent");
}

#[test]
fn up_adds_workspaces_to_an_existing_archive_with_every_card_in_the_human_workspace() {
    let home = TestHome::new();
    home.configure();
    fs::create_dir_all(&home.archive_root).expect("archive root");
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("old db");
    conn.execute_batch(
        "CREATE TABLE cards (
            row_id INTEGER PRIMARY KEY,
            location TEXT UNIQUE,
            citation_key TEXT UNIQUE,
            is_topic INTEGER NOT NULL CHECK(is_topic IN (0, 1)),
            is_lit INTEGER NOT NULL CHECK(is_lit IN (0, 1)),
            bibtex TEXT,
            text TEXT NOT NULL
         );
         CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO metadata VALUES('card_format', 'markdown');
         INSERT INTO metadata VALUES('next_topic_id', '1');
         INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)
           VALUES('0/0', NULL, 1, 0, NULL, '# Old Topic\n\nold\n\n<!-- zt:reverse-links -->\n');
         INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)
           VALUES(NULL, 'Old2020', 0, 1, '@book{Old2020, title={Old}}', '# Old\n\n<!-- zt:reverse-links -->\n');",
    )
    .expect("pre-workspace archive");
    drop(conn);

    home.cmd().arg("up").assert().success();
    assert_eq!(home.workspace("0/0"), "human");
    assert_eq!(home.workspace("Old2020"), "human");
    assert!(
        home.db()
            .execute(
                "UPDATE cards SET workspace = 'shared' WHERE location = '0/0'",
                []
            )
            .is_err(),
        "the schema accepts only the two workspaces"
    );
    home.cmd()
        .arg("ws")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "human (Human): 1 topic trees, 1 literature trees, 2 cards",
        ));
    home.cmd().arg("down").assert().success();
    home.cmd().arg("up").assert().success();
    assert_eq!(home.workspace("0/0"), "human");
}

#[test]
fn mcp_reads_both_workspaces_but_changes_only_the_agent_workspace() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    let human_topic_text = home.text("0/0");
    let mut mcp = home.mcp();

    let tools = mcp.request("tools/list", json!({}));
    let names = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    for expected in [
        "list_workspaces",
        "list_trees",
        "get_tree",
        "get_card",
        "search_cards",
        "list_links",
        "list_broken_links",
        "create_topic",
        "create_literature",
        "create_successor",
        "edit_card",
        "edit_literature_metadata",
        "delete_card",
        "move_card",
    ] {
        assert!(
            names.contains(&expected.to_string()),
            "missing tool {expected}"
        );
    }

    let workspaces = mcp.ok("list_workspaces", json!({}));
    assert_eq!(workspaces["workspaces"][0]["id"], "human");
    assert_eq!(workspaces["workspaces"][0]["access"], "read-only");
    assert_eq!(workspaces["workspaces"][0]["zones"]["topic"]["trees"], 1);
    assert_eq!(workspaces["workspaces"][1]["id"], "agent");
    assert_eq!(workspaces["workspaces"][1]["access"], "read-write");

    // Reads cover the human workspace.
    let card = mcp.ok("get_card", json!({ "at": "0/1" }));
    assert_eq!(
        (
            card["title"].as_str(),
            card["workspace"].as_str(),
            card["writable"].as_bool()
        ),
        (Some("Human Card"), Some("human"), Some(false))
    );
    assert_eq!(card["parent"]["address"], "0/0");

    // Writes to the human workspace are refused and change nothing.
    let read_only = "the human workspace is read-only for AI agents";
    assert!(
        mcp.err(
            "create_topic",
            json!({ "title": "Sneaky", "workspace": "human" })
        )
        .contains(read_only)
    );
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "0/1", "title": "Sneaky" })
        )
        .contains(read_only)
    );
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "0/1", "kind": "side", "title": "Sneaky" })
        )
        .contains(read_only)
    );
    assert!(
        mcp.err("edit_card", json!({ "at": "0/1", "body": "rewritten" }))
            .contains(read_only)
    );
    assert!(
        mcp.err("delete_card", json!({ "at": "0/1", "confirm": "delete" }))
            .contains(read_only)
    );
    assert!(
        mcp.err(
            "move_card",
            json!({ "at": "0/1", "to": "0/0", "confirm": "move" })
        )
        .contains(read_only)
    );
    assert!(home.exists("0/1") && !home.exists("0/2") && !home.exists("0/1|a"));
    assert_eq!(home.text("0/0"), human_topic_text);

    // The agent grows its own Topic tree, linking into the human workspace.
    let topic = mcp.ok(
        "create_topic",
        json!({ "title": "Agent Research", "body": "What the agent is exploring." }),
    );
    assert_eq!(
        (topic["address"].as_str(), topic["workspace"].as_str()),
        (Some("1/0"), Some("agent"))
    );
    let first = mcp.ok(
        "create_successor",
        json!({ "at": "1/0", "title": "Builds on the human", "body": "Extends [[0/1]]." }),
    );
    assert_eq!(first["address"], "1/1");
    let side = mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "kind": "side", "title": "A tangent" }),
    );
    assert_eq!(side["address"], "1/1|a");
    assert_eq!(home.workspace("1/1|a"), "agent");
    assert!(
        home.text("0/1")
            .contains("## Reverse links\n\n- [[1/1]] Builds on the human"),
        "the human Card's generated Reverse links show the agent's Link"
    );
    assert!(home.text("0/1").starts_with("# Human Card\n\nmy idea\n"));

    // Literature Zone.
    let literature = mcp.ok(
        "create_literature",
        json!({ "bibtex": "@article{Agent2026, title = {Agent Paper}}", "body": "Summary." }),
    );
    assert_eq!(literature["address"], "Agent2026");
    assert!(
        mcp.err(
            "create_literature",
            json!({ "bibtex": "@article{Human2026, title = {X}}", "workspace": "human" })
        )
        .contains(read_only)
    );
    assert_eq!(
        mcp.ok(
            "create_successor",
            json!({ "at": "Agent2026", "title": "Note" })
        )["address"],
        "Agent2026/1"
    );

    let trees = mcp.ok("list_trees", json!({ "workspace": "agent" }));
    let roots = trees["trees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tree| {
            (
                tree["root"].as_str().unwrap(),
                tree["kind"].as_str().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(roots, [("1/0", "topic"), ("Agent2026", "literature")]);
    let literature_zone = mcp.ok(
        "list_trees",
        json!({ "workspace": "agent", "zone": "literature" }),
    );
    assert_eq!(literature_zone["trees"].as_array().unwrap().len(), 1);
    let tree = mcp.ok("get_tree", json!({ "tree": "1/1|a" }));
    assert_eq!(
        (
            tree["root"].as_str(),
            tree["zone"].as_str(),
            tree["cards"].as_array().unwrap().len()
        ),
        (Some("1/0"), Some("topic"), 3)
    );

    let hits = mcp.ok("search_cards", json!({ "query": "idea" }));
    assert_eq!(hits["results"][0]["address"], "0/1");
    let hits = mcp.ok(
        "search_cards",
        json!({ "query": "idea", "workspace": "agent" }),
    );
    assert!(hits["results"].as_array().unwrap().is_empty());

    let links = mcp.ok("list_links", json!({ "cross_workspace_only": true }));
    assert_eq!(
        links["links"],
        json!([{
            "source": "1/1",
            "source_workspace": "agent",
            "target": "0/1",
            "target_workspace": "human",
            "broken": false,
        }])
    );

    // Editing the agent's own Cards.
    mcp.ok(
        "edit_card",
        json!({ "at": "1/1", "body": "Extends [[0/1]] and [[Agent2026]]." }),
    );
    assert!(
        home.text("1/1")
            .starts_with("# Builds on the human\n\nExtends [[0/1]] and [[Agent2026]].\n")
    );
    mcp.ok(
        "edit_card",
        json!({ "at": "1/1|a", "title": "A better tangent" }),
    );
    assert!(home.text("1/1|a").starts_with("# A better tangent\n"));
    assert!(
        mcp.err("edit_card", json!({ "at": "1/1", "text": "no heading" }))
            .contains("`# Title` heading")
    );
    assert!(
        mcp.err(
            "edit_card",
            json!({ "at": "1/1", "body": "x", "base": "stale text" })
        )
        .contains("changed since it was opened")
    );

    // Moves stay inside the agent workspace.
    assert!(
        mcp.err(
            "move_card",
            json!({ "at": "1/1|a", "to": "0/2", "confirm": "move" })
        )
        .contains("Cards cannot move between workspaces")
    );
    let plan = mcp.ok("move_card", json!({ "at": "1/1|a", "to": "1/2" }));
    assert_eq!(
        (plan["applied"].as_bool(), plan["confirm"].as_str()),
        (Some(false), Some("move"))
    );
    assert!(home.exists("1/1|a") && !home.exists("1/2"));
    let moved = mcp.ok(
        "move_card",
        json!({ "at": "1/1|a", "to": "1/2", "confirm": "move" }),
    );
    assert_eq!(moved["applied"], true);
    assert!(home.exists("1/2") && !home.exists("1/1|a"));

    // Renaming the agent's Citation key rewrites the agent's Links to it.
    let plan = mcp.ok(
        "edit_literature_metadata",
        json!({ "at": "Agent2026", "bibtex": "@article{AgentPaper2026, title = {Agent Paper}}" }),
    );
    assert_eq!(plan["confirm"], "move");
    let renamed = mcp.ok(
        "edit_literature_metadata",
        json!({
            "at": "Agent2026",
            "bibtex": "@article{AgentPaper2026, title = {Agent Paper}}",
            "confirm": "move",
        }),
    );
    assert_eq!(renamed["address"], "AgentPaper2026");
    assert!(home.exists("AgentPaper2026/1"));
    assert!(home.text("1/1").contains("[[AgentPaper2026]]"));

    // Deleting needs the confirmation the plan names.
    let plan = mcp.ok("delete_card", json!({ "at": "1/0" }));
    assert_eq!(plan["confirm"], "1/0");
    assert!(mcp.ok("delete_card", json!({ "at": "1/0", "confirm": "delete" }))["applied"] == false);
    assert!(home.exists("1/0"));
    let deleted = mcp.ok("delete_card", json!({ "at": "1/0", "confirm": "1/0" }));
    assert_eq!(deleted["deleted"], 3);
    assert!(!home.exists("1/0") && !home.exists("1/1") && !home.exists("1/2"));
    assert!(home.exists("0/1"), "the human tree is untouched");
    assert!(
        !home.text("0/1").contains("Reverse links"),
        "the agent's Link is gone from the human Card's Reverse links"
    );
    assert!(
        mcp.ok("list_broken_links", json!({}))["broken"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn mcp_refuses_agent_changes_that_would_rewrite_links_in_human_cards() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({ "title": "Agent" }));
    mcp.ok("create_successor", json!({ "at": "1/0", "title": "Base" }));
    mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "kind": "side", "title": "Side A" }),
    );
    mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "kind": "side", "title": "Side B" }),
    );
    mcp.ok("create_successor", json!({ "at": "1/1", "title": "Next" }));
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{AgentBook, title = {Agent Book}}" }),
    );
    // The human links to agent Cards from their own tree.
    home.cmd_with_editor("# Human Notes\n\nSee [[1/1|b]], [[1/2]], and [[AgentBook]].\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success()
        .stdout("0/2\n");
    let human_text = home.text("0/2");

    let refused = "which is read-only for AI agents";
    assert!(
        mcp.err(
            "move_card",
            json!({ "at": "1/2", "to": "1/1|a|1", "confirm": "move" })
        )
        .contains(&format!(
            "rewrite Links in human workspace Card `0/2`, {refused}"
        ))
    );
    assert!(
        mcp.err("delete_card", json!({ "at": "1/1|a", "confirm": "delete" }))
            .contains(refused),
        "compacting 1/1|b to 1/1|a would rewrite the human Link"
    );
    assert!(
        mcp.err(
            "edit_literature_metadata",
            json!({ "at": "AgentBook", "bibtex": "@book{AgentBook2, title = {Agent Book}}", "confirm": "move" })
        )
        .contains(refused)
    );
    assert_eq!(home.text("0/2"), human_text);
    assert!(home.exists("1/1|a") && home.exists("1/1|b") && home.exists("1/2"));
    assert!(home.exists("AgentBook"));

    // Deleting a linked agent Card leaves a Broken link without editing the human body.
    let deleted = mcp.ok("delete_card", json!({ "at": "1/2", "confirm": "delete" }));
    assert_eq!(deleted["deleted"], 1);
    assert!(
        home.text("0/2")
            .starts_with("# Human Notes\n\nSee [[1/1|b]], [[1/2]], and [[AgentBook]].\n")
    );
    let broken = mcp.ok("list_broken_links", json!({ "workspace": "human" }));
    assert_eq!(broken["broken"][0]["target"], "1/2");
    assert_eq!(broken["broken"][0]["workspace"], "human");

    // The owner may still do what the agent may not.
    home.cmd()
        .args(["del", "--at", "1/1|a"])
        .write_stdin("delete\n")
        .assert()
        .success();
    assert!(
        home.text("0/2").contains("[[1/1|a]]"),
        "owner deletes compact and rewrite Links"
    );
}

#[test]
fn mcp_protocol_errors_and_service_state_are_reported() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    assert_eq!(mcp.request("ping", json!({}))["result"], json!({}));
    assert_eq!(
        mcp.request("bogus/method", json!({}))["error"]["code"],
        -32601
    );
    assert_eq!(
        mcp.request(
            "tools/call",
            json!({ "name": "no_such_tool", "arguments": {} })
        )["error"]["code"],
        -32602
    );
    assert!(
        mcp.err("get_card", json!({ "at": "9/9" }))
            .contains("card `9/9` does not exist")
    );
    assert!(
        mcp.err("create_topic", json!({ "title": "two\nlines" }))
            .contains("single-line")
    );
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "0/0", "kind": "sideways", "title": "x" })
        )
        .contains("unknown successor kind `sideways`")
    );

    home.cmd().arg("down").assert().success();
    assert!(
        mcp.err("list_workspaces", json!({}))
            .contains("the ZT service is not available; run `zt up`")
    );
    home.cmd().arg("up").assert().success();
    assert_eq!(mcp.ok("list_workspaces", json!({}))["cards"], 0);
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
fn gui_creates_trees_in_either_workspace_and_reports_workspaces() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut gui = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .args(["gui", "--no-open", "--port", "0"])
        .env("ZT_CONFIG_DIR", &home.config_dir)
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
    assert!(page.contains("id=\"workspace-switch\""));
    let token = page
        .split("name=\"zt-token\" content=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("token")
        .to_string();
    let api = |method: &str, path: &str, body: Option<&str>| {
        http_request(port, method, path, &token, body)
    };

    assert_eq!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"topic","text":"# Mine\n"}"##)
        )["address"],
        "0/0"
    );
    assert_eq!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"topic","workspace":"agent","text":"# Theirs\n\nsee [[0/0]]\n"}"##)
        )["address"],
        "1/0"
    );
    assert_eq!(
        api(
            "POST",
            "/api/literature",
            Some(
                r##"{"workspace":"agent","bibtex":"@book{GuiBook, title={Gui Book}}","text":"notes"}"##
            )
        )["address"],
        "GuiBook"
    );
    assert!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"topic","workspace":"nowhere","text":"# X\n"}"##)
        )["error"]
            .as_str()
            .unwrap()
            .contains("unknown workspace")
    );
    // The GUI is the owner's tool: it may edit the agent workspace too.
    assert_eq!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"direct","at":"1/0","text":"# Owner note\n"}"##)
        )["address"],
        "1/1"
    );
    assert_eq!(home.workspace("1/1"), "agent");

    let overview = api("GET", "/api/overview", None);
    assert_eq!(overview["workspaces"][0]["id"], "human");
    assert_eq!(overview["workspaces"][0]["agent_writable"], false);
    assert_eq!(overview["workspaces"][1]["id"], "agent");
    assert_eq!(overview["workspaces"][1]["topic_trees"], 1);
    assert_eq!(overview["workspaces"][1]["literature_trees"], 1);
    let trees = overview["trees"].as_array().unwrap();
    assert_eq!(
        trees
            .iter()
            .map(|tree| (
                tree["root"].as_str().unwrap(),
                tree["workspace"].as_str().unwrap(),
                tree["kind"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        [
            ("0/0", "human", "topic"),
            ("1/0", "agent", "topic"),
            ("GuiBook", "agent", "literature")
        ]
    );
    let card = api("GET", "/api/card?at=0%2F0", None);
    assert_eq!(card["workspace"], "human");
    assert_eq!(card["inbound"][0]["address"], "1/0");
    assert_eq!(card["inbound"][0]["workspace"], "agent");
    let card = api("GET", "/api/card?at=1%2F0", None);
    assert_eq!(card["outbound"][0]["workspace"], "human");
    assert!(
        api(
            "POST",
            "/api/move",
            Some(r#"{"at":"1/1","to":"0/1","confirm":"move"}"#)
        )["error"]
            .as_str()
            .unwrap()
            .contains("Cards cannot move between workspaces")
    );

    api("POST", "/api/quit", Some("{}"));
    assert!(gui.wait().expect("gui exits").success());
}

#[cfg(windows)]
fn zip_backend_available() -> bool {
    StdCommand::new("tar.exe")
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

/// One Card as SQLite stores it: address, kind, Workspace, BibTeX, and text.
type CardRow = (String, &'static str, String, Option<String>, String);

#[test]
fn dump_records_workspaces_and_next_topic_id_so_it_replicates_the_archive() {
    if !zip_backend_available() {
        return;
    }
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    home.cmd()
        .write_stdin(
            "l\n@book{HumanLit, title={Human Lit}}\n\x13notes on [[0/1]]\n\x13q\n".as_bytes(),
        )
        .assert()
        .success();
    {
        let mut mcp = home.mcp();
        mcp.ok(
            "create_topic",
            json!({ "title": "Agent Topic", "body": "builds on [[0/1]]" }),
        );
        mcp.ok(
            "create_successor",
            json!({ "at": "1/0", "title": "Agent Card" }),
        );
        mcp.ok("create_topic", json!({ "title": "Discarded" }));
        mcp.ok("delete_card", json!({ "at": "2/0", "confirm": "2/0" }));
        mcp.ok(
            "create_literature",
            json!({ "bibtex": "@article{AgentLit,\n  title = {Agent Lit}\n}\n", "body": "agent notes" }),
        );
        mcp.ok(
            "create_successor",
            json!({ "at": "AgentLit", "title": "Agent Lit Note" }),
        );
    }
    assert_eq!(home.workspace("HumanLit"), "human");

    home.cmd()
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("dumped 7 cards to "));

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
    let mut read = |name: &str| {
        let mut body = String::new();
        std::io::Read::read_to_string(
            &mut archive
                .by_name(&format!("{folder}/{name}"))
                .unwrap_or_else(|_| panic!("dump entry {name}")),
            &mut body,
        )
        .expect("UTF-8 entry");
        body
    };
    let mapping: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&read("mapping.json")).expect("mapping.json");
    let manifest: Value = serde_json::from_str(&read("archive.json")).expect("archive.json");

    // The manifest names a Workspace for exactly the dumped addresses.
    let workspaces = manifest["workspaces"]
        .as_object()
        .expect("workspaces object");
    assert_eq!(
        workspaces.keys().collect::<Vec<_>>(),
        mapping.keys().collect::<Vec<_>>()
    );
    // Topic 2 was deleted, so the next Topic number is not derivable from the Cards.
    assert_eq!(manifest["next_topic_id"], 3);
    let stored_next: String = home
        .db()
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_next, "3");

    // Rebuild every Card from the dump alone.
    let mut rebuilt: Vec<CardRow> = mapping
        .iter()
        .map(|(address, file)| {
            let markdown = read(file);
            let (bibtex, text) = match markdown.strip_prefix("---\nzt_kind: literature\n") {
                Some(rest) => {
                    let closing = rest.find("\n---\n").expect("front matter end");
                    (
                        Some(read(&format!("{address}.bib"))),
                        rest[closing + 5..].to_string(),
                    )
                }
                None => (None, markdown),
            };
            let kind = if bibtex.is_some() {
                "literature"
            } else if address.ends_with("/0") {
                "topic"
            } else {
                "regular"
            };
            let workspace = workspaces[address].as_str().expect("workspace").to_string();
            (address.clone(), kind, workspace, bibtex, text)
        })
        .collect();
    rebuilt.sort();

    let conn = home.db();
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(location, citation_key), is_topic, is_lit, bibtex, text, workspace
             FROM cards ORDER BY COALESCE(location, citation_key)",
        )
        .unwrap();
    let mut stored: Vec<CardRow> = stmt
        .query_map([], |row| {
            let kind = match (row.get::<_, i64>(1)?, row.get::<_, i64>(2)?) {
                (_, 1) => "literature",
                (1, _) => "topic",
                _ => "regular",
            };
            Ok((row.get(0)?, kind, row.get(5)?, row.get(3)?, row.get(4)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    stored.sort();

    assert_eq!(rebuilt, stored);
    let by_workspace = |workspace: &str| {
        stored
            .iter()
            .filter(|row| row.2 == workspace)
            .map(|row| (row.0.as_str(), row.1))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        by_workspace("human"),
        [
            ("0/0", "topic"),
            ("0/1", "regular"),
            ("HumanLit", "literature")
        ]
    );
    assert_eq!(
        by_workspace("agent"),
        [
            ("1/0", "topic"),
            ("1/1", "regular"),
            ("AgentLit", "literature"),
            ("AgentLit/1", "regular")
        ]
    );
}
