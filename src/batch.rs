//! Bounded identified writes within one archive transaction.

use anyhow::{Context, Result, bail};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug)]
pub(crate) struct Conflict {
    pub(crate) address: String,
    pub(crate) reason: String,
    pub(crate) observed_body: String,
    pub(crate) observed_fingerprint: String,
}

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at `{}`", self.reason, self.address)
    }
}
impl std::error::Error for Conflict {}
impl Conflict {
    pub(crate) fn value(&self) -> Value {
        json!({"error":self.to_string(),"code":"write_conflict","address":self.address,"observed_body":self.observed_body,"observed_fingerprint":self.observed_fingerprint,"complete":false})
    }
}

/// One resolved argument tree and every reference name that no successful item bound, so a preview
/// can tell a dependent item from an ordinary unresolved reference (R196).
struct Resolved {
    value: Value,
    missing: BTreeSet<String>,
}

impl Resolved {
    /// The ordinary failure contract: an unbound reference stops the request.
    fn checked(self) -> Result<Value> {
        match self.missing.into_iter().next() {
            Some(name) => bail!("unresolved earlier client reference `@@{name}`"),
            None => Ok(self.value),
        }
    }
}

fn resolve(value: &Value, refs: &BTreeMap<String, String>) -> Resolved {
    let mut missing = BTreeSet::new();
    let value = resolve_into(value, refs, &mut missing);
    Resolved { value, missing }
}

/// Resolve every reference an earlier successful item bound; an unbound reference keeps its literal
/// `@@name` and is reported, so nothing is ever fabricated for it.
fn resolve_into(
    value: &Value,
    refs: &BTreeMap<String, String>,
    missing: &mut BTreeSet<String>,
) -> Value {
    match value {
        Value::String(text) => {
            let mut rest = text.as_str();
            let mut out = String::new();
            while let Some(offset) = rest.find("@@") {
                out.push_str(&rest[..offset]);
                rest = &rest[offset + 2..];
                let end = rest
                    .bytes()
                    .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
                    .count();
                let name = &rest[..end];
                match refs.get(name) {
                    Some(address) => out.push_str(address),
                    None => {
                        missing.insert(name.to_string());
                        out.push_str("@@");
                        out.push_str(name);
                    }
                }
                rest = &rest[end..];
            }
            out.push_str(rest);
            Value::String(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|v| resolve_into(v, refs, missing))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), resolve_into(v, refs, missing)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn hash(value: &Value) -> String {
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), canonical(v)))
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .collect(),
            ),
            Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
            other => other.clone(),
        }
    }
    format!(
        "{:x}",
        Sha256::digest(canonical(value).to_string().as_bytes())
    )
}

fn request(tool: &str, arguments: &Value) -> Result<Value> {
    fn line_endings(value: &Value) -> Value {
        match value {
            Value::String(text) => json!(text.replace("\r\n", "\n")),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), line_endings(v)))
                    .collect(),
            ),
            Value::Array(items) => Value::Array(items.iter().map(line_endings).collect()),
            other => other.clone(),
        }
    }
    let arguments = crate::mcp::canonical_write_args(tool, &line_endings(arguments))?;
    Ok(json!({"tool":tool,"arguments":arguments}))
}

fn fingerprint(conn: &rusqlite::Connection, address: &str) -> Result<String> {
    let card = crate::load_card(conn, address)?;
    let parsed = crate::parse_card_text(&card.text)?;
    Ok(hash(
        &json!({"title":parsed.title,"body":parsed.body,"bibtex":card.bibtex,"type":card.card_type.id(),"facet":card.facet.map(|f|f.id().to_string()),"emphasis":card.emphasis.map(crate::model::Emphasis::id),"level":card.level.map(crate::model::Level::id),"hub_level":card.hub_level.map(crate::hub::HubLevel::id),"form":card.form.map(crate::math::MathForm::id)}),
    ))
}

