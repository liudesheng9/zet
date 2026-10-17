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

/// A complete required-address read from one archive revision.
pub struct CardRead {
    pub cards: Vec<Value>,
    pub archive_revision: u64,
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
        let mut zt = Self {
            child,
            stdin: Some(stdin),
            stdout,
            next: 1,
        };
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
            let Ok(reply) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            if reply.get("id") == Some(id) {
                return Ok(reply);
            }
        }
    }

    /// One tool call; the whole JSON-RPC reply, an error reply included.
    pub fn request(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        let id = json!(self.next);
        self.next += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": tool, "arguments": arguments } }))?;
        self.receive(&id)
    }

    /// One tool call; its result is the JSON text of the reply's content.
    pub fn call(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        let reply = self.request(tool, arguments)?;
        let text = reply_text(&reply);
        if let Some(message) = reply_error(&reply) {
            bail!("zt: {message}");
        }
        serde_json::from_str(&text).with_context(|| format!("{tool}: the answer is not JSON"))
    }

    /// Bounded reads through this command's existing MCP process. A missing
    /// required Card is incomplete input, never an empty successful dataset.
    pub fn cards(&mut self, addresses: &[String], view: &str) -> Result<CardRead> {
        let mut cards = Vec::new();
        let mut revision = None;
        let mut offset = 0;
        let mut width = 200;
        let mut first = true;
        while first || offset < addresses.len() {
            first = false;
            let chunk = &addresses[offset..(offset + width).min(addresses.len())];
            let reply = self.request("get_cards", json!({"addresses":chunk,"view":view}))?;
            if let Some(message) = reply_error(&reply) {
                let fault = reply["result"]
                    .get("structuredContent")
                    .cloned()
                    .unwrap_or_else(|| {
                        serde_json::from_str(&reply_text(&reply)).unwrap_or(Value::Null)
                    });
                if fault["code"] == "read_limit"
                    && matches!(
                        fault["limit_kind"].as_str(),
                        Some("reply_bytes" | "addresses")
                    )
                    && chunk.len() > 1
                {
                    width = chunk.len() / 2;
                    first = true;
                    continue;
                }
                bail!("zt: {message}");
            }
            let value: Value = serde_json::from_str(&reply_text(&reply))
                .context("incomplete Card read: answer is not JSON")?;
            let rows = value["cards"]
                .as_array()
                .context("incomplete Card read: missing cards array")?;
            let missing = value["missing"]
                .as_array()
                .context("incomplete Card read: missing missing-address list")?;
            if value["complete"] != true || !missing.is_empty() || rows.len() != chunk.len() {
                bail!(
                    "incomplete Card read: {} of {} required addresses; missing {}",
                    rows.len(),
                    chunk.len(),
                    value["missing"]
                );
            }
            for (at, row) in chunk.iter().zip(rows) {
                if row["address"].as_str() != Some(at) {
                    bail!("incomplete Card read: result order/address mismatch at `{at}`");
                }
            }
            let current = value["archive_revision"]
                .as_u64()
                .context("incomplete Card read: missing archive revision")?;
            if revision.is_some_and(|old| old != current) {
                bail!("incomplete Card read: changing view across archive revisions");
            }
            revision = Some(current);
            cards.extend(rows.iter().cloned());
            offset += chunk.len();
        }
        Ok(CardRead {
            cards,
            archive_revision: revision.context("incomplete Card read: no revision")?,
        })
    }
}

/// The text parts of a reply's content, joined by newlines.
pub fn reply_text(reply: &Value) -> String {
    reply["result"]["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The message of an error reply (`error`, or `isError` with the text, whose
/// JSON `error` field wins), or None for a reply that succeeded.
pub fn reply_error(reply: &Value) -> Option<String> {
    if !(reply.get("error").is_some_and(|e| !e.is_null())
        || reply["result"]["isError"].as_bool() == Some(true))
    {
        return None;
    }
    let message = reply["error"]["message"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| reply_text(reply));
    Some(
        serde_json::from_str::<Value>(&message)
            .ok()
            .and_then(|v| v["error"].as_str().map(str::to_string))
            .unwrap_or(message),
    )
}

impl Drop for Zt {
    fn drop(&mut self) {
        // Closing stdin ends the server; then reap it.
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}
