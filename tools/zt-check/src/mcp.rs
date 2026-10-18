//! A small client of `zt mcp` (the server Claude Code, Codex, and the zt
//! helper script use). zt-check reads Cards through it, so it sees the same
//! `get_card`, `get_model`, `get_tree`, and `list_trees` answers as every
//! other zt client, and it needs none of zt's code.
//! It mirrors the protocol of `zt-mcp.ps1`: `initialize`, then one
//! `tools/call` per call, one JSON message per line.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

mod owned;

/// Stable transport classification; a timed-out write may already have committed.
#[derive(Debug)]
pub struct TransportFault {
    pub code: &'static str,
    pub operation: String,
    pub child_pid: u32,
    pub cleanup_confirmed: bool,
    pub cause: String,
}
impl std::fmt::Display for TransportFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}: {}; owned_child={}; cleanup_confirmed={}",
            self.code, self.operation, self.cause, self.child_pid, self.cleanup_confirmed
        )
    }
}
impl std::error::Error for TransportFault {}

struct Exchange {
    message: Value,
    id: Option<Value>,
}

pub struct Zt {
    process: owned::Owned,
    commands: Option<Sender<Exchange>>,
    replies: Receiver<std::result::Result<Value, String>>,
    worker: Option<JoinHandle<()>>,
    deadline: Duration,
    shutdown: Duration,
    next: u64,
}

fn limits() -> (Duration, Duration) {
    // Production defaults are versioned tool policy, not workflow knobs.
    // Shorter deadlines exist only in isolated lifecycle fixtures.
    let isolated = std::env::var_os("ZT_CONFIG_DIR")
        .and_then(|p| std::fs::canonicalize(p).ok())
        .is_some_and(|p| {
            std::fs::canonicalize(std::env::temp_dir())
                .ok()
                .is_some_and(|t| p.starts_with(t))
                && p.parent()
                    .and_then(std::path::Path::file_name)
                    .is_some_and(|n| n.to_string_lossy().starts_with("mcp-lifecycle-test-"))
        });
    let test = |name, default| {
        if isolated {
            std::env::var(name)
                .ok()
                .and_then(|n| n.parse::<u64>().ok())
                .filter(|n| (100..=10000).contains(n))
                .unwrap_or(default)
        } else {
            default
        }
    };
    (
        Duration::from_millis(test("ZT_MCP_TEST_TIMEOUT_MS", 240000)),
        Duration::from_millis(test("ZT_MCP_TEST_SHUTDOWN_MS", 2000)),
    )
}