fn guard_edit(conn: &rusqlite::Connection, args: &Value, current: &str) -> Result<()> {
    let address = args["at"].as_str().context("missing edit at")?;
    let literal = args.get("base_fingerprint");
    let receipt = args.get("base_receipt");
    if literal.is_none() && receipt.is_none() {
        bail!("identified edit requires an authored base_fingerprint or base_receipt");
    }
    if args.get("base").is_some() {
        bail!(
            "apply_batch uses an authored fingerprint or receipt; raw base belongs to single edit_card"
        );
    }
    if literal.is_some() && receipt.is_some() {
        bail!("use one authored base_fingerprint or base_receipt");
    }
    let mut expected = None;
    if let Some(value) = literal {
        expected = Some(
            value
                .as_str()
                .filter(|value| value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()))
                .context("base_fingerprint must be a 64-character authored SHA-256")?
                .to_ascii_lowercase(),
        );
    }
    if let Some(key) = receipt {
        let key = key
            .as_str()
            .context("base_receipt must be an operation key")?;
        let stored: Option<String> = conn
            .query_row(
                "SELECT result_json FROM applied_ops WHERE op_key=?1",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(stored) = stored {
            let result: Value = serde_json::from_str(&stored).context("invalid base receipt")?;
            if result["address"] == address {
                expected = result["after_fingerprint"].as_str().map(str::to_string);
            }
        }
        if expected.is_none() {
            expected = Some(String::new());
        }
    }
    if expected
        .as_deref()
        .is_some_and(|expected| expected != current)
    {
        let card = crate::load_card(conn, address)?;
        let reason = if expected.as_deref() == Some("") {
            "base receipt was not found for this Card"
        } else {
            "authored base changed"
        };
        return Err(Conflict {
            address: address.to_string(),
            reason: reason.into(),
            observed_body: crate::parse_card_text(&card.text)?.body,
            observed_fingerprint: current.to_string(),
        }
        .into());
    }
    Ok(())
}

fn paragraph_spans(body: &str, paragraph: &str) -> Vec<std::ops::Range<usize>> {
    let fenced = crate::link::fenced_code_ranges(body);
    let code = crate::link::code_ranges(body);
    body.match_indices(paragraph)
        .filter_map(|(start, _)| {
            let end = start + paragraph.len();
            let before = start == 0
                || (body[..start].ends_with('\n')
                    && body[..start]
                        .lines()
                        .last()
                        .is_none_or(|line| line.trim().is_empty()));
            let after = end == body.len()
                || body[end..]
                    .strip_prefix("\r\n")
                    .or_else(|| body[end..].strip_prefix('\n'))
                    .is_some_and(|rest| {
                        rest.lines()
                            .next()
                            .is_none_or(|line| line.trim().is_empty())
                    });
            let crosses = code.iter().any(|code| {
                (code.start < start && start < code.end) || (code.start < end && end < code.end)
            });
            (before
                && after
                && !crosses
                && !fenced
                    .iter()
                    .any(|code| code.start < end && start < code.end))
            .then_some(start..end)
        })
        .collect()
}

