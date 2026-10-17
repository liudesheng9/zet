//! The model tools: `model` (model-compiler.ps1), `model-context`
//! (model-context.ps1), and `list-models` (list-models.ps1).
//!
//! Each keeps its script's parameters, output lines, files, and refusals.
//! One process holds one `zt mcp` for every read and write, runs zt-check
//! in-process, and builds a model's context pack in-process (the script ran
//! model-context.ps1 as a child).

use crate::ps;

mod compiler;
mod context;
mod list;

mod text;

use crate::zt::Zt;
use anyhow::Result;
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;
use zt_check::mcp::{reply_error, reply_text};

pub fn cli(argv: &[String]) -> Result<i32> {
    compiler::cli(argv)
}

pub fn context_cli(argv: &[String]) -> Result<i32> {
    context::cli(argv)
}

pub fn list_cli(argv: &[String]) -> Result<i32> {
    list::cli(argv)
}

static WS_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\r?\n\s*").unwrap());

/// `Invoke-Zt` with a batch of reads, as zt-calls.ps1 ran it (its direct
/// path, one `zt mcp`): the answers that are JSON objects, in order. A failed
/// call stops the batch with `zt-mcp failed: <i> error <message>`.
pub(crate) fn read_calls(zt: &mut Zt, calls: &[(&str, Value)]) -> Result<Vec<Value>> {
    let mut values = Vec::new();
    for (i, (tool, arguments)) in calls.iter().enumerate() {
        let reply = zt.request(tool, arguments.clone())?;
        let text = reply_text(&reply);
        if let Some(message) = reply_error(&reply) {
            anyhow::bail!("zt-mcp failed: {i} error {}", WS_NEWLINE.replace_all(&message, " "));
        }
        if text.trim_start().starts_with('{') {
            values.push(serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("Conversion from JSON failed with error: {e}"))?);
        }
    }
    Ok(values)
}

/// One read: its answer, or null when the answer is not a JSON object.
pub(crate) fn read_one(zt: &mut Zt, tool: &str, arguments: Value) -> Result<Value> {
    Ok(read_calls(zt, &[(tool, arguments)])?.into_iter().next().unwrap_or(Value::Null))
}

/// `get_card` / `get_model` at an address (the address as the script passed it).
pub(crate) fn get_card(zt: &mut Zt, at: &Value) -> Result<Value> {
    read_one(zt, "get_card", json!({ "at": at }))
}

pub(crate) fn get_model(zt: &mut Zt, at: &Value) -> Result<Value> {
    read_one(zt, "get_model", json!({ "at": at }))
}

/// Where the scripts live: `<repo>\scripts\book-digest`. The repository is
/// the one zt-digest was built from, or ZT_DIGEST_REPO. Job files name the
/// scripts by these paths, as the scripts named themselves.
pub(crate) struct Scripts {
    pub dir: String,
    /// The helper command job files name on their `HELPER:` line: `zt-digest call`
    /// (the old path `skills\zt\scripts\zt-mcp.ps1` forwards to it).
    pub helper: String,
}

impl Scripts {
    pub fn find() -> Self {
        let repo = match std::env::var("ZT_DIGEST_REPO") {
            Ok(repo) if !repo.trim().is_empty() => repo,
            _ => ps::parent(&ps::parent(env!("CARGO_MANIFEST_DIR"))),
        };
        let repo = repo.replace('/', "\\");
        let repo = repo.trim_end_matches('\\');
        Self { dir: format!("{repo}\\scripts\\book-digest"), helper: "zt-digest call".to_string() }
    }

    pub fn file(&self, name: &str) -> String {
        ps::join(&self.dir, name)
    }
}
