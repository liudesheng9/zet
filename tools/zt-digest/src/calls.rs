//! The zt helper (`zt-digest call`), the Rust form of `skills/zt/scripts/zt-mcp.ps1`.
//!
//! It runs a JSON array of calls (or one call) through one `zt mcp` in order
//! and prints one compact line per call: `N ok <address>` for writes,
//! `N <json>` for reads, `N error <message>` on failure, after which the last
//! line is `resume: -From N -Addresses a0,a1,...` and the exit code is 1.
//!
//!   `@@N`      in any string argument: the address call N returned (0-based).
//!   -From N -Addresses a0,...   skip the calls before N; their addresses still resolve `@@`.
//!   -Log f     append the address of every Card a create call makes.
//!   -Allow f   only those Cards (one per line) and Cards made in this run may change;
//!              create_topic, move_card, insert_card, delete_card, create_literature,
//!              and edit_literature_metadata are refused.
//!   -Moves f   append {"i":N,"moved":{...}} per call that renumbered Cards.
//!   -SrcLog f  append `<address>\t<src>` per Card made from a call with a `src` field
//!              (the helper removes `src` before the call reaches zt).
//!   -TopicGate f   lock f around create_topic and refuse a Topic whose title equals or
//!              resembles (character bigrams, -TopicSimilarity 0.8) an agent Topic.
//!   append_card {at, text}: add a paragraph to the end of a Card (helper-only call).
//!
//! The digest tools run the same code in-process through `run` and `invoke`.

use crate::args::Args;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use zt_check::mcp::{reply_error, reply_text};

#[derive(Default)]
pub struct Options {
    pub from: usize,
    pub addresses: Vec<String>,
    pub log: String,
    pub allow: Option<HashSet<String>>,
    /// Earlier agent Regular Cards this run may continue with a Side successor (S06, #103). The
    /// earlier Card never becomes writable: only a new Card beneath it is created.
    pub beneath: HashSet<String>,
    pub moves: String,
    pub src_log: String,
    pub topic_gate: String,
    pub topic_similarity: f64,
}

impl Options {
    pub fn new() -> Self {
        Self { topic_similarity: 0.8, ..Self::default() }
    }

    /// Loads the allow-list file: one address per line.
    pub fn allow_file(mut self, path: &str) -> Result<Self> {
        if !path.is_empty() {
            let text = util::read_text(path)?;
            self.allow = Some(text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect());
        }
        Ok(self)
    }

    /// Loads the place-beside file: one earlier agent Regular Card address per line.
    pub fn beneath_file(mut self, path: &str) -> Result<Self> {
        if !path.is_empty() {
            let text = util::read_text(path)?;
            self.beneath = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect();
        }
        Ok(self)
    }
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
}

impl Failure {
    pub fn lines(&self) -> Vec<String> {
        vec![
            format!("{} error {}", self.index, one_line(&self.message)),
            format!("resume: -From {} -Addresses {}", self.index, self.resolved.join(",")),
        ]
    }
}

static WS_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s*\r?\n\s*").unwrap());
static REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"@@(\d+)").unwrap());
static TITLE_DROP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s\p{P}\p{S}]").unwrap());

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

/// A title without spaces and punctuation, in lower case: the Topic gate compares them.
pub fn title_key(title: &str) -> String {
    TITLE_DROP.replace_all(title, "").to_lowercase()
}

/// The Dice coefficient of the character bigrams of two keys.
pub fn similarity(a: &str, b: &str) -> f64 {
    let grams = |t: &str| {
        let chars: Vec<char> = t.chars().collect();
        chars.windows(2).map(|w| (w[0], w[1])).collect::<HashSet<_>>()
    };
    let (x, y) = (grams(a), grams(b));
    if x.is_empty() || y.is_empty() {
        return if a == b { 1.0 } else { 0.0 };
    }
    let common = x.iter().filter(|g| y.contains(g)).count();
    2.0 * common as f64 / (x.len() + y.len()) as f64
}