fn owned_paragraph(
    root: &Path,
    conn: &rusqlite::Connection,
    args: &Value,
    current: &str,
) -> Result<(Value, bool)> {
    for key in args
        .as_object()
        .context("paragraph arguments must be an object")?
        .keys()
    {
        if !["at", "base_fingerprint", "base_receipt", "owned_paragraph"].contains(&key.as_str()) {
            bail!("owned_paragraph cannot combine with argument `{key}`");
        }
    }
    let address = args["at"].as_str().context("missing paragraph at")?;
    let card = crate::load_card(conn, address)?;
    crate::Access::Agent.ensure_writable(card.workspace)?;
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Edit(card.row_id))?;
    let range = crate::card::authored_body_range(&card.text)?;
    let original_body = &card.text[range.clone()];
    let spec = args["owned_paragraph"]
        .as_object()
        .context("owned_paragraph must be an object")?;
    for key in spec.keys() {
        if !["text", "replaces_receipt", "adopt_legacy"].contains(&key.as_str()) {
            bail!("unknown owned_paragraph field `{key}`");
        }
    }
    let text = spec
        .get("text")
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .context("owned paragraph text must be nonempty")?;
    if text.starts_with(['\r', '\n'])
        || text.ends_with(['\r', '\n'])
        || text.lines().any(|line| line.trim().is_empty())
        || text.contains(crate::card::REVERSE_MARKER)
        || !crate::link::fenced_code_ranges(text).is_empty()
    {
        bail!(
            "owned paragraph text must be one paragraph without a generated marker or fenced code"
        );
    }
    let replaces = match spec.get("replaces_receipt") {
        None | Some(Value::Null) => None,
        Some(Value::String(key)) if !key.is_empty() => Some(key.as_str()),
        _ => bail!("replaces_receipt must be an operation key or null"),
    };
    let adopt = match spec.get("adopt_legacy") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        _ => bail!("adopt_legacy must be a boolean"),
    };
    let conflict = |reason: &str| {
        anyhow::Error::from(Conflict {
            address: address.to_string(),
            reason: reason.into(),
            observed_body: original_body.to_string(),
            observed_fingerprint: current.to_string(),
        })
    };
    let desired = paragraph_spans(original_body, text);
    let (body, adopted) = if let Some(key) = replaces {
        let stored: Option<String> = conn
            .query_row(
                "SELECT result_json FROM applied_ops WHERE op_key=?1",
                [key],
                |r| r.get(0),
            )
            .optional()?;
        let prior: Value = stored
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .unwrap_or(Value::Null);
        let old = prior["owned_paragraph"]["text"]
            .as_str()
            .filter(|s| !s.is_empty());
        if prior["address"] != address || old.is_none() {
            return Err(conflict(
                "the named receipt does not own a paragraph of this Card",
            ));
        }
        let old = old.unwrap();
        let spans = paragraph_spans(original_body, old);
        if spans.len() == 1 {
            if old != text && !desired.is_empty() {
                return Err(conflict(
                    "both the owned and desired paragraphs are present; review duplicate content",
                ));
            }
            let mut body = original_body.to_string();
            body.replace_range(spans[0].clone(), text);
            (body, false)
        } else if spans.is_empty() && adopt && desired.len() == 1 {
            (original_body.to_string(), true)
        } else {
            return Err(conflict(if spans.is_empty() {
                "owned paragraph is absent or changed"
            } else {
                "owned paragraph occurs more than once"
            }));
        }
    } else if adopt {
        if desired.len() != 1 {
            return Err(conflict(
                "verified legacy adoption requires exactly one desired paragraph",
            ));
        }
        (original_body.to_string(), true)
    } else {
        if !desired.is_empty() {
            return Err(conflict(
                "desired paragraph is already present; explicit verified legacy adoption is required",
            ));
        }
        (
            if original_body.is_empty() {
                text.to_string()
            } else {
                format!("{original_body}\n\n{text}")
            },
            false,
        )
    };
    if paragraph_spans(&body, text).len() != 1 {
        return Err(conflict(
            "desired text is not one standalone authored paragraph in the resulting Card",
        ));
    }
    crate::link_index::validate(conn, text, None)?;
    let changed = body != original_body;
    let mut accepted = crate::gate::Accepted::none();
    if changed {
        let mut written = card.text.clone();
        let inserted = if range.is_empty() {
            let prefix = &card.text[..range.start];
            let separator = if !prefix.ends_with('\n') {
                "\n\n"
            } else if prefix
                .lines()
                .last()
                .is_some_and(|line| !line.trim().is_empty())
            {
                "\n"
            } else {
                ""
            };
            format!("{separator}{body}\n\n")
        } else {
            body.clone()
        };
        written.replace_range(range, &inserted);
        crate::artifact::require_present(root, &written)?;
        accepted = crate::gate::check_proposed(
            conn,
            &crate::gate::Candidate::edit(
                &card,
                &written,
                card.card_type,
                card.facet.clone(),
                card.emphasis,
                card.level,
                card.hub_level,
                card.form,
            ),
        )?;
        conn.execute(
            "UPDATE cards SET text=?1 WHERE row_id=?2",
            rusqlite::params![written, card.row_id],
        )?;
    }
    let mut result = json!({"address":address,"owned_paragraph":{"address":address,"text":text,"replaces_receipt":replaces,"adopted":adopted}});
    accepted.attach(&mut result);
    Ok((result, changed))
}

fn authored_bytes(arguments: &Value) -> usize {
    ["title", "body", "text", "bibtex", "base"]
        .iter()
        .map(|name| arguments[name].as_str().map(str::len).unwrap_or(0))
        .sum::<usize>()
        + arguments["owned_paragraph"]["text"]
            .as_str()
            .map(str::len)
            .unwrap_or(0)
}

