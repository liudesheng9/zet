//! The zt helper (`zt-digest call`), the Rust form of `skills/zt/scripts/zt-mcp.ps1`.
//!
//! It runs a JSON array of calls (or one call) through one `zt mcp` in order
//! and prints one compact line per call: `N ok <address>` for writes,
//! `N <json>` for reads, `N error <message>` on failure, after which the last
//! line is `resume: -From N -Addresses a0,a1,...` and the exit code is 1.
//! A failure the archive answered with a typed refusal also prints one line
//! before the resume line: `<code> <compact JSON>` (for example
//! `card_locked {"error":"card_locked","tree":"K","reason":"create-inside",...}`),
//! so a Lock refusal keeps its tree, Lock id, reason and hint across this
//! helper boundary and is never retried as a provider or transient fault.
//!
//!   `@@N`      in any string argument: the address call N returned (0-based).
//!   -From N -Addresses a0,...   skip the calls before N; their addresses still resolve `@@`.
//!   -Log f     append the address of every Card a create call makes.
//!   -Allow f   only those Cards (one per line) and Cards made in this run may change.
//!   -Beneath f these earlier Cards take only a new Side successor.
//!   -Moves f   append {"i":N,"moved":{...}} per call that renumbered Cards.
//!   -SrcLog f  append `<address>\t<src>` per Card made from a call with a `src` field
//!              (the helper removes `src` before the call reaches zt).
//!   append_card {at, text}: add a paragraph to the end of a Card (helper-only call); the edit
//!              carries the text it read as its base, so a change in between is refused.
//!
//! The Write scope is the server's (docs/agent-write-scope-proposal.md): `call` starts its `zt mcp`
//! with `--scope literature` and passes `-Allow`/`-Beneath` as `--allow`/`--beneath`; `open-call` is
//! the same helper in the open scope, for hand work on Topic trees, and no digest job's approval
//! allows it. A write outside the scope fails with the server's typed `scope_refused` line.
//!
//! The digest tools run the same code in-process through `run` and `invoke`.

use crate::args::Args;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use zt_check::mcp::{reply_error, reply_text};

/// The tree operations a zt 0.51 batch item may be (PRD #228): a chunk holding one is confirmed by
/// its plan hash.
const TREE_TOOLS: [&str; 9] = ["move_card", "insert_card", "delete_card", "move_span", "delete_span", "indent", "outdent", "reorder", "relink"];

#[derive(Default)]
pub struct Options {
    pub from: usize,
    pub addresses: Vec<String>,
    pub log: String,
    pub moves: String,
    pub src_log: String,
}

impl Options {
    pub fn new() -> Self {
        Self::default()
    }
}

/// The `zt mcp` declarations of an allow list and a beneath list (`--allow f`, `--beneath f`): the
/// server enforces them for every write of the connection that starts with them. An empty path
/// declares nothing.
pub fn scope_lists(allow: &str, beneath: &str) -> Result<Vec<String>> {
    let mut declared = Vec::new();
    for (flag, path) in [("--allow", allow), ("--beneath", beneath)] {
        if !path.is_empty() {
            let full = std::path::absolute(path).with_context(|| format!("cannot resolve {flag} list `{path}`"))?;
            declared.extend([flag.to_string(), full.to_string_lossy().into_owned()]);
        }
    }
    Ok(declared)
}

/// A connection that may change only these Cards (and the Cards it makes): the list goes to a file
/// under `work` and `zt mcp` starts with `--allow` on it.
pub fn start_allowed(work: &str, name: &str, addresses: &[String]) -> Result<Zt> {
    let path = std::path::Path::new(work).join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, addresses.iter().map(|a| format!("{a}\n")).collect::<String>())
        .with_context(|| format!("cannot write the allow list {}", path.display()))?;
    crate::zt::start_with(&scope_lists(&path.to_string_lossy(), "")?)
}

/// One call that succeeded.
pub struct Done {
    #[allow(dead_code)]
    pub index: usize,
    /// The line the helper prints for it.
    pub line: String,
    /// A read's answer (the reply text as JSON), or None for a write or a non-JSON answer.
    pub value: Option<Value>,
}

/// The call that failed: its index, the message, and the addresses so far.
pub struct Failure {
    pub index: usize,
    pub message: String,
    pub resolved: Vec<String>,
    /// The typed refusal the MCP replied with, when it sent one (R183, R338). A `card_locked`
    /// refusal keeps its tree, Lock id, reason and hint as JSON across this helper boundary, so a
    /// caller recognizes a Lock without matching English text and never retries it as a fault.
    pub refusal: Option<Value>,
}

impl Failure {
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![format!("{} error {}", self.index, one_line(&self.message))];
        if let Some(refusal) = &self.refusal {
            lines.push(format!("{} {}", refusal_code(refusal), util::compact(refusal)));
            // S27 (#166, R0447): the blocking fault lines of a `card_check` refusal are the content
            // a writer must fix. They follow the refusal's JSON, one per line, exactly as the
            // checker wrote them, so no writer parses the JSON or the message to find them.
            lines.extend(self.refusal_fault_lines());
        }
        lines.push(format!("resume: -From {} -Addresses {}", self.index, self.resolved.join(",")));
        lines
    }

    /// The blocking fault lines of a `card_check` refusal (S27, #166, R0447): the refusal's own
    /// `faults` array without the lines the checker marked `(advisory)`. Any other refusal carries
    /// none, and a Lock keeps exactly its three lines.
    pub fn refusal_fault_lines(&self) -> Vec<String> {
        let Some(refusal) = self.refusal.as_ref().filter(|refusal| refusal["error"] == json!("card_check")) else {
            return Vec::new();
        };
        refusal["faults"]
            .as_array()
            .map(|lines| {
                lines
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|line| !line.is_empty() && !line.contains("(advisory)"))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The `error` code of a structured refusal (`card_locked`, a read limit, a batch conflict).
fn refusal_code(refusal: &Value) -> &str {
    refusal["error"].as_str().unwrap_or("refusal")
}

/// The structured refusal of an MCP error reply: the reply's `structuredContent` when it carries an
/// `error` code. A transport-level error reply has none, and stays text only.
pub(crate) fn refusal_of(reply: &Value) -> Option<Value> {
    let structured = reply["result"]["structuredContent"].as_object()?;
    structured.get("error").and_then(Value::as_str)?;
    Some(Value::Object(structured.clone()))
}

/// The one message line of a typed refusal: its code and the sentence the archive wrote, so the
/// text a person or an agent reads still explains what stopped and why.
fn typed_message(message: &str, refusal: &Option<Value>) -> String {
    match refusal {
        Some(refusal) => match refusal["message"].as_str() {
            Some(said) => format!("{}: {said}", refusal_code(refusal)),
            None => message.to_string(),
        },
        None => message.to_string(),
    }
}

static WS_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\r?\n\s*").unwrap());
static REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"@@(\d+)").unwrap());

