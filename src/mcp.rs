//! `zt mcp`: a Model Context Protocol server for AI agents.
//!
//! The server speaks JSON-RPC 2.0 over stdio, one message per line. Its tools
//! read every Card in both Workspaces, but change only the agent Workspace:
//! the human Workspace is read-only here. Changes go through the same
//! operations as the GUI, so every Card rule still holds, and destructive
//! changes need the same typed confirmation, passed as `confirm`.

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use crate::model::{Emphasis, Facet};
use crate::ops::{self, Outcome, Snapshot, Successor};
use crate::{Access, CardType, Workspace};

/// Every change made through MCP is limited to the agent Workspace.
const ACCESS: Access = Access::Agent;
const SERVER_NAME: &str = "zt";
const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "ZT is a slip-box of addressable Markdown Cards split into two Workspaces: \
`human` (read-only for you) and `agent` (yours to read and change). Each Workspace has two Zones: \
Topic trees (rooted at a Topic such as `3/0`) and Literature trees (rooted at a Literature Card \
named by its BibTeX Citation key, such as `Smith2024`). Card addresses are unique across both \
Workspaces: Locations look like `3/1`, `3/2|a`, or `Smith2024/1`. Link Cards by writing \
`[[address]]` in Markdown bodies; Links may cross Workspaces, but Cards and trees never move \
between them. Start with list_workspaces and list_trees, read with get_card or search_cards, and \
create new trees with create_topic or create_literature, then grow them with create_successor. \
Most Cards are Idea Cards (type `idea`); a Regular Card may instead be an Event Card (`event`: one \
real-world event with its date, place, actors, and source) or a Model Card (`model`: one whole \
model), set with `type` on create_successor or edit_card and listed with search_cards by `type`. \
A model is presented by its Model Card (its entry point: the question and what the model shows) and \
by Model parts: Model Cards with a `facet`, a short label you choose for what the part presents in \
this model (`设定`, `均衡`, `数值例子`), and optionally an `emphasis` (`core` for what a reader must \
see, `aside` for detail). Where a part is filed does not decide its model: a part belongs to the \
Model Card it is filed under and to every Model Card that links it, so file it where it first \
connects and let one part serve several models. Every Link says what it is by the words before it \
in its sentence: rests on (`依赖`, `基于`), illustrates (`例示`, `案例`), contrasts (`对照`, `区别`), \
relaxes (`放宽`, `推广`), corresponds (`对应`, `类比`), objects (`反驳`, `批评`); with none it is a \
plain reference. get_model shows a model's parts, their kinded relations, its loose parts, and its \
Links with Cards outside it. Card bodies may hold \
TeX math in `$...$` or `$$...$$` and HTML charts in ```chart fences, which the GUI renders. \
delete_card, move_card, and Citation key renames return a plan first; repeat the call with the \
`confirm` text the plan names to apply it. The scholar_archive_* tools read the local Scholar Archive \
(scholars, Works, abstracts, journals, workspaces, PDF/Markdown documents) through its read-only \
bridge; file a paper you use as a Literature Card and cite its DOI. To show a paper's figure in a Card, list the Work's figures with scholar_archive_read `list_work_artifacts`, take one with scholar_archive_artifact, and paste the `![caption](artifact:<sha256>)` Markdown it returns.";

pub(crate) fn run() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut server = Server::default();
    for line in stdin.lock().lines() {
        let line = line.context("failed to read an MCP message")?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = server.handle_line(&line) {
            writeln!(stdout, "{reply}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

#[derive(Default)]
struct Server {
    initialized: bool,
}

impl Server {
    /// Answer one line: a request, a notification, or a batch of them.
    fn handle_line(&mut self, line: &str) -> Option<String> {
        let message: Value = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(error) => {
                return Some(
                    error_response(Value::Null, -32700, &format!("parse error: {error}"))
                        .to_string(),
                );
            }
        };
        match message {
            Value::Array(batch) => {
                let replies = batch
                    .into_iter()
                    .filter_map(|message| self.handle(message))
                    .collect::<Vec<_>>();
                (!replies.is_empty()).then(|| Value::Array(replies).to_string())
            }
            message => self.handle(message).map(|reply| reply.to_string()),
        }
    }

    fn handle(&mut self, message: Value) -> Option<Value> {
        let Some(object) = message.as_object() else {
            return Some(error_response(
                Value::Null,
                -32600,
                "invalid request: expected a JSON object",
            ));
        };
        let id = object.get("id").cloned();
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            // A response from the client: nothing to answer.
            return id.map(|id| error_response(id, -32600, "invalid request: missing method"));
        };
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        let Some(id) = id else {
            if method == "notifications/initialized" {
                self.initialized = true;
            }
            return None;
        };
        Some(match self.request(method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(RpcError { code, message }) => error_response(id, code, &message),
        })
    }

    fn request(&mut self, method: &str, params: &Value) -> std::result::Result<Value, RpcError> {
        match method {
            "initialize" => {
                let requested = params.get("protocolVersion").and_then(Value::as_str);
                let version = requested
                    .filter(|version| PROTOCOL_VERSIONS.contains(version))
                    .unwrap_or(PROTOCOL_VERSIONS[0]);
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": SERVER_NAME,
                        "title": "ZT slip-box",
                        "version": crate::VERSION,
                    },
                    "instructions": INSTRUCTIONS,
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tool_definitions() })),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| RpcError::invalid_params("missing tool `name`"))?;
                let empty = Value::Object(Map::new());
                let arguments = params.get("arguments").unwrap_or(&empty);
                if !arguments.is_object() {
                    return Err(RpcError::invalid_params(
                        "tool `arguments` must be an object",
                    ));
                }
                if !tool_definitions()
                    .iter()
                    .any(|tool| tool["name"].as_str() == Some(name))
                {
                    return Err(RpcError::invalid_params(&format!("unknown tool `{name}`")));
                }
                Ok(match call_tool(name, arguments) {
                    Ok(value) => tool_result(value, false),
                    Err(error) => tool_result(json!({ "error": format!("{error:#}") }), true),
                })
            }
            "resources/list" => Ok(json!({ "resources": [] })),
            "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
            "prompts/list" => Ok(json!({ "prompts": [] })),
            other => Err(RpcError {
                code: -32601,
                message: format!("method not found: {other}"),
            }),
        }
    }
}

struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn invalid_params(message: &str) -> Self {
        Self {
            code: -32602,
            message: message.to_string(),
        }
    }
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn tool_result(value: Value, is_error: bool) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    let mut result = json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    });
    if value.is_object() {
        result["structuredContent"] = value;
    }
    result
}

fn archive_root() -> Result<PathBuf> {
    crate::require_service_up().context("the ZT service is not available; run `zt up`")
}

fn call_tool(name: &str, args: &Value) -> Result<Value> {
    if name == "scholar_archive_artifact" {
        let root = archive_root()?;
        let file_id = args
            .get("file_id")
            .and_then(Value::as_u64)
            .context("missing integer argument `file_id`")?;
        let path = string_arg(args, "path")?;
        let caption = args.get("caption").and_then(Value::as_str);
        let mut bridge = crate::scholar::Bridge::connect()?;
        return crate::scholar::keep_artifact(&mut bridge, &root, file_id, &path, caption);
    }
    if name.starts_with("scholar_archive_") {
        return call_scholar_tool(name, args);
    }
    let root = archive_root()?;
    match name {
        "list_workspaces" => {
            let snapshot = Snapshot::read(&root)?;
            let workspaces = Workspace::ALL
                .into_iter()
                .map(|workspace| {
                    let counts = crate::workspace::counts(&snapshot.cards, workspace);
                    json!({
                        "id": workspace.id(),
                        "label": workspace.label(),
                        "access": if ACCESS.can_write(workspace) { "read-write" } else { "read-only" },
                        "zones": {
                            "topic": { "trees": counts.topic_trees },
                            "literature": { "trees": counts.literature_trees },
                        },
                        "cards": counts.cards,
                    })
                })
                .collect::<Vec<_>>();
            Ok(json!({
                "workspaces": workspaces,
                "cards": snapshot.cards.len(),
                "broken_links": snapshot.lifecycle.broken_links()?.len(),
            }))
        }
        "list_trees" => {
            let snapshot = Snapshot::read(&root)?;
            let workspace = optional_workspace(args)?;
            let zone = optional_zone(args)?;
            let trees = snapshot
                .trees()
                .into_iter()
                .filter(|tree| workspace.is_none_or(|ws| tree["workspace"] == ws.id()))
                .filter(|tree| zone.is_none_or(|zone| tree["kind"] == zone))
                .collect::<Vec<_>>();
            Ok(json!({ "trees": trees }))
        }
        "get_tree" => {
            let snapshot = Snapshot::read(&root)?;
            let tree = string_arg(args, "tree")?;
            let tree = crate::tree_id(&tree).to_string();
            let root_address = crate::tree_root_address(&tree);
            let Some(root_card) = snapshot.by_address.get(&root_address) else {
                bail!("tree `{tree}` does not exist");
            };
            let cards = snapshot
                .cards
                .iter()
                .filter(|card| crate::is_tree_member(&card.address, &tree))
                .map(|card| snapshot.summary(card))
                .collect::<Vec<_>>();
            Ok(json!({
                "tree": tree,
                "root": root_address,
                "workspace": root_card.workspace.id(),
                "zone": ops::zone(root_card),
                "title": snapshot.title(&root_address),
                "cards": cards,
            }))
        }
        "get_card" => {
            let snapshot = Snapshot::read(&root)?;
            let at = string_arg(args, "at")?;
            let card = snapshot
                .by_address
                .get(&at)
                .with_context(|| format!("card `{at}` does not exist"))?;
            let parsed = &snapshot.parsed[&card.address];
            Ok(json!({
                "address": card.address,
                "tree": crate::tree_id(&card.address),
                "workspace": card.workspace.id(),
                "writable": ACCESS.can_write(card.workspace),
                "kind": ops::kind(card),
                "type": card.card_type.id(),
                "facet": card.facet.as_ref().map(Facet::id),
                "emphasis": card.emphasis.map(Emphasis::id),
                "model": snapshot.model(&card.address),
                "title": parsed.title,
                "body": parsed.body,
                "text": card.text,
                "bibtex": card.bibtex,
                "citation_key": card.citation_key,
                "parent": snapshot.nav(crate::parent_location(&card.address)),
                "direct": snapshot.nav(crate::direct_successor(&card.address).ok()),
                "sides": snapshot.sides(card),
                "outbound": snapshot.outbound(&card.address),
                "inbound": snapshot.inbound_refs(&card.address),
            }))
        }
        "get_model" => {
            let snapshot = Snapshot::read(&root)?;
            let at = string_arg(args, "at")?;
            if !snapshot.by_address.contains_key(&at) {
                bail!("card `{at}` does not exist");
            }
            let mut model = snapshot.model(&at).with_context(|| {
                format!("`{at}` is not a Model Card or Model part; set type `model` first")
            })?;
            // Each Card of the model with its body, so one call reads the whole model.
            let body = |address: &str| {
                snapshot
                    .parsed
                    .get(address)
                    .map(|parsed| parsed.body.clone())
                    .unwrap_or_default()
            };
            if let Some(head) = model["head"]["address"].as_str().map(str::to_string) {
                model["head"]["body"] = json!(body(&head));
            }
            if let Some(parts) = model["parts"].as_array_mut() {
                for part in parts {
                    if let Some(address) = part["address"].as_str().map(str::to_string) {
                        part["body"] = json!(body(&address));
                    }
                }
            }
            Ok(model)
        }
        "search_cards" => {
            let snapshot = Snapshot::read(&root)?;
            let card_type = optional_card_type(args)?;
            // With a type, the query may be left out to list every Card of that type.
            let query = match card_type {
                Some(_) => optional_string(args, "query")?,
                None => string_arg(args, "query")?,
            };
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(30)
                .clamp(1, 200) as usize;
            Ok(json!({
                "results": snapshot.search(&query, optional_workspace(args)?, card_type, limit),
            }))
        }
        "list_links" => {
            let snapshot = Snapshot::read(&root)?;
            let workspace = optional_workspace(args)?;
            let cross_only = args
                .get("cross_workspace_only")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut links = Vec::new();
            for card in &snapshot.cards {
                for link in snapshot.outbound(&card.address) {
                    let target_workspace = link["workspace"].as_str();
                    let involved = workspace
                        .is_none_or(|ws| card.workspace == ws || target_workspace == Some(ws.id()));
                    let crosses = target_workspace.is_some_and(|ws| ws != card.workspace.id());
                    if involved && (!cross_only || crosses) {
                        links.push(json!({
                            "source": card.address,
                            "source_workspace": card.workspace.id(),
                            "target": link["address"],
                            "target_workspace": link["workspace"],
                            "broken": link["broken"],
                        }));
                    }
                }
            }
            Ok(json!({ "links": links }))
        }
        "list_broken_links" => {
            let snapshot = Snapshot::read(&root)?;
            Ok(json!({ "broken": snapshot.broken(optional_workspace(args)?)? }))
        }
        "create_topic" => {
            let workspace = target_workspace(args)?;
            let title = title_arg(args)?;
            let text = crate::compose_card_text(&title, &optional_string(args, "body")?, "");
            crate::artifact::require_present(&root, &text)?;
            let address = ops::create_topic(&root, ACCESS, workspace, &text)?;
            Ok(json!({ "address": address, "workspace": workspace.id() }))
        }
        "create_literature" => {
            let workspace = target_workspace(args)?;
            let bibtex = string_arg(args, "bibtex")?;
            let body = optional_string(args, "body")?;
            crate::artifact::require_present(&root, &body)?;
            let address = ops::create_literature(&root, ACCESS, workspace, &bibtex, &body)?;
            Ok(json!({ "address": address, "workspace": workspace.id() }))
        }
        "create_successor" => {
            let at = string_arg(args, "at")?;
            let kind = match optional_string(args, "kind")?.as_str() {
                "" | "direct" => Successor::Direct,
                "side" => Successor::Side,
                other => bail!("unknown successor kind `{other}`; expected direct or side"),
            };
            let part = crate::model::Part::from_args(
                optional_facet(args)?.flatten(),
                optional_emphasis(args)?.flatten(),
            )?;
            let card_type = match (optional_card_type(args)?, &part) {
                (Some(card_type), Some(_)) if card_type != CardType::Model => {
                    bail!("only Model Cards can have a Model facet")
                }
                (_, Some(_)) => CardType::Model,
                (card_type, None) => card_type.unwrap_or_default(),
            };
            let title = title_arg(args)?;
            let text = crate::compose_card_text(&title, &optional_string(args, "body")?, "");
            crate::artifact::require_present(&root, &text)?;
            let address =
                ops::create_successor(&root, ACCESS, kind, &at, card_type, part.clone(), &text)?;
            Ok(json!({
                "address": address,
                "type": card_type.id(),
                "facet": part.as_ref().map(|part| part.facet.id()),
                "emphasis": part.and_then(|part| part.emphasis).map(Emphasis::id),
            }))
        }
        "edit_card" => {
            let at = string_arg(args, "at")?;
            let conn = crate::open_db(&root)?;
            let card = crate::load_card(&conn, &at)?;
            drop(conn);
            ACCESS.ensure_writable(card.workspace)?;
            let change = ops::TypeChange {
                card_type: optional_card_type(args)?,
                facet: optional_facet(args)?,
                emphasis: optional_emphasis(args)?,
            };
            let parsed = crate::parse_card_text(&card.text)?;
            let text_given = ["text", "title", "body"]
                .iter()
                .any(|name| args.get(name).is_some());
            let text = match args.get("text").and_then(Value::as_str) {
                // A type or facet alone changes only the type.
                _ if !change.is_empty() && !text_given => None,
                Some(text) => Some(text.to_string()),
                None => {
                    if card.is_lit && args.get("title").is_some() {
                        bail!(
                            "a Literature Card takes its title from BibTeX; change it with edit_literature_metadata"
                        );
                    }
                    let title = match args.get("title") {
                        Some(_) => title_arg(args)?,
                        None => parsed.title.clone(),
                    };
                    let body = match args.get("body") {
                        Some(_) => optional_string(args, "body")?,
                        None => parsed.body.clone(),
                    };
                    Some(crate::compose_card_text(&title, &body, ""))
                }
            };
            if let Some(text) = &text {
                crate::artifact::require_present(&root, text)?;
            }
            let base = args.get("base").and_then(Value::as_str);
            let address = ops::edit(&root, ACCESS, &at, text.as_deref(), change, base)?;
            Ok(json!({ "address": address }))
        }
        "edit_literature_metadata" => {
            let outcome = ops::edit_metadata(
                &root,
                ACCESS,
                &string_arg(args, "at")?,
                &string_arg(args, "bibtex")?,
                None,
                args.get("confirm").and_then(Value::as_str),
            )?;
            Ok(plan_or_done(outcome))
        }
        "delete_card" => {
            let outcome = ops::delete(
                &root,
                ACCESS,
                &string_arg(args, "at")?,
                args.get("confirm").and_then(Value::as_str),
            )?;
            Ok(plan_or_done(outcome))
        }
        "move_card" => {
            let outcome = ops::move_card(
                &root,
                ACCESS,
                &string_arg(args, "at")?,
                &string_arg(args, "to")?,
                args.get("confirm").and_then(Value::as_str),
            )?;
            Ok(plan_or_done(outcome))
        }
        other => bail!("unknown tool `{other}`"),
    }
}

