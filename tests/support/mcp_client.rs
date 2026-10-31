//! A scratch archive with a running service, and a `zt mcp` client over stdio. Shared by the
//! test files that drive the MCP server through its real process, so each one does not carry its
//! own copy of the client.

#![allow(dead_code)]

use assert_cmd::Command;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command as StdCommand, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// One scratch archive and its config folder; the service is stopped when it drops.
pub struct ScratchBox {
    _tmp: TempDir,
    pub config_dir: PathBuf,
    pub archive_root: PathBuf,
}

impl ScratchBox {
    /// A configured archive with the service up.
    pub fn up() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let scratch = Self {
            config_dir: tmp.path().join("config"),
            archive_root: tmp.path().join("archive"),
            _tmp: tmp,
        };
        scratch
            .cmd()
            .args(["config", "set", "archive_root", scratch.archive_root.to_str().unwrap()])
            .assert()
            .success();
        scratch.cmd().arg("up").assert().success();
        scratch
    }

    /// The `zt` binary under test, pointed at this box.
    pub fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir).env("ZT_GUI_PORT", "0");
        cmd
    }

    /// `zt mcp` with these extra arguments, after the MCP handshake.
    pub fn mcp(&self, arguments: &[&str]) -> Mcp {
        let mut child = self.mcp_command(arguments).spawn().expect("start zt mcp");
        let stdin = child.stdin.take().expect("mcp stdin");
        let stdout = BufReader::new(child.stdout.take().expect("mcp stdout"));
        let mut mcp = Mcp { child, stdin, stdout, next_id: 1 };
        let init = mcp.request(
            "initialize",
            json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"zt-tests","version":"1"}}),
        );
        assert_eq!(init["result"]["serverInfo"]["name"], "zt", "{init}");
        mcp.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        mcp
    }

    /// The `zt mcp` command line with these extra arguments, not yet started.
    pub fn mcp_command(&self, arguments: &[&str]) -> StdCommand {
        let mut command = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"));
        command
            .arg("mcp")
            .args(arguments)
            .env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// A file in the box's scratch folder holding these lines.
    pub fn list_file(&self, name: &str, lines: &[&str]) -> String {
        fs::create_dir_all(&self.config_dir).expect("config dir");
        let path = self.config_dir.join(name);
        fs::write(&path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).expect("list file");
        path.to_string_lossy().into_owned()
    }
}

impl Drop for ScratchBox {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.archive_root.join("zt.pid"));
        thread::sleep(Duration::from_millis(80));
    }
}

/// A `zt mcp` child process spoken to one JSON-RPC line at a time.
pub struct Mcp {
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

    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read MCP reply");
        let reply: Value = serde_json::from_str(&line).expect("MCP reply is JSON");
        assert_eq!(reply["id"], id, "replies arrive in request order");
        reply
    }

    /// A tool call: its structured result and whether it failed.
    pub fn call(&mut self, name: &str, arguments: Value) -> (Value, bool) {
        let reply = self.request("tools/call", json!({"name":name,"arguments":arguments}));
        let result = &reply["result"];
        (result["structuredContent"].clone(), result["isError"].as_bool().expect("isError flag"))
    }

    pub fn ok(&mut self, name: &str, arguments: Value) -> Value {
        let (value, failed) = self.call(name, arguments.clone());
        assert!(!failed, "{name}({arguments}) failed: {value}");
        value
    }

    /// A call that must fail; its structured error.
    pub fn refused(&mut self, name: &str, arguments: Value) -> Value {
        let (value, failed) = self.call(name, arguments.clone());
        assert!(failed, "{name}({arguments}) should fail but returned {value}");
        value
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