fn one_line(message: &str) -> String {
    WS_NEWLINE.replace_all(message, " ").into_owned()
}

/// `@@N` in every string of a value: the address of call N.
fn resolve_refs(value: &Value, resolved: &[String]) -> Result<Value> {
    Ok(match value {
        Value::String(text) => {
            let mut error = None;
            let out = REF.replace_all(text, |c: &regex::Captures| {
                let n: usize = c[1].parse().unwrap_or(usize::MAX);
                match resolved.get(n).filter(|a| !a.is_empty()) {
                    Some(a) => a.clone(),
                    None => {
                        error.get_or_insert(format!("@@{} has no address yet", &c[1]));
                        String::new()
                    }
                }
            });
            if let Some(error) = error {
                bail!(error);
            }
            Value::String(out.into_owned())
        }
        Value::Object(map) => {
            let mut copy = Map::new();
            for (k, v) in map {
                copy.insert(k.clone(), resolve_refs(v, resolved)?);
            }
            Value::Object(copy)
        }
        Value::Array(list) => Value::Array(list.iter().map(|v| resolve_refs(v, resolved)).collect::<Result<_>>()?),
        other => other.clone(),
    })
}

/// The Topic title comparison the server's Topic title rule uses too (one implementation).
pub use zt_check::scope::{similarity, title_key};