/// A plan that still needs confirmation says so plainly; nothing has changed.
fn plan_or_done(outcome: Outcome) -> Value {
    match outcome {
        Outcome::Confirm { confirm, lines } => json!({
            "applied": false,
            "confirm": confirm,
            "plan": lines,
            "next": format!("nothing changed yet; call again with confirm = \"{confirm}\" to apply"),
        }),
        Outcome::Done(mut value) => {
            value["applied"] = json!(true);
            value
        }
    }
}

fn string_arg(args: &Value, name: &str) -> Result<String> {
    args.get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("missing string argument `{name}`"))
}

fn optional_string(args: &Value, name: &str) -> Result<String> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(value)) => Ok(value.clone()),
        Some(_) => bail!("argument `{name}` must be a string"),
    }
}

fn title_arg(args: &Value) -> Result<String> {
    let title = string_arg(args, "title")?;
    let title = title.trim();
    if title.is_empty() || title.contains(['\n', '\r']) {
        bail!("title must be non-empty single-line text");
    }
    Ok(title.to_string())
}

fn optional_workspace(args: &Value) -> Result<Option<Workspace>> {
    match args.get("workspace") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value == "all" => Ok(None),
        Some(Value::String(value)) => Workspace::parse(value).map(Some),
        Some(_) => bail!("argument `workspace` must be a string"),
    }
}