/// The edit patience the child `zt mcp` gets (R269). Long batch writes need more than the
/// ordinary short bound; ordinary reads and prechecks keep the default. The value is
/// process policy (a versioned tool default), never a workflow knob, so a caller sets it
/// once before it starts the client instead of passing it through every command.
static CHILD_EDIT_PATIENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn child_edit_patience(seconds: u64) {
    CHILD_EDIT_PATIENCE.store(seconds, std::sync::atomic::Ordering::Relaxed);
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
        let mut command = Command::new(&program);
        command
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let patience = CHILD_EDIT_PATIENCE.load(std::sync::atomic::Ordering::Relaxed);
        if patience > 0 && patience <= 300 {
            command.env("ZT_EDIT_PATIENCE", patience.to_string());
        }
        let mut process = owned::Owned::spawn(&mut command)
            .with_context(|| format!("failed to start `{program} mcp`"))?;
        let mut stdin = process.child.stdin.take().context("zt mcp has no stdin")?;
        let mut stdout = BufReader::new(
            process
                .child
                .stdout
                .take()
                .context("zt mcp has no stdout")?,
        );
        let (send, commands) = mpsc::channel::<Exchange>();
        let (reply, replies) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            while let Ok(work) = commands.recv() {
                let result = (|| -> Result<Value> {
                    writeln!(stdin, "{}", work.message)?;
                    stdin.flush()?;
                    let Some(id) = work.id else {
                        return Ok(Value::Null);
                    };
                    loop {
                        let mut bytes = Vec::new();
                        // Root MCP bounds encoded replies; this larger transport
                        // ceiling also prevents an endless line from growing RAM.
                        std::io::Read::take(&mut stdout, 32 * 1024 * 1024 + 1)
                            .read_until(b'\n', &mut bytes)?;
                        if bytes.is_empty() {
                            bail!("zt mcp closed");
                        }
                        if bytes.len() > 32 * 1024 * 1024 {
                            bail!("MCP reply line exceeds 32 MiB");
                        }
                        let text =
                            std::str::from_utf8(&bytes).context("MCP reply is not strict UTF-8")?;
                        let Ok(value) = serde_json::from_str::<Value>(text.trim()) else {
                            continue;
                        };
                        if value.get("id") == Some(&id) {
                            return Ok(value);
                        }
                    }
                })();
                let failed = result.is_err();
                if reply.send(result.map_err(|e| format!("{e:#}"))).is_err() || failed {
                    break;
                }
            }
        });
        let (deadline, shutdown) = limits();
        let mut zt = Self {
            process,
            commands: Some(send),
            replies,
            worker: Some(worker),
            deadline,
            shutdown,
            next: 1,
        };
        zt.exchange(json!({ "jsonrpc": "2.0", "id": 0, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }),Some(json!(0)),"initialize")?;
        zt.exchange(
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            None,
            "initialized notification",
        )?;
        Ok(zt)
    }

    fn exchange(&mut self, message: Value, id: Option<Value>, operation: &str) -> Result<Value> {
        self.commands
            .as_ref()
            .context("zt mcp is closed")?
            .send(Exchange { message, id })
            .context("zt mcp I/O worker closed")?;
        let (code, cause) = match self.replies.recv_timeout(self.deadline) {
            Ok(Ok(reply)) => return Ok(reply),
            Ok(Err(cause)) => ("transport_closed", cause),
            Err(mpsc::RecvTimeoutError::Timeout) => (
                "command_timeout",
                format!("MCP exchange exceeded {} ms", self.deadline.as_millis()),
            ),
            Err(mpsc::RecvTimeoutError::Disconnected) => (
                "transport_closed",
                "MCP I/O worker disconnected".to_string(),
            ),
        };
        self.commands.take();
        let cleanup = self
            .process
            .terminate(self.shutdown)
            .and_then(|_| self.join_worker());
        Err(TransportFault {
            code,
            operation: operation.to_string(),
            child_pid: self.process.child.id(),
            cleanup_confirmed: cleanup.is_ok(),
            cause: match cleanup {
                Ok(()) => cause,
                Err(e) => format!("{cause}; {e:#}"),
            },
        }
        .into())
    }

    fn join_worker(&mut self) -> Result<()> {
        let until = Instant::now() + self.shutdown;
        while self.worker.as_ref().is_some_and(|w| !w.is_finished()) && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        if self.worker.as_ref().is_some_and(|w| !w.is_finished()) {
            bail!("owned_cleanup_unconfirmed: MCP I/O worker did not stop");
        }
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("MCP I/O worker panicked"))?;
        }
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.commands.take(); // The idle worker drops stdin; give the child EOF first.
        if !self.process.wait(self.shutdown)? {
            self.process.terminate(self.shutdown)?;
        }
        self.join_worker()
    }

    /// One tool call; the whole JSON-RPC reply, an error reply included.
    pub fn request(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        let id = json!(self.next);
        self.next += 1;
        self.exchange(json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": tool, "arguments": arguments } }),Some(id),tool)
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

    /// A body-free identity/revision probe for immutable preparation binding.
    /// Refuse a partial or malformed view before any caller can begin writing.
    pub fn archive_view(&mut self) -> Result<Value> {
        let view = self.call("get_cards", json!({"addresses":[],"view":"text"}))?;
        let identity = &view["archive_identity"];
        if view["complete"] != true
            || view["archive_revision"].as_u64().is_none()
            || view["cards"]
                .as_array()
                .is_none_or(|cards| !cards.is_empty())
            || view["missing"]
                .as_array()
                .is_none_or(|missing| !missing.is_empty())
            || identity["id"]
                .as_str()
                .is_none_or(|id| id.len() != 32 || !id.bytes().all(|c| c.is_ascii_hexdigit()))
            || identity["path"].as_str().is_none_or(str::is_empty)
        {
            bail!("incomplete archive identity view; preparation cannot bind this archive");
        }
        Ok(view)
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
        if self.commands.is_some() {
            if let Err(e) = self.shutdown() {
                eprintln!("MCP cleanup: {e:#}");
            }
        }
    }
}
