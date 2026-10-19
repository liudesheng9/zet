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

use crate::model::{Emphasis, Facet, Level};
use crate::ops::{self, Outcome, Snapshot, Successor};
use crate::{Access, CardType, Workspace};

/// Every change made through MCP is limited to the agent Workspace.
const ACCESS: Access = Access::Agent;
const SERVER_NAME: &str = "zt";
const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// How long a write waits for a busy edit lock before refusing (R269/R315).
///
/// A person's editor holds the lock while it is open, so an ordinary agent write
/// gives up quickly instead of hanging. A digest batch client needs long patience
/// and sets `ZT_EDIT_PATIENCE=180` for its child `zt mcp`. The value is validated as
/// whole seconds from 1 through 300; anything else keeps the default 20 and is
/// reported once in the archive's log, so a silent fallback cannot hide a
/// misconfigured batch client.
const DEFAULT_EDIT_PATIENCE_SECONDS: u64 = 20;
const EDIT_PATIENCE_SECONDS: std::ops::RangeInclusive<u64> = 1..=300;

pub(crate) fn edit_patience() -> std::time::Duration {
    patience_seconds(std::env::var("ZT_EDIT_PATIENCE").ok().as_deref()).0
}

/// The one note a child with an invalid declared value writes at its start.
pub(crate) fn patience_note() -> Option<String> {
    patience_seconds(std::env::var("ZT_EDIT_PATIENCE").ok().as_deref()).1
}

/// The patience a declared value asks for, and why a declared value was refused: whole seconds
/// from 1 through 300, else the default 20.
fn patience_seconds(declared: Option<&str>) -> (std::time::Duration, Option<String>) {
    let default = std::time::Duration::from_secs(DEFAULT_EDIT_PATIENCE_SECONDS);
    let Some(declared) = declared else {
        return (default, None);
    };
    match declared.trim().parse::<u64>() {
        Ok(seconds) if EDIT_PATIENCE_SECONDS.contains(&seconds) => {
            (std::time::Duration::from_secs(seconds), None)
        }
        _ => (
            default,
            Some(format!(
                "edit patience: ZT_EDIT_PATIENCE={declared} is not a whole number of seconds from {} through {}; using the default {}",
                EDIT_PATIENCE_SECONDS.start(),
                EDIT_PATIENCE_SECONDS.end(),
                DEFAULT_EDIT_PATIENCE_SECONDS
            )),
        ),
    }
}

const INSTRUCTIONS: &str = "ZT is a slip-box of addressable Markdown Cards split into two Workspaces: \
`human` (read-only for you) and `agent` (yours to read and change). Each Workspace has two Zones: \
Topic trees (rooted at a Topic such as `3/0`) and Literature trees (rooted at a Literature Card \
named by its BibTeX Citation key, such as `Smith2024`). Card addresses are unique across both \
Workspaces: Locations look like `3/1`, `3/2|a`, or `Smith2024/1`. Link Cards by writing \
`[[address]]` in Markdown bodies; Links may cross Workspaces, but Cards and trees never move \
between them. Start with list_workspaces and list_trees, read with get_card or search_cards, and \
use get_cards for ordered full/text bulk reads (at most 200 addresses). search_cards `mode: entry` \
ranks ways in first, exactly as the GUI's search does: an exact address, a matching register term and \
its entry Cards, a matching hub member line and the Card it opens, a matching model head, a matching \
title, then a matching body; an omitted mode keeps the plain lexical order. A search starts no model. \
Check missing, complete and \
archive_revision. get_tree with bodies=true includes full text, and export_titles reads selected \
titles in one pass. These bulk replies have an 8 MiB encoded tool-result limit; a read_limit error \
requires a smaller chunk. If related reads have different revisions, retry once or label a changing view. \
create new trees with create_topic or create_literature, then grow them with create_successor. \
Most Cards are Idea Cards (type `idea`); a Regular Card may instead be an Event Card (`event`: one \
real-world event with its date, place, actors, and source), a Finding Card (`finding`: one \
empirical result from a source: what was measured, the data, the method, the result with its size \
or direction, its limits, and the source line), or a Model Card (`model`: one whole \
model), set with `type` on create_successor or edit_card and listed with search_cards by `type`. \
A model is presented by its Model Card (its entry point: the question and what the model shows) and \
by Model parts: Model Cards with a `facet`, a short label you choose for what the part presents in \
this model (`设定`, `均衡`, `数值例子`), and optionally an `emphasis` (`core` for what a reader must \
see, `aside` for detail). A Model Card may state one mathematical unit, named by its `form`: `definition`, `axiom`, `theorem`, `proposition`, `lemma`, or `corollary` (a unit: its Model Card holds the statement with labelled hypotheses (H1) or clauses (C1), and its parts take the fixed facets 前提, 证明思路, 证明：…, 推导：…, 结论, 直觉, 图示, 例子, 非例, 反例, 假设作用：Hi, 等价刻画, 另证, 习题, 历史), or `theory` (a Theory Card: the way into one theory, which takes in whole units by Link). A proof rests on the statement of an earlier unit (a cited premise: `依赖`, `基于`, `按定义`), never on a step inside another unit's proof (a reach-in); a narrower concept links a broader one with `特例` (specializes). get_model reports a unit's or theory's `math`: tiers, faults, load, used_by, and study order. A Model Card also carries its model's elaboration `level`: `concept` (level 1: a concept model, stated in words in the Model Card and a few parts, with no Derivation parts) or `formal` (level 2: a formal model, built self-sufficient with a derivation and Derivation parts for the steps the source skips); set it with create_successor or edit_card. Where a part is filed does not decide its model: a part belongs to the \
Model Card it is filed under and to every Model Card that links it, so file it where it first \
connects and let one part serve several models. Every Link says what it is by the words before it \
in its sentence: rests on (`依赖`, `基于`), illustrates (`例示`, `案例`, `印证`), contrasts \
(`对照`, `区别`), relaxes (`放宽`, `推广`), corresponds (`对应`, `类比`, `参照`), objects (`反驳`, \
`批评`); with none, or after `引出` (the linked Card builds on this one), it is a plain reference. get_model shows a model's parts, their kinded relations, its loose parts, and its \
Links with Cards outside it, and its `build`: the members in build order (the setting, then the \
main line of Direct successors, each example beside its step, shared parts before the step that \
needs them and after the shared parts they rest on), its `chains` (runs of Derivation parts: one long step may take \
any number), and the faults that keep it from being self-sufficient (`closure`: a rests-on Link \
to a Card outside the model; `forward`: a step rests on a later step; `unanchored`: a step with no \
rests-on path to the setting; and `beyond_level`: a Derivation part in a concept model). A model rests on parts, never on a whole other Model Card. To put \
a missing step between two Cards, use insert_card with `before`; \
A Hub Card (`hub`) is a way into one subject: its body lists, under `##` group headings, one list \
item per member, `- [[address]]：what this Card adds`, and states no claim of its own. Its `level` \
says how wide the subject is: `line` (one line of thought, filed as a Side successor of the Card \
where the line starts), `topic` (one Topic tree, a Side successor of `<n>/1`, linked from the Topic \
body), `field` (several Topic trees, filed under the register), or `register` (the Workspace: one \
key term per item under `## 词条` with 1 to 3 Links to the Cards that carry its substance). A hub \
lists only hubs of a lower level and reaches into other \
trees in a `邻近` group. get_hub reports a hub's lines, upper and lower hubs, and faults, and on a \
register its flattened entries too; get_register reads those entries for one Workspace (an absent \
register answers `register_missing`, not an error); list_hubs \
shows the hub graph and the hubs the policy requires but the Workspace lacks; move_card also takes `before` \
(another Card's place) and `alone` (only the Card; its chain closes the gap). Write Card text \
plainly: one statement per sentence, no semicolons, active voice, one term per meaning. Card \
bodies may hold TeX math in `$...$` or `$$...$$` and HTML charts in ```chart fences, which the GUI \
renders. delete_card, move_card, insert_card, and Citation key renames return a plan first; \
repeat the call with the `confirm` text the plan names to apply it. The scholar_archive_* tools read the local Scholar Archive \
(scholars, Works, abstracts, journals, workspaces, PDF/Markdown documents) through its read-only \
bridge; file a paper you use as a Literature Card and cite its DOI. To show a paper's figure in a Card, list the Work's figures with scholar_archive_read `list_work_artifacts`, take one with scholar_archive_artifact, and paste the `![caption](artifact:<sha256>)` Markdown it returns. To point a reader at the source document itself (a PDF or Markdown the archive manages), write a Document link `[label](scholar:<file_id>)` (add `#page=12` for a PDF page); scholar_archive_file returns one as `link`.";

