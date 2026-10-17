//! Bounded identified writes within one archive transaction.

use anyhow::{Context, Result, bail};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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

pub(crate) fn apply(root: &Path, args: &Value) -> Result<Value> {
    let items = args["items"]
        .as_array()
        .context("missing array argument `items`")?;
    if items.is_empty() || items.len() > 64 {
        bail!("a batch requires 1 through 64 items");
    }
    let authored_bytes: usize = items
        .iter()
        .flat_map(|item| {
            ["title", "body", "text", "bibtex", "base"]
                .map(|name| item["arguments"][name].as_str().map(str::len).unwrap_or(0))
        })
        .sum();
    if authored_bytes > 1024 * 1024 {
        bail!("a batch is limited to 1048576 authored bytes; received {authored_bytes}");
    }
    for item in items {
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
    let _lock = crate::acquire_edit_lock_within(root, std::time::Duration::from_secs(20))?;
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS applied_ops(op_key TEXT NOT NULL PRIMARY KEY, request_hash TEXT NOT NULL, result_json TEXT NOT NULL, committed_at TEXT NOT NULL)")?;
    let mut refs = BTreeMap::new();
    let mut changed = BTreeSet::new();
    let mut results = Vec::new();
    for item in items {
        let op_key = item["op_key"].as_str().context("missing item op_key")?;
        let client_ref = item["client_ref"]
            .as_str()
            .context("missing item client_ref")?;
        if refs.contains_key(client_ref) {
            bail!("duplicate client_ref `{client_ref}`");
        }
        let tool = item["tool"].as_str().context("missing item tool")?;
        let arguments = resolve(&item["arguments"], &refs)?;
        let canonical = request(tool, &item["arguments"])?;
        let resolved = resolve(&canonical, &refs)?;
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
            results.push(
                json!({"op_key":op_key,"client_ref":client_ref,"result":result,"replayed":true}),
            );
            continue;
        }
        let before = if tool == "edit_card" {
            Some(fingerprint(
                &tx,
                arguments["at"].as_str().context("missing edit at")?,
            )?)
        } else {
            None
        };
        let (mut result, did_change) = crate::mcp::write_card_tx(root, &tx, tool, &arguments)?;
        let address = result["address"]
            .as_str()
            .context("write result missing address")?
            .to_string();
        result["before_fingerprint"] = json!(before);
        result["after_fingerprint"] = json!(fingerprint(&tx, &address)?);
        tx.execute("INSERT INTO applied_ops(op_key,request_hash,result_json,committed_at) VALUES(?1,?2,?3,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",rusqlite::params![op_key,request_hash,result.to_string()])?;
        refs.insert(client_ref.to_string(), address.clone());
        if did_change {
            changed.insert(address);
        }
        results.push(
            json!({"op_key":op_key,"client_ref":client_ref,"result":result,"replayed":false}),
        );
    }
    if !changed.is_empty() {
        crate::link_index::sync_sources(&tx, &changed)?;
        crate::link_index::bump_revision(&tx)?;
    }
    tx.commit()?;
    Ok(json!({"results":results}))
}
