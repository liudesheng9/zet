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

/// The most Cards one batch may rename or delete (D3, #231): a larger reorganization is several
/// batches.
const MAX_TREE_CHANGES: usize = 2000;

/// Where every Card of a tree-operation batch's start state is now (D1, #231). A literal address in
/// such a batch names the Card that held it when the batch started; earlier items' renames carry it
/// along, and a Card an earlier item deleted cannot be named again.
struct Follow {
    start: BTreeSet<String>,
    /// Start address to current address, for the start Cards that moved.
    current_of: BTreeMap<String, String>,
    /// Current address to start address, for the same Cards.
    start_of: BTreeMap<String, String>,
    /// Current addresses of the Cards the batch made.
    created: BTreeSet<String>,
    /// Start addresses of the deleted start Cards.
    deleted: BTreeSet<String>,
    rewritten: usize,
    breaks: BTreeSet<(String, String)>,
}

impl Follow {
    fn start(conn: &rusqlite::Connection) -> Result<Self> {
        Ok(Self {
            start: crate::addresses(conn)?,
            current_of: BTreeMap::new(),
            start_of: BTreeMap::new(),
            created: BTreeSet::new(),
            deleted: BTreeSet::new(),
            rewritten: 0,
            breaks: BTreeSet::new(),
        })
    }

    /// The current address of the start Card `literal` names.
    fn address(&self, literal: &str) -> Result<String> {
        if self.deleted.contains(literal) {
            bail!("card_deleted_in_batch: `{literal}` was deleted by an earlier item of this batch");
        }
        if !self.start.contains(literal) {
            bail!(
                "`{literal}` names no Card at the start of this batch; name a Card the batch made by its @@client_ref"
            );
        }
        Ok(self
            .current_of
            .get(literal)
            .cloned()
            .unwrap_or_else(|| literal.to_string()))
    }

    /// The start address of the Card now at `current`.
    fn start_address(&self, current: &str) -> String {
        self.start_of
            .get(current)
            .cloned()
            .unwrap_or_else(|| current.to_string())
    }

    /// An item's arguments with every literal address read on the start state: the address fields,
    /// and the Links an authored title, body or text names. A `@@client_ref` is left for the
    /// reference resolver, which binds it to the Card's current address.
    fn translate(&self, tool: &str, arguments: &Value) -> Result<Value> {
        let mut translated = arguments.clone();
        let Some(fields) = translated.as_object_mut() else {
            return Ok(translated);
        };
        // `to` names a Card only for relink; for move_card it is an empty slot, refused in a batch.
        let names: &[&str] = if tool == "relink" {
            &["from", "to"]
        } else {
            &["at", "before", "after", "side_of", "into", "from", "through", "parent"]
        };
        let follow = |value: &Value| -> Result<Value> {
            match value {
                Value::String(literal) if !literal.starts_with("@@") && !literal.is_empty() => {
                    Ok(json!(self.address(literal)?))
                }
                Value::Array(items) => Ok(Value::Array(
                    items
                        .iter()
                        .map(|item| match item {
                            Value::String(literal) if !literal.starts_with("@@") && !literal.is_empty() => {
                                Ok(json!(self.address(literal)?))
                            }
                            Value::Array(pair) => Ok(Value::Array(
                                pair.iter()
                                    .map(|end| match end {
                                        Value::String(literal) if !literal.starts_with("@@") => {
                                            Ok(json!(self.address(literal)?))
                                        }
                                        other => Ok(other.clone()),
                                    })
                                    .collect::<Result<_>>()?,
                            )),
                            other => Ok(other.clone()),
                        })
                        .collect::<Result<_>>()?,
                )),
                other => Ok(other.clone()),
            }
        };
        for name in names.iter().chain(["sources", "side", "direct"].iter()) {
            if let Some(value) = fields.get(*name) {
                let current = follow(value)?;
                fields.insert(name.to_string(), current);
            }
        }
        if !self.current_of.is_empty() {
            for name in ["title", "body", "text"] {
                if let Some(Value::String(authored)) = fields.get(name) {
                    let body = crate::link::LinkBody::parse(authored)?
                        .rewrite_targets(&self.current_of)
                        .body;
                    fields.insert(name.to_string(), json!(body));
                }
            }
        }
        Ok(translated)
    }

    /// One item renamed these Cards (current addresses, old to new).
    fn renamed(&mut self, mapping: &BTreeMap<String, String>) {
        let mut moves = Vec::new();
        for (old, new) in mapping {
            if self.created.remove(old) {
                moves.push((None, new.clone()));
            } else {
                let start = self.start_of.remove(old).unwrap_or_else(|| old.clone());
                moves.push((Some(start), new.clone()));
            }
        }
        for (start, new) in moves {
            match start {
                None => {
                    self.created.insert(new);
                }
                Some(start) => {
                    self.start_of.insert(new.clone(), start.clone());
                    self.current_of.insert(start, new);
                }
            }
        }
    }

    /// One item deleted these Cards (current addresses).
    fn removed(&mut self, deleted: &BTreeSet<String>) {
        for current in deleted {
            if self.created.remove(current) {
                continue;
            }
            let start = self.start_of.remove(current).unwrap_or_else(|| current.clone());
            self.current_of.remove(&start);
            self.deleted.insert(start);
        }
    }

    fn moved(&self) -> BTreeMap<&String, &String> {
        self.current_of.iter().filter(|(start, now)| start != now).collect()
    }

    /// How many start Cards the batch renamed or deleted, for the cap.
    fn size(&self) -> usize {
        self.moved().len() + self.deleted.len()
    }