pub(crate) fn run() -> Result<()> {
    // R315: a declared value outside the documented range is reported once, here, instead of
    // quietly turning a batch client's long patience back into the ordinary short bound.
    if let Some(note) = patience_note()
        && let Ok(root) = crate::require_archive_root()
    {
        crate::log_event(&root, &note).ok();
    }
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
                    Ok(value) => {
                        let result = tool_result(value, false);
                        if matches!(name, "get_cards" | "get_tree" | "export_titles") || (name == "apply_batch" && arguments["preview"] == true) {
                            let size = serde_json::to_vec(&result)
                                .map_err(|e| {
                                    RpcError::invalid_params(&format!(
                                        "cannot encode read reply: {e}"
                                    ))
                                })?
                                .len();
                            if size > 8 * 1024 * 1024 {
                                tool_result(
                                    crate::read::Limit {
                                        kind: "reply_bytes",
                                        limit: 8 * 1024 * 1024,
                                        actual: size,
                                    }
                                    .value(),
                                    true,
                                )
                            } else {
                                result
                            }
                        } else {
                            result
                        }
                    }
                    Err(error) => tool_result(
                        error
                            .downcast_ref::<crate::read::Limit>()
                            .map(crate::read::Limit::value)
                            .or_else(||error.downcast_ref::<crate::batch::Conflict>().map(crate::batch::Conflict::value))
                            .unwrap_or_else(|| json!({ "error": format!("{error:#}") })),
                        true,
                    ),
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
        "apply_batch" => crate::batch::apply(&root, args),
        "list_workspaces" => crate::read::workspaces(&root),
        "list_trees" => crate::read::trees(&root, optional_workspace(args)?, optional_zone(args)?),
        "get_tree" => {
            let tree = string_arg(args, "tree")?;
            let bodies = args
                .get("bodies")
                .map(|v| v.as_bool().context("argument `bodies` must be a boolean"))
                .transpose()?
                .unwrap_or(false);
            crate::read::tree(&root, &tree, bodies)
        }
        "export_titles" => {
            crate::read::titles(&root, optional_workspace(args)?, optional_zone(args)?)
        }
        "get_card" => {
            let at = string_arg(args, "at")?;
            crate::read::card(&root, &at)
        }
        "get_cards" => {
            let addresses = args
                .get("addresses")
                .and_then(Value::as_array)
                .context("missing array argument `addresses`")?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .context("each address must be a string")
                })
                .collect::<Result<Vec<_>>>()?;
            let text = match args
                .get("view")
                .map(|v| v.as_str().context("argument `view` must be full or text"))
                .transpose()?
                .unwrap_or("full")
            {
                "full" => false,
                "text" => true,
                other => bail!("unknown Card view `{other}`; expected full or text"),
            };
            crate::read::cards(&root, &addresses, text)
        }
        "get_model" => {
            let at = string_arg(args, "at")?;
            crate::read::model(&root, &at)
        }
        "get_hub" => {
            let at = string_arg(args, "at")?;
            let limits = crate::hub::Limits::from_args(args)?;
            let entries = crate::entry::EntryArgs::from_args(args)?;
            crate::read::hubs(&root, Some(&at), None, &limits, &entries)
        }
        "list_hubs" => {
            let limits = crate::hub::Limits::from_args(args)?;
            crate::read::hubs(
                &root,
                None,
                optional_workspace(args)?,
                &limits,
                &crate::entry::EntryArgs::default(),
            )
        }
        "get_register" => {
            let workspace = crate::Workspace::parse(&string_arg(args, "workspace")?)?;
            let entries = crate::entry::EntryArgs::from_args(args)?;
            crate::read::register(&root, workspace, &entries)
        }
        "search_cards" => {
            let mode = crate::entry::Mode::from_args(args)?;
            let snapshot = Snapshot::read(&root)?;
            let card_type = optional_card_type(args)?;
            let form = optional_form(args)?.flatten();
            // With a type or a form, the query may be left out to list every Card of it.
            let query = match (card_type, form) {
                (None, None) => string_arg(args, "query")?,
                _ => optional_string(args, "query")?,
            };
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(30)
                .clamp(1, crate::entry::SEARCH_LIMIT_MAX as u64) as usize;
            // An empty query stays the existing type/form listing, in every mode (D198, US024).
            if mode == crate::entry::Mode::Entry && !query.trim().is_empty() {
                return Ok(crate::entry::search(
                    &snapshot,
                    &crate::entry::SearchArgs {
                        query,
                        workspace: optional_workspace(args)?,
                        card_type,
                        form,
                        limit,
                    },
                ));
            }
            Ok(json!({
                "results": snapshot.search_in(&query, optional_workspace(args)?, card_type, form, limit),
            }))
        }
        "list_links" => crate::read::links(
            &root,
            optional_workspace(args)?,
            args.get("cross_workspace_only")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
        "list_broken_links" => crate::read::broken(&root, optional_workspace(args)?),
        "create_topic" | "create_literature" | "create_successor" | "edit_card" => {
            // R316: the wait line names the tool that waited.
            let operation = match name {
                "create_topic" => "create_topic",
                "create_literature" => "create_literature",
                "create_successor" => "create_successor",
                _ => "edit_card",
            };
            let _lock=crate::acquire_edit_lock_within(&root,edit_patience(),operation)?;
            let mut conn=crate::open_db(&root)?;
            let tx=conn.transaction()?;
            let (result,changed)=write_card_tx(&root,&tx,name,args)?;
            if changed {crate::link_index::source_changed(&tx,result["address"].as_str().context("write result missing address")?)?;}
            tx.commit()?;
            Ok(result)
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
            let into = optional_string(args, "into")?;
            let outcome = ops::delete(
                &root,
                ACCESS,
                &string_arg(args, "at")?,
                (!into.is_empty()).then_some(into.as_str()),
                args.get("confirm").and_then(Value::as_str),
            )?;
            Ok(plan_or_done(outcome))
        }
        "move_card" => {
            let target = match (
                optional_string(args, "to")?,
                optional_string(args, "before")?,
            ) {
                (to, before) if !to.is_empty() && !before.is_empty() => {
                    bail!("give either `to` or `before`, not both")
                }
                (to, _) if !to.is_empty() => ops::Target::To(to),
                (_, before) if !before.is_empty() => ops::Target::Before(before),
                _ => bail!("missing string argument `to` or `before`"),
            };
            let alone = match args.get("alone") {
                None | Some(Value::Null) => false,
                Some(Value::Bool(alone)) => *alone,
                Some(_) => bail!("argument `alone` must be true or false"),
            };
            let outcome = ops::move_card(
                &root,
                ACCESS,
                &string_arg(args, "at")?,
                &target,
                alone,
                args.get("confirm").and_then(Value::as_str),
            )?;
            Ok(plan_or_done(outcome))
        }
        "insert_card" => {
            let before = string_arg(args, "before")?;
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
            let outcome = ops::insert_card(
                &root,
                ACCESS,
                &before,
                card_type,
                part,
                &text,
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

pub(crate) fn write_card_tx(
    root: &std::path::Path,
    conn: &rusqlite::Connection,
    name: &str,
    args: &Value,
) -> Result<(Value, bool)> {
    if name == "edit_card" {
        if args.get("base_fingerprint").is_some()
            || args.get("base_receipt").is_some()
            || args.get("owned_paragraph").is_some()
        {
            bail!(
                "base_fingerprint, base_receipt and owned_paragraph are only available within apply_batch; single edit_card keeps its raw base guard"
            );
        }
        let at = string_arg(args, "at")?;
        let card = crate::load_card(conn, &at)?;
        ACCESS.ensure_writable(card.workspace)?;
        let (level, hub_level) = ops::parse_level_arg(args.get("level"))?;
        let change = ops::TypeChange {
            card_type: optional_card_type(args)?,
            facet: optional_facet(args)?,
            emphasis: optional_emphasis(args)?,
            level,
            hub_level,
            form: optional_form(args)?,
        };
        let parsed = crate::parse_card_text(&card.text)?;
        let text_given = ["text", "title", "body"]
            .iter()
            .any(|name| args.get(name).is_some());
        let text = match args.get("text").and_then(Value::as_str) {
            _ if !change.is_empty() && !text_given => None,
            Some(text) => Some(text.to_string()),
            None => {
                if card.is_lit && args.get("title").is_some() {
                    bail!(
                        "a Literature Card takes its title from BibTeX; change it with edit_literature_metadata"
                    );
                }
                let title = if args.get("title").is_some() {
                    title_arg(args)?
                } else {
                    parsed.title
                };
                let body = if args.get("body").is_some() {
                    optional_string(args, "body")?
                } else {
                    parsed.body
                };
                Some(crate::compose_card_text(&title, &body, ""))
            }
        };
        if let Some(text) = &text {
            crate::artifact::require_present(root, text)?;
        }
        let (address, changed) = ops::edit_tx(
            conn,
            ACCESS,
            &at,
            text.as_deref(),
            change,
            args.get("base").and_then(Value::as_str),
        )?;
        return Ok((json!({"address":address}), changed));
    }
    if name == "create_literature" {
        let body = optional_string(args, "body")?;
        crate::artifact::require_present(root, &body)?;
        let workspace = target_workspace(args)?;
        let address = ops::create_literature_tx(
            conn,
            ACCESS,
            workspace,
            &string_arg(args, "bibtex")?,
            &body,
        )?;
        return Ok((json!({"address":address,"workspace":workspace.id()}), true));
    }
    let text = crate::compose_card_text(&title_arg(args)?, &optional_string(args, "body")?, "");
    crate::artifact::require_present(root, &text)?;
    let result = match name {
        "create_topic" => {
            let workspace = target_workspace(args)?;
            let address = ops::create_topic_tx(conn, ACCESS, workspace, &text)?;
            json!({"address":address,"workspace":workspace.id()})
        }
        "create_successor" => {
            let kind = successor_kind(args)?;
            let (part, card_type) = successor_part_type(args)?;
            let (level, hub_level) = ops::parse_level_arg(args.get("level"))?;
            let (level, hub_level) = (level.flatten(), hub_level.flatten());
            let form = optional_form(args)?.flatten();
            if level.is_some() && (card_type != CardType::Model || part.is_some()) {
                bail!(crate::model::LEVEL_NEEDS_MODEL_CARD);
            }
            if form.is_some() && (card_type != CardType::Model || part.is_some()) {
                bail!(crate::math::FORM_NEEDS_MODEL_CARD);
            }
            if hub_level.is_some() && card_type != CardType::Hub {
                bail!(crate::hub::LEVEL_NEEDS_HUB_CARD);
            }
            let address = ops::create_successor_tx(
                conn,
                ACCESS,
                kind,
                &string_arg(args, "at")?,
                card_type,
                part.as_ref(),
                &text,
            )?;
            let card = crate::load_card(conn, &address)?;
            if level.is_some() {
                crate::model::set_level(conn, &card, level)?;
            }
            if hub_level.is_some() {
                crate::hub::set_level(conn, &card, hub_level)?;
            }
            if form.is_some() {
                crate::math::set_form(conn, &card, form)?;
            }
            json!({"address":address,"type":card_type.id(),"facet":part.as_ref().map(|p|p.facet.id()),"emphasis":part.and_then(|p|p.emphasis).map(Emphasis::id),"level":level.map(Level::id).or(hub_level.map(crate::hub::HubLevel::id)),"form":form.map(crate::math::MathForm::id)})
        }
        other => bail!("unsupported batch tool `{other}`"),
    };
    Ok((result, true))
}

fn successor_kind(args: &Value) -> Result<Successor> {
    match optional_string(args, "kind")?.as_str() {
        "" | "direct" => Ok(Successor::Direct),
        "side" => Ok(Successor::Side),
        other => bail!("unknown successor kind `{other}`; expected direct or side"),
    }
}

fn successor_part_type(args: &Value) -> Result<(Option<crate::model::Part>, CardType)> {
    let part = crate::model::Part::from_args(
        optional_facet(args)?.flatten(),
        optional_emphasis(args)?.flatten(),
    )?;
    let card_type = match (optional_card_type(args)?, &part) {
        (Some(t), Some(_)) if t != CardType::Model => {
            bail!("only Model Cards can have a Model facet")
        }
        (_, Some(_)) => CardType::Model,
        (t, None) => t.unwrap_or_default(),
    };
    Ok((part, card_type))
}

/// Canonical explicit creation defaults use the ordinary MCP argument parsers.
/// Edit omissions remain omissions: an omitted field leaves stored content alone.
pub(crate) fn canonical_write_args(name: &str, args: &Value) -> Result<Value> {
    let mut canonical = args.clone();
    canonical
        .as_object()
        .context("item arguments must be an object")?;
    match name {
        "create_topic" | "create_literature" => {
            canonical["workspace"] = json!(target_workspace(args)?.id());
            canonical["body"] = json!(optional_string(args, "body")?);
            if name == "create_topic" {
                canonical["title"] = json!(title_arg(args)?);
            }
        }
        "create_successor" => {
            let (part, card_type) = successor_part_type(args)?;
            canonical["kind"] = json!(if successor_kind(args)? == Successor::Side {
                "side"
            } else {
                "direct"
            });
            canonical["type"] = json!(card_type.id());
            canonical["facet"] = json!(part.as_ref().map(|p| p.facet.id()));
            canonical["emphasis"] = json!(part.and_then(|p| p.emphasis).map(Emphasis::id));
            let (level, hub_level) = ops::parse_level_arg(args.get("level"))?;
            canonical["level"] = json!(
                level
                    .flatten()
                    .map(Level::id)
                    .or(hub_level.flatten().map(crate::hub::HubLevel::id))
            );
            canonical["form"] = json!(
                optional_form(args)?
                    .flatten()
                    .map(crate::math::MathForm::id)
            );
            canonical["title"] = json!(title_arg(args)?);
            canonical["body"] = json!(optional_string(args, "body")?);
        }
        "edit_card" => {
            if let Some(value) = args.get("base_fingerprint").and_then(Value::as_str) {
                canonical["base_fingerprint"] = json!(value.to_ascii_lowercase());
            }
            if let Some(spec) = canonical
                .get_mut("owned_paragraph")
                .and_then(Value::as_object_mut)
            {
                spec.entry("replaces_receipt").or_insert(Value::Null);
                if spec.get("adopt_legacy").is_none_or(Value::is_null) {
                    spec.insert("adopt_legacy".into(), json!(false));
                }
            }
        }
        other => bail!("unsupported batch tool `{other}`"),
    }
    Ok(canonical)
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

/// `None` when absent; `Some(None)` for null, an empty string, or `none`,
/// which clears a Math form.
fn optional_form(args: &Value) -> Result<Option<Option<crate::math::MathForm>>> {
    match args.get("form") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(value)) => crate::math::parse_optional_form(value).map(Some),
        Some(_) => bail!("argument `form` must be a string"),
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
            "idempotentHint": read_only || name == "apply_batch",
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
    let batch_item = json!({"type":"object","properties":{"op_key":{"type":"string","pattern":"^[^/]+/[^/]+/[1-9][0-9]*/[^/]+/[^/]+$"},"client_ref":{"type":"string","pattern":"^[A-Za-z0-9_-]{1,64}$"},"tool":{"type":"string","enum":["create_topic","create_literature","create_successor","edit_card"]},"arguments":{"type":"object"}},"required":["op_key","client_ref","tool","arguments"],"additionalProperties":false});
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
    let level = |description: &str| {
        json!({
            "type": "string",
            "enum": ["concept", "formal", "line", "topic", "field", "register", ""],
            "description": description,
        })
    };
    let form = |description: &str| {
        json!({
            "type": "string",
            "enum": ["theory", "definition", "axiom", "theorem", "proposition", "lemma", "corollary", ""],
            "description": description,
        })
    };
    let card_type = |description: &str| {
        json!({
            "type": "string",
            "enum": ["idea", "event", "finding", "model", "hub"],
            "description": description,
        })
    };
    let limit =
        |description: &str| json!({ "type": "integer", "minimum": 1, "description": description });
    let entry_offset = json!({
        "type": "integer",
        "minimum": 0,
        "description": "Skip this many flattened register entries (default 0).",
    });
    let entry_limit = json!({
        "type": "integer",
        "minimum": 1,
        "maximum": 1000,
        "description": "Most flattened register entries to return (default 200, maximum 1000).",
    });
    let hub_limits = json!({
        "max_groups": limit("Most groups a hub may hold (default 7)."),
        "max_lines": limit("Most member lines a hub may hold (default 25)."),
        "min_reason": limit("Fewest characters of a line's reason (default 8)."),
        "topic_min": limit("Cards in a Topic tree that require a topic hub (default 7)."),
        "line_min": limit("Cards in a branch that require a line hub (default 12)."),
        "neighbour_links": limit("Links between two trees that make them neighbours (default 3)."),
    });
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
            "List tree roots (Topics and Literature Cards) with their Workspace, Zone, title, size, and how many Event Cards, Finding Cards, and models each holds.",
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
            "Every Card of one tree, root first, with its type (idea, event, finding, model, hub), edge (root, direct, side), parent, excerpt, and Links.",
            json!({ "tree": { "type": "string", "description": "A tree id (Topic number or Citation key) or any address in the tree." },"bodies":{"type":"boolean","description":"Include complete body, stored text and BibTeX in each summary."} }),
            &["tree"],
            true,
            false,
        ),
        tool(
            "export_titles",
            "Export titles",
            "Read titles in one selected-text pass, filtered by Workspace and Zone. Includes count and archive revision; no bodies.",
            json!({"workspace":workspace_filter,"zone":{"type":"string","enum":["topic","literature","all"]}}),
            &[],
            true,
            false,
        ),
        tool(
            "get_card",
            "Get card",
            "Read one Card: its title, Markdown body, stored text, type (idea, event, finding, model, or hub), Model facet, BibTeX, Workspace, whether you may change it, neighbors, and Links in and out. A Model Card or Model part also reports `model`: its model's structure.",
            json!({ "at": at }),
            &["at"],
            true,
            false,
        ),
        tool(
            "get_cards",
            "Get cards",
            "Read ordered Card addresses in one archive revision. Full matches get_card; text omits model and hub projections. Reports missing addresses explicitly. archive_identity contains the persisted random 128-bit id and resolved archive path, including for an empty address list; bind prepared runs to it rather than a path alone.",
            json!({"addresses":{"type":"array","items":{"type":"string"},"maxItems":200},"view":{"type":"string","enum":["full","text"],"default":"full"}}),
            &["addresses"],
            true,
            false,
        ),
        tool(
            "get_model",
            "Get model",
            "Read a whole model from its Model Card or any of its Model parts: `models` (every model the Card belongs to; the first is shown); the Model Card; its parts, in the order the Model Card names them and then in filing order, with their facets, emphasis, and bodies, whether each is filed under the Model Card or joined by its Link (`filed`), and the other models it serves (`also`); the `relations` between parts (each Link from one part to another, with its `kind` (rests-on, illustrates, contrasts, relaxes, corresponds, objects, or refers) and the sentence that makes it); the `loose` parts in no relation; the `neighbours`, Links between the model and Cards outside it either way, with kind and sentence; how many outside Cards discuss it; and `issues` (a part that belongs to no model). `level` is the Model Card's elaboration level (concept, formal, or null), and `build` gives the build order, the Derivation chains (`chains`: each run of Derivation parts with the members `after` and `before` it), and the faults (`closure`, `reach_in` (a rests-on Link into a part of another mathematical unit), `forward`, `unanchored`, and for a concept model `beyond_level`); `cited` lists the cited premises (rests-on Links to the Model Card of a mathematical unit, or to the book's notation table), which are no fault; `tolerated` lists premises of shared parts that stay in their home model. `form` is the Math form; `math` is the unit report (labels, tiers, core, crux, budget, faults, theories, load, building_block, used_by, and `study`: the stations of Learn mode) or, for a Theory Card, the theory report (units in dependency order with weight and load, main_results, objects, cycle, forward, tools, users, faults, and study passes).",
            json!({ "at": at }),
            &["at"],
            true,
            false,
        ),
        tool(
            "get_hub",
            "Get hub",
            "Read one Hub Card as a hub: its `level` (line, topic, field, register), `anchor` (the Card it is filed under), `groups`, and `lines` (each member with its group, title, type, tree, Relation kind, and reason); its `lower` and `upper` hubs; how many Cards its range holds; its `faults` (level, anchor-title, oversize, flat, bare, broken, shape, empty, duplicate-target, upward, cycle, unanchored, duplicate, unlisted, isolated); and the advice: `mirror` (it only repeats its own tree in address order), `candidates` (Cards of its range that link two or more members but are in no line), and `unreached` (Cards of its range that no hub reaches, directly or through a Card above them). On a register its `词条` rows are checked by their own grammar (`shape`, `empty`, `oversize` above three Links, `duplicate-target`, and one `broken` per missing target) and it adds the flattened `entries` projection of `get_register` (every target of every term, bounded target bodies, `parts`, `entry_total`, `entry_next_offset`, `complete`, `archive_revision`, `archive_identity`); `entry_offset` and `entry_limit` page that projection only and never truncate the hub report. The limits may be set per call.",
            {
                let mut properties = hub_limits.clone();
                properties["at"] = at.clone();
                properties["entry_offset"] = entry_offset.clone();
                properties["entry_limit"] = entry_limit.clone();
                properties
            },
            &["at"],
            true,
            false,
        ),
        tool(
            "list_hubs",
            "List hubs",
            "The hub graph of a Workspace: every Hub Card with its level, tree, line count, upper and lower hubs, fault names, `mirror`, and unreached count; `counts` per level; and `missing`: the hubs the policy requires but the Workspace lacks (a topic hub for a Topic tree of `topic_min` or more Cards, a line hub at the first Card of a branch of `line_min` or more Cards or with 2 or more Model Cards, and one register per Workspace that holds hubs). Pass the limits a run chose to check against them.",
            {
                let mut properties = hub_limits.clone();
                properties["workspace"] = workspace_filter.clone();
                properties
            },
            &[],
            true,
            false,
        ),
        tool(
            "get_register",
            "Get register",
            "Read the concept register of one Workspace: the Hub Card of level `register` and its accepted `entries`, one per term row of the `词条` section (or the older `关键词` section). Each entry names its `term`, its origin (`card`, `line`, and the declared `part` when it lives in one), every one of its one to three `targets` (address, title, type, Workspace, `exists`, and the bounded authored `body` with `body_chars`, `body_lines`, `body_truncated`), whether it is `complete`, and its `faults`. The view also carries the declared `parts`, the register's own `faults`, `complete`, `terms_by_card` (which terms name each target), the confirmed `restatements` (`## 同述入口` entries), the `bounds` of the body projection, and the `archive_revision`/`archive_identity` it was read at. `entry_offset` and `entry_limit` page the entries (default 200, maximum 1000); `entry_next_offset` is null on the last page. A Workspace with no register answers `register_missing: true` with no entries, not an error; two registers are reported, never chosen between.",
            {
                let mut properties = serde_json::Map::new();
                properties.insert(
                    "workspace".to_string(),
                    json!({
                        "type": "string",
                        "enum": ["human", "agent"],
                        "description": "The Workspace whose register to read.",
                    }),
                );
                properties.insert("entry_offset".to_string(), entry_offset.clone());
                properties.insert("entry_limit".to_string(), entry_limit.clone());
                Value::Object(properties)
            },
            &["workspace"],
            true,
            false,
        ),
        tool(
            "search_cards",
            "Search cards",
            "Case-insensitive full-text search over Card addresses, titles, and bodies. Each result names the Card's type. With `type` or `form`, only Cards of that type or Math form match, and `query` may be empty or left out to list every Card of it (the existing listing, in every mode). `mode: entry` ranks ways in first, exactly as the GUI's search does: an exact address, a matching register term and its entry Cards, a matching hub member line and the Card it opens, a matching model head, a matching Card title, then a matching authored body. Inside a tier, exact matches precede prefixes and partial hits, the current lexical score orders the rest, and natural address order breaks the final tie. Each row keeps the summary fields and adds `role` (`entry`, `member`, or `hit`), `rank_tier` (1 to 6), `matched_term` or `matched_line`, `reason`, the navigation `origin`, and the term's surviving `entry_targets`; `registers`, `register_missing`, `register_faults`, `bounds`, `scanned`, and `archive_fingerprint` state the evidence and the bounded work. A search starts no model. An omitted `mode` is `plain`, the existing lexical order and result shape.",
            json!({
                "query": { "type": "string", "description": "Text to find; required unless `type` or `form` is given." },
                "mode": { "type": "string", "enum": ["plain", "entry"], "description": "`plain` (default) keeps the existing lexical order; `entry` ranks register terms and hub ways in before ordinary title and body hits." },
                "type": card_type("Only Cards of this type: idea, event (Event Cards), finding (Finding Cards), model (Model Cards), or hub (Hub Cards)."),
                "form": form("Only Model Cards of this Math form: theory (Theory Cards) or a unit (definition, axiom, theorem, proposition, lemma, corollary)."),
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
        {
            let mut batch = tool(
                "apply_batch",
                "Apply identified Card writes",
                "Atomically apply 1–64 ordered Topic/Literature/successor creates or content/type edits, with at most 1 MiB of authored input including nested paragraph text. Stable op_key: run/unit/positive-plan-revision/step/stable-item; no attempt component. A later item can use @@client_ref from an earlier new or replayed result. Edits require exactly one authored base_fingerprint or same-Card base_receipt, excluding generated Reverse links. An edit may instead of body/metadata supply owned_paragraph {text, replaces_receipt?, adopt_legacy?}: append or replace exactly the named receipt's single paragraph; explicit legacy adoption requires one exact existing span. Matching keys replay original allocated results; changed content or reference bindings conflict. One failed unseen item rolls back every unseen write and receipt. preview:true validates the same transaction and returns complete draft Cards/paragraph facts, then rolls back all changes; a preview is never a commit receipt. Agent Access and ordinary Card/model/Link/artifact validators apply. Destructive operations retain their separate confirmation path.",
                json!({"preview":{"type":"boolean","description":"Validate and return the transaction draft, then roll back all Cards, receipts, allocation and index changes. No write is applied."},"items":{"type":"array","minItems":1,"maxItems":64,"items":batch_item},"preview_items":{"type":"array","minItems":1,"maxItems":1024,"items":batch_item,"description":"Read-only complete sequence, exclusive with items and requiring preview:true. At most 8 MiB authored input. Numeric client_ref equals the zero-based sequence index; references to earlier chunks bind exactly as the thin client's 64-item/1-MiB writes. All simulated changes roll back."}}),
                &[],
                false,
                false,
            );
            batch["inputSchema"]["oneOf"] = json!([{"required":["items"],"not":{"required":["preview_items"]}},{"required":["preview","preview_items"],"properties":{"preview":{"const":true}},"not":{"required":["items"]}}]);
            batch
        },
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
            "Add a Card to an agent tree after the Card at `at`: a direct successor continues the line (`3/1` -> `3/2`), a side successor branches (`3/1` -> `3/1|a`; not from a tree root). Set `type` to `event` for an Event Card (one real-world event), `finding` for a Finding Card (one empirical result from a source), or `model` for a Model Card (one whole model). Set `facet` to make a Model part: a Model Card presenting one part of a model. It belongs to the Model Card it is filed under, if any, and to every Model Card that links it, so it may be filed wherever it first connects; `emphasis` marks it `core` or `aside`. A Model Card (type model, no facet) may carry its model's elaboration `level`.",
            json!({
                "at": at,
                "kind": { "type": "string", "enum": ["direct", "side"], "description": "Default direct." },
                "title": { "type": "string", "description": "The Card title (one line)." },
                "body": body,
                "type": card_type("The Card type: idea (default), event, finding, model, or hub."),
                "facet": facet("Make the new Card a Model part presenting this facet of its model (implies type model)."),
                "emphasis": emphasis("For a Model part: core (the heart of the model, what a reader must see first) or aside (detail a reader may skip); omit for a regular part."),
                "level": level("For a Model Card (type model, no facet): concept (level 1, a concept model: stated in words, no Derivation parts) or formal (level 2, a formal model: self-sufficient, with a derivation and Derivation parts); omit when not yet decided. For a Hub Card (type hub): line, topic, field, or register."),
                "form": form("For a Model Card (type model, no facet) that states a mathematical unit: definition, axiom, theorem, proposition, lemma, or corollary; or theory for a Theory Card, the way into one theory. Omit for an ordinary model."),
            }),
            &["at", "title"],
            false,
            false,
        ),
        tool(
            "edit_card",
            "Edit card",
            "Change an agent Card. Pass `title` and/or `body` to replace just those parts, or `text` for the whole Markdown document (`# Title` then the body). Pass `type` to make a Regular Card an Idea, Event, Finding, Model, or Hub Card, `facet` to make a Model Card a Model part or rename its facet (an empty `facet` makes it a Model Card again), `emphasis` to mark a part core or aside (empty for regular), `level` to set a Model Card's elaboration level (concept or formal; empty for none), and `form` to set the mathematical unit a Model Card states (theory, definition, axiom, theorem, proposition, lemma, corollary; empty for none); each may be given alone. Reverse links are regenerated automatically.",
            json!({
                "at": at,
                "title": { "type": "string", "description": "New title (not for Literature Cards)." },
                "body": body,
                "type": card_type("New Card type: idea, event, finding, model, or hub (event, finding, model, and hub only for Regular Cards)."),
                "facet": facet("New Model facet (implies type model); empty for none."),
                "emphasis": emphasis("New emphasis of a Model part: core, aside, or empty for a regular part."),
                "level": level("New elaboration level of a Model Card (concept or formal), or new hub level of a Hub Card (line, topic, field, or register); empty for none."),
                "form": form("New Math form of a Model Card: theory, definition, axiom, theorem, proposition, lemma, or corollary; empty for none."),
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
            "Delete an agent Card and all its successors; deleting a tree root deletes the whole tree. The plan lists every Link from a Card that stays to a Card that goes (`links that will break`). When you delete a Card whose content now lives in another Card (a merged Model part), pass `into` with that Card: each of those Links is re-pointed to it, so none breaks. Returns a plan first; confirm with the text it names (`delete`, or the root's address).",
            json!({
                "at": at,
                "into": { "type": "string", "description": "A Card that stays and now holds the deleted Card's content: every Link to a deleted Card is re-pointed to it." },
                "confirm": confirm,
            }),
            &["at"],
            false,
            true,
        ),
        tool(
            "move_card",
            "Move card",
            "Move an agent Regular Card and its successors to a new Location in the agent Workspace, rewriting Links. Give `to` for a free slot, or `before` to move into the place of an existing Card: that Card and its subtree move down and follow the moved Card. With `alone: true` only the Card moves (it must have no Side successors) and its Direct successor chain moves up one place to close the gap; `alone` with `before` reorders one step of a chain in one call. All addresses are today's addresses. Literature tree Cards stay in their tree; nothing moves between Workspaces. Returns a plan of every renamed address first; confirm with `move`.",
            json!({
                "at": at,
                "to": { "type": "string", "description": "A free Location: the next direct or side successor slot of an existing Card." },
                "before": { "type": "string", "description": "An existing Regular Card whose place the moved Card takes; it moves down one place (or by the length of the moved chain)." },
                "alone": { "type": "boolean", "description": "Move only this Card, not its successors; its Direct successors move up to close the gap. Default false." },
                "confirm": confirm,
            }),
            &["at"],
            false,
            true,
        ),
        tool(
            "insert_card",
            "Insert card",
            "Insert a new Card between two Cards of an agent tree: it takes the place of the Regular Card `before`, which moves down one place with its whole subtree and becomes the new Card's Direct successor (`X/2` -> new Card, old `X/2` -> `X/3`, …; before `P|a` the old `P|a` becomes `P|a|1`). Use it to add a missing step to a chain, such as a Derivation part between two parts of a model. Takes title, body, type, facet, and emphasis as create_successor does; it has no `level` or `form`, so set a Model Card's level or Math form with edit_card after the insert. Write Links in today's addresses, and they follow the move. Returns a plan of every renamed address first; confirm with `insert`.",
            json!({
                "before": { "type": "string", "description": "The existing Regular Card whose place the new Card takes." },
                "title": { "type": "string", "description": "The Card title (one line)." },
                "body": body,
                "type": card_type("The Card type: idea (default), event, finding, model, or hub."),
                "facet": facet("Make the new Card a Model part presenting this facet of its model (implies type model); a Derivation part's facet starts with `推导`."),
                "emphasis": emphasis("For a Model part: core or aside; omit for a regular part."),
                "confirm": confirm,
            }),
            &["before", "title"],
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
            "Fetch the bytes of a PDF or Markdown document that Scholar Archive manages (a `file_id` from list_work_files or get_work), verify its SHA-256, and keep a copy in ZET's cache. Markdown and text come back as `text`; for a PDF, open the returned `path`. `link` is the Document link `[name](scholar:<file_id>)` that lets a Card's reader open the document in the GUI.",
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
            value["link"] = json!(crate::scholar::document_markdown(&copy.name, file_id));
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
    fn edit_patience_is_a_bounded_whole_number_of_seconds() {
        let default = std::time::Duration::from_secs(20);
        assert_eq!(patience_seconds(None), (default, None));
        assert_eq!(
            patience_seconds(Some("180")),
            (std::time::Duration::from_secs(180), None)
        );
        assert_eq!(
            patience_seconds(Some(" 1 ")),
            (std::time::Duration::from_secs(1), None)
        );
        assert_eq!(
            patience_seconds(Some("300")),
            (std::time::Duration::from_secs(300), None)
        );
        // An unreadable or out-of-range value keeps the ordinary short bound instead of hanging
        // a write, and its one note repeats the refused value and the default that applied.
        for value in ["0", "301", "-5", "1.5", "abc", ""] {
            let (patience, note) = patience_seconds(Some(value));
            assert_eq!(patience, default, "{value}");
            let note = note.unwrap_or_else(|| panic!("{value} is refused with a note"));
            assert!(note.contains(&format!("ZT_EDIT_PATIENCE={value}")), "{note}");
            assert!(note.contains("1 through 300"), "{note}");
            assert!(note.contains("default 20"), "{note}");
        }
    }

    #[test]
    fn insert_card_names_the_create_successor_fields_it_does_not_take() {
        let tools = tool_definitions();
        let find = |name: &str| {
            tools
                .iter()
                .find(|tool| tool["name"] == name)
                .unwrap()
                .clone()
        };
        let (insert, create) = (find("insert_card"), find("create_successor"));
        let insert_fields = insert["inputSchema"]["properties"].as_object().unwrap();
        let description = insert["description"].as_str().unwrap();
        for field in create["inputSchema"]["properties"]
            .as_object()
            .unwrap()
            .keys()
        {
            if ["at", "kind"].contains(&field.as_str()) || insert_fields.contains_key(field) {
                continue;
            }
            assert!(
                description.contains(&format!("`{field}`")) && description.contains("edit_card"),
                "insert_card has no `{field}` and its description does not send it to edit_card: {description}"
            );
        }
    }

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
