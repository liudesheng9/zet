//! S06 (#148): the running service's check contract through real stdio MCP.
//!
//! The verification seam of the issue: a real `zt mcp` initialization and the
//! empty `get_cards` metadata view a self-check reads before it decides, over one
//! isolated scratch archive. The live archive is never touched.

use assert_cmd::Command;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command as StdCommand, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use zt_check::core::Limits;
use zt_check::contract::{self, Contract};
use zt_check::policy::PolicySet;

struct Home {
    _tmp: TempDir,
    config_dir: std::path::PathBuf,
    archive_root: std::path::PathBuf,
}

impl Home {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = Self {
            config_dir: tmp.path().join("config"),
            archive_root: tmp.path().join("archive"),
            _tmp: tmp,
        };
        home.run(&[
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ]);
        home.run(&["up"]);
        home
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    fn run(&self, args: &[&str]) -> String {
        let out = self.cmd().args(args).output().expect("zt command");
        assert!(
            out.status.success(),
            "zt {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn policy_hash(&self) -> String {
        self.run(&["config", "show"])
            .lines()
            .find_map(|line| line.strip_prefix("policy_hash="))
            .expect("config show publishes the policy hash")
            .to_string()
    }

    fn mcp(&self) -> Mcp {
        let mut command = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"));
        command
            .arg("mcp")
            .env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().expect("start zt mcp");
        let stdin = Some(child.stdin.take().expect("mcp stdin"));
        let stdout = BufReader::new(child.stdout.take().expect("mcp stdout"));
        let mut mcp = Mcp {
            child,
            stdin,
            stdout,
            next_id: 1,
            initialize: Value::Null,
        };
        let initialize = mcp.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "zt-check-contract-tests", "version": "1" },
            }),
        );
        mcp.initialize = initialize["result"].clone();
        mcp.notify("notifications/initialized");
        mcp
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.archive_root.join("zt.pid"));
        thread::sleep(Duration::from_millis(80));
    }
}

struct Mcp {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    initialize: Value,
}

impl Mcp {
    fn send(&mut self, message: &Value) {
        let stdin = self.stdin.as_mut().expect("mcp stdin");
        writeln!(stdin, "{message}").expect("write MCP message");
        stdin.flush().expect("flush MCP message");
    }