/// New trees go to the agent Workspace; naming another one is refused.
fn target_workspace(args: &Value) -> Result<Workspace> {
    let workspace = optional_workspace(args)?.unwrap_or(Workspace::Agent);
    ACCESS.ensure_writable(workspace)?;
    Ok(workspace)
}

fn optional_card_type(args: &Value) -> Result<Option<CardType>> {
    match args.get("type") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => CardType::parse(value).map(Some),
        Some(_) => bail!("argument `type` must be a string"),
    }
}

/// `None` when absent; `Some(None)` for an empty string or `none`, which
/// clears a facet.
fn optional_facet(args: &Value) -> Result<Option<Option<Facet>>> {
    match args.get("facet") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(value)) => crate::model::parse_optional(value).map(Some),
        Some(_) => bail!("argument `facet` must be a string"),
    }
}

/// `None` when absent; `Some(None)` for an empty string, `none`, or
/// `regular`, which clears an emphasis.
fn optional_emphasis(args: &Value) -> Result<Option<Option<Emphasis>>> {
    match args.get("emphasis") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(value)) => crate::model::parse_optional_emphasis(value).map(Some),
        Some(_) => bail!("argument `emphasis` must be a string"),
    }
}

fn optional_zone(args: &Value) -> Result<Option<&'static str>> {
    match args.get("zone").and_then(Value::as_str) {
        None | Some("all") => Ok(None),
        Some("topic") | Some("topics") => Ok(Some("topic")),
        Some("literature") => Ok(Some("literature")),
        Some(other) => bail!("unknown zone `{other}`; expected topic or literature"),
    }
}