    /// The composed plan, in start-state addresses.
    fn plan(&self) -> Value {
        json!({
            "moved": self.moved(),
            "deleted": self.deleted,
            "links_that_will_break": self.breaks.iter().map(|(source, target)| json!({"source": source, "target": target})).collect::<Vec<_>>(),
            "link_macros_rewritten": self.rewritten,
        })
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
    let mut authored = json!({"title":parsed.title,"body":parsed.body,"bibtex":card.bibtex,"type":card.card_type.id(),"facet":card.facet.map(|f|f.id().to_string()),"emphasis":card.emphasis.map(crate::model::Emphasis::id),"level":card.level.map(crate::model::Level::id),"hub_level":card.hub_level.map(crate::hub::HubLevel::id),"form":card.form.map(crate::math::MathForm::id)});
    // A nonempty term name is part of the receipt identity, in the same field convention the digest
    // guard uses. A Card without one keeps exactly its old projection, so retained receipts stay
    // comparable (S07-AC06, R0245).
    if let Some(term) = &card.term_name {
        authored["term"] = json!(term);
    }
    Ok(hash(&authored))
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

pub(crate) fn apply(root: &Path, args: &Value, scope: &mut crate::scope::WriteScope) -> Result<Value> {
    apply_as(root, crate::Access::Agent, args, scope)
}

/// [`apply`] under the given Access: MCP applies as the agent, the owner's `zt apply` and the GUI as
/// the owner (S6 #234, S7 #235).
pub(crate) fn apply_as(
    root: &Path,
    access: crate::Access,
    args: &Value,
    scope: &mut crate::scope::WriteScope,
) -> Result<Value> {
    let preview = match args.get("preview") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        _ => bail!("preview must be a boolean"),
    };
    // A lean preview checks every item as a preview does and answers only the identities, addresses,
    // refusals and blocked items, so a large plan's scan stays inside the reply bound (#226).
    let lean = match args.get("lean") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        _ => bail!("lean must be a boolean"),
    };
    if lean && !preview {
        bail!("lean is a preview option; pass preview:true with lean:true");
    }
    let confirm = match args.get("confirm") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        _ => bail!("confirm must be a string"),
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
    // A batch with a tree operation reads every address on its start state and is confirmed by its
    // plan hash (S3, #231).
    let structural = items
        .iter()
        .any(|item| item["tool"].as_str().is_some_and(crate::tree_ops::is_structural));
    if structural && sequence {
        bail!(
            "tree operations (move_card, insert_card, delete_card) go in `items`, at most 64 per batch, not in `preview_items`"
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
    let plan_hash = if structural {
        Some(plan_hash(archive_revision, items)?)
    } else {
        None
    };
    let simulated = simulate(&mut tx, root, access, items, sequence, preview, structural, scope);
    match simulated {
        // R198: the outer transaction is rolled back before any response leaves this module, the
        // ordinary failure contract's error included.
        Err(error) => {
            tx.rollback()?;
            Err(error)
        }
        Ok(simulated) if preview => {
            let plan = simulated.plan.clone();
            let mut value = if lean { simulated.lean(archive_revision) } else { simulated.value(archive_revision, true) };
            if let (Some(hash), Some(plan)) = (&plan_hash, plan) {
                value["plan_hash"] = json!(hash);
                value["plan"] = plan;
            }
            tx.rollback()?;
            Ok(value)
        }
        Ok(mut simulated) => {
            let plan = simulated.plan.take();
            // D2: a tree-operation batch commits only under the hash of the plan its client saw. A
            // pure replay of a committed batch writes nothing, so its old hash still answers.
            if let Some(hash) = &plan_hash
                && !simulated.replayed_all
                && confirm.as_deref() != Some(hash.as_str())
            {
                tx.rollback()?;
                if confirm.is_none() {
                    return Ok(json!({
                        "applied": false,
                        "confirm": hash,
                        "plan": plan,
                        "next": "nothing changed yet; call again with the same items and confirm = this plan hash",
                    }));
                }
                bail!(
                    "plan_changed: the archive or the items changed since this plan was made; preview the batch again"
                );
            }
            let made = std::mem::take(&mut simulated.made);
            let mut value = simulated.value(archive_revision, false);
            tx.commit()?;
            // The Cards a committed batch made are this connection's from now on (D6).
            scope.record(made);
            if let Some(plan) = plan {
                value["applied"] = json!(true);
                value["plan"] = plan;
            }
            Ok(value)
        }
    }
}

fn has_receipt(conn: &rusqlite::Connection, op_key: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM applied_ops WHERE op_key=?1)",
        [op_key],
        |r| r.get(0),
    )?)
}

