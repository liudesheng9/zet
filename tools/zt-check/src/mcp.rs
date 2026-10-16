//! A small client of `zt mcp` (the server Claude Code, Codex, and the zt
//! helper script use). zt-check reads Cards through it, so it sees the same
//! `get_card`, `get_model`, `get_tree`, and `list_trees` answers as every
//! other zt client, and it needs none of zt's code.
//! It mirrors the protocol of `zt-mcp.ps1`: `initialize`, then one
//! `tools/call` per call, one JSON message per line.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct Zt {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Zt {
    /// Starts `zt mcp`: the `zt` on PATH, or the program in ZT_CHECK_ZT.
    pub fn start() -> Result<Self> {
        let program = std::env::var("ZT_CHECK_ZT").unwrap_or_else(|_| "zt".to_string());
        let mut child = Command::new(&program)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("failed to start `{program} mcp`"))?;
        let stdin = child.stdin.take().context("zt mcp has no stdin")?;
        let stdout = BufReader::new(child.stdout.take().context("zt mcp has no stdout")?);
        let mut zt = Self { child, stdin: Some(stdin), stdout, next: 1 };
        zt.send(&json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }))?;
        zt.receive(&json!(0))?;
        zt.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))?;
        Ok(zt)
    }

    fn send(&mut self, message: &Value) -> Result<()> {
        let stdin = self.stdin.as_mut().context("zt mcp is closed")?;
        writeln!(stdin, "{message}")?;
        stdin.flush()?;
        Ok(())
    }

    fn receive(&mut self, id: &Value) -> Result<Value> {
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line)? == 0 {
                bail!("zt mcp closed");
            }
            let Ok(reply) = serde_json::from_str::<Value>(line.trim()) else { continue };
            if reply.get("id") == Some(id) {
                return Ok(reply);
            }
        }
    }

    /// One tool call; its result is the JSON text of the reply's content.
    pub fn call(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        let id = json!(self.next);
        self.next += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": tool, "arguments": arguments } }))?;
        let reply = self.receive(&id)?;
        let text = reply["result"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if reply.get("error").is_some_and(|e| !e.is_null()) || reply["result"]["isError"].as_bool() == Some(true) {
            let message = reply["error"]["message"].as_str().map(str::to_string).unwrap_or(text);
            let message = serde_json::from_str::<Value>(&message)
                .ok()
                .and_then(|v| v["error"].as_str().map(str::to_string))
                .unwrap_or(message);
            bail!("zt: {message}");
        }
        serde_json::from_str(&text).with_context(|| format!("{tool}: the answer is not JSON"))
    }
}

impl Drop for Zt {
    fn drop(&mut self) {
        // Closing stdin ends the server; then reap it.
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}