fn tool(
    name: &str,
    title: &str,
    description: &str,
    properties: Value,
    required: &[&str],
    read_only: bool,
    destructive: bool,
) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
        "annotations": {
            "title": title,
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "idempotentHint": read_only,
            "openWorldHint": false,
        },
    })
}

fn tool_definitions() -> Vec<Value> {
    let workspace_filter = json!({
        "type": "string",
        "enum": ["human", "agent", "all"],
        "description": "Only this Workspace; omit for both.",
    });
    let at = json!({
        "type": "string",
        "description": "A Card address: a Location such as `3/1|a` or a Citation key such as `Smith2024`.",
    });
    let confirm = json!({
        "type": "string",
        "description": "The confirmation text returned by the plan. Omit it to see the plan without changing anything.",
    });
    let body = json!({
        "type": "string",
        "description": "Markdown body. Write `[[address]]` to link any Card in either Workspace.",
    });
    let facet = |description: &str| {
        json!({
            "type": "string",
            "maxLength": Facet::MAX_CHARS,
            "description": format!(
                "{description} A short label of your choice (at most {} characters), in the Card's language, naming what this part presents in this model (`设定`, `均衡`, `数值例子`, `两种情形`); no fixed list or order.",
                Facet::MAX_CHARS
            ),
        })
    };
    let emphasis = |description: &str| {
        json!({
            "type": "string",
            "enum": ["core", "aside", ""],
            "description": description,
        })
    };
    let card_type = |description: &str| {
        json!({
            "type": "string",
            "enum": ["idea", "event", "model"],
            "description": description,
        })
    };
    vec![
        tool(
            "list_workspaces",
            "List workspaces",
            "List the human and agent Workspaces with their Topic and Literature Zones, Card counts, and your access (the human Workspace is read-only).",
            json!({}),
            &[],
            true,
            false,
        ),
        tool(
            "list_trees",
            "List trees",
            "List tree roots (Topics and Literature Cards) with their Workspace, Zone, title, and size.",
            json!({
                "workspace": workspace_filter,
                "zone": { "type": "string", "enum": ["topic", "literature", "all"], "description": "Only this Zone; omit for both." },
            }),
            &[],
            true,
            false,
        ),
        tool(
            "get_tree",
            "Get tree",
            "Every Card of one tree, root first, with its type (idea, event, model), edge (root, direct, side), parent, excerpt, and Links.",
            json!({ "tree": { "type": "string", "description": "A tree id (Topic number or Citation key) or any address in the tree." } }),
            &["tree"],
            true,
            false,
        ),
        tool(
            "get_card",
            "Get card",
            "Read one Card: its title, Markdown body, stored text, type (idea, event, or model), Model facet, BibTeX, Workspace, whether you may change it, neighbors, and Links in and out. A Model Card or Model part also reports `model`: its model's structure.",
            json!({ "at": at }),
            &["at"],
            true,
            false,
        ),
        tool(
            "get_model",
            "Get model",
            "Read a whole model from its Model Card or any of its Model parts: `models` (every model the Card belongs to; the first is shown); the Model Card; its parts, in the order the Model Card names them and then in filing order, with their facets, emphasis, and bodies, whether each is filed under the Model Card or joined by its Link (`filed`), and the other models it serves (`also`); the `relations` between parts (each Link from one part to another, with its `kind` (rests-on, illustrates, contrasts, relaxes, corresponds, objects, or refers) and the sentence that makes it); the `loose` parts in no relation; the `neighbours`, Links between the model and Cards outside it either way, with kind and sentence; how many outside Cards discuss it; and `issues` (a part that belongs to no model).",
            json!({ "at": at }),
            &["at"],
            true,
            false,
        ),
        tool(
            "search_cards",
            "Search cards",
            "Case-insensitive full-text search over Card addresses, titles, and bodies. Each result names the Card's type. With `type`, only Cards of that type match, and `query` may be empty or left out to list every Card of that type.",
            json!({
                "query": { "type": "string", "description": "Text to find; required unless `type` is given." },
                "type": card_type("Only Cards of this type: idea, event (Event Cards), or model (Model Cards)."),
                "workspace": workspace_filter,
                "limit": { "type": "integer", "minimum": 1, "maximum": 200, "description": "Default 30." },
            }),
            &[],
            true,
            false,
        ),
        tool(
            "list_links",
            "List links",
            "Every `[[target]]` Link with the Workspace at each end. Use cross_workspace_only to see how the two Workspaces connect.",
            json!({
                "workspace": workspace_filter,
                "cross_workspace_only": { "type": "boolean" },
            }),
            &[],
            true,
            false,
        ),
        tool(
            "list_broken_links",
            "List broken links",
            "Links whose target Card no longer exists.",
            json!({ "workspace": workspace_filter }),
            &[],
            true,
            false,
        ),
        tool(
            "create_topic",
            "Create topic",
            "Start a new Topic tree in the agent Workspace's Topic Zone. Returns the new Topic Location, such as `7/0`.",
            json!({
                "title": { "type": "string", "description": "The Topic name (one line)." },
                "body": { "type": "string", "description": "A short description in Markdown." },
                "workspace": { "type": "string", "enum": ["agent"], "description": "Always the agent Workspace." },
            }),
            &["title"],
            false,
            false,
        ),
        tool(
            "create_literature",
            "Create literature card",
            "Start a new Literature tree in the agent Workspace's Literature Zone from one BibTeX entry. Its entry key becomes the Citation key and its `title` field the Card title.",
            json!({
                "bibtex": { "type": "string", "description": "Exactly one BibTeX entry with a title field, e.g. `@book{Smith2024, title = {Example}}`." },
                "body": body,
                "workspace": { "type": "string", "enum": ["agent"], "description": "Always the agent Workspace." },
            }),
            &["bibtex"],
            false,
            false,
        ),
        tool(
            "create_successor",
            "Create successor card",
            "Add a Card to an agent tree after the Card at `at`: a direct successor continues the line (`3/1` -> `3/2`), a side successor branches (`3/1` -> `3/1|a`; not from a tree root). Set `type` to `event` for an Event Card (one real-world event) or `model` for a Model Card (one whole model). Set `facet` to make a Model part: a Model Card presenting one part of a model. It belongs to the Model Card it is filed under, if any, and to every Model Card that links it, so it may be filed wherever it first connects; `emphasis` marks it `core` or `aside`.",
            json!({
                "at": at,
                "kind": { "type": "string", "enum": ["direct", "side"], "description": "Default direct." },
                "title": { "type": "string", "description": "The Card title (one line)." },
                "body": body,
                "type": card_type("The Card type: idea (default), event, or model."),
                "facet": facet("Make the new Card a Model part presenting this facet of its model (implies type model)."),
                "emphasis": emphasis("For a Model part: core (the heart of the model, what a reader must see first) or aside (detail a reader may skip); omit for a regular part."),
            }),
            &["at", "title"],
            false,
            false,
        ),
        tool(
            "edit_card",
            "Edit card",
            "Change an agent Card. Pass `title` and/or `body` to replace just those parts, or `text` for the whole Markdown document (`# Title` then the body). Pass `type` to make a Regular Card an Idea, Event, or Model Card, `facet` to make a Model Card a Model part or rename its facet (an empty `facet` makes it a Model Card again), and `emphasis` to mark a part core or aside (empty for regular); each may be given alone. Reverse links are regenerated automatically.",
            json!({
                "at": at,
                "title": { "type": "string", "description": "New title (not for Literature Cards)." },
                "body": body,
                "type": card_type("New Card type: idea, event, or model (event and model only for Regular Cards)."),
                "facet": facet("New Model facet (implies type model); empty for none."),
                "emphasis": emphasis("New emphasis of a Model part: core, aside, or empty for a regular part."),
                "text": { "type": "string", "description": "The whole Card Markdown; overrides title and body." },
                "base": { "type": "string", "description": "The stored text you read; refuse the edit if the Card changed since." },
            }),
            &["at"],
            false,
            false,
        ),
        tool(
            "edit_literature_metadata",
            "Edit literature metadata",
            "Replace the BibTeX of an agent Literature Card. A new entry key renames the whole Literature tree and rewrites Links to it, so it first returns a plan to confirm with `move`.",
            json!({ "at": at, "bibtex": { "type": "string" }, "confirm": confirm }),
            &["at", "bibtex"],
            false,
            true,
        ),
        tool(
            "delete_card",
            "Delete card",
            "Delete an agent Card and all its successors; deleting a tree root deletes the whole tree. Returns a plan first; confirm with the text it names (`delete`, or the root's address).",
            json!({ "at": at, "confirm": confirm }),
            &["at"],
            false,
            true,
        ),
        tool(
            "move_card",
            "Move card",
            "Move an agent Regular Card and its successors to a new Location in the agent Workspace, rewriting Links. Literature tree Cards stay in their tree; nothing moves between Workspaces. Returns a plan first; confirm with `move`.",
            json!({
                "at": at,
                "to": { "type": "string", "description": "The new Location: the next direct or side successor slot of an existing Card." },
                "confirm": confirm,
            }),
            &["at", "to"],
            false,
            true,
        ),
        scholar_tool(
            "scholar_archive_status",
            "Scholar Archive status",
            "Connect to the local Scholar Archive through the read-only bridge (pairing and starting it automatically) and report its archive identity, counts, and the read tools it offers.",
            json!({}),
            &[],
        ),
        scholar_tool(
            "scholar_archive_read",
            "Read Scholar Archive",
            &format!(
                "Run one of Scholar Archive's read tools through the bridge and return its result: scholars, Works (titles, authors, years, venues, abstracts, DOIs, sources, versions, topics), journals, workspaces, linked PDF/Markdown files, source labels and history, duplicates, jobs, and semantic search. Tools: {}. Start with list_scholars, list_works, get_work, list_work_files, or semantic_search; list_work_artifacts lists a Work's figures for scholar_archive_artifact; pass the tool's own arguments in `arguments` (scholar_archive_status lists each schema). Nothing can change the archive.",
                crate::scholar::READ_TOOLS.join(", ")
            ),
            json!({
                "tool": { "type": "string", "enum": crate::scholar::READ_TOOLS, "description": "The Scholar Archive read tool." },
                "arguments": { "type": "object", "description": "That tool's arguments, as its schema names them." },
            }),
            &["tool"],
        ),
        scholar_tool(
            "scholar_archive_sql",
            "Query Scholar Archive",
            "Run one read-only SQL statement (a SELECT or a schema pragma such as `PRAGMA table_info(works)`) on Scholar Archive's SQLite database, or on its vector database. Writes, ATTACH, and other pragmas are refused. Tables include scholars, works, work_sources, work_files, workspaces, workspace_works, source_labels, and scholar_source_updates.",
            json!({
                "sql": { "type": "string", "description": "One SELECT statement; `?` placeholders take `params`." },
                "params": { "type": "array", "description": "Values for the `?` placeholders." },
                "limit": { "type": "integer", "minimum": 1, "maximum": 10000, "description": "Most rows to return; default 500." },
                "database": { "type": "string", "enum": ["archive", "vectors"], "description": "Which database; default archive." },
            }),
            &["sql"],
        ),
        scholar_tool(
            "scholar_archive_file",
            "Read Scholar Archive document",
            "Fetch the bytes of a PDF or Markdown document that Scholar Archive manages (a `file_id` from list_work_files or get_work), verify its SHA-256, and keep a copy in ZET's cache. Markdown and text come back as `text`; for a PDF, open the returned `path`.",
            json!({
                "file_id": { "type": "integer", "minimum": 1, "description": "The document's file_id." },
                "max_text_bytes": { "type": "integer", "minimum": 0, "description": "Largest text document to return inline; default 1000000." },
            }),
            &["file_id"],
        ),
        {
            // Reads the Scholar Archive, but keeps the image in this archive's artifact store.
            let mut value = tool(
                "scholar_archive_artifact",
                "Take a figure from Scholar Archive",
                "Copy one artifact (a figure or table image of a Work's Markdown, as `list_work_artifacts` lists it: its Markdown's `file_id` and its `path`) into this archive's artifact store, verified by SHA-256, and return the `markdown` that shows it in a Card: `![caption](artifact:<sha256>)`. Paste that into a Card body; a Card may show only artifacts kept this way. The returned `path` is the local image, which you can open to see the figure. `caption` overrides the caption the archive read from the Markdown.",
                json!({
                    "file_id": { "type": "integer", "minimum": 1, "description": "The Markdown document's file_id (the artifact's `file_id`)." },
                    "path": { "type": "string", "description": "The artifact's path below the archive's files folder, exactly as listed." },
                    "caption": { "type": "string", "description": "The figure's caption in the Card; default the archive's caption." },
                }),
                &["file_id", "path"],
                false,
                false,
            );
            value["annotations"]["openWorldHint"] = json!(true);
            value
        },
        scholar_tool(
            "scholar_archive_export",
            "Export Scholar Archive",
            "Export the whole archive, or one scholar's Works, as JSON, CSV, or BibTeX through the bridge. The export is saved in ZET's cache; small ones also come back as `text`.",
            json!({
                "scholar_id": { "type": "integer", "minimum": 1, "description": "Only this scholar; omit for the whole archive." },
                "format": { "type": "string", "enum": ["json", "csv", "bibtex"], "description": "Default json." },
                "view": { "type": "string", "enum": ["profile", "combined"], "description": "combined unions a verified person's profiles; default profile." },
            }),
            &[],
        ),
    ]
}