/// Match the thin client's concrete bindings for references to earlier chunks.
fn resolve_before(
    value: &Value,
    refs: &BTreeMap<String, String>,
    start: usize,
) -> Result<Resolved> {
    let mut missing = BTreeSet::new();
    let value = resolve_before_into(value, refs, start, &mut missing)?;
    Ok(Resolved { value, missing })
}

fn resolve_before_into(
    value: &Value,
    refs: &BTreeMap<String, String>,
    start: usize,
    missing: &mut BTreeSet<String>,
) -> Result<Value> {
    Ok(match value {
        Value::String(text) => {
            let mut rest = text.as_str();
            let mut out = String::new();
            while let Some(offset) = rest.find("@@") {
                out.push_str(&rest[..offset]);
                rest = &rest[offset + 2..];
                let end = rest.bytes().take_while(u8::is_ascii_digit).count();
                if end == 0 {
                    out.push_str("@@");
                    continue;
                }
                let name = &rest[..end];
                let n = name
                    .parse::<usize>()
                    .context("invalid numeric sequence reference")?;
                if n < start {
                    match refs.get(name) {
                        Some(address) => out.push_str(address),
                        None => {
                            missing.insert(name.to_string());
                            out.push_str("@@");
                            out.push_str(name);
                        }
                    }
                } else {
                    out.push_str("@@");
                    out.push_str(name);
                }
                rest = &rest[end..];
            }
            out.push_str(rest);
            json!(out)
        }
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|v| resolve_before_into(v, refs, start, missing))
                .collect::<Result<_>>()?,
        ),
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(k, v)| Ok((k.clone(), resolve_before_into(v, refs, start, missing)?)))
                .collect::<Result<_>>()?,
        ),
        other => other.clone(),
    })
}

pub(crate) fn apply(root: &Path, args: &Value) -> Result<Value> {
    let preview = match args.get("preview") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        _ => bail!("preview must be a boolean"),
    };
    let sequence = args.get("preview_items").is_some();
    if sequence && !preview {
        bail!("preview_items requires preview:true");
    }
    if sequence && args.get("items").is_some() {
        bail!("cannot combine items and preview_items");
    }
    let items = args[if sequence { "preview_items" } else { "items" }]
        .as_array()
        .context("missing array argument `items` or `preview_items`")?;
    let max_items = if sequence { 1024 } else { 64 };
    if items.is_empty() || items.len() > max_items {
        bail!(
            "a {} requires 1 through {max_items} items",
            if sequence {
                "preview sequence"
            } else {
                "batch"
            }
        );
    }
    let bytes: usize = items.iter().map(|i| authored_bytes(&i["arguments"])).sum();
    let max_bytes = if sequence {
        8 * 1024 * 1024
    } else {
        1024 * 1024
    };
    if bytes > max_bytes {
        bail!(
            "a {} is limited to {max_bytes} authored bytes; received {bytes}",
            if sequence {
                "preview sequence"
            } else {
                "batch"
            }
        );
    }
    for (index, item) in items.iter().enumerate() {
        let key = item["op_key"].as_str().context("missing item op_key")?;
        let parts: Vec<_> = key.split('/').collect();
        if parts.len() != 5
            || parts
                .iter()
                .any(|part| part.trim().is_empty() || part.chars().any(char::is_control))
            || parts[2].parse::<u64>().ok().filter(|n| *n > 0).is_none()
        {
            bail!(
                "op_key must have run/unit/positive-plan-revision/step/stable-item components; attempts are not key components"
            );
        }
        let name = item["client_ref"]
            .as_str()
            .context("missing item client_ref")?;
        if sequence && name != index.to_string() {
            bail!("preview sequence client_ref must be its zero-based item index");
        }
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            bail!(
                "client_ref must have 1 through 64 ASCII letters, digits, underscores or hyphens"
            );
        }
    }
    // R315: a batch client's declared patience applies to its batch writes too, not only to the
    // single-Card tools; a read or precheck keeps the ordinary short bound.
    let _lock = crate::acquire_edit_lock_within(
        root,
        crate::mcp::edit_patience(),
        "apply_batch",
    )?;
    let mut conn = crate::open_db(root)?;
    let mut tx = conn.transaction()?;
    let archive_revision = crate::link_index::revision(&tx)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS applied_ops(op_key TEXT NOT NULL PRIMARY KEY, request_hash TEXT NOT NULL, result_json TEXT NOT NULL, committed_at TEXT NOT NULL)")?;
    let simulated = simulate(&mut tx, root, items, sequence, preview);
    match simulated {
        // R198: the outer transaction is rolled back before any response leaves this module, the
        // ordinary failure contract's error included.
        Err(error) => {
            tx.rollback()?;
            Err(error)
        }
        Ok(simulated) if preview => {
            let value = simulated.value(archive_revision, true);
            tx.rollback()?;
            Ok(value)
        }
        Ok(simulated) => {
            let value = simulated.value(archive_revision, false);
            tx.commit()?;
            Ok(value)
        }
    }
}