    fn notify(&mut self, method: &str) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method }));
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            self.stdout.read_line(&mut line).expect("read MCP reply");
            assert!(!line.is_empty(), "zt mcp closed");
            let Ok(reply) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if reply["id"] == json!(id) {
                return reply;
            }
        }
    }

    fn call(&mut self, name: &str, arguments: Value) -> Value {
        let reply = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        assert_eq!(reply["result"]["isError"], json!(false), "{reply}");
        serde_json::from_str(
            reply["result"]["content"][0]["text"]
                .as_str()
                .expect("tool result text"),
        )
        .expect("tool result JSON")
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        // Closing stdin ends the server on EOF; reaping it keeps the test host clean.
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_initialization_publishes_the_checker_contract() {
    let home = Home::new();
    let mut mcp = home.mcp();
    assert_eq!(
        mcp.initialize["serverInfo"]["name"],
        json!("zt"),
        "the initialization answer names the server"
    );
    assert_eq!(
        mcp.initialize["zt_check_version"],
        json!(zt_check::VERSION),
        "the proxy names the checker build it carries"
    );
    let declared = mcp.initialize["check_contract"].clone();
    assert_eq!(declared["rule_version"], json!(contract::RULE_VERSION));
    assert_eq!(
        declared["diagnostic_schema"],
        json!(contract::DIAGNOSTIC_SCHEMA)
    );
    assert_eq!(
        declared["applicability_version"],
        json!(contract::APPLICABILITY_VERSION)
    );
    // The same facts come from the running service's read reply, which is
    // authoritative when a stdio proxy and the daemon are different builds.
    let view = mcp.call("get_cards", json!({"addresses": [], "view": "text"}));
    let served = Contract::read(&view["check_contract"]).expect("the read reply publishes a contract");
    assert_eq!(served.rule_version, declared["rule_version"].as_str().unwrap());
    assert_eq!(
        served.diagnostic_schema,
        declared["diagnostic_schema"].as_str().unwrap()
    );
}

#[test]
fn an_empty_get_cards_view_publishes_the_running_policy_limits_and_archive() {
    let home = Home::new();
    let mut mcp = home.mcp();
    let view = mcp.call("get_cards", json!({"addresses": [], "view": "text"}));
    assert_eq!(view["complete"], json!(true));
    assert_eq!(view["cards"], json!([]));
    let published = &view["check_contract"];
    let served = Contract::read(published).expect("the read reply publishes a contract");

    // The effective policy is the archive's own: both Workspaces, and the hash
    // `zt config show` publishes (design 6.1, R0120).
    assert_eq!(served.policy_hash, home.policy_hash());
    let stored = PolicySet::new();
    assert_eq!(served.policy_hash, stored.hash(), "a fresh archive has no override");
    assert_eq!(
        served.policy.policy(zt_check::policy::Workspace::Agent).effective("R2"),
        Some(zt_check::policy::Setting::Block)
    );
    assert_eq!(
        served.policy.policy(zt_check::policy::Workspace::Human).effective("R2"),
        Some(zt_check::policy::Setting::Advise)
    );
    // The limits that decide the gate (R0202).
    assert_eq!(served.limits, Limits::default());
    // The archive identity and revision (R0203).
    assert_eq!(
        served.archive.as_ref().map(|archive| archive.revision),
        view["archive_revision"].as_u64()
    );
    assert_eq!(
        served.archive.as_ref().map(|archive| archive.id.as_str()),
        view["archive_identity"]["id"].as_str()
    );
    assert_eq!(
        served.archive.as_ref().map(|archive| archive.path.as_str()),
        view["archive_identity"]["path"].as_str()
    );
    // The gate stage and the syntax capabilities (R0204).
    assert_eq!(served.stage, "advisory");
    assert!(!served.capabilities.term_links);
    assert!(served.capabilities.term_cards);
    assert_eq!(served.checker_version, zt_check::VERSION);
    assert!(served.version_gaps().is_empty());
    assert!(served.limit_gaps(&Limits::default()).is_empty());
}

#[test]
fn a_stored_policy_override_reaches_the_published_contract_hash() {
    let home = Home::new();
    let before = home.policy_hash();
    home.run(&["config", "set", "gate.agent.R2", "advise"]);
    let after = home.policy_hash();
    assert_ne!(before, after, "a changed decision moves the policy hash");

    let mut mcp = home.mcp();
    let view = mcp.call("get_cards", json!({"addresses": [], "view": "text"}));
    let served =
        Contract::read(&view["check_contract"]).expect("the read reply publishes a contract");
    assert_eq!(served.policy_hash, after);
    assert_eq!(view["check_contract"]["policy"]["agent"]["R2"], json!("advise"));
    assert_eq!(view["check_contract"]["policy"]["human"]["R2"], json!("advise"));
}

/// S06-AC04/R0198/R0205: the read contract and the write reply carry the same
/// rule version and policy hash, so a client that read the contract predicts the
/// gate that actually decides its write. The reply names the build separately.
#[test]
fn a_write_reply_names_the_same_rule_contract_as_the_read() {
    let home = Home::new();
    let mut mcp = home.mcp();
    let view = mcp.call("get_cards", json!({"addresses": [], "view": "text"}));
    let served =
        Contract::read(&view["check_contract"]).expect("the read reply publishes a contract");
    mcp.call("create_topic", json!({"title": "合同主题"}));
    let written = mcp.call(
        "create_successor",
        json!({"at": "0/0", "title": "一次写入", "body": "写入一个普通的想法，说明它自己的内容。\n\n来源：[[0/0]]"}),
    );
    assert_eq!(written["check"]["complete"], json!(true));
    assert_eq!(
        written["check"]["rule_version"],
        json!(contract::RULE_VERSION),
        "the write names the rule contract that decided it"
    );
    assert_eq!(
        written["check"]["policy_hash"],
        json!(served.policy_hash),
        "the read and the write agree on the effective policy"
    );
    assert_eq!(written["check"]["checker_version"], json!(zt_check::VERSION));
    assert_eq!(written["check"]["stage"], json!("advisory"));
}

/// S06-AC08: no extra check tool or policy endpoint is added. The existing
/// verified read publishes the contract, and the existing write reply/preview
/// carries the version and hash that decided.
#[test]
fn no_extra_check_or_policy_tool_is_published() {
    let home = Home::new();
    let mut mcp = home.mcp();
    let tools = mcp.request("tools/list", json!({}));
    let names = tools["result"]["tools"]
        .as_array()
        .expect("tools list")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    for absent in ["check_cards", "check_card", "card_check", "gate_policy", "check_contract"] {
        assert!(
            !names.contains(&absent),
            "the check contract rides on get_cards; `{absent}` must not exist: {names:?}"
        );
    }
    assert!(names.contains(&"get_cards"), "the existing verified read stays");
    assert!(
        names.contains(&"apply_batch"),
        "the service preview stays the placement-dependent preflight"
    );
}