/// The concept gate's lock: the file opened with no sharing, for at most 120 seconds of waiting.
pub(crate) struct Gate(#[allow(dead_code)] std::fs::File);

pub(crate) fn lock_gate(path: &str) -> Result<Gate> {
    let full = std::path::absolute(path)?;
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        match options.open(&full) {
            Ok(file) => return Ok(Gate(file)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(error.into()),
            Err(_) => {
                if Instant::now() > deadline {
                    bail!("the Topic gate `{path}` stayed locked for 120 seconds");
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
}

fn arg_str(arguments: &Value, key: &str) -> String {
    match &arguments[key] {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Runs the calls in order and stops at the first failure. Err only when
/// zt mcp itself fails (it closed, or wrote no reply).
pub fn run(zt: &mut Zt, calls: &[Value], options: &mut Options) -> Result<(Vec<Done>, Option<Failure>)> {
    let mut done = Vec::new();
    let mut resolved: Vec<String> = (0..options.from).map(|i| options.addresses.get(i).cloned().unwrap_or_default()).collect();
    macro_rules! fail {
        ($i:expr, $message:expr) => {
            return Ok((done, Some(Failure { index: $i, message: $message.to_string(), resolved, refusal: None })))
        };
        ($i:expr, $message:expr, $refusal:expr) => {
            return Ok((done, Some(Failure { index: $i, message: $message.to_string(), resolved, refusal: $refusal })))
        };
    }
    for (i, call) in calls.iter().enumerate().skip(options.from) {
        let call_tool = call["tool"].as_str().unwrap_or_default().to_string();
        let mut arguments = match resolve_refs(call.get("arguments").filter(|a| !a.is_null()).unwrap_or(&json!({})), &resolved) {
            Ok(a) => a,
            Err(error) => fail!(i, error),
        };
        let mut tool = call_tool.clone();
        if tool == "append_card" {
            let at = arg_str(&arguments, "at");
            let current = zt.request("get_card", json!({ "at": at }))?;
            if reply_error(&current).is_some() {
                fail!(i, format!("cannot read `{at}`"));
            }
            let read = &current["result"]["structuredContent"];
            let body = arg_str(read, "body");
            let text = arg_str(&arguments, "text");
            // D8: the text read is the edit's base, so a write between the read and this edit is
            // refused ("changed since it was opened") instead of being overwritten.
            arguments = json!({ "at": at, "body": format!("{}\n\n{}", body.trim_end(), text.trim()), "base": read["text"] });
            tool = "edit_card".to_string();
        }
        // `src` is for the source log only; zt never sees it.
        let mut src = String::new();
        if let Some(map) = arguments.as_object_mut()
            && let Some(value) = map.remove("src")
        {
            src = match value {
                Value::String(s) => s,
                Value::Null => String::new(),
                other => other.to_string(),
            };
        }
        let reply = zt.request(&tool, arguments)?;
        let text = reply_text(&reply);
        if let Some(message) = reply_error(&reply) {
            let refusal = refusal_of(&reply);
            let message = typed_message(&message, &refusal);
            fail!(i, message, refusal);
        }
        let structured = &reply["result"]["structuredContent"];
        let address = structured["address"].as_str().map(str::to_string);
        resolved.push(address.clone().unwrap_or_default());
        if let Some(address) = &address
            && call_tool.starts_with("create")
        {
            if !options.log.is_empty() {
                util::append_text(&options.log, &format!("{address}\n"))?;
            }
            if !options.src_log.is_empty() && !src.is_empty() {
                util::append_text(&options.src_log, &format!("{address}\t{src}\n"))?;
            }
        }
        if !options.moves.is_empty()
            && let Some(moved) = structured["moved"].as_object().filter(|m| !m.is_empty())
        {
            util::append_text(&options.moves, &format!("{}\n", util::compact(&json!({ "i": i, "moved": moved }))))?;
        }
        let write = ["create", "edit", "append", "move", "insert", "delete"].iter().any(|p| call_tool.starts_with(p));
        match address.filter(|_| write) {
            Some(address) => done.push(Done { index: i, line: format!("{i} ok {address}"), value: None }),
            None => {
                let value = serde_json::from_str::<Value>(&text).ok();
                let shown = value.as_ref().map(util::compact).unwrap_or_else(|| text.clone());
                let value = value.filter(|_| text.trim_start().starts_with('{'));
                done.push(Done { index: i, line: format!("{i} {shown}"), value });
            }
        }
    }
    Ok((done, None))
}

/// `Invoke-Zt`: runs the calls and returns the answers of the reads (objects
/// only), in order. A failed call is an error `zt: <message>`.
pub fn invoke(zt: &mut Zt, calls: &[Value]) -> Result<Vec<Value>> {
    let (done, failure) = run(zt, calls, &mut Options::new())?;
    if let Some(failure) = failure {
        bail!("zt: {}", one_line(&failure.message));
    }
    Ok(done.into_iter().filter_map(|d| d.value).collect())
}

/// One read call; its answer.
#[allow(dead_code)]
pub fn read(zt: &mut Zt, tool: &str, arguments: Value) -> Result<Value> {
    invoke(zt, &[json!({ "tool": tool, "arguments": arguments })])?
        .into_iter()
        .next()
        .with_context(|| format!("{tool}: no answer"))
}

/// Apply an immutable accepted call list with archive receipts. Retrying starts
/// at the beginning and recovers each original address from its receipt. Each
/// chunk is atomic; the full list is not. The caller still verifies its outputs.
pub(crate) fn invoke_identified(
    zt: &mut Zt,
    calls: &[Value],
    prefix: &str,
) -> Result<Vec<Value>> {
    Ok(execute_identified(zt, calls, prefix, false, false, None, true)?.0)
}

/// Apply an identified call list whose outputs are staged for a later publication step: the
/// register's parts, and the root shell they are filed under, are rewritten by the root call that
/// follows them (D189). Only the receipts are returned; the caller's own fresh read of the staged
/// Cards is the completion evidence. A final output still uses [`invoke_identified`].
pub(crate) fn invoke_identified_staged(
    zt: &mut Zt,
    calls: &[Value],
    prefix: &str,
) -> Result<Vec<Value>> {
    Ok(execute_identified(zt, calls, prefix, false, false, None, false)?.0)
}

/// Apply an immutable accepted call list whose Topic creation was cleared by a concept decision
/// (S07, #100). A committed operation returns its receipt before any new concept decision; an
/// uncommitted `create_topic` takes the archive-shared Topic gate, revalidates the decision and
/// holds the lock until its receipt is known (D146–D153).
pub(crate) fn invoke_identified_gated(
    zt: &mut Zt,
    calls: &[Value],
    prefix: &str,
    concept: &Value,
) -> Result<Vec<Value>> {
    Ok(execute_identified(zt, calls, prefix, false, false, Some(concept), true)?.0)
}

/// Validate a complete accepted sequence against real archive validators. The
/// server simulates normal write chunks, then rolls back before returning.
pub(crate) fn preview_identified(
    zt: &mut Zt,
    calls: &[Value],
    prefix: &str,
) -> Result<Value> {
    let (receipts, draft) = execute_identified(zt, calls, prefix, true, false, None, false)?;
    let draft = draft.context("incomplete identified preview: no draft")?;
    if draft["preview"] != true
        || draft["applied"] != false
        || draft["complete"] != true
        || !draft["archive_revision"].is_u64()
    {
        bail!("incomplete identified preview");
    }
    let mut expected = BTreeMap::new();
    for receipt in &receipts {
        expected.insert(
            receipt["result"]["address"]
                .as_str()
                .context("incomplete preview address")?,
            receipt["result"]["after_fingerprint"]
                .as_str()
                .context("incomplete preview fingerprint")?,
        );
    }
    let cards = draft["cards"]
        .as_array()
        .context("incomplete preview Cards")?;
    if cards.len() != expected.len() {
        bail!("incomplete preview Card count");
    }
    for card in cards {
        let at = card["address"]
            .as_str()
            .context("incomplete preview Card address")?;
        if card["workspace"] != "agent"
            || expected.remove(at) != Some(authored_fingerprint(card).as_str())
        {
            bail!("incomplete preview authored output at {at}");
        }
    }
    Ok(draft)
}

fn execute_identified(
    zt: &mut Zt,
    calls: &[Value],
    prefix: &str,
    preview: bool,
    lean: bool,
    concept: Option<&Value>,
    verify: bool,
) -> Result<(Vec<Value>, Option<Value>)> {
    let segments: Vec<_> = prefix.split('/').collect();
    if segments.len() != 4
        || segments.iter().any(|s| s.is_empty())
        || segments[2].parse::<u64>().ok().filter(|n| *n > 0).is_none()
    {
        bail!(
            "OperationPrefix must be run/unit/positive-plan-revision/step, without an attempt component"
        );
    }
    fn arguments(value: &Value, resolved: &[String], index: usize, start: usize) -> Result<Value> {
        Ok(match value {
            Value::String(text) => {
                let mut error = None;
                let replaced = REF.replace_all(text, |capture: &regex::Captures| {
                    let n = capture[1].parse::<usize>().unwrap_or(usize::MAX);
                    if n >= index {
                        error = Some(format!("call {index}: @@{n} is not an earlier result"));
                        String::new()
                    } else if n < start {
                        match resolved.get(n) {
                            Some(address) => address.clone(),
                            None => {
                                error =
                                    Some(format!("call {index}: @@{n} has no committed address"));
                                String::new()
                            }
                        }
                    } else {
                        format!("@@{n}")
                    }
                });
                if let Some(error) = error {
                    bail!(error);
                }
                json!(replaced.into_owned())
            }
            Value::Array(values) => Value::Array(
                values
                    .iter()
                    .map(|v| arguments(v, resolved, index, start))
                    .collect::<Result<_>>()?,
            ),
            Value::Object(values) => Value::Object(
                values
                    .iter()
                    .map(|(k, v)| Ok((k.clone(), arguments(v, resolved, index, start)?)))
                    .collect::<Result<_>>()?,
            ),
            other => other.clone(),
        })
    }
    fn authored_bytes(arguments: &Value) -> usize {
        ["title", "body", "text", "bibtex", "base"]
            .iter()
            .map(|key| arguments[key].as_str().map(str::len).unwrap_or(0))
            .sum::<usize>()
            + arguments["owned_paragraph"]["text"]
                .as_str()
                .map(str::len)
                .unwrap_or(0)
    }
    if preview && (calls.is_empty() || calls.len() > 1024) {
        bail!("identified preview requires 1 through 1024 operations");
    }
    // Reject unsupported operations, forward dependencies and oversized single
    // items before committing any chunk. No placeholder makes a target exist.
    for (index, call) in calls.iter().enumerate() {
        let tool = call["tool"].as_str().unwrap_or_default();
        if !matches!(tool, "create_topic" | "create_literature" | "create_successor" | "edit_card")
            && !TREE_TOOLS.contains(&tool)
        {
            bail!("identified call {index}: unsupported write tool");
        }
        if !call["arguments"].is_object() {
            bail!("identified call {index}: arguments must be an object");
        }
        arguments(&call["arguments"], &[], index, 0)?;
        if call["tool"] == "edit_card" {
            let args = &call["arguments"];
            if args.get("base").is_some() {
                bail!(
                    "identified call {index}: use an authored base_fingerprint/base_receipt; raw base belongs to single edit_card"
                );
            }
            let earlier = args["at"]
                .as_str()
                .and_then(|at| at.strip_prefix("@@"))
                .and_then(|n| n.parse::<usize>().ok())
                .is_some_and(|n| n < index);
            if args.get("base_fingerprint").is_none()
                && args.get("base_receipt").is_none()
                && !earlier
            {
                bail!(
                    "identified call {index}: edit requires an authored base or an earlier accepted result reference"
                );
            }
        }
        if authored_bytes(&call["arguments"]) > 1024 * 1024 {
            bail!("identified call {index}: exceeds 1048576 authored bytes");
        }
    }
    // D6 (#226): the server checks an allow list chunk by chunk, and one chunk is atomic. A list
    // that needs more than one chunk is scanned first with a lean preview (no bodies come back), so
    // a call outside the list still writes nothing at all.
    let total: usize = calls.iter().map(|call| authored_bytes(&call["arguments"])).sum();
    if !preview && zt.declares("--allow") && (calls.len() > 64 || total > 1024 * 1024) {
        execute_identified(zt, calls, prefix, true, true, None, false)?;
    }
    let mut resolved = Vec::new();
    let mut done = Vec::new();
    let mut draft = None;
    while done.len() < calls.len() {
        let start = done.len();
        let mut items = Vec::new();
        let mut bytes = 0;
        for index in start..if preview {
            calls.len()
        } else {
            calls.len().min(start + 64)
        } {
            let mut args = arguments(&calls[index]["arguments"], &resolved, index, start)?;
            if calls[index]["tool"] == "edit_card"
                && args.get("base_fingerprint").is_none()
                && args.get("base_receipt").is_none()
            {
                let n = calls[index]["arguments"]["at"]
                    .as_str()
                    .and_then(|at| at.strip_prefix("@@"))
                    .and_then(|n| n.parse::<usize>().ok())
                    .context("identified edit missing an earlier base reference")?;
                args["base_receipt"] = json!(format!("{prefix}/item-{n:06}"));
            }
            let size = authored_bytes(&args);
            if !preview && bytes + size > 1024 * 1024 {
                if items.is_empty() {
                    bail!("identified call {index}: resolved input exceeds 1048576 authored bytes");
                }
                break;
            }
            bytes += size;
            items.push(json!({"op_key":format!("{prefix}/item-{index:06}"),"client_ref":index.to_string(),"tool":calls[index]["tool"],"arguments":args}));
        }
        let count = items.len();
        // S07 (#100): a Topic creation in this chunk needs the concept gate. A committed operation
        // is replayed by that same call, so its receipt comes back before any new decision; an
        // uncommitted one takes the archive-shared lock, revalidates the decision and holds the lock
        // until its receipt is known.
        let mut concept_lock: Option<crate::gate::Held> = None;
        if !preview
            && let Some(decision) = concept
        {
            for (offset, item) in items.iter().enumerate() {
                if item["tool"] != "create_topic" {
                    continue;
                }
                let peek = zt.call(
                    "apply_batch",
                    json!({"preview":true,"preview_items":[item.clone()]}),
                )?;
                let replayed = peek["results"]
                    .as_array()
                    .and_then(|results| results.first())
                    .is_some_and(|result| result["replayed"] == json!(true));
                if replayed {
                    continue;
                }
                if concept_lock.is_none() {
                    concept_lock = Some(crate::gate::acquire(zt, decision)?);
                }
                let expected = decision["proposed"]["title"].as_str().unwrap_or_default();
                let wanted = item["arguments"]["title"].as_str().unwrap_or_default();
                if title_key(expected) != title_key(wanted) {
                    bail!(
                        "identified call {} creates Topic `{wanted}`, which the concept decision did not clear (`{expected}`)",
                        start + offset
                    );
                }
            }
        }
        // A chunk with a tree operation (zt 0.51, PRD #228) is one start-state batch: it previews in
        // `items` and commits only under the plan hash its own preview returns.
        let tree = items
            .iter()
            .any(|item| item["tool"].as_str().is_some_and(|tool| TREE_TOOLS.contains(&tool)));
        let request = if tree && preview {
            json!({"preview":true,"lean":lean,"items":items})
        } else if tree {
            let planned = zt.request("apply_batch", json!({"preview":true,"items":items}))?;
            if let Some(message) = reply_error(&planned) {
                bail!("zt: identified chunk starting at call {start}: {message}");
            }
            let plan: Value = serde_json::from_str(&reply_text(&planned)).context("apply_batch preview is not JSON")?;
            if plan["complete"] != true {
                bail!("zt: identified chunk starting at call {start} has refused or blocked items: {}", util::compact(&plan["refusals"]));
            }
            json!({"items":items,"confirm":plan["plan_hash"]})
        } else if preview && lean {
            json!({"preview":true,"lean":true,"preview_items":items})
        } else if preview {
            json!({"preview":true,"preview_items":items})
        } else {
            json!({"items":items})
        };
        let raw=zt.request("apply_batch",request).with_context(||format!("identified chunk starting at call {start}; repeat the same accepted input and OperationPrefix to recover"))?;
        drop(concept_lock);
        // R338: an atomic chunk that the archive refused keeps its typed refusal, so the stage stops
        // on `card_locked` instead of reading a permission change or a provider fault into it. The
        // chunk itself wrote nothing; the earlier chunks' receipts are already committed.
        if let Some(message) = reply_error(&raw) {
            let refusal = refusal_of(&raw);
            let said = typed_message(&message, &refusal);
            match &refusal {
                Some(refusal) if crate::lockcheck::is_planning_stop(refusal) => {
                    // The typed refusal travels as itself (R338, S16-AC05): a stage stops the
                    // affected write and answers the archive's own JSON, not a sentence.
                    return Err(anyhow::Error::new(crate::lockcheck::Stopped(refusal.clone())));
                }
                Some(refusal) => bail!("zt: {said} {}", util::compact(refusal)),
                None => bail!("zt: {said}"),
            }
        }
        let reply: Value = serde_json::from_str(&reply_text(&raw))
            .context("incomplete identified batch: the answer is not JSON")?;
        // A preview collects a refused item instead of failing (D5): a planning stop it reports ends
        // the run here, with the archive's own JSON, before any chunk is written.
        if let Some(refusal) = reply["refusals"].as_array().and_then(|r| r.first()).map(|r| r["refusal"].clone())
            && crate::lockcheck::is_planning_stop(&refusal)
        {
            return Err(anyhow::Error::new(crate::lockcheck::Stopped(refusal)));
        }
        // A preview collects a refused item instead of failing (D5): a planning stop it reports ends
        // the run here, with the archive's own JSON, before any chunk is written.
        if let Some(refusal) = reply["refusals"].as_array().and_then(|r| r.first()).map(|r| r["refusal"].clone())
            && crate::lockcheck::is_planning_stop(&refusal)
        {
            return Err(anyhow::Error::new(crate::lockcheck::Stopped(refusal)));
        }
        let results = reply["results"]
            .as_array()
            .context("incomplete identified batch: missing results")?;
        if results.len() != count {
            bail!(
                "incomplete identified batch: {} of {count} results",
                results.len()
            );
        }
        for (offset, result) in results.iter().enumerate() {
            let index = start + offset;
            if result["client_ref"].as_str() != Some(index.to_string().as_str())
                || result["op_key"].as_str() != Some(format!("{prefix}/item-{index:06}").as_str())
                || !result["replayed"].is_boolean()
            {
                bail!(
                    "incomplete identified batch: result identity/order mismatch at call {index}"
                );
            }
            let address = result["result"]["address"]
                .as_str()
                .filter(|s| !s.is_empty())
                .context("incomplete identified batch: no allocated address")?;
            resolved.push(address.to_string());
            done.push(result.clone());
        }
        if preview {
            draft = Some(reply);
        }
    }
    if !preview && verify {
        verify_identified(zt, &done)?;
    }
    Ok((done, draft))
}

/// Validate the real transaction draft, then run only the Link paragraph rules.
/// No preview result is a receipt or a guarantee that a later base still matches.
fn precheck_identified(
    zt: &mut Zt,
    calls: &[Value],
    prefix: &str,
) -> Result<Vec<String>> {
    let bytes: usize = calls
        .iter()
        .map(|c| {
            ["title", "body", "text", "bibtex", "base"]
                .iter()
                .map(|k| c["arguments"][k].as_str().map(str::len).unwrap_or(0))
                .sum::<usize>()
                + c["arguments"]["owned_paragraph"]["text"]
                    .as_str()
                    .map(str::len)
                    .unwrap_or(0)
        })
        .sum();
    if calls.is_empty()
        || calls.len() > 64
        || bytes > 1024 * 1024
        || calls
            .iter()
            .any(|c| c["tool"] != "edit_card" || !c["arguments"]["owned_paragraph"].is_object())
    {
        bail!(
            "CheckOnly requires 1 through 64 receipt-owned paragraph edits within one 1048576-byte batch; check each accepted Link draft separately"
        );
    }
    let (receipts, draft) = execute_identified(zt, calls, prefix, true, false, None, false)?;
    let draft = draft.context("incomplete Link draft: no preview")?;
    if draft["preview"] != true || draft["applied"] != false || draft["complete"] != true {
        bail!("incomplete Link draft: explicit complete, unapplied preview required");
    }
    let revision = draft["archive_revision"]
        .as_u64()
        .context("incomplete Link draft: missing revision")?;
    let mut expected = BTreeMap::new();
    for receipt in &receipts {
        expected.insert(
            receipt["result"]["address"]
                .as_str()
                .context("incomplete Link draft address")?,
            receipt["result"]["after_fingerprint"]
                .as_str()
                .context("incomplete Link draft fingerprint")?,
        );
    }
    let cards = draft["cards"]
        .as_array()
        .context("incomplete Link draft: missing Cards")?;
    if cards.len() != expected.len() {
        bail!("incomplete Link draft: wrong Card count");
    }
    for card in cards {
        let address = card["address"]
            .as_str()
            .context("incomplete Link draft Card address")?;
        if card["workspace"] != "agent"
            || expected.remove(address) != Some(authored_fingerprint(card).as_str())
        {
            bail!("incomplete Link draft: authored output mismatch at `{address}`");
        }
    }
    if !expected.is_empty() {
        bail!("incomplete Link draft: {} accepted outputs were not previewed", expected.len());
    }
    let paragraphs = draft["paragraphs"]
        .as_array()
        .context("incomplete Link draft: missing paragraphs")?;
    if paragraphs.len() != receipts.len() {
        bail!("incomplete Link draft: wrong paragraph count");
    }
    let mut entries = Vec::new();
    for (paragraph, receipt) in paragraphs.iter().zip(&receipts) {
        let address = receipt["result"]["address"].as_str().unwrap();
        if paragraph["address"] != address
            || paragraph["body"] != receipt["result"]["owned_paragraph"]["text"]
            || !paragraph["body"].is_string()
            || ["links", "kinds"].iter().any(|key| {
                paragraph[key]
                    .as_array()
                    .is_none_or(|values| values.iter().any(|v| !v.is_string()))
            })
        {
            bail!("incomplete Link draft: paragraph identity or Link facts mismatch");
        }
        let title = cards
            .iter()
            .find(|card| card["address"] == address)
            .and_then(|card| card["title"].as_str())
            .context("incomplete Link draft: missing Card title")?;
        entries.push(json!({"address":address,"title":title,"body":paragraph["body"],"links":paragraph["links"],"kinds":paragraph["kinds"],"category":"idea"}));
    }
    // The preview is one transaction's own projection, so its completeness and fingerprints
    // above are the whole check. A later empty read would refuse a preview merely because
    // another client committed unrelated work; the draft names its own revision instead.
    let mut file = tempfile::NamedTempFile::new()?;
    serde_json::to_writer(&mut file, &entries)?;
    std::io::Write::flush(&mut file)?;
    let check = crate::check::Check {
        json: file.path().to_string_lossy().into_owned(),
        ..Default::default()
    };
    let checked = zt_check::check::run_lines(&check.args(), None)
        .context("Link draft checker did not run")?;
    if !checked.iter().any(|line| {
        line.strip_prefix("faults: ")
            .is_some_and(|count| count.parse::<usize>().is_ok())
    }) {
        bail!("Link draft checker did not report completeness");
    }
    let mut faults: Vec<_> = checked
        .into_iter()
        .filter(|line| {
            line.split_whitespace().nth(1).is_some_and(|rule| {
                ["L1", "L2", "R2", "R3", "R4", "R5", "R7", "R9", "R18", "R19"].contains(&rule)
            })
        })
        .collect();
    // The published `(advisory)` tag is the one advisory decision (S06, R0187): a
    // caller counts blockers, never every printed line.
    let count = faults
        .iter()
        .filter(|line| !line.contains("(advisory)"))
        .count();
    faults.insert(0, format!("preview: not_applied revision={revision}"));
    faults.push(format!("faults: {count}"));
    Ok(faults)
}
/// A receipt proves a historical commit. Completion additionally needs current
/// authored outputs to match the final accepted write for each address, seen in one
/// coherent read. Another client's commit is not this unit's fault: the archive
/// revision moves for unrelated work, so only the unit's own outputs are compared.
fn verify_identified(zt: &mut Zt, receipts: &[Value]) -> Result<()> {
    let mut expected = BTreeMap::new();
    for receipt in receipts {
        let result = &receipt["result"];
        // A tree operation (zt 0.51, PRD #228) carries no fingerprint: it may rename, delete, or
        // rewrite the Links of the outputs before it, so only the outputs after it are compared.
        if result.get("after_fingerprint").is_none() && result.get("moved").is_some() {
            expected.clear();
            continue;
        }
        expected.insert(
            result["address"]
                .as_str()
                .context("receipt missing address")?
                .to_string(),
            result["after_fingerprint"]
                .as_str()
                .context("receipt missing authored fingerprint")?
                .to_string(),
        );
    }
    if expected.is_empty() {
        return Ok(());
    }
    let read = zt.cards(&expected.keys().cloned().collect::<Vec<_>>(), "text")?;
    verified_outputs(&expected, &read.cards)
}

/// Every accepted output must be present, in the agent Workspace, with exactly its
/// accepted authored fingerprint. The count check makes a deleted output a refusal
/// instead of a silently skipped one, so a separate whole-archive revision refresh is
/// unnecessary — and would wrongly refuse a unit whose neighbour committed meanwhile.
fn verified_outputs(expected: &BTreeMap<String, String>, cards: &[Value]) -> Result<()> {
    if cards.len() != expected.len() {
        bail!(
            "incomplete identified output verification: {} of {} accepted outputs were returned",
            cards.len(),
            expected.len()
        );
    }
    let mut remaining = expected.clone();
    for card in cards {
        let address = card["address"]
            .as_str()
            .context("incomplete identified output address")?;
        let fingerprint = authored_fingerprint(card);
        if card["workspace"] != "agent" || remaining.remove(address).as_deref() != Some(&fingerprint)
        {
            bail!(
                "identified output changed: `{address}` no longer matches its accepted receipt; keep the current Card and use a newly accepted plan revision for a correction"
            );
        }
    }
    if !remaining.is_empty() {
        bail!(
            "incomplete identified output verification: {} accepted outputs were not returned",
            remaining.len()
        );
    }
    Ok(())
}

/// The archive receipt projection uses standard JSON and excludes generated text. A nonempty term
/// name joins it in the same field convention the service uses; a Card without one keeps exactly
/// its old projection, so retained receipts stay comparable (S07-AC06, R0245).
pub(crate) fn authored_fingerprint(card: &Value) -> String {
    let model = card["type"] == "model";
    let hub = card["type"] == "hub";
    let mut authored = json!({"title":card["title"],"body":card["body"],"bibtex":card["bibtex"],"type":card["type"],"facet":card["facet"],"emphasis":card["emphasis"],"level":if model{card["level"].clone()}else{Value::Null},"hub_level":if hub{card["level"].clone()}else{Value::Null},"form":card["form"]});
    if let Some(term) = card["term"].as_str().filter(|term| !term.is_empty()) {
        authored["term"] = json!(term);
    }
    crate::canon::sha256_hex(crate::canon::canonical(&authored).to_string().as_bytes())
}

/// `zt-digest call` (the Literature scope) and `zt-digest open-call` (the open scope).
pub fn cli(argv: &[String], open: bool) -> Result<i32> {
    let a = Args::parse(
        argv,
        &["File", "From", "Addresses", "Log", "Allow", "Beneath", "Moves", "SrcLog", "Tool", "Json", "OperationPrefix", "ConceptGate"],
        &["CheckOnly", "Open"],
    )?;
    if a.flag("Open") {
        // D10: the open scope is a command of its own, so a job cannot widen its scope by a switch.
        bail!(if open {
            "open-call is already the open scope; drop -Open"
        } else {
            "the helper has no -Open switch; hand work on Topic trees runs `zt-digest open-call` with the same arguments"
        });
    }
    let declared = scope_lists(&a.str("Allow"), &a.str("Beneath"))?;
    let calls: Vec<Value> = if a.has("Tool") {
        let arguments: Value = serde_json::from_str(&a.str_or("Json", "{}")).context("-Json is not JSON")?;
        vec![json!({ "tool": a.str("Tool"), "arguments": arguments })]
    } else {
        let file = if a.has("File") { a.str("File") } else { a.positional.first().cloned().context("give a file, or - for stdin")? };
        let source = if file == "-" {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)?;
            text.strip_prefix('\u{FEFF}').map(str::to_string).unwrap_or(text)
        } else {
            util::read_text(&file)?
        };
        match serde_json::from_str::<Value>(&source).context("the calls are not JSON")? {
            Value::Array(list) => list,
            one => vec![one],
        }
    };
    if a.flag("CheckOnly") && !a.has("OperationPrefix") {
        bail!("CheckOnly requires OperationPrefix and a receipt-owned Link draft");
    }
    if a.has("OperationPrefix") {
        if ["From","Addresses","Log","Moves","SrcLog"].iter().any(|name|a.has(name)) {
            bail!("OperationPrefix replays accepted writes from the beginning and cannot combine with legacy resume, logs or moves");
        }
        let mut zt=crate::zt::start_with(&declared)?;
        if a.flag("CheckOnly") {
            let lines=precheck_identified(&mut zt,&calls,&a.required("OperationPrefix")?)?;
            let code=if lines.last().is_some_and(|line|line=="faults: 0") {0}else{1};
            util::print_lines(&lines)?;
            return Ok(code);
        }
        // S07 (#100): a concept decision may accompany an identified Topic creation. It is the one
        // combination the legacy Topic gate options cannot express, so it is permitted here and
        // nowhere else; the decision file is checked against the live concept inputs at the write.
        let results = if a.has("ConceptGate") {
            let concept = crate::canon::read_json(&a.required("ConceptGate")?)?;
            crate::gate::check_decision(&concept)?;
            invoke_identified_gated(&mut zt,&calls,&a.required("OperationPrefix")?,&concept)?
        } else {
            invoke_identified(&mut zt,&calls,&a.required("OperationPrefix")?)?
        };
        let lines:Vec<_>=results.iter().enumerate().map(|(index,result)|format!("{index} ok {}",result["result"]["address"].as_str().unwrap())).collect();
        util::print_lines(&lines)?;
        return Ok(0);
    }
    if a.has("ConceptGate") {
        bail!("ConceptGate requires OperationPrefix and a receipt-owned Topic creation");
    }
    let mut options = Options::new();
    options.from = a.int_or("From", 0)?.max(0) as usize;
    options.addresses = a.str("Addresses").split(',').map(|s| s.trim().to_string()).collect();
    options.log = a.str("Log");
    options.moves = a.str("Moves");
    options.src_log = a.str("SrcLog");
    let mut zt = crate::zt::start_with(&declared)?;
    let (done, failure) = run(&mut zt, &calls, &mut options)?;
    let mut lines: Vec<String> = done.into_iter().map(|d| d.line).collect();
    let code = match &failure {
        Some(failure) => {
            lines.extend(failure.lines());
            1
        }
        None => 0,
    };
    util::print_lines(&lines)?;
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_resolve_and_fail_without_address() {
        let resolved = vec!["3/1".to_string(), String::new()];
        let v = resolve_refs(&json!({ "at": "@@0", "body": ["see [[@@0]]"] }), &resolved).unwrap();
        assert_eq!(v, json!({ "at": "3/1", "body": ["see [[3/1]]"] }));
        assert_eq!(resolve_refs(&json!("@@1"), &resolved).unwrap_err().to_string(), "@@1 has no address yet");
        assert!(resolve_refs(&json!("@@7"), &resolved).is_err());
    }

    fn card(address: &str, body: &str, workspace: &str) -> Value {
        json!({"address":address,"title":"A part","body":body,"bibtex":"","type":"model",
            "facet":"设定","emphasis":"","level":"formal","hub_level":"","form":"","workspace":workspace})
    }

    #[test]
    fn a_typed_refusal_keeps_its_json_and_the_resume_line_ends_the_output() {
        let refusal = json!({"error":"card_locked","message":"card `K/1` is inside locked tree `K`; a locked tree takes no new Cards",
            "tree":"K","cards":["K/1"],"reason":"create-inside","hint":"unlock the tree in the GUI first",
            "locks":[{"lock_id":2,"tree":"K"}]});
        let reply = json!({"result":{"isError":true,"content":[{"type":"text","text":"..."}],"structuredContent":refusal}});
        let got = refusal_of(&reply).expect("a structured refusal");
        assert_eq!(got, refusal);
        let failure = Failure {
            index: 1,
            message: typed_message("card_locked", &Some(got.clone())),
            resolved: vec!["0/0".into()],
            refusal: Some(got),
        };
        let lines = failure.lines();
        assert_eq!(lines.len(), 3, "{lines:?}");
        assert_eq!(lines[0], "1 error card_locked: card `K/1` is inside locked tree `K`; a locked tree takes no new Cards");
        assert!(lines[1].starts_with("card_locked {"), "{}", lines[1]);
        let quoted = lines[1].strip_prefix("card_locked ").unwrap();
        let parsed: Value = serde_json::from_str(quoted).expect("the structured line is JSON");
        assert_eq!(parsed["tree"], json!("K"));
        assert_eq!(parsed["reason"], json!("create-inside"));
        assert_eq!(parsed["locks"][0]["lock_id"], json!(2));
        assert_eq!(lines[2], "resume: -From 1 -Addresses 0/0");

        // A transport-level error reply carries no structured refusal: its text is all there is.
        let plain = json!({"error":{"code":-32602,"message":"unknown tool `nope`"}});
        assert_eq!(refusal_of(&plain), None);
        assert_eq!(
            typed_message("unknown tool `nope`", &None),
            "unknown tool `nope`"
        );
        let quiet = Failure { index: 0, message: "unknown tool `nope`".into(), resolved: vec![], refusal: None };
        assert_eq!(quiet.lines(), vec!["0 error unknown tool `nope`", "resume: -From 0 -Addresses "]);
    }

    #[test]
    fn a_card_check_refusal_keeps_its_blocking_fault_lines_for_the_fix_cycle() {
        // S27 (#166, R0447): a refused write is content, not a provider fault. The refusal's own
        // blocking fault lines travel with the failure, so a writer reads the lines it must fix.
        let refusal = json!({"error":"card_check","address":"K/1","blocking":1,
            "faults":["K/1 R3 分号把两句连在一起。","K/1 R2 (advisory) 一句很长的话。"],
            "message":"1 blocking fault"});
        let reply = json!({"result":{"isError":true,"content":[{"type":"text","text":"..."}],"structuredContent":refusal}});
        let got = refusal_of(&reply).expect("a structured refusal");
        let failure = Failure {
            index: 0,
            message: typed_message("1 blocking fault", &Some(got.clone())),
            resolved: vec![],
            refusal: Some(got),
        };
        assert_eq!(failure.refusal_fault_lines(), vec!["K/1 R3 分号把两句连在一起。".to_string()]);
        let lines = failure.lines();
        assert_eq!(lines[0], "0 error card_check: 1 blocking fault");
        assert!(lines[1].starts_with("card_check {"), "{}", lines[1]);
        assert_eq!(lines[2], "K/1 R3 分号把两句连在一起。");
        assert_eq!(lines.len(), 4, "{lines:?}");
        assert_eq!(lines[3], "resume: -From 0 -Addresses ");

        // Any other refusal has no fault lines; a Lock keeps exactly its old three lines.
        let locked = json!({"error":"card_locked","message":"locked","tree":"K"});
        let failure = Failure { index: 1, message: "locked".into(), resolved: vec![], refusal: Some(locked) };
        assert!(failure.refusal_fault_lines().is_empty());
        assert_eq!(failure.lines().len(), 3, "{:?}", failure.lines());
    }

    #[test]
    fn accepted_outputs_must_all_return_unchanged() {
        let first = card("9/1", "第一版正文。", "agent");
        let second = card("9/1|a", "第二步正文。", "agent");
        let mut expected = BTreeMap::new();
        expected.insert("9/1".to_string(), authored_fingerprint(&first));
        expected.insert("9/1|a".to_string(), authored_fingerprint(&second));

        // One coherent read that returns every accepted output unchanged completes, whatever
        // another client did to the archive revision meanwhile (that fact is not passed in).
        verified_outputs(&expected, &[first.clone(), second.clone()]).unwrap();

        // A changed output is refused with the correction rule.
        let edited = card("9/1", "另一版正文。", "agent");
        let changed = verified_outputs(&expected, &[edited, second.clone()]).unwrap_err().to_string();
        assert!(changed.starts_with("identified output changed: `9/1`"), "{changed}");

        // A deleted output is a refusal, not a silently skipped address.
        let missing = verified_outputs(&expected, &[second.clone()]).unwrap_err().to_string();
        assert!(missing.contains("1 of 2 accepted outputs were returned"), "{missing}");

        // So is a reply that repeats one address instead of returning the other.
        let repeated = verified_outputs(&expected, &[first.clone(), first.clone()]).unwrap_err().to_string();
        assert!(repeated.contains("no longer matches its accepted receipt"), "{repeated}");

        // And a human Card is never accepted as an agent unit's output.
        let human = card("9/1", "第一版正文。", "human");
        let foreign = verified_outputs(&expected, &[human, second]).unwrap_err().to_string();
        assert!(foreign.starts_with("identified output changed: `9/1`"), "{foreign}");
    }

    /// S07-AC06/R0245: a nonempty Term name joins the receipt projection in the service's field
    /// convention, and a Card without one keeps exactly its old projection. The two hex values are
    /// the service test's fixture, so both implementations are checked against one contract.
    #[test]
    fn a_nonempty_term_name_joins_the_receipt_projection_only() {
        let card = json!({"title":"均衡","body":"定义：","bibtex":null,"type":"term","facet":null,"emphasis":null,"level":null,"hub_level":null,"form":null,"term":"均衡"});
        assert_eq!(
            authored_fingerprint(&card),
            "35aaae5adf8cfe486781b0c8ddc8728f701119292b4ba93cbc1083ec8969b423"
        );
        let mut list = card.clone();
        list["term"] = json!(null);
        assert_eq!(
            authored_fingerprint(&list),
            "2caa06f0e57fa7874839259b3bfa5f8d05e60c75c6266198a57869ad03726cee"
        );
        // An empty string is no name too.
        list["term"] = json!("");
        assert_eq!(
            authored_fingerprint(&list),
            "2caa06f0e57fa7874839259b3bfa5f8d05e60c75c6266198a57869ad03726cee"
        );
    }
}
