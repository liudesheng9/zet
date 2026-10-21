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

fn resolve(value: &Value, refs: &BTreeMap<String, String>) -> Result<Value> {
    Ok(match value {
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
                let address = refs
                    .get(name)
                    .with_context(|| format!("unresolved earlier client reference `@@{name}`"))?;
                out.push_str(address);
                rest = &rest[end..];
            }
            out.push_str(rest);
            Value::String(out)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|v| resolve(v, refs))
                .collect::<Result<_>>()?,
        ),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(k, v)| Ok((k.clone(), resolve(v, refs)?)))
                .collect::<Result<_>>()?,
        ),
        other => other.clone(),
    })
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
        crate::validate_card_text(conn, address, &written, Some(&card.text))?;
        conn.execute(
            "UPDATE cards SET text=?1 WHERE row_id=?2",
            rusqlite::params![written, card.row_id],
        )?;
    }
    Ok((
        json!({"address":address,"owned_paragraph":{"address":address,"text":text,"replaces_receipt":replaces,"adopted":adopted}}),
        changed,
    ))
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
fn resolve_before(value: &Value, refs: &BTreeMap<String, String>, start: usize) -> Result<Value> {
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
                    out.push_str(refs.get(name).with_context(|| {
                        format!("unresolved earlier client reference `@@{name}`")
                    })?);
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
                .map(|v| resolve_before(v, refs, start))
                .collect::<Result<_>>()?,
        ),
        Value::Object(values) => Value::Object(
            values
                .iter()
                .map(|(k, v)| Ok((k.clone(), resolve_before(v, refs, start)?)))
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
    let tx = conn.transaction()?;
    let archive_revision = crate::link_index::revision(&tx)?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS applied_ops(op_key TEXT NOT NULL PRIMARY KEY, request_hash TEXT NOT NULL, result_json TEXT NOT NULL, committed_at TEXT NOT NULL)")?;
    let mut refs = BTreeMap::new();
    let mut chunk_refs = BTreeMap::new();
    let mut changed = BTreeSet::new();
    let mut results = Vec::new();
    let mut chunk_start = 0usize;
    let mut chunk_bytes = 0usize;
    let mut sequence_bytes = 0usize;
    for (index, item) in items.iter().enumerate() {
        let op_key = item["op_key"].as_str().context("missing item op_key")?;
        let client_ref = item["client_ref"]
            .as_str()
            .context("missing item client_ref")?;
        if refs.contains_key(client_ref) {
            bail!("duplicate client_ref `{client_ref}`");
        }
        let tool = item["tool"].as_str().context("missing item tool")?;
        let normalized = if sequence {
            let mut arguments = resolve_before(&item["arguments"], &refs, chunk_start)?;
            if index - chunk_start >= 64 || chunk_bytes + authored_bytes(&arguments) > 1024 * 1024 {
                chunk_start = index;
                chunk_bytes = 0;
                chunk_refs.clear();
                arguments = resolve_before(&item["arguments"], &refs, chunk_start)?;
            }
            let bytes = authored_bytes(&arguments);
            if bytes > 1024 * 1024 {
                bail!("resolved sequence item {index} exceeds 1048576 authored bytes");
            }
            chunk_bytes += bytes;
            sequence_bytes += bytes;
            if sequence_bytes > 8 * 1024 * 1024 {
                bail!("resolved preview sequence exceeds 8388608 authored bytes");
            }
            Some(arguments)
        } else {
            None
        };
        let raw = normalized.as_ref().unwrap_or(&item["arguments"]);
        let bindings = if sequence { &chunk_refs } else { &refs };
        let arguments = resolve(raw, bindings)?;
        let canonical = request(tool, raw)?;
        let resolved = resolve(&canonical, bindings)?;
        let request_hash = hash(&json!({"request":canonical,"resolved":resolved}));
        let prior: Option<(String, String)> = tx
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
                .context("stored operation result missing address")?;
            refs.insert(client_ref.to_string(), address.to_string());
            if sequence {
                chunk_refs.insert(client_ref.to_string(), address.to_string());
            }
            results.push(
                json!({"op_key":op_key,"client_ref":client_ref,"result":result,"replayed":true}),
            );
            continue;
        }
        let before = if tool == "edit_card" {
            crate::Access::Agent.ensure_writable(
                crate::load_card(&tx, arguments["at"].as_str().context("missing edit at")?)?
                    .workspace,
            )?;
            Some(fingerprint(
                &tx,
                arguments["at"].as_str().context("missing edit at")?,
            )?)
        } else {
            None
        };
        if let Some(before) = &before {
            guard_edit(&tx, &arguments, before)?;
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
        let (mut result, did_change) =
            if tool == "edit_card" && arguments.get("owned_paragraph").is_some() {
                owned_paragraph(root, &tx, &arguments, before.as_deref().expect("edit base"))?
            } else {
                crate::mcp::write_card_tx(root, &tx, tool, &write_arguments)?
            };
        let address = result["address"]
            .as_str()
            .context("write result missing address")?
            .to_string();
        result["before_fingerprint"] = json!(before);
        result["after_fingerprint"] = json!(fingerprint(&tx, &address)?);
        tx.execute("INSERT INTO applied_ops(op_key,request_hash,result_json,committed_at) VALUES(?1,?2,?3,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",rusqlite::params![op_key,request_hash,result.to_string()])?;
        refs.insert(client_ref.to_string(), address.clone());
        if sequence {
            chunk_refs.insert(client_ref.to_string(), address.clone());
        }
        if did_change {
            changed.insert(address);
        }
        results.push(
            json!({"op_key":op_key,"client_ref":client_ref,"result":result,"replayed":false}),
        );
    }
    if !changed.is_empty() {
        crate::link_index::sync_sources(&tx, &changed)?;
        if !preview {
            crate::link_index::bump_revision(&tx)?;
        }
    }
    if preview {
        let mut expected = BTreeMap::new();
        let mut paragraphs = Vec::new();
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
            let observed = fingerprint(&tx, address)?;
            if observed != expected {
                let card = crate::load_card(&tx, address)?;
                return Err(anyhow::Error::new(Conflict {
                    address: address.to_string(),
                    reason: "current output differs from the final draft receipt".to_string(),
                    observed_body: crate::parse_card_text(&card.text)?.body,
                    observed_fingerprint: observed,
                }));
            }
            previewed.push(crate::load_card(&tx, address)?);
        }
        // A preview's final projections carry the same Lock facts a committed read would show,
        // read from the same uncommitted transaction (R208, S03-AC04).
        let rows = previewed
            .iter()
            .map(|card| card.row_id)
            .collect::<Vec<_>>();
        let lock = crate::tree_lock::LockView::read(&tx, Some(&rows))?;
        let mut cards = Vec::new();
        for card in &previewed {
            cards.push(crate::read::text_facts(&tx, card, &lock)?);
        }
        tx.rollback()?;
        return Ok(
            json!({"results":results,"preview":true,"applied":false,"complete":true,"archive_revision":archive_revision,"cards":cards,"paragraphs":paragraphs}),
        );
    }
    tx.commit()?;
    Ok(json!({"results":results}))
}