/// The hash a tree-operation batch is confirmed with: the archive revision it was planned on and its
/// canonical items (D2, #231).
fn plan_hash(archive_revision: u64, items: &[Value]) -> Result<String> {
    let canonical = items
        .iter()
        .map(|item| {
            let tool = item["tool"].as_str().context("missing item tool")?;
            Ok(json!({
                "op_key": item["op_key"],
                "client_ref": item["client_ref"],
                "request": request(tool, &item["arguments"])?,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(hash(&json!({"archive_revision": archive_revision, "items": canonical})))
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
    /// A tree operation ran: what it renamed, deleted and made.
    Tree {
        address: String,
        result: Value,
        applied: crate::tree_ops::Applied,
    },
}

/// The facts one batch or preview gathered inside its transaction.
struct Simulated {
    results: Vec<Value>,
    refusals: Vec<Value>,
    blocked: Vec<Value>,
    cards: Vec<Value>,
    paragraphs: Vec<Value>,
    /// The Cards the request's create items made or replayed: the connection's own once committed.
    made: BTreeSet<String>,
    /// A tree-operation batch's composed plan, in start-state addresses.
    plan: Option<Value>,
    /// Every item replayed its receipt, so the request writes nothing.
    replayed_all: bool,
}

impl Simulated {
    /// The reply shape: a real batch answers only its results, and a preview states whether it
    /// checked the whole plan, every ordered Lock refusal and every blocked item identity.
    /// The lean preview reply: the preview's fields with each result cut to its identity and address,
    /// and no Card or paragraph projections.
    fn lean(mut self, archive_revision: u64) -> Value {
        self.results = self
            .results
            .into_iter()
            .map(|r| json!({"op_key":r["op_key"],"client_ref":r["client_ref"],"replayed":r["replayed"],"result":{"address":r["result"]["address"]}}))
            .collect();
        self.cards.clear();
        self.paragraphs.clear();
        let mut value = self.value(archive_revision, true);
        value["lean"] = json!(true);
        value
    }

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
    access: crate::Access,
    items: &[Value],
    sequence: bool,
    preview: bool,
    structural: bool,
    scope: &crate::scope::WriteScope,
) -> Result<Simulated> {
    let mut follow = if structural { Some(Follow::start(tx)?) } else { None };
    let mut changes = crate::link_index::Changes::default();
    // The reference names whose Card an earlier item deleted.
    let mut gone: BTreeSet<String> = BTreeSet::new();
    let (mut replayed, mut written) = (0usize, 0usize);
    let mut made = BTreeSet::new();
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
            // An item that already holds its receipt replays it as sent: its literals named the start
            // state of the batch that committed it, not today's archive.
            let arguments = match &follow {
                Some(follow) if !has_receipt(tx, op_key)? => follow.translate(tool, &item["arguments"])?,
                _ => item["arguments"].clone(),
            };
            (arguments, &refs, BTreeSet::new())
        };
        let Resolved {
            value: arguments,
            missing: unresolved,
        } = resolve(&raw, bindings);
        missing.extend(unresolved);
        if let Some(name) = missing.iter().find(|name| gone.contains(*name)) {
            bail!("card_deleted_in_batch: `@@{name}` names a Card an earlier item of this batch deleted");
        }
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
        let request_hash = if structural {
            // The request as sent: a replay reads the start state again, so it must not hash the
            // addresses earlier items carried along (D1, #231).
            hash(&json!({"request": request(tool, &item["arguments"])?}))
        } else {
            let canonical = request(tool, &raw)?;
            let resolved = resolve(&canonical, bindings).checked()?;
            hash(&json!({"request":canonical,"resolved":resolved}))
        };
        let tree = structural.then_some(&mut changes);
        let outcome = if preview {
            // S10-AC02 / R195: one savepoint per item, so a Lock refusal rolls back exactly this item
            // and every independent item after it still runs.
            let savepoint = tx.savepoint()?;
            match apply_item(&savepoint, root, access, tool, &arguments, op_key, &request_hash, scope, &made, tree) {
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
            apply_item(tx, root, access, tool, &arguments, op_key, &request_hash, scope, &made, tree)
        };
        match outcome {
            Ok(ItemOutcome::Tree {
                address,
                result,
                applied,
            }) => {
                written += 1;
                let follow = follow.as_mut().expect("a tree operation runs only in a structural batch");
                for (source, target) in &applied.breaks {
                    follow
                        .breaks
                        .insert((follow.start_address(source), follow.start_address(target)));
                }
                follow.rewritten += applied.rewritten;
                // A delete names its set before its Side compaction renames the later siblings.
                follow.removed(&applied.deleted);
                follow.renamed(&applied.mapping);
                if let Some(created) = &applied.created {
                    follow.created.insert(created.clone());
                }
                // References and the Cards this request made follow the same renames and deletes.
                for (name, bound) in refs.iter_mut() {
                    if applied.deleted.contains(bound.as_str()) {
                        gone.insert(name.clone());
                    } else if let Some(new) = applied.mapping.get(bound.as_str()) {
                        *bound = new.clone();
                    }
                }
                refs.retain(|name, _| !gone.contains(name));
                made = made
                    .into_iter()
                    .filter(|at| !applied.deleted.contains(at))
                    .map(|at| applied.mapping.get(&at).cloned().unwrap_or(at))
                    .collect();
                refs.insert(client_ref.to_string(), address);
                results.push(
                    json!({"op_key":op_key,"client_ref":client_ref,"result":result,"replayed":false}),
                );
            }
            Ok(ItemOutcome::Replayed { address, result }) => {
                replayed += 1;
                // A replayed create is the Card this operation made, so a resumed run may continue it.
                if tool.starts_with("create") {
                    made.insert(address.clone());
                }
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
                written += 1;
                if let Some(follow) = follow.as_mut() {
                    // In a tree-operation batch a later item may move this Card, so its index rows are
                    // read now; a Side placement's shift is a rename like any other.
                    if let Some(moved) = result.get("moved").and_then(Value::as_object) {
                        let mapping: BTreeMap<String, String> = moved
                            .iter()
                            .filter_map(|(old, new)| Some((old.clone(), new.as_str()?.to_string())))
                            .collect();
                        follow.renamed(&mapping);
                        for bound in refs.values_mut() {
                            if let Some(new) = mapping.get(bound.as_str()) {
                                *bound = new.clone();
                            }
                        }
                    }
                    if tool.starts_with("create") {
                        follow.created.insert(address.clone());
                    }
                    if wrote {
                        changes.resync(tx, &address)?;
                    }
                }
                if tool.starts_with("create") {
                    made.insert(address.clone());
                }
                refs.insert(client_ref.to_string(), address.clone());
                if sequence {
                    chunk_refs.insert(client_ref.to_string(), address.clone());
                }
                if wrote && follow.is_none() {
                    changed.insert(address);
                }
                results.push(
                    json!({"op_key":op_key,"client_ref":client_ref,"result":result,"replayed":false}),
                );
            }
            Err(error) => {
                // R0179/R0180: a preview treats a blocking Card check like the Lock refusal it
                // already collects. The item keeps its typed refusal and identity, an independent
                // later item still simulates, and a dependent reference is blocked instead of
                // evaluated with made-up target data. A real batch stays fail-fast and names its
                // first blocking item (R0175/R0178).
                match error.downcast_ref::<crate::gate::Refusal>() {
                    Some(refusal) if preview => {
                        refusals.push(
                            json!({"index":index,"op_key":op_key,"client_ref":client_ref,"refusal":refusal.value()}),
                        );
                        failed.insert(client_ref.to_string(), identity);
                    }
                    Some(refusal) => {
                        return Err(anyhow::Error::new(
                            refusal.clone().in_batch(index, tool, op_key),
                        ));
                    }
                    // #226: a preview collects a Topic another agent Topic already names like the other
                    // destination refusals, so a client can probe a creation; a real batch fails fast on it.
                    None if preview && error.downcast_ref::<crate::scope::TopicExists>().is_some() => {
                        let refusal = error.downcast_ref::<crate::scope::TopicExists>().expect("a Topic title refusal");
                        refusals.push(
                            json!({"index":index,"op_key":op_key,"client_ref":client_ref,"refusal":refusal.value()}),
                        );
                        failed.insert(client_ref.to_string(), identity);
                    }
                    None if error.downcast_ref::<crate::scope::Refusal>().is_some() => {
                        let refusal = error.downcast_ref::<crate::scope::Refusal>().expect("a scope refusal");
                        // D5: a preview collects a scope refusal like a Lock refusal; a real batch
                        // stops on it, names the item, and rolls back.
                        if !preview {
                            return Err(anyhow::Error::new(refusal.clone().in_batch(index, tool, op_key)));
                        }
                        refusals.push(
                            json!({"index":index,"op_key":op_key,"client_ref":client_ref,"refusal":refusal.value()}),
                        );
                        failed.insert(client_ref.to_string(), identity);
                    }
                    None => match error.downcast_ref::<crate::tree_lock::Refusal>() {
                        // R195: record the refusal with its item identity and keep the plan going.
                        Some(refusal) if preview => {
                            refusals.push(
                                json!({"index":index,"op_key":op_key,"client_ref":client_ref,"refusal":refusal.value()}),
                            );
                            failed.insert(client_ref.to_string(), identity);
                        }
                        // R197: every non-Lock, non-Card-check failure keeps its current fail-fast
                        // contract.
                        _ => return Err(error),
                    },
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
    let replayed_all = written == 0;
    let plan = match &follow {
        Some(follow) => {
            if replayed > 0 && written > 0 {
                bail!(
                    "a tree-operation batch replays completely or not at all: {replayed} item(s) replayed and {written} item(s) are new; send the original batch, or new operation keys"
                );
            }
            if follow.size() > MAX_TREE_CHANGES {
                bail!(
                    "a batch may rename or delete at most {MAX_TREE_CHANGES} Cards; this one changes {}; split it into several batches",
                    follow.size()
                );
            }
            if written > 0 {
                // D1: one refresh of Reverse links, Term uses and the revision for the whole batch.
                changes.finish(tx)?;
            }
            Some(follow.plan())
        }
        None => None,
    };
    let mut cards = Vec::new();
    let mut paragraphs = Vec::new();
    if preview && follow.is_some() {
        // A tree-operation batch projects the Cards its creates and edits left, at their final
        // addresses; a later move may rewrite an edited body's Links, so no receipt fingerprint is
        // compared here.
        let mut finals = BTreeSet::new();
        for (receipt, item) in results.iter().zip(items) {
            let tool = item["tool"].as_str().unwrap_or_default();
            if crate::tree_ops::is_structural(tool) {
                continue;
            }
            if let Some(address) = refs.get(receipt["client_ref"].as_str().unwrap_or_default()) {
                finals.insert(address.clone());
            }
        }
        let previewed = finals
            .iter()
            .filter(|at| crate::target_exists(tx, at).unwrap_or(false))
            .map(|at| crate::load_card(tx, at))
            .collect::<Result<Vec<_>>>()?;
        let rows = previewed.iter().map(|card| card.row_id).collect::<Vec<_>>();
        let lock = crate::tree_lock::LockView::read(tx, Some(&rows))?;
        for card in &previewed {
            cards.push(crate::read::text_facts(tx, card, &lock, crate::read::TermLinks::Raw)?);
        }
    } else if preview {
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
                    .filter(|link| link.kind.names_a_relation())
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
            cards.push(crate::read::text_facts(
                tx,
                card,
                &lock,
                crate::read::TermLinks::Raw,
            )?);
        }
    }
    Ok(Simulated {
        results,
        refusals,
        blocked,
        cards,
        paragraphs,
        made,
        plan,
        replayed_all,
    })
}

/// One item through the ordinary receipt, guard and writer path, unchanged by preview aggregation.
fn apply_item(
    conn: &rusqlite::Connection,
    root: &Path,
    access: crate::Access,
    tool: &str,
    arguments: &Value,
    op_key: &str,
    request_hash: &str,
    scope: &crate::scope::WriteScope,
    made: &BTreeSet<String>,
    tree: Option<&mut crate::link_index::Changes>,
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
    // A pure receipt replay is not a write; every new write meets the connection's Write scope.
    scope.check(conn, tool, arguments, made)?;
    if crate::tree_ops::is_structural(tool) {
        let changes = tree.context("a tree operation runs only inside a structural batch")?;
        let applied = crate::tree_ops::apply_tx(root, conn, access, tool, arguments, changes)?;
        let mut result = json!({
            "address": applied.address,
            "moved": applied.mapping,
            "deleted": applied.deleted,
            "link_macros_rewritten": applied.rewritten,
            "plan": applied.lines,
        });
        applied.accepted.attach(&mut result);
        conn.execute("INSERT INTO applied_ops(op_key,request_hash,result_json,committed_at) VALUES(?1,?2,?3,strftime('%Y-%m-%dT%H:%M:%fZ','now'))",rusqlite::params![op_key,request_hash,result.to_string()])?;
        return Ok(ItemOutcome::Tree {
            address: applied.address.clone(),
            result,
            applied,
        });
    }
    let before = if tool == "edit_card" {
        let at = arguments["at"].as_str().context("missing edit at")?;
        access.ensure_writable(crate::load_card(conn, at)?.workspace)?;
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
        crate::mcp::write_card_as(root, conn, access, tool, &write_arguments)?
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

#[cfg(test)]
mod tests {
    use super::*;

    /// S07-AC06/R0245: the service's receipt projection adds a nonempty Term name in one explicit
    /// field, and a Card without one keeps exactly its old projection. Both hex values are the
    /// same fixture the digest guard pins, so the two implementations are checked against one
    /// contract rather than against each other.
    #[test]
    fn the_receipt_projection_adds_a_nonempty_term_name_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        crate::initialize_database(temp.path()).expect("initialize");
        let mut conn = crate::open_db(temp.path()).expect("open");
        let (root, _) = crate::create_topic(&mut conn, crate::Workspace::Agent, "# Terms\n\n")
            .expect("topic");
        let first = format!("{}/1", crate::tree_id(&root));
        crate::insert_card_with_term(
            &mut conn,
            &first,
            false,
            crate::Workspace::Agent,
            crate::CardType::Term,
            None,
            Some("均衡"),
            "# 均衡\n\n定义：",
        )
        .expect("term card");
        assert_eq!(
            fingerprint(&conn, &first).expect("fingerprint"),
            "35aaae5adf8cfe486781b0c8ddc8728f701119292b4ba93cbc1083ec8969b423"
        );
        conn.execute("UPDATE cards SET term_name = NULL WHERE location = ?1", [&first])
            .expect("clear the name");
        assert_eq!(
            fingerprint(&conn, &first).expect("fingerprint"),
            "2caa06f0e57fa7874839259b3bfa5f8d05e60c75c6266198a57869ad03726cee"
        );
    }

    /// S33 (#182): a Side placement is one identified batch item. The first call takes the named
    /// sibling's label and shifts it one on; the same operation key replays the stored receipt, so a
    /// lost reply never shifts the list a second time.
    #[test]
    fn a_side_placement_batch_item_replays_under_its_operation_key_without_moving_again() {
        let temp = tempfile::tempdir().expect("tempdir");
        crate::initialize_database(temp.path()).expect("initialize");
        let root = temp.path();
        {
            let mut conn = crate::open_db(root).expect("open");
            let (topic, _) = crate::create_topic(&mut conn, crate::Workspace::Agent, "# 书\n")
                .expect("topic");
            let parent = format!("{}/1", crate::tree_id(&topic));
            for (at, text) in [
                (parent.clone(), "# 列表\n\n第一行。\n"),
                (format!("{parent}|a"), "# 甲\n\n第一张。\n"),
                (format!("{parent}|b"), "# 乙\n\n第二张。\n"),
            ] {
                crate::insert_card_with_term(
                    &mut conn,
                    &at,
                    false,
                    crate::Workspace::Agent,
                    crate::CardType::Idea,
                    None,
                    None,
                    text,
                )
                .expect("card");
            }
        }
        let items = json!([{
            "op_key": "term-cards-0123456789abcdef/K/2/apply/r003",
            "client_ref": "r003",
            "tool": "create_successor",
            "arguments": {
                "at": "0/1",
                "kind": "side",
                "before": "0/1|b",
                "type": "term",
                "term": "丙",
                "title": "丙",
                "body": "术语：丙\n",
            },
        }]);
        let first = apply(root, &json!({ "items": items }), &mut Default::default()).expect("first batch");
        assert_eq!(first["results"][0]["replayed"], json!(false));
        assert_eq!(first["results"][0]["result"]["address"], json!("0/1|b"));
        assert_eq!(
            first["results"][0]["result"]["moved"],
            json!({"0/1|b": "0/1|c"}),
            "the reply names every shifted Card"
        );
        {
            let conn = crate::open_db(root).expect("open");
            assert!(crate::load_card(&conn, "0/1|b").expect("new").text.contains("术语：丙"));
            assert!(crate::load_card(&conn, "0/1|c").expect("shifted").text.contains("# 乙\n"));
        }
        let again = apply(root, &json!({ "items": items }), &mut Default::default()).expect("replayed batch");
        assert_eq!(again["results"][0]["replayed"], json!(true));
        assert_eq!(again["results"][0]["result"]["address"], json!("0/1|b"));
        let conn = crate::open_db(root).expect("open");
        assert!(
            crate::load_card(&conn, "0/1|c").expect("shifted once").text.contains("# 乙\n"),
            "a replay shifts nothing again"
        );
        let later: i64 = conn
            .query_row("SELECT COUNT(*) FROM cards WHERE location = '0/1|d'", [], |row| row.get(0))
            .expect("count");
        assert_eq!(later, 0, "no Card was shifted twice");
        let receipts: i64 = conn
            .query_row("SELECT COUNT(*) FROM applied_ops", [], |row| row.get(0))
            .expect("receipts");
        assert_eq!(receipts, 1, "one operation key holds one receipt");
        drop(conn);
        // The named sibling is part of the request identity: the same key with another `before`
        // conflicts instead of silently replaying a different placement.
        let mut changed = items.clone();
        changed[0]["arguments"]["before"] = json!("0/1|a");
        let error = apply(root, &json!({ "items": changed }), &mut Default::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains("operation key conflict"), "{error}");
    }

    /// S33 (#182): the placement names an immediate Side sibling, and only `kind` `side` files one.
    #[test]
    fn a_side_placement_needs_the_side_kind_and_an_immediate_side_sibling() {
        let temp = tempfile::tempdir().expect("tempdir");
        crate::initialize_database(temp.path()).expect("initialize");
        let root = temp.path();
        {
            let mut conn = crate::open_db(root).expect("open");
            let (topic, _) = crate::create_topic(&mut conn, crate::Workspace::Agent, "# 书\n")
                .expect("topic");
            let parent = format!("{}/1", crate::tree_id(&topic));
            for (at, text) in [
                (parent.clone(), "# 列表\n\n第一行。\n"),
                (format!("{parent}|a"), "# 甲\n\n第一张。\n"),
            ] {
                crate::insert_card_with_term(
                    &mut conn,
                    &at,
                    false,
                    crate::Workspace::Agent,
                    crate::CardType::Idea,
                    None,
                    None,
                    text,
                )
                .expect("card");
            }
        }
        let item = |kind: &str, before: &str| {
            json!({"items": [{
                "op_key": "term-cards-0123456789abcdef/K/2/apply/r002",
                "client_ref": "r002",
                "tool": "create_successor",
                "arguments": {
                    "at": "0/1",
                    "kind": kind,
                    "before": before,
                    "type": "term",
                    "term": "丙",
                    "title": "丙",
                    "body": "术语：丙\n",
                },
            }]})
        };
        let error = apply(root, &item("direct", "0/1|a"), &mut Default::default()).unwrap_err().to_string();
        assert!(error.contains("pass kind `side`"), "{error}");
        let error = apply(root, &item("side", "0/1"), &mut Default::default()).unwrap_err().to_string();
        assert!(error.contains("immediate Side successor"), "{error}");
        let conn = crate::open_db(root).expect("open");
        assert!(
            crate::load_card(&conn, "0/1|a").expect("sibling").text.contains("# 甲\n"),
            "a refused placement moves nothing"
        );
        let receipts: i64 = conn
            .query_row("SELECT COUNT(*) FROM applied_ops", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(receipts, 0, "a refused placement stores no receipt");
        let written: i64 = conn
            .query_row("SELECT COUNT(*) FROM cards WHERE location = '0/1|b'", [], |row| row.get(0))
            .expect("count");
        assert_eq!(written, 0, "a refused placement writes no Card");
    }
    /// An agent archive: Topic `0/0` with `0/1` and its Side successors `0/1|a`, `0/1|b`, then
    /// `0/2`; Topic `1/0` with `1/1`.
    fn tree_archive() -> tempfile::TempDir {
        let temp = tempfile::tempdir().expect("tempdir");
        crate::initialize_database(temp.path()).expect("initialize");
        let root = temp.path();
        let owner = crate::Access::Owner;
        let agent = crate::Workspace::Agent;
        crate::ops::create_topic(root, owner, agent, "# 甲书\n").expect("topic");
        crate::ops::create_topic(root, owner, agent, "# 乙书\n").expect("topic");
        let make = |kind, at: &str, text: &str| {
            crate::ops::create_successor(root, owner, kind, at, crate::CardType::Idea, None, text).expect("card");
        };
        make(crate::ops::Successor::Direct, "0/0", "# 一\n\n一。\n");
        make(crate::ops::Successor::Side, "0/1", "# 甲\n\n甲。\n");
        make(crate::ops::Successor::Side, "0/1", "# 乙\n\n乙。见 [[0/1|a]]。\n");
        make(crate::ops::Successor::Direct, "0/1", "# 二\n\n二。\n");
        make(crate::ops::Successor::Direct, "1/0", "# 壹\n\n壹。\n");
        temp
    }

    fn tree_item(n: usize, tool: &str, arguments: Value) -> Value {
        json!({"op_key": format!("tree/unit/1/step/i{n}"), "client_ref": format!("t{n}"), "tool": tool, "arguments": arguments})
    }

    fn title_at(root: &Path, at: &str) -> Option<String> {
        let conn = crate::open_db(root).expect("open");
        crate::load_card(&conn, at)
            .ok()
            .map(|card| crate::parse_card_text(&card.text).expect("parse").title)
    }

    /// S3 (#231): a tree operation into a Locked tree is refused like any Lock refusal. A real batch
    /// commits nothing; its preview names the refused item, keeps checking the independent ones and
    /// reports the item that depends on the refused one as blocked.
    #[test]
    fn a_tree_operation_into_a_locked_tree_refuses_the_whole_batch_and_previews_the_refusal() {
        let temp = tree_archive();
        let root = temp.path();
        {
            let mut conn = crate::open_db(root).expect("open");
            let plan = crate::tree_lock::plan(&conn, "1/0", None).expect("lock plan");
            let tx = conn.transaction().expect("tx");
            crate::tree_lock::lock_tx(&tx, &plan).expect("lock");
            tx.commit().expect("commit");
        }
        let items = json!([
            tree_item(0, "move_card", json!({"at": "0/1|b", "before": "0/1|a"})),
            tree_item(1, "move_card", json!({"at": "0/2", "side_of": "1/1"})),
            tree_item(2, "create_successor", json!({"at": "@@t1", "kind": "side", "title": "跟随"})),
            tree_item(3, "create_successor", json!({"at": "0/1", "kind": "side", "title": "独立"})),
        ]);
        let preview = apply(root, &json!({"items": items, "preview": true}), &mut Default::default()).expect("preview");
        assert_eq!(preview["complete"], false, "{preview}");
        assert_eq!(preview["refusals"].as_array().unwrap().len(), 1, "{preview}");
        assert_eq!(preview["refusals"][0]["index"], 1);
        assert_eq!(preview["refusals"][0]["refusal"]["error"], "card_locked");
        assert_eq!(preview["blocked"][0]["index"], 2, "{preview}");
        assert_eq!(preview["scan"]["checked"], 2);
        let error = apply(root, &json!({"items": items, "confirm": preview["plan_hash"]}), &mut Default::default())
            .expect_err("a real batch stops on the Lock");
        assert!(format!("{error:#}").contains("lock"), "{error:#}");
        assert_eq!(title_at(root, "0/1|a").as_deref(), Some("甲"), "the first move rolled back");
        assert_eq!(title_at(root, "0/2").as_deref(), Some("二"));
    }

    /// S3 (#231, D3): one batch renames or deletes at most 2,000 Cards; a larger one is refused before
    /// anything is written.
    #[test]
    fn a_batch_that_renames_more_than_two_thousand_cards_is_refused() {
        let temp = tree_archive();
        let root = temp.path();
        {
            let conn = crate::open_db(root).expect("open");
            let tx = conn.unchecked_transaction().expect("tx");
            let mut at = "0/1|a".to_string();
            for n in 0..2001 {
                at = crate::direct_successor(&at).expect("next");
                tx.execute(
                    "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type) VALUES(?1, NULL, 0, 0, NULL, ?2, 'agent', 'idea')",
                    rusqlite::params![at, format!("# 行{n}\n\n<!-- zt:reverse-links -->\n")],
                )
                .expect("chain card");
            }
            crate::link_index::rebuild(&tx).expect("index the seed");
            tx.commit().expect("commit");
        }
        let items = json!([tree_item(0, "move_card", json!({"at": "0/1|a", "side_of": "1/1"}))]);
        let error = apply(root, &json!({"items": items, "preview": true}), &mut Default::default())
            .expect_err("over the cap");
        assert!(format!("{error:#}").contains("at most 2000"), "{error:#}");
        assert_eq!(title_at(root, "0/1|a").as_deref(), Some("甲"));
    }

    /// S3 (#231): a committed tree-operation batch replays completely; a request that mixes its old
    /// receipts with a new tree operation is refused, because the old literals named another start.
    #[test]
    fn a_tree_operation_batch_replays_completely_or_not_at_all() {
        let temp = tree_archive();
        let root = temp.path();
        let first = json!([tree_item(0, "move_card", json!({"at": "0/1|b", "before": "0/1|a"}))]);
        let plan = apply(root, &json!({"items": first, "preview": true}), &mut Default::default()).expect("preview");
        let done = apply(root, &json!({"items": first, "confirm": plan["plan_hash"]}), &mut Default::default())
            .expect("commit");
        assert_eq!(done["applied"], true, "{done}");
        assert_eq!(title_at(root, "0/1|a").as_deref(), Some("乙"));
        let replay = apply(root, &json!({"items": first, "confirm": plan["plan_hash"]}), &mut Default::default())
            .expect("replay");
        assert_eq!(replay["results"][0]["replayed"], true);
        let mixed = json!([
            tree_item(0, "move_card", json!({"at": "0/1|b", "before": "0/1|a"})),
            tree_item(1, "move_card", json!({"at": "0/2", "side_of": "1/1"})),
        ]);
        let error = apply(root, &json!({"items": mixed, "preview": true}), &mut Default::default())
            .expect_err("mixed replay");
        assert!(format!("{error:#}").contains("replays completely or not at all"), "{error:#}");
    }
    /// Every Regular Card by its unique title: its address and the titles its Links reach (`?` for a
    /// target that names no Card).
    fn identities(root: &Path) -> BTreeMap<String, (String, Vec<String>)> {
        let conn = crate::open_db(root).expect("open");
        let cards = crate::load_cards(&conn).expect("cards");
        let titles: BTreeMap<String, String> = cards
            .iter()
            .map(|card| (card.address.clone(), crate::parse_card_text(&card.text).expect("parse").title))
            .collect();
        cards
            .iter()
            .filter(|card| !crate::is_tree_root(card))
            .map(|card| {
                let body = crate::parse_card_text(&card.text).expect("parse").body;
                let links = crate::link::LinkBody::parse(&body)
                    .expect("links")
                    .occurrences()
                    .iter()
                    .map(|o| titles.get(o.target).cloned().unwrap_or_else(|| "?".to_string()))
                    .collect();
                (titles[&card.address].clone(), (card.address.clone(), links))
            })
            .collect()
    }

    /// S3 (#231): random batches of up to eight tree operations, every address read on the start
    /// state. After each commit every surviving Card stands where the composed plan says, no deleted
    /// Card remains, and every Link reaches the same Card it reached before; in test builds the
    /// batch's one refresh also proves the Link index against a fresh parse.
    #[test]
    fn random_tree_batches_keep_every_card_where_the_plan_says_and_every_link_on_its_card() {
        let mut seen = BTreeSet::new();
        for seed in [3_u64, 11, 29, 57, 101, 211, 307, 401] {
            let temp = tree_archive();
            let root = temp.path();
            let owner = crate::Access::Owner;
            let mut n = 0;
            for (at, kind) in [("0/1|a", "side"), ("0/1|a", "direct"), ("0/2", "side"), ("0/2", "direct"), ("1/1", "side"), ("1/1", "direct"), ("0/2|a", "side")] {
                let kind = if kind == "side" { crate::ops::Successor::Side } else { crate::ops::Successor::Direct };
                let links = ["0/1", "0/1|a", "0/2", "1/1"][n % 4];
                let text = format!("# 卡{n}\n\n卡{n}的正文。见 [[{links}]]。\n");
                let _ = crate::ops::create_successor(root, owner, kind, at, crate::CardType::Idea, None, &text);
                n += 1;
            }
            let before = identities(root);
            let start: Vec<String> = before.values().map(|(at, _)| at.clone()).collect();
            let mut state = seed;
            let mut next = || {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (state >> 33) as usize
            };
            let mut items: Vec<Value> = Vec::new();
            for attempt in 0..40 {
                if items.len() == 8 {
                    break;
                }
                let a = start[next() % start.len()].clone();
                let b = start[next() % start.len()].clone();
                // A span runs from `a` to the next Card on its chain when the start state has one.
                let longer = next() % 2 == 0;
                let through = crate::direct_successor(&a)
                    .ok()
                    .filter(|after| longer && start.contains(after))
                    .unwrap_or_else(|| a.clone());
                // A reorder reverses the Side successors of a Card that has at least two.
                let side_children = |parent: &String| -> Vec<String> {
                    let mut children: Vec<String> = start
                        .iter()
                        .filter(|at| crate::parent_location(at).as_deref() == Some(parent.as_str()))
                        .filter(|at| crate::immediate_side_label(at, parent).is_some())
                        .cloned()
                        .collect();
                    children.sort_by_key(|at| crate::side_label_to_number(crate::last_segment(at)));
                    children.reverse();
                    children
                };
                let parents: Vec<&String> = start.iter().filter(|at| side_children(at).len() >= 2).collect();
                let parent = parents[next() % parents.len()].clone();
                let children = side_children(&parent);
                // Each batch opens with a reorder, so the later items must follow its renames.
                let kind = if attempt == 0 { 9 } else { next() % 10 };
                let (tool, arguments) = match kind {
                    0 => ("move_card", json!({"at": a, "before": b})),
                    1 => ("move_card", json!({"at": a, "side_of": b})),
                    2 => ("move_card", json!({"at": a, "after": b})),
                    3 => ("move_card", json!({"at": a, "before": b, "alone": true})),
                    4 => ("delete_card", json!({"at": a, "into": b})),
                    5 => ("move_span", json!({"from": a, "through": through, "side_of": b})),
                    6 => ("indent", json!({"at": a, "through": through})),
                    7 => ("outdent", json!({"at": a})),
                    8 => ("delete_span", json!({"from": a, "through": through, "into": b})),
                    _ => ("reorder", json!({"parent": parent, "side": children})),
                };
                let mut candidate = items.clone();
                candidate.push(json!({"op_key": format!("rand/s{seed}/1/step/i{attempt}"), "client_ref": format!("r{attempt}"), "tool": tool, "arguments": arguments}));
                if let Ok(preview) = apply(root, &json!({"items": candidate, "preview": true}), &mut Default::default())
                    && preview["complete"] == true
                {
                    items = candidate;
                }
            }
            assert!(items.len() >= 3, "seed {seed}: only {} items formed a valid batch", items.len());
            let preview = apply(root, &json!({"items": items, "preview": true}), &mut Default::default()).expect("preview");
            let done = apply(root, &json!({"items": items, "confirm": preview["plan_hash"]}), &mut Default::default())
                .expect("commit");
            assert_eq!(done["plan"], preview["plan"], "seed {seed}: the commit ran the previewed plan");
            seen.extend(items.iter().map(|item| item["tool"].as_str().unwrap_or_default().to_string()));
            let plan = &done["plan"];
            let deleted: BTreeSet<String> = plan["deleted"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
            let gone_titles: BTreeSet<&String> = before.iter().filter(|(_, (at, _))| deleted.contains(at)).map(|(title, _)| title).collect();
            let after = identities(root);
            for (title, (at, links)) in &before {
                if deleted.contains(at) {
                    assert!(!after.contains_key(title), "seed {seed}: deleted {title} remains");
                    continue;
                }
                let expected = plan["moved"].get(at).and_then(Value::as_str).unwrap_or(at);
                let (now, links_now) = after.get(title).unwrap_or_else(|| panic!("seed {seed}: {title} vanished"));
                assert_eq!(now, expected, "seed {seed}: {title} is not where the plan says: {plan}");
                for (old, new) in links.iter().zip(links_now) {
                    if !gone_titles.contains(old) {
                        assert_eq!(old, new, "seed {seed}: a Link of {title} changed its Card");
                    }
                }
            }
        }
        for tool in ["move_card", "delete_card", "move_span", "indent", "outdent", "delete_span", "reorder"] {
            assert!(seen.contains(tool), "no committed batch exercised {tool}: {seen:?}");
        }
    }
    /// S5 (#233): a reorder item swaps two Side branches inside a batch; its addresses are read on
    /// the start state like every other item.
    #[test]
    fn a_reorder_item_swaps_two_side_branches() {
        let temp = tree_archive();
        let root = temp.path();
        let items = json!([tree_item(0, "reorder", json!({"parent": "0/1", "side": ["0/1|b", "0/1|a"]}))]);
        let preview = apply(root, &json!({"items": items, "preview": true}), &mut Default::default()).expect("preview");
        assert_eq!(preview["plan"]["moved"], json!({"0/1|a": "0/1|b", "0/1|b": "0/1|a"}), "{preview}");
        apply(root, &json!({"items": items, "confirm": preview["plan_hash"]}), &mut Default::default()).expect("commit");
        assert_eq!(title_at(root, "0/1|a").as_deref(), Some("乙"));
        assert_eq!(title_at(root, "0/1|b").as_deref(), Some("甲"));
    }
}