/// What one simulated item produced. Only a successful item publishes its facts (R194).
enum ItemOutcome {
    /// The operation key already holds this identical request, so its stored result answers again.
    Replayed { address: String, result: Value },
    /// The ordinary receipt and writer path ran and wrote this item.
    Written {
        address: String,
        result: Value,
        wrote: bool,
    },
}

/// The facts one batch or preview gathered inside its transaction.
struct Simulated {
    results: Vec<Value>,
    refusals: Vec<Value>,
    blocked: Vec<Value>,
    cards: Vec<Value>,
    paragraphs: Vec<Value>,
}

impl Simulated {
    /// The reply shape: a real batch answers only its results, and a preview states whether it
    /// checked the whole plan, every ordered Lock refusal and every blocked item identity.
    fn value(self, archive_revision: u64, preview: bool) -> Value {
        if !preview {
            return json!({"results":self.results});
        }
        let scan = json!({
            "items": self.results.len() + self.refusals.len() + self.blocked.len(),
            "checked": self.results.len(),
            "refused": self.refusals.len(),
            "blocked": self.blocked.len(),
        });
        let complete = self.refusals.is_empty() && self.blocked.is_empty();
        json!({
            "results": self.results,
            "preview": true,
            "applied": false,
            "complete": complete,
            "archive_revision": archive_revision,
            "scan": scan,
            "refusals": self.refusals,
            "blocked": self.blocked,
            "cards": self.cards,
            "paragraphs": self.paragraphs,
        })
    }
}