/// The Topic gate's lock: the file opened with no sharing, for at most 120 seconds of waiting.
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
            return Ok((done, Some(Failure { index: $i, message: $message.to_string(), resolved })))
        };
    }
    for (i, call) in calls.iter().enumerate().skip(options.from) {
        let call_tool = call["tool"].as_str().unwrap_or_default().to_string();
        let mut arguments = match resolve_refs(call.get("arguments").filter(|a| !a.is_null()).unwrap_or(&json!({})), &resolved) {
            Ok(a) => a,
            Err(error) => fail!(i, error),
        };
        if let Some(allowed) = &options.allow {
            if ["create_topic", "move_card", "insert_card", "delete_card", "create_literature", "edit_literature_metadata"]
                .contains(&call_tool.as_str())
            {
                fail!(i, format!("{call_tool} is not allowed in this run"));
            }
            let at = arg_str(&arguments, "at");
            // S06 (#103, D100/D104): a standalone claim may continue an earlier agent Regular Card
            // that this run read in full, as one new Side successor. Nothing of the earlier Card
            // becomes writable, and a Direct successor would extend its chain, so only `side` is
            // accepted.
            let beside = call_tool == "create_successor"
                && options.beneath.contains(&at)
                && arguments["kind"].as_str() == Some("side");
            if call_tool == "create_successor" && options.beneath.contains(&at) && !beside {
                fail!(i, format!("file a new Card beside `{at}` only as a Side successor; its Direct chain belongs to its own line of thought"));
            }
            if ["edit_card", "append_card", "create_successor"].contains(&call_tool.as_str())
                && !allowed.contains(&at)
                && !beside
            {
                fail!(i, format!("`{at}` is not one of your Cards; you may only change your own Cards"));
            }
        }
        let mut tool = call_tool.clone();
        if tool == "append_card" {
            let at = arg_str(&arguments, "at");
            let current = zt.request("get_card", json!({ "at": at }))?;
            if reply_error(&current).is_some() {
                fail!(i, format!("cannot read `{at}`"));
            }
            let body = arg_str(&current["result"]["structuredContent"], "body");
            let text = arg_str(&arguments, "text");
            arguments = json!({ "at": at, "body": format!("{}\n\n{}", body.trim_end(), text.trim()) });
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
        let mut gate = None;
        if !options.topic_gate.is_empty() && tool == "create_topic" {
            let lock = match lock_gate(&options.topic_gate) {
                Ok(lock) => lock,
                Err(error) => fail!(i, error),
            };
            let listed = zt.request("list_trees", json!({ "workspace": "agent" }))?;
            let key = title_key(&arg_str(&arguments, "title"));
            let trees = listed["result"]["structuredContent"]["trees"].as_array().cloned().unwrap_or_default();
            let same = trees.iter().filter(|t| t["kind"] == "topic").find(|t| {
                let other = title_key(&arg_str(t, "title"));
                other == key || similarity(&other, &key) >= options.topic_similarity
            });
            if let Some(same) = same {
                drop(lock);
                fail!(
                    i,
                    format!(
                        "topic exists: `{}` {}. File the Card under that Topic instead of creating one.",
                        arg_str(same, "root"),
                        arg_str(same, "title")
                    )
                );
            }
            gate = Some(lock);
        }
        let reply = zt.request(&tool, arguments)?;
        drop(gate);
        let text = reply_text(&reply);
        if let Some(message) = reply_error(&reply) {
            fail!(i, message);
        }
        let structured = &reply["result"]["structuredContent"];
        let address = structured["address"].as_str().map(str::to_string);
        resolved.push(address.clone().unwrap_or_default());
        if let Some(address) = &address
            && call_tool.starts_with("create")
        {
            if let Some(allowed) = options.allow.as_mut() {
                allowed.insert(address.clone());
            }
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
    allow: Option<&HashSet<String>>,
) -> Result<Vec<Value>> {
    Ok(execute_identified(zt, calls, prefix, allow, false, None)?.0)
}

/// Apply an immutable accepted call list whose Topic creation was cleared by a concept decision
/// (S07, #100). A committed operation returns its receipt before any new concept decision; an
/// uncommitted `create_topic` takes the archive-shared Topic gate, revalidates the decision and
/// holds the lock until its receipt is known (D146–D153).
pub(crate) fn invoke_identified_gated(
    zt: &mut Zt,
    calls: &[Value],
    prefix: &str,
    allow: Option<&HashSet<String>>,
    concept: &Value,
) -> Result<Vec<Value>> {
    Ok(execute_identified(zt, calls, prefix, allow, false, Some(concept))?.0)
}

/// Validate a complete accepted sequence against real archive validators. The
/// server simulates normal write chunks, then rolls back before returning.
pub(crate) fn preview_identified(
    zt: &mut Zt,
    calls: &[Value],
    prefix: &str,
    allow: Option<&HashSet<String>>,
) -> Result<Value> {
    let (receipts, draft) = execute_identified(zt, calls, prefix, allow, true, None)?;
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
    allow: Option<&HashSet<String>>,
    preview: bool,
    concept: Option<&Value>,
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
        if !matches!(
            call["tool"].as_str(),
            Some("create_topic" | "create_literature" | "create_successor" | "edit_card")
        ) {
            bail!("identified call {index}: unsupported write tool");
        }
        if !call["arguments"].is_object() {
            bail!("identified call {index}: arguments must be an object");
        }
        if let Some(allowed) = allow {
            if matches!(
                call["tool"].as_str(),
                Some("create_topic" | "create_literature")
            ) {
                bail!(
                    "identified call {index}: {} is not allowed in this run",
                    call["tool"]
                );
            }
            let at = call["arguments"]["at"].as_str().unwrap_or_default();
            let earlier = at
                .strip_prefix("@@")
                .and_then(|n| n.parse::<usize>().ok())
                .is_some_and(|n| n < index);
            if !allowed.contains(at) && !earlier {
                bail!(
                    "identified call {index}: `{at}` is not allowed; use an owned address or an exact earlier result reference"
                );
            }
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
        let request = if preview {
            json!({"preview":true,"preview_items":items})
        } else {
            json!({"items":items})
        };
        let reply=zt.call("apply_batch",request).with_context(||format!("identified chunk starting at call {start}; repeat the same accepted input and OperationPrefix to recover"))?;
        drop(concept_lock);
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
    if !preview {
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
    allow: Option<&HashSet<String>>,
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
    let (receipts, draft) = execute_identified(zt, calls, prefix, allow, true, None)?;
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
    let count = faults.len();
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

/// The archive receipt projection uses standard JSON and excludes generated text.
pub(crate) fn authored_fingerprint(card: &Value) -> String {
    let model = card["type"] == "model";
    let hub = card["type"] == "hub";
    let authored = json!({"title":card["title"],"body":card["body"],"bibtex":card["bibtex"],"type":card["type"],"facet":card["facet"],"emphasis":card["emphasis"],"level":if model{card["level"].clone()}else{Value::Null},"hub_level":if hub{card["level"].clone()}else{Value::Null},"form":card["form"]});
    crate::canon::sha256_hex(crate::canon::canonical(&authored).to_string().as_bytes())
}

/// `zt-digest call`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(
        argv,
        &["File", "From", "Addresses", "Log", "Allow", "Beneath", "Moves", "SrcLog", "TopicGate", "TopicSimilarity", "Tool", "Json", "OperationPrefix", "ConceptGate"],
        &["CheckOnly"],
    )?;
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
        if ["From","Addresses","Log","Moves","SrcLog","TopicGate","TopicSimilarity"].iter().any(|name|a.has(name)) {
            bail!("OperationPrefix replays accepted writes from the beginning and cannot combine with legacy resume, logs, moves or Topic-gate options");
        }
        let options=Options::new().allow_file(&a.str("Allow"))?;
        let mut zt=crate::zt::start()?;
        if a.flag("CheckOnly") {
            let lines=precheck_identified(&mut zt,&calls,&a.required("OperationPrefix")?,options.allow.as_ref())?;
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
            invoke_identified_gated(&mut zt,&calls,&a.required("OperationPrefix")?,options.allow.as_ref(),&concept)?
        } else {
            invoke_identified(&mut zt,&calls,&a.required("OperationPrefix")?,options.allow.as_ref())?
        };
        let lines:Vec<_>=results.iter().enumerate().map(|(index,result)|format!("{index} ok {}",result["result"]["address"].as_str().unwrap())).collect();
        util::print_lines(&lines)?;
        return Ok(0);
    }
    if a.has("ConceptGate") {
        bail!("ConceptGate requires OperationPrefix and a receipt-owned Topic creation");
    }
    let mut options = Options::new().allow_file(&a.str("Allow"))?.beneath_file(&a.str("Beneath"))?;
    options.from = a.int_or("From", 0)?.max(0) as usize;
    options.addresses = a.str("Addresses").split(',').map(|s| s.trim().to_string()).collect();
    options.log = a.str("Log");
    options.moves = a.str("Moves");
    options.src_log = a.str("SrcLog");
    options.topic_gate = a.str("TopicGate");
    options.topic_similarity = a.float_or("TopicSimilarity", 0.8)?;
    let mut zt = crate::zt::start()?;
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

    #[test]
    fn titles_compare_without_punctuation() {
        assert_eq!(title_key("Price, Theory!"), "pricetheory");
        assert_eq!(title_key("价格 理论：导论"), "价格理论导论");
        assert!(similarity("价格理论导论", "价格理论导引") >= 0.8);
        assert!(similarity("价格理论", "货币银行") < 0.8);
        assert_eq!(similarity("a", "a"), 1.0);
    }

    fn card(address: &str, body: &str, workspace: &str) -> Value {
        json!({"address":address,"title":"A part","body":body,"bibtex":"","type":"model",
            "facet":"设定","emphasis":"","level":"formal","hub_level":"","form":"","workspace":workspace})
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
}