/// Scholar Archive bridge tools only read, and reach a separate local archive.
fn scholar_tool(
    name: &str,
    title: &str,
    description: &str,
    properties: Value,
    required: &[&str],
) -> Value {
    let mut value = tool(name, title, description, properties, required, true, false);
    value["annotations"]["openWorldHint"] = json!(true);
    value
}

fn call_scholar_tool(name: &str, args: &Value) -> Result<Value> {
    let mut bridge = crate::scholar::Bridge::connect()?;
    match name {
        "scholar_archive_status" => {
            let hello = bridge.hello()?;
            let status = bridge.call("archive_status", &json!({}))?;
            Ok(json!({
                "connected": true,
                "url": bridge.url(),
                "read_only": hello["read_only"],
                "archive_uuid": hello["archive_uuid"],
                "database": hello["database"],
                "tools": hello["tools"],
                "status": status,
            }))
        }
        "scholar_archive_read" => {
            let tool = string_arg(args, "tool")?;
            let arguments = args.get("arguments").cloned().unwrap_or_else(|| json!({}));
            if !arguments.is_object() {
                bail!("`arguments` must be an object");
            }
            Ok(json!({ "tool": tool, "result": bridge.call(&tool, &arguments)? }))
        }
        "scholar_archive_sql" => {
            let params = args.get("params").cloned().unwrap_or_else(|| json!([]));
            let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(500);
            let database = match optional_string(args, "database")?.as_str() {
                "" => "archive".to_string(),
                other => other.to_string(),
            };
            bridge.sql(&string_arg(args, "sql")?, &params, limit, &database)
        }
        "scholar_archive_file" => {
            let file_id = args
                .get("file_id")
                .and_then(Value::as_u64)
                .context("missing integer argument `file_id`")?;
            let limit = args
                .get("max_text_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(1_000_000) as usize;
            let copy = bridge.file(file_id, None)?;
            let mut value = copy.summary();
            if let Some(text) = copy.text(limit) {
                value["text"] = json!(text);
            }
            Ok(value)
        }
        "scholar_archive_export" => {
            let scholar = args.get("scholar_id").and_then(Value::as_u64);
            let format = match optional_string(args, "format")?.as_str() {
                "" => "json".to_string(),
                other => other.to_string(),
            };
            let view = match optional_string(args, "view")?.as_str() {
                "" => "profile".to_string(),
                other => other.to_string(),
            };
            let answer = bridge.export(scholar, &format, &view)?;
            let path = crate::scholar::export_path(scholar, &format)?;
            std::fs::write(&path, &answer.body)
                .with_context(|| format!("failed to write {}", path.display()))?;
            let mut value = json!({ "path": path.display().to_string(), "bytes": answer.body.len(), "format": format });
            if answer.body.len() <= 1_000_000 {
                value["text"] = json!(String::from_utf8_lossy(&answer.body));
            }
            Ok(value)
        }
        other => bail!("unknown tool `{other}`"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_negotiates_a_known_protocol_version() {
        let mut server = Server::default();
        let reply = server
            .handle_line(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#)
            .unwrap();
        let reply: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(reply["result"]["serverInfo"]["name"], "zt");
        let reply = server
            .handle_line(r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"1999-01-01"}}"#)
            .unwrap();
        let reply: Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_methods_are_errors() {
        let mut server = Server::default();
        assert!(
            server
                .handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .is_none()
        );
        assert!(server.initialized);
        let reply: Value = serde_json::from_str(
            &server
                .handle_line(r#"{"jsonrpc":"2.0","id":"x","method":"bogus"}"#)
                .unwrap(),
        )
        .unwrap();
        assert_eq!(reply["error"]["code"], -32601);
        let reply: Value = serde_json::from_str(&server.handle_line("{not json").unwrap()).unwrap();
        assert_eq!(reply["error"]["code"], -32700);
    }

    #[test]
    fn every_tool_has_a_unique_name_and_object_schema() {
        let tools = tool_definitions();
        let mut names = tools
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), tools.len());
        for tool in &tools {
            assert_eq!(tool["inputSchema"]["type"], "object");
            for required in tool["inputSchema"]["required"].as_array().unwrap() {
                assert!(
                    tool["inputSchema"]["properties"]
                        .get(required.as_str().unwrap())
                        .is_some()
                );
            }
        }
    }

    #[test]
    fn new_trees_cannot_target_the_human_workspace() {
        assert_eq!(target_workspace(&json!({})).unwrap(), Workspace::Agent);
        assert!(target_workspace(&json!({ "workspace": "human" })).is_err());
    }
}