/// Run the ordinary receipt and writer path over every item. Only a preview keeps going after a
/// Lock refusal, and only a preview simulates an item inside its own savepoint (R190).
fn simulate(
    tx: &mut rusqlite::Transaction<'_>,
    root: &Path,
    items: &[Value],
    sequence: bool,
    preview: bool,
) -> Result<Simulated> {
    let mut refs = BTreeMap::new();
    let mut chunk_refs = BTreeMap::new();
    // The identities whose reference no later item may resolve: a refused item, or one this refusal
    // already blocked (R196).
    let mut failed: BTreeMap<String, Value> = BTreeMap::new();
    let mut changed = BTreeSet::new();
    let mut results = Vec::new();
    let mut refusals = Vec::new();
    let mut blocked = Vec::new();
    let mut chunk_start = 0usize;
    let mut chunk_bytes = 0usize;
    let mut sequence_bytes = 0usize;
    for (index, item) in items.iter().enumerate() {
        let op_key = item["op_key"].as_str().context("missing item op_key")?;
        let client_ref = item["client_ref"]
            .as_str()
            .context("missing item client_ref")?;
        if refs.contains_key(client_ref) || failed.contains_key(client_ref) {
            bail!("duplicate client_ref `{client_ref}`");
        }
        let tool = item["tool"].as_str().context("missing item tool")?;
        // R192: resolve the earlier successful references, then begin the item's own savepoint.
        let (raw, bindings, mut missing) = if sequence {
            let mut normalized = resolve_before(&item["arguments"], &refs, chunk_start)?;
            if index - chunk_start >= 64
                || chunk_bytes + authored_bytes(&normalized.value) > 1024 * 1024
            {
                chunk_start = index;
                chunk_bytes = 0;
                chunk_refs.clear();
                normalized = resolve_before(&item["arguments"], &refs, chunk_start)?;
            }
            let bytes = authored_bytes(&normalized.value);
            if bytes > 1024 * 1024 {
                bail!("resolved sequence item {index} exceeds 1048576 authored bytes");
            }
            chunk_bytes += bytes;
            sequence_bytes += bytes;
            if sequence_bytes > 8 * 1024 * 1024 {
                bail!("resolved preview sequence exceeds 8388608 authored bytes");
            }
            let Resolved { value, missing } = normalized;
            (value, &chunk_refs, missing)
        } else {
            (item["arguments"].clone(), &refs, BTreeSet::new())
        };
        let Resolved {
            value: arguments,
            missing: unresolved,
        } = resolve(&raw, bindings);
        missing.extend(unresolved);
        let identity = json!({"index":index,"op_key":op_key,"client_ref":client_ref});
        if !missing.is_empty() {
            if preview {
                let mut blocked_by = Vec::new();
                let mut unbound = Vec::new();
                for name in &missing {
                    match failed.get(name) {
                        Some(identity) => blocked_by.push(identity.clone()),
                        None => unbound.push(name.clone()),
                    }
                }
                // A reference an item that was refused or blocked would have bound is blocked, not
                // an ordinary error; any other unbound reference keeps the existing contract.
                if unbound.is_empty() {
                    blocked.push(json!({
                        "index":index,
                        "op_key":op_key,
                        "client_ref":client_ref,
                        "reason":"blocked_by_refusal",
                        "blocked_by":blocked_by,
                    }));
                    failed.insert(client_ref.to_string(), identity);
                    continue;
                }
            }
            bail!(
                "unresolved earlier client reference `@@{}`",
                missing.into_iter().next().expect("a missing reference")
            );
        }
        let canonical = request(tool, &raw)?;
        let resolved = resolve(&canonical, bindings).checked()?;
        let request_hash = hash(&json!({"request":canonical,"resolved":resolved}));
        let outcome = if preview {
            // S10-AC02 / R195: one savepoint per item, so a Lock refusal rolls back exactly this item
            // and every independent item after it still runs.
            let savepoint = tx.savepoint()?;
            match apply_item(&savepoint, root, tool, &arguments, op_key, &request_hash) {
                // R194: release the item into the outer temporary transaction, then publish it.
                Ok(outcome) => {
                    savepoint.commit()?;
                    Ok(outcome)
                }
                Err(error) => {
                    // `finish` rolls this item's savepoint back and releases it, so the outer
                    // transaction — and every item released into it — stays usable.
                    savepoint.finish()?;
                    Err(error)
                }
            }
        } else {
            apply_item(tx, root, tool, &arguments, op_key, &request_hash)
        };
        match outcome {
            Ok(ItemOutcome::Replayed { address, result }) => {
                refs.insert(client_ref.to_string(), address.clone());
                if sequence {
                    chunk_refs.insert(client_ref.to_string(), address);
                }
                results.push(
                    json!({"op_key":op_key,"client_ref":client_ref,"result":result,"replayed":true}),
                );
            }
            Ok(ItemOutcome::Written {
                address,
                result,
                wrote,
            }) => {
                refs.insert(client_ref.to_string(), address.clone());
                if sequence {
                    chunk_refs.insert(client_ref.to_string(), address.clone());
                }
                if wrote {
                    changed.insert(address);
                }
                results.push(
                    json!({"op_key":op_key,"client_ref":client_ref,"result":result,"replayed":false}),
                );
            }
            Err(error) => {
                // R0175: a blocking Card-check refusal names the zero-based item, its tool and its
                // operation key, and rolls the whole real batch back. The exhaustive preview
                // collection of card_check refusals is S05 (#147).
                if let Some(refusal) = error.downcast_ref::<crate::gate::Refusal>() {
                    return Err(anyhow::Error::new(
                        refusal.clone().in_batch(index, tool, op_key),
                    ));
                }
                match error.downcast_ref::<crate::tree_lock::Refusal>() {
                    // R195: record the refusal with its item identity and keep the plan going.
                    Some(refusal) if preview => {
                        refusals.push(
                            json!({"index":index,"op_key":op_key,"client_ref":client_ref,"refusal":refusal.value()}),
                        );
                        failed.insert(client_ref.to_string(), identity);
                    }
                    // R197: every non-Lock failure keeps its current fail-fast contract.
                    _ => return Err(error),
                }
            }
        }
    }
    if !changed.is_empty() {
        crate::link_index::sync_sources(tx, &changed)?;
        if !preview {
            crate::link_index::bump_revision(tx)?;
        }
    }
    let mut cards = Vec::new();
    let mut paragraphs = Vec::new();
    if preview {
        // A preview projects its own simulation: it must still match the final receipt per address,
        // so replaying a historical receipt after an independent edit is refused.
        let mut expected = BTreeMap::new();
        for receipt in &results {
            let result = &receipt["result"];
            let address = result["address"]
                .as_str()
                .context("draft receipt missing address")?;
            expected.insert(
                address,
                result["after_fingerprint"]
                    .as_str()
                    .context("draft receipt missing fingerprint")?,
            );
            if let Some(text) = result["owned_paragraph"]["text"].as_str() {
                let links = crate::model::read_links(text, str::to_string);
                let kinds: BTreeSet<_> = links
                    .iter()
                    .filter(|link| link.kind != crate::model::RelationKind::Refers)
                    .map(|link| link.target.clone())
                    .collect();
                let targets: BTreeSet<_> = links.iter().map(|link| link.target.clone()).collect();
                paragraphs
                    .push(json!({"address":address,"body":text,"links":targets,"kinds":kinds}));
            }
        }
        let mut previewed = Vec::new();
        for (address, expected) in expected {
            let observed = fingerprint(tx, address)?;
            if observed != expected {
                let card = crate::load_card(tx, address)?;
                return Err(anyhow::Error::new(Conflict {
                    address: address.to_string(),
                    reason: "current output differs from the final draft receipt".to_string(),
                    observed_body: crate::parse_card_text(&card.text)?.body,
                    observed_fingerprint: observed,
                }));
            }
            previewed.push(crate::load_card(tx, address)?);
        }
        // A preview's final projections carry the same Lock facts a committed read would show,
        // read from the same uncommitted transaction (R208, S03-AC04).
        let rows = previewed
            .iter()
            .map(|card| card.row_id)
            .collect::<Vec<_>>();
        let lock = crate::tree_lock::LockView::read(tx, Some(&rows))?;
        for card in &previewed {
            cards.push(crate::read::text_facts(tx, card, &lock)?);
        }
    }
    Ok(Simulated {
        results,
        refusals,
        blocked,
        cards,
        paragraphs,
    })
}

/// One item through the ordinary receipt, guard and writer path, unchanged by preview aggregation.
fn apply_item(
    conn: &rusqlite::Connection,
    root: &Path,
    tool: &str,
    arguments: &Value,
    op_key: &str,
    request_hash: &str,
) -> Result<ItemOutcome> {
    let prior: Option<(String, String)> = conn
        .query_row(
            "SELECT request_hash,result_json FROM applied_ops WHERE op_key=?1",
            [op_key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    if let Some((prior_hash, result_json)) = prior {
        if prior_hash != request_hash {
            bail!("operation key conflict: `{op_key}`");
        }
        let result: Value =
            serde_json::from_str(&result_json).context("invalid stored operation result")?;
        let address = result["address"]
            .as_str()
            .context("stored operation result missing address")?
            .to_string();
        return Ok(ItemOutcome::Replayed { address, result });
    }
    let before = if tool == "edit_card" {
        let at = arguments["at"].as_str().context("missing edit at")?;
        crate::Access::Agent.ensure_writable(crate::load_card(conn, at)?.workspace)?;
        Some(fingerprint(conn, at)?)
    } else {
        None
    };
    if let Some(before) = &before {
        guard_edit(conn, arguments, before)?;
    }
    let mut write_arguments = arguments.clone();
    if tool == "edit_card" {
        write_arguments
            .as_object_mut()
            .unwrap()
            .remove("base_fingerprint");
        write_arguments
            .as_object_mut()
            .unwrap()
            .remove("base_receipt");
    }
    let (mut result, wrote) = if tool == "edit_card" && arguments.get("owned_paragraph").is_some() {
        owned_paragraph(root, conn, arguments, before.as_deref().expect("edit base"))?
    } else {
        crate::mcp::write_card_tx(root, conn, tool, &write_arguments)?
    };
    let address = result["address"]
        .as_str()
        .context("write result missing address")?
        .to_string();
    result["before_fingerprint"] = json!(before);
    result["after_fingerprint"] = json!(fingerprint(conn, &address)?);
    conn.execute("INSERT INTO applied_ops(op_key,request_hash,result_json,committed_at) VALUES(?1,?2,?3,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",rusqlite::params![op_key,request_hash,result.to_string()])?;
    Ok(ItemOutcome::Written {
        address,
        result,
        wrote,
    })
}
